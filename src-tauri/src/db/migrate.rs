//! SQLite のマイグレーション（ADR 0013）。
//!
//! `migrations/NNNN_name.sql` をビルド時に埋め込み、適用済みの版を `PRAGMA user_version` で持つ。
//! 1 つのマイグレーションは 1 つのトランザクションで適用し、失敗したら版を進めない。

use std::fmt;

use rusqlite::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

impl fmt::Display for Migration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}_{}", self.version, self.name)
    }
}

/// 埋め込んだマイグレーション。版は 1 からの連番であることを build.rs が保証する
pub const MIGRATIONS: &[Migration] = include!(concat!(env!("OUT_DIR"), "/migrations.rs"));

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    /// データベースの版が、アプリの知っている最新の版より新しい
    NewerThanApp {
        database: u32,
        app: u32,
    },
    /// マイグレーションの SQL の実行に失敗した
    Failed {
        migration: String,
        source: rusqlite::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "{e}"),
            Error::NewerThanApp { database, app } => write!(
                f,
                "database version {database} is newer than the latest migration {app}"
            ),
            Error::Failed { migration, source } => {
                write!(f, "migration {migration} failed: {source}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Sqlite(e) | Error::Failed { source: e, .. } => Some(e),
            Error::NewerThanApp { .. } => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

/// 適用済みの版。新しいデータベースでは 0
pub fn current_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
}

fn latest_version(migrations: &[Migration]) -> u32 {
    migrations.last().map_or(0, |m| m.version)
}

/// まだ適用していないマイグレーション
pub fn pending<'a>(
    conn: &Connection,
    migrations: &'a [Migration],
) -> Result<&'a [Migration], Error> {
    let current = current_version(conn)?;
    let latest = latest_version(migrations);
    if current > latest {
        return Err(Error::NewerThanApp {
            database: current,
            app: latest,
        });
    }
    Ok(&migrations[current as usize..])
}

/// 未適用のマイグレーションを版の順に適用し、適用したものを返す
pub fn migrate<'a>(
    conn: &mut Connection,
    migrations: &'a [Migration],
) -> Result<&'a [Migration], Error> {
    let pending = pending(conn, migrations)?;
    for migration in pending {
        let tx = conn.transaction()?;
        tx.execute_batch(migration.sql)
            .map_err(|source| Error::Failed {
                migration: migration.to_string(),
                source,
            })?;
        tx.pragma_update(None, "user_version", migration.version)?;
        tx.commit()?;
    }
    Ok(pending)
}

/// 次に作るマイグレーションのファイル名。`existing` は `migrations/` にあるファイル名
pub fn next_file_name<S: AsRef<str>>(existing: &[S], name: &str) -> Result<String, String> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(format!(
            "migration name must be snake_case ([a-z0-9_]+): {name:?}"
        ));
    }
    let latest = existing
        .iter()
        .filter_map(|file| file.as_ref().strip_suffix(".sql"))
        .filter_map(|stem| stem.split_once('_'))
        .filter_map(|(version, _)| version.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    Ok(format!("{:04}_{name}.sql", latest + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &[Migration] = &[
        Migration {
            version: 1,
            name: "create_a",
            sql: "CREATE TABLE a (id INTEGER PRIMARY KEY);",
        },
        Migration {
            version: 2,
            name: "create_b",
            sql: "CREATE TABLE b (id INTEGER PRIMARY KEY);",
        },
    ];

    fn tables(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn applies_all_migrations_to_new_database() {
        let mut conn = Connection::open_in_memory().unwrap();
        let applied = migrate(&mut conn, TWO).unwrap();
        assert_eq!(applied, TWO);
        assert_eq!(current_version(&conn).unwrap(), 2);
        assert_eq!(tables(&conn), ["a", "b"]);
    }

    #[test]
    fn applies_only_pending_migrations() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn, &TWO[..1]).unwrap();
        let applied = migrate(&mut conn, TWO).unwrap();
        assert_eq!(applied, &TWO[1..]);
        assert_eq!(current_version(&conn).unwrap(), 2);
    }

    #[test]
    fn does_nothing_when_up_to_date() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn, TWO).unwrap();
        assert!(migrate(&mut conn, TWO).unwrap().is_empty());
        assert_eq!(current_version(&conn).unwrap(), 2);
    }

    #[test]
    fn rolls_back_failed_migration() {
        const BROKEN: &[Migration] = &[
            TWO[0],
            Migration {
                version: 2,
                name: "broken",
                sql: "CREATE TABLE c (id INTEGER); THIS IS NOT SQL;",
            },
        ];
        let mut conn = Connection::open_in_memory().unwrap();
        let err = migrate(&mut conn, BROKEN).unwrap_err();
        assert!(matches!(err, Error::Failed { ref migration, .. } if migration == "0002_broken"));
        // 失敗したマイグレーションの途中までの変更は残らず、版も進まない
        assert_eq!(current_version(&conn).unwrap(), 1);
        assert_eq!(tables(&conn), ["a"]);
    }

    #[test]
    fn rejects_database_newer_than_app() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn, TWO).unwrap();
        let err = migrate(&mut conn, &TWO[..1]).unwrap_err();
        assert!(matches!(
            err,
            Error::NewerThanApp {
                database: 2,
                app: 1
            }
        ));
    }

    #[test]
    fn embedded_migrations_are_consecutive() {
        for (i, migration) in MIGRATIONS.iter().enumerate() {
            assert_eq!(migration.version, i as u32 + 1);
        }
    }

    #[test]
    fn next_file_name_follows_latest_version() {
        assert_eq!(
            next_file_name::<&str>(&[], "create_novels").unwrap(),
            "0001_create_novels.sql"
        );
        assert_eq!(
            next_file_name(&["0001_a.sql", ".gitkeep", "0002_b.sql"], "add_index").unwrap(),
            "0003_add_index.sql"
        );
    }

    #[test]
    fn next_file_name_rejects_invalid_name() {
        assert!(next_file_name::<&str>(&[], "").is_err());
        assert!(next_file_name::<&str>(&[], "Create Novels").is_err());
        assert!(next_file_name::<&str>(&[], "create-novels").is_err());
    }
}
