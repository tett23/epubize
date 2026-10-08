//! 画面から呼ぶ Tauri のコマンド。

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::db::Database;
use crate::environment::Environment;
use crate::fetch::crawler::{Command, Crawler, ProcessCrawler, Request};
use crate::fetch::queue::Queues;
use crate::fetch::{self, Fetcher};
use crate::library::query;
use crate::pipeline;
use crate::schedule::Schedule;
use crate::settings::{self, Settings};
use crate::subscriptions::{self, AddResult, Site, Subscription};

/// 環境ごとに分かれたファイルの置き場所（ADR 0014、ADR 0020）
pub struct Paths {
    pub env: Environment,
    pub subscriptions: PathBuf,
    pub settings: PathBuf,
    pub database: PathBuf,
}

/// データの置き場所を決めている環境。画面の見出しに出す
#[tauri::command]
pub fn environment(paths: State<'_, Paths>) -> String {
    paths.env.to_string()
}

#[derive(serde::Serialize)]
pub struct SubscriptionItem {
    #[serde(flatten)]
    subscription: Subscription,
    url: String,
}

#[tauri::command]
pub fn list_subscriptions(paths: State<'_, Paths>) -> Result<Vec<SubscriptionItem>, String> {
    let items = subscriptions::list(&paths.subscriptions).map_err(|e| e.to_string())?;
    Ok(items
        .into_iter()
        .map(|subscription| SubscriptionItem {
            url: subscription.url(),
            subscription,
        })
        .collect())
}

#[tauri::command]
pub fn add_subscription(url: &str, paths: State<'_, Paths>) -> Result<AddResult, String> {
    let subscription = Subscription::from_url(url).map_err(|e| e.to_string())?;
    subscriptions::add(&paths.subscriptions, subscription).map_err(|e| e.to_string())
}

/// 作品を購読から外す（ADR 0020）。取得済みの作品のデータは消さない。外したら真を返す
#[tauri::command]
pub fn remove_subscription(
    site_key: &str,
    site_id: &str,
    paths: State<'_, Paths>,
) -> Result<bool, String> {
    let subscription = Subscription {
        site: site(site_key)?,
        id: site_id.to_owned(),
    };
    subscriptions::remove(&paths.subscriptions, &subscription).map_err(|e| e.to_string())
}

/// 取得のキュー（ADR 0015）。クローラーは管理画面で差し替える（ADR 0020）
pub struct FetchState(Fetcher);

/// 取得が終わったことを画面に知らせるイベント。画面はこれを受けて表示を読み直す
#[derive(Clone, serde::Serialize)]
struct FetchDone {
    command: &'static str,
    url: String,
    /// 取得または保存に失敗したときの理由。成功なら null
    error: Option<String>,
}

const FETCH_DONE: &str = "fetch-done";

/// 取得の結果を保存し、続く取得を積んで、画面に知らせる（ADR 0018）
fn on_done<R: Runtime>(
    app: &AppHandle<R>,
    request: Request,
    result: Result<String, fetch::crawler::CrawlError>,
) {
    let outcome = result.map_err(|e| e.to_string()).and_then(|stdout| {
        let database = app.state::<Database>();
        let mut conn = database.0.lock().map_err(|e| e.to_string())?;
        pipeline::import(&mut conn, &request, &stdout)
    });
    let error = match outcome {
        Ok(next) => {
            let fetcher = &app.state::<FetchState>().0;
            for request in next {
                let _ = fetcher.enqueue(request);
            }
            None
        }
        Err(e) => Some(e),
    };
    let _ = app.emit(
        FETCH_DONE,
        FetchDone {
            command: request.command.as_str(),
            url: request.url,
            error,
        },
    );
}

/// 設定で指定したクローラーか、指定がなければ自動で探したもの（ADR 0018、ADR 0020）
fn crawler(settings: &Settings) -> Option<ProcessCrawler> {
    match &settings.crawler_path {
        Some(path) => Some(ProcessCrawler::new(path)),
        None => ProcessCrawler::find(),
    }
}

