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
    /// 掲載しているサイト（`narou` など）
    pub site: String,
    /// 元の作品の URL。話だけの本なら話の URL
    pub url: String,
    /// 本を作った日（ローカル時刻、`YYYY-MM-DD`）。奥付に書く
    pub created_on: String,
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
    /// 目次の上での 1 始まりの位置
    pub no: i64,
    pub url: String,
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
    let (site, site_id, novel_title, author, description, options, novel_url) = conn
        .query_row(
            "SELECT site, site_id, title, author_name, description, normalize_options, url
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
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .map_err(sql)?
        .ok_or_else(|| format!("no novel with id {novel_id}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, no, url, title, chapter, preface, body, afterword FROM episodes
             WHERE novel_id = ?1 AND body_fetched_at IS NOT NULL AND (?2 IS NULL OR id = ?2)
             ORDER BY no",
        )
        .map_err(sql)?;
    let mut episodes: Vec<Episode> = stmt
        .query_map(params![novel_id, episode.map(|(id, _)| id)], |row| {
            Ok(Episode {
                id: row.get(0)?,
                no: row.get(1)?,
                url: row.get(2)?,
                title: row.get(3)?,
                chapter: row.get(4)?,
                preface: row.get(5)?,
                body: row.get(6)?,
                afterword: row.get(7)?,
            })
        })
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)?;
    if episodes.is_empty() {
        return Err("本文を取得した話がありません".into());
    }

    let (identifier, title, url) = match episode {
        None => (format!("epubize:{site}:{site_id}"), novel_title, novel_url),
        Some((_, no)) => {
            // 一話だけの本では章にまとめない
            episodes[0].chapter = None;
            (
                format!("epubize:{site}:{site_id}:{no}"),
                format!("{novel_title} {}", episodes[0].title),
                episodes[0].url.clone(),
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
        site,
        url,
        created_on: chrono::Local::now().format("%Y-%m-%d").to_string(),
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

    fs::create_dir_all(dir.join("meta")).map_err(io)?;
    fs::write(dir.join("meta/titlepage.xhtml"), titlepage(book)).map_err(io)?;
    fs::write(dir.join("meta/colophon.xhtml"), colophon(book)).map_err(io)?;

    let groups = groups(&book.episodes);
    let outer = width(groups.len());
    // 二話以上の本には、本文の最初に目次のページを置く。epub-builder は目次のページを作らないため
    if book.episodes.len() > 1 {
        let path = dir.join("body").join(format!("{:0outer$}.xhtml", 0));
        fs::write(path, contents(&groups, outer)).map_err(io)?;
    }
    for (chapter, files) in body_path(&groups, outer) {
        // 章ごとに扉を置く。epub-builder はディレクトリの index を部の扉とし、目次の章の項目の行き先にする
        if let Some((name, chapter_dir)) = chapter {
            let path = dir.join("body").join(chapter_dir);
            fs::create_dir_all(&path).map_err(io)?;
            fs::write(path.join("index.xhtml"), chapter_title(name)).map_err(io)?;
        }
        for (path, episode) in files {
            let path_in_body = dir.join("body").join(&path);
            if let Some(parent) = path_in_body.parent() {
                fs::create_dir_all(parent).map_err(io)?;
            }
            // 章のディレクトリの中からは、もう一段上を指す
            let root = if path.contains('/') { "../../" } else { "../" };
            fs::write(path_in_body, episode_markdown(episode, &names, root)).map_err(io)?;
        }
    }
    Ok(())
}

/// 章（名前とディレクトリ）と、その章の話の本文のファイル（`body/` からの相対パス）。
/// 章のない話は章を持たない
type BodyFiles<'a> = Vec<(Option<(&'a str, String)>, Vec<(String, &'a Episode)>)>;

fn body_path<'a>(groups: &[Group<'a>], outer: usize) -> BodyFiles<'a> {
    groups
        .iter()
        .enumerate()
        .map(|(index, group)| match group {
            Group::Single(episode) => (None, vec![(format!("{:0outer$}.md", index + 1), *episode)]),
            Group::Chapter(name, episodes) => {
                let dir = format!("{:0outer$}-{}", index + 1, file_name(name));
                let inner = width(episodes.len());
                (
                    Some((*name, dir.clone())),
                    episodes
                        .iter()
                        .enumerate()
                        .map(|(i, episode)| (format!("{dir}/{:0inner$}.md", i + 1), *episode))
                        .collect(),
                )
            }
        })
        .collect()
}

