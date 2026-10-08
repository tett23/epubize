//! EPUB と zip の書き出しと、Kindle への送信（ADR 0029）。
//!
//! データベースの作品や話から epub-builder のプロジェクトのディレクトリ（`book.toml`、`body/`、`assets/`）を作り、
//! epub-builder を子プロセスとして起動して EPUB 3.0 を作る（ADR 0021、ADR 0026）。
//! Kindle へは、作った EPUB を send-to-kindle に渡して送る（ADR 0027、ADR 0028）。

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;

fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

fn io(e: std::io::Error) -> String {
    e.to_string()
}

/// 書き出す範囲。作品なら本文を取得済みの話の全て、話ならその話だけ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum Scope {
    Novel(i64),
    Episode(i64),
}

/// EPUB にする本。データベースから読む
#[derive(Debug, Clone, PartialEq)]
pub struct Book {
    pub identifier: String,
    pub title: String,
    pub author: String,
    pub description: String,
    /// 縦書きか。整形の設定の組方向（ADR 0011）から決める
    pub vertical: bool,
    /// 段落の間の余白をなくすか。整形の設定の removeEmptyLine（ADR 0030）
    pub remove_empty_line: bool,
    pub episodes: Vec<Episode>,
    /// 挿絵の URL から、取得済みの画像へ
    pub images: HashMap<String, Image>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Episode {
    pub id: i64,
    pub title: String,
    /// 章の名前。続く話で同じ名前なら同じ章にまとめる
    pub chapter: Option<String>,
    pub preface: Option<String>,
    pub body: Option<String>,
    pub afterword: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub content_type: String,
    pub data: Vec<u8>,
}

/// 書き出す本を読む。本文を取得した話がなければ誤りとする
pub fn load(conn: &Connection, scope: Scope) -> Result<Book, String> {
    let (novel_id, episode) = match scope {
        Scope::Novel(id) => (id, None),
        Scope::Episode(id) => {
            let (novel_id, no) = conn
                .query_row(
                    "SELECT novel_id, no FROM episodes WHERE id = ?1",
                    [id],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()
                .map_err(sql)?
                .ok_or_else(|| format!("no episode with id {id}"))?;
            (novel_id, Some((id, no)))
        }
    };
    let (site, site_id, novel_title, author, description, options) = conn
        .query_row(
            "SELECT site, site_id, title, author_name, description, normalize_options
             FROM novels WHERE id = ?1",
            [novel_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()
        .map_err(sql)?
        .ok_or_else(|| format!("no novel with id {novel_id}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, title, chapter, preface, body, afterword FROM episodes
             WHERE novel_id = ?1 AND body_fetched_at IS NOT NULL AND (?2 IS NULL OR id = ?2)
             ORDER BY no",
        )
        .map_err(sql)?;
    let mut episodes: Vec<Episode> = stmt
        .query_map(params![novel_id, episode.map(|(id, _)| id)], |row| {
            Ok(Episode {
                id: row.get(0)?,
                title: row.get(1)?,
                chapter: row.get(2)?,
                preface: row.get(3)?,
                body: row.get(4)?,
                afterword: row.get(5)?,
            })
        })
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)?;
    if episodes.is_empty() {
        return Err("本文を取得した話がありません".into());
    }

    let (identifier, title) = match episode {
        None => (format!("epubize:{site}:{site_id}"), novel_title),
        Some((_, no)) => {
            // 一話だけの本では章にまとめない
            episodes[0].chapter = None;
            (
                format!("epubize:{site}:{site_id}:{no}"),
                format!("{novel_title} {}", episodes[0].title),
            )
        }
    };

    let mut images = HashMap::new();
    let mut stmt = conn
        .prepare(
            "SELECT i.url, i.content_type, i.data FROM episode_images ei
             JOIN images i ON i.url = ei.url
             WHERE ei.episode_id = ?1",
        )
        .map_err(sql)?;
    for episode in &episodes {
        let rows = stmt
            .query_map([episode.id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Image {
                        content_type: row.get(1)?,
                        data: row.get(2)?,
                    },
                ))
            })
            .map_err(sql)?;
        for row in rows {
            let (url, image) = row.map_err(sql)?;
            images.insert(url, image);
        }
    }

    Ok(Book {
        identifier,
        title,
        author,
        description,
        vertical: is_vertical(options.as_deref()),
        remove_empty_line: removes_empty_line(options.as_deref()),
        episodes,
        images,
    })
}

/// 整形の設定の項目。設定がない、または読めなければ None
fn option(options: Option<&str>, key: &str) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(options?).ok()?;
    value.get(key).cloned()
}

/// 整形の設定の組方向。設定がない、または読めなければ縦書き（既定）
fn is_vertical(options: Option<&str>) -> bool {
    option(options, "direction")
        .and_then(|d| d.as_str().map(|d| d != "horizontal"))
        .unwrap_or(true)
}

/// 整形の設定の removeEmptyLine。設定がない、または読めなければ有効（既定）
fn removes_empty_line(options: Option<&str>) -> bool {
    option(options, "removeEmptyLine")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// epub-builder が扱える画像の拡張子（epub-builder の仕様の 5 節）
fn image_extension(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

static IMAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"!\[([^\]]*)\]\(([^()\s]+)\)").expect("valid regex"));

/// epub-builder のプロジェクトのディレクトリを `dir` に作る
pub fn write_project(dir: &Path, book: &Book) -> Result<(), String> {
    fs::create_dir_all(dir.join("body")).map_err(io)?;
    fs::create_dir_all(dir.join("assets/images")).map_err(io)?;
    fs::write(dir.join("book.toml"), book_toml(book)).map_err(io)?;
    fs::write(
        dir.join("assets/style.css"),
        stylesheet(book.vertical, book.remove_empty_line),
    )
    .map_err(io)?;

    // 挿絵は、話の中で最初に現れた順に番号を振って assets/images に置く
    let mut names: HashMap<&str, String> = HashMap::new();
    for episode in &book.episodes {
        for part in parts(episode) {
            for caps in IMAGE.captures_iter(part) {
                let url = caps.get(2).expect("group").as_str();
                if names.contains_key(url) {
                    continue;
                }
                let Some((image, ext)) = book
                    .images
                    .get(url)
                    .and_then(|image| Some((image, image_extension(&image.content_type)?)))
                else {
                    continue;
                };
                let name = format!("{:04}.{ext}", names.len() + 1);
                fs::write(dir.join("assets/images").join(&name), &image.data).map_err(io)?;
                names.insert(url, name);
            }
        }
    }

    let groups = groups(&book.episodes);
    let outer = width(groups.len());
    for (index, group) in groups.iter().enumerate() {
        match group {
            Group::Single(episode) => {
                let path = dir.join("body").join(format!("{:0outer$}.md", index + 1));
                fs::write(path, episode_markdown(episode, &names, "../")).map_err(io)?;
            }
            Group::Chapter(name, episodes) => {
                let chapter =
                    dir.join("body")
                        .join(format!("{:0outer$}-{}", index + 1, file_name(name)));
                fs::create_dir_all(&chapter).map_err(io)?;
                let inner = width(episodes.len());
                for (i, episode) in episodes.iter().enumerate() {
                    let path = chapter.join(format!("{:0inner$}.md", i + 1));
                    fs::write(path, episode_markdown(episode, &names, "../../")).map_err(io)?;
                }
            }
        }
    }
    Ok(())
}

enum Group<'a> {
    Single(&'a Episode),
    Chapter(&'a str, Vec<&'a Episode>),
}

/// 続く話で同じ章の名前を持つものをまとめる。章のない話は一つずつ置く
fn groups(episodes: &[Episode]) -> Vec<Group<'_>> {
    let mut groups: Vec<Group> = Vec::new();
    for episode in episodes {
        match (episode.chapter.as_deref(), groups.last_mut()) {
            (Some(name), Some(Group::Chapter(last, members))) if *last == name => {
                members.push(episode);
            }
            (Some(name), _) => groups.push(Group::Chapter(name, vec![episode])),
            (None, _) => groups.push(Group::Single(episode)),
        }
    }
    groups
}