pub fn fetch_state<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> FetchState {
    let queues = Queues::new(tauri::async_runtime::handle().inner().clone());
    let handle = app.clone();
    let done: fetch::OnDone = Arc::new(move |request, result| on_done(&handle, request, result));
    let crawler = crawler(settings).map(|c| Arc::new(c) as Arc<dyn Crawler>);
    FetchState(Fetcher::new(queues, crawler, done))
}

fn fetcher<'a>(fetch: &'a State<'_, FetchState>) -> Result<&'a Fetcher, String> {
    if fetch.0.has_crawler() {
        return Ok(&fetch.0);
    }
    Err(format!(
        "クローラーが見つかりません。管理画面で実行ファイルを指定するか、PATH に {} を置いてください",
        fetch::crawler::CRAWLER_NAME
    ))
}

#[derive(serde::Serialize)]
pub struct FetchStatus {
    /// クローラーがあるか
    configured: bool,
    /// キューに残っているタスクの数（取得と待ちの両方を数える）
    queued: usize,
}

#[tauri::command]
pub fn fetch_status(fetch: State<'_, FetchState>) -> FetchStatus {
    FetchStatus {
        configured: fetch.0.has_crawler(),
        queued: fetch.0.queues().len(),
    }
}

/// 全てのキューを破棄し、購読している全作品の目次の取得を積み直す。積んだ作品の数を返す。
/// `follow_up` が真なら、目次の後に本文と挿絵まで取得する（fetch all）。偽なら目次だけ（fetch all metadata）
fn requeue_all(paths: &Paths, fetcher: &Fetcher, follow_up: bool) -> Result<usize, String> {
    let requests: Vec<Request> = subscriptions::list(&paths.subscriptions)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|subscription| Request::new(Command::Toc, subscription.url(), follow_up))
        .collect();
    let total = requests.len();
    if let Some(first) = fetcher.fetch_all(requests).first() {
        return Err(first.clone());
    }
    Ok(total)
}

/// いまの定期取得の設定。管理画面で保存すると書き換わる（ADR 0020）
pub struct ScheduleState(pub RwLock<Schedule>);

/// 毎日決めた時刻に fetch all を行う（ADR 0019）。設定は見るたびに読み直す。クローラーがなければ何もしない
pub fn start_scheduled_fetch<R: Runtime>(app: AppHandle<R>) {
    let reader = app.clone();
    tauri::async_runtime::spawn(crate::schedule::run_daily(
        move || *reader.state::<ScheduleState>().0.read().unwrap(),
        crate::schedule::local_now,
        move || {
            let app = app.clone();
            async move {
                let paths = app.state::<Paths>();
                let fetch = app.state::<FetchState>();
                let Ok(fetcher) = fetcher(&fetch) else {
                    return;
                };
                let result = requeue_all(&paths, fetcher, true);
                let _ = app.emit(SCHEDULED_FETCH, result.map_err(|e| e.to_string()));
            }
        },
    ));
}

/// 定期取得で fetch all を行ったことを画面に知らせるイベント。積んだ作品の数か、失敗の理由を送る
const SCHEDULED_FETCH: &str = "scheduled-fetch";

#[derive(serde::Serialize)]
pub struct ScheduleInfo {
    enabled: bool,
    /// 毎日の時刻（ローカル時刻、`HH:MM`）
    at: String,
}

