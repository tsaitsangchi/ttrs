//! IBM Carbon 元件的 Dioxus 包裝：產生與 @carbon/react 相同的 HTML 結構與 `cds--` 類別，
//! 樣式全部來自 assets/carbon/carbon-*.min.css。畫面只用這裡的元件，不直接寫 `cds--` 類別。
//!
//! `readonly`：瀏覽模式（可選取複製、不可修改，Carbon 有專屬樣式）。
//! `disabled`：依規則暫時不可輸入（例如非總帳模組的傳遞帳別）。

use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Default)]
pub enum ButtonKind {
    #[default]
    Primary,
    Secondary,
    Tertiary,
    Danger,
}

impl ButtonKind {
    fn class(self) -> &'static str {
        match self {
            Self::Primary => "cds--btn--primary",
            Self::Secondary => "cds--btn--secondary",
            Self::Tertiary => "cds--btn--tertiary",
            Self::Danger => "cds--btn--danger",
        }
    }
}

#[component]
pub fn Button(
    #[props(default)] kind: ButtonKind,
    #[props(default)] disabled: bool,
    onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "cds--btn cds--btn--sm {kind.class()}",
            disabled,
            onclick: move |e| onclick.call(e),
            {children}
        }
    }
}

#[component]
pub fn TextInput(
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
        div {
            class: "cds--form-item cds--text-input-wrapper {class}",
            class: if readonly { "cds--text-input-wrapper--readonly" },
            div { class: "cds--text-input__label-wrapper",
                label {
                    r#for: "{id}",
                    class: "cds--label",
                    class: if disabled { "cds--label--disabled" },
                    "{label}"
                }
            }
            div { class: "cds--text-input__field-outer-wrapper",
                div { class: "cds--text-input__field-wrapper",
                    input {
                        id: "{id}",
                        class: "cds--text-input",
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
        div { class: "cds--form-item",
            div { class: "cds--text-area__label-wrapper",
                label { r#for: "{id}", class: "cds--label", "{label}" }
            }
            div {
                class: "cds--text-area__wrapper",
                class: if readonly { "cds--text-area__wrapper--readonly" },
                textarea {
                    id: "{id}",
                    class: "cds--text-area",
                    rows,
                    readonly,
                    value: "{value}",
                    oninput: move |e| oninput.call(e.value()),
                }
            }
        }
    }
}

/// 下拉選單；`options` 為 (值, 顯示文字)
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
        div { class: "cds--form-item",
            div {
                class: "cds--select",
                class: if readonly { "cds--select--readonly" },
                class: if disabled { "cds--select--disabled" },
                label {
                    r#for: "{id}",
                    class: "cds--label",
                    class: if disabled { "cds--label--disabled" },
                    "{label}"
                }
                div { class: "cds--select-input__wrapper",
                    select {
                        id: "{id}",
                        class: "cds--select-input",
                        // 原生 select 沒有 readonly；瀏覽模式改用 disabled，外觀由 cds--select--readonly 控制
                        disabled: disabled || readonly,
                        onchange: move |e| onchange.call(e.value()),
                        for (v, text) in options.iter() {
                            option { key: "{v}", value: "{v}", selected: *v == value, "{text}" }
                        }
                    }
                    svg {
                        class: "cds--select__arrow",
                        width: "16",
                        height: "16",
                        view_box: "0 0 16 16",
                        fill: "currentColor",
                        path { d: "M8 11L3 6 3.7 5.3 8 9.6 12.3 5.3 13 6z" }
                    }
                }
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
        div {
            class: "cds--form-item cds--checkbox-wrapper",
            class: if readonly { "cds--checkbox-wrapper--readonly" },
            input {
                id: "{id}",
                r#type: "checkbox",
                class: "cds--checkbox",
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
            label { r#for: "{id}", class: "cds--checkbox-label",
                span { class: "cds--checkbox-label-text", "{label}" }
            }
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
        fieldset {
            class: "cds--fieldset",
            disabled,
            legend { class: "cds--label", "{legend}" }
            div {
                class: "cds--radio-button-group cds--radio-button-group--vertical",
                class: if readonly { "cds--radio-button-group--readonly" },
                for (v, text) in options.iter().cloned() {
                    div { key: "{v}", class: "cds--radio-button-wrapper",
                        input {
                            id: "{name}-{v}",
                            r#type: "radio",
                            class: "cds--radio-button",
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
                        label { r#for: "{name}-{v}", class: "cds--radio-button__label",
                            span { class: "cds--radio-button__appearance" }
                            span { class: "cds--radio-button__label-text", "{text}" }
                        }
                    }
                }
            }
        }
    }
}

/// Contained 頁籤（4fd 的 PageControl）；內容由呼叫端依 `active` 自行切換
#[component]
pub fn Tabs(tabs: Vec<&'static str>, active: Signal<usize>) -> Element {
    rsx! {
        div { class: "cds--tabs cds--tabs--contained",
            div { class: "cds--tab--list", role: "tablist",
                for (i, t) in tabs.iter().enumerate() {
                    button {
                        key: "{i}",
                        r#type: "button",
                        role: "tab",
                        aria_selected: active() == i,
                        class: "cds--tabs__nav-item cds--tabs__nav-link",
                        class: if active() == i { "cds--tabs__nav-item--selected" },
                        onclick: move |_| active.set(i),
                        div { class: "cds--tabs__nav-item-label-wrapper",
                            span { class: "cds--tabs__nav-item-label", "{t}" }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum NotificationKind {
    Error,
    Success,
}

/// Inline notification（low contrast）
#[component]
pub fn Notification(kind: NotificationKind, title: String, onclose: EventHandler<()>) -> Element {
    let (class, role, icon) = match kind {
        NotificationKind::Error => (
            "cds--inline-notification--error",
            "alert",
            "M10 1c-5 0-9 4-9 9s4 9 9 9 9-4 9-9-4-9-9-9zm3.5 13.5l-8-8 1-1 8 8-1 1z",
        ),
        NotificationKind::Success => (
            "cds--inline-notification--success",
            "status",
            "M10 1c-4.9 0-9 4.1-9 9s4.1 9 9 9 9-4 9-9-4-9-9-9zM8.7 13.5l-3.2-3.2 1-1 2.2 2.2 4.8-4.8 1 1-5.8 5.8z",
        ),
    };
    rsx! {
        div {
            class: "cds--inline-notification cds--inline-notification--low-contrast {class}",
            role,
            div { class: "cds--inline-notification__details",
                svg {
                    class: "cds--inline-notification__icon",
                    width: "20",
                    height: "20",
                    view_box: "0 0 20 20",
                    fill: "currentColor",
                    path { d: icon }
                }
                div { class: "cds--inline-notification__text-wrapper",
                    div { class: "cds--inline-notification__title", "{title}" }
                }
            }
            button {
                r#type: "button",
                class: "cds--inline-notification__close-button",
                aria_label: "關閉",
                onclick: move |_| onclose.call(()),
                svg {
                    class: "cds--inline-notification__close-icon",
                    width: "16",
                    height: "16",
                    view_box: "0 0 32 32",
                    fill: "currentColor",
                    path { d: "M17.4 16l6.3-6.3-1.4-1.4-6.3 6.3-6.3-6.3-1.4 1.4 6.3 6.3-6.3 6.3 1.4 1.4 6.3-6.3 6.3 6.3 1.4-1.4z" }
                }
            }
        }
    }
}
