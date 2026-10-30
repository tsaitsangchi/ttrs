//! 語系（繁中、英文、簡中）；資料字典與程式維護多語言共用
//! TipTop 對應：gat02、gaq02、gaz02 語言別（0 繁中、1 英文、2 簡中）

use crate::DomainError;

/// 支援的語系（D：繁中、英文、簡中）
pub const LANGS: [&str; 3] = ["zh-TW", "en", "zh-CN"];

/// 語系代碼
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lang(String);

impl Lang {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if LANGS.contains(&v) {
            Ok(Self(v.to_owned()))
        } else {
            Err(DomainError::LanguageNotSupported(v.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