/// 連番の桁。epub-builder は名前を文字で比べるため、同じディレクトリの中で桁をそろえる
fn width(count: usize) -> usize {
    count.to_string().len().max(4)
}

fn parts(episode: &Episode) -> impl Iterator<Item = &str> {
    [&episode.preface, &episode.body, &episode.afterword]
        .into_iter()
        .filter_map(|part| part.as_deref())
}

/// 話の題名、前書き、本文、後書きを一つの Markdown にする。画面の表示（`src/lib/markdown.ts`）と同じく、
/// 前書きと後書きは区切り線で分ける。`root` は、この文書からプロジェクトの直下への相対パス
fn episode_markdown(episode: &Episode, images: &HashMap<&str, String>, root: &str) -> String {
    let sections: Vec<String> = parts(episode)
        .map(|part| {
            let text = escape_body(part);
            IMAGE
                .replace_all(&text, |caps: &regex::Captures| {
                    let alt = &caps[1];
                    match images.get(&caps[2]) {
                        Some(name) => format!("![{alt}]({root}assets/images/{name})"),
                        // 取得していない挿絵と扱えない形式の挿絵は、外部を指せないため代替テキストにする
                        None => alt.to_owned(),
                    }
                })
                .into_owned()
        })
        .collect();
    format!(
        "# {}\n\n{}\n",
        escape_title(&episode.title),
        sections.join("\n\n---\n\n")
    )
}

