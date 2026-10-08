use super::*;
use crate::library::records::{parse, tests as fixtures};
use crate::library::store::{save_episode, save_image, save_toc};
use crate::subscriptions::{Site, Subscription};

/// 2 話の目次と、1 話目の本文（挿絵 2 枚のうち 1 枚だけ取得済み）を持つデータベース
fn database() -> (Connection, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    crate::db::migrate::migrate(&mut conn, crate::db::migrate::MIGRATIONS).unwrap();
    let toc = [
        fixtures::NOVEL.to_owned(),
        fixtures::toc_entry(1, None),
        fixtures::toc_entry(2, None),
    ]
    .join("\n");
    let id = save_toc(
        &mut conn,
        &Subscription {
            site: Site::Narou,
            id: "n0001aa".into(),
        },
        &parse(&toc).unwrap(),
    )
    .unwrap();
    save_episode(
        &mut conn,
        &parse(&fixtures::episode(
            1,
            &["https://example.com/a.png", "https://example.com/b.png"],
        ))
        .unwrap(),
    )
    .unwrap();
    save_image(
        &conn,
        &parse(&fixtures::image("https://example.com/a.png", b"png")).unwrap(),
    )
    .unwrap();
    (conn, id)
}

fn episode_id(conn: &Connection, no: i64) -> i64 {
    conn.query_row("SELECT id FROM episodes WHERE no = ?1", [no], |row| {
        row.get(0)
    })
    .unwrap()
}

fn episode(id: i64, title: &str, chapter: Option<&str>, body: &str) -> Episode {
    Episode {
        id,
        no: id,
        url: format!("https://ncode.syosetu.com/n0001aa/{id}/"),
        title: title.into(),
        chapter: chapter.map(Into::into),
        preface: None,
        body: Some(body.into()),
        afterword: None,
    }
}

fn book(episodes: Vec<Episode>) -> Book {
    Book {
        identifier: "epubize:narou:n0001aa".into(),
        title: "合成データの作品".into(),
        author: "見本 太郎".into(),
        description: "あらすじ".into(),
        site: "narou".into(),
        url: "https://ncode.syosetu.com/n0001aa/".into(),
        created_on: "2026-10-09".into(),
        vertical: true,
        remove_empty_line: true,
        episodes,
        images: HashMap::new(),
    }
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).unwrap()
}

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

#[test]
fn loads_fetched_episodes_of_a_novel() {
    let (conn, id) = database();
    let book = load(&conn, Scope::Novel(id)).unwrap();
    assert_eq!(book.identifier, "epubize:narou:n0001aa");
    assert_eq!(book.title, "合成データの作品");
    assert_eq!(book.author, "見本 太郎");
    assert_eq!(book.site, "narou");
    assert_eq!(book.url, "https://ncode.syosetu.com/n0001aa/");
    assert_eq!(book.created_on.len(), "2026-10-09".len());
    assert_eq!(book.episodes[0].no, 1);
    assert!(book.vertical);
    assert!(book.remove_empty_line);
    // 本文のない 2 話目は入れない
    assert_eq!(book.episodes.len(), 1);
    assert_eq!(book.episodes[0].title, "第1話");
    assert_eq!(book.episodes[0].chapter.as_deref(), Some("第一章"));
    assert_eq!(book.episodes[0].afterword.as_deref(), Some("後書き"));
    // 取得済みの挿絵だけを持つ
    assert_eq!(
        book.images.keys().collect::<Vec<_>>(),
        ["https://example.com/a.png"]
    );
    assert_eq!(book.images["https://example.com/a.png"].data, b"png");
}

#[test]
fn loads_one_episode_without_chapter() {
    let (conn, _) = database();
    let book = load(&conn, Scope::Episode(episode_id(&conn, 1))).unwrap();
    assert_eq!(book.identifier, "epubize:narou:n0001aa:1");
    assert_eq!(book.title, "合成データの作品 第1話");
    // 話だけの本は、話の URL を奥付に書く
    assert_eq!(book.url, "https://ncode.syosetu.com/n0001aa/1/");
    assert_eq!(book.episodes.len(), 1);
    assert_eq!(book.episodes[0].chapter, None);
}

