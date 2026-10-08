//! 購読している作品（ADR 0014）。
//!
//! SQLite ではなく `novels.json` に、kindlize と同じ形で持つ。
//!
//! ```json
//! { "subscribe": { "narou": ["n0000aa"], "novel18": [], "hameln": [100000], "kakuyomu": ["1000"] } }
//! ```
//!
//! `subscribe` 以外のキーと、キーの順番はそのまま残す。書き換える前に `novels.json.bak` を作る。

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::environment::Environment;

pub const FILENAME: &str = "novels.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Site {
    Narou,
    Novel18,
    Hameln,
    Kakuyomu,
}

impl Site {
    pub const ALL: [Site; 4] = [Site::Narou, Site::Novel18, Site::Hameln, Site::Kakuyomu];

    /// `novels.json` のキー
    pub fn key(self) -> &'static str {
        match self {
            Site::Narou => "narou",
            Site::Novel18 => "novel18",
            Site::Hameln => "hameln",
            Site::Kakuyomu => "kakuyomu",
        }
    }

    fn from_host(host: &str) -> Option<Self> {
        match host {
            "ncode.syosetu.com" => Some(Site::Narou),
            "novel18.syosetu.com" => Some(Site::Novel18),
            "syosetu.org" => Some(Site::Hameln),
            "kakuyomu.jp" => Some(Site::Kakuyomu),
            _ => None,
        }
    }
}

/// 購読している 1 作品
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Subscription {
    pub site: Site,
    pub id: String,
}

impl Subscription {
    /// 作品のページの URL
    pub fn url(&self) -> String {
        match self.site {
            Site::Narou => format!("https://ncode.syosetu.com/{}/", self.id),
            Site::Novel18 => format!("https://novel18.syosetu.com/{}/", self.id),
            Site::Hameln => format!("https://syosetu.org/novel/{}/", self.id),
            Site::Kakuyomu => format!("https://kakuyomu.jp/works/{}", self.id),
        }
    }

    /// 作品の URL から、サイトと作品 ID を取り出す。作品の中の話の URL でもよい
    pub fn from_url(url: &str) -> Result<Self, Error> {
        let invalid = || Error::InvalidUrl(url.to_owned());
        let rest = url
            .trim()
            .strip_prefix("https://")
            .or_else(|| url.trim().strip_prefix("http://"))
            .ok_or_else(invalid)?;
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        let site = Site::from_host(host).ok_or_else(invalid)?;
        let mut segments = path.split(['/', '?', '#']).filter(|s| !s.is_empty());

        let id = match site {
            Site::Narou | Site::Novel18 => segments
                .next()
                .map(str::to_ascii_lowercase)
                .filter(|id| is_ncode(id)),
            Site::Hameln => (segments.next() == Some("novel"))
                .then(|| segments.next())
                .flatten()
                .filter(|id| is_digits(id))
                .map(str::to_owned),
            Site::Kakuyomu => (segments.next() == Some("works"))
                .then(|| segments.next())
                .flatten()
                .filter(|id| is_digits(id))
                .map(str::to_owned),
        };
        id.map(|id| Subscription { site, id }).ok_or_else(invalid)
    }
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// `n` で始まり、数字と英小文字が続く
fn is_ncode(s: &str) -> bool {
    s.len() > 1
        && s.starts_with('n')
        && s.bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
}

#[derive(Debug)]
pub enum Error {
    InvalidUrl(String),
    Io(io::Error),
    InvalidJson(serde_json::Error),
    /// JSON としては読めるが、形が想定と違う
    InvalidFormat(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidUrl(url) => write!(f, "対応していない URL です: {url}"),
            Error::Io(e) => write!(f, "{e}"),
            Error::InvalidJson(e) => write!(f, "{FILENAME} を JSON として読めません: {e}"),
            Error::InvalidFormat(message) => {
                write!(f, "{FILENAME} の形が正しくありません: {message}")
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

/// `~/.config/epubize/<環境>/novels.json`。`XDG_CONFIG_HOME` があればその下に置く
pub fn default_path(env: Environment) -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))?;
    Some(
        config_home
            .join("epubize")
            .join(env.dir_name())
            .join(FILENAME),
    )
}

fn read(path: &Path) -> Result<Option<Value>, Error> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(Error::InvalidJson),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn subscribe_of(root: &Value) -> Result<Option<&Map<String, Value>>, Error> {
    let root = root
        .as_object()
        .ok_or_else(|| Error::InvalidFormat("最上位がオブジェクトではありません".into()))?;
    match root.get("subscribe") {
        None => Ok(None),
        Some(Value::Object(map)) => Ok(Some(map)),
        Some(_) => Err(Error::InvalidFormat(
            "subscribe がオブジェクトではありません".into(),
        )),
    }
}

/// ID は文字列か数値（hameln）で持つ。比べるときは文字列にする
fn id_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn ids_of(subscribe: &Map<String, Value>, site: Site) -> Result<Vec<String>, Error> {
    match subscribe.get(site.key()) {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                id_string(item).ok_or_else(|| {
                    Error::InvalidFormat(format!(
                        "subscribe.{} に文字列でも数値でもない値があります",
                        site.key()
                    ))
                })
            })
            .collect(),
        Some(_) => Err(Error::InvalidFormat(format!(
            "subscribe.{} が配列ではありません",
            site.key()
        ))),
    }
}