/// 章の扉。章の名前を置く
fn chapter_title(name: &str) -> String {
    format!(
        "<div class=\"chapter-title\">\n<h1>{}</h1>\n</div>\n",
        xml_text(name)
    )
}

/// 本の扉。題名と著者を置く
fn titlepage(book: &Book) -> String {
    let mut html = String::from("<div class=\"titlepage\">\n");
    let _ = writeln!(html, "<h1>{}</h1>", xml_text(&book.title));
    if !book.author.trim().is_empty() {
        let _ = writeln!(html, "<p class=\"author\">{}</p>", xml_text(&book.author));
    }
    html.push_str("</div>\n");
    html
}

/// 目次のページ。章は入れ子にし、話は本文の文書を指す。epub-builder がリンクを書き換える
fn contents(groups: &[Group], outer: usize) -> String {
    let mut html = String::from("<h1>目次</h1>\n<ol class=\"contents\">\n");
    let link = |path: &str, episode: &Episode| {
        format!(
            "<li><a href=\"{}\">{}</a></li>\n",
            percent_encode(path),
            xml_text(&episode.title)
        )
    };
    for (chapter, files) in body_path(groups, outer) {
        match chapter {
            None => {
                for (path, episode) in files {
                    html.push_str(&link(&path, episode));
                }
            }
            Some((name, dir)) => {
                let _ = writeln!(
                    html,
                    "<li><a href=\"{}\">{}</a>\n<ol>",
                    percent_encode(&format!("{dir}/index.xhtml")),
                    xml_text(name)
                );
                for (path, episode) in files {
                    html.push_str(&link(&path, episode));
                }
                html.push_str("</ol>\n</li>\n");
            }
        }
    }
    html.push_str("</ol>\n");
    html
}

/// 奥付。題名、著者、掲載しているサイトと URL、収録した話、作った日を置く
fn colophon(book: &Book) -> String {
    let mut html = String::from("<h1>奥付</h1>\n<dl class=\"colophon\">\n");
    let mut row = |term: &str, value: String| {
        let _ = writeln!(html, "<dt>{term}</dt>\n<dd>{value}</dd>");
    };
    row("題名", xml_text(&book.title));
    if !book.author.trim().is_empty() {
        row("著者", xml_text(&book.author));
    }
    row(
        "掲載",
        format!(
            "{}<br/><a href=\"{url}\">{url}</a>",
            xml_text(site_name(&book.site)),
            url = xml_text(&book.url)
        ),
    );
    let first = book.episodes.first().map_or(0, |e| e.no);
    let last = book.episodes.last().map_or(0, |e| e.no);
    let count = book.episodes.len();
    row(
        "収録",
        if count == 1 {
            format!("第{first}話")
        } else {
            format!("第{first}話〜第{last}話のうち本文を取得した {count} 話")
        },
    );
    row("作成", format!("{} epubize", xml_text(&book.created_on)));
    html.push_str("</dl>\n");
    html
}

/// 掲載しているサイトの名前
fn site_name(site: &str) -> &str {
    match site {
        "narou" => "小説家になろう",
        "novel18" => "小説家になろう（R18）",
        "hameln" => "ハーメルン",
        "kakuyomu" => "カクヨム",
        other => other,
    }
}

/// XHTML の文字と属性の値に書けるようにする
fn xml_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .fold(String::with_capacity(text.len()), |mut s, c| {
            match c {
                '&' => s.push_str("&amp;"),
                '<' => s.push_str("&lt;"),
                '>' => s.push_str("&gt;"),
                '"' => s.push_str("&quot;"),
                c => s.push(c),
            }
            s
        })
}

/// リンクのパスを、ASCII の英数字と `-._~/` のほかをパーセントエンコードした形にする
fn percent_encode(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            encoded.push(byte as char);
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
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
/// 扉、目次、奥付のスタイル。縦書きと横書きの両方で使えるよう、論理プロパティで書く
const FRONT_AND_BACK_MATTER: &str = "\
.titlepage {
  margin-block-start: 3em;
}
.chapter-title {
  margin-block-start: 3em;
}
.titlepage .author {
  margin-block-start: 2em;
  text-align: end;
}
ol.contents, ol.contents ol {
  list-style: none;
  padding: 0;
  margin: 0;
}
ol.contents ol {
  padding-inline-start: 1em;
}
dl.colophon dt {
  font-weight: bold;
}
dl.colophon dd {
  margin: 0 0 0.5em 0;
}
";

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
    css.push_str(FRONT_AND_BACK_MATTER);
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
