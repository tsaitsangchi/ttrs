//! TipTop 對照目錄（TipTop Catalog）：從資料字典維護切換進入，唯讀瀏覽 TipTop 資料表與欄位的英文命名。
//! 規格：specs/features/azz/legacy_catalog.feature（C1）。資料由 make legacy-catalog 匯入，畫面不修改（C2）。
//! 版面比照資料字典維護（p_zta.4fd）：查詢＋上下筆、主畫面（檔案）、欄位頁籤、清單。

use dioxus::prelude::*;
use ttrs_shared::{LegacyTableDto, LegacyTableSummaryDto};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Chip, IconButton, TextArea, TextField, Tone, icons,
};
use crate::i18n::{self, tf};

/// TipTop 對照目錄瀏覽；`onclose` 回到資料字典
#[component]
pub fn LegacyCatalog(onclose: EventHandler<()>) -> Element {
    let mut query = use_signal(String::new);
    let mut applied = use_signal(String::new);
    let list = use_resource(move || {
        let q = applied();
        async move { api::search_legacy(&q).await }
    });
    let mut pos = use_signal(|| 0usize);

    let items: Vec<LegacyTableSummaryDto> = list
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let total = items.len();
    let cur = items.get(pos()).map(|t| t.legacy_name.clone());

    rsx! {
        div { class: "action-bar",
            TextField {
                id: "legacy-q",
                class: "legacy-search",
                label: i18n::t("legacy-field-query"),
                placeholder: i18n::t("legacy-hint-query"),
                value: query(),
                oninput: move |v| query.set(v),
            }
            Button {
                icon: icons::SEARCH, tone: Tone::Primary,
                variant: ButtonVariant::Tonal,
                onclick: move |_| {
                    pos.set(0);
                    applied.set(query());
                },
                {i18n::t("common-action-query")}
            }
            div { class: "spacer" }
            Button { icon: icons::ARROW_BACK, tone: Tone::Neutral, variant: ButtonVariant::Outlined, onclick: move |_| onclose.call(()), {i18n::t("legacy-action-back")} }
        }

        if total > 0 {
            div { class: "record-nav",
                IconButton { label: i18n::t("nav-first"), path: icons::FIRST_PAGE, disabled: pos() == 0, onclick: move |_| pos.set(0) }
                IconButton { label: i18n::t("nav-previous"), path: icons::CHEVRON_LEFT, disabled: pos() == 0, onclick: move |_| pos.set(pos().saturating_sub(1)) }
                span { class: "record-nav__count",
                    {tf("nav-record-position", &[("pos", (pos() + 1).to_string().as_str()), ("total", total.to_string().as_str())])}
                }
                IconButton { label: i18n::t("nav-next"), path: icons::CHEVRON_RIGHT, disabled: pos() + 1 >= total, onclick: move |_| pos.set((pos() + 1).min(total - 1)) }
                IconButton { label: i18n::t("nav-last"), path: icons::LAST_PAGE, disabled: pos() + 1 >= total, onclick: move |_| pos.set(total - 1) }
            }
        }

        match cur {
            Some(legacy) => rsx! { LegacyDetail { key: "{legacy}", legacy: legacy.clone() } },
            None => rsx! {
                Card { div { class: "card-body muted",
                    match &*list.read() {
                        None => rsx! { {i18n::t("common-msg-loading")} },
                        Some(Err(e)) => rsx! { "{e}" },
                        Some(Ok(_)) => rsx! { {i18n::t("legacy-msg-empty")} },
                    }
                } }
            },
        }

        // ----- 清單 -----
        Card {
            div { class: "table-header",
                h3 { {i18n::t("legacy-list-title")} }
                p { {tf("list-count-all", &[("total", total.to_string().as_str())])} {i18n::t("list-hint-click")} }
            }
            div { class: "table-scroll",
                table { class: "md-table",
                    thead { tr {
                        for id in ["legacy-col-legacy-table", "legacy-col-name", "legacy-col-title", "legacy-col-module", "legacy-col-column-count"] {
                            th { key: "{id}", {i18n::t(id)} }
                        }
                    } }
                    tbody {
                        for (i, t) in items.iter().cloned().enumerate() {
                            tr {
                                key: "{t.legacy_name}",
                                class: "md-table__row--clickable",
                                class: if pos() == i { "md-table__row--selected" },
                                onclick: move |_| pos.set(i),
                                td { class: "mono", "{t.legacy_name}" }
                                td { class: "mono", "{t.name}" }
                                td { "{t.title}" }
                                td { "{t.module}" }
                                td { class: "num", "{t.column_count}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 一張 TipTop 資料表的對照：主畫面＋欄位
#[component]
fn LegacyDetail(legacy: String) -> Element {
    let table = use_resource(move || {
        let l = legacy.clone();
        async move { api::get_legacy(&l).await }
    });
    let loaded = table.read().clone();
    rsx! {
        match loaded {
            None => rsx! { Card { div { class: "card-body muted", {i18n::t("common-msg-loading")} } } },
            Some(Err(e)) => rsx! { Card { div { class: "card-body muted", "{e}" } } },
            Some(Ok(t)) => rsx! {
                Card {
                    div { class: "card-body",
                        div { class: "form-grid cols-4",
                            TextField { id: "legacy-table", label: i18n::t("legacy-col-legacy-table"), value: t.legacy_name.clone(), readonly: true }
                            TextField { id: "legacy-name", label: i18n::t("legacy-col-name"), value: t.name.clone(), readonly: true }
                            TextField { id: "legacy-title", label: i18n::t("legacy-col-title"), value: t.title.clone(), readonly: true }
                            TextField { id: "legacy-module", label: i18n::t("legacy-col-module"), value: t.module.clone(), readonly: true }
                            if let Some(p) = t.purpose.clone() {
                                div { class: "span-all",
                                    TextArea { id: "legacy-purpose", label: i18n::t("legacy-field-purpose"), rows: 2, readonly: true, value: p }
                                }
                            }
                        }
                    }
                }
                Card {
                    div { class: "card-body", LegacyColumns { table: t.clone(), ttrs_columns: None } }
                }
            },
        }
    }
}

/// 欄位對照表格。`ttrs_columns` 有值時（資料字典的 TipTop 對照頁籤）另顯示該欄位是否已在 TTRS 資料表中
#[component]
pub fn LegacyColumns(table: LegacyTableDto, ttrs_columns: Option<Vec<String>>) -> Element {
    let in_ttrs = ttrs_columns.clone();
    rsx! {
        div { class: "table-scroll",
            table { class: "md-table",
                thead { tr {
                    for id in ["legacy-col-legacy-column", "legacy-col-name", "legacy-col-type", "legacy-col-pk", "legacy-col-description", "legacy-col-notes", "legacy-col-status"] {
                        th { key: "{id}", {i18n::t(id)} }
                    }
                    if ttrs_columns.is_some() {
                        th { {i18n::t("legacy-col-in-ttrs")} }
                    }
                } }
                tbody {
                    for c in table.columns.iter().cloned() {
                        tr {
                            key: "{c.legacy_name}",
                            class: if c.unused { "legacy-row--unused" },
                            td { class: "mono", "{c.legacy_name}" }
                            td { class: "mono", "{c.name}" }
                            td { class: "mono", "{c.data_type}" }
                            td { if c.primary_key { "PK" } }
                            td { "{c.description}" }
                            td { class: "dict-extra", {c.notes.clone().unwrap_or_default()} }
                            td {
                                if c.unused { Chip { label: i18n::t("legacy-chip-unused") } }
                                if c.verify { Chip { label: i18n::t("legacy-chip-verify"), warning: true } }
                            }
                            if let Some(cols) = in_ttrs.as_ref() {
                                td { if cols.contains(&c.name) { "✓" } }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 資料字典的「TipTop 對照」頁籤：目前資料表對應的 TipTop 舊表（C1）
#[component]
pub fn LegacyMappingTab(table: String) -> Element {
    let t1 = table.clone();
    let legacy = use_resource(move || {
        let t = t1.clone();
        async move { api::table_legacy(&t).await }
    });
    let columns = use_resource(move || {
        let t = table.clone();
        async move { api::table_columns(&t).await }
    });
    let ttrs: Vec<String> = columns
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|cs| cs.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default();
    rsx! {
        match &*legacy.read() {
            None => rsx! { p { class: "muted", {i18n::t("common-msg-loading")} } },
            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
            Some(Ok(None)) => rsx! { p { class: "muted", {i18n::t("legacy-msg-no-mapping")} } },
            Some(Ok(Some(t))) => rsx! {
                p { class: "muted", {tf("legacy-msg-mapped", &[("legacy", t.legacy_name.as_str()), ("title", t.title.as_str())])} }
                LegacyColumns { table: t.clone(), ttrs_columns: Some(ttrs.clone()) }
            },
        }
    }
}
