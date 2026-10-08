//! 取得のキューと、取得のタスク（ADR 0004、ADR 0015）。

pub mod crawler;
pub mod queue;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crawler::{CrawlError, Crawler, ErrorKind, Request};
use queue::{Queues, Task, task, wait};

/// 取得の後に空ける間隔
pub const INTERVAL: Duration = Duration::from_secs(5);

/// `server_error` と `network` で積み直す回数の上限
pub const MAX_RETRIES: u32 = 3;

/// レート制限で `retryAfter` がないときに待つ時間
pub const DEFAULT_RATE_LIMIT_WAIT: Duration = Duration::from_secs(60);

/// 取得が終わったとき（成功、またはあきらめたとき）に呼ぶ
pub type OnDone = Arc<dyn Fn(Request, Result<String, CrawlError>) + Send + Sync>;

type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

struct Inner {
    queues: Queues,
    crawler: Arc<dyn Crawler>,
    on_done: OnDone,
    now: Clock,
}

/// 取得のタスクを作ってキューに積むもの。再試行するかどうかはタスクが決め、キューは結果を見ない
#[derive(Clone)]
pub struct Fetcher {
    inner: Arc<Inner>,
}

impl Fetcher {
    pub fn new(queues: Queues, crawler: Arc<dyn Crawler>, on_done: OnDone) -> Self {
        Self::with_clock(queues, crawler, on_done, Arc::new(Utc::now))
    }

    fn with_clock(queues: Queues, crawler: Arc<dyn Crawler>, on_done: OnDone, now: Clock) -> Self {
        Self {
            inner: Arc::new(Inner {
                queues,
                crawler,
                on_done,
                now,
            }),
        }
    }

    pub fn queues(&self) -> &Queues {
        &self.inner.queues
    }

    /// 取得と、その後の待ちを 1 組で末尾に積む。URL からドメインを決められなければ積まない
    pub fn enqueue(&self, request: Request) -> Result<(), String> {
        let domain = request
            .domain()
            .ok_or_else(|| format!("cannot determine the domain of {}", request.url))?;
        self.inner
            .queues
            .push_back(&domain, self.fetch_pair(request, 0));
        Ok(())
    }

    /// 全てのキューを破棄してから積み直す。続けて呼んでも同じタスクが重複しない。
    ///
    /// 破棄すると、実行中の取得の後ろにあった待ちも消える。積み直した取得が実行中の取得の直後に
    /// 送られないよう、各ドメインの先頭に間隔の分の待ちを置く（ADR 0016）
    pub fn fetch_all(&self, requests: Vec<Request>) -> Vec<String> {
        self.inner.queues.clear();
        let mut errors = Vec::new();
        let mut started = HashSet::new();
        for request in requests {
            let Some(domain) = request.domain() else {
                errors.push(format!("cannot determine the domain of {}", request.url));
                continue;
            };
            if started.insert(domain.clone()) {
                self.inner.queues.push_back(&domain, vec![wait(INTERVAL)]);
            }
            self.inner
                .queues
                .push_back(&domain, self.fetch_pair(request, 0));
        }
        errors
    }

    fn fetch_pair(&self, request: Request, retries: u32) -> Vec<Task> {
        vec![self.fetch_task(request, retries), wait(INTERVAL)]
    }

    /// 1 回の取得。`retries` はこれまでに積み直した回数（レート制限の分は数えない）
    fn fetch_task(&self, request: Request, retries: u32) -> Task {
        let fetcher = self.clone();
        task(move || async move {
            let result = fetcher.inner.crawler.crawl(&request).await;
            fetcher.handle_result(request, retries, result);
        })
    }