/// 定期取得の設定（ADR 0019）。画面に表示する
#[tauri::command]
pub fn fetch_schedule(schedule: State<'_, ScheduleState>) -> ScheduleInfo {
    let schedule = schedule.0.read().unwrap();
    ScheduleInfo {
        enabled: schedule.enabled,
        at: schedule.at.format("%H:%M").to_string(),
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// ファイルに保存している設定。ファイルが壊れていれば既定値
    settings: Settings,
    /// 設定のファイルを読めなかったときの理由
    load_error: Option<String>,
    /// いま使っているクローラーの実行ファイル。見つからなければ null
    crawler_in_use: Option<String>,
    /// 実行ファイルを指定しなかったときに自動で見つかるもの
    crawler_found: Option<String>,
    /// いま使う epub-builder の実行ファイル。見つからなければ null（ADR 0024）
    epub_builder_in_use: Option<String>,
    /// epub-builder を指定しなかったときに自動で見つかるもの
    epub_builder_found: Option<String>,
    /// いま使う send-to-kindle の実行ファイル。見つからなければ null（ADR 0027）
    send_to_kindle_in_use: Option<String>,
    /// send-to-kindle を指定しなかったときに自動で見つかるもの
    send_to_kindle_found: Option<String>,
    /// .env を指定しなかったときに使うもの（ADR 0028）
    send_to_kindle_env_default: String,
    /// .env.example を指定しなかったときに使うもの
    send_to_kindle_env_example_default: String,
    /// .env.example と比べた .env の状態。.env.example がなければ null
    send_to_kindle_env_check: Option<crate::kindle::EnvCheck>,
    environment: String,
    subscriptions_path: String,
    settings_path: String,
    database_path: String,
}

/// 管理画面に出す設定と、データの置き場所（ADR 0020）
#[tauri::command]
pub fn get_settings(paths: State<'_, Paths>) -> SettingsView {
    let (settings, load_error) = match settings::load(&paths.settings) {
        Ok(settings) => (settings, None),
        Err(e) => (Settings::default(), Some(e)),
    };
    let path = |p: &std::path::Path| p.to_string_lossy().into_owned();
    // 環境ごとのアプリ用データ領域。データベースと同じディレクトリ
    let data_dir = paths.database.parent().unwrap_or(std::path::Path::new(""));
    SettingsView {
        crawler_in_use: crawler(&settings).map(|c| path(c.program())),
        crawler_found: ProcessCrawler::find().map(|c| path(c.program())),
        epub_builder_in_use: crate::epub::epub_builder(&settings).map(|p| path(&p)),
        epub_builder_found: crate::epub::find().map(|p| path(&p)),
        send_to_kindle_in_use: crate::kindle::send_to_kindle(&settings).map(|p| path(&p)),
        send_to_kindle_found: crate::kindle::find().map(|p| path(&p)),
        send_to_kindle_env_default: path(&crate::kindle::env_path(&Settings::default(), data_dir)),
        send_to_kindle_env_example_default: path(&crate::kindle::env_example_path(
            &Settings::default(),
            data_dir,
        )),
        send_to_kindle_env_check: crate::kindle::check_env(&settings, data_dir),
        settings,
        load_error,
        environment: paths.env.to_string(),
        subscriptions_path: path(&paths.subscriptions),
        settings_path: path(&paths.settings),
        database_path: path(&paths.database),
    }
}

/// 設定を確かめて保存し、すぐに反映する。クローラーを差し替え、定期取得の設定を書き換える（ADR 0020）
#[tauri::command]
pub fn save_settings(
    settings: Settings,
    paths: State<'_, Paths>,
    fetch: State<'_, FetchState>,
    schedule: State<'_, ScheduleState>,
) -> Result<(), String> {
    apply_settings(&paths.settings, &fetch.0, &schedule.0, &settings)
}

/// 設定を保存し、保存できたら取得のキューと定期取得に反映する。保存できなければ何も変えない
fn apply_settings(
    path: &std::path::Path,
    fetcher: &Fetcher,
    schedule: &RwLock<Schedule>,
    settings: &Settings,
) -> Result<(), String> {
    let next = settings::save(path, settings)?;
    fetcher.set_crawler(crawler(settings).map(|c| Arc::new(c) as Arc<dyn Crawler>));
    *schedule.write().unwrap() = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ScheduleSettings;
    use chrono::NaiveTime;

    fn executable(dir: &std::path::Path) -> String {
        let path = dir.join("crawler");
        std::fs::write(&path, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path.to_string_lossy().into_owned()
    }

    fn fetcher() -> Fetcher {
        let queues = Queues::new(tokio::runtime::Handle::current());
        Fetcher::new(queues, None, Arc::new(|_, _| {}))
    }

    #[tokio::test]
    async fn applies_saved_settings_to_crawler_and_schedule() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(settings::FILENAME);
        let fetcher = fetcher();
        let schedule = RwLock::new(Schedule::default());
        let settings = Settings {
            crawler_path: Some(executable(dir.path())),
            epub_builder_path: None,
            send_to_kindle_path: None,
            send_to_kindle_env_path: None,
            send_to_kindle_env_example_path: None,
            schedule: ScheduleSettings {
                enabled: false,
                at: "04:30".into(),
            },
        };

        apply_settings(&path, &fetcher, &schedule, &settings).unwrap();
        assert!(fetcher.has_crawler());
        let applied = *schedule.read().unwrap();
        assert!(!applied.enabled);
        assert_eq!(applied.at, NaiveTime::from_hms_opt(4, 30, 0).unwrap());
        assert_eq!(settings::load(&path).unwrap(), settings);
    }

    fn paths(dir: &std::path::Path) -> Paths {
        Paths {
            env: Environment::Development,
            subscriptions: dir.join(subscriptions::FILENAME),
            settings: dir.join(settings::FILENAME),
            database: dir.join("epubize.sqlite3"),
        }
    }

    #[test]
    fn uses_configured_crawler_path_over_auto_detection() {
        let settings = Settings {
            crawler_path: Some("/opt/example/novel-crawler".into()),
            ..Settings::default()
        };
        assert_eq!(
            crawler(&settings).unwrap().program(),
            std::path::Path::new("/opt/example/novel-crawler")
        );
        // 指定がなければ自動で探したものになる。見つかるかは環境による
        assert_eq!(
            crawler(&Settings::default()).map(|c| c.program().to_owned()),
            ProcessCrawler::find().map(|c| c.program().to_owned())
        );
    }

    #[tokio::test(start_paused = true)]
    async fn requeue_all_queues_tables_of_contents_for_every_subscription() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        for url in [
            "https://ncode.syosetu.com/n0001aa/",
            "https://syosetu.org/novel/100001/",
        ] {
            subscriptions::add(&paths.subscriptions, Subscription::from_url(url).unwrap()).unwrap();
        }
        let fetcher = fetcher();

        assert_eq!(requeue_all(&paths, &fetcher, true).unwrap(), 2);
        // ドメインごとに、先頭の待ち、目次の取得、その後の待ち（ADR 0016）
        assert_eq!(fetcher.queues().len(), 6);

        // 押し直すと積み直し、重複しない
        assert_eq!(requeue_all(&paths, &fetcher, false).unwrap(), 2);
        assert_eq!(fetcher.queues().len(), 6);
    }

    #[tokio::test]
    async fn requeue_all_reports_broken_subscriptions() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::write(&paths.subscriptions, "{ not json").unwrap();
        assert!(requeue_all(&paths, &fetcher(), true).is_err());
    }

    #[tokio::test]
    async fn requeue_all_with_no_subscriptions_queues_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        let fetcher = fetcher();
        assert_eq!(requeue_all(&paths, &fetcher, true).unwrap(), 0);
        assert!(fetcher.queues().is_empty());
    }

    #[tokio::test]
    async fn changes_nothing_when_settings_are_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(settings::FILENAME);
        let fetcher = fetcher();
        let schedule = RwLock::new(Schedule::default());
        let invalid = Settings {
            crawler_path: Some("/nonexistent/crawler".into()),
            ..Settings::default()
        };

        assert!(apply_settings(&path, &fetcher, &schedule, &invalid).is_err());
        assert!(!fetcher.has_crawler());
        assert_eq!(*schedule.read().unwrap(), Schedule::default());
        assert!(!path.exists());
    }
}

