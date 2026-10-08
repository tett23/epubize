//! 取得のキュー（ADR 0015）と取り込み（ADR 0018）を通した結合テスト。
//! 偽のクローラーが合成データの JSON Lines を返し、キューが取得を積み、結果をデータベースに保存する。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use epubize_lib::db::migrate;
use epubize_lib::fetch::crawler::{Command, CrawlError, CrawlFuture, Crawler, ErrorKind, Request};
use epubize_lib::fetch::queue::Queues;
use epubize_lib::fetch::{Fetcher, OnDone};
use epubize_lib::pipeline;
use rusqlite::Connection;
use tokio::runtime::Handle;
use tokio::time::Instant;

const NOVEL_URL: &str = "https://ncode.syosetu.com/n0001aa/";
const IMAGE_URL: &str = "https://img.example.com/map.png";

fn line(value: serde_json::Value) -> String {
    value.to_string()
}

/// 合成データを返す偽のクローラー。呼ばれた URL を、時刻とともに記録する
struct FakeCrawler {
    start: Instant,
    calls: Arc<Mutex<Vec<(u64, String)>>>,
    errors: HashMap<String, ErrorKind>,
}

impl FakeCrawler {
    fn output(request: &Request) -> String {
        match request.command {
            Command::Toc => {
                let mut lines = vec![line(serde_json::json!({
                    "v": 1, "type": "novel", "site": "narou", "novelId": "n0001aa", "url": NOVEL_URL,
                    "title": "合成データの作品", "author": {"name": "見本 太郎", "url": null},
                    "description": "あらすじ", "episodeCount": 2, "isConcluded": false,
                    "fetchedAt": "2026-01-01T00:00:00Z",
                }))];
                for no in 1..=2 {
                    lines.push(line(serde_json::json!({
                        "v": 1, "type": "toc_entry", "novelUrl": NOVEL_URL, "no": no,
                        "url": format!("{NOVEL_URL}{no}/"), "title": format!("第{no}話"), "chapter": null,
                        "publishedAt": "2026-01-01T00:00:00Z", "revisedAt": null,
                    })));
                }
                lines.join("\n")
            }
            Command::Episode => {
                let images: Vec<serde_json::Value> = if request.url.ends_with("/1/") {
                    vec![serde_json::json!({"url": IMAGE_URL, "alt": "地図"})]
                } else {
                    vec![]
                };
                line(serde_json::json!({
                    "v": 1, "type": "episode", "url": request.url, "title": "話", "body": "　本文。",
                    "preface": null, "afterword": null, "images": images, "charCount": 3,
                    "fetchedAt": "2026-01-02T00:00:00Z",
                }))
            }
            Command::Image => line(serde_json::json!({
                "v": 1, "type": "image", "url": request.url, "contentType": "image/png",
                "data": "AAEC", "byteLength": 3, "fetchedAt": "2026-01-02T00:00:00Z",
            })),
        }
    }
}

impl Crawler for FakeCrawler {
    fn crawl(&self, request: &Request) -> CrawlFuture {
        let secs = self.start.elapsed().as_secs();
        self.calls.lock().unwrap().push((secs, request.url.clone()));
        let result = match self.errors.get(&request.url) {
            Some(kind) => Err(CrawlError {
                kind: *kind,
                message: "fake".into(),
                retry_after: None,
            }),
            None => Ok(Self::output(request)),
        };
        Box::pin(async move { result })
    }
}

struct Harness {
    fetcher: Arc<OnceLock<Fetcher>>,
    db: Arc<Mutex<Connection>>,
    calls: Arc<Mutex<Vec<(u64, String)>>>,
    failures: Arc<Mutex<Vec<String>>>,
}

impl Harness {
    fn new(errors: HashMap<String, ErrorKind>) -> Self {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        migrate::migrate(&mut conn, migrate::MIGRATIONS).unwrap();
        let db = Arc::new(Mutex::new(conn));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let failures = Arc::new(Mutex::new(Vec::new()));
        let fetcher: Arc<OnceLock<Fetcher>> = Arc::new(OnceLock::new());

        // アプリの on_done（commands.rs）と同じく、保存してから続く取得を積む
        let (db2, failures2, fetcher2) = (db.clone(), failures.clone(), fetcher.clone());
        let on_done: OnDone = Arc::new(
            move |request: Request, result: Result<String, CrawlError>| {
                let outcome = result.map_err(|e| e.to_string()).and_then(|stdout| {
                    let mut conn = db2.lock().unwrap();
                    pipeline::import(&mut conn, &request, &stdout)
                });
                match outcome {
                    Ok(next) => {
                        for request in next {
                            fetcher2.get().unwrap().enqueue(request).unwrap();
                        }
                    }
                    Err(e) => failures2
                        .lock()
                        .unwrap()
                        .push(format!("{} {e}", request.url)),
                }
            },
        );
        let crawler = Arc::new(FakeCrawler {
            start: Instant::now(),
            calls: calls.clone(),
            errors,
        });
        let _ = fetcher.set(Fetcher::new(
            Queues::new(Handle::current()),
            Some(crawler),
            on_done,
        ));
        Self {
            fetcher,
            db,
            calls,
            failures,
        }
    }

