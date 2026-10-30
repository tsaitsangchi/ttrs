//! MultilingualField 多語言欄位（ADR 0005）：同一筆資料的各語系文字並列編輯、一次儲存。
//!
//! 每個語系一列（繁中 zh-TW 為預設語系，放第一列），每列放 `fields` 指定的欄位
//! （如程式名稱；或表名稱＋目的＋備註）。非預設語系空白時，以繁中內容作為提示，
//! 表示顯示時會退回繁中（共用退回規則：指定語系 → zh-TW → 空白）。
//! 各作業只負責把 DTO 轉成 [`LangValues`]、儲存時再轉回去。
//!
//! 版面較擠的欄位用 [`MultilingualDialogField`]：表單上只顯示介面語系的值，
//! 旁邊的翻譯圖示開啟視窗，在視窗內並列編輯各語系。

use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::components::material::{
    Button, ButtonVariant, Dialog, IconButton, TextArea, TextField, icons,
};
use crate::i18n::{UiLang, current, t, tf};

/// 一個多語言欄位的定義
#[derive(Clone, PartialEq)]
pub struct LangField {
    /// 欄位鍵值（對應 DTO 欄位，如 `name`、`purpose`）
    pub key: &'static str,
    pub label: String,
    /// 長度上限（字元），依 TipTop .sch
    pub maxlength: Option<usize>,
    /// 多行輸入的列數；None 為單行
    pub rows: Option<i64>,
}

impl LangField {
    pub fn text(key: &'static str, label: String, maxlength: usize) -> Self {
        Self {
            key,
            label,
            maxlength: Some(maxlength),
            rows: None,
        }
    }

    pub fn area(key: &'static str, label: String, maxlength: usize, rows: i64) -> Self {
        Self {
            rows: Some(rows),
            ..Self::text(key, label, maxlength)
        }
    }
}