#[tauri::command]
pub fn fetch_all(paths: State<'_, Paths>, fetch: State<'_, FetchState>) -> Result<usize, String> {
    requeue_all(&paths, fetcher(&fetch)?, true)
}

#[tauri::command]
pub fn fetch_all_metadata(
    paths: State<'_, Paths>,
    fetch: State<'_, FetchState>,
) -> Result<usize, String> {
    requeue_all(&paths, fetcher(&fetch)?, false)
}

fn site(key: &str) -> Result<Site, String> {
    Site::ALL
        .into_iter()
        .find(|site| site.key() == key)
        .ok_or_else(|| format!("unknown site: {key}"))
}

/// 購読している作品を作品として加える。目次だけを取得する（kindlize の add）
#[tauri::command]
pub fn add_novel(
    site_key: &str,
    site_id: &str,
    fetch: State<'_, FetchState>,
) -> Result<(), String> {
    let subscription = Subscription {
        site: site(site_key)?,
        id: site_id.to_owned(),
    };
    fetcher(&fetch)?.enqueue(Request::new(Command::Toc, subscription.url(), false))
}

/// 作品の目次を取り直し、未取得と改稿された話の本文と挿絵を取得する
#[tauri::command]
pub fn fetch_novel(
    novel_id: i64,
    database: State<'_, Database>,
    fetch: State<'_, FetchState>,
) -> Result<(), String> {
    let (site_key, site_id) = {
        let conn = database.0.lock().map_err(|e| e.to_string())?;
        query::novel_site(&conn, novel_id)?.ok_or_else(|| format!("no novel with id {novel_id}"))?
    };
    let subscription = Subscription {
        site: site(&site_key)?,
        id: site_id,
    };
    fetcher(&fetch)?.enqueue(Request::new(Command::Toc, subscription.url(), true))
}