    fn handle_result(&self, request: Request, retries: u32, result: Result<String, CrawlError>) {
        let error = match result {
            Ok(output) => return (self.inner.on_done)(request, Ok(output)),
            Err(error) => error,
        };
        let Some(domain) = request.domain() else {
            return (self.inner.on_done)(request, Err(error));
        };

        match error.kind {
            ErrorKind::RateLimited => {
                let delay = error
                    .retry_after
                    .map(|until| {
                        (until - (self.inner.now)())
                            .to_std()
                            .unwrap_or(Duration::ZERO)
                    })
                    .unwrap_or(DEFAULT_RATE_LIMIT_WAIT);
                let mut tasks = vec![wait(delay)];
                tasks.extend(self.fetch_pair(request, retries));
                self.inner.queues.push_front(&domain, tasks);
            }
            ErrorKind::ServerError | ErrorKind::Network if retries < MAX_RETRIES => {
                self.inner
                    .queues
                    .push_back(&domain, self.fetch_pair(request, retries + 1));
            }
            _ => (self.inner.on_done)(request, Err(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::crawler::{Command, CrawlFuture};
    use crate::fetch::queue::tests::{Log, settle};
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use tokio::runtime::Handle;

    /// 決めた順に結果を返す偽のクローラー。呼ばれた URL を記録する
    struct FakeCrawler {
        results: Mutex<VecDeque<Result<String, CrawlError>>>,
        log: Log,
        /// 1 回の取得にかかる秒数
        secs: u64,
    }

    impl Crawler for FakeCrawler {
        fn crawl(&self, request: &Request) -> CrawlFuture {
            self.log.push(format!("crawl {}", request.url));
            let result = self
                .results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(String::new()));
            let secs = self.secs;
            Box::pin(async move {
                tokio::time::sleep(Duration::from_secs(secs)).await;
                result
            })
        }
    }

    fn error(kind: ErrorKind) -> Result<String, CrawlError> {
        Err(CrawlError {
            kind,
            message: String::new(),
            retry_after: None,
        })
    }

    fn request(url: &str) -> Request {
        Request {
            command: Command::Episode,
            url: url.into(),
        }
    }

    /// 偽のクローラーと、終わった取得の記録を持つ Fetcher
    fn fetcher(results: Vec<Result<String, CrawlError>>, log: &Log, now: DateTime<Utc>) -> Fetcher {
        slow_fetcher(results, log, now, 0)
    }

    fn slow_fetcher(
        results: Vec<Result<String, CrawlError>>,
        log: &Log,
        now: DateTime<Utc>,
        secs: u64,
    ) -> Fetcher {
        let crawler = Arc::new(FakeCrawler {
            results: Mutex::new(results.into()),
            log: log.clone(),
            secs,
        });
        let done_log = log.clone();
        let on_done: OnDone = Arc::new(move |request, result| {
            done_log.push(format!(
                "done {} {}",
                request.url,
                match result {
                    Ok(_) => "ok".to_owned(),
                    Err(e) => format!("{:?}", e.kind),
                }
            ));
        });
        Fetcher::with_clock(
            Queues::new(Handle::current()),
            crawler,
            on_done,
            Arc::new(move || now),
        )
    }

    fn now() -> DateTime<Utc> {
        "2026-01-01T00:00:00Z".parse().unwrap()
    }

    const A1: &str = "https://a.example/1";
    const A2: &str = "https://a.example/2";

    #[tokio::test(start_paused = true)]
    async fn waits_interval_after_each_fetch() {
        let log = Log::new();
        let fetcher = fetcher(vec![], &log, now());
        fetcher.enqueue(request(A1)).unwrap();
        fetcher.enqueue(request(A2)).unwrap();
        settle(30).await;

        assert_eq!(
            log.events(),
            [
                format!("0s crawl {A1}"),
                format!("0s done {A1} ok"),
                format!("5s crawl {A2}"),
                format!("5s done {A2} ok"),
            ]
        );
        assert!(fetcher.queues().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn retries_server_errors_three_times_at_the_back() {
        let log = Log::new();
        let fetcher = fetcher(
            vec![
                error(ErrorKind::ServerError),
                Ok(String::new()),
                error(ErrorKind::Network),
                error(ErrorKind::ServerError),
                error(ErrorKind::Network),
            ],
            &log,
            now(),
        );
        fetcher.enqueue(request(A1)).unwrap();
        fetcher.enqueue(request(A2)).unwrap();
        settle(60).await;

        assert_eq!(
            log.events(),
            [
                format!("0s crawl {A1}"),
                // 失敗した A1 は、並んでいた A2 の後ろに積み直される
                format!("5s crawl {A2}"),
                format!("5s done {A2} ok"),
                format!("10s crawl {A1}"),
                format!("15s crawl {A1}"),
                format!("20s crawl {A1}"),
                // 3 回積み直した後の失敗であきらめる
                format!("20s done {A1} Network"),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limit_waits_until_retry_after_at_the_front() {
        let log = Log::new();
        let fetcher = fetcher(
            vec![
                Err(CrawlError {
                    kind: ErrorKind::RateLimited,
                    message: String::new(),
                    retry_after: Some(now() + chrono::Duration::seconds(30)),
                }),
                Ok(String::new()),
                Ok(String::new()),
            ],
            &log,
            now(),
        );
        fetcher.enqueue(request(A1)).unwrap();
        fetcher.enqueue(request(A2)).unwrap();
        settle(60).await;

        assert_eq!(
            log.events(),
            [
                format!("0s crawl {A1}"),
                // 30 秒待ってから、後ろに並んでいた A2 より先に A1 を取り直す。
                // 積み直した組の待ちと、失敗した取得の後に残っていた待ちが続くため、A2 は 10 秒後になる
                format!("30s crawl {A1}"),
                format!("30s done {A1} ok"),
                format!("40s crawl {A2}"),
                format!("40s done {A2} ok"),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limit_without_retry_after_waits_default() {
        let log = Log::new();
        let fetcher = fetcher(vec![error(ErrorKind::RateLimited)], &log, now());
        fetcher.enqueue(request(A1)).unwrap();
        settle(120).await;

        assert_eq!(
            log.events(),
            [
                format!("0s crawl {A1}"),
                format!("60s crawl {A1}"),
                format!("60s done {A1} ok"),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limits_do_not_count_as_retries() {
        let log = Log::new();
        let mut results = vec![error(ErrorKind::ServerError); 3];
        results.extend(vec![error(ErrorKind::RateLimited); 2]);
        results.push(error(ErrorKind::ServerError));
        let fetcher = fetcher(results, &log, now());
        fetcher.enqueue(request(A1)).unwrap();
        settle(600).await;

        let crawls = log.events().iter().filter(|e| e.contains("crawl")).count();
        assert_eq!(crawls, 6);
        assert!(
            log.events()
                .last()
                .unwrap()
                .ends_with("done https://a.example/1 ServerError")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_immediately_on_other_errors() {
        for kind in [
            ErrorKind::UnsupportedUrl,
            ErrorKind::NotFound,
            ErrorKind::Forbidden,
            ErrorKind::HttpError,
            ErrorKind::ParseError,
            ErrorKind::Crawler,
        ] {
            let log = Log::new();
            let fetcher = fetcher(vec![error(kind)], &log, now());
            fetcher.enqueue(request(A1)).unwrap();
            settle(30).await;
            assert_eq!(
                log.events(),
                [format!("0s crawl {A1}"), format!("0s done {A1} {kind:?}")],
                "{kind:?}"
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_all_replaces_queued_tasks() {
        let log = Log::new();
        let fetcher = fetcher(vec![], &log, now());
        let requests = vec![request(A1), request(A2), request("https://b.example/1")];
        fetcher.fetch_all(requests.clone());
        // 続けて押しても重複しない
        fetcher.fetch_all(requests.clone());
        fetcher.fetch_all(requests);
        settle(60).await;

        let mut crawls: Vec<String> = log
            .events()
            .into_iter()
            .filter(|e| e.contains("crawl"))
            .collect();
        crawls.sort();
        assert_eq!(
            crawls,
            [
                format!("10s crawl {A2}"),
                format!("5s crawl {A1}"),
                "5s crawl https://b.example/1".to_owned(),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_all_keeps_interval_after_running_fetch() {
        let log = Log::new();
        // 1 回の取得に 2 秒かかる。最初の取得は 5 秒から 7 秒まで
        let fetcher = slow_fetcher(vec![], &log, now(), 2);
        fetcher.fetch_all(vec![request(A1)]);
        // 取得の最中に押し直す。破棄で消えた待ちの代わりに、先頭の待ちで間隔が空く
        settle(6).await;
        fetcher.fetch_all(vec![request(A1)]);
        settle(30).await;

        let crawls: Vec<String> = log
            .events()
            .into_iter()
            .filter(|e| e.contains("crawl"))
            .collect();
        assert_eq!(
            crawls,
            [format!("5s crawl {A1}"), format!("12s crawl {A1}")]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_url_without_domain() {
        let log = Log::new();
        let fetcher = fetcher(vec![], &log, now());
        assert!(fetcher.enqueue(request("not a url")).is_err());
        assert_eq!(fetcher.fetch_all(vec![request("not a url")]).len(), 1);
    }
}
