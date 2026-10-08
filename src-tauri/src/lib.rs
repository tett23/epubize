pub mod db;
pub mod environment;
pub mod fetch;
pub mod subscriptions;
mod window_state;

use std::path::PathBuf;
use std::sync::Arc;

use environment::Environment;
use fetch::Fetcher;
use fetch::crawler::{Command, ProcessCrawler, Request};
use fetch::queue::Queues;
use subscriptions::{AddResult, Subscription};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};

/// 環境ごとに分かれたファイルの置き場所（ADR 0014）
struct Paths {
    env: Environment,
    subscriptions: PathBuf,
}

#[derive(serde::Serialize)]
struct SubscriptionItem {
    #[serde(flatten)]
    subscription: Subscription,
    url: String,
}

/// データの置き場所を決めている環境。画面の見出しに出す
#[tauri::command]
fn environment(paths: State<'_, Paths>) -> String {
    paths.env.to_string()
}

#[tauri::command]
fn list_subscriptions(paths: State<'_, Paths>) -> Result<Vec<SubscriptionItem>, String> {
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
fn add_subscription(url: &str, paths: State<'_, Paths>) -> Result<AddResult, String> {
    let subscription = Subscription::from_url(url).map_err(|e| e.to_string())?;
    subscriptions::add(&paths.subscriptions, subscription).map_err(|e| e.to_string())
}

/// 取得のキュー（ADR 0015）。クローラーが指定されていなければ None
struct FetchState(Option<Fetcher>);

/// 取得が終わったことを画面に知らせるイベント
#[derive(Clone, serde::Serialize)]
struct FetchDone {
    command: &'static str,
    url: String,
    /// 失敗したときの理由。成功なら null
    error: Option<String>,
}

const FETCH_DONE: &str = "fetch-done";

fn fetcher(app: &AppHandle) -> FetchState {
    let Some(crawler) = ProcessCrawler::from_env() else {
        return FetchState(None);
    };
    let queues = Queues::new(tauri::async_runtime::handle().inner().clone());
    let app = app.clone();
    let on_done: fetch::OnDone = Arc::new(move |request: Request, result| {
        // 取得した結果を保存する先はまだないため、画面に知らせるだけにする（ADR 0016）
        let _ = app.emit(
            FETCH_DONE,
            FetchDone {
                command: request.command.as_str(),
                url: request.url,
                error: result.err().map(|e| e.to_string()),
            },
        );
    });
    FetchState(Some(Fetcher::new(queues, Arc::new(crawler), on_done)))
}

#[derive(serde::Serialize)]
struct FetchStatus {
    /// クローラーが指定されているか
    configured: bool,
    /// キューに残っているタスクの数（取得と待ちの両方を数える）
    queued: usize,
}

#[tauri::command]
fn fetch_status(fetch: State<'_, FetchState>) -> FetchStatus {
    FetchStatus {
        configured: fetch.0.is_some(),
        queued: fetch.0.as_ref().map_or(0, |f| f.queues().len()),
    }
}

/// 全てのキューを破棄し、購読している全作品の目次の取得を積み直す。積んだ数を返す
#[tauri::command]
fn fetch_all(paths: State<'_, Paths>, fetch: State<'_, FetchState>) -> Result<usize, String> {
    let fetcher = fetch
        .0
        .as_ref()
        .ok_or("クローラーが指定されていません。環境変数 EPUBIZE_CRAWLER に実行ファイルのパスを指定してください")?;
    let requests: Vec<Request> = subscriptions::list(&paths.subscriptions)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|subscription| Request {
            command: Command::Toc,
            url: subscription.url(),
        })
        .collect();
    let total = requests.len();
    let errors = fetcher.fetch_all(requests);
    if let Some(first) = errors.first() {
        return Err(first.clone());
    }
    Ok(total)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            environment,
            list_subscriptions,
            add_subscription,
            fetch_status,
            fetch_all
        ])
        .setup(|app| {
            let env = Environment::current()?;

            // ファイルがなければ作り、未適用のマイグレーションを全て流す（ADR 0013）
            let db_path = db::default_path(env).ok_or("cannot determine the database path")?;
            let conn = db::open(&db_path)?;
            app.manage(db::Database(std::sync::Mutex::new(conn)));

            let subscriptions = subscriptions::default_path(env)
                .ok_or("cannot determine the subscriptions path")?;
            app.manage(Paths { env, subscriptions });
            app.manage(window_state::WindowStateStore::new(env));
            app.manage(fetcher(app.handle()));

            let window = app
                .get_webview_window("main")
                .expect("main window is defined in tauri.conf.json");
            // 位置を戻すまで隠しておき、既定の位置から動いて見えないようにする
            window_state::restore_and_track(&window)?;
            window.show()?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                window_state::save(app);
            }
        });
}
