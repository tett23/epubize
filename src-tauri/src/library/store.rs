//! 取得した結果を保存する（ADR 0017、ADR 0018）。

use base64::Engine;
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use super::records::{EpisodeRecord, ImageRecord, Record};
use crate::subscriptions::Subscription;

/// 目次を入れ直す間、既存の話の位置を退避させる量。目次の話数はこれより十分少ない
const NO_OFFSET: i64 = 1_000_000;

/// 保存する日時。UTC に揃え、文字列のままで前後を比べられるようにする
pub(crate) fn utc(value: &str) -> Result<String, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|t| {
            t.with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, true)
        })
        .map_err(|e| format!("invalid date-time {value:?}: {e}"))
}

fn utc_opt(value: Option<&str>) -> Result<Option<String>, String> {
    value.map(utc).transpose()
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

/// `toc` コマンドの結果を保存し、作品の id を返す。
/// 作品は購読と同じ `site` と `site_id` で対応させる。目次から消えた話は消す
pub fn save_toc(
    conn: &mut Connection,
    subscription: &Subscription,
    records: &[Record],
) -> Result<i64, String> {
    let mut novels = records.iter().filter_map(|r| match r {
        Record::Novel(n) => Some(n),
        _ => None,
    });
    let novel = novels.next().ok_or("the toc output has no novel record")?;
    if novels.next().is_some() {
        return Err("the toc output has more than one novel record".into());
    }

    let tx = conn.transaction().map_err(sql)?;
    let now = now();
    let novel_id: i64 = tx
        .query_row(
            "INSERT INTO novels (site, site_id, url, title, author_name, author_url, description,
                                 episode_count, is_concluded, metadata_fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT (site, site_id) DO UPDATE SET
                 url = excluded.url, title = excluded.title,
                 author_name = excluded.author_name, author_url = excluded.author_url,
                 description = excluded.description, episode_count = excluded.episode_count,
                 is_concluded = excluded.is_concluded,
                 metadata_fetched_at = excluded.metadata_fetched_at, updated_at = ?11
             RETURNING id",
            params![
                subscription.site.key(),
                subscription.id,
                novel.url,
                novel.title,
                novel.author.name,
                novel.author.url,
                novel.description,
                novel.episode_count,
                novel.is_concluded,
                utc(&novel.fetched_at)?,
                now,
            ],
            |row| row.get(0),
        )
        .map_err(sql)?;

    // 位置の一意性に触れないよう、既存の話の位置を退避させてから入れ直す
    tx.execute(
        "UPDATE episodes SET no = no + ?1 WHERE novel_id = ?2",
        params![NO_OFFSET, novel_id],
    )
    .map_err(sql)?;
    for entry in records.iter().filter_map(|r| match r {
        Record::TocEntry(t) => Some(t),
        _ => None,
    }) {
        tx.execute(
            "INSERT INTO episodes (novel_id, no, url, title, chapter, published_at, revised_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (url) DO UPDATE SET
                 novel_id = excluded.novel_id, no = excluded.no, title = excluded.title,
                 chapter = excluded.chapter, published_at = excluded.published_at,
                 revised_at = excluded.revised_at, updated_at = ?8",
            params![
                novel_id,
                entry.no,
                entry.url,
                entry.title,
                entry.chapter,
                utc_opt(entry.published_at.as_deref())?,
                utc_opt(entry.revised_at.as_deref())?,
                now,
            ],
        )
        .map_err(sql)?;
    }
    tx.execute(
        "DELETE FROM episodes WHERE novel_id = ?1 AND no >= ?2",
        params![novel_id, NO_OFFSET],
    )
    .map_err(sql)?;
    tx.commit().map_err(sql)?;
    Ok(novel_id)
}

/// 本文を取得すべき話の URL。未取得か、目次の改稿日時が本文を取得したときより新しいもの
pub fn episodes_to_fetch(conn: &Connection, novel_id: i64) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT url FROM episodes
             WHERE novel_id = ?1
               AND (body_fetched_at IS NULL
                    OR (revised_at IS NOT NULL
                        AND (body_revised_at IS NULL OR revised_at > body_revised_at)))
             ORDER BY no",
        )
        .map_err(sql)?;
    stmt.query_map([novel_id], |row| row.get(0))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)
}