/// 欄位鍵值與取值函式，供 [`LangValues::from_rows`] 使用
pub type FieldGetter<T> = (&'static str, fn(&T) -> Option<String>);

/// 各語系各欄位的值；空字串視為未填
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LangValues(BTreeMap<(&'static str, &'static str), String>);

impl LangValues {
    pub fn get(&self, lang: UiLang, key: &'static str) -> String {
        self.0.get(&(lang.code(), key)).cloned().unwrap_or_default()
    }

    /// 去頭尾空白；空白為 None
    pub fn opt(&self, lang: UiLang, key: &'static str) -> Option<String> {
        Some(self.get(lang, key).trim().to_owned()).filter(|s| !s.is_empty())
    }

    pub fn set(&mut self, lang: UiLang, key: &'static str, value: impl Into<String>) {
        self.0.insert((lang.code(), key), value.into());
    }

    /// 由 DTO 清單建立：`lang_of` 取語系代碼，`fields` 取各欄位值；不支援的語系略過
    pub fn from_rows<T>(
        rows: &[T],
        lang_of: impl Fn(&T) -> &str,
        fields: &[FieldGetter<T>],
    ) -> Self {
        let mut v = Self::default();
        for row in rows {
            let Some(lang) = UiLang::from_code(lang_of(row)) else {
                continue;
            };
            for (key, get) in fields {
                v.set(lang, key, get(row).unwrap_or_default());
            }
        }
        v
    }

    /// 該語系是否有任何欄位有值
    pub fn has(&self, lang: UiLang) -> bool {
        self.0
            .iter()
            .any(|((l, _), v)| *l == lang.code() && !v.trim().is_empty())
    }

    /// 與 `before` 相比有異動的語系（依 `UiLang::ALL` 順序）
    pub fn changed(&self, before: &Self) -> Vec<UiLang> {
        UiLang::ALL
            .into_iter()
            .filter(|l| {
                let pick = |v: &Self| {
                    v.0.iter()
                        .filter(|((x, _), s)| *x == l.code() && !s.trim().is_empty())
                        .map(|((_, k), s)| (*k, s.trim().to_owned()))
                        .collect::<Vec<_>>()
                };
                pick(self) != pick(before)
            })
            .collect()
    }
}

#[component]
pub fn MultilingualField(
    /// 輸入框 id 前綴（建議用 TipTop 欄位代碼，如 `gaz03`）
    id: String,
    fields: Vec<LangField>,
    values: Signal<LangValues>,
    #[props(default)] readonly: bool,
) -> Element {
    let cols = fields.len().max(1);
    rsx! {
        div { class: "md-multilingual", role: "group",
            for lang in UiLang::ALL {
                div { key: "{lang.code()}", class: "md-multilingual__row",
                    div { class: "md-multilingual__lang", lang: lang.code(),
                        span { "{lang.native_name()}" }
                        if lang == UiLang::ZhTw {
                            span { class: "md-multilingual__default", {t("common-lang-default")} }
                        }
                    }
                    div {
                        class: "md-multilingual__inputs",
                        style: "grid-template-columns: repeat({cols}, minmax(0, 1fr));",
                        for f in fields.iter().cloned() {
                            {field(&id, lang, f, values, readonly)}
                        }
                    }
                }
            }
        }
    }
}

/// 單一多語言欄位，表單上只顯示目前介面語系的值（可直接輸入），
/// 翻譯圖示開啟視窗並列編輯各語系；視窗按「放棄」還原開啟前的內容。
#[component]
pub fn MultilingualDialogField(
    /// 輸入框 id 前綴（建議用 TipTop 欄位代碼，如 `gaz03`）
    id: String,
    field: LangField,
    values: Signal<LangValues>,
    #[props(default)] readonly: bool,
    #[props(default)] class: String,
) -> Element {
    let mut open = use_signal(|| false);
    // 開啟視窗時的內容，按「放棄」時還原
    let mut before = use_signal(LangValues::default);
    let lang = current();
    let key = field.key;
    let value = values.read().get(lang, key);
    // 介面語系空白時，提示顯示時會退回的繁中內容
    let placeholder = if lang == UiLang::ZhTw || readonly {
        String::new()
    } else {
        values.read().get(UiLang::ZhTw, key)
    };
    let filled = UiLang::ALL
        .into_iter()
        .filter(|l| values.read().opt(*l, key).is_some())
        .count();
    let title = tf(
        "common-multilingual-title",
        &[("field", field.label.as_str())],
    );
    let mut cancel = move || {
        values.set(before());
        open.set(false);
    };
    rsx! {
        div { class: "md-multilingual-inline {class}",
            TextField {
                id: format!("{id}-{}", lang.code()),
                label: field.label.clone(),
                readonly,
                maxlength: field.maxlength,
                placeholder,
                value,
                oninput: move |v: String| values.write().set(lang, key, v),
            }
            div { class: "md-multilingual-inline__button",
                IconButton {
                    label: tf("common-multilingual-open", &[("count", filled.to_string().as_str())]),
                    path: icons::TRANSLATE,
                    onclick: move |_| {
                        before.set(values());
                        open.set(true);
                    },
                }
                span { class: "md-multilingual-inline__count", "{filled}/{UiLang::ALL.len()}" }
            }
        }
        if open() {
            Dialog {
                title,
                oncancel: move |_| cancel(),
                actions: rsx! {
                    if readonly {
                        Button { variant: ButtonVariant::Text, onclick: move |_| open.set(false), {t("common-action-close")} }
                    } else {
                        Button { variant: ButtonVariant::Text, onclick: move |_| cancel(), {t("common-action-cancel")} }
                        Button { onclick: move |_| open.set(false), {t("common-action-ok")} }
                    }
                },
                MultilingualField { id: format!("{id}-dialog"), fields: vec![field.clone()], values, readonly }
            }
        }
    }
}

fn field(
    id: &str,
    lang: UiLang,
    f: LangField,
    mut values: Signal<LangValues>,
    readonly: bool,
) -> Element {
    let key = f.key;
    let input_id = format!("{id}-{}-{key}", lang.code());
    let value = values.read().get(lang, key);
    // 非預設語系空白時，提示顯示時會退回的繁中內容
    let placeholder = if lang == UiLang::ZhTw || readonly {
        String::new()
    } else {
        values.read().get(UiLang::ZhTw, key)
    };
    let oninput = move |v: String| values.write().set(lang, key, v);
    match f.rows {
        Some(rows) => rsx! {
            TextArea {
                key: "{input_id}",
                id: input_id.clone(),
                label: f.label,
                rows,
                readonly,
                maxlength: f.maxlength,
                placeholder,
                value,
                oninput,
            }
        },
        None => rsx! {
            TextField {
                key: "{input_id}",
                id: input_id.clone(),
                label: f.label,
                readonly,
                maxlength: f.maxlength,
                placeholder,
                value,
                oninput,
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_changed_languages() {
        let mut before = LangValues::default();
        before.set(UiLang::ZhTw, "name", "料件"); // i18n-allow：測試資料
        let mut after = before.clone();
        after.set(UiLang::En, "name", "  ");
        assert!(after.changed(&before).is_empty(), "只填空白不算異動"); // i18n-allow：測試訊息
        after.set(UiLang::En, "name", "Item");
        assert_eq!(after.changed(&before), [UiLang::En]);
        assert!(after.has(UiLang::En) && !after.has(UiLang::ZhCn));
        assert_eq!(after.opt(UiLang::ZhCn, "name"), None);
    }
}
