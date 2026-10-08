//! 画面に返すための読み出し（ADR 0018）。一覧では本文の列を読まない。

use std::collections::BTreeMap;

use base64::Engine;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use serde_json::Value;

fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelSummary {
    pub id: i64,
    pub site: String,
    pub site_id: String,
    pub url: String,
    pub title: String,
    pub author_name: String,
    pub author_url: Option<String>,
    pub is_concluded: Option<bool>,
    /// 最新話の公開日時
    pub latest_published_at: Option<String>,
    /// 本文を取得済みの話数
    pub fetched: i64,
    /// 目次の話数
    pub total: i64,
}

const SUMMARY_COLUMNS: &str =
    "n.id, n.site, n.site_id, n.url, n.title, n.author_name, n.author_url,
    n.is_concluded,
    (SELECT max(published_at) FROM episodes e WHERE e.novel_id = n.id) AS latest_published_at,
    (SELECT count(*) FROM episodes e WHERE e.novel_id = n.id AND e.body_fetched_at IS NOT NULL),
    (SELECT count(*) FROM episodes e WHERE e.novel_id = n.id)";

fn summary(row: &Row) -> rusqlite::Result<NovelSummary> {
    Ok(NovelSummary {
        id: row.get(0)?,
        site: row.get(1)?,
        site_id: row.get(2)?,
        url: row.get(3)?,
        title: row.get(4)?,
        author_name: row.get(5)?,
        author_url: row.get(6)?,
        is_concluded: row.get(7)?,
        latest_published_at: row.get(8)?,
        fetched: row.get(9)?,
        total: row.get(10)?,
    })
}

/// 作品の一覧。最新話の新しい順
pub fn list_novels(conn: &Connection) -> Result<Vec<NovelSummary>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SUMMARY_COLUMNS} FROM novels n
             ORDER BY latest_published_at IS NULL, latest_published_at DESC, n.id"
        ))
        .map_err(sql)?;
    stmt.query_map([], summary)
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeSummary {
    pub id: i64,
    pub novel_id: i64,
    pub no: i64,
    pub url: String,
    pub title: String,
    pub chapter: Option<String>,
    pub published_at: Option<String>,
    pub revised_at: Option<String>,
    pub body_fetched_at: Option<String>,
    pub sent_at: Option<String>,
}

const EPISODE_COLUMNS: &str = "e.id, e.novel_id, e.no, e.url, e.title, e.chapter, e.published_at, e.revised_at, e.body_fetched_at, e.sent_at";

fn episode_summary(row: &Row) -> rusqlite::Result<EpisodeSummary> {
    Ok(EpisodeSummary {
        id: row.get(0)?,
        novel_id: row.get(1)?,
        no: row.get(2)?,
        url: row.get(3)?,
        title: row.get(4)?,
        chapter: row.get(5)?,
        published_at: row.get(6)?,
        revised_at: row.get(7)?,
        body_fetched_at: row.get(8)?,
        sent_at: row.get(9)?,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelDetail {
    #[serde(flatten)]
    pub summary: NovelSummary,
    pub description: String,
    /// 取得済みの話の文字数の合計
    pub char_count: i64,
    pub metadata_fetched_at: String,
    /// 整形の設定。null なら既定
    pub normalize_options: Option<Value>,
    pub episodes: Vec<EpisodeSummary>,
}

pub fn novel_detail(conn: &Connection, novel_id: i64) -> Result<Option<NovelDetail>, String> {
    let head = conn
        .query_row(
            &format!(
                "SELECT {SUMMARY_COLUMNS}, n.description,
                        (SELECT coalesce(sum(char_count), 0) FROM episodes e WHERE e.novel_id = n.id),
                        n.metadata_fetched_at, n.normalize_options
                 FROM novels n WHERE n.id = ?1"
            ),
            [novel_id],
            |row| {
                let options: Option<String> = row.get(14)?;
                Ok((
                    summary(row)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, i64>(12)?,
                    row.get::<_, String>(13)?,
                    options,
                ))
            },
        )
        .optional()
        .map_err(sql)?;
    let Some((summary, description, char_count, metadata_fetched_at, options)) = head else {
        return Ok(None);
    };
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {EPISODE_COLUMNS} FROM episodes e WHERE e.novel_id = ?1 ORDER BY e.no"
        ))
        .map_err(sql)?;
    let episodes = stmt
        .query_map([novel_id], episode_summary)
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)?;
    Ok(Some(NovelDetail {
        summary,
        description,
        char_count,
        metadata_fetched_at,
        normalize_options: options
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| e.to_string())?,
        episodes,
    }))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeDetail {
    #[serde(flatten)]
    pub summary: EpisodeSummary,
    pub preface: Option<String>,
    pub body: Option<String>,
    pub afterword: Option<String>,
    pub char_count: Option<i64>,
    /// 取得済みの挿絵。URL から data URL へ
    pub images: BTreeMap<String, String>,
}

