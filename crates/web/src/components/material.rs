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
    #[props(default)] oninput: EventHandler<String>,
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
                readonly,
                disabled,
                oninput: move |e| oninput.call(e.value()),
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
pub fn Tabs(tabs: Vec<&'static str>, active: Signal<usize>) -> Element {
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

/// Material Symbols 的路徑（Apache-2.0）
pub mod icons {
    pub const FIRST_PAGE: &str = "M18.41 16.59L13.82 12l4.59-4.59L17 6l-6 6 6 6zM6 6h2v12H6z";
    pub const CHEVRON_LEFT: &str = "M15.41 7.41L14 6l-6 6 6 6 1.41-1.41L10.83 12z";
    pub const CHEVRON_RIGHT: &str = "M10 6L8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z";
    pub const LAST_PAGE: &str = "M5.59 7.41L10.18 12l-4.59 4.59L7 18l6-6-6-6zM16 6h2v12h-2z";
}