#[test]
fn rejects_scope_without_fetched_body() {
    let (conn, id) = database();
    let unfetched = episode_id(&conn, 2);
    assert_eq!(
        load(&conn, Scope::Episode(unfetched)),
        Err("本文を取得した話がありません".into())
    );
    assert!(load(&conn, Scope::Episode(9999)).is_err());
    assert!(load(&conn, Scope::Novel(id + 1)).is_err());
}

#[test]
fn reads_direction_from_normalize_options() {
    let (conn, id) = database();
    conn.execute(
        "UPDATE novels SET normalize_options = '{\"direction\":\"horizontal\"}' WHERE id = ?1",
        [id],
    )
    .unwrap();
    assert!(!load(&conn, Scope::Novel(id)).unwrap().vertical);
    assert!(is_vertical(None));
    assert!(is_vertical(Some("{\"direction\":\"vertical\"}")));
    assert!(is_vertical(Some("not json")));
}

#[test]
fn reads_remove_empty_line_from_normalize_options() {
    let (conn, id) = database();
    conn.execute(
        "UPDATE novels SET normalize_options = '{\"removeEmptyLine\":false}' WHERE id = ?1",
        [id],
    )
    .unwrap();
    assert!(!load(&conn, Scope::Novel(id)).unwrap().remove_empty_line);
    assert!(removes_empty_line(None));
    assert!(removes_empty_line(Some("{\"removeEmptyLine\":true}")));
    // 型の合わない値は既定（画面の resolveNormalizeOptions と同じ）
    assert!(removes_empty_line(Some("{\"removeEmptyLine\":\"no\"}")));
    assert!(removes_empty_line(Some("not json")));
}