/// `episode` コマンドの結果を保存し、まだ持っていない挿絵の URL を返す
pub fn save_episode(conn: &mut Connection, records: &[Record]) -> Result<Vec<String>, String> {
    let episode: &EpisodeRecord = records
        .iter()
        .find_map(|r| match r {
            Record::Episode(e) => Some(e),
            _ => None,
        })
        .ok_or("the episode output has no episode record")?;

    let tx = conn.transaction().map_err(sql)?;
    let episode_id: i64 = tx
        .query_row(
            "UPDATE episodes
             SET preface = ?1, body = ?2, afterword = ?3, char_count = ?4,
                 body_fetched_at = ?5, body_revised_at = revised_at, updated_at = ?6
             WHERE url = ?7
             RETURNING id",
            params![
                episode.preface,
                episode.body,
                episode.afterword,
                episode.char_count,
                utc(&episode.fetched_at)?,
                now(),
                episode.url,
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql)?
        .ok_or_else(|| {
            format!(
                "the episode is not in any table of contents: {}",
                episode.url
            )
        })?;

    tx.execute(
        "DELETE FROM episode_images WHERE episode_id = ?1",
        [episode_id],
    )
    .map_err(sql)?;
    let mut missing = Vec::new();
    for (i, image) in episode.images.iter().enumerate() {
        tx.execute(
            "INSERT INTO episode_images (episode_id, position, url) VALUES (?1, ?2, ?3)",
            params![episode_id, i as i64 + 1, image.url],
        )
        .map_err(sql)?;
        let stored: bool = tx
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM images WHERE url = ?1)",
                [&image.url],
                |row| row.get(0),
            )
            .map_err(sql)?;
        if !stored && !missing.contains(&image.url) {
            missing.push(image.url.clone());
        }
    }
    tx.commit().map_err(sql)?;
    Ok(missing)
}

