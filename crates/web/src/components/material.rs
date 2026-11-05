//! Material 3 元件的 Dioxus 實作，樣式在 assets/material/m3.css（類別前綴 `md-`）。
//! 畫面只用這裡的元件，不直接寫 `md-` 類別。
//!
//! `readonly`：瀏覽模式（可選取複製、不可修改）。
//! `disabled`：依規則暫時不可輸入（例如非總帳模組的傳遞帳別）。

use dioxus::prelude::*;

/// M3 Common buttons
#[derive(Clone, Copy, PartialEq, Default)]
pub enum ButtonVariant {
    /// 主要動作，一個畫面原則上只有一個
    #[default]
    Filled,
    /// 次要但需要強調的動作
    Tonal,
    Outlined,
    Text,
    /// 刪除等破壞性動作
    Error,
}

impl ButtonVariant {
    fn class(self) -> &'static str {
        match self {
            Self::Filled => "md-button--filled",
            Self::Tonal => "md-button--tonal",
            Self::Outlined => "md-button--outlined",
            Self::Text => "md-button--text",
            Self::Error => "md-button--error",
        }
    }
}

#[component]
pub fn Button(
    #[props(default)] variant: ButtonVariant,
    #[props(default)] disabled: bool,
    onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "md-button md-state {variant.class()}",
            disabled,
            onclick: move |e| onclick.call(e),
            {children}
        }
    }
}

fn field_class(readonly: bool, disabled: bool, extra: &str) -> String {
    let mut c = String::from("md-field");
    if readonly {
        c.push_str(" md-field--readonly");
    }
    if disabled {
        c.push_str(" md-field--disabled");
    }
    if !extra.is_empty() {
        c.push(' ');
        c.push_str(extra);
    }
    c
}

/// Outlined text field（標籤固定浮在框線上）
#[component]
pub fn TextField(
    id: String,
    label: String,
    value: String,
    #[props(default)] readonly: bool,
    #[props(default)] disabled: bool,
    #[props(default)] placeholder: String,
    #[props(default = "text".to_string())] input_type: String,
    #[props(default)] class: String,
    /// 長度上限（字元），對應 TipTop 資料表定義
    #[props(default)]
    maxlength: Option<usize>,
    #[props(default)] oninput: EventHandler<String>,
    /// 離開欄位時（例如登入畫面輸入完使用者代號後載入營運中心選項）
    #[props(default)]
    onblur: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: field_class(readonly, disabled, &class),
            label { r#for: "{id}", class: "md-field__label", "{label}" }
            input {
                id: "{id}",
                class: "md-field__input",
                r#type: "{input_type}",
                value: "{value}",
                placeholder: "{placeholder}",
                maxlength: maxlength.map(|n| n.to_string()),
                readonly,
                disabled,
                oninput: move |e| oninput.call(e.value()),
                onblur: move |_| onblur.call(()),
            }
        }
    }
}

#[component]
pub fn TextArea(
    id: String,
    label: String,
    value: String,
    #[props(default = 4)] rows: i64,
    #[props(default)] readonly: bool,
    #[props(default)] maxlength: Option<usize>,
    #[props(default)] placeholder: String,
    #[props(default)] oninput: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: field_class(readonly, false, ""),
            label { r#for: "{id}", class: "md-field__label", "{label}" }
            textarea {
                id: "{id}",
                class: "md-field__input",
                rows,
                readonly,
                maxlength: maxlength.map(|n| n.to_string()),
                placeholder: "{placeholder}",
                value: "{value}",
                oninput: move |e| oninput.call(e.value()),
            }
        }
    }
}

/// Outlined select；`options` 為 (值, 顯示文字)
#[component]
pub fn Select(
    id: String,
    label: String,
    options: Vec<(String, String)>,
    value: String,
    #[props(default)] readonly: bool,
    #[props(default)] disabled: bool,
    #[props(default)] onchange: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: field_class(readonly, disabled, ""),
            label { r#for: "{id}", class: "md-field__label", "{label}" }
            select {
                id: "{id}",
                class: "md-field__input",
                // 原生 select 沒有 readonly；瀏覽模式改用 disabled，外觀由 md-field--readonly 控制
                disabled: disabled || readonly,
                onchange: move |e| onchange.call(e.value()),
                for (v, text) in options.iter() {
                    option { key: "{v}", value: "{v}", selected: *v == value, "{text}" }
                }
            }
            svg {
                class: "md-field__trailing",
                view_box: "0 0 24 24",
                fill: "currentColor",
                path { d: "M7 10l5 5 5-5z" }
            }
        }
    }
}

