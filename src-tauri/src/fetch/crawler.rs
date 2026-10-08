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
    /// 取得の後に、続く取得（目次なら話、話なら挿絵）を積むか（ADR 0018）。クローラーには渡さない
    pub follow_up: bool,
}

impl Request {
    pub fn new(command: Command, url: impl Into<String>, follow_up: bool) -> Self {
        Self {
            command,
            url: url.into(),
            follow_up,
        }
    }

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
    pub(crate) fn crawler(message: impl Into<String>) -> Self {
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

/// PATH から探すクローラーの実行ファイルの名前
pub const CRAWLER_NAME: &str = "novel-crawler";

/// Finder から起動したアプリはシェルの PATH を引き継がないため、PATH の後に探す場所
fn fallback_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join("bin"));
        dirs.push(home.join(".local/bin"));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

/// `dirs` の順に、`name` という実行できるファイルを探す
/// `name` という実行ファイルを、PATH から、それでもなければよく使う場所から探す。
/// クローラーと epub-builder で共通に使う（ADR 0018、ADR 0024）
pub fn find_executable(name: &str) -> Option<PathBuf> {
    let path_dirs = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_default();
    find_program(name, path_dirs.into_iter().chain(fallback_dirs()))
}

fn find_program(name: &str, dirs: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    dirs.into_iter()
        .map(|dir| dir.join(name))
        .find(|path| is_executable(path))
}

/// 実行できるファイルか
pub fn is_executable(path: &std::path::Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
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

    /// クローラーの実行ファイルを探す（ADR 0018）。設定画面ができるまでの仮の方法とする。
    /// 環境変数 `EPUBIZE_CRAWLER` があればそれを、なければ PATH から、
    /// それでもなければよく使う場所から `novel-crawler` を探す
    pub fn find() -> Option<Self> {
        if let Some(path) = std::env::var_os("EPUBIZE_CRAWLER").filter(|v| !v.is_empty()) {
            return Some(Self::new(path));
        }
        find_executable(CRAWLER_NAME).map(Self::new)
    }

    pub fn program(&self) -> &std::path::Path {
        &self.program
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
            follow_up: false,
        };
        assert_eq!(request.domain().as_deref(), Some("example.com"));
        let invalid = Request {
            command: Command::Toc,
            url: "not a url".into(),
            follow_up: false,
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
                follow_up: false,
            })
            .await;
        assert_eq!(ok.unwrap(), "ok https://example.com/\n");

        let err = crawler
            .crawl(&Request {
                command: Command::Episode,
                url: "https://example.com/1".into(),
                follow_up: false,
            })
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);
    }

    #[test]
    fn finds_executable_in_order() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let program = second.path().join(CRAWLER_NAME);
        std::fs::write(&program, "#!/bin/sh\n").unwrap();
        // 実行できないファイルは飛ばす
        let not_executable = first.path().join(CRAWLER_NAME);
        std::fs::write(&not_executable, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::set_permissions(&not_executable, std::fs::Permissions::from_mode(0o644))
                .unwrap();
        }
        let dirs = vec![
            PathBuf::from("/nonexistent"),
            first.path().to_owned(),
            second.path().to_owned(),
        ];
        assert_eq!(find_program(CRAWLER_NAME, dirs), Some(program));
        assert_eq!(
            find_program("no-such-program", vec![second.path().to_owned()]),
            None
        );
    }

    #[tokio::test]
    async fn reports_stderr_when_crawler_fails_without_error_record() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("crawler");
        std::fs::write(&script, "#!/bin/sh\necho 'something broke' >&2\nexit 3\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let err = ProcessCrawler::new(&script)
            .crawl(&Request::new(
                Command::Image,
                "https://example.com/a.png",
                false,
            ))
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::Crawler);
        assert!(err.message.contains("something broke"), "{}", err.message);
        assert!(err.to_string().starts_with("Crawler: "), "{err}");
    }

    #[tokio::test]
    async fn reports_missing_program() {
        let crawler = ProcessCrawler::new("/nonexistent/crawler");
        let err = crawler
            .crawl(&Request {
                command: Command::Toc,
                url: "https://example.com/".into(),
                follow_up: false,
            })
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::Crawler);
    }
}