    fn fetcher(&self) -> &Fetcher {
        self.fetcher.get().unwrap()
    }

    fn count(&self, sql: &str) -> i64 {
        self.db
            .lock()
            .unwrap()
            .query_row(sql, [], |r| r.get(0))
            .unwrap()
    }
}

async fn settle(secs: u64) {
    tokio::time::sleep(Duration::from_secs(secs)).await;
}

#[tokio::test(start_paused = true)]
async fn fetch_all_saves_toc_bodies_and_images_in_order() {
    let h = Harness::new(HashMap::new());
    h.fetcher()
        .fetch_all(vec![Request::new(Command::Toc, NOVEL_URL, true)]);
    settle(120).await;

    assert!(
        h.failures.lock().unwrap().is_empty(),
        "{:?}",
        h.failures.lock().unwrap()
    );
    assert_eq!(h.count("SELECT count(*) FROM novels"), 1);
    assert_eq!(
        h.count("SELECT count(*) FROM episodes WHERE body_fetched_at IS NOT NULL"),
        2
    );
    assert_eq!(h.count("SELECT count(*) FROM episode_images"), 1);
    assert_eq!(h.count("SELECT count(*) FROM images"), 1);

    // 同じドメインは前の取得から 5 秒空き、挿絵は別のドメインなので並行して取る。
    // fetch all は先頭に 5 秒の待ちを置く（ADR 0016）
    assert_eq!(
        *h.calls.lock().unwrap(),
        [
            (5, NOVEL_URL.to_owned()),
            (10, format!("{NOVEL_URL}1/")),
            (10, IMAGE_URL.to_owned()),
            (15, format!("{NOVEL_URL}2/")),
        ]
    );
    assert!(h.fetcher().queues().is_empty());
}

#[tokio::test(start_paused = true)]
async fn metadata_only_saves_toc_without_follow_ups() {
    let h = Harness::new(HashMap::new());
    h.fetcher()
        .fetch_all(vec![Request::new(Command::Toc, NOVEL_URL, false)]);
    settle(60).await;

    assert_eq!(h.count("SELECT count(*) FROM episodes"), 2);
    assert_eq!(
        h.count("SELECT count(*) FROM episodes WHERE body_fetched_at IS NOT NULL"),
        0
    );
    assert_eq!(h.calls.lock().unwrap().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn failed_episode_is_reported_and_others_continue() {
    let errors = HashMap::from([(format!("{NOVEL_URL}1/"), ErrorKind::NotFound)]);
    let h = Harness::new(errors);
    h.fetcher()
        .fetch_all(vec![Request::new(Command::Toc, NOVEL_URL, true)]);
    settle(120).await;

    let failures = h.failures.lock().unwrap().clone();
    assert_eq!(failures.len(), 1);
    assert!(
        failures[0].starts_with(&format!("{NOVEL_URL}1/")),
        "{failures:?}"
    );
    // 1 話目は取れず、2 話目は取れる。1 話目の挿絵は積まれない
    assert_eq!(
        h.count("SELECT count(*) FROM episodes WHERE body_fetched_at IS NOT NULL"),
        1
    );
    assert_eq!(h.count("SELECT count(*) FROM images"), 0);
}

#[tokio::test(start_paused = true)]
async fn refetching_fetched_novel_queues_nothing_new() {
    let h = Harness::new(HashMap::new());
    h.fetcher()
        .fetch_all(vec![Request::new(Command::Toc, NOVEL_URL, true)]);
    settle(120).await;
    let first = h.calls.lock().unwrap().len();

    // 全て取得済みなら、目次を取り直しても本文と挿絵は積まない
    h.fetcher()
        .fetch_all(vec![Request::new(Command::Toc, NOVEL_URL, true)]);
    settle(60).await;
    assert_eq!(h.calls.lock().unwrap().len(), first + 1);
}