#[test]
fn deserializes_scope_from_the_frontend() {
    let scope: Scope = serde_json::from_str(r#"{"kind":"novel","id":3}"#).unwrap();
    assert_eq!(scope, Scope::Novel(3));
    let scope: Scope = serde_json::from_str(r#"{"kind":"episode","id":4}"#).unwrap();
    assert_eq!(scope, Scope::Episode(4));
}

#[test]
fn writes_book_toml_and_stylesheet() {
    let dir = tempfile::tempdir().unwrap();
    let mut book = book(vec![episode(1, "第1話", None, "本文")]);
    book.title = "題名に\"引用符\"と\\と\n改行".into();
    write_project(dir.path(), &book).unwrap();
    assert_eq!(
        read(dir.path().join("book.toml")),
        "identifier = \"epubize:narou:n0001aa\"\n\
         title = \"題名に\\\"引用符\\\"と\\\\と\\n改行\"\n\
         language = \"ja\"\n\
         authors = [\"見本 太郎\"]\n\
         description = \"あらすじ\"\n\
         page_progression_direction = \"rtl\"\n"
    );
    let css = read(dir.path().join("assets/style.css"));
    assert!(css.contains("writing-mode: vertical-rl"));
    assert!(css.contains("p {\n  margin: 0;\n}"));

    // 横書きでは左から右へ送り、縦書きの指定をしない。空の作者とあらすじは書かない
    let dir = tempfile::tempdir().unwrap();
    book.vertical = false;
    book.remove_empty_line = false;
    book.author = " ".into();
    book.description = String::new();
    write_project(dir.path(), &book).unwrap();
    let toml = read(dir.path().join("book.toml"));
    assert!(toml.contains("page_progression_direction = \"ltr\""));
    assert!(!toml.contains("authors"));
    assert!(!toml.contains("description"));
    let css = read(dir.path().join("assets/style.css"));
    assert!(!css.contains("writing-mode"));
    // 段落の余白はリーダーの既定に任せる
    assert!(!css.contains("p {"));
}

#[test]
fn groups_consecutive_episodes_of_the_same_chapter_into_directories() {
    let dir = tempfile::tempdir().unwrap();
    let book = book(vec![
        episode(1, "プロローグ", None, "一"),
        episode(2, "第1話", Some("第一章/始まり"), "二"),
        episode(3, "第2話", Some("第一章/始まり"), "三"),
        episode(4, "第3話", Some("第二章"), "四"),
        episode(5, "幕間", None, "五"),
        // 間に別の章を挟めば、同じ名前でも別の章にする
        episode(6, "第4話", Some("第一章/始まり"), "六"),
    ]);
    write_project(dir.path(), &book).unwrap();
    let body = dir.path().join("body");
    assert_eq!(read(body.join("0001.md")), "# プロローグ\n\n一\n");
    assert_eq!(
        read(body.join("0002-第一章／始まり/0001.md")),
        "# 第1話\n\n二\n"
    );
    assert_eq!(
        read(body.join("0002-第一章／始まり/0002.md")),
        "# 第2話\n\n三\n"
    );
    assert_eq!(read(body.join("0003-第二章/0001.md")), "# 第3話\n\n四\n");
    assert_eq!(read(body.join("0004.md")), "# 幕間\n\n五\n");
    assert_eq!(
        read(body.join("0005-第一章／始まり/0001.md")),
        "# 第4話\n\n六\n"
    );
}

#[test]
fn writes_titlepage_and_colophon() {
    let dir = tempfile::tempdir().unwrap();
    let mut novel = book(vec![
        episode(3, "第3話", None, "三"),
        episode(5, "第5話", None, "五"),
    ]);
    novel.title = "<題名> & \"引用\"".into();
    write_project(dir.path(), &novel).unwrap();
    assert_eq!(
        read(dir.path().join("meta/titlepage.xhtml")),
        "<div class=\"titlepage\">\n\
         <h1>&lt;題名&gt; &amp; &quot;引用&quot;</h1>\n\
         <p class=\"author\">見本 太郎</p>\n\
         </div>\n"
    );
    assert_eq!(
        read(dir.path().join("meta/colophon.xhtml")),
        "<h1>奥付</h1>\n<dl class=\"colophon\">\n\
         <dt>題名</dt>\n<dd>&lt;題名&gt; &amp; &quot;引用&quot;</dd>\n\
         <dt>著者</dt>\n<dd>見本 太郎</dd>\n\
         <dt>掲載</dt>\n<dd>小説家になろう<br/><a href=\"https://ncode.syosetu.com/n0001aa/\">https://ncode.syosetu.com/n0001aa/</a></dd>\n\
         <dt>収録</dt>\n<dd>第3話〜第5話のうち本文を取得した 2 話</dd>\n\
         <dt>作成</dt>\n<dd>2026-10-09 epubize</dd>\n\
         </dl>\n"
    );
    let css = read(dir.path().join("assets/style.css"));
    assert!(css.contains(".titlepage"));
    assert!(css.contains("ol.contents"));
    assert!(css.contains("dl.colophon"));

    // 一話だけの本と、著者のない本
    let dir = tempfile::tempdir().unwrap();
    let mut single = book(vec![episode(7, "第7話", None, "七")]);
    single.author = String::new();
    single.site = "kakuyomu".into();
    write_project(dir.path(), &single).unwrap();
    assert!(!read(dir.path().join("meta/titlepage.xhtml")).contains("author"));
    let colophon = read(dir.path().join("meta/colophon.xhtml"));
    assert!(!colophon.contains("著者"));
    assert!(colophon.contains("<dd>カクヨム<br/>"));
    assert!(colophon.contains("<dd>第7話</dd>"));
}

#[test]
fn puts_a_title_page_in_each_chapter() {
    let dir = tempfile::tempdir().unwrap();
    let novel = book(vec![
        episode(1, "プロローグ", None, "一"),
        episode(2, "第1話", Some("第一章 <出会い>"), "二"),
        episode(3, "第2話", Some("第一章 <出会い>"), "三"),
        episode(4, "第3話", Some("第二章"), "四"),
    ]);
    write_project(dir.path(), &novel).unwrap();
    assert_eq!(
        read(dir.path().join("body/0002-第一章 <出会い>/index.xhtml")),
        "<div class=\"chapter-title\">\n<h1>第一章 &lt;出会い&gt;</h1>\n</div>\n"
    );
    assert!(dir.path().join("body/0003-第二章/index.xhtml").is_file());
    // 章のない話と、話の単位には扉を置かない
    let mut names: Vec<String> = fs::read_dir(dir.path().join("body"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "0000.xhtml",
            "0001.md",
            "0002-第一章 <出会い>",
            "0003-第二章"
        ]
    );
    assert_eq!(
        fs::read_dir(dir.path().join("body/0003-第二章"))
            .unwrap()
            .count(),
        2
    );
}

#[test]
fn puts_contents_before_the_body_of_books_with_episodes() {
    let dir = tempfile::tempdir().unwrap();
    let novel = book(vec![
        episode(1, "プロローグ", None, "一"),
        episode(2, "第1話 <始まり>", Some("第一章 出会い"), "二"),
        episode(3, "第2話", Some("第一章 出会い"), "三"),
    ]);
    write_project(dir.path(), &novel).unwrap();
    // 名前の順で本文の最初に来る
    assert_eq!(
        read(dir.path().join("body/0000.xhtml")),
        "<h1>目次</h1>\n<ol class=\"contents\">\n\
         <li><a href=\"0001.md\">プロローグ</a></li>\n\
         <li><a href=\"0002-%E7%AC%AC%E4%B8%80%E7%AB%A0%20%E5%87%BA%E4%BC%9A%E3%81%84/index.xhtml\">第一章 出会い</a>\n<ol>\n\
         <li><a href=\"0002-%E7%AC%AC%E4%B8%80%E7%AB%A0%20%E5%87%BA%E4%BC%9A%E3%81%84/0001.md\">第1話 &lt;始まり&gt;</a></li>\n\
         <li><a href=\"0002-%E7%AC%AC%E4%B8%80%E7%AB%A0%20%E5%87%BA%E4%BC%9A%E3%81%84/0002.md\">第2話</a></li>\n\
         </ol>\n</li>\n\
         </ol>\n"
    );

    // 一話だけの本には置かない
    let dir = tempfile::tempdir().unwrap();
    write_project(dir.path(), &book(vec![episode(1, "第1話", None, "一")])).unwrap();
    assert!(!dir.path().join("body/0000.xhtml").exists());
}

#[test]
fn widens_numbers_when_there_are_many_entries() {
    let dir = tempfile::tempdir().unwrap();
    let episodes = (1..=10_000)
        .map(|n| episode(n, &format!("第{n}話"), None, "本文"))
        .collect();
    write_project(dir.path(), &book(episodes)).unwrap();
    assert!(dir.path().join("body/00000.xhtml").is_file());
    assert!(dir.path().join("body/00001.md").is_file());
    assert!(dir.path().join("body/10000.md").is_file());
}

#[test]
fn joins_preface_body_and_afterword_and_escapes_notation() {
    let dir = tempfile::tempdir().unwrap();
    let mut first = episode(
        1,
        "第1話 *強調* と [リンク]《序》",
        None,
        "本文《括弧》と[^1]",
    );
    first.preface = Some("前書き".into());
    first.afterword = Some("後書き".into());
    write_project(dir.path(), &book(vec![first])).unwrap();
    assert_eq!(
        read(dir.path().join("body/0001.md")),
        "# 第1話 \\*強調\\* と \\[リンク\\]\\《序》\n\n\
         前書き\n\n---\n\n本文\\《括弧》と\\[^1]\n\n---\n\n後書き\n"
    );
}

#[test]
fn copies_fetched_images_and_drops_the_others() {
    let dir = tempfile::tempdir().unwrap();
    let mut book = book(vec![
        episode(
            1,
            "第1話",
            None,
            "![挿絵A](https://example.com/a.png)\n\n![未取得](https://example.com/missing.png)\n\n![WebP](https://example.com/c.webp)",
        ),
        episode(
            2,
            "第2話",
            Some("第一章"),
            "![挿絵B](https://example.com/b.jpg) ![挿絵A](https://example.com/a.png)",
        ),
    ]);
    let image = |content_type: &str, data: &[u8]| Image {
        content_type: content_type.into(),
        data: data.to_vec(),
    };
    book.images = HashMap::from([
        ("https://example.com/a.png".into(), image("image/png", b"a")),
        (
            "https://example.com/b.jpg".into(),
            image("image/jpeg", b"b"),
        ),
        // epub-builder が扱えない形式
        (
            "https://example.com/c.webp".into(),
            image("image/webp", b"c"),
        ),
    ]);
    write_project(dir.path(), &book).unwrap();

    assert_eq!(
        read(dir.path().join("body/0001.md")),
        "# 第1話\n\n![挿絵A](../assets/images/0001.png)\n\n未取得\n\nWebP\n"
    );
    // 章のディレクトリの中からは、もう一段上を指す。同じ挿絵は同じファイルを使う
    assert_eq!(
        read(dir.path().join("body/0002-第一章/0001.md")),
        "# 第2話\n\n![挿絵B](../../assets/images/0002.jpg) ![挿絵A](../../assets/images/0001.png)\n"
    );
    assert_eq!(
        fs::read(dir.path().join("assets/images/0001.png")).unwrap(),
        b"a"
    );
    assert_eq!(
        fs::read(dir.path().join("assets/images/0002.jpg")).unwrap(),
        b"b"
    );
    assert_eq!(
        fs::read_dir(dir.path().join("assets/images"))
            .unwrap()
            .count(),
        2
    );
}

#[test]
fn makes_file_names_safe() {
    assert_eq!(file_name("a/b:c\u{0}d"), "a／b：cd");
    assert_eq!(file_name("  ..隠し  "), "隠し");
    assert_eq!(file_name(""), "epubize");
    assert_eq!(file_name("\u{7}"), "epubize");
    let long = file_name(&"あ".repeat(100));
    assert!(long.len() <= 200);
    assert!(long.chars().all(|c| c == 'あ'));
}

#[test]
fn numbers_paths_that_already_exist() {
    let dir = tempfile::tempdir().unwrap();
    let first = unique_path(dir.path(), "作品", "epub");
    assert_eq!(first, dir.path().join("作品.epub"));
    fs::write(&first, "").unwrap();
    let second = unique_path(dir.path(), "作品", "epub");
    assert_eq!(second, dir.path().join("作品 (2).epub"));
    fs::write(&second, "").unwrap();
    assert_eq!(
        unique_path(dir.path(), "作品", "epub"),
        dir.path().join("作品 (3).epub")
    );
}

#[test]
fn zips_project_inside_a_directory() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("合成データの作品");
    write_project(
        &project,
        &book(vec![episode(1, "第1話", Some("第一章"), "本文")]),
    )
    .unwrap();
    let output = dir.path().join("book.zip");
    zip_dir(&project, &output).unwrap();

    let mut archive = zip::ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
    let mut names: Vec<String> = archive.file_names().map(String::from).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "合成データの作品/assets/style.css",
            "合成データの作品/body/0001-第一章/0001.md",
            "合成データの作品/body/0001-第一章/index.xhtml",
            "合成データの作品/book.toml",
            "合成データの作品/meta/colophon.xhtml",
            "合成データの作品/meta/titlepage.xhtml",
        ]
    );
    let mut body = String::new();
    std::io::Read::read_to_string(
        &mut archive
            .by_name("合成データの作品/body/0001-第一章/0001.md")
            .unwrap(),
        &mut body,
    )
    .unwrap();
    assert_eq!(body, "# 第1話\n\n本文\n");
}

