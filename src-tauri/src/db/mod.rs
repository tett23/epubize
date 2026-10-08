//! epubize の SQLite（ADR 0003、ADR 0013、ADR 0014）。

pub mod migrate;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::{fmt, fs, io};

use rusqlite::Connection;

use crate::environment::Environment;

/// `tauri.conf.json` の `identifier` と同じ値。アプリ用データ領域のディレクトリ名になる
pub const IDENTIFIER: &str = "com.github.tett23.epubize";

pub const FILENAME: &str = "epubize.sqlite3";

/// アプリが使うデータベースのパス。Tauri の `app_data_dir` の下に、環境ごとのディレクトリを作って置く
pub fn default_path(env: Environment) -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join(IDENTIFIER).join(env.dir_name()).join(FILENAME))
}

/// アプリが持つデータベースへの接続
pub struct Database(pub Mutex<Connection>);

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Migrate(migrate::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Migrate(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Migrate(e) => Some(e),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<migrate::Error> for Error {
    fn from(e: migrate::Error) -> Self {
        Error::Migrate(e)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Migrate(e.into())
    }
}

/// 接続を開くだけで、マイグレーションはしない。ファイルとディレクトリがなければ作る
pub fn connect(path: &Path) -> Result<Connection, Error> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    // journal_mode は設定後の値を 1 行返すため、pragma_update ではなく行を読む
    let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    Ok(conn)
}

/// 接続を開き、未適用のマイグレーションを全て適用する。
/// ファイルがなければ新しく作るため、消えていた場合は全てのマイグレーションが流れる
pub fn open(path: &Path) -> Result<Connection, Error> {
    let mut conn = connect(path)?;
    migrate::migrate(&mut conn, migrate::MIGRATIONS)?;
    Ok(conn)
}

/// データベースのファイルを、WAL のファイルも含めて消す。ないファイルは無視する
pub fn remove(path: &Path) -> Result<(), Error> {
    for suffix in ["", "-wal", "-shm"] {
        let mut file = path.as_os_str().to_owned();
        file.push(suffix);
        match fs::remove_file(&file) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_missing_directory_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join(FILENAME);

        let conn = open(&path).unwrap();
        assert!(path.exists());
        assert_eq!(
            migrate::current_version(&conn).unwrap(),
            migrate::MIGRATIONS.len() as u32
        );
    }

    #[test]
    fn open_recreates_removed_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILENAME);

        drop(open(&path).unwrap());
        remove(&path).unwrap();
        assert!(!path.exists());

        let conn = open(&path).unwrap();
        assert!(path.exists());
        assert_eq!(
            migrate::current_version(&conn).unwrap(),
            migrate::MIGRATIONS.len() as u32
        );
    }

    #[test]
    fn enables_foreign_keys_and_wal() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(&dir.path().join(FILENAME)).unwrap();
        let foreign_keys: bool = conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        let journal_mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert!(foreign_keys);
        assert_eq!(journal_mode, "wal");
    }

    #[test]
    fn remove_ignores_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        remove(&dir.path().join(FILENAME)).unwrap();
    }

    #[test]
    fn default_path_separates_environments() {
        let dev = default_path(Environment::Development).unwrap();
        let prod = default_path(Environment::Production).unwrap();
        assert!(dev.ends_with(format!("{IDENTIFIER}/development/{FILENAME}")));
        assert!(prod.ends_with(format!("{IDENTIFIER}/production/{FILENAME}")));
    }

    #[test]
    fn identifier_matches_tauri_config() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        assert_eq!(config["identifier"], IDENTIFIER);
    }
}
