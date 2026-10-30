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

/// 預設語系，也是退回對象（ADR 0005）
pub const DEFAULT_LANG: &str = "zh-TW";

impl Lang {
    pub fn default_lang() -> Self {
        Self(DEFAULT_LANG.to_owned())
    }

    pub fn is_default(&self) -> bool {
        self.0 == DEFAULT_LANG
    }
}

/// 多語言文字的共用介面：每種資料的語言列（如程式名稱、表說明）都實作它
pub trait LangText {
    fn lang(&self) -> &Lang;
}

impl<T: LangText> LangText for &T {
    fn lang(&self) -> &Lang {
        (**self).lang()
    }
}

/// 取某語系的顯示文字，退回規則（ADR 0005、L2）：指定語系 → zh-TW → 無。
/// `field` 回傳 None 代表該語系沒有這個欄位（空字串請先轉成 None，可用 [`non_empty`]）。
pub fn localized<T: LangText, V>(
    texts: &[T],
    lang: &Lang,
    field: impl Fn(&T) -> Option<V>,
) -> Option<V> {
    let get = |l: &str| {
        texts
            .iter()
            .find(|t| t.lang().as_str() == l)
            .and_then(&field)
    };
    get(lang.as_str()).or_else(|| (!lang.is_default()).then(|| get(DEFAULT_LANG)).flatten())
}

/// 輸入的文字去頭尾空白，空白視為未填
pub fn normalize_text(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// 空字串視為無
pub fn non_empty(s: &str) -> Option<String> {
    Some(s.to_owned()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Text(Lang, &'static str);

    impl LangText for Text {
        fn lang(&self) -> &Lang {
            &self.0
        }
    }

    fn texts() -> Vec<Text> {
        vec![
            Text(Lang::parse("zh-TW").unwrap(), "料件"),
            Text(Lang::parse("en").unwrap(), ""),
        ]
    }

    #[test]
    fn falls_back_to_default_language() {
        let t = texts();
        let pick = |l: &str| localized(&t, &Lang::parse(l).unwrap(), |x| non_empty(x.1));
        assert_eq!(pick("zh-TW").as_deref(), Some("料件"));
        assert_eq!(pick("en").as_deref(), Some("料件"), "英文空白時退回繁中");
        assert_eq!(pick("zh-CN").as_deref(), Some("料件"), "沒有簡中時退回繁中");
    }

    #[test]
    fn nothing_when_default_missing() {
        let t = vec![Text(Lang::parse("en").unwrap(), "Item")];
        assert_eq!(
            localized(&t, &Lang::parse("zh-CN").unwrap(), |x| non_empty(x.1)),
            None
        );
    }

    #[test]
    fn normalizes_blank_input() {
        assert_eq!(normalize_text(Some("  ".into())), None);
        assert_eq!(normalize_text(Some(" a ".into())).as_deref(), Some("a"));
    }
}
