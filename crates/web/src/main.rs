//! TTRS 前端入口。畫面依 TipTop 模組分資料夾：pages/aim（庫存/料件）、pages/azz（系統管理）…

mod api;
mod components;
mod i18n;
mod pages;

use components::material::{MenuButton, icons};
use dioxus::prelude::*;
use i18n::{UiLang, t};
use pages::{
    aim::ItemList,
    azz::{DataDictionary, ProgramMaintenance},
    home::Home,
};

/// Material 3：色彩 token（自動產生）→ 元件樣式 → 版面，順序不可調換
const M3_COLORS_CSS: Asset = asset!("/assets/material/m3-colors.css");
const M3_CSS: Asset = asset!("/assets/material/m3.css");
const MAIN_CSS: Asset = asset!("/assets/main.css");

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Shell)]
        #[route("/")]
        Home {},
        /// aimi100 料件基本資料
        #[route("/aim/items")]
        ItemList {},
        /// Program Maintenance 程式維護（TipTop p_zz）
        #[route("/azz/program-maintenance")]
        ProgramMaintenance {},
        /// Data Dictionary 資料字典維護（TipTop p_zta）
        #[route("/azz/data-dictionary")]
        DataDictionary {},
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // 介面語系（L3）：啟動時讀取瀏覽器記住的語系；供外框切換
    use_context_provider(|| Signal::new(i18n::load()));
    rsx! {
        document::Link { rel: "stylesheet", href: M3_COLORS_CSS }
        document::Link { rel: "stylesheet", href: M3_CSS }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
fn Shell() -> Element {
    let mut lang = use_context::<Signal<UiLang>>();
    let current = lang();
    rsx! {
        // M3 淺色主題；深色改為 md-theme-dark
        div { class: "shell md-theme-light", lang: current.code(),
            nav { class: "sidebar",
                div { class: "sidebar-header",
                    // 介面語系（L3）：最左上方的地球圖示，點開選單切換
                    MenuButton {
                        label: t("shell-language"),
                        path: icons::LANGUAGE,
                        items: UiLang::ALL.iter().map(|l| (l.code().to_owned(), l.native_name().to_owned())).collect::<Vec<_>>(),
                        selected: current.code().to_owned(),
                        onselect: move |code: String| {
                            if let Some(l) = UiLang::from_code(&code) {
                                i18n::set(l);
                                lang.set(l);
                            }
                        },
                    }
                    h1 { "TTRS ERP" }
                }
                Link { active_class: "active", to: Route::Home {}, {t("shell-nav-home")} }
                p { class: "group", {t("shell-group-aim")} }
                Link { active_class: "active", to: Route::ItemList {}, {t("shell-nav-aimi100")} }
                p { class: "group", {t("shell-group-azz")} }
                Link { active_class: "active", to: Route::ProgramMaintenance {}, title: t("program-title"), "Program Maintenance" }
                Link { active_class: "active", to: Route::DataDictionary {}, title: t("shell-nav-dictionary"), "Data Dictionary" }
            }
            // 以語系為 key：切換語系時重新載入畫面，所有文字改用新語系
            for l in [current] {
                main { key: "{l.code()}", class: "content", Outlet::<Route> {} }
            }
        }
    }
}
