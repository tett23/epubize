//! Tauri のコマンドを、偽の実行環境（`tauri::test`）の上のアプリで呼んで確かめる。
//! ファイルの置き場所は全て一時ディレクトリにする。データは合成したもの。

use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use rusqlite::Connection;
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use tauri::{App, Listener, Manager};

use super::*;
use crate::db::Database;
use crate::fetch::crawler::CrawlError;
use crate::library::records::tests as fixtures;

const NOVEL_URL: &str = "https://ncode.syosetu.com/n0001aa/";

fn database() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    crate::db::migrate::migrate(&mut conn, crate::db::migrate::MIGRATIONS).unwrap();
    conn
}

fn executable(dir: &Path) -> String {
    let path = dir.join("crawler");
    std::fs::write(&path, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path.to_string_lossy().into_owned()
}

/// アプリと同じ状態を持つ、偽の実行環境のアプリ。`crawler` が偽ならクローラーなし
fn app(dir: &Path, crawler: bool) -> App<MockRuntime> {
    let app = mock_builder().build(mock_context(noop_assets())).unwrap();
    app.manage(Database(Mutex::new(database())));
    app.manage(Paths {
        env: Environment::Development,
        subscriptions: dir.join(subscriptions::FILENAME),
        settings: dir.join(settings::FILENAME),
        database: dir.join("epubize.sqlite3"),
    });
    let crawler: Option<Arc<dyn Crawler>> =
        crawler.then(|| Arc::new(ProcessCrawler::new(executable(dir))) as Arc<dyn Crawler>);
    let queues = Queues::new(tokio::runtime::Handle::current());
    app.manage(FetchState(Fetcher::new(
        queues,
        crawler,
        Arc::new(|_, _| {}),
    )));
    app.manage(ScheduleState(RwLock::new(Schedule::default())));
    app
}

fn queued(app: &App<MockRuntime>) -> usize {
    app.state::<FetchState>().0.queues().len()
}

/// 目次と 1 話目の本文を保存した状態にする
fn import_novel(app: &App<MockRuntime>) {
    let database = app.state::<Database>();
    let mut conn = database.0.lock().unwrap();
    let toc = [
        fixtures::NOVEL.to_owned(),
        fixtures::toc_entry(1, None),
        fixtures::toc_entry(2, None),
    ]
    .join("\n");
    pipeline::import(
        &mut conn,
        &Request::new(Command::Toc, NOVEL_URL, false),
        &toc,
    )
    .unwrap();
    pipeline::import(
        &mut conn,
        &Request::new(Command::Episode, format!("{NOVEL_URL}1/"), false),
        &fixtures::episode(1, &[]),
    )
    .unwrap();
}

#[tokio::test]
async fn reports_environment() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), true);
    assert_eq!(environment(app.state()), "development");
}

#[tokio::test]
async fn adds_lists_and_removes_subscriptions() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), true);
    assert!(list_subscriptions(app.state()).unwrap().is_empty());

    let added = add_subscription(NOVEL_URL, app.state()).unwrap();
    assert!(added.added);
    assert!(add_subscription("https://example.com/", app.state()).is_err());

    let items = list_subscriptions(app.state()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].url, NOVEL_URL);

    assert!(remove_subscription("narou", "n0001aa", app.state()).unwrap());
    assert!(!remove_subscription("narou", "n0001aa", app.state()).unwrap());
    assert!(remove_subscription("unknown", "x", app.state()).is_err());
    assert!(list_subscriptions(app.state()).unwrap().is_empty());
}

#[tokio::test]
async fn fetch_commands_need_a_crawler() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), false);
    let status = fetch_status(app.state());
    assert!(!status.configured);
    let error = fetch_all(app.state(), app.state()).unwrap_err();
    assert!(error.contains("クローラーが見つかりません"), "{error}");
    assert!(fetch_all_metadata(app.state(), app.state()).is_err());
    assert!(add_novel("narou", "n0001aa", app.state()).is_err());
}

#[tokio::test(start_paused = true)]
async fn fetch_all_queues_subscriptions() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), true);
    add_subscription(NOVEL_URL, app.state()).unwrap();
    assert!(fetch_status(app.state()).configured);

    assert_eq!(fetch_all(app.state(), app.state()).unwrap(), 1);
    // 先頭の待ち、目次の取得、その後の待ち
    assert_eq!(queued(&app), 3);
    assert_eq!(fetch_status(app.state()).queued, 3);
    assert_eq!(fetch_all_metadata(app.state(), app.state()).unwrap(), 1);
    assert_eq!(queued(&app), 3);
}

