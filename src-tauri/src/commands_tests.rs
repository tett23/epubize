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
        downloads: dir.join("downloads"),
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
    assert_eq!(
        view.send_to_kindle_env_default,
        dir.path().join(".env").to_string_lossy()
    );
    assert_eq!(
        view.send_to_kindle_env_example_default,
        dir.path().join(".env.example").to_string_lossy()
    );
    assert_eq!(view.send_to_kindle_env_check, None);

    let crawler = executable(dir.path());
    let env = dir.path().join(".env");
    let example = dir.path().join(".env.example");
    std::fs::write(&env, "EMAIL=a@example.com\n").unwrap();
    std::fs::write(&example, "EMAIL=\nSMTP_PASSWORD=\n").unwrap();
    let settings = Settings {
        crawler_path: Some(crawler.clone()),
        epub_builder_path: Some(crawler.clone()),
        send_to_kindle_path: Some(crawler.clone()),
        send_to_kindle_env_path: Some(env.to_string_lossy().into_owned()),
        send_to_kindle_env_example_path: Some(example.to_string_lossy().into_owned()),
        kindlegen_path: Some(crawler.clone()),
        striptool_path: Some(crawler.clone()),
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
    assert_eq!(
        view.send_to_kindle_in_use.as_deref(),
        Some(crawler.as_str())
    );
    assert_eq!(view.kindlegen_in_use.as_deref(), Some(crawler.as_str()));
    assert_eq!(view.striptool_in_use.as_deref(), Some(crawler.as_str()));
    assert_eq!(
        view.send_to_kindle_env_check,
        Some(crate::kindle::EnvCheck::Missing {
            keys: vec!["SMTP_PASSWORD".into()]
        })
    );
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

/// 実行できる sh のスクリプトを作る
fn script(dir: &Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path.to_string_lossy().into_owned()
}

/// `--output` の次の引数に、プロジェクトの 1 話目を書く偽の epub-builder
const FAKE_EPUB_BUILDER: &str = "project=$2\nwhile [ \"$1\" != --output ]; do shift; done\ncat \"$project\"/body/*/0001.md \"$project\"/body/0001.md > \"$2\" 2>/dev/null\nexit 0";

fn episode_ids(app: &App<MockRuntime>) -> Vec<i64> {
    let database = app.state::<Database>();
    let conn = database.0.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id FROM episodes ORDER BY no").unwrap();
    stmt.query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[tokio::test]
async fn downloads_epub_and_zip_into_downloads() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), false);
    import_novel(&app);
    let novel = list_novels(app.state()).unwrap()[0].id;
    let scope = export::Scope::Novel(novel);

    let settings = Settings {
        epub_builder_path: Some(script(dir.path(), "epub-builder", FAKE_EPUB_BUILDER)),
        ..Settings::default()
    };
    save_settings(settings, app.state(), app.state(), app.state()).unwrap();

    let path = download_epub(scope, app.state(), app.state())
        .await
        .unwrap();
    assert_eq!(
        Path::new(&path),
        dir.path().join("downloads/合成データの作品.epub")
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "# 第1話\n\n　本文。\n\n---\n\n後書き\n"
    );
    // 同じ名前があれば番号を付ける
    let again = download_epub(scope, app.state(), app.state())
        .await
        .unwrap();
    assert!(again.ends_with("合成データの作品 (2).epub"), "{again}");

    let zip = download_zip(
        export::Scope::Episode(episode_ids(&app)[0]),
        app.state(),
        app.state(),
    )
    .unwrap();
    assert!(
        zip.ends_with("downloads/合成データの作品 第1話.zip"),
        "{zip}"
    );
    let archive = zip::ZipArchive::new(std::fs::File::open(&zip).unwrap()).unwrap();
    assert!(
        archive
            .file_names()
            .any(|name| name == "合成データの作品 第1話/book.toml")
    );

    // 本文のない話は書き出せない
    let unfetched = export::Scope::Episode(episode_ids(&app)[1]);
    assert!(
        download_epub(unfetched, app.state(), app.state())
            .await
            .is_err()
    );
    assert!(download_zip(unfetched, app.state(), app.state()).is_err());
}

