//! 「異動記錄」對話框（docs/adr/0012-change-history.md §8；功能 history）：每次異動的時間、操作者、程式、動作，
//! 以及改前／改後的欄位對照（欄位名稱為資料字典的欄位說明，依語系，CH5）。

use dioxus::prelude::*;
use ttrs_shared::{ChangeDto, DateTime, Utc};

use crate::components::material::{Button, ButtonVariant, Chip, Dialog, Tone, icons};
use crate::i18n::{t, tf};

/// 交易時間以瀏覽器的時區顯示（到秒）
pub(crate) fn local_time(at: DateTime<Utc>) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        #[allow(clippy::cast_precision_loss)]
        let ms = at.timestamp_millis() as f64;
        let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms));
        format!(
            "{:04}/{:02}/{:02} {:02}:{:02}:{:02}",
            d.get_full_year(),
            d.get_month() + 1,
            d.get_date(),
            d.get_hours(),
            d.get_minutes(),
            d.get_seconds()
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // 桌面版（非瀏覽器）沒有瀏覽器時區，以 UTC 顯示
        at.format("%Y/%m/%d %H:%M:%S UTC").to_string()
    }
}

/// 動作的顯示文字與色調
fn operation(op: &str) -> (String, Tone) {
    match op {
        "insert" => (t("history-op-insert"), Tone::Success),
        "update" => (t("history-op-update"), Tone::Primary),
        "delete" => (t("history-op-delete"), Tone::Error),
        "activate" => (t("history-op-activate"), Tone::Success),
        "deactivate" => (t("history-op-deactivate"), Tone::Error),
        other => (other.to_owned(), Tone::Neutral),
    }
}

/// 操作者：AI 代表使用者、直接修改資料庫另外標示
fn actor(c: &ChangeDto) -> String {
    match c.actor_kind.as_str() {
        "ai" => tf(
            "history-actor-ai",
            &[("user", c.on_behalf_of.as_deref().unwrap_or(&c.user_id))],
        ),
        "db" => tf("history-actor-db", &[("user", c.user_id.as_str())]),
        _ => c.user_id.clone(),
    }
}

#[component]
pub fn HistoryDialog(
    /// 資料的鍵值（例如部門編號），顯示在標題
    record: String,
    changes: Vec<ChangeDto>,
    onclose: EventHandler<()>,
) -> Element {
    rsx! {
        Dialog {
            title: tf("history-dialog-title", &[("key", record.as_str())]),
            oncancel: move |_| onclose.call(()),
            actions: rsx! {
                Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| onclose.call(()), {t("common-action-close")} }
            },
            if changes.is_empty() {
                p { class: "muted", {t("history-msg-none")} }
            }
            div { class: "history-list",
                // 新的在上
                for c in changes.iter().rev() {
                    div { key: "{c.change_id}", class: "history-entry",
                        div { class: "history-entry__head",
                            {
                                let (label, tone) = operation(&c.operation);
                                rsx! { Chip { label, tone } }
                            }
                            span { class: "mono", {local_time(c.occurred_at)} }
                            span { {actor(c)} }
                            if let Some(p) = &c.program {
                                span { class: "muted mono", "{p}" }
                            }
                            if let Some(v) = c.version {
                                span { class: "muted", {tf("history-version", &[("version", v.to_string().as_str())])} }
                            }
                        }
                        if !c.fields.is_empty() {
                            table { class: "md-table history-entry__fields",
                                thead { tr {
                                    th { {t("history-col-field")} }
                                    th { {t("history-col-before")} }
                                    th { {t("history-col-after")} }
                                } }
                                tbody {
                                    for f in c.fields.iter() {
                                        tr { key: "{f.column}",
                                            td { "{f.label}" }
                                            td { class: "history-before", {f.before.clone().unwrap_or_default()} }
                                            td { class: "history-after", {f.after.clone().unwrap_or_default()} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 共用層作業的「異動記錄」按鈕：點按時查詢並開啟對話框（功能 history，由呼叫端依權限決定是否顯示）
#[component]
pub fn HistoryButton(
    /// 程式代號，例如 user-maintenance
    program: &'static str,
    /// 目前資料的鍵值；None 時按鈕停用
    record: Option<String>,
) -> Element {
    let mut open = use_signal(|| None::<(String, Vec<ChangeDto>)>);
    let mut error = use_signal(|| None::<String>);
    let key = record.clone();
    rsx! {
        Button {
            icon: icons::HISTORY,
            tone: Tone::Primary,
            variant: ButtonVariant::Outlined,
            disabled: record.is_none(),
            onclick: move |_| {
                let Some(k) = key.clone() else { return };
                spawn(async move {
                    match crate::api::shared_history(program, &k).await {
                        Ok(h) => {
                            error.set(None);
                            open.set(Some((k, h)));
                        }
                        Err(e) => error.set(Some(e)),
                    }
                });
            },
            {t("common-action-history")}
        }
        if let Some(msg) = error() {
            span { class: "history-error", "{msg}" }
        }
        if let Some((k, changes)) = open() {
            HistoryDialog { record: k, changes, onclose: move |_| open.set(None) }
        }
    }
}
