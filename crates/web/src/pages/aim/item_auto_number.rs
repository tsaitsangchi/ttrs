//! 料件自動編號（item_auto_number.feature AN2、AN3）：料件新增、複製時依編碼類別取料號
//! 參數 aza28 未開啟或沒有新增／複製權限時不顯示。取號不保留號碼（AN5），帶入後仍可修改。

use dioxus::prelude::*;
use ttrs_shared::ItemNumberRequest;

use crate::api;
use crate::components::material::{Button, ButtonVariant, Select, Tone, icons};
use crate::i18n::t;

/// `onpicked` 收到（料號, 名稱）
#[component]
pub fn ItemAutoNumber(plant: String, onpicked: EventHandler<(String, String)>) -> Element {
    let options = use_resource({
        let plant = plant.clone();
        move || {
            let plant = plant.clone();
            async move { api::item_number_options(&plant).await }
        }
    });
    let mut class = use_signal(String::new);
    let mut picks = use_signal(Vec::<String>::new);
    let mut error = use_signal(|| None::<String>);
    let Some(Ok(o)) = options.read().clone() else {
        return rsx! {};
    };
    if !o.enabled || o.classes.is_empty() {
        return rsx! {};
    }
    let cur = o
        .classes
        .iter()
        .find(|c| c.class_code == class())
        .cloned()
        .or_else(|| o.classes.first().cloned());
    let class_options: Vec<(String, String)> = o
        .classes
        .iter()
        .map(|c| {
            (
                c.class_code.clone(),
                format!("{} {} ({})", c.class_code, c.name, c.nature_code),
            )
        })
        .collect();
    let take = {
        let plant = plant.clone();
        let code = cur
            .as_ref()
            .map(|c| c.class_code.clone())
            .unwrap_or_default();
        move |_| {
            let req = ItemNumberRequest {
                class_code: code.clone(),
                selections: picks(),
            };
            let plant = plant.clone();
            spawn(async move {
                match api::next_item_number(&plant, &req).await {
                    Ok(r) => {
                        error.set(None);
                        onpicked.call((r.number, r.name));
                    }
                    Err(e) => error.set(Some(e)),
                }
            });
        }
    };
    rsx! {
        div { class: "action-bar",
            Select {
                id: "auto-class",
                label: t("item-auto-number-class"),
                options: class_options,
                value: cur.as_ref().map(|c| c.class_code.clone()).unwrap_or_default(),
                onchange: move |v| { class.set(v); picks.set(Vec::new()); },
            }
            if let Some(c) = cur.clone() {
                for (i, s) in c.segments.iter().cloned().enumerate() {
                    Select {
                        key: "{c.class_code}-{s.seq_no}",
                        id: "auto-pick-{s.seq_no}",
                        label: s.description.clone().unwrap_or_else(|| s.segment_code.clone()),
                        options: std::iter::once((String::new(), String::new()))
                            .chain(s.values.iter().map(|(v, n)| (v.clone(), format!("{v} {}", n.clone().unwrap_or_default()))))
                            .collect::<Vec<_>>(),
                        value: picks.read().get(i).cloned().unwrap_or_default(),
                        onchange: move |v: String| {
                            let mut p = picks.write();
                            if p.len() <= i { p.resize(i + 1, String::new()); }
                            p[i] = v;
                        },
                    }
                }
            }
            Button { icon: icons::ADD, tone: Tone::Primary, variant: ButtonVariant::Tonal, onclick: take, {t("item-auto-number-take")} }
        }
        if let Some(e) = error() {
            p { class: "query-hint", "{e}" }
        }
    }
}