#[tokio::test]
async fn sends_epub_to_kindle_and_records_sent_time() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), false);
    import_novel(&app);
    let novel = list_novels(app.state()).unwrap()[0].id;
    let scope = export::Scope::Novel(novel);
    let args = dir.path().join("args");
    let env = dir.path().join("kindle.env");
    std::fs::write(&env, "EMAIL=a@example.com\n").unwrap();
    let settings = Settings {
        epub_builder_path: Some(script(dir.path(), "epub-builder", FAKE_EPUB_BUILDER)),
        send_to_kindle_path: Some(script(
            dir.path(),
            "send-to-kindle",
            &format!(
                "printf '%s\\n' \"$@\" > '{}'\ncat \"$3\" >> '{}'",
                args.display(),
                args.display()
            ),
        )),
        send_to_kindle_env_path: Some(env.to_string_lossy().into_owned()),
        ..Settings::default()
    };
    save_settings(settings.clone(), app.state(), app.state(), app.state()).unwrap();

    assert_eq!(send_to_kindle(scope, app.state(), app.state()).await, Ok(1));
    let sent = std::fs::read_to_string(&args).unwrap();
    let lines: Vec<&str> = sent.lines().collect();
    assert_eq!(lines[0], "--env-file");
    assert_eq!(lines[1], env.to_string_lossy());
    assert!(lines[2].ends_with("/合成データの作品.epub"), "{sent}");
    assert_eq!(lines[3], "# 第1話");
    let detail = novel_detail(novel, app.state()).unwrap().unwrap();
    assert!(detail.episodes[0].sent_at.is_some());
    assert!(detail.episodes[1].sent_at.is_none());

    // 送れなければ、送った日時を記録しない
    let fresh = tempfile::tempdir().unwrap();
    let app = app_with_failing_sender(fresh.path(), settings);
    import_novel(&app);
    let novel = list_novels(app.state()).unwrap()[0].id;
    let error = send_to_kindle(export::Scope::Novel(novel), app.state(), app.state())
        .await
        .unwrap_err();
    assert!(error.contains("SMTP_PASSWORD"), "{error}");
    let detail = novel_detail(novel, app.state()).unwrap().unwrap();
    assert!(detail.episodes[0].sent_at.is_none());
}

/// `settings` の send-to-kindle を、設定が足りずに失敗するものに差し替えたアプリ
fn app_with_failing_sender(dir: &Path, settings: Settings) -> App<MockRuntime> {
    let app = app(dir, false);
    let settings = Settings {
        send_to_kindle_path: Some(script(
            dir,
            "failing",
            "echo 'send-to-kindle: 次の設定がありません: SMTP_PASSWORD' >&2\nexit 1",
        )),
        ..settings
    };
    save_settings(settings, app.state(), app.state(), app.state()).unwrap();
    app
}

#[tokio::test]
async fn export_commands_need_executables() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), false);
    import_novel(&app);
    let novel = list_novels(app.state()).unwrap()[0].id;
    let scope = export::Scope::Novel(novel);
    // 存在しない場所を指す設定を書き、自動で探したものを使わせない
    std::fs::write(
        dir.path().join(settings::FILENAME),
        r#"{"epubBuilderPath":"/nonexistent/epub-builder","sendToKindlePath":"/nonexistent/send-to-kindle"}"#,
    )
    .unwrap();
    let error = download_epub(scope, app.state(), app.state())
        .await
        .unwrap_err();
    assert!(
        error.starts_with("epub-builder を起動できません"),
        "{error}"
    );

    std::fs::write(
        dir.path().join(settings::FILENAME),
        format!(
            r#"{{"epubBuilderPath":{:?},"sendToKindlePath":"/nonexistent/send-to-kindle"}}"#,
            script(dir.path(), "epub-builder", FAKE_EPUB_BUILDER)
        ),
    )
    .unwrap();
    let error = send_to_kindle(scope, app.state(), app.state())
        .await
        .unwrap_err();
    assert!(
        error.starts_with("send-to-kindle を起動できません"),
        "{error}"
    );
    let detail = novel_detail(novel, app.state()).unwrap().unwrap();
    assert!(detail.episodes[0].sent_at.is_none());
}

#[tokio::test]
async fn downloads_stripped_mobi_into_downloads() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), false);
    import_novel(&app);
    let novel = list_novels(app.state()).unwrap()[0].id;
    let settings = Settings {
        epub_builder_path: Some(script(dir.path(), "epub-builder", FAKE_EPUB_BUILDER)),
        // EPUB の中身に印を付けて、-o の名前で入力の隣に書く
        kindlegen_path: Some(script(
            dir.path(),
            "kindlegen",
            "in=$1\nwhile [ \"$1\" != -o ]; do shift; done\n{ echo kindlegen; cat \"$in\"; } > \"$(dirname \"$in\")/$2\"\nexit 1",
        )),
        // 入力の隣の数字のディレクトリに書く
        striptool_path: Some(script(
            dir.path(),
            "striptool",
            "out=$(dirname \"$1\")/1902840192\nmkdir -p \"$out\"\n{ echo stripped; cat \"$1\"; } > \"$out/$(basename \"$1\")\"",
        )),
        ..Settings::default()
    };
    save_settings(settings.clone(), app.state(), app.state(), app.state()).unwrap();

    let path = download_mobi(export::Scope::Novel(novel), app.state(), app.state())
        .await
        .unwrap();
    assert_eq!(
        Path::new(&path),
        dir.path().join("downloads/合成データの作品.mobi")
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "stripped\nkindlegen\n# 第1話\n\n　本文。\n\n---\n\n後書き\n"
    );

    // striptool がなければ、その誤りを返す。ほかの指定は残し、自動で探したものを使わせない
    let missing = Settings {
        striptool_path: Some("/nonexistent/striptool".into()),
        ..settings
    };
    std::fs::write(
        dir.path().join(settings::FILENAME),
        serde_json::to_string(&missing).unwrap(),
    )
    .unwrap();
    let error = download_mobi(export::Scope::Novel(novel), app.state(), app.state())
        .await
        .unwrap_err();
    assert!(error.starts_with("striptool を起動できません"), "{error}");
}