#[tokio::test(start_paused = true)]
async fn queues_per_novel_and_per_episode_fetches() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), true);
    import_novel(&app);
    let novel = list_novels(app.state()).unwrap().remove(0);
    let detail = novel_detail(novel.id, app.state()).unwrap().unwrap();

    add_novel("hameln", "100001", app.state()).unwrap();
    assert_eq!(queued(&app), 2);
    fetch_novel(novel.id, app.state(), app.state()).unwrap();
    assert_eq!(queued(&app), 4);
    refetch_episode(detail.episodes[0].id, app.state(), app.state()).unwrap();
    assert_eq!(queued(&app), 6);

    assert!(fetch_novel(9999, app.state(), app.state()).is_err());
    assert!(refetch_episode(9999, app.state(), app.state()).is_err());
    assert!(add_novel("unknown", "x", app.state()).is_err());
}

#[tokio::test]
async fn reads_library_through_commands() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), true);
    import_novel(&app);

    let novels = list_novels(app.state()).unwrap();
    assert_eq!((novels[0].fetched, novels[0].total), (1, 2));
    let detail = novel_detail(novels[0].id, app.state()).unwrap().unwrap();
    let episode = episode_detail(detail.episodes[0].id, app.state())
        .unwrap()
        .unwrap();
    assert_eq!(episode.body.as_deref(), Some("　本文。"));
    assert_eq!(latest_episodes(10, app.state()).unwrap().len(), 2);
    assert_eq!(novel_detail(9999, app.state()).unwrap(), None);

    let options = serde_json::json!({"direction": "horizontal"});
    set_normalize_options(novels[0].id, Some(options.clone()), app.state()).unwrap();
    assert_eq!(
        novel_detail(novels[0].id, app.state())
            .unwrap()
            .unwrap()
            .normalize_options,
        Some(options)
    );

    assert_eq!(remove_episodes(novels[0].id, app.state()).unwrap(), 2);
    assert!(
        novel_detail(novels[0].id, app.state())
            .unwrap()
            .unwrap()
            .episodes
            .is_empty()
    );
}

#[tokio::test]
async fn shows_and_saves_settings() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), false);

    let view = get_settings(app.state());
    assert_eq!(view.settings, Settings::default());
    assert_eq!(view.load_error, None);
    assert_eq!(view.environment, "development");
    assert!(view.settings_path.ends_with("settings.json"));

    let crawler = executable(dir.path());
    let settings = Settings {
        crawler_path: Some(crawler.clone()),
        epub_builder_path: Some(crawler.clone()),
        schedule: crate::settings::ScheduleSettings {
            enabled: true,
            at: "05:15".into(),
        },
    };
    save_settings(settings.clone(), app.state(), app.state(), app.state()).unwrap();
    assert!(fetch_status(app.state()).configured);
    let schedule = fetch_schedule(app.state());
    assert!(schedule.enabled);
    assert_eq!(schedule.at, "05:15");

    let view = get_settings(app.state());
    assert_eq!(view.settings, settings);
    assert_eq!(view.crawler_in_use.as_deref(), Some(crawler.as_str()));
    assert_eq!(view.epub_builder_in_use.as_deref(), Some(crawler.as_str()));
}

#[tokio::test]
async fn shows_defaults_with_reason_when_settings_file_is_broken() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), true);
    std::fs::write(dir.path().join(settings::FILENAME), "{ not json").unwrap();
    let view = get_settings(app.state());
    assert_eq!(view.settings, Settings::default());
    assert!(view.load_error.is_some());
}

#[tokio::test(start_paused = true)]
async fn on_done_saves_results_queues_follow_ups_and_notifies() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), true);
    let events: Arc<Mutex<Vec<String>>> = Arc::default();
    let recorded = events.clone();
    app.listen(FETCH_DONE, move |event| {
        recorded.lock().unwrap().push(event.payload().to_owned());
    });

    let toc = [
        fixtures::NOVEL.to_owned(),
        fixtures::toc_entry(1, None),
        fixtures::toc_entry(2, None),
    ]
    .join("\n");
    on_done(
        app.handle(),
        Request::new(Command::Toc, NOVEL_URL, true),
        Ok(toc),
    );

    // 目次を保存し、2 話分の取得（取得と待ちの組）を積む
    assert_eq!(list_novels(app.state()).unwrap().len(), 1);
    assert_eq!(queued(&app), 4);

    on_done(
        app.handle(),
        Request::new(Command::Toc, "https://ncode.syosetu.com/n0002bb/", true),
        Err(CrawlError {
            kind: crate::fetch::crawler::ErrorKind::NotFound,
            message: "fake".into(),
            retry_after: None,
        }),
    );
    on_done(
        app.handle(),
        Request::new(Command::Toc, NOVEL_URL, true),
        Ok("not json".into()),
    );

    let events = events.lock().unwrap().clone();
    assert_eq!(events.len(), 3);
    let parsed: Vec<serde_json::Value> = events
        .iter()
        .map(|e| serde_json::from_str(e).unwrap())
        .collect();
    assert_eq!(parsed[0]["command"], "toc");
    assert_eq!(parsed[0]["error"], serde_json::Value::Null);
    assert!(parsed[1]["error"].as_str().unwrap().starts_with("NotFound"));
    assert!(parsed[2]["error"].as_str().unwrap().contains("not JSON"));
    // 失敗したものは何も積まない
    assert_eq!(queued(&app), 4);
}