/// 本文で epub-builder の記法と取られる文字を、書いたとおりの文字にする。
/// ルビはクローラーが `<ruby>` にしている（ADR 0009）ため、残った `《` は文字として扱う。
/// `[^` は脚注の参照と取られるため、文字にする
fn escape_body(text: &str) -> String {
    text.replace('《', "\\《").replace("[^", "\\[^")
}

/// 題名の ASCII の記号と `《` を、Markdown の記法と取られないようにする
fn escape_title(title: &str) -> String {
    let mut escaped = String::with_capacity(title.len());
    for c in title.chars() {
        if c.is_ascii_punctuation() || c == '《' {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

fn book_toml(book: &Book) -> String {
    let mut toml = String::new();
    let _ = writeln!(toml, "identifier = {}", toml_string(&book.identifier));
    let _ = writeln!(toml, "title = {}", toml_string(&book.title));
    toml.push_str("language = \"ja\"\n");
    if !book.author.trim().is_empty() {
        let _ = writeln!(toml, "authors = [{}]", toml_string(&book.author));
    }
    if !book.description.trim().is_empty() {
        let _ = writeln!(toml, "description = {}", toml_string(&book.description));
    }
    let direction = if book.vertical { "rtl" } else { "ltr" };
    let _ = writeln!(toml, "page_progression_direction = \"{direction}\"");
    toml
}

/// TOML の基本文字列
fn toml_string(value: &str) -> String {
    let mut quoted = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(quoted, "\\u{:04X}", c as u32);
            }
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// 本文は 1 行が 1 段落になっている（ADR 0009）。removeEmptyLine が有効なら段落の余白をなくし、
/// 行の間が空かないようにする。無効なら余白を指定せず、リーダーの既定の余白で段落の間を空ける。
/// 作者の入れた空行（`<br>` だけの段落）は、どちらでも 1 行分残る
fn stylesheet(vertical: bool, remove_empty_line: bool) -> String {
    let mut css = String::new();
    if vertical {
        css.push_str(
            "html {\n  writing-mode: vertical-rl;\n  -webkit-writing-mode: vertical-rl;\n  -epub-writing-mode: vertical-rl;\n}\n",
        );
    }
    if remove_empty_line {
        css.push_str("p {\n  margin: 0;\n}\n");
    }
    css.push_str("img {\n  max-width: 100%;\n  max-height: 100%;\n}\n");
    css
}

/// ファイルやディレクトリの名前に使えるようにする。区切りの文字は全角に、制御文字は除き、長すぎれば切る
pub fn file_name(name: &str) -> String {
    let mut cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| match c {
            '/' => '／',
            ':' => '：',
            c => c,
        })
        .collect::<String>()
        .trim()
        .trim_start_matches('.')
        .to_owned();
    // macOS の名前の上限（255 バイト）に、連番と拡張子の余地を残す
    while cleaned.len() > 200 {
        cleaned.pop();
    }
    if cleaned.is_empty() {
        "epubize".into()
    } else {
        cleaned
    }
}

/// `dir` の中で、まだない `<stem>.<ext>`。あれば `<stem> (2).<ext>` のように番号を付ける
pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}).{ext}")))
        .find(|path| !path.exists())
        .expect("some name is free")
}