#[component]
pub fn Checkbox(
    id: String,
    label: String,
    checked: bool,
    #[props(default)] readonly: bool,
    #[props(default)] disabled: bool,
    #[props(default)] onchange: EventHandler<bool>,
) -> Element {
    rsx! {
        label {
            r#for: "{id}",
            class: "md-check",
            class: if readonly { "md-check--readonly" },
            class: if disabled { "md-check--disabled" },
            input {
                id: "{id}",
                r#type: "checkbox",
                checked,
                disabled,
                onclick: move |e| {
                    // checkbox 沒有原生 readonly：瀏覽模式擋下點擊
                    if readonly {
                        e.prevent_default();
                    }
                },
                onchange: move |e| {
                    if !readonly {
                        onchange.call(e.checked());
                    }
                },
            }
            "{label}"
        }
    }
}

/// 單選群組（垂直排列）；`options` 為 (值, 顯示文字)
#[component]
pub fn RadioGroup(
    name: String,
    legend: String,
    options: Vec<(String, String)>,
    value: Option<String>,
    #[props(default)] readonly: bool,
    #[props(default)] disabled: bool,
    #[props(default)] onchange: EventHandler<String>,
) -> Element {
    rsx! {
        fieldset { class: "md-radio-group", disabled,
            legend { "{legend}" }
            for (v, text) in options.iter().cloned() {
                label {
                    key: "{v}",
                    class: "md-check",
                    class: if readonly { "md-check--readonly" },
                    class: if disabled { "md-check--disabled" },
                    input {
                        r#type: "radio",
                        name: "{name}",
                        checked: value.as_deref() == Some(v.as_str()),
                        onclick: move |e| {
                            if readonly {
                                e.prevent_default();
                            }
                        },
                        onchange: {
                            let v = v.clone();
                            move |_| {
                                if !readonly {
                                    onchange.call(v.clone());
                                }
                            }
                        },
                    }
                    "{text}"
                }
            }
        }
    }
}

/// Primary tabs（4fd 的 PageControl）；內容由呼叫端依 `active` 自行切換
#[component]
pub fn Tabs(tabs: Vec<String>, active: Signal<usize>) -> Element {
    rsx! {
        div { class: "md-tabs", role: "tablist",
            for (i, t) in tabs.iter().enumerate() {
                button {
                    key: "{i}",
                    r#type: "button",
                    role: "tab",
                    aria_selected: active() == i,
                    class: "md-tab md-state",
                    class: if active() == i { "md-tab--active" },
                    onclick: move |_| active.set(i),
                    span { class: "md-tab__label", "{t}" }
                }
            }
        }
    }
}

/// 卡片容器（outlined card）
#[component]
pub fn Card(children: Element) -> Element {
    rsx! {
        div { class: "md-card", {children} }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum NotificationKind {
    Error,
    Success,
}

/// 提示橫幅（M3 container 色）
#[component]
pub fn Notification(kind: NotificationKind, title: String, onclose: EventHandler<()>) -> Element {
    let (class, role, icon) = match kind {
        NotificationKind::Error => (
            "md-banner--error",
            "alert",
            "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z",
        ),
        NotificationKind::Success => (
            "md-banner--success",
            "status",
            "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm-2 15l-5-5 1.4-1.4 3.6 3.6 7.6-7.6L19 8l-9 9z",
        ),
    };
    rsx! {
        div { class: "md-banner {class}", role,
            svg { width: "24", height: "24", view_box: "0 0 24 24", fill: "currentColor",
                path { d: icon }
            }
            span { class: "md-banner__text", "{title}" }
            button {
                r#type: "button",
                class: "md-icon-button md-state",
                aria_label: "關閉",
                onclick: move |_| onclose.call(()),
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "currentColor",
                    path { d: "M19 6.4L17.6 5 12 10.6 6.4 5 5 6.4 10.6 12 5 17.6 6.4 19 12 13.4 17.6 19 19 17.6 13.4 12z" }
                }
            }
        }
    }
}

/// 圖示按鈕（Standard icon button）；`path` 為 24×24 的 SVG 路徑
#[component]
pub fn IconButton(
    label: String,
    path: &'static str,
    #[props(default)] disabled: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "md-icon-button md-state",
            title: "{label}",
            aria_label: "{label}",
            disabled,
            onclick: move |e| onclick.call(e),
            svg { width: "24", height: "24", view_box: "0 0 24 24", fill: "currentColor",
                path { d: path }
            }
        }
    }
}

