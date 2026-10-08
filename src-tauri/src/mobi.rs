//! 端末に直接入れるための MOBI の生成（ADR 0032）。
//!
//! epub-builder で作った EPUB を kindlegen で MOBI にし、striptool で中に埋め込まれた元の EPUB（SRCS）を取り除く。
//! kindlegen と striptool は Kindle Previewer に入っているものを使い、epubize には同梱しない。

use std::path::{Path, PathBuf};

use crate::settings::Settings;

/// PATH などから探す kindlegen の実行ファイルの名前
pub const KINDLEGEN_NAME: &str = "kindlegen";
/// PATH などから探す striptool の実行ファイルの名前
pub const STRIPTOOL_NAME: &str = "striptool";

/// 設定で指定した kindlegen か、指定がなければ自動で探したもの
pub fn kindlegen(settings: &Settings) -> Option<PathBuf> {
    match &settings.kindlegen_path {
        Some(path) => Some(PathBuf::from(path)),
        None => find_kindlegen(),
    }
}

/// 設定で指定した striptool か、指定がなければ自動で探したもの
pub fn striptool(settings: &Settings) -> Option<PathBuf> {
    match &settings.striptool_path {
        Some(path) => Some(PathBuf::from(path)),
        None => find_striptool(),
    }
}

/// PATH から、それでもなければよく使う場所から kindlegen を探す
pub fn find_kindlegen() -> Option<PathBuf> {
    crate::fetch::crawler::find_executable(KINDLEGEN_NAME)
}

/// PATH から、それでもなければよく使う場所から striptool を探す
pub fn find_striptool() -> Option<PathBuf> {
    crate::fetch::crawler::find_executable(STRIPTOOL_NAME)
}

/// kindlegen の標準出力から、誤りと警告の行を拾う。なければ最後の数行
fn kindlegen_messages(stdout: &str) -> String {
    let lines: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let picked: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| line.contains("エラー") || line.contains("Error"))
        .collect();
    if picked.is_empty() {
        lines[lines.len().saturating_sub(3)..].join("\n")
    } else {
        picked.join("\n")
    }
}