#[tokio::test]
async fn builds_epub_3_with_epub_builder() {
    let dir = tempfile::tempdir().unwrap();
    let args = dir.path().join("args");
    // 引数を記録し、--output の次の引数に書く偽の epub-builder
    let program = script(
        dir.path(),
        "epub-builder",
        &format!(
            "printf '%s\\n' \"$@\" > '{}'\nwhile [ \"$1\" != --output ]; do shift; done\necho epub > \"$2\"",
            args.display()
        ),
    );
    let project = dir.path().join("project");
    let output = dir.path().join("book.epub");
    build_epub(&program, &project, &output).await.unwrap();
    assert_eq!(read(&output), "epub\n");
    assert_eq!(
        read(&args),
        format!(
            "build\n{}\n--epub-version\n3.0\n--quiet\n--output\n{}\n",
            project.display(),
            output.display()
        )
    );
}

#[tokio::test]
async fn reports_epub_builder_failures() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("book.epub");
    let failing = script(
        dir.path(),
        "failing",
        "echo '誤り: body/0001.md:1:1: 壊れている' >&2\nexit 1",
    );
    let error = build_epub(&failing, dir.path(), &output).await.unwrap_err();
    assert!(error.starts_with("epub-builder が失敗しました"), "{error}");
    assert!(
        error.ends_with("誤り: body/0001.md:1:1: 壊れている"),
        "{error}"
    );

    let silent = script(dir.path(), "silent", "exit 0");
    let error = build_epub(&silent, dir.path(), &output).await.unwrap_err();
    assert!(
        error.starts_with("epub-builder が EPUB を書きませんでした"),
        "{error}"
    );

    let error = build_epub(&dir.path().join("missing"), dir.path(), &output)
        .await
        .unwrap_err();
    assert!(
        error.starts_with("epub-builder を起動できません"),
        "{error}"
    );
}

