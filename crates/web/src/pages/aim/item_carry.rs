//! 料件維護的「拋轉」「拋轉歷史」（TipTop aimi100 carry、qry_carry_history；specs/features/aim/item_carry.feature IC1）
//! 拋轉：跳出拋轉營運中心清單（拋轉方式 3 可取消勾選、2 先確認），執行後顯示每個營運中心的結果；
//! 確認時自動拋轉（拋轉方式 1，IC3）的結果也在這裡顯示。規則在後端 ItemCarryService。

use std::collections::BTreeSet;

use dioxus::prelude::*;
use ttrs_shared::{CarryLogDto, ItemCarryInfoDto, ItemCarryRequest, ItemCarryResultDto};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Tone, icons,
};
use crate::components::permission::Perm;
use crate::i18n::{t, tf};

fn outcome_label(o: &str) -> String {
    t(&format!("item-carry-outcome-{o}"))
}

/// 拋轉、拋轉歷史按鈕與結果；`result` 由料件確認時的自動拋轉一併帶入
#[component]
pub fn ItemCarryActions(
    plant: String,
    item_no: Option<String>,
    result: Signal<Option<ItemCarryResultDto>>,
) -> Element {
    let perm = use_context::<Perm>();
    let mut info = use_signal(|| None::<ItemCarryInfoDto>);
    let mut selected = use_signal(BTreeSet::<String>::new);
    let mut error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<Vec<CarryLogDto>>);
    let mut result = result;
    let no_item = item_no.is_none();

    let open = {
        let plant = plant.clone();
        move |_| {
            let plant = plant.clone();
            spawn(async move {
                match api::item_carry_info(&plant).await {
                    Ok(i) => {
                        error.set(None);
                        selected.set(i.targets.iter().cloned().collect());
                        info.set(Some(i));
                    }
                    Err(e) => error.set(Some(e)),
                }
            });
        }
    };
    let run = {
        let (plant, no) = (plant.clone(), item_no.clone().unwrap_or_default());
        move |_| {
            let Some(i) = info() else { return };
            let req = ItemCarryRequest {
                targets: (i.mode == "3").then(|| selected().into_iter().collect()),
            };
            let (plant, no) = (plant.clone(), no.clone());
            spawn(async move {
                match api::run_item_carry(&plant, &no, &req).await {
                    Ok(r) => {
                        info.set(None);
                        error.set(None);
                        result.set(Some(r));
                    }
                    Err(e) => error.set(Some(e)),
                }
            });
        }
    };
    let show_history = {
        let no = item_no.clone().unwrap_or_default();
        move |_| {
            let no = no.clone();
            spawn(async move {
                match api::item_carry_history(&no).await {
                    Ok(h) => history.set(Some(h)),
                    Err(e) => error.set(Some(e)),
                }
            });
        }
    };

    rsx! {
        if perm.can("carry") {
            Button { icon: icons::SYNC, tone: Tone::Primary, variant: ButtonVariant::Outlined, disabled: no_item, onclick: open, {t("item-carry-action-carry")} }
        }
        if perm.can("qry_carry_history") {
            Button { icon: icons::LIST, tone: Tone::Neutral, variant: ButtonVariant::Outlined, disabled: no_item, onclick: show_history, {t("item-carry-action-history")} }
        }
        if let Some(msg) = error() {
            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
        }
        if let Some(r) = result() {
            Card {
                div { class: "table-header",
                    h3 { {t("item-carry-result-title")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| result.set(None), {t("common-action-close")} }
                }
                table { class: "md-table",
                    thead { tr { th { {t("carry-setting-field-plant")} } th { {t("basic-carry-col-result")} } } }
                    tbody {
                        for row in r.rows.iter().cloned() {
                            tr { key: "{row.plant}",
                                td { class: "mono", "{row.plant}" }
                                td { {match &row.error { Some(e) => tf("basic-carry-failed", &[("reason", e.as_str())]), None => outcome_label(&row.outcome) }} }
                            }
                        }
                    }
                }
                for w in r.mail_warnings.iter() {
                    p { class: "query-hint", {tf("basic-carry-mail-failed", &[("detail", w.as_str())])} }
                }
            }
        }
        if let Some(i) = info() {
            Dialog {
                title: tf("item-carry-dialog-title", &[("center", i.center_code.as_str())]),
                oncancel: move |_| info.set(None),
                actions: rsx! {
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| info.set(None), {t("common-action-cancel")} }
                    Button { icon: icons::CHECK, tone: Tone::Success, disabled: selected.read().is_empty(), onclick: run, {t("basic-carry-action-carry")} }
                },
                p { class: "muted", {t(&format!("carry-setting-mode-{}", i.mode))} }
                if i.mode == "2" {
                    p { {t("item-carry-confirm-body")} }
                }
                table { class: "md-table",
                    tbody {
                        for p in i.targets.iter().cloned() {
                            tr { key: "{p}",
                                td {
                                    input {
                                        r#type: "checkbox",
                                        disabled: i.mode != "3",
                                        checked: selected.read().contains(&p),
                                        onchange: {
                                            let p = p.clone();
                                            move |_| {
                                                let mut s = selected.write();
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
            }
        }
        if let Some(h) = history() {
            Dialog {
                title: t("item-carry-history-title"),
                oncancel: move |_| history.set(None),
                actions: rsx! {
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| history.set(None), {t("common-action-close")} }
                },
                table { class: "md-table",
                    thead {
                        tr {
                            th { {t("carry-history-col-plant")} }
                            th { {t("carry-history-col-program")} }
                            th { {t("carry-history-col-key")} }
                            th { {t("carry-history-col-action")} }
                            th { {t("carry-history-col-at")} }
                            th { {t("carry-history-col-by")} }
                        }
                    }
                    tbody {
                        if h.is_empty() {
                            tr { td { colspan: 6, class: "muted", {t("common-msg-no-data")} } }
                        }
                        for (i, l) in h.iter().cloned().enumerate() {
                            tr { key: "{i}",
                                td { class: "mono", "{l.plant}" }
                                td { class: "mono", "{l.program}" }
                                td { class: "mono", "{l.record_key}" }
                                td { {t(&format!("carry-history-action-{}", l.action))} }
                                td { {crate::components::history::local_time(l.carried_at)} }
                                td { "{l.carried_by}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
