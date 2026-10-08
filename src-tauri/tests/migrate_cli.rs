//! マイグレーションの CLI（ADR 0013）の結合テスト。一時的なデータベースに対して実行ファイルを動かす。

use std::path::Path;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_migrate");

fn run(db: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .arg("--db")
        .arg(db)
        .args(args)
        .env_remove("EPUBIZE_DB")
        .env_remove("EPUBIZE_ENV")
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn user_version(db: &Path) -> i64 {
    rusqlite::Connection::open(db)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

#[test]
fn status_reports_missing_database_without_creating_it() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("nested").join("epubize.sqlite3");
    let output = run(&db, &["status"]);
    assert!(output.status.success());
    assert!(
        stdout(&output).contains("not created yet"),
        "{}",
        stdout(&output)
    );
    assert!(!db.exists());
}

#[test]
fn up_applies_all_migrations_then_does_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("epubize.sqlite3");

    let first = run(&db, &["up"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        stdout(&first).contains("applied 0001_create_tables"),
        "{}",
        stdout(&first)
    );
    let latest = user_version(&db);
    assert!(latest >= 1);

    let second = run(&db, &["up"]);
    assert!(second.status.success());
    assert!(
        stdout(&second).contains("already up to date"),
        "{}",
        stdout(&second)
    );
    assert_eq!(user_version(&db), latest);

    let status = run(&db, &["status"]);
    assert!(stdout(&status).contains(&format!("version: {latest} / {latest}")));
}

#[test]
fn reset_recreates_database_and_drops_data() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("epubize.sqlite3");
    assert!(run(&db, &["up"]).status.success());
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute(
            "INSERT INTO novels (site, site_id, url, title, author_name, description, episode_count, metadata_fetched_at)
             VALUES ('narou', 'n0001aa', 'u', 't', 'a', 'd', 0, 'x')",
            [],
        )
        .unwrap();

    let output = run(&db, &["reset"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("removed"));
    let count: i64 = rusqlite::Connection::open(&db)
        .unwrap()
        .query_row("SELECT count(*) FROM novels", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn rejects_unknown_command_and_invalid_new_name() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("epubize.sqlite3");
    for args in [&["unknown"][..], &[], &["new"], &["new", "Bad Name"]] {
        let output = run(&db, args);
        assert!(!output.status.success(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).starts_with("error:"),
            "{args:?}"
        );
    }
    assert!(!db.exists());
}

#[test]
fn rejects_db_flag_without_path() {
    let output = Command::new(BIN).args(["up", "--db"]).output().unwrap();
    assert!(!output.status.success());
}