/// 購読している作品の一覧。ファイルがなければ空とする
pub fn list(path: &Path) -> Result<Vec<Subscription>, Error> {
    let Some(root) = read(path)? else {
        return Ok(Vec::new());
    };
    let Some(subscribe) = subscribe_of(&root)? else {
        return Ok(Vec::new());
    };
    let mut items = Vec::new();
    for site in Site::ALL {
        for id in ids_of(subscribe, site)? {
            items.push(Subscription { site, id });
        }
    }
    Ok(items)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddResult {
    pub subscription: Subscription,
    /// 新しく加えたか。すでに購読していれば false で、ファイルは書き換えない
    pub added: bool,
}

/// 作品を購読に加える。
/// 書き換える前に、元のファイルを `novels.json.bak` に写す。書き込みは一時ファイルに書いてから置き換える
pub fn add(path: &Path, subscription: Subscription) -> Result<AddResult, Error> {
    let existing = read(path)?;
    let mut root = existing
        .clone()
        .unwrap_or_else(|| Value::Object(Map::new()));

    if let Some(subscribe) = subscribe_of(&root)?
        && ids_of(subscribe, subscription.site)?.contains(&subscription.id)
    {
        return Ok(AddResult {
            subscription,
            added: false,
        });
    }

    let subscribe = root
        .as_object_mut()
        .expect("checked by subscribe_of")
        .entry("subscribe")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .expect("checked by subscribe_of");
    let ids = subscribe
        .entry(subscription.site.key())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .expect("checked by ids_of");
    // kindlize に合わせ、hameln の ID は数値で持つ
    ids.push(match subscription.site {
        Site::Hameln => subscription
            .id
            .parse::<u64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::from(subscription.id.clone())),
        _ => Value::from(subscription.id.clone()),
    });

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    if existing.is_some() {
        fs::copy(path, backup_path(path))?;
    }
    let tmp = with_suffix(path, ".tmp");
    fs::write(
        &tmp,
        serde_json::to_string_pretty(&root).expect("Value always serializes") + "\n",
    )?;
    fs::rename(&tmp, path)?;

    Ok(AddResult {
        subscription,
        added: true,
    })
}

