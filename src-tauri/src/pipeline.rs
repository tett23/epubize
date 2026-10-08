//! 取得した結果を保存し、続く取得を決める（ADR 0018）。

use rusqlite::Connection;

use crate::fetch::crawler::{Command, Request};
use crate::library::{records, store};
use crate::subscriptions::Subscription;

/// クローラーの標準出力を保存し、続けて積む取得を返す。
/// 目次の後には本文を取得すべき話を、話の後にはまだ持っていない挿絵を積む。`follow_up` が偽なら積まない
pub fn import(
    conn: &mut Connection,
    request: &Request,
    stdout: &str,
) -> Result<Vec<Request>, String> {
    let records = records::parse(stdout)?;
    let next = match request.command {
        Command::Toc => {
            let subscription = Subscription::from_url(&request.url).map_err(|e| e.to_string())?;
            let novel_id = store::save_toc(conn, &subscription, &records)?;
            if !request.follow_up {
                return Ok(Vec::new());
            }
            store::episodes_to_fetch(conn, novel_id)?
                .into_iter()
                .map(|url| Request::new(Command::Episode, url, true))
                .collect()
        }
        Command::Episode => {
            let missing = store::save_episode(conn, &records)?;
            if !request.follow_up {
                return Ok(Vec::new());
            }
            missing
                .into_iter()
                .map(|url| Request::new(Command::Image, url, false))
                .collect()
        }
        Command::Image => {
            store::save_image(conn, &records)?;
            Vec::new()
        }
    };
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::records::tests as fixtures;

    fn database() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        crate::db::migrate::migrate(&mut conn, crate::db::migrate::MIGRATIONS).unwrap();
        conn
    }

    const NOVEL_URL: &str = "https://ncode.syosetu.com/n0001aa/";

    fn toc_output() -> String {
        [
            fixtures::NOVEL.to_owned(),
            fixtures::toc_entry(1, None),
            fixtures::toc_entry(2, None),
        ]
        .join("\n")
    }

    fn episode_url(no: i64) -> String {
        format!("{NOVEL_URL}{no}/")
    }

    #[test]
    fn toc_follows_up_with_unfetched_episodes() {
        let mut conn = database();
        let next = import(
            &mut conn,
            &Request::new(Command::Toc, NOVEL_URL, true),
            &toc_output(),
        )
        .unwrap();
        assert_eq!(
            next,
            [
                Request::new(Command::Episode, episode_url(1), true),
                Request::new(Command::Episode, episode_url(2), true),
            ]
        );
    }

    #[test]
    fn metadata_only_toc_does_not_follow_up() {
        let mut conn = database();
        let next = import(
            &mut conn,
            &Request::new(Command::Toc, NOVEL_URL, false),
            &toc_output(),
        )
        .unwrap();
        assert!(next.is_empty());
        let novels: i64 = conn
            .query_row("SELECT count(*) FROM novels", [], |r| r.get(0))
            .unwrap();
        assert_eq!(novels, 1);
    }

    #[test]
    fn episode_follows_up_with_missing_images() {
        let mut conn = database();
        import(
            &mut conn,
            &Request::new(Command::Toc, NOVEL_URL, false),
            &toc_output(),
        )
        .unwrap();
        let output = fixtures::episode(1, &["https://example.com/a.png"]);

        let next = import(
            &mut conn,
            &Request::new(Command::Episode, episode_url(1), true),
            &output,
        )
        .unwrap();
        assert_eq!(
            next,
            [Request::new(
                Command::Image,
                "https://example.com/a.png",
                false
            )]
        );

        let image = fixtures::image("https://example.com/a.png", b"a");
        let next = import(
            &mut conn,
            &Request::new(Command::Image, "https://example.com/a.png", false),
            &image,
        )
        .unwrap();
        assert!(next.is_empty());

        // 挿絵を持った後に取り直しても、挿絵は積まない
        let next = import(
            &mut conn,
            &Request::new(Command::Episode, episode_url(1), true),
            &output,
        )
        .unwrap();
        assert!(next.is_empty());
    }

    #[test]
    fn rejects_output_that_does_not_match_the_schema() {
        let mut conn = database();
        let error = import(
            &mut conn,
            &Request::new(Command::Toc, NOVEL_URL, true),
            r#"{"v":1,"type":"novel"}"#,
        )
        .unwrap_err();
        assert!(error.contains("schema"), "{error}");
    }
}
