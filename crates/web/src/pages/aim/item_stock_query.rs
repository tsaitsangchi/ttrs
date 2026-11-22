//! Item Stock Query 料件庫存數量查詢（TipTop aimq400 / ima_file、img_file）
//! 版面比照 tiptop/aim/4fd/aimq400.4fd：上方料件與三個合計，下方庫存明細；第一筆／上一筆／下一筆／最後一筆切換料件（SQ1）。
//! 唯讀查詢：只有查詢、列印、匯出 Excel（SQ7）。

use dioxus::prelude::*;
use ttrs_shared::{ItemStockDto, QbeCondition, QueryRequest};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Chip, Notification, NotificationKind, TextField, Tone, icons,
};
use crate::components::permission::ProgramGate;
use crate::components::plant::PlantPicker;
use crate::components::qbe::{Qbe, RecordNav, conditions_of};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

const PROGRAM: &str = "item-stock-query";

/// 查詢條件：料件（料號、品名、規格）＋庫存明細（倉庫、儲位、批號、庫存量）（SQ2）
const Q_FIELDS: [(&str, &str); 7] = [
    ("sq-field-item-no", "item_no"),
    ("sq-field-name", "name"),
    ("sq-field-spec", "spec"),
    ("sq-field-warehouse", "warehouse"),
    ("sq-field-location", "location"),
    ("sq-field-lot", "lot"),
    ("sq-field-qty", "qty"),
];

/// 一次最多載入的料件數（查詢作業以料件切換，不分頁顯示）
const MAX_ITEMS: u32 = 1000;

#[component]
pub fn ItemStockQuery() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ItemStockPage {} }
    }
}

#[component]
fn ItemStockPage() -> Element {
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| true);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut items = use_signal(Vec::<ItemStockDto>::new);
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
            match api::query_item_stock(&p, &req).await {
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
                "item-stock",
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
    let cur = list.get(pos()).cloned();
    let date = |d: Option<ttrs_shared::NaiveDate>| d.map(|x| x.to_string()).unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("sq-breadcrumb")} }
                h2 { {t("sq-title")} }
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
                    p { class: "query-hint", {t("sq-hint-query")} }
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

            if let Some(s) = cur {
                Card {
                    div { class: "form-grid",
                        TextField { id: "ima01", label: t("sq-field-item-no"), value: s.item_no.clone(), readonly: true }
                        TextField { id: "ima02", label: t("sq-field-name"), value: s.name.clone().unwrap_or_default(), readonly: true }
                        TextField { id: "ima021", label: t("sq-field-spec"), value: s.spec.clone().unwrap_or_default(), readonly: true }
                        TextField { id: "ima05", label: t("sq-field-version"), value: s.current_version.clone().unwrap_or_default(), readonly: true }
                        TextField { id: "ima08", label: t("sq-field-source"), value: s.source_code.clone().unwrap_or_default(), readonly: true }
                        TextField { id: "ima06", label: t("sq-field-group"), value: s.group_code.clone().unwrap_or_default(), readonly: true }
                        TextField { id: "ima07", label: t("sq-field-abc"), value: s.abc_code.clone().unwrap_or_default(), readonly: true }
                        TextField { id: "ima26", label: t("sq-field-mrp-available"), value: s.mrp_available.normalize().to_string(), readonly: true }
                        TextField { id: "ima262", label: t("sq-field-available"), value: s.available.normalize().to_string(), readonly: true }
                        TextField { id: "ima261", label: t("sq-field-unavailable"), value: s.unavailable.normalize().to_string(), readonly: true }
                    }
                }
                Card {
                    div { class: "table-header",
                        h3 { {tf("sq-lines-title", &[("count", s.lines.len().to_string().as_str()), ("total", s.total.normalize().to_string().as_str())])} }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("sq-field-warehouse")} }
                                    th { {t("sq-field-location")} }
                                    th { {t("sq-field-lot")} }
                                    th { {t("sq-field-usable")} }
                                    th { {t("sq-field-unit")} }
                                    th { {t("sq-field-qty")} }
                                    th { {t("sq-field-expiry")} }
                                    th { {t("sq-field-idle-on")} }
                                }
                            }
                            tbody {
                                if s.lines.is_empty() {
                                    tr { td { colspan: 8, class: "muted", {t("sq-msg-no-stock")} } }
                                }
                                for (i, l) in s.lines.iter().cloned().enumerate() {
                                    tr { key: "{i}",
                                        td { class: "mono", "{l.warehouse}" }
                                        td { class: "mono", "{l.location}" }
                                        td { class: "mono", "{l.lot}" }
                                        td {
                                            Chip {
                                                label: t(if l.usable { "sq-usable-yes" } else { "sq-usable-no" }),
                                                tone: if l.usable { Tone::Success } else { Tone::Error },
                                            }
                                        }
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
