//! Kindle への送信（ADR 0027）。send-to-kindle を子プロセスとして起動する。
//! 送信を実装するまでは、実行ファイルの場所を決めるところと、.env の検査（ADR 0028）を持つ。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::settings::Settings;

/// PATH などから探す send-to-kindle の実行ファイルの名前
pub const SEND_TO_KINDLE_NAME: &str = "send-to-kindle";

/// 設定で指定した send-to-kindle か、指定がなければ自動で探したもの
pub fn send_to_kindle(settings: &Settings) -> Option<PathBuf> {
    match &settings.send_to_kindle_path {
        Some(path) => Some(PathBuf::from(path)),
        None => find(),
    }
}

/// PATH から、それでもなければよく使う場所から send-to-kindle を探す
pub fn find() -> Option<PathBuf> {
    crate::fetch::crawler::find_executable(SEND_TO_KINDLE_NAME)
}

/// .env.example と比べた .env の状態。値は持たない（ADR 0028）
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EnvCheck {
    /// .env.example のキーがすべて .env にあり、値が空でない
    Complete,
    /// .env にない、または値が空のキー。.env.example に書かれた順
    Missing { keys: Vec<String> },
    /// どちらかのファイルを読めない
    Unreadable { error: String },
}

/// send-to-kindle が読む .env。指定がなければ、環境ごとのアプリ用データ領域（`data_dir`）の `.env`
pub fn env_path(settings: &Settings, data_dir: &Path) -> PathBuf {
    configured_or(
        settings.send_to_kindle_env_path.as_deref(),
        data_dir,
        ".env",
    )
}

/// .env に要るキーを並べた .env.example。指定がなければ、`data_dir` の `.env.example`
pub fn env_example_path(settings: &Settings, data_dir: &Path) -> PathBuf {
    configured_or(
        settings.send_to_kindle_env_example_path.as_deref(),
        data_dir,
        ".env.example",
    )
}

fn configured_or(path: Option<&str>, data_dir: &Path, name: &str) -> PathBuf {
    path.map_or_else(|| data_dir.join(name), PathBuf::from)
}

/// .env.example があれば、.env に足りないキーを調べる。.env.example がなければ調べない
pub fn check_env(settings: &Settings, data_dir: &Path) -> Option<EnvCheck> {
    let env = env_path(settings, data_dir);
    let example = env_example_path(settings, data_dir);
    if !example.is_file() {
        return None;
    }
    if !env.is_file() {
        return Some(EnvCheck::Unreadable {
            error: format!(".env がありません: {}", env.display()),
        });
    }
    let read = |path: &Path| {
        std::fs::read_to_string(path).map_err(|e| format!("{} を読めません: {e}", path.display()))
    };
    Some(match (read(&env), read(&example)) {
        (Ok(env), Ok(example)) => {
            let keys = missing_keys(&env, &example);
            if keys.is_empty() {
                EnvCheck::Complete
            } else {
                EnvCheck::Missing { keys }
            }
        }
        (Err(error), _) | (_, Err(error)) => EnvCheck::Unreadable { error },
    })
}

/// .env.example にあるキーのうち、.env にないか値が空のもの
fn missing_keys(env: &str, example: &str) -> Vec<String> {
    let filled: HashSet<&str> = entries(env)
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, _)| key)
        .collect();
    let mut seen = HashSet::new();
    entries(example)
        .map(|(key, _)| key)
        .filter(|key| !filled.contains(key) && seen.insert(*key))
        .map(String::from)
        .collect()
}

/// dotenv の `KEY=VALUE` の行を読む。空行と `#` の行は飛ばし、`export ` と値を囲む引用符を外す。
/// 複数行にわたる値は扱わない
fn entries(text: &str) -> impl Iterator<Item = (&str, &str)> {
    text.lines().filter_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (key, value) = line.split_once('=')?;
        let value = value.trim();
        let value = ['"', '\'']
            .iter()
            .find_map(|q| value.strip_prefix(*q).and_then(|v| v.strip_suffix(*q)))
            .unwrap_or(value);
        Some((key.trim(), value))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_keys_missing_or_empty_in_env_in_example_order() {
        let example = "# 送信元\nEMAIL=\nSEND_TO_KINDLE_EMAIL=\n\nSMTP_HOST=smtp.example.com\nSMTP_PORT=587\nEMAIL=\n";
        let env = "export EMAIL=a@example.com\nSMTP_HOST=\"\"\nSMTP_PORT = '587'\nUNUSED=1\n";
        assert_eq!(
            missing_keys(env, example),
            ["SEND_TO_KINDLE_EMAIL", "SMTP_HOST"]
        );
        assert!(missing_keys("A=1\nB=x=y\n", "A=\nB=\n").is_empty());
    }

    #[test]
    fn checks_env_in_data_dir_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        assert_eq!(env_path(&settings, dir.path()), dir.path().join(".env"));
        assert_eq!(
            env_example_path(&settings, dir.path()),
            dir.path().join(".env.example")
        );
        // .env.example がなければ調べない
        assert_eq!(check_env(&settings, dir.path()), None);

        std::fs::write(dir.path().join(".env.example"), "EMAIL=\nSMTP_PASSWORD=\n").unwrap();
        assert!(matches!(
            check_env(&settings, dir.path()),
            Some(EnvCheck::Unreadable { error }) if error.starts_with(".env がありません")
        ));

        std::fs::write(dir.path().join(".env"), "EMAIL=a@example.com\n").unwrap();
        assert_eq!(
            check_env(&settings, dir.path()),
            Some(EnvCheck::Missing {
                keys: vec!["SMTP_PASSWORD".into()]
            })
        );

        std::fs::write(
            dir.path().join(".env"),
            "EMAIL=a@example.com\nSMTP_PASSWORD=secret\n",
        )
        .unwrap();
        assert_eq!(check_env(&settings, dir.path()), Some(EnvCheck::Complete));
    }

    #[test]
    fn uses_configured_env_files_over_data_dir() {
        let data = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let env = other.path().join(".env");
        let example = other.path().join("example.env");
        std::fs::write(data.path().join(".env.example"), "EMAIL=\n").unwrap();
        std::fs::write(&env, "SMTP_HOST=smtp.example.com\n").unwrap();
        std::fs::write(&example, "SMTP_HOST=\n").unwrap();
        let path = |p: &Path| Some(p.to_string_lossy().into_owned());
        let settings = Settings {
            send_to_kindle_env_path: path(&env),
            send_to_kindle_env_example_path: path(&example),
            ..Settings::default()
        };
        assert_eq!(env_path(&settings, data.path()), env);
        assert_eq!(env_example_path(&settings, data.path()), example);
        assert_eq!(check_env(&settings, data.path()), Some(EnvCheck::Complete));
    }

    #[test]
    fn uses_configured_path_over_auto_detection() {
        let settings = Settings {
            send_to_kindle_path: Some("/opt/example/send-to-kindle".into()),
            ..Settings::default()
        };
        assert_eq!(
            send_to_kindle(&settings),
            Some(PathBuf::from("/opt/example/send-to-kindle"))
        );
        assert_eq!(send_to_kindle(&Settings::default()), find());
    }
}
