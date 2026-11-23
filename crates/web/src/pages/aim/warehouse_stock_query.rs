//! Warehouse Stock Query 倉庫料件數量查詢（TipTop aimq401 / imd_file、img_file、ima_file）
//! 版面比照 tiptop/aim/4fd/aimq401.4fd：上方倉庫，下方庫存明細；第一筆／上一筆／下一筆／最後一筆切換倉庫（WK1）。
//! 唯讀查詢：只有查詢、列印、匯出 Excel（WK6）。

use dioxus::prelude::*;
use ttrs_shared::{QbeCondition, QueryRequest, WarehouseStockDto};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Notification, NotificationKind, TextField, Tone, icons,
};
use crate::components::permission::ProgramGate;
use crate::components::plant::PlantPicker;
use crate::components::qbe::{Qbe, RecordNav, conditions_of};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

const PROGRAM: &str = "warehouse-stock-query";

/// 查詢條件：倉庫（單頭）＋料件、儲位、批號、單位、庫存量、呆滯日期（單身）（WK2）
const Q_FIELDS: [(&str, &str); 7] = [
    ("wk-field-warehouse", "warehouse"),
    ("wk-field-item-no", "item_no"),
    ("wk-field-location", "location"),
    ("wk-field-lot", "lot"),
    ("wk-field-unit", "unit"),
    ("wk-field-qty", "qty"),
    ("wk-field-idle-on", "idle_on"),
];

/// 一次最多載入的倉庫數（查詢作業以倉庫切換，不分頁顯示）
const MAX_WAREHOUSES: u32 = 1000;

#[component]
pub fn WarehouseStockQuery() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, WarehouseStockPage {} }
    }
}

#[component]
fn WarehouseStockPage() -> Element {
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| true);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut warehouses = use_signal(Vec::<WarehouseStockDto>::new);
    let mut total = use_signal(|| 0u64);
    let mut pos = use_signal(|| 0usize);

    let run_query = move |_| {
        let conds = conditions_of(&qbe.read());
        let req = QueryRequest {
            conditions: conds.clone(),
            page: Some(1),
            page_size: Some(MAX_WAREHOUSES),
            lang: None,
        };
        let p = plant();
        spawn(async move {
            match api::query_warehouse_stock(&p, &req).await {
                Ok(result) => {
                    error.set(None);
                    querying.set(false);
                    conditions.set(conds);
                    total.set(result.total);
                    warehouses.set(result.items);
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
                "warehouse-stock",
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

    let list = warehouses();
    let cur = list.get(pos()).cloned();
    let date = |d: Option<ttrs_shared::NaiveDate>| d.map(|x| x.to_string()).unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("wk-breadcrumb")} }
                h2 { {t("wk-title")} }
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
                    p { class: "query-hint", {t("wk-hint-query")} }
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

            if !querying() && total() > 0 {
                RecordNav {
                    total: total(),
                    pos: Some(pos() as u64),
                    ongo: move |i: u64| { message.set(None); pos.set(usize::try_from(i).unwrap_or(0).min(warehouses.read().len().saturating_sub(1))); },
                    onerror: move |e| error.set(Some(e)),
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            if let Some(s) = cur {
                Card {
                    div { class: "form-grid",
                        TextField { id: "imd01", label: t("wk-field-warehouse"), value: s.warehouse.clone(), readonly: true }
                        TextField { id: "imd02", label: t("wk-field-warehouse-name"), value: s.name.clone().unwrap_or_default(), readonly: true }
                    }
                }
                Card {
                    div { class: "table-header",
                        h3 { {tf("wk-lines-title", &[("count", s.lines.len().to_string().as_str())])} }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("wk-field-item-no")} }
                                    th { {t("wk-field-name")} }
                                    th { {t("wk-field-spec")} }
                                    th { {t("wk-field-location")} }
                                    th { {t("wk-field-lot")} }
                                    th { {t("wk-field-unit")} }
                                    th { {t("wk-field-qty")} }
                                    th { {t("wk-field-expiry")} }
                                    th { {t("wk-field-idle-on")} }
                                }
                            }
                            tbody {
                                if s.lines.is_empty() {
                                    tr { td { colspan: 9, class: "muted", {t("wk-msg-no-stock")} } }
                                }
                                for (i, l) in s.lines.iter().cloned().enumerate() {
                                    tr { key: "{i}",
                                        td { class: "mono", "{l.item_no}" }
                                        td { {l.name.clone().unwrap_or_default()} }
                                        td { {l.spec.clone().unwrap_or_default()} }
                                        td { class: "mono", "{l.location}" }
                                        td { class: "mono", "{l.lot}" }
                                        td { class: "mono", "{l.unit}" }
                                        td { class: "num", {l.qty.normalize().to_string()} }
                                        td { {date(l.expiry)} }
                                        td { {date(l.idle_on)} }
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