/// 標記（M3 assist chip 的靜態樣式）；`warning` 用於待確認等需注意的標記
#[component]
pub fn Chip(label: String, #[props(default)] warning: bool) -> Element {
    rsx! {
        span { class: if warning { "md-chip md-chip--small md-chip--warning" } else { "md-chip md-chip--small" }, "{label}" }
    }
}

/// 對話框（M3 Basic dialog）：遮罩＋標題＋內容＋動作列。點遮罩或按 Esc 視同取消。
/// 由呼叫端決定是否顯示（`if open() { Dialog { … } }`）。
#[component]
pub fn Dialog(
    title: String,
    oncancel: EventHandler<()>,
    /// 右下角動作按鈕（通常為「放棄」「確定」）
    actions: Element,
    children: Element,
) -> Element {
    rsx! {
        div { class: "md-dialog-scrim", onclick: move |_| oncancel.call(()) }
        div {
            class: "md-dialog",
            role: "dialog",
            aria_modal: "true",
            aria_label: "{title}",
            tabindex: "-1",
            onkeydown: move |e: KeyboardEvent| {
                if e.key() == Key::Escape {
                    oncancel.call(());
                }
            },
            h2 { class: "md-dialog__headline", "{title}" }
            div { class: "md-dialog__content", {children} }
            div { class: "md-dialog__actions", {actions} }
        }
    }
}

/// 圖示按鈕＋下拉選單（M3 Menu）；`items` 為 (值, 顯示文字)，目前值打勾。
/// 點選項目或選單外即關閉。
#[component]
pub fn MenuButton(
    label: String,
    path: &'static str,
    items: Vec<(String, String)>,
    selected: String,
    onselect: EventHandler<String>,
) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        div { class: "md-menu-anchor",
            button {
                r#type: "button",
                class: "md-icon-button md-state",
                class: if open() { "md-icon-button--selected" },
                title: "{label}",
                aria_label: "{label}",
                aria_haspopup: "menu",
                aria_expanded: open(),
                onclick: move |_| open.toggle(),
                svg { width: "24", height: "24", view_box: "0 0 24 24", fill: "currentColor",
                    path { d: path }
                }
            }
            if open() {
                // 點選單外關閉
                div { class: "md-menu-scrim", onclick: move |_| open.set(false) }
                ul { class: "md-menu", role: "menu", aria_label: "{label}",
                    for (value, text) in items {
                        li {
                            key: "{value}",
                            role: "menuitemradio",
                            aria_checked: value == selected,
                            class: "md-menu__item md-state",
                            class: if value == selected { "md-menu__item--selected" },
                            onclick: move |_| {
                                open.set(false);
                                onselect.call(value.clone());
                            },
                            span { class: "md-menu__check",
                                if value == selected {
                                    svg { width: "18", height: "18", view_box: "0 0 24 24", fill: "currentColor",
                                        path { d: icons::CHECK }
                                    }
                                }
                            }
                            span { "{text}" }
                        }
                    }
                }
            }
        }
    }
}

