//! Item Available Query 料件數量查詢（TipTop aimq402 / ima_file、img_file）
//! 版面比照 tiptop/aim/4fd/aimq402.4fd：單一清單，一個料件一列；第一筆／上一筆／下一筆／最後一筆移動目前列（IA1、IA6）。
//! 唯讀查詢：只有查詢、列印、匯出 Excel（IA6）。

use dioxus::prelude::*;
use ttrs_shared::{ItemAvailableDto, QbeCondition, QueryRequest};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Notification, NotificationKind, TextField, Tone, icons,
};
use crate::components::permission::ProgramGate;
use crate::components::plant::PlantPicker;
use crate::components::qbe::{Qbe, RecordNav, conditions_of};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

const PROGRAM: &str = "item-available-query";

/// 查詢條件：料件編號、品名、規格（IA2）
const Q_FIELDS: [(&str, &str); 3] = [
    ("ia-field-item-no", "item_no"),
    ("ia-field-name", "name"),
    ("ia-field-spec", "spec"),
];

/// 一次最多載入的料件數
const MAX_ITEMS: u32 = 1000;

#[component]
pub fn ItemAvailableQuery() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ItemAvailablePage {} }
    }
}

#[component]
fn ItemAvailablePage() -> Element {
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| true);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut items = use_signal(Vec::<ItemAvailableDto>::new);
    let mut total = use_signal(|| 0u64);
    let mut pos = use_signal(|| 0usize);

    let run_query = move |_| {
        let conds = conditions_of(&qbe.read());
        let req = QueryRequest {
            conditions: conds.clone(),
            page: Some(1),
            page_size: Some(MAX_ITEMS),
            lang: None,
        };
        let p = plant();
        spawn(async move {
            match api::query_item_available(&p, &req).await {
                Ok(result) => {
                    error.set(None);
                    querying.set(false);
                    conditions.set(conds);
                    total.set(result.total);
                    items.set(result.items);
                    pos.set(0);
                    message.set(Some(if result.total == 0 {
                        t("query-msg-none")
                    } else {
                        tf(
                            "query-msg-matched",
                            &[("count", result.total.to_string().as_str())],
                        )
                    }));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        let p = plant();
        spawn(async move {
            if let Err(e) = api::list_report(
                &p,
                "item-available",
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

    let list = items();
    let loaded = list.len() as u64;
    let text = |v: &Option<String>| v.clone().unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("ia-breadcrumb")} }
                h2 { {t("ia-title")} }
            }
            PlantPicker { plant }

            if querying() {
                Card {
                    div { class: "form-grid",
                        for (id, field) in Q_FIELDS {
                            TextField {
                                key: "{field}",
                                id: "q-{field}",
                                label: t(id),
                                value: qbe.read().get(field).cloned().unwrap_or_default(),
                                oninput: move |v: String| { qbe.write().insert(field, v); },
                            }
                        }
                    }
                }
                div { class: "action-bar",
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { error.set(None); querying.set(false); }, {t("common-action-cancel")} }
                }
            } else {
                div { class: "action-bar",
                    MaintenanceToolbar {
                        query: Act::new(true, move |_| { message.set(None); error.set(None); querying.set(true); }),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                    }
                }
            }

            if !querying() && loaded > 0 {
                RecordNav {
                    total: loaded,
                    pos: Some(pos() as u64),
                    ongo: move |i: u64| { message.set(None); pos.set(usize::try_from(i).unwrap_or(0).min(items.read().len().saturating_sub(1))); },
                    onerror: move |e| error.set(Some(e)),
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            if !querying() {
                Card {
                    div { class: "table-header",
                        h3 { {tf("ia-list-title", &[("count", total().to_string().as_str())])} }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("ia-field-item-no")} }
                                    th { {t("ia-field-name")} }
                                    th { {t("ia-field-spec")} }
                                    th { {t("ia-field-unit")} }
                                    th { {t("ia-field-available")} }
                                    th { {t("ia-field-version")} }
                                    th { {t("ia-field-source")} }
                                    th { {t("ia-field-group")} }
                                    th { {t("ia-field-abc")} }
                                }
                            }
                            tbody {
                                if list.is_empty() {
                                    tr { td { colspan: 9, class: "muted", {t("common-msg-no-data")} } }
                                }
                                for (i, d) in list.iter().cloned().enumerate() {
                                    tr {
                                        key: "{d.item_no}",
                                        class: "md-table__row--clickable",
                                        class: if i == pos() { "md-table__row--selected" },
                                        onclick: move |_| pos.set(i),
                                        td { class: "mono", "{d.item_no}" }
                                        td { {text(&d.name)} }
                                        td { {text(&d.spec)} }
                                        td { class: "mono", {text(&d.unit)} }
                                        td { class: "num", {d.available.normalize().to_string()} }
                                        td { {text(&d.current_version)} }
                                        td { {text(&d.source_code)} }
                                        td { {text(&d.group_code)} }
                                        td { {text(&d.abc_code)} }
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
