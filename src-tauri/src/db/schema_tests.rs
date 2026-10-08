//! マイグレーションで作るスキーマのテスト（ADR 0017）。データは全て合成したもの。

use rusqlite::{Connection, params};

use super::migrate;

fn database() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    migrate::migrate(&mut conn, migrate::MIGRATIONS).unwrap();
    conn
}

/// ADR 0005 の novel レコードから作る行
fn insert_novel(conn: &Connection, site: &str, site_id: &str) -> i64 {
    conn.execute(
        "INSERT INTO novels (site, site_id, url, title, author_name, author_url, description,
                             episode_count, is_concluded, metadata_fetched_at)
         VALUES (?1, ?2, ?3, '合成データの作品', '見本 太郎', 'https://example.com/authors/1/',
                 'あらすじ', 2, 1, '2026-01-01T00:00:00Z')",
        params![
            site,
            site_id,
            format!("https://example.com/{site}/{site_id}/")
        ],
    )
    .unwrap();
    conn.last_insert_rowid()
}

/// ADR 0005 の toc_entry レコードから作る行
fn insert_episode(conn: &Connection, novel_id: i64, no: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO episodes (novel_id, no, url, title, chapter, published_at, revised_at)
         VALUES (?1, ?2, ?3, ?4, '第一章', '2026-01-01T00:00:00Z', NULL)",
        params![
            novel_id,
            no,
            format!("https://example.com/{novel_id}/{no}/"),
            format!("第{no}話")
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

#[test]
fn stores_and_reads_back_novel() {
    let conn = database();
    let id = insert_novel(&conn, "narou", "n0001aa");

    let row: (
        String,
        String,
        String,
        Option<String>,
        i64,
        Option<bool>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT site, site_id, author_name, author_url, episode_count, is_concluded,
                    normalize_options
             FROM novels WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        row,
        (
            "narou".into(),
            "n0001aa".into(),
            "見本 太郎".into(),
            Some("https://example.com/authors/1/".into()),
            2,
            Some(true),
            None
        )
    );
}

#[test]
fn allows_unknown_concluded_and_author_url() {
    let conn = database();
    let id = insert_novel(&conn, "hameln", "100001");
    conn.execute(
        "UPDATE novels SET is_concluded = NULL, author_url = NULL WHERE id = ?1",
        [id],
    )
    .unwrap();
    let row: (Option<bool>, Option<String>) = conn
        .query_row(
            "SELECT is_concluded, author_url FROM novels WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(row, (None, None));
}

#[test]
fn stores_normalize_options_as_json() {
    let conn = database();
    let id = insert_novel(&conn, "narou", "n0001aa");
    let options = r#"{"direction":"horizontal","insertIndent":false}"#;
    conn.execute(
        "UPDATE novels SET normalize_options = ?1 WHERE id = ?2",
        params![options, id],
    )
    .unwrap();
    let direction: String = conn
        .query_row(
            "SELECT normalize_options ->> '$.direction' FROM novels WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(direction, "horizontal");

    let invalid = conn.execute(
        "UPDATE novels SET normalize_options = 'not json' WHERE id = ?1",
        [id],
    );
    assert!(invalid.is_err());
}

#[test]
fn stores_and_reads_back_episode_body() {
    let conn = database();
    let novel = insert_novel(&conn, "kakuyomu", "1000000000000000001");
    let episode = insert_episode(&conn, novel, 1).unwrap();
    assert!(
        conn.query_row(
            "SELECT body_fetched_at FROM episodes WHERE id = ?1",
            [episode],
            |r| r.get::<_, Option<String>>(0)
        )
        .unwrap()
        .is_none()
    );

    // ADR 0005 の episode レコードから埋める
    conn.execute(
        "UPDATE episodes
         SET body = ?1, preface = NULL, afterword = '後書き', char_count = 3,
             body_fetched_at = '2026-01-02T00:00:00Z', body_revised_at = revised_at
         WHERE id = ?2",
        params!["　本文。", episode],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO episode_images (episode_id, position, url) VALUES (?1, 1, 'https://example.com/a.png')",
        [episode],
    )
    .unwrap();

    let row: (String, Option<String>, String, i64) = conn
        .query_row(
            "SELECT body, preface, afterword, char_count FROM episodes WHERE id = ?1",
            [episode],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(row, ("　本文。".into(), None, "後書き".into(), 3));
}

#[test]
fn stores_and_reads_back_image() {
    let conn = database();
    let data: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0, 1, 2];
    conn.execute(
        "INSERT INTO images (url, content_type, data, byte_length, fetched_at)
         VALUES ('https://example.com/a.png', 'image/png', ?1, ?2, '2026-01-01T00:00:00Z')",
        params![data, data.len() as i64],
    )
    .unwrap();
    let stored: Vec<u8> = conn
        .query_row(
            "SELECT data FROM images WHERE url = 'https://example.com/a.png'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, data);
}

#[test]
fn enforces_uniqueness() {
    let conn = database();
    let novel = insert_novel(&conn, "narou", "n0001aa");
    // 同じサイトの同じ作品
    assert!(
        conn.execute(
            "INSERT INTO novels (site, site_id, url, title, author_name, description, episode_count,
                                 metadata_fetched_at)
             VALUES ('narou', 'n0001aa', 'u', 't', 'a', 'd', 0, 'x')",
            [],
        )
        .is_err()
    );
    // 別のサイトなら同じ ID でもよい
    insert_novel(&conn, "novel18", "n0001aa");

    insert_episode(&conn, novel, 1).unwrap();
    // 同じ作品の同じ位置
    assert!(insert_episode(&conn, novel, 1).is_err());
    // 同じ URL の話
    assert!(
        conn.execute(
            "INSERT INTO episodes (novel_id, no, url, title) VALUES (?1, 2, ?2, 't')",
            params![novel, format!("https://example.com/{novel}/1/")],
        )
        .is_err()
    );
}

#[test]
fn deleting_novel_cascades_to_episodes_but_keeps_images() {
    let conn = database();
    let novel = insert_novel(&conn, "narou", "n0001aa");
    let episode = insert_episode(&conn, novel, 1).unwrap();
    conn.execute(
        "INSERT INTO episode_images (episode_id, position, url) VALUES (?1, 1, 'https://example.com/a.png')",
        [episode],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO images (url, content_type, data, byte_length, fetched_at)
         VALUES ('https://example.com/a.png', 'image/png', x'00', 1, 'x')",
        [],
    )
    .unwrap();

    conn.execute("DELETE FROM novels WHERE id = ?1", [novel])
        .unwrap();
    assert_eq!(count(&conn, "episodes"), 0);
    assert_eq!(count(&conn, "episode_images"), 0);
    assert_eq!(count(&conn, "images"), 1);
}

#[test]
fn rejects_values_out_of_range() {
    let conn = database();
    let insert_site = |site: &str| {
        conn.execute(
            "INSERT INTO novels (site, site_id, url, title, author_name, description, episode_count,
                                 metadata_fetched_at)
             VALUES (?1, 'x', 'u', 't', 'a', 'd', 0, 'x')",
            [site],
        )
    };
    assert!(insert_site("example").is_err());

    let novel = insert_novel(&conn, "narou", "n0001aa");
    assert!(insert_episode(&conn, novel, 0).is_err());
    assert!(
        conn.execute("UPDATE novels SET is_concluded = 2 WHERE id = ?1", [novel])
            .is_err()
    );
    assert!(
        conn.execute(
            "UPDATE novels SET episode_count = -1 WHERE id = ?1",
            [novel]
        )
        .is_err()
    );
    // データの長さと合わない byte_length
    assert!(
        conn.execute(
            "INSERT INTO images (url, content_type, data, byte_length, fetched_at)
             VALUES ('u', 'image/png', x'0001', 3, 'x')",
            [],
        )
        .is_err()
    );
    // 存在しない作品の話
    assert!(insert_episode(&conn, 9999, 1).is_err());
}

#[test]
fn rejects_values_of_wrong_type() {
    let conn = database();
    let novel = insert_novel(&conn, "narou", "n0001aa");
    // STRICT のため、数値の列に数値でない文字列は入らない
    assert!(
        conn.execute(
            "UPDATE novels SET episode_count = 'many' WHERE id = ?1",
            [novel]
        )
        .is_err()
    );
    // 文字列の列は数値を文字列に直して受け入れるが、BLOB は受け入れない
    assert!(
        conn.execute("UPDATE novels SET title = x'00' WHERE id = ?1", [novel])
            .is_err()
    );
}
