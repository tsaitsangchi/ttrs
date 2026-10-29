//! TTRS 前端入口。畫面依 TipTop 模組分資料夾：pages/aim（庫存/料件）、pages/azz（系統管理）…

mod api;
mod components;
mod pages;

use dioxus::prelude::*;
use pages::{aim::ItemList, azz::ProgramList, home::Home};

/// IBM Carbon（見 assets/carbon/README.md），必須在 main.css 之前載入
const CARBON_CSS: Asset = asset!("/assets/carbon/carbon-1.116.0.min.css");
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
        document::Link { rel: "stylesheet", href: CARBON_CSS }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
fn Shell() -> Element {
    rsx! {
        // Carbon 主題 Gray 10：頁面底色 #f4f4f4、面板白色
        div { class: "shell cds--g10",
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
