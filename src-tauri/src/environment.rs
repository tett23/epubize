//! 開発中の版と配布する版でデータを分けるための環境（ADR 0014）。

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

impl Environment {
    /// 環境変数 `EPUBIZE_ENV` があればそれに、なければビルドの種類に従う。
    /// debug ビルドは development、release ビルドは production とする
    pub fn current() -> Result<Self, String> {
        Self::resolve(
            std::env::var("EPUBIZE_ENV").ok().as_deref(),
            cfg!(debug_assertions),
        )
    }

    fn resolve(var: Option<&str>, debug_build: bool) -> Result<Self, String> {
        match var {
            Some("development") => Ok(Self::Development),
            Some("production") => Ok(Self::Production),
            Some(other) => Err(format!(
                "EPUBIZE_ENV must be development or production: {other:?}"
            )),
            None if debug_build => Ok(Self::Development),
            None => Ok(Self::Production),
        }
    }

    /// データを置くディレクトリの名前
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.dir_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_build_kind_without_variable() {
        assert_eq!(
            Environment::resolve(None, true),
            Ok(Environment::Development)
        );
        assert_eq!(
            Environment::resolve(None, false),
            Ok(Environment::Production)
        );
    }

    #[test]
    fn variable_overrides_build_kind() {
        assert_eq!(
            Environment::resolve(Some("production"), true),
            Ok(Environment::Production)
        );
        assert_eq!(
            Environment::resolve(Some("development"), false),
            Ok(Environment::Development)
        );
    }

    #[test]
    fn rejects_unknown_value() {
        assert!(Environment::resolve(Some("staging"), true).is_err());
    }
}
