//! 画面から呼ぶ Tauri のコマンド。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db::Database;
use crate::environment::Environment;
use crate::fetch::crawler::{Command, ProcessCrawler, Request};
use crate::fetch::queue::Queues;
use crate::fetch::{self, Fetcher};
use crate::library::query;
use crate::pipeline;
use crate::subscriptions::{self, AddResult, Site, Subscription};

/// 環境ごとに分かれたファイルの置き場所（ADR 0014）
pub struct Paths {
    pub env: Environment,
    pub subscriptions: PathBuf,
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

/// 取得のキュー（ADR 0015）。クローラーが見つからなければ None
pub struct FetchState(Option<Fetcher>);

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
fn on_done(app: &AppHandle, request: Request, result: Result<String, fetch::crawler::CrawlError>) {
    let outcome = result.map_err(|e| e.to_string()).and_then(|stdout| {
        let database = app.state::<Database>();
        let mut conn = database.0.lock().map_err(|e| e.to_string())?;
        pipeline::import(&mut conn, &request, &stdout)
    });
    let error = match outcome {
        Ok(next) => {
            if let Some(fetcher) = &app.state::<FetchState>().0 {
                for request in next {
                    let _ = fetcher.enqueue(request);
                }
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

pub fn fetch_state(app: &AppHandle) -> FetchState {
    let Some(crawler) = ProcessCrawler::find() else {
        return FetchState(None);
    };
    let queues = Queues::new(tauri::async_runtime::handle().inner().clone());
    let handle = app.clone();
    let done: fetch::OnDone = Arc::new(move |request, result| on_done(&handle, request, result));
    FetchState(Some(Fetcher::new(queues, Arc::new(crawler), done)))
}

fn fetcher<'a>(fetch: &'a State<'_, FetchState>) -> Result<&'a Fetcher, String> {
    fetch.0.as_ref().ok_or_else(|| {
        format!(
            "クローラーが見つかりません。PATH に {} を置くか、環境変数 EPUBIZE_CRAWLER に実行ファイルのパスを指定してください",
            fetch::crawler::CRAWLER_NAME
        )
    })
}

#[derive(serde::Serialize)]
pub struct FetchStatus {
    /// クローラーが見つかったか
    configured: bool,
    /// キューに残っているタスクの数（取得と待ちの両方を数える）
    queued: usize,
}

#[tauri::command]
pub fn fetch_status(fetch: State<'_, FetchState>) -> FetchStatus {
    FetchStatus {
        configured: fetch.0.is_some(),
        queued: fetch.0.as_ref().map_or(0, |f| f.queues().len()),
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

/// 毎日決めた時刻に fetch all を行う（ADR 0019）。クローラーが見つからなければ何もしない
pub fn start_scheduled_fetch(app: AppHandle, schedule: crate::schedule::Schedule) {
    tauri::async_runtime::spawn(crate::schedule::run_daily(
        schedule,
        crate::schedule::local_now,
        move || {
            let app = app.clone();
            async move {
                let paths = app.state::<Paths>();
                let fetch = app.state::<FetchState>();
                let Some(fetcher) = &fetch.0 else {
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
pub fn fetch_schedule(schedule: State<'_, crate::schedule::Schedule>) -> ScheduleInfo {
    ScheduleInfo {
        enabled: schedule.enabled,
        at: schedule.at.format("%H:%M").to_string(),
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
