use dioxus::prelude::*;

#[component]
pub fn Home() -> Element {
    rsx! {
        // 尚未改用 Carbon 元件，沿用舊樣式（main.css .classic）
        div { class: "classic",
        h2 { "歡迎使用 TTRS" }
        p { "TipTop ERP 移植版（Rust + PostgreSQL）。請由左側選單選擇作業。" }
        }
    }
}