/// `epub` を kindlegen で MOBI にし、striptool で元の EPUB を取り除いた MOBI のパスを返す。
/// `epub` は、ほかのファイルのない作業用のディレクトリに置く。kindlegen と striptool はそこに書く
pub async fn build_mobi(
    kindlegen: &Path,
    striptool: &Path,
    epub: &Path,
) -> Result<PathBuf, String> {
    let dir = epub.parent().ok_or("EPUB の置き場所がありません")?;
    let stem = epub
        .file_stem()
        .ok_or("EPUB の名前がありません")?
        .to_string_lossy()
        .into_owned();
    let name = format!("{stem}.mobi");
    let mobi = dir.join(&name);

    // kindlegen は -o にファイルの名前だけを受け取り、入力と同じディレクトリに書く。
    // 警告があると終了コード 1 で終わるが、MOBI はできている
    let result = tokio::process::Command::new(kindlegen)
        .arg(epub)
        .args(["-locale", "ja", "-o"])
        .arg(&name)
        .current_dir(dir)
        .env("PATH", crate::fetch::crawler::child_path())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("kindlegen を起動できません: {e}"))?;
    let succeeded = matches!(result.status.code(), Some(0 | 1));
    if !succeeded || !mobi.is_file() {
        return Err(format!(
            "kindlegen が失敗しました（{}）: {}",
            result.status,
            kindlegen_messages(&String::from_utf8_lossy(&result.stdout))
        ));
    }

    // striptool は、入力と同じディレクトリに数字の名前のディレクトリを作り、
    // そこに元の EPUB を取り除いた MOBI と、取り出した EPUB を書く
    let result = tokio::process::Command::new(striptool)
        .arg(&mobi)
        .current_dir(dir)
        .env("PATH", crate::fetch::crawler::child_path())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("striptool を起動できません: {e}"))?;
    if !result.status.success() {
        let stdout = String::from_utf8_lossy(&result.stdout);
        let message: Vec<&str> = stdout
            .lines()
            .filter(|line| !line.starts_with('*') && !line.trim().is_empty())
            .collect();
        return Err(format!(
            "striptool が失敗しました（{}）: {}",
            result.status,
            message.join("\n")
        ));
    }
    std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(&name))
        .find(|path| path.is_file())
        .ok_or_else(|| "striptool が元の EPUB を取り除いた MOBI を書きませんでした".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 実行できる sh のスクリプトを作る
    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    /// 引数を記録し、`-o` の名前で入力の隣に書き、警告ありの終了コード 1 で終わる偽の kindlegen
    fn fake_kindlegen(bin: &Path, args: &Path) -> PathBuf {
        script(
            bin,
            "kindlegen",
            &format!(
                "printf '%s\\n' \"$@\" > '{}'\nwhile [ \"$1\" != -o ]; do shift; done\necho mobi-with-source > \"$2\"\nexit 1",
                args.display()
            ),
        )
    }

    /// 本物と同じく、入力の隣の数字のディレクトリに、元の EPUB を取り除いた MOBI を書く偽の striptool
    fn fake_striptool(bin: &Path) -> PathBuf {
        script(
            bin,
            "striptool",
            "dir=$(dirname \"$1\")/4018903265\nmkdir -p \"$dir\"\necho stripped > \"$dir/$(basename \"$1\")\"",
        )
    }

    #[test]
    fn uses_configured_paths_over_auto_detection() {
        let settings = Settings {
            kindlegen_path: Some("/opt/example/kindlegen".into()),
            striptool_path: Some("/opt/example/striptool".into()),
            ..Settings::default()
        };
        assert_eq!(
            kindlegen(&settings),
            Some(PathBuf::from("/opt/example/kindlegen"))
        );
        assert_eq!(
            striptool(&settings),
            Some(PathBuf::from("/opt/example/striptool"))
        );
        assert_eq!(kindlegen(&Settings::default()), find_kindlegen());
        assert_eq!(striptool(&Settings::default()), find_striptool());
    }

    #[tokio::test]
    async fn builds_mobi_and_strips_the_source() {
        let bin = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let args = bin.path().join("args");
        let epub = work.path().join("合成データの作品.epub");
        fs::write(&epub, "epub").unwrap();

        let mobi = build_mobi(
            &fake_kindlegen(bin.path(), &args),
            &fake_striptool(bin.path()),
            &epub,
        )
        .await
        .unwrap();
        assert_eq!(mobi, work.path().join("4018903265/合成データの作品.mobi"));
        assert_eq!(fs::read_to_string(&mobi).unwrap(), "stripped\n");
        assert_eq!(
            fs::read_to_string(&args).unwrap(),
            format!(
                "{}\n-locale\nja\n-o\n合成データの作品.mobi\n",
                epub.display()
            )
        );
    }

    #[tokio::test]
    async fn reports_kindlegen_errors() {
        let bin = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let epub = work.path().join("book.epub");
        fs::write(&epub, "epub").unwrap();
        let striptool = fake_striptool(bin.path());

        let failing = script(
            bin.path(),
            "failing",
            "echo '情報(prcgen):I1047: 内容を読みます'\necho 'エラー(prcgen):E21018: 壊れている'\nexit 2",
        );
        let error = build_mobi(&failing, &striptool, &epub).await.unwrap_err();
        assert!(error.starts_with("kindlegen が失敗しました"), "{error}");
        assert!(
            error.ends_with("エラー(prcgen):E21018: 壊れている"),
            "{error}"
        );

        // 終了コードが 1 でも、MOBI を書かなければ誤りとする
        let silent = script(bin.path(), "silent", "echo 最後の行\nexit 1");
        let error = build_mobi(&silent, &striptool, &epub).await.unwrap_err();
        assert!(error.ends_with("最後の行"), "{error}");

        let error = build_mobi(&bin.path().join("missing"), &striptool, &epub)
            .await
            .unwrap_err();
        assert!(error.starts_with("kindlegen を起動できません"), "{error}");
    }

    #[tokio::test]
    async fn reports_striptool_errors() {
        let bin = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let args = bin.path().join("args");
        let epub = work.path().join("book.epub");
        fs::write(&epub, "epub").unwrap();
        let kindlegen = fake_kindlegen(bin.path(), &args);

        let failing = script(
            bin.path(),
            "failing",
            "echo '****'\necho 'Error: file not found: book.mobi'\nexit 255",
        );
        let error = build_mobi(&kindlegen, &failing, &epub).await.unwrap_err();
        assert!(error.starts_with("striptool が失敗しました"), "{error}");
        assert!(
            error.ends_with("Error: file not found: book.mobi"),
            "{error}"
        );

        // 成功しても、元の EPUB を取り除いた MOBI がなければ誤りとする
        let silent = script(bin.path(), "silent", "exit 0");
        let error = build_mobi(&kindlegen, &silent, &epub).await.unwrap_err();
        assert!(error.contains("MOBI を書きませんでした"), "{error}");
    }

    #[test]
    fn picks_error_lines_from_kindlegen_output() {
        assert_eq!(
            kindlegen_messages("情報: a\nエラー: b\n警告: c\nError: d\n"),
            "エラー: b\nError: d"
        );
        assert_eq!(kindlegen_messages("1\n2\n\n3\n4\n"), "2\n3\n4");
        assert_eq!(kindlegen_messages(""), "");
    }
}
