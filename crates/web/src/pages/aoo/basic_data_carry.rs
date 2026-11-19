//! Basic Data Carry 基本資料拋轉（TipTop aoop603 基本資料拋轉作業）
//! 版面比照 tiptop/aoo/4fd/aoop603.4fd（BC1）：上方資料中心與目前營運中心，中間是可拋轉的基本資料（勾選），
//! 下方是拋轉營運中心（拋轉方式 3 才可取消勾選，BC5），按「拋轉」後顯示每個營運中心、每項資料的結果。
//! 批次作業，不套用維護作業的標準功能鈕（ADR 0013 SA1）。規則在後端 BasicCarryService。

use std::collections::BTreeSet;

use dioxus::prelude::*;
use ttrs_shared::{BasicCarryRequest, BasicCarryResultDto};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::i18n::{t, tf};

/// 權限檢查用的程式代號
const PROGRAM: &str = "basic-data-carry";

fn item_label(code: &str) -> String {
    t(&format!("basic-carry-item-{code}"))
}

#[component]
pub fn BasicDataCarry() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, CarryPage {} }
    }
}

#[component]
fn CarryPage() -> Element {
    let perm = use_context::<Perm>();
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut items = use_signal(BTreeSet::<String>::new);
    let mut targets = use_signal(BTreeSet::<String>::new);
    let mut result = use_signal(|| None::<BasicCarryResultDto>);
    let mut asking = use_signal(|| false);
    let mut running = use_signal(|| false);
    let info = use_resource(move || {
        let p = plant();
        async move {
            if p.is_empty() {
                Err(String::new())
            } else {
                api::basic_carry_info(&p).await
            }
        }
    });
    // 開啟時預設全部拋轉營運中心
    use_effect(move || {
        if let Some(Ok(i)) = &*info.read() {
            targets.set(i.targets.iter().cloned().collect());
        }
    });
    let data = info.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let load_error = info
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().err())
        .filter(|e| !e.is_empty())
        .cloned();
    let manual = data.as_ref().is_some_and(|d| d.mode == "3");

    let mut run = move || {
        let p = plant();
        let req = BasicCarryRequest {
            items: items().into_iter().collect(),
            // 拋轉方式 1、2 拋給全部拋轉營運中心（BC5）
            targets: if manual {
                Some(targets().into_iter().collect())
            } else {
                None
            },
        };
        running.set(true);
        spawn(async move {
            match api::run_basic_carry(&p, &req).await {
                Ok(r) => {
                    error.set(None);
                    result.set(Some(r));
                }
                Err(e) => error.set(Some(e)),
            }
            running.set(false);
        });
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("basic-carry-breadcrumb")} }
                h2 { {t("basic-carry-title")} }
            }
            PlantPicker { plant }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = load_error {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| {} }
            }

            if let Some(d) = data.clone() {
                Card {
                    div { class: "table-header",
                        h3 { {tf("basic-carry-center", &[("center", d.center_code.as_str()), ("name", d.center_name.clone().unwrap_or_default().as_str())])} }
                        p { {t(&format!("carry-setting-mode-{}", d.mode))} }
                    }
                    div { class: "action-bar",
                        Button { icon: icons::CHECK, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: {
                            let all = d.items.clone();
                            move |_| items.set(all.iter().cloned().collect())
                        }, {t("basic-carry-action-select-all")} }
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| items.set(BTreeSet::new()), {t("basic-carry-action-select-none")} }
                    }
                    table { class: "md-table",
                        thead { tr { th {} th { {t("basic-carry-field-item")} } } }
                        tbody {
                            for code in d.items.iter().cloned() {
                                tr { key: "{code}",
                                    td {
                                        input {
                                            r#type: "checkbox",
                                            checked: items.read().contains(&code),
                                            onchange: {
                                                let code = code.clone();
                                                move |_| {
                                                    let mut s = items.write();
                                                    if !s.remove(&code) {
                                                        s.insert(code.clone());
                                                    }
                                                }
                                            },
                                        }
                                    }
                                    td { {item_label(&code)} }
                                }
                            }
                        }
                    }
                }
                Card {
                    div { class: "table-header",
                        h3 { {t("basic-carry-targets-title")} }
                        if !manual { p { class: "muted", {t("basic-carry-hint-fixed")} } }
                    }
                    table { class: "md-table",
                        thead { tr { th {} th { {t("carry-setting-field-plant")} } } }
                        tbody {
                            for p in d.targets.iter().cloned() {
                                tr { key: "{p}",
                                    td {
                                        input {
                                            r#type: "checkbox",
                                            disabled: !manual,
                                            checked: targets.read().contains(&p),
                                            onchange: {
                                                let p = p.clone();
                                                move |_| {
                                                    let mut s = targets.write();
                                                    if !s.remove(&p) {
                                                        s.insert(p.clone());
                                                    }
                                                }
                                            },
                                        }
                                    }
                                    td { class: "mono", "{p}" }
                                }
                            }
                        }
                    }
                    if perm.can("carry") {
                        div { class: "action-bar",
                            Button { icon: icons::SYNC, tone: Tone::Primary, disabled: running() || items.read().is_empty() || targets.read().is_empty(), onclick: move |_| {
                                // 拋轉方式 2「詢問後拋轉」先確認（BC5）
                                if d.mode == "2" { asking.set(true); } else { run(); }
                            }, {t("basic-carry-action-carry")} }
                        }
                    }
                }
            }

            if let Some(r) = result() {
                Card {
                    div { class: "table-header", h3 { {t("basic-carry-result-title")} } }
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("carry-setting-field-plant")} }
                                th { {t("basic-carry-field-item")} }
                                th { {t("basic-carry-col-inserted")} }
                                th { {t("basic-carry-col-updated")} }
                                th { {t("basic-carry-col-skipped")} }
                                th { {t("basic-carry-col-result")} }
                            }
                        }
                        tbody {
                            for row in r.rows.iter().cloned() {
                                tr { key: "{row.plant}-{row.item}",
                                    td { class: "mono", "{row.plant}" }
                                    td { {item_label(&row.item)} }
                                    td { class: "num", "{row.inserted}" }
                                    td { class: "num", "{row.updated}" }
                                    td { class: "num", "{row.skipped}" }
                                    td { {if row.ok { t("basic-carry-ok") } else { tf("basic-carry-failed", &[("reason", row.error.clone().unwrap_or_default().as_str())]) }} }
                                }
                            }
                        }
                    }
                    for w in r.mail_warnings.iter() {
                        p { class: "query-hint", {tf("basic-carry-mail-failed", &[("detail", w.as_str())])} }
                    }
                }
            }

            if asking() {
                Dialog {
                    title: t("basic-carry-confirm-title"),
                    oncancel: move |_| asking.set(false),
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| asking.set(false), {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: move |_| { asking.set(false); run(); }, {t("common-action-ok")} }
                    },
                    p { {t("basic-carry-confirm-body")} }
                }
            }
        }
    }
}
