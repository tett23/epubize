//! クローラーの出力（ADR 0005）を読む。各行を JSON Schema で検証してから、型のあるレコードにする。

use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::Value;

const SCHEMA: &str = include_str!("../../../schema/crawler-output.v1.schema.json");

fn validator() -> &'static jsonschema::Validator {
    static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let schema: Value = serde_json::from_str(SCHEMA).expect("the schema is valid JSON");
        jsonschema::options()
            .should_validate_formats(true)
            .build(&schema)
            .expect("the schema is a valid JSON Schema")
    })
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Author {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelRecord {
    pub url: String,
    pub title: String,
    pub author: Author,
    pub description: String,
    pub episode_count: i64,
    #[serde(default)]
    pub is_concluded: Option<bool>,
    pub fetched_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TocEntryRecord {
    pub no: i64,
    pub url: String,
    pub title: String,
    pub chapter: Option<String>,
    pub published_at: Option<String>,
    pub revised_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ImageRef {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeRecord {
    pub url: String,
    pub body: String,
    pub preface: Option<String>,
    pub afterword: Option<String>,
    pub images: Vec<ImageRef>,
    pub char_count: i64,
    pub fetched_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRecord {
    pub url: String,
    pub content_type: String,
    /// base64
    pub data: String,
    pub byte_length: i64,
    pub fetched_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Record {
    Novel(NovelRecord),
    TocEntry(TocEntryRecord),
    Episode(EpisodeRecord),
    Image(ImageRecord),
    Error(Value),
}

/// 標準出力の JSON Lines を読む。空行は飛ばす。
/// JSON Schema に合わない行が 1 つでもあれば、どの行かを示してエラーにする
pub fn parse(stdout: &str) -> Result<Vec<Record>, String> {
    stdout
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            let value: Value =
                serde_json::from_str(line).map_err(|e| format!("line {}: not JSON: {e}", i + 1))?;
            if let Some(error) = validator().iter_errors(&value).next() {
                return Err(format!(
                    "line {}: does not match the schema at {}: {error}",
                    i + 1,
                    error.instance_path()
                ));
            }
            serde_json::from_value(value).map_err(|e| format!("line {}: {e}", i + 1))
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const NOVEL: &str = r#"{"v":1,"type":"novel","site":"narou","novelId":"n0001aa","url":"https://ncode.syosetu.com/n0001aa/","title":"合成データの作品","author":{"name":"見本 太郎","url":null},"description":"あらすじ","episodeCount":2,"isConcluded":false,"fetchedAt":"2026-01-01T09:00:00+09:00","unknownField":1}"#;

    pub(crate) fn toc_entry(no: i64, revised_at: Option<&str>) -> String {
        serde_json::json!({
            "v": 1,
            "type": "toc_entry",
            "novelUrl": "https://ncode.syosetu.com/n0001aa/",
            "no": no,
            "url": format!("https://ncode.syosetu.com/n0001aa/{no}/"),
            "title": format!("第{no}話"),
            "chapter": "第一章",
            "publishedAt": "2026-01-01T09:00:00+09:00",
            "revisedAt": revised_at,
        })
        .to_string()
    }

    pub(crate) fn episode(no: i64, images: &[&str]) -> String {
        serde_json::json!({
            "v": 1,
            "type": "episode",
            "url": format!("https://ncode.syosetu.com/n0001aa/{no}/"),
            "title": format!("第{no}話"),
            "body": "　本文。",
            "preface": null,
            "afterword": "後書き",
            "images": images.iter().map(|url| serde_json::json!({"url": url, "alt": ""})).collect::<Vec<_>>(),
            "charCount": 3,
            "fetchedAt": "2026-01-02T00:00:00Z",
        })
        .to_string()
    }

    pub(crate) fn image(url: &str, bytes: &[u8]) -> String {
        use base64::Engine;
        serde_json::json!({
            "v": 1,
            "type": "image",
            "url": url,
            "contentType": "image/png",
            "data": base64::engine::general_purpose::STANDARD.encode(bytes),
            "byteLength": bytes.len(),
            "fetchedAt": "2026-01-02T00:00:00Z",
        })
        .to_string()
    }

    #[test]
    fn parses_each_record_type_and_ignores_unknown_fields() {
        let stdout = [
            NOVEL.to_owned(),
            toc_entry(1, None),
            episode(1, &["https://example.com/a.png"]),
            image("https://example.com/a.png", b"png"),
            String::new(),
        ]
        .join("\n");
        let records = parse(&stdout).unwrap();
        assert_eq!(records.len(), 4);
        assert!(
            matches!(&records[0], Record::Novel(n) if n.is_concluded == Some(false) && n.author.url.is_none())
        );
        assert!(matches!(&records[1], Record::TocEntry(t) if t.no == 1 && t.revised_at.is_none()));
        assert!(
            matches!(&records[2], Record::Episode(e) if e.images[0].url == "https://example.com/a.png")
        );
        assert!(matches!(&records[3], Record::Image(i) if i.byte_length == 3));
    }

    #[test]
    fn accepts_novel_without_is_concluded() {
        let mut value: Value = serde_json::from_str(NOVEL).unwrap();
        value.as_object_mut().unwrap().remove("isConcluded");
        let records = parse(&value.to_string()).unwrap();
        assert!(matches!(&records[0], Record::Novel(n) if n.is_concluded.is_none()));
    }

    #[test]
    fn rejects_records_that_do_not_match_the_schema() {
        let mut missing_title: Value = serde_json::from_str(NOVEL).unwrap();
        missing_title.as_object_mut().unwrap().remove("title");
        let mut bad_date: Value = serde_json::from_str(NOVEL).unwrap();
        bad_date["fetchedAt"] = "yesterday".into();
        let mut bad_version: Value = serde_json::from_str(NOVEL).unwrap();
        bad_version["v"] = 2.into();

        for line in [
            missing_title.to_string(),
            bad_date.to_string(),
            bad_version.to_string(),
            r#"{"v":1,"type":"unknown"}"#.to_owned(),
            "not json".to_owned(),
        ] {
            let stdout = format!("{}\n{line}", toc_entry(1, None));
            let error = parse(&stdout).unwrap_err();
            assert!(error.starts_with("line 2:"), "{error}");
        }
    }
}
