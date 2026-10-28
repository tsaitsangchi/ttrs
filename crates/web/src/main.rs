//! TTRS 前端入口。畫面依 TipTop 模組分資料夾：pages/aim（庫存/料件）、pages/axm（銷售）…

mod api;
mod pages;

use dioxus::prelude::*;
use pages::{aim::ItemList, home::Home};

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
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
fn Shell() -> Element {
    rsx! {
        div { class: "shell",
            nav { class: "sidebar",
                h1 { "TTRS ERP" }
                Link { to: Route::Home {}, "首頁" }
                p { class: "group", "庫存管理 (aim)" }
                Link { to: Route::ItemList {}, "aimi100 料件基本資料" }
            }
            main { class: "content", Outlet::<Route> {} }
        }
    }
}
