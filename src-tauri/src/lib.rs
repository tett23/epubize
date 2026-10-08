pub mod db;
pub mod environment;
pub mod fetch;
pub mod subscriptions;
mod window_state;

use std::path::PathBuf;

use environment::Environment;
use subscriptions::{AddResult, Subscription};
use tauri::{Manager, RunEvent, State};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            environment,
            list_subscriptions,
            add_subscription
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
