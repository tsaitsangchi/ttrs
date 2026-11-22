//! Item Bin Card 料件 BIN 卡查詢（TipTop aimq231 / img_file、tlf_file）
//! 版面比照 tiptop/aim/4fd/aimq231.4fd：上方料件、倉儲批、年度期別、期初與目前庫存，下方異動明細；
//! 第一筆／上一筆／下一筆／最後一筆切換倉儲批（BN1）。唯讀查詢：只有查詢、列印、匯出 Excel（BN6）。

use dioxus::prelude::*;
use ttrs_shared::{BinCardDto, BinCardQuery, QbeCondition, QueryRequest};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Notification, NotificationKind, TextField, Tone, icons,
};
use crate::components::permission::ProgramGate;
use crate::components::plant::PlantPicker;
use crate::components::qbe::{Qbe, RecordNav, conditions_of};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

const PROGRAM: &str = "item-bin-card";

/// 查詢條件（BN2）
const Q_FIELDS: [(&str, &str); 9] = [
    ("bn-field-item-no", "item_no"),
    ("bn-field-name", "name"),
    ("bn-field-spec", "spec"),
    ("bn-field-source", "source_code"),
    ("bn-field-warehouse", "warehouse"),
    ("bn-field-location", "location"),
    ("bn-field-lot", "lot"),
    ("bn-field-unit", "unit"),
    ("bn-field-qty", "qty"),
];

const MAX_CARDS: u32 = 1000;

/// 今天的年、月（期別預設值；後端檢查該期是否存在）
fn this_month() -> (String, String) {
    #[cfg(target_arch = "wasm32")]
    {
        let d = js_sys::Date::new_0();
        (
            d.get_full_year().to_string(),
            (d.get_month() + 1).to_string(),
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        (String::new(), String::new())
    }
}

fn program_name(code: &str) -> String {
    let id = format!("bn-program-{code}");
    let s = t(&id);
    if s == id { code.to_owned() } else { s }
}

#[component]
pub fn ItemBinCard() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, BinCardPage {} }
    }
}

#[component]
fn BinCardPage() -> Element {
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| true);
    let mut qbe: Qbe = use_signal(Default::default);
    // 年度、期別預設今天的年月（TipTop 依今天的會計期間；後端檢查期別是否存在）
    let (y0, m0) = this_month();
    let mut year = use_signal(move || y0.clone());
    let mut period = use_signal(move || m0.clone());
    let mut asked = use_signal(|| (0i32, 0i16, Vec::<QbeCondition>::new()));
    let mut cards = use_signal(Vec::<BinCardDto>::new);
    let mut total = use_signal(|| 0u64);
    let mut pos = use_signal(|| 0usize);

    let run_query = move |_| {
        let (Ok(y), Ok(m)) = (year().trim().parse::<i32>(), period().trim().parse::<i16>()) else {
            error.set(Some(t("bn-msg-period-required")));
            return;
        };
        let conds = conditions_of(&qbe.read());
        let req = BinCardQuery {
            year: y,
            period: m,
            request: QueryRequest {
                conditions: conds.clone(),
                page: Some(1),
                page_size: Some(MAX_CARDS),
                lang: None,
            },
        };
        let p = plant();
        spawn(async move {
            match api::query_bin_cards(&p, &req).await {
                Ok(result) => {
                    error.set(None);
                    querying.set(false);
                    asked.set((y, m, conds));
                    total.set(result.total);
                    cards.set(result.items);
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
        let (y, m, conds) = asked();
        let tab = api::ReportTab::open(format);
        let p = plant();
        spawn(async move {
            if let Err(e) =
                api::bin_card_report(&p, y, m, format, &conds, crate::i18n::current().code(), tab)
                    .await
            {
                error.set(Some(e));
            }
        });
    };

    let list = cards();
    let cur = list.get(pos()).cloned();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("bn-breadcrumb")} }
                h2 { {t("bn-title")} }
            }
            PlantPicker { plant }

            if querying() {
                Card {
                    div { class: "form-grid",
                        TextField { id: "yy", label: t("bn-field-year"), input_type: "number", value: year(), oninput: move |v| year.set(v) }
                        TextField { id: "mm", label: t("bn-field-period"), input_type: "number", value: period(), oninput: move |v| period.set(v) }
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
                    p { class: "query-hint", {t("bn-hint-query")} }
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
                    ongo: move |i: u64| { message.set(None); pos.set(usize::try_from(i).unwrap_or(0).min(cards.read().len().saturating_sub(1))); },
                    onerror: move |e| error.set(Some(e)),
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            if let Some(c) = cur {
                Card {
                    div { class: "form-grid",
                        TextField { id: "img01", label: t("bn-field-item-no"), value: c.item_no.clone(), readonly: true }
                        TextField { id: "ima02", label: t("bn-field-name"), value: c.name.clone().unwrap_or_default(), readonly: true }
                        TextField { id: "ima021", label: t("bn-field-spec"), value: c.spec.clone().unwrap_or_default(), readonly: true }
                        TextField { id: "ima08", label: t("bn-field-source"), value: c.source_code.clone().unwrap_or_default(), readonly: true }
                        TextField { id: "img02", label: t("bn-field-warehouse"), value: c.warehouse.clone(), readonly: true }
                        TextField { id: "img03", label: t("bn-field-location"), value: c.location.clone(), readonly: true }
                        TextField { id: "img04", label: t("bn-field-lot"), value: c.lot.clone(), readonly: true }
                        TextField { id: "img09", label: t("bn-field-unit"), value: c.unit.clone(), readonly: true }
                        TextField { id: "yymm", label: t("bn-field-year-period"), value: format!("{}/{:02}", c.year, c.period), readonly: true } // i18n-allow
                        TextField { id: "imk09", label: t("bn-field-opening"), value: c.opening.normalize().to_string(), readonly: true }
                        TextField { id: "img10", label: t("bn-field-current"), value: c.current.normalize().to_string(), readonly: true }
                    }
                    p { class: "muted", {t("bn-hint-opening")} }
                }
                Card {
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("bn-field-txn-date")} }
                                    th { {t("bn-field-program")} }
                                    th { {t("bn-field-doc-no")} }
                                    th { {t("bn-field-doc-line")} }
                                    th { {t("bn-field-qty")} }
                                    th { {t("bn-field-unit")} }
                                    th { {t("bn-field-balance")} }
                                }
                            }
                            tbody {
                                if c.lines.is_empty() {
                                    tr { td { colspan: 7, class: "muted", {t("bn-msg-no-movement")} } }
                                }
                                for (i, l) in c.lines.iter().cloned().enumerate() {
                                    tr { key: "{i}",
                                        td { "{l.txn_date}" }
                                        td { {program_name(&l.program)} }
                                        td { class: "mono", "{l.doc_no}" }
                                        td { class: "mono", "{l.doc_line}" }
                                        td { class: "num", {l.qty.normalize().to_string()} }
                                        td { class: "mono", "{l.unit}" }
                                        td { class: "num", {l.balance.normalize().to_string()} }
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