pub fn episode_detail(conn: &Connection, episode_id: i64) -> Result<Option<EpisodeDetail>, String> {
    let row = conn
        .query_row(
            &format!(
                "SELECT {EPISODE_COLUMNS}, e.preface, e.body, e.afterword, e.char_count
                 FROM episodes e WHERE e.id = ?1"
            ),
            [episode_id],
            |row| {
                Ok(EpisodeDetail {
                    summary: episode_summary(row)?,
                    preface: row.get(10)?,
                    body: row.get(11)?,
                    afterword: row.get(12)?,
                    char_count: row.get(13)?,
                    images: BTreeMap::new(),
                })
            },
        )
        .optional()
        .map_err(sql)?;
    let Some(mut detail) = row else {
        return Ok(None);
    };
    let mut stmt = conn
        .prepare(
            "SELECT i.url, i.content_type, i.data FROM episode_images ei
             JOIN images i ON i.url = ei.url
             WHERE ei.episode_id = ?1",
        )
        .map_err(sql)?;
    let images = stmt
        .query_map([episode_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })
        .map_err(sql)?;
    for image in images {
        let (url, content_type, data) = image.map_err(sql)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        detail
            .images
            .insert(url, format!("data:{content_type};base64,{encoded}"));
    }
    Ok(Some(detail))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestEpisode {
    pub novel_id: i64,
    pub site: String,
    pub site_id: String,
    pub novel_url: String,
    pub novel_title: String,
    pub episode: EpisodeSummary,
}

/// 全作品の話を、公開日時の新しい順に `limit` 件
pub fn latest_episodes(conn: &Connection, limit: i64) -> Result<Vec<LatestEpisode>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {EPISODE_COLUMNS}, n.site, n.site_id, n.url, n.title
             FROM episodes e JOIN novels n ON n.id = e.novel_id
             WHERE e.published_at IS NOT NULL
             ORDER BY e.published_at DESC, e.id DESC
             LIMIT ?1"
        ))
        .map_err(sql)?;
    stmt.query_map([limit], |row| {
        Ok(LatestEpisode {
            episode: episode_summary(row)?,
            novel_id: row.get(1)?,
            site: row.get(10)?,
            site_id: row.get(11)?,
            novel_url: row.get(12)?,
            novel_title: row.get(13)?,
        })
    })
    .map_err(sql)?
    .collect::<Result<_, _>>()
    .map_err(sql)
}