/// `image` コマンドの結果を保存する
pub fn save_image(conn: &Connection, records: &[Record]) -> Result<(), String> {
    let image: &ImageRecord = records
        .iter()
        .find_map(|r| match r {
            Record::Image(i) => Some(i),
            _ => None,
        })
        .ok_or("the image output has no image record")?;
    let data = base64::engine::general_purpose::STANDARD
        .decode(&image.data)
        .map_err(|e| format!("invalid base64 in the image {}: {e}", image.url))?;
    if data.len() as i64 != image.byte_length {
        return Err(format!(
            "the image {} has {} bytes but byteLength is {}",
            image.url,
            data.len(),
            image.byte_length
        ));
    }
    conn.execute(
        "INSERT INTO images (url, content_type, data, byte_length, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (url) DO UPDATE SET
             content_type = excluded.content_type, data = excluded.data,
             byte_length = excluded.byte_length, fetched_at = excluded.fetched_at,
             updated_at = ?6",
        params![
            image.url,
            image.content_type,
            data,
            image.byte_length,
            utc(&image.fetched_at)?,
            now(),
        ],
    )
    .map_err(sql)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::records::{parse, tests as fixtures};
    use crate::subscriptions::Site;

    pub(crate) fn database() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        crate::db::migrate::migrate(&mut conn, crate::db::migrate::MIGRATIONS).unwrap();
        conn
    }

    fn subscription() -> Subscription {
        Subscription {
            site: Site::Narou,
            id: "n0001aa".into(),
        }
    }

    fn toc(entries: &[(i64, Option<&str>)]) -> Vec<Record> {
        let mut lines = vec![fixtures::NOVEL.to_owned()];
        lines.extend(
            entries
                .iter()
                .map(|(no, revised)| fixtures::toc_entry(*no, *revised)),
        );
        parse(&lines.join("\n")).unwrap()
    }

    fn episode_rows(conn: &Connection) -> Vec<(i64, String)> {
        let mut stmt = conn
            .prepare("SELECT no, url FROM episodes ORDER BY no")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    fn url(no: i64) -> String {
        format!("https://ncode.syosetu.com/n0001aa/{no}/")
    }

    #[test]
    fn normalizes_dates_to_utc() {
        assert_eq!(
            utc("2026-01-01T09:00:00+09:00").unwrap(),
            "2026-01-01T00:00:00Z"
        );
        assert!(utc("yesterday").is_err());
    }

    #[test]
    fn saves_toc_and_updates_it_in_place() {
        let mut conn = database();
        let id = save_toc(&mut conn, &subscription(), &toc(&[(1, None), (2, None)])).unwrap();
        assert_eq!(episode_rows(&conn), [(1, url(1)), (2, url(2))]);
        let (fetched_at, author_url): (String, Option<String>) = conn
            .query_row(
                "SELECT metadata_fetched_at, author_url FROM novels WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (fetched_at.as_str(), author_url),
            ("2026-01-01T00:00:00Z", None)
        );

        // 同じ作品を取り直すと、同じ行を書き換える
        let again = save_toc(&mut conn, &subscription(), &toc(&[(1, None), (2, None)])).unwrap();
        assert_eq!(again, id);
        let novels: i64 = conn
            .query_row("SELECT count(*) FROM novels", [], |r| r.get(0))
            .unwrap();
        assert_eq!(novels, 1);
    }

    #[test]
    fn keeps_bodies_when_toc_is_reordered_and_drops_removed_episodes() {
        let mut conn = database();
        save_toc(
            &mut conn,
            &subscription(),
            &toc(&[(1, None), (2, None), (3, None)]),
        )
        .unwrap();
        save_episode(&mut conn, &parse(&fixtures::episode(2, &[])).unwrap()).unwrap();

        // 2 話目の URL が 1 番目に、1 話目の URL が 2 番目に入れ替わり、3 話目が消えた目次
        let mut lines = vec![fixtures::NOVEL.to_owned()];
        for (no, from) in [(1, 2), (2, 1)] {
            let mut entry: serde_json::Value =
                serde_json::from_str(&fixtures::toc_entry(from, None)).unwrap();
            entry["no"] = no.into();
            lines.push(entry.to_string());
        }
        save_toc(
            &mut conn,
            &subscription(),
            &parse(&lines.join("\n")).unwrap(),
        )
        .unwrap();

        assert_eq!(episode_rows(&conn), [(1, url(2)), (2, url(1))]);
        let body: Option<String> = conn
            .query_row("SELECT body FROM episodes WHERE url = ?1", [url(2)], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(body.as_deref(), Some("　本文。"));
    }

    #[test]
    fn rejects_toc_without_single_novel() {
        let mut conn = database();
        let only_entries = parse(&fixtures::toc_entry(1, None)).unwrap();
        assert!(save_toc(&mut conn, &subscription(), &only_entries).is_err());
        let twice = parse(&[fixtures::NOVEL, fixtures::NOVEL].join("\n")).unwrap();
        assert!(save_toc(&mut conn, &subscription(), &twice).is_err());
    }

    #[test]
    fn lists_unfetched_and_revised_episodes() {
        let mut conn = database();
        let id = save_toc(
            &mut conn,
            &subscription(),
            &toc(&[(1, None), (2, Some("2026-01-01T00:00:00Z")), (3, None)]),
        )
        .unwrap();
        assert_eq!(
            episodes_to_fetch(&conn, id).unwrap(),
            [url(1), url(2), url(3)]
        );

        for no in [1, 2] {
            save_episode(&mut conn, &parse(&fixtures::episode(no, &[])).unwrap()).unwrap();
        }
        assert_eq!(episodes_to_fetch(&conn, id).unwrap(), [url(3)]);

        // 2 話目が改稿された目次。日時の書き方が違っても UTC で比べる
        save_toc(
            &mut conn,
            &subscription(),
            &toc(&[(1, None), (2, Some("2026-01-05T09:00:00+09:00")), (3, None)]),
        )
        .unwrap();
        assert_eq!(episodes_to_fetch(&conn, id).unwrap(), [url(2), url(3)]);
    }

    #[test]
    fn saves_episode_and_reports_missing_images() {
        let mut conn = database();
        save_toc(&mut conn, &subscription(), &toc(&[(1, None)])).unwrap();
        save_image(
            &conn,
            &parse(&fixtures::image("https://example.com/a.png", b"a")).unwrap(),
        )
        .unwrap();

        let missing = save_episode(
            &mut conn,
            &parse(&fixtures::episode(
                1,
                &[
                    "https://example.com/a.png",
                    "https://example.com/b.png",
                    "https://example.com/b.png",
                ],
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(missing, ["https://example.com/b.png"]);

        let positions: Vec<(i64, String)> = {
            let mut stmt = conn
                .prepare("SELECT position, url FROM episode_images ORDER BY position")
                .unwrap();
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[1], (2, "https://example.com/b.png".to_owned()));

        // 取り直すと、挿絵の対応を入れ直す
        save_episode(&mut conn, &parse(&fixtures::episode(1, &[])).unwrap()).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM episode_images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn rejects_episode_not_in_toc() {
        let mut conn = database();
        assert!(save_episode(&mut conn, &parse(&fixtures::episode(1, &[])).unwrap()).is_err());
    }

    #[test]
    fn saves_image_bytes() {
        let conn = database();
        save_image(
            &conn,
            &parse(&fixtures::image("https://example.com/a.png", &[0, 1, 2])).unwrap(),
        )
        .unwrap();
        let data: Vec<u8> = conn
            .query_row("SELECT data FROM images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(data, [0, 1, 2]);

        // byteLength が中身と合わなければ保存しない
        let mut wrong: serde_json::Value =
            serde_json::from_str(&fixtures::image("https://example.com/b.png", &[0])).unwrap();
        wrong["byteLength"] = 2.into();
        assert!(save_image(&conn, &parse(&wrong.to_string()).unwrap()).is_err());
    }
}
