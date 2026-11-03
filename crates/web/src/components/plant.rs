//! 營運中心選擇（工廠層作業用，docs/adr/0007-multi-plant.md）：選項來自營運中心維護的有效營運中心。
//! 已登入時一律用目前營運中心（sign_in.feature，由外框切換）；尚未登入（開發用身分未註冊）時在畫面上自行選擇。

use dioxus::prelude::*;

use crate::api;
use crate::components::material::{Select, TextField};
use crate::i18n::t;

/// 有效營運中心的下拉選項（代碼 名稱）；`current` 不在清單時仍列出
pub fn plant_options(plants: &[ttrs_shared::PlantDto], current: &str) -> Vec<(String, String)> {
    let mut opts: Vec<(String, String)> = plants
        .iter()
        .filter(|p| p.active || p.plant_id == current)
        .map(|p| (p.plant_id.clone(), format!("{} {}", p.plant_id, p.name)))
        .collect();
    if !current.is_empty() && !opts.iter().any(|(k, _)| k == current) {
        opts.push((current.to_owned(), current.to_owned()));
    }
    if current.is_empty() {
        opts.insert(0, (String::new(), String::new()));
    }
    opts
}

/// 營運中心下拉選單；尚未選擇時自動選第一個有效的營運中心
#[component]
pub fn PlantPicker(plant: Signal<String>) -> Element {
    let session = use_context::<Signal<Option<Result<ttrs_shared::SessionDto, String>>>>();
    let current = match session() {
        Some(Ok(s)) => Some((s.plant.clone(), format!("{} {}", s.plant, s.plant_name))),
        _ => None,
    };
    use_effect(move || {
        if let Some(Ok(s)) = session()
            && *plant.peek() != s.plant
        {
            plant.set(s.plant.clone());
        }
    });
    if let Some((_, label)) = current {
        return rsx! {
            TextField { id: "plant", label: t("common-field-plant"), readonly: true, value: label }
        };
    }
    let plants = use_resource(|| async { api::list_plants().await.unwrap_or_default() });
    use_effect(move || {
        if plant.peek().is_empty()
            && let Some(first) = plants
                .read()
                .as_ref()
                .and_then(|ps| ps.iter().find(|p| p.active))
        {
            plant.set(first.plant_id.clone());
        }
    });
    let list = plants.read().clone().unwrap_or_default();
    rsx! {
        Select {
            id: "plant",
            label: t("common-field-plant"),
            options: plant_options(&list, &plant()),
            value: plant(),
            onchange: move |x| plant.set(x),
        }
    }
}