/// `dir` の中身を、`dir` の名前のディレクトリに入れた zip にする
pub fn zip_dir(dir: &Path, output: &Path) -> Result<(), String> {
    let root = dir
        .file_name()
        .ok_or("cannot zip a directory without a name")?
        .to_string_lossy()
        .into_owned();
    let mut zip = zip::ZipWriter::new(fs::File::create(output).map_err(io)?);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut files = Vec::new();
    collect_files(dir, &mut files).map_err(io)?;
    files.sort();
    for file in files {
        let relative = file.strip_prefix(dir).expect("under dir");
        let name = Path::new(&root).join(relative);
        let name = name
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(&fs::read(&file).map_err(io)?).map_err(io)?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

/// 子プロセスの誤りを、標準エラー出力の内容とあわせて文にする
fn failure(name: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        format!("{name} が失敗しました（{}）", output.status)
    } else {
        format!("{name} が失敗しました（{}）: {stderr}", output.status)
    }
}

/// epub-builder で `project` から EPUB 3.0 を作り、`output` に書く（ADR 0026）
pub async fn build_epub(program: &Path, project: &Path, output: &Path) -> Result<(), String> {
    let result = tokio::process::Command::new(program)
        .arg("build")
        .arg(project)
        .args(["--epub-version", "3.0", "--quiet", "--output"])
        .arg(output)
        .env("PATH", crate::fetch::crawler::child_path())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("epub-builder を起動できません: {e}"))?;
    if !result.status.success() {
        return Err(failure("epub-builder", &result));
    }
    if !output.is_file() {
        return Err(format!(
            "epub-builder が EPUB を書きませんでした: {}",
            output.display()
        ));
    }
    Ok(())
}

/// send-to-kindle で `file` を Kindle に送る。`env_file` があれば `-e` で渡す（ADR 0028）
pub async fn send_to_kindle(
    program: &Path,
    env_file: Option<&Path>,
    file: &Path,
) -> Result<(), String> {
    let mut command = tokio::process::Command::new(program);
    if let Some(env_file) = env_file {
        command.arg("--env-file").arg(env_file);
    }
    let result = command
        .arg(file)
        .env("PATH", crate::fetch::crawler::child_path())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("send-to-kindle を起動できません: {e}"))?;
    if !result.status.success() {
        return Err(failure("send-to-kindle", &result));
    }
    Ok(())
}

/// Kindle に送った話に、送った日時を記録する
pub fn mark_sent(conn: &mut Connection, episode_ids: &[i64]) -> Result<(), String> {
    let tx = conn.transaction().map_err(sql)?;
    for id in episode_ids {
        tx.execute(
            "UPDATE episodes SET sent_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            [id],
        )
        .map_err(sql)?;
    }
    tx.commit().map_err(sql)
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