#[tokio::test]
async fn sends_file_with_env_file_option() {
    let dir = tempfile::tempdir().unwrap();
    let args = dir.path().join("args");
    let program = script(
        dir.path(),
        "send-to-kindle",
        &format!("printf '%s\\n' \"$@\" > '{}'", args.display()),
    );
    let file = dir.path().join("book.epub");
    let env = dir.path().join("kindle.env");
    send_to_kindle(&program, Some(&env), &file).await.unwrap();
    assert_eq!(
        read(&args),
        format!("--env-file\n{}\n{}\n", env.display(), file.display())
    );
    send_to_kindle(&program, None, &file).await.unwrap();
    assert_eq!(read(&args), format!("{}\n", file.display()));

    let failing = script(
        dir.path(),
        "failing",
        "echo 'send-to-kindle: .env に次の設定がありません: SMTP_PASSWORD' >&2\nexit 1",
    );
    let error = send_to_kindle(&failing, None, &file).await.unwrap_err();
    assert!(
        error.starts_with("send-to-kindle が失敗しました"),
        "{error}"
    );
    assert!(error.ends_with("SMTP_PASSWORD"), "{error}");
}

#[tokio::test]
async fn child_processes_find_programs_in_mise_shims() {
    let dir = tempfile::tempdir().unwrap();
    let args = dir.path().join("args");
    let program = script(
        dir.path(),
        "send-to-kindle",
        &format!("printf '%s' \"$PATH\" > '{}'", args.display()),
    );
    send_to_kindle(&program, None, dir.path()).await.unwrap();
    assert!(read(&args).ends_with("/.local/share/mise/shims"));
}

#[test]
fn records_sent_time() {
    let (mut conn, _) = database();
    let first = episode_id(&conn, 1);
    let second = episode_id(&conn, 2);
    mark_sent(&mut conn, &[first]).unwrap();
    let sent = |id: i64| -> Option<String> {
        conn.query_row("SELECT sent_at FROM episodes WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .unwrap()
    };
    assert!(sent(first).is_some());
    assert_eq!(sent(second), None);
}