/// 話の本文を取り直す
#[tauri::command]
pub fn refetch_episode(
    episode_id: i64,
    database: State<'_, Database>,
    fetch: State<'_, FetchState>,
) -> Result<(), String> {
    let url = {
        let conn = database.0.lock().map_err(|e| e.to_string())?;
        query::episode_url(&conn, episode_id)?
            .ok_or_else(|| format!("no episode with id {episode_id}"))?
    };
    fetcher(&fetch)?.enqueue(Request::new(Command::Episode, url, true))
}

#[tauri::command]
pub fn remove_episodes(novel_id: i64, database: State<'_, Database>) -> Result<usize, String> {
    let conn = database.0.lock().map_err(|e| e.to_string())?;
    query::remove_episodes(&conn, novel_id)
}

#[tauri::command]
pub fn list_novels(database: State<'_, Database>) -> Result<Vec<query::NovelSummary>, String> {
    let conn = database.0.lock().map_err(|e| e.to_string())?;
    query::list_novels(&conn)
}

#[tauri::command]
pub fn novel_detail(
    novel_id: i64,
    database: State<'_, Database>,
) -> Result<Option<query::NovelDetail>, String> {
    let conn = database.0.lock().map_err(|e| e.to_string())?;
    query::novel_detail(&conn, novel_id)
}

#[tauri::command]
pub fn episode_detail(
    episode_id: i64,
    database: State<'_, Database>,
) -> Result<Option<query::EpisodeDetail>, String> {
    let conn = database.0.lock().map_err(|e| e.to_string())?;
    query::episode_detail(&conn, episode_id)
}

#[tauri::command]
pub fn latest_episodes(
    limit: i64,
    database: State<'_, Database>,
) -> Result<Vec<query::LatestEpisode>, String> {
    let conn = database.0.lock().map_err(|e| e.to_string())?;
    query::latest_episodes(&conn, limit)
}

#[tauri::command]
pub fn set_normalize_options(
    novel_id: i64,
    options: Option<Value>,
    database: State<'_, Database>,
) -> Result<(), String> {
    let conn = database.0.lock().map_err(|e| e.to_string())?;
    query::set_normalize_options(&conn, novel_id, options.as_ref())
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod app_tests;
