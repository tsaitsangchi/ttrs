//! TTRS 前端入口。畫面依 TipTop 模組分資料夾：pages/aim（庫存/料件）、pages/azz（系統管理）…

mod api;
mod components;
mod pages;

use dioxus::prelude::*;
use pages::{aim::ItemList, azz::ProgramList, home::Home};

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
        /// p_zz 程式登錄
        #[route("/azz/programs")]
        ProgramList {},
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: M3_COLORS_CSS }
        document::Link { rel: "stylesheet", href: M3_CSS }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
fn Shell() -> Element {
    rsx! {
        // M3 淺色主題；深色改為 md-theme-dark
        div { class: "shell md-theme-light",
            nav { class: "sidebar",
                h1 { "TTRS ERP" }
                Link { active_class: "active", to: Route::Home {}, "首頁" }
                p { class: "group", "庫存管理 (aim)" }
                Link { active_class: "active", to: Route::ItemList {}, "aimi100 料件基本資料" }
                p { class: "group", "系統管理 (azz)" }
                Link { active_class: "active", to: Route::ProgramList {}, "p_zz 程式資料建立" }
            }
            main { class: "content", Outlet::<Route> {} }
        }
    }
}
