//! aimi100 料件基本資料維護

use dioxus::prelude::*;
use ttrs_shared::{CreateItemRequest, ItemStatusDto};

use crate::api;
use crate::i18n::{t, tf};

// TODO: 登入後改由使用者所屬營運中心帶入
const PLANT: &str = "TW01";

#[component]
pub fn ItemList() -> Element {
    let mut items = use_resource(|| api::list_items(PLANT));
    let mut error = use_signal(|| None::<String>);

    let mut item_no = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut spec = use_signal(String::new);
    let mut unit = use_signal(|| "PCS".to_string());

    let create = move |_| {
        spawn(async move {
            let req = CreateItemRequest {
                plant_id: PLANT.into(),
                item_no: item_no(),
                name: name(),
                spec: Some(spec()).filter(|s| !s.trim().is_empty()),
                stock_unit: unit(),
            };
            match api::create_item(&req).await {
                Ok(_) => {
                    error.set(None);
                    item_no.set(String::new());
                    name.set(String::new());
                    spec.set(String::new());
                    items.restart();
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    rsx! {
        // 尚未改用 Carbon 元件，沿用舊樣式（main.css .classic）
        div { class: "classic",
        h2 { {t("item-title")} }

        div { class: "form-row",
            input { placeholder: t("item-field-item-no"), value: "{item_no}", oninput: move |e| item_no.set(e.value()) }
            input { placeholder: t("item-field-name"), value: "{name}", oninput: move |e| name.set(e.value()) }
            input { placeholder: t("item-field-spec"), value: "{spec}", oninput: move |e| spec.set(e.value()) }
            input { placeholder: t("item-field-stock-unit"), value: "{unit}", oninput: move |e| unit.set(e.value()) }
            button { onclick: create, {t("common-action-new")} }
        }

        if let Some(msg) = error() {
            p { class: "error", "{msg}" }
        }

        match &*items.read() {
            None => rsx! { p { {t("common-msg-loading")} } },
            Some(Err(e)) => rsx! { p { class: "error", {tf("item-msg-load-failed", &[("error", e.as_str())])} } },
            Some(Ok(list)) if list.is_empty() => rsx! { p { {t("item-msg-empty")} } },
            Some(Ok(list)) => rsx! {
                table {
                    thead {
                        tr {
                            th { {t("item-field-item-no")} }
                            th { {t("item-field-name")} }
                            th { {t("item-field-spec")} }
                            th { {t("item-col-unit")} }
                            th { {t("item-col-status")} }
                            th { "" }
                        }
                    }
                    tbody {
                        for it in list.iter().cloned() {
                            tr { key: "{it.item_no}",
                                td { "{it.item_no}" }
                                td { "{it.name}" }
                                td { {it.spec.clone().unwrap_or_default()} }
                                td { "{it.stock_unit}" }
                                td { {t(status_id(it.status))} }
                                td {
                                    if it.status == ItemStatusDto::Unconfirmed {
                                        button {
                                            onclick: {
                                                let no = it.item_no.clone();
                                                move |_| {
                                                    let no = no.clone();
                                                    spawn(async move {
                                                        match api::confirm_item(PLANT, &no).await {
                                                            Ok(_) => { error.set(None); items.restart(); }
                                                            Err(e) => error.set(Some(e)),
                                                        }
                                                    });
                                                }
                                            },
                                            {t("item-action-confirm")}
                                        }
                                    }
                                    if it.status == ItemStatusDto::Confirmed {
                                        button {
                                            onclick: {
                                                let no = it.item_no.clone();
                                                move |_| {
                                                    let no = no.clone();
                                                    spawn(async move {
                                                        match api::unconfirm_item(PLANT, &no).await {
                                                            Ok(_) => { error.set(None); items.restart(); }
                                                            Err(e) => error.set(Some(e)),
                                                        }
                                                    });
                                                }
                                            },
                                            {t("item-action-unconfirm")}
                                        }
                                    }
                                    button {
                                        class: "danger",
                                        onclick: {
                                            let no = it.item_no.clone();
                                            move |_| {
                                                let no = no.clone();
                                                spawn(async move {
                                                    match api::delete_item(PLANT, &no).await {
                                                        Ok(()) => { error.set(None); items.restart(); }
                                                        Err(e) => error.set(Some(e)),
                                                    }
                                                });
                                            }
                                        },
                                        {t("common-action-delete")}
                                    }
                                }
                            }
                        }
                    }
                }
            },
        }
        }
    }
}

fn status_id(s: ItemStatusDto) -> &'static str {
    match s {
        ItemStatusDto::Unconfirmed => "item-status-unconfirmed",
        ItemStatusDto::Confirmed => "item-status-confirmed",
        ItemStatusDto::Voided => "item-status-voided",
    }
}