/// 作品の URL を取得し直すための情報（サイトと作品 ID）
pub fn novel_site(conn: &Connection, novel_id: i64) -> Result<Option<(String, String)>, String> {
    conn.query_row(
        "SELECT site, site_id FROM novels WHERE id = ?1",
        [novel_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(sql)
}

pub fn episode_url(conn: &Connection, episode_id: i64) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT url FROM episodes WHERE id = ?1",
        [episode_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(sql)
}

/// 整形の設定を保存する。null なら既定に戻す
pub fn set_normalize_options(
    conn: &Connection,
    novel_id: i64,
    options: Option<&Value>,
) -> Result<(), String> {
    let changed = conn
        .execute(
            "UPDATE novels SET normalize_options = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2",
            params![options.map(Value::to_string), novel_id],
        )
        .map_err(sql)?;
    if changed == 0 {
        return Err(format!("no novel with id {novel_id}"));
    }
    Ok(())
}

/// 作品の話を全て消す（kindlize の remove episodes）。作品と挿絵は残す
pub fn remove_episodes(conn: &Connection, novel_id: i64) -> Result<usize, String> {
    conn.execute("DELETE FROM episodes WHERE novel_id = ?1", [novel_id])
        .map_err(sql)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::records::{parse, tests as fixtures};
    use crate::library::store::{save_episode, save_image, save_toc};
    use crate::subscriptions::{Site, Subscription};

    fn database() -> (Connection, i64) {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        crate::db::migrate::migrate(&mut conn, crate::db::migrate::MIGRATIONS).unwrap();
        let toc = [
            fixtures::NOVEL.to_owned(),
            fixtures::toc_entry(1, None),
            fixtures::toc_entry(2, None),
        ]
        .join("\n");
        let id = save_toc(
            &mut conn,
            &Subscription {
                site: Site::Narou,
                id: "n0001aa".into(),
            },
            &parse(&toc).unwrap(),
        )
        .unwrap();
        save_episode(
            &mut conn,
            &parse(&fixtures::episode(
                1,
                &["https://example.com/a.png", "https://example.com/b.png"],
            ))
            .unwrap(),
        )
        .unwrap();
        save_image(
            &conn,
            &parse(&fixtures::image("https://example.com/a.png", b"png")).unwrap(),
        )
        .unwrap();
        (conn, id)
    }

    #[test]
    fn lists_novels_with_counts() {
        let (conn, id) = database();
        let novels = list_novels(&conn).unwrap();
        assert_eq!(novels.len(), 1);
        let novel = &novels[0];
        assert_eq!((novel.id, novel.fetched, novel.total), (id, 1, 2));
        assert_eq!(
            novel.latest_published_at.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert_eq!(novel.is_concluded, Some(false));
    }

    #[test]
    fn reads_novel_detail_with_episodes() {
        let (conn, id) = database();
        let detail = novel_detail(&conn, id).unwrap().unwrap();
        assert_eq!(detail.char_count, 3);
        assert_eq!(detail.normalize_options, None);
        assert_eq!(
            detail.episodes.iter().map(|e| e.no).collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(detail.episodes[0].body_fetched_at.is_some());
        assert!(detail.episodes[1].body_fetched_at.is_none());
        assert_eq!(novel_detail(&conn, 9999).unwrap(), None);
    }

    #[test]
    fn reads_episode_with_stored_images_only() {
        let (conn, id) = database();
        let episode_id = novel_detail(&conn, id).unwrap().unwrap().episodes[0].id;
        let detail = episode_detail(&conn, episode_id).unwrap().unwrap();
        assert_eq!(detail.body.as_deref(), Some("　本文。"));
        assert_eq!(
            detail.images.keys().collect::<Vec<_>>(),
            ["https://example.com/a.png"]
        );
        assert_eq!(
            detail.images["https://example.com/a.png"],
            "data:image/png;base64,cG5n"
        );
    }

    #[test]
    fn lists_latest_episodes() {
        let (conn, id) = database();
        let latest = latest_episodes(&conn, 1).unwrap();
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].novel_id, id);
        assert_eq!(latest[0].novel_title, "合成データの作品");
    }

    #[test]
    fn saves_and_resets_normalize_options() {
        let (conn, id) = database();
        let options = serde_json::json!({"direction": "horizontal"});
        set_normalize_options(&conn, id, Some(&options)).unwrap();
        assert_eq!(
            novel_detail(&conn, id).unwrap().unwrap().normalize_options,
            Some(options)
        );
        set_normalize_options(&conn, id, None).unwrap();
        assert_eq!(
            novel_detail(&conn, id).unwrap().unwrap().normalize_options,
            None
        );
        assert!(set_normalize_options(&conn, 9999, None).is_err());
    }

    /// 作品を 1 つ足す。`published` は各話の公開日時（None なら公開日時のない話）
    fn add_novel(conn: &Connection, site_id: &str, published: &[Option<&str>]) -> i64 {
        conn.execute(
            "INSERT INTO novels (site, site_id, url, title, author_name, description, episode_count,
                                 metadata_fetched_at)
             VALUES ('narou', ?1, ?2, ?1, 'a', 'd', ?3, '2026-01-01T00:00:00Z')",
            params![site_id, format!("https://example.com/{site_id}/"), published.len() as i64],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        for (i, at) in published.iter().enumerate() {
            conn.execute(
                "INSERT INTO episodes (novel_id, no, url, title, published_at) VALUES (?1, ?2, ?3, 't', ?4)",
                params![id, i as i64 + 1, format!("https://example.com/{site_id}/{}/", i + 1), at],
            )
            .unwrap();
        }
        id
    }

    fn empty_database() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        crate::db::migrate::migrate(&mut conn, crate::db::migrate::MIGRATIONS).unwrap();
        conn
    }

    #[test]
    fn orders_novels_by_latest_episode_and_puts_empty_ones_last() {
        let conn = empty_database();
        let old = add_novel(&conn, "n0001aa", &[Some("2026-01-01T00:00:00Z")]);
        let empty = add_novel(&conn, "n0002bb", &[]);
        let new = add_novel(
            &conn,
            "n0003cc",
            &[Some("2025-12-01T00:00:00Z"), Some("2026-02-01T00:00:00Z")],
        );
        let unknown = add_novel(&conn, "n0004dd", &[None]);

        let ids: Vec<i64> = list_novels(&conn).unwrap().iter().map(|n| n.id).collect();
        assert_eq!(ids, [new, old, empty, unknown]);
    }

    #[test]
    fn latest_spans_novels_skips_unknown_dates_and_limits() {
        let conn = empty_database();
        add_novel(
            &conn,
            "n0001aa",
            &[
                Some("2026-01-01T00:00:00Z"),
                Some("2026-01-03T00:00:00Z"),
                None,
            ],
        );
        add_novel(&conn, "n0002bb", &[Some("2026-01-02T00:00:00Z")]);

        let all = latest_episodes(&conn, 10).unwrap();
        let dates: Vec<&str> = all
            .iter()
            .map(|e| e.episode.published_at.as_deref().unwrap())
            .collect();
        assert_eq!(
            dates,
            [
                "2026-01-03T00:00:00Z",
                "2026-01-02T00:00:00Z",
                "2026-01-01T00:00:00Z"
            ]
        );
        assert_eq!(all[1].site_id, "n0002bb");

        assert_eq!(latest_episodes(&conn, 2).unwrap().len(), 2);
    }

    #[test]
    fn returns_none_for_missing_rows() {
        let conn = empty_database();
        assert_eq!(episode_detail(&conn, 1).unwrap(), None);
        assert_eq!(novel_site(&conn, 1).unwrap(), None);
        assert_eq!(episode_url(&conn, 1).unwrap(), None);
        assert_eq!(remove_episodes(&conn, 1).unwrap(), 0);
    }

    #[test]
    fn removes_episodes_but_keeps_novel() {
        let (conn, id) = database();
        assert_eq!(remove_episodes(&conn, id).unwrap(), 2);
        let detail = novel_detail(&conn, id).unwrap().unwrap();
        assert!(detail.episodes.is_empty());
    }
}
