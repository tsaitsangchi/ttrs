//! 營運中心選擇（工廠層作業用，docs/adr/0007-multi-plant.md）：選項來自營運中心維護的有效營運中心。
//! 已登入時一律用登入（或由使用者選單切換）的營運中心，畫面不再顯示營運中心欄位；
//! 只有固定身分（TTRS_AUTH=fixed）且未註冊、沒有登入營運中心時，才在畫面上自行選擇。

use dioxus::prelude::*;

use crate::api;
use crate::components::material::Select;
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

/// 工廠層作業的營運中心：已登入時同步為目前營運中心、不顯示；沒有登入營運中心時顯示下拉選單，
/// 尚未選擇時自動選第一個有效的營運中心
#[component]
pub fn PlantPicker(plant: Signal<String>) -> Element {
    let session = crate::components::session::use_session();
    let current = session()
        .session()
        .map(|s| (s.plant.clone(), format!("{} {}", s.plant, s.plant_name)));
    use_effect(move || {
        if let Some(s) = session().session()
            && *plant.peek() != s.plant
        {
            plant.set(s.plant.clone());
        }
    });
    if current.is_some() {
        return rsx! {};
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
        div { class: "form-grid",
            Select {
                id: "plant",
                label: t("common-field-plant"),
                options: plant_options(&list, &plant()),
                value: plant(),
                onchange: move |x| plant.set(x),
            }
        }
    }
}