/// 作品を購読から外す。外したら真、購読していなければ偽を返し、ファイルは書き換えない。
/// 書き換える前に、元のファイルを `novels.json.bak` に写す（ADR 0020）
pub fn remove(path: &Path, subscription: &Subscription) -> Result<bool, Error> {
    let Some(mut root) = read(path)? else {
        return Ok(false);
    };
    let Some(subscribe) = subscribe_of(&root)? else {
        return Ok(false);
    };
    if !ids_of(subscribe, subscription.site)?.contains(&subscription.id) {
        return Ok(false);
    }
    let ids = root["subscribe"][subscription.site.key()]
        .as_array_mut()
        .expect("checked by ids_of");
    ids.retain(|value| id_string(value).as_deref() != Some(subscription.id.as_str()));

    fs::copy(path, backup_path(path))?;
    let tmp = with_suffix(path, ".tmp");
    fs::write(
        &tmp,
        serde_json::to_string_pretty(&root).expect("Value always serializes") + "\n",
    )?;
    fs::rename(&tmp, path)?;
    Ok(true)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

pub fn backup_path(path: &Path) -> PathBuf {
    with_suffix(path, ".bak")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(site: Site, id: &str) -> Subscription {
        Subscription {
            site,
            id: id.into(),
        }
    }

    #[test]
    fn parses_novel_urls() {
        let cases = [
            (
                "https://ncode.syosetu.com/n0001aa/",
                sub(Site::Narou, "n0001aa"),
            ),
            (
                "https://ncode.syosetu.com/N0001AA/12/",
                sub(Site::Narou, "n0001aa"),
            ),
            (
                "https://novel18.syosetu.com/n0002bb",
                sub(Site::Novel18, "n0002bb"),
            ),
            (
                "https://syosetu.org/novel/100001/",
                sub(Site::Hameln, "100001"),
            ),
            (
                "https://syosetu.org/novel/100001/3.html",
                sub(Site::Hameln, "100001"),
            ),
            (
                "https://kakuyomu.jp/works/1000000000000000001",
                sub(Site::Kakuyomu, "1000000000000000001"),
            ),
            (
                "https://kakuyomu.jp/works/1000000000000000001/episodes/2000?x=1",
                sub(Site::Kakuyomu, "1000000000000000001"),
            ),
            (
                "  http://ncode.syosetu.com/n0001aa  ",
                sub(Site::Narou, "n0001aa"),
            ),
        ];
        for (url, expected) in cases {
            assert_eq!(Subscription::from_url(url).unwrap(), expected, "{url}");
        }
    }

    #[test]
    fn rejects_unsupported_urls() {
        for url in [
            "",
            "ncode.syosetu.com/n0001aa/",
            "https://example.com/n0001aa/",
            "https://ncode.syosetu.com/",
            "https://ncode.syosetu.com/rank/",
            "https://syosetu.org/user/1/",
            "https://kakuyomu.jp/users/someone",
            "https://kakuyomu.jp/works/abc",
        ] {
            assert!(Subscription::from_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn builds_url_that_parses_back() {
        for s in [
            sub(Site::Narou, "n0001aa"),
            sub(Site::Novel18, "n0002bb"),
            sub(Site::Hameln, "100001"),
            sub(Site::Kakuyomu, "1000000000000000001"),
        ] {
            assert_eq!(Subscription::from_url(&s.url()).unwrap(), s);
        }
    }

    const SAMPLE: &str = r#"{
  "subscribe": {
    "narou": ["n0001aa"],
    "novel18": [],
    "hameln": [100001],
    "kakuyomu": ["1000000000000000001"]
  },
  "other": {
    "z": 1,
    "a": "keep"
  }
}
"#;

    fn write_sample(dir: &Path) -> PathBuf {
        let path = dir.join(FILENAME);
        fs::write(&path, SAMPLE).unwrap();
        path
    }

    #[test]
    fn lists_subscriptions_in_site_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_sample(dir.path());
        assert_eq!(
            list(&path).unwrap(),
            [
                sub(Site::Narou, "n0001aa"),
                sub(Site::Hameln, "100001"),
                sub(Site::Kakuyomu, "1000000000000000001"),
            ]
        );
    }

    #[test]
    fn lists_nothing_without_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(&dir.path().join(FILENAME)).unwrap().is_empty());
    }

    #[test]
    fn adds_and_backs_up_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_sample(dir.path());

        let result = add(&path, sub(Site::Narou, "n0003cc")).unwrap();
        assert!(result.added);
        assert_eq!(fs::read_to_string(backup_path(&path)).unwrap(), SAMPLE);

        let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            root["subscribe"]["narou"],
            serde_json::json!(["n0001aa", "n0003cc"])
        );
        assert!(!with_suffix(&path, ".tmp").exists());
    }

    #[test]
    fn keeps_other_keys_and_their_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_sample(dir.path());
        add(&path, sub(Site::Kakuyomu, "1000000000000000002")).unwrap();

        let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let keys: Vec<&str> = root
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["subscribe", "other"]);
        let other_keys: Vec<&str> = root["other"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(other_keys, ["z", "a"]);
        assert_eq!(root["other"]["a"], "keep");
    }

    #[test]
    fn stores_hameln_id_as_number() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_sample(dir.path());
        add(&path, sub(Site::Hameln, "100002")).unwrap();

        let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            root["subscribe"]["hameln"],
            serde_json::json!([100001, 100002])
        );
    }

    #[test]
    fn does_not_rewrite_when_already_subscribed() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_sample(dir.path());

        // hameln は数値で持っていても、文字列の ID と同じものとみなす
        let result = add(&path, sub(Site::Hameln, "100001")).unwrap();
        assert!(!result.added);
        assert_eq!(fs::read_to_string(&path).unwrap(), SAMPLE);
        assert!(!backup_path(&path).exists());
    }

    #[test]
    fn creates_file_and_directory_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("development").join(FILENAME);

        add(&path, sub(Site::Novel18, "n0002bb")).unwrap();
        assert!(!backup_path(&path).exists());
        assert_eq!(list(&path).unwrap(), [sub(Site::Novel18, "n0002bb")]);
    }

    #[test]
    fn removes_subscription_and_backs_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_sample(dir.path());

        // hameln は数値で持っていても、文字列の ID で外せる
        assert!(remove(&path, &sub(Site::Hameln, "100001")).unwrap());
        assert_eq!(fs::read_to_string(backup_path(&path)).unwrap(), SAMPLE);
        assert_eq!(
            list(&path).unwrap(),
            [
                sub(Site::Narou, "n0001aa"),
                sub(Site::Kakuyomu, "1000000000000000001")
            ]
        );
        let root: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(root["other"]["a"], "keep");
    }

    #[test]
    fn does_not_rewrite_when_not_subscribed() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_sample(dir.path());
        assert!(!remove(&path, &sub(Site::Narou, "n9999zz")).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), SAMPLE);
        assert!(!backup_path(&path).exists());
        assert!(
            !remove(
                &dir.path().join("missing.json"),
                &sub(Site::Narou, "n0001aa")
            )
            .unwrap()
        );
    }

    #[test]
    fn leaves_broken_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        for broken in [
            "{ not json",
            r#"{"subscribe": []}"#,
            r#"{"subscribe": {"narou": "n0001aa"}}"#,
        ] {
            let path = dir.path().join(FILENAME);
            fs::write(&path, broken).unwrap();
            assert!(add(&path, sub(Site::Narou, "n0003cc")).is_err(), "{broken}");
            assert_eq!(fs::read_to_string(&path).unwrap(), broken);
            assert!(!backup_path(&path).exists());
        }
    }

    #[test]
    fn default_path_separates_environments() {
        let dev = default_path(Environment::Development).unwrap();
        let prod = default_path(Environment::Production).unwrap();
        assert!(dev.ends_with("epubize/development/novels.json"));
        assert!(prod.ends_with("epubize/production/novels.json"));
    }
}