/// Material Symbols 的路徑（Apache-2.0）
pub mod icons {
    pub const FIRST_PAGE: &str = "M18.41 16.59L13.82 12l4.59-4.59L17 6l-6 6 6 6zM6 6h2v12H6z";
    pub const CHEVRON_LEFT: &str = "M15.41 7.41L14 6l-6 6 6 6 1.41-1.41L10.83 12z";
    pub const CHEVRON_RIGHT: &str = "M10 6L8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z";
    pub const LAST_PAGE: &str = "M5.59 7.41L10.18 12l-4.59 4.59L7 18l6-6-6-6zM16 6h2v12h-2z";
    /// 翻譯（translate）：開啟多語言輸入視窗
    pub const TRANSLATE: &str = "M12.87 15.07l-2.54-2.51.03-.03c1.74-1.94 2.98-4.17 3.71-6.53H17V4h-7V2H8v2H1v1.99h11.17C11.5 7.92 10.44 9.75 9 11.35 8.07 10.32 7.3 9.19 6.69 8h-2c.73 1.63 1.73 3.17 2.98 4.56l-5.09 5.02L4 19l5-5 3.11 3.11.76-2.04zM18.5 10h-2L12 22h2l1.12-3h4.75L21 22h2l-4.5-12zm-2.62 7l1.62-4.33L19.12 17h-3.24z";
    /// 營運中心（Material Symbols business）
    pub const PLANT: &str = "M12 7V3H2v18h20V7H12zM6 19H4v-2h2v2zm0-4H4v-2h2v2zm0-4H4V9h2v2zm0-4H4V5h2v2zm4 12H8v-2h2v2zm0-4H8v-2h2v2zm0-4H8V9h2v2zm0-4H8V5h2v2zm10 12h-8v-2h2v-2h-2v-2h2v-2h-2V9h8v10zm-2-8h-2v2h2v-2zm0 4h-2v2h2v-2z";
    pub const CHECK: &str = "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z";
    /// 使用者（account_circle）：登入者選單
    pub const ACCOUNT: &str = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 4c1.93 0 3.5 1.57 3.5 3.5S13.93 13 12 13s-3.5-1.57-3.5-3.5S10.07 6 12 6zm0 14c-2.03 0-4.43-.82-6.14-2.88C7.55 15.8 9.68 15 12 15s4.45.8 6.14 2.12C16.43 19.18 14.03 20 12 20z";
    /// 鑰匙（key）：變更密碼
    pub const KEY: &str = "M12.65 10C11.83 7.67 9.61 6 7 6c-3.31 0-6 2.69-6 6s2.69 6 6 6c2.61 0 4.83-1.67 5.65-4H17v4h4v-4h2v-4H12.65zM7 14c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2z";
    /// 登出（logout）
    pub const LOGOUT: &str = "M17 7l-1.41 1.41L18.17 11H8v2h10.17l-2.58 2.58L17 17l5-5zM4 5h8V3H4c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h8v-2H4V5z";
    /// 地球（language）：切換介面語系
    pub const LANGUAGE: &str = "M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zm6.93 6h-2.95c-.32-1.25-.78-2.45-1.38-3.56 1.84.63 3.37 1.91 4.33 3.56zM12 4.04c.83 1.2 1.48 2.53 1.91 3.96h-3.82c.43-1.43 1.08-2.76 1.91-3.96zM4.26 14C4.1 13.36 4 12.69 4 12s.1-1.36.26-2h3.38c-.08.66-.14 1.32-.14 2s.06 1.34.14 2H4.26zm.82 2h2.95c.32 1.25.78 2.45 1.38 3.56-1.84-.63-3.37-1.9-4.33-3.56zm2.95-8H5.08c.96-1.66 2.49-2.93 4.33-3.56C8.81 5.55 8.35 6.75 8.03 8zM12 19.96c-.83-1.2-1.48-2.53-1.91-3.96h3.82c-.43 1.43-1.08 2.76-1.91 3.96zM14.34 14H9.66c-.09-.66-.16-1.32-.16-2s.07-1.35.16-2h4.68c.09.65.16 1.32.16 2s-.07 1.34-.16 2zm.25 5.56c.6-1.11 1.06-2.31 1.38-3.56h2.95c-.96 1.65-2.49 2.93-4.33 3.56zM16.36 14c.08-.66.14-1.32.14-2s-.06-1.34-.14-2h3.38c.16.64.26 1.31.26 2s-.1 1.36-.26 2h-3.38z";
}
