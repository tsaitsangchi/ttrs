//! aimi100 料件基本資料維護

use dioxus::prelude::*;
use ttrs_shared::{CreateItemRequest, ItemStatusDto};

use crate::api;

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
        h2 { "aimi100 料件基本資料" }

        div { class: "form-row",
            input { placeholder: "料件編號", value: "{item_no}", oninput: move |e| item_no.set(e.value()) }
            input { placeholder: "品名", value: "{name}", oninput: move |e| name.set(e.value()) }
            input { placeholder: "規格", value: "{spec}", oninput: move |e| spec.set(e.value()) }
            input { placeholder: "庫存單位", value: "{unit}", oninput: move |e| unit.set(e.value()) }
            button { onclick: create, "新增" }
        }

        if let Some(msg) = error() {
            p { class: "error", "{msg}" }
        }

        match &*items.read() {
            None => rsx! { p { "載入中…" } },
            Some(Err(e)) => rsx! { p { class: "error", "無法載入料件：{e}" } },
            Some(Ok(list)) if list.is_empty() => rsx! { p { "尚無料件資料" } },
            Some(Ok(list)) => rsx! {
                table {
                    thead {
                        tr {
                            th { "料件編號" }
                            th { "品名" }
                            th { "規格" }
                            th { "單位" }
                            th { "狀態" }
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
                                td { {it.status.label()} }
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
                                            "確認"
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
                                            "取消確認"
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
                                        "刪除"
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
