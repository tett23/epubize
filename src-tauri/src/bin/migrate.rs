//! マイグレーションの CLI（ADR 0013）。mise のタスクから呼ぶ。
//!
//! 使い方: migrate [--db <path>] <command>
//!   up           未適用のマイグレーションを適用する
//!   status       適用済みの版と、未適用のマイグレーションを表示する
//!   new <name>   次の版の空のマイグレーションを migrations/ に作る
//!   reset        データベースのファイルを消し、全てのマイグレーションを流し直す
//!
//! データベースのパスは --db、環境変数 EPUBIZE_DB、アプリの既定の場所の順に決める。
//! 既定の場所は環境（EPUBIZE_ENV、なければ debug ビルドの development）ごとに分かれる（ADR 0014）。

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs};

use epubize_lib::db::{self, migrate};
use epubize_lib::environment::Environment;

const USAGE: &str = "usage: migrate [--db <path>] <up|status|new <name>|reset>";

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let db_arg = match args.iter().position(|a| a == "--db") {
        Some(i) if i + 1 < args.len() => {
            let path = args.remove(i + 1);
            args.remove(i);
            Some(PathBuf::from(path))
        }
        Some(_) => return Err(USAGE.into()),
        None => None,
    };
    let db_path = || -> Result<PathBuf, Box<dyn std::error::Error>> {
        db_arg
            .clone()
            .or_else(|| env::var_os("EPUBIZE_DB").map(PathBuf::from))
            .map(Ok)
            .unwrap_or_else(|| {
                db::default_path(Environment::current()?)
                    .ok_or_else(|| "cannot determine the database path; pass --db".to_owned())
            })
            .map_err(Into::into)
    };

    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["up"] => up(&db_path()?),
        ["status"] => status(&db_path()?),
        ["new", name] => new(name),
        ["reset"] => reset(&db_path()?),
        _ => Err(USAGE.into()),
    }
}

fn up(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = db::connect(path)?;
    let applied = migrate::migrate(&mut conn, migrate::MIGRATIONS)?;
    println!("database: {}", path.display());
    if applied.is_empty() {
        println!(
            "already up to date (version {})",
            migrate::current_version(&conn)?
        );
    }
    for migration in applied {
        println!("applied {migration}");
    }
    Ok(())
}

fn status(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("database: {}", path.display());
    if !path.exists() {
        println!(
            "not created yet; all {} migrations will run on open",
            migrate::MIGRATIONS.len()
        );
        return Ok(());
    }
    let conn = db::connect(path)?;
    println!(
        "version: {} / {}",
        migrate::current_version(&conn)?,
        migrate::MIGRATIONS.len()
    );
    for migration in migrate::pending(&conn, migrate::MIGRATIONS)? {
        println!("pending {migration}");
    }
    Ok(())
}

fn new(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let existing: Vec<String> = fs::read_dir(&dir)?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<_>>()?;
    let file_name = migrate::next_file_name(&existing, name)?;
    let path = dir.join(&file_name);
    fs::write(&path, format!("-- {file_name}\n"))?;
    println!("created {}", path.display());
    Ok(())
}

fn reset(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    db::remove(path)?;
    println!("removed {}", path.display());
    up(path)
}
