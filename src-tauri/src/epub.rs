//! EPUB の生成（ADR 0021）。epub-builder を子プロセスとして起動する。
//! インターフェースが決まるまでは、実行ファイルの場所を決めるところまでを持つ（ADR 0024）。

use std::path::PathBuf;

use crate::settings::Settings;

/// PATH などから探す epub-builder の実行ファイルの名前
pub const EPUB_BUILDER_NAME: &str = "epub-builder";

/// 設定で指定した epub-builder か、指定がなければ自動で探したもの
pub fn epub_builder(settings: &Settings) -> Option<PathBuf> {
    match &settings.epub_builder_path {
        Some(path) => Some(PathBuf::from(path)),
        None => find(),
    }
}

/// PATH から、それでもなければよく使う場所から epub-builder を探す
pub fn find() -> Option<PathBuf> {
    crate::fetch::crawler::find_executable(EPUB_BUILDER_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_configured_path_over_auto_detection() {
        let settings = Settings {
            epub_builder_path: Some("/opt/example/epub-builder".into()),
            ..Settings::default()
        };
        assert_eq!(
            epub_builder(&settings),
            Some(PathBuf::from("/opt/example/epub-builder"))
        );
        assert_eq!(epub_builder(&Settings::default()), find());
    }
}
