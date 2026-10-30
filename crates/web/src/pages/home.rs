use dioxus::prelude::*;

use crate::i18n::t;

#[component]
pub fn Home() -> Element {
    rsx! {
        // 尚未改用 Carbon 元件，沿用舊樣式（main.css .classic）
        div { class: "classic",
        h2 { {t("home-title")} }
        p { {t("home-intro")} }
        }
    }
}
