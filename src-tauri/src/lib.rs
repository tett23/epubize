mod commands;
pub mod db;
pub mod environment;
pub mod fetch;
pub mod library;
pub mod pipeline;
pub mod schedule;
pub mod settings;
pub mod subscriptions;
mod window_state;

use commands::Paths;
use environment::Environment;
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::environment,
            commands::list_subscriptions,
            commands::add_subscription,
            commands::remove_subscription,
            commands::get_settings,
            commands::save_settings,
            commands::fetch_status,
            commands::fetch_schedule,
            commands::fetch_all,
            commands::fetch_all_metadata,
            commands::add_novel,
            commands::fetch_novel,
            commands::refetch_episode,
            commands::remove_episodes,
            commands::list_novels,
            commands::novel_detail,
            commands::episode_detail,
            commands::latest_episodes,
            commands::set_normalize_options,
        ])
        .setup(|app| {
            let env = Environment::current()?;

            // ファイルがなければ作り、未適用のマイグレーションを全て流す（ADR 0013）
            let db_path = db::default_path(env).ok_or("cannot determine the database path")?;
            let conn = db::open(&db_path)?;
            app.manage(db::Database(std::sync::Mutex::new(conn)));

            let subscriptions = subscriptions::default_path(env)
                .ok_or("cannot determine the subscriptions path")?;
            let settings_path =
                settings::default_path(env).ok_or("cannot determine the settings path")?;
            // 設定のファイルが壊れていても起動できるよう、読めなければ既定値で動かす。理由は管理画面に出す
            let current = settings::load(&settings_path).unwrap_or_default();
            let schedule = current.validate().unwrap_or_default();
            app.manage(Paths {
                env,
                subscriptions,
                settings: settings_path,
                database: db_path,
            });
            app.manage(window_state::WindowStateStore::new(env));
            app.manage(commands::fetch_state(app.handle(), &current));
            app.manage(commands::ScheduleState(std::sync::RwLock::new(schedule)));
            commands::start_scheduled_fetch(app.handle().clone());

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
