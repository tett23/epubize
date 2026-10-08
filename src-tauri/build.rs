use std::fmt::Write as _;
use std::path::Path;
use std::{env, fs};

fn main() {
    embed_migrations();
    tauri_build::build()
}

/// `migrations/NNNN_name.sql` を版の順に読み、埋め込む一覧を `$OUT_DIR/migrations.rs` に書き出す。
/// 名前の形が違うものや、1 から連番になっていないものがあればビルドを止める（ADR 0013）。
fn embed_migrations() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut entries: Vec<(u32, String, String)> = fs::read_dir(&dir)
        .expect("migrations/ must exist")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .map(|path| {
            let stem = path.file_stem().unwrap().to_str().unwrap().to_owned();
            let (version, name) = stem
                .split_once('_')
                .filter(|(v, n)| {
                    v.len() == 4 && v.bytes().all(|b| b.is_ascii_digit()) && !n.is_empty()
                })
                .unwrap_or_else(|| {
                    panic!(
                        "migration file must be named NNNN_name.sql: {}",
                        path.display()
                    )
                });
            (
                version.parse().unwrap(),
                name.to_owned(),
                path.canonicalize().unwrap().to_str().unwrap().to_owned(),
            )
        })
        .collect();
    entries.sort();

    for (i, (version, name, _)) in entries.iter().enumerate() {
        let expected = i as u32 + 1;
        assert!(
            *version == expected,
            "migration versions must be consecutive from 1: expected {expected:04}, found {version:04}_{name}.sql"
        );
    }

    let mut out = String::from("&[\n");
    for (version, name, path) in &entries {
        writeln!(
            out,
            "    Migration {{ version: {version}, name: {name:?}, sql: include_str!({path:?}) }},"
        )
        .unwrap();
    }
    out.push(']');

    let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join("migrations.rs");
    fs::write(out_path, out).unwrap();
}
