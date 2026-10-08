//! クローラーの呼び出し（ADR 0004、ADR 0005、ADR 0015）。
//!
//! クローラーを子プロセスとして起動し、成功なら標準出力の JSON Lines を、失敗なら `error` レコードを返す。

use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// クローラーのコマンド（ADR 0005）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Toc,
    Episode,
    Image,
}

impl Command {
    pub fn as_str(self) -> &'static str {
        match self {
            Command::Toc => "toc",
            Command::Episode => "episode",
            Command::Image => "image",
        }
    }
}

/// 1 回の取得
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub command: Command,
    pub url: String,
}

impl Request {
    /// キューを分ける単位。いまは URL のホスト名とする（ADR 0015）
    pub fn domain(&self) -> Option<String> {
        url::Url::parse(&self.url)
            .ok()?
            .host_str()
            .map(str::to_owned)
    }
}

/// `error` レコードの `kind`（ADR 0005）と、クローラーを動かせなかった場合
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    UnsupportedUrl,
    NotFound,
    Forbidden,
    RateLimited,
    ServerError,
    HttpError,
    Network,
    ParseError,
    /// クローラーを起動できなかった、または出力を読めなかった
    #[serde(skip)]
    Crawler,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawlError {
    pub kind: ErrorKind,
    pub message: String,
    /// レート制限のとき、次に取得してよい日時
    pub retry_after: Option<DateTime<Utc>>,
}

impl CrawlError {
    fn crawler(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Crawler,
            message: message.into(),
            retry_after: None,
        }
    }
}

impl fmt::Display for CrawlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for CrawlError {}

pub type CrawlFuture = Pin<Box<dyn Future<Output = Result<String, CrawlError>> + Send>>;

/// 取得を行うもの。テストでは偽物に差し替える
pub trait Crawler: Send + Sync + 'static {
    /// 成功なら標準出力（JSON Lines）を返す
    fn crawl(&self, request: &Request) -> CrawlFuture;
}

/// 子プロセスとして起動するクローラー
pub struct ProcessCrawler {
    program: PathBuf,
}

impl ProcessCrawler {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    /// 環境変数 `EPUBIZE_CRAWLER` で指定した実行ファイル。設定画面ができるまでの仮の指定方法
    pub fn from_env() -> Option<Self> {
        std::env::var_os("EPUBIZE_CRAWLER")
            .filter(|v| !v.is_empty())
            .map(Self::new)
    }
}

impl Crawler for ProcessCrawler {
    fn crawl(&self, request: &Request) -> CrawlFuture {
        let mut command = tokio::process::Command::new(&self.program);
        command
            .arg(request.command.as_str())
            .arg(&request.url)
            .kill_on_drop(true);
        Box::pin(async move {
            let output = command
                .output()
                .await
                .map_err(|e| CrawlError::crawler(format!("failed to start the crawler: {e}")))?;
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            if output.status.success() {
                return Ok(stdout);
            }
            Err(parse_error(&stdout).unwrap_or_else(|| {
                CrawlError::crawler(format!(
                    "the crawler exited with {} without an error record: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ))
            }))
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorRecord {
    #[serde(rename = "type")]
    record_type: String,
    kind: ErrorKind,
    message: String,
    #[serde(default)]
    retry_after: Option<String>,
}

/// 標準出力の最後の `error` レコードを読む
fn parse_error(stdout: &str) -> Option<CrawlError> {
    let record = stdout
        .lines()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .find_map(|line| serde_json::from_str::<ErrorRecord>(line).ok())
        .filter(|record| record.record_type == "error")?;
    Some(CrawlError {
        kind: record.kind,
        message: record.message,
        retry_after: record
            .retry_after
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|t| t.with_timezone(&Utc)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_is_host_name() {
        let request = Request {
            command: Command::Toc,
            url: "https://example.com/novels/1/".into(),
        };
        assert_eq!(request.domain().as_deref(), Some("example.com"));
        let invalid = Request {
            command: Command::Toc,
            url: "not a url".into(),
        };
        assert_eq!(invalid.domain(), None);
    }

    #[test]
    fn parses_error_record() {
        let stdout = r#"{"v":1,"type":"error","url":"https://example.com/","kind":"server_error","message":"503","status":503}"#;
        assert_eq!(
            parse_error(stdout),
            Some(CrawlError {
                kind: ErrorKind::ServerError,
                message: "503".into(),
                retry_after: None,
            })
        );
    }

    #[test]
    fn parses_retry_after() {
        let stdout = r#"{"v":1,"type":"error","url":"u","kind":"rate_limited","message":"429","status":429,"retryAfter":"2026-10-08T12:00:30+09:00"}"#;
        let error = parse_error(stdout).unwrap();
        assert_eq!(error.kind, ErrorKind::RateLimited);
        assert_eq!(
            error.retry_after,
            Some("2026-10-08T03:00:30Z".parse::<DateTime<Utc>>().unwrap())
        );
    }

    #[test]
    fn ignores_non_error_output() {
        assert_eq!(parse_error(""), None);
        assert_eq!(parse_error("not json\n"), None);
        assert_eq!(
            parse_error(r#"{"v":1,"type":"novel","kind":"x","message":"m"}"#),
            None
        );
    }

    #[tokio::test]
    async fn runs_process_and_reads_output() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("crawler");
        std::fs::write(
            &script,
            "#!/bin/sh\nif [ \"$1\" = toc ]; then echo \"ok $2\"; exit 0; fi\n\
             echo '{\"v\":1,\"type\":\"error\",\"url\":\"u\",\"kind\":\"not_found\",\"message\":\"gone\",\"status\":404}'\nexit 1\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let crawler = ProcessCrawler::new(&script);

        let ok = crawler
            .crawl(&Request {
                command: Command::Toc,
                url: "https://example.com/".into(),
            })
            .await;
        assert_eq!(ok.unwrap(), "ok https://example.com/\n");

        let err = crawler
            .crawl(&Request {
                command: Command::Episode,
                url: "https://example.com/1".into(),
            })
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn reports_missing_program() {
        let crawler = ProcessCrawler::new("/nonexistent/crawler");
        let err = crawler
            .crawl(&Request {
                command: Command::Toc,
                url: "https://example.com/".into(),
            })
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::Crawler);
    }
}
