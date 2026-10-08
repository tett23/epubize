pub mod db;
mod window_state;

use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(window_state::WindowStateStore::default())
        .setup(|app| {
            // ファイルがなければ作り、未適用のマイグレーションを全て流す（ADR 0013）
            let path = db::default_path().ok_or("cannot determine the database path")?;
            let conn = db::open(&path)?;
            app.manage(db::Database(std::sync::Mutex::new(conn)));

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
