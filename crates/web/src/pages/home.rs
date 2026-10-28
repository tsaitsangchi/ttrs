use dioxus::prelude::*;

#[component]
pub fn Home() -> Element {
    rsx! {
        h2 { "歡迎使用 TTRS" }
        p { "TipTop ERP 移植版（Rust + PostgreSQL）。請由左側選單選擇作業。" }
    }
}
