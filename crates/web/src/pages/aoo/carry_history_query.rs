//! Carry History Query 拋轉歷史查詢（TipTop aooq604 資料拋轉歷史查詢）
//! 版面比照 tiptop/aoo/4fd/aooq604.4fd（BC9）：查詢條件（資料中心、資料類別、拋轉營運中心、程式代號、主鍵值、拋轉日期、人員），
//! 下方為拋轉紀錄（新到舊）；可列印、匯出 Excel。查詢類作業不套用新增、修改等功能鈕（ADR 0013 SA1）。

use dioxus::prelude::*;
use ttrs_shared::{QbeCondition, QueryRequest};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Notification, NotificationKind, TextField, Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::qbe::{ListPager, PAGE_SIZE, Qbe, conditions_of};
use crate::i18n::{t, tf};

/// 權限檢查用的程式代號
const PROGRAM: &str = "carry-history-query";

#[component]
pub fn CarryHistoryQuery() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, HistoryPage {} }
    }
}

#[component]
fn HistoryPage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let logs = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move { api::query_carry_history(&req).await }
    });
    let data = logs.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let total = data.as_ref().map_or(0, |p| p.total);
    let pages = data.as_ref().map_or(1, |p| {
        u32::try_from(p.total.div_ceil(u64::from(p.page_size.max(1))))
            .unwrap_or(1)
            .max(1)
    });

    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            if let Err(e) = api::system_list_report(
                "/api/carry-history",
                "carry-history",
                format,
                &conds,
                crate::i18n::current().code(),
                tab,
            )
            .await
            {
                error.set(Some(e));
            }
        });
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("carry-history-breadcrumb")} }
                h2 { {t("carry-history-title")} }
            }
            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            Card {
                div { class: "form-grid",
                    for (id, field, hint) in [("carry-history-col-center", "center_code", ""), ("carry-history-col-data-type", "data_type", ""), ("carry-history-col-plant", "plant", ""), ("carry-history-col-program", "program", ""), ("carry-history-col-key", "record_key", ""), ("carry-history-field-date", "carried_on", crate::components::qbe::HINT_DATE), ("carry-history-col-by", "carried_by", "")] {
                        TextField {
                            key: "{field}",
                            id: "q-{field}",
                            label: t(id),
                            placeholder: if hint.is_empty() { String::new() } else { t(hint) },
                            value: qbe.read().get(field).cloned().unwrap_or_default(),
                            oninput: move |v: String| { qbe.write().insert(field, v); },
                        }
                    }
                }
                div { class: "action-bar",
                    Button { icon: icons::SEARCH, tone: Tone::Primary, onclick: move |_| { error.set(None); conditions.set(conditions_of(&qbe.read())); page.set(1); }, {t("common-action-query")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { qbe.set(Default::default()); conditions.set(Vec::new()); page.set(1); }, {t("carry-history-action-clear")} }
                    if perm.can("output") {
                        Button { icon: icons::PRINT, tone: Tone::Neutral, variant: ButtonVariant::Outlined, onclick: move |_| report(api::ReportFormat::Pdf), {t("common-action-print")} }
                    }
                    if perm.can("exporttoexcel") {
                        Button { icon: icons::DOWNLOAD, tone: Tone::Neutral, variant: ButtonVariant::Outlined, onclick: move |_| report(api::ReportFormat::Excel), {t("common-action-export-excel")} }
                    }
                }
            }
            Card {
                div { class: "table-header",
                    h3 { {t("carry-history-list-title")} }
                    p { {tf("list-count-all", &[("total", total.to_string().as_str())])} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("carry-history-col-center")} }
                                th { {t("carry-history-col-data-type")} }
                                th { {t("carry-history-col-plant")} }
                                th { {t("carry-history-col-program")} }
                                th { {t("carry-history-col-key")} }
                                th { {t("carry-history-col-action")} }
                                th { {t("carry-history-col-at")} }
                                th { {t("carry-history-col-by")} }
                            }
                        }
                        tbody {
                            match &*logs.read() {
                                None => rsx! { tr { td { colspan: 8, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 8, "{e}" } } },
                                Some(Ok(p)) if p.items.is_empty() => rsx! { tr { td { colspan: 8, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(p)) => rsx! {
                                    for (i, l) in p.items.iter().cloned().enumerate() {
                                        tr { key: "{i}",
                                            td { class: "mono", "{l.center_code}" }
                                            td { {t(&format!("group-center-data-type-{}", l.data_type))} }
                                            td { class: "mono", "{l.plant}" }
                                            td { class: "mono", "{l.program}" }
                                            td { class: "mono", "{l.record_key}" }
                                            td { {t(&format!("carry-history-action-{}", l.action))} }
                                            td { {crate::components::history::local_time(l.carried_at)} }
                                            td { "{l.carried_by}" }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
                ListPager { page, pages }
            }
        }
    }
}
