//! 管理画面で変える設定（ADR 0020）。
//!
//! `~/.config/epubize/<環境>/settings.json` に持つ。ファイルがなければ既定値を使う。
//! 書き換える前に `settings.json.bak` を作り、一時ファイルに書いてから置き換える。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::NaiveTime;
use serde::{Deserialize, Serialize};

use crate::environment::Environment;
use crate::schedule::Schedule;

pub const FILENAME: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScheduleSettings {
    pub enabled: bool,
    /// 毎日の時刻（ローカル時刻、`HH:MM`）
    pub at: String,
}

impl Default for ScheduleSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            at: "03:00".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// クローラーの実行ファイル。null なら自動で探す（ADR 0018）
    pub crawler_path: Option<String>,
    /// 定期取得（ADR 0019）
    pub schedule: ScheduleSettings,
}

impl Settings {
    /// 値を確かめ、定期取得の設定を返す
    pub fn validate(&self) -> Result<Schedule, String> {
        let at = NaiveTime::parse_from_str(&self.schedule.at, "%H:%M")
            .map_err(|_| format!("時刻は HH:MM で指定してください: {:?}", self.schedule.at))?;
        if let Some(path) = &self.crawler_path {
            if path.trim().is_empty() {
                return Err("クローラーの実行ファイルが空です。自動で探すときは空欄ではなく未指定にしてください".into());
            }
            if !crate::fetch::crawler::is_executable(Path::new(path)) {
                return Err(format!(
                    "クローラーの実行ファイルが見つからないか、実行できません: {path}"
                ));
            }
        }
        Ok(Schedule {
            enabled: self.schedule.enabled,
            at,
        })
    }
}

/// `~/.config/epubize/<環境>/settings.json`。購読の一覧（ADR 0014）と同じディレクトリに置く
pub fn default_path(env: Environment) -> Option<PathBuf> {
    crate::subscriptions::default_path(env).map(|p| p.with_file_name(FILENAME))
}

/// 設定を読む。ファイルがなければ既定値とする。知らない項目は無視する
pub fn load(path: &Path) -> Result<Settings, String> {
    match fs::read_to_string(path) {
        Ok(text) => {
            serde_json::from_str(&text).map_err(|e| format!("{FILENAME} を読めません: {e}"))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e.to_string()),
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// 設定を確かめてから書く。前の内容は `.bak` に残す
pub fn save(path: &Path, settings: &Settings) -> Result<Schedule, String> {
    let schedule = settings.validate()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    if path.exists() {
        fs::copy(path, with_suffix(path, ".bak")).map_err(|e| e.to_string())?;
    }
    let tmp = with_suffix(path, ".tmp");
    let text = serde_json::to_string_pretty(settings).expect("settings always serialize") + "\n";
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(schedule)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn executable(dir: &Path) -> String {
        let path = dir.join("crawler");
        fs::write(&path, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn defaults_when_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let settings = load(&dir.path().join(FILENAME)).unwrap();
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.schedule.at, "03:00");
        assert!(settings.schedule.enabled);
        assert_eq!(settings.crawler_path, None);
    }

    #[test]
    fn fills_missing_fields_with_defaults_and_ignores_unknown_ones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILENAME);
        fs::write(&path, r#"{"schedule":{"enabled":false},"unknown":1}"#).unwrap();
        let settings = load(&path).unwrap();
        assert!(!settings.schedule.enabled);
        assert_eq!(settings.schedule.at, "03:00");
    }

    #[test]
    fn saves_and_backs_up_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("development").join(FILENAME);
        let crawler = executable(dir.path());

        let first = Settings::default();
        save(&path, &first).unwrap();
        assert!(!with_suffix(&path, ".bak").exists());

        let second = Settings {
            crawler_path: Some(crawler),
            schedule: ScheduleSettings {
                enabled: false,
                at: "04:30".into(),
            },
        };
        let schedule = save(&path, &second).unwrap();
        assert_eq!(load(&path).unwrap(), second);
        assert_eq!(load(&with_suffix(&path, ".bak")).unwrap(), first);
        assert!(!with_suffix(&path, ".tmp").exists());
        assert_eq!(schedule.at, NaiveTime::from_hms_opt(4, 30, 0).unwrap());
        assert!(!schedule.enabled);
    }

    #[test]
    fn rejects_invalid_values_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILENAME);
        save(&path, &Settings::default()).unwrap();
        let before = fs::read_to_string(&path).unwrap();

        for invalid in [
            Settings {
                schedule: ScheduleSettings {
                    enabled: true,
                    at: "25:00".into(),
                },
                ..Settings::default()
            },
            Settings {
                schedule: ScheduleSettings {
                    enabled: true,
                    at: "3時".into(),
                },
                ..Settings::default()
            },
            Settings {
                crawler_path: Some("/nonexistent/crawler".into()),
                ..Settings::default()
            },
            Settings {
                crawler_path: Some("  ".into()),
                ..Settings::default()
            },
        ] {
            assert!(save(&path, &invalid).is_err(), "{invalid:?}");
            assert_eq!(fs::read_to_string(&path).unwrap(), before);
        }
    }

    #[test]
    fn sits_next_to_subscriptions() {
        let path = default_path(Environment::Development).unwrap();
        assert!(path.ends_with("epubize/development/settings.json"));
        assert_eq!(
            path.parent(),
            crate::subscriptions::default_path(Environment::Development)
                .unwrap()
                .parent()
        );
    }

    #[test]
    fn reports_unreadable_path() {
        let dir = tempfile::tempdir().unwrap();
        // ディレクトリはファイルとして読めない
        assert!(load(dir.path()).is_err());
    }

    #[test]
    fn rejects_broken_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILENAME);
        fs::write(&path, "{ not json").unwrap();
        assert!(load(&path).is_err());
    }
}
