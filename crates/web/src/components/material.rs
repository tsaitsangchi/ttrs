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
    /// 前置圖示（icons 的路徑），依功能配對
    #[props(default)]
    icon: Option<&'static str>,
    /// 圖示顏色；實心（filled、error）按鈕上維持與文字同色以保持對比
    #[props(default)]
    tone: Tone,
    onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "md-button md-state {variant.class()}",
            class: if icon.is_some() { "md-button--with-icon" },
            disabled,
            onclick: move |e| onclick.call(e),
            if let Some(d) = icon {
                svg {
                    class: "md-button__icon {tone.icon_class()}",
                    width: "18",
                    height: "18",
                    view_box: "0 0 24 24",
                    fill: "currentColor",
                    "aria-hidden": "true",
                    path { d }
                }
            }
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
    /// 前置圖示（icons 的路徑），以 `tone` 著色
    #[props(default)]
    leading_icon: Option<&'static str>,
    #[props(default)] tone: Tone,
    /// 後置動作（例如顯示／隱藏密碼的圖示按鈕）
    #[props(default)]
    trailing: Option<Element>,
) -> Element {
    let mut cls = field_class(readonly, disabled, &class);
    if leading_icon.is_some() {
        cls.push_str(" md-field--leading");
    }
    if trailing.is_some() {
        cls.push_str(" md-field--trailing-action");
    }
    rsx! {
        div { class: cls,
            label { r#for: "{id}", class: "md-field__label", "{label}" }
            if let Some(d) = leading_icon {
                svg { class: "md-field__leading {tone.icon_class()}", view_box: "0 0 24 24", fill: "currentColor", "aria-hidden": "true",
                    path { d }
                }
            }
            if let Some(t) = trailing {
                div { class: "md-field__action", {t} }
            }
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
    /// 前置圖示（icons 的路徑），以 `tone` 著色
    #[props(default)]
    leading_icon: Option<&'static str>,
    #[props(default)] tone: Tone,
) -> Element {
    let mut cls = field_class(readonly, disabled, "");
    if leading_icon.is_some() {
        cls.push_str(" md-field--leading");
    }
    rsx! {
        div { class: cls,
            label { r#for: "{id}", class: "md-field__label", "{label}" }
            if let Some(d) = leading_icon {
                svg { class: "md-field__leading {tone.icon_class()}", view_box: "0 0 24 24", fill: "currentColor", "aria-hidden": "true",
                    path { d }
                }
            }
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
/// 單一選項鈕（例如表格每列的「設為預設」）；`label` 空白時以 `aria_label` 供螢幕閱讀器朗讀
#[component]
pub fn Radio(
    id: String,
    name: String,
    checked: bool,
    #[props(default)] label: String,
    #[props(default)] aria_label: String,
    #[props(default)] readonly: bool,
    #[props(default)] disabled: bool,
    #[props(default)] onselect: EventHandler<()>,
) -> Element {
    rsx! {
        label {
            class: "md-check",
            class: if readonly { "md-check--readonly" },
            class: if disabled { "md-check--disabled" },
            input {
                id: "{id}",
                r#type: "radio",
                name: "{name}",
                checked,
                disabled,
                aria_label: if aria_label.is_empty() { None } else { Some(aria_label.clone()) },
                onclick: move |e| {
                    if readonly {
                        e.prevent_default();
                    }
                },
                onchange: move |_| {
                    if !readonly {
                        onselect.call(());
                    }
                },
            }
            "{label}"
        }
    }
}

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
pub fn Chip(
    label: String,
    #[props(default)] warning: bool,
    /// 狀態色（例如資料有效＝綠、停用＝紅）；`warning` 為提醒用的第三色
    #[props(default)]
    tone: Tone,
) -> Element {
    let class = match (warning, tone) {
        (true, _) => "md-chip md-chip--small md-chip--warning",
        (false, Tone::Success) => "md-chip md-chip--small md-chip--success",
        (false, Tone::Error) => "md-chip md-chip--small md-chip--error",
        (false, Tone::Neutral | Tone::Primary | Tone::Tertiary) => "md-chip md-chip--small",
    };
    rsx! {
        span { class, "{label}" }
    }
}

/// 色調（只用 M3 色彩 token）：狀態標籤用 Success、Error；按鈕圖示另用 Primary、Tertiary
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Neutral,
    Primary,
    Tertiary,
    Success,
    Error,
}

impl Tone {
    pub fn icon_class(self) -> &'static str {
        match self {
            Self::Neutral => "md-icon--neutral",
            Self::Primary => "md-icon--primary",
            Self::Tertiary => "md-icon--tertiary",
            Self::Success => "md-icon--success",
            Self::Error => "md-icon--error",
        }
    }
}

/// 資料有效碼的色調：有效＝綠、停用／無效＝紅
pub fn active_tone(active: bool) -> Tone {
    if active { Tone::Success } else { Tone::Error }
}

/// 唯讀的資料有效欄位用：文字依狀態著色；沒有資料時不著色
pub fn active_field_class(active: Option<bool>) -> &'static str {
    match active {
        Some(true) => "md-field--success",
        Some(false) => "md-field--error",
        None => "",
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
    // ----- 功能鈕圖示（Material Symbols）-----
    pub const SEARCH: &str = "M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z";
    pub const ADD: &str = "M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z";
    pub const EDIT: &str = "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z";
    pub const DELETE: &str =
        "M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z";
    pub const COPY: &str = "M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z";
    pub const CLOSE: &str = "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z";
    /// 停用（block）
    pub const BLOCK: &str = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zM4 12c0-4.42 3.58-8 8-8 1.85 0 3.55.63 4.9 1.69L5.69 16.9C4.63 15.55 4 13.85 4 12zm8 8c-1.85 0-3.55-.63-4.9-1.69L18.31 7.1C19.37 8.45 20 10.15 20 12c0 4.42-3.58 8-8 8z";
    /// 啟用（check_circle）
    pub const CHECK_CIRCLE: &str = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z";
    pub const ARROW_BACK: &str = "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z";
    pub const ARROW_FORWARD: &str = "M12 4l-1.41 1.41L16.17 11H4v2h12.17l-5.58 5.59L12 20l8-8z";
    pub const REMOVE: &str = "M19 13H5v-2h14v2z";
    pub const DELETE_SWEEP: &str = "M15 16h4v2h-4zm0-8h7v2h-7zm0 4h6v2h-6zM3 18c0 1.1.9 2 2 2h6c1.1 0 2-.9 2-2V8H3v10zM14 5h-3l-1-1H6L5 5H2v2h12z";
    pub const CHECK_BOX: &str = "M19 3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.11 0 2-.9 2-2V5c0-1.1-.89-2-2-2zm-9 14l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z";
    pub const CHECK_BOX_BLANK: &str = "M19 5v14H5V5h14m0-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2z";
    /// 授權（verified_user）
    pub const VERIFIED_USER: &str = "M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm-2 16l-4-4 1.41-1.41L10 14.17l6.59-6.59L18 9l-8 8z";
    pub const SYNC: &str = "M12 4V1L8 5l4 4V6c3.31 0 6 2.69 6 6 0 1.01-.25 1.97-.7 2.8l1.46 1.46C19.54 15.03 20 13.57 20 12c0-4.42-3.58-8-8-8zm0 14c-3.31 0-6-2.69-6-6 0-1.01.25-1.97.7-2.8L5.24 7.74C4.46 8.97 4 10.43 4 12c0 4.42 3.58 8 8 8v3l4-4-4-4v3z";
    pub const OPEN_IN_NEW: &str = "M19 19H5V5h7V3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2v-7h-2v7zM14 3v2h3.59l-9.83 9.83 1.41 1.41L19 6.41V10h2V3h-7z";
    pub const CLEAR_ALL: &str = "M5 13h14v-2H5v2zm-2 4h14v-2H3v2zM7 7v2h14V7H7z";
    pub const PLAYLIST_ADD: &str =
        "M14 10H2v2h12v-2zm0-4H2v2h12V6zm4 8v-4h-2v4h-4v2h4v4h2v-4h4v-2h-4zM2 16h8v-2H2v2z";
    pub const PLAYLIST_REMOVE: &str = "M14 10H3v2h11v-2zm0-4H3v2h11V6zM3 16h7v-2H3v2zm11.41 6L17 19.41 19.59 22 21 20.59 18.41 18 21 15.41 19.59 14 17 16.59 14.41 14 13 15.41 15.59 18 13 20.59 14.41 22z";
    pub const LOCK_OPEN: &str = "M12 17c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm6-9h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6h1.9c0-1.71 1.39-3.1 3.1-3.1 1.71 0 3.1 1.39 3.1 3.1v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zm0 12H6V10h12v10z";
    /// 會計資料（account_balance）
    pub const ACCOUNT_BALANCE: &str =
        "M4 10v7h3v-7H4zm6 0v7h3v-7h-3zM2 22h19v-3H2v3zm14-12v7h3v-7h-3zm-4.5-9L2 6v2h19V6l-9.5-5z";
    pub const HELP: &str = "M11 18h2v-2h-2v2zm1-16C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm0-14c-2.21 0-4 1.79-4 4h2c0-1.1.9-2 2-2s2 .9 2 2c0 2-3 1.75-3 5h2c0-2.25 3-2.5 3-5 0-2.21-1.79-4-4-4z";
    /// 登入（login）
    pub const LOGIN: &str = "M11 7L9.6 8.4l2.6 2.6H2v2h10.2l-2.6 2.6L11 17l5-5-5-5zm9 12h-8v2h8c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2h-8v2h8v14z";
    /// 取消確認（undo）
    pub const UNDO: &str = "M12.5 8c-2.65 0-5.05.99-6.9 2.6L2 7v9h9l-3.62-3.62c1.39-1.16 3.16-1.88 5.12-1.88 3.54 0 6.55 2.31 7.6 5.5l2.37-.78C21.08 11.03 17.15 8 12.5 8z";
    /// 列印（print）
    pub const PRINT: &str = "M19 8H5c-1.66 0-3 1.34-3 3v6h4v4h12v-4h4v-6c0-1.66-1.34-3-3-3zm-3 11H8v-5h8v5zm3-7c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1zm-1-9H6v4h12V3z";
    pub const DOWNLOAD: &str = "M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z";
    /// 異動記錄（history）
    pub const HISTORY: &str = "M13 3c-4.97 0-9 4.03-9 9H1l3.89 3.89.07.14L9 12H6c0-3.87 3.13-7 7-7s7 3.13 7 7-3.13 7-7 7c-1.93 0-3.68-.79-4.94-2.06l-1.42 1.42C8.27 19.99 10.51 21 13 21c4.97 0 9-4.03 9-9s-4.03-9-9-9zm-1 5v5l4.28 2.54.72-1.21-3.5-2.08V8H12z";
    /// 品牌（factory）
    pub const FACTORY: &str = "M22 22H2V10l7-3v2l5-2v3h3l1-8h3l1 8v12zM12 9.95l-5 2V10l-3 1.32V20h16v-8h-8V9.95zM11 18h2v-4h-2v4zm-4 0h2v-4H7v4zm10-4h-2v4h2v-4z";
    // ----- 側欄節點 -----
    pub const HOME: &str = "M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z";
    pub const INVENTORY: &str = "M20 2H4c-1 0-2 .9-2 2v3.01c0 .72.43 1.34 1 1.69V20c0 1.1 1.1 2 2 2h14c.9 0 2-.9 2-2V8.7c.57-.35 1-.97 1-1.69V4c0-1.1-1-2-2-2zm-5 12H9v-2h6v2zm5-7H4V4l16-.02V7z";
    pub const CATEGORY: &str = "M12 2l-5.5 9h11L12 2zm5.5 11c-2.49 0-4.5 2.01-4.5 4.5s2.01 4.5 4.5 4.5 4.5-2.01 4.5-4.5-2.01-4.5-4.5-4.5zM3 21.5h8v-8H3v8z";
    pub const ACCOUNT_TREE: &str = "M22 11V3h-7v3H9V3H2v8h7V8h2v10h4v3h7v-8h-7v3h-2V8h2v3z";
    pub const GROUP_PEOPLE: &str = "M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3zm-8 0c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5C6.34 5 5 6.34 5 8s1.34 3 3 3zm0 2c-2.33 0-7 1.17-7 3.5V19h14v-2.5c0-2.33-4.67-3.5-7-3.5zm8 0c-.29 0-.62.02-.97.05 1.16.84 1.97 1.97 1.97 3.45V19h6v-2.5c0-2.33-4.67-3.5-7-3.5z";
    pub const SETTINGS: &str = "M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.05.3-.09.63-.09.94s.02.64.07.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z";
    pub const APPS: &str = "M4 8h4V4H4v4zm6 12h4v-4h-4v4zm-6 0h4v-4H4v4zm0-6h4v-4H4v4zm6 0h4v-4h-4v4zm6-10v4h4V4h-4zm-6 4h4V4h-4v4zm6 6h4v-4h-4v4zm0 6h4v-4h-4v4z";
    pub const MENU_BOOK: &str = "M21 5c-1.11-.35-2.33-.5-3.5-.5-1.95 0-4.05.4-5.5 1.5-1.45-1.1-3.55-1.5-5.5-1.5S2.45 4.9 1 6v14.65c0 .25.25.5.5.5.1 0 .15-.05.25-.05C3.1 20.45 5.05 20 6.5 20c1.95 0 4.05.4 5.5 1.5 1.35-.85 3.8-1.5 5.5-1.5 1.65 0 3.35.3 4.75 1.05.1.05.15.05.25.05.25 0 .5-.25.5-.5V6c-.6-.45-1.25-.75-2-1zm0 13.5c-1.1-.35-2.3-.5-3.5-.5-1.7 0-4.15.65-5.5 1.5V8c1.35-.85 3.8-1.5 5.5-1.5 1.2 0 2.4.15 3.5.5v11.5z";
    /// 分類碼（label）
    /// straighten（尺）：單位
    pub const STRAIGHTEN: &str = "M21 6H3c-1.1 0-2 .9-2 2v8c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm0 10H3V8h2v4h2V8h2v4h2V8h2v4h2V8h2v4h2V8h2v8z";
    /// local_offer（價格標籤）：產品分類
    pub const LOCAL_OFFER: &str = "M21.41 11.58l-9-9C12.05 2.22 11.55 2 11 2H4c-1.1 0-2 .9-2 2v7c0 .55.22 1.05.59 1.42l9 9c.36.36.86.58 1.41.58.55 0 1.05-.22 1.41-.59l7-7c.37-.36.59-.86.59-1.41 0-.55-.23-1.06-.59-1.42zM5.5 7C4.67 7 4 6.33 4 5.5S4.67 4 5.5 4 7 4.67 7 5.5 6.33 7 5.5 7z";
    pub const LABEL: &str = "M17.63 5.84C17.27 5.33 16.67 5 16 5L5 5.01C3.9 5.01 3 5.9 3 7v10c0 1.1.9 1.99 2 1.99L16 19c.67 0 1.27-.33 1.63-.84L22 12l-4.37-6.16z";
    /// 上移、下移（arrow_upward／arrow_downward）
    pub const ARROW_UPWARD: &str = "M4 12l1.41 1.41L11 7.83V20h2V7.83l5.58 5.59L20 12l-8-8-8 8z";
    pub const ARROW_DOWNWARD: &str =
        "M20 12l-1.41-1.41L13 16.17V4h-2v12.17l-5.58-5.59L4 12l8 8 8-8z";
    /// 目錄（folder）
    pub const FOLDER: &str = "M10 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z";
    /// 選單維護（list）
    pub const LIST: &str =
        "M3 13h2v-2H3v2zm0 4h2v-2H3v2zm0-8h2V7H3v2zm4 4h14v-2H7v2zm0 4h14v-2H7v2zM7 7v2h14V7H7z";
    pub const SHIELD: &str = "M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4z";
    pub const PERSON: &str = "M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z";
    pub const LOCK: &str = "M18 8h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zm-6 9c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zm3.1-9H8.9V6c0-1.71 1.39-3.1 3.1-3.1 1.71 0 3.1 1.39 3.1 3.1v2z";
    pub const VISIBILITY: &str = "M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z";
    pub const VISIBILITY_OFF: &str = "M12 7c2.76 0 5 2.24 5 5 0 .65-.13 1.26-.36 1.83l2.92 2.92c1.51-1.26 2.7-2.89 3.43-4.75-1.73-4.39-6-7.5-11-7.5-1.4 0-2.74.25-3.98.7l2.16 2.16C10.74 7.13 11.35 7 12 7zM2 4.27l2.28 2.28.46.46C3.08 8.3 1.78 10.02 1 12c1.73 4.39 6 7.5 11 7.5 1.55 0 3.03-.3 4.38-.84l.42.42L19.73 22 21 20.73 3.27 3 2 4.27zM7.53 9.8l1.55 1.55c-.05.21-.08.43-.08.65 0 1.66 1.34 3 3 3 .22 0 .44-.03.65-.08l1.55 1.55c-.67.33-1.41.53-2.2.53-2.76 0-5-2.24-5-5 0-.79.2-1.53.53-2.2zm4.31-.78l3.15 3.15.02-.16c0-1.66-1.34-3-3-3l-.17.01z";
    pub const SEND: &str = "M2.01 21L23 12 2.01 3 2 10l15 2-15 2z";
    /// 使用者（account_circle）：登入者選單
    pub const ACCOUNT: &str = "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 4c1.93 0 3.5 1.57 3.5 3.5S13.93 13 12 13s-3.5-1.57-3.5-3.5S10.07 6 12 6zm0 14c-2.03 0-4.43-.82-6.14-2.88C7.55 15.8 9.68 15 12 15s4.45.8 6.14 2.12C16.43 19.18 14.03 20 12 20z";
    /// 鑰匙（key）：變更密碼
    pub const KEY: &str = "M12.65 10C11.83 7.67 9.61 6 7 6c-3.31 0-6 2.69-6 6s2.69 6 6 6c2.61 0 4.83-1.67 5.65-4H17v4h4v-4h2v-4H12.65zM7 14c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2z";
    /// 登出（logout）
    pub const LOGOUT: &str = "M17 7l-1.41 1.41L18.17 11H8v2h10.17l-2.58 2.58L17 17l5-5zM4 5h8V3H4c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h8v-2H4V5z";
    /// 地球（language）：切換介面語系
    pub const LANGUAGE: &str = "M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zm6.93 6h-2.95c-.32-1.25-.78-2.45-1.38-3.56 1.84.63 3.37 1.91 4.33 3.56zM12 4.04c.83 1.2 1.48 2.53 1.91 3.96h-3.82c.43-1.43 1.08-2.76 1.91-3.96zM4.26 14C4.1 13.36 4 12.69 4 12s.1-1.36.26-2h3.38c-.08.66-.14 1.32-.14 2s.06 1.34.14 2H4.26zm.82 2h2.95c.32 1.25.78 2.45 1.38 3.56-1.84-.63-3.37-1.9-4.33-3.56zm2.95-8H5.08c.96-1.66 2.49-2.93 4.33-3.56C8.81 5.55 8.35 6.75 8.03 8zM12 19.96c-.83-1.2-1.48-2.53-1.91-3.96h3.82c-.43 1.43-1.08 2.76-1.91 3.96zM14.34 14H9.66c-.09-.66-.16-1.32-.16-2s.07-1.35.16-2h4.68c.09.65.16 1.32.16 2s-.07 1.34-.16 2zm.25 5.56c.6-1.11 1.06-2.31 1.38-3.56h2.95c-.96 1.65-2.49 2.93-4.33 3.56zM16.36 14c.08-.66.14-1.32.14-2s-.06-1.34-.14-2h3.38c.16.64.26 1.31.26 2s-.1 1.36-.26 2h-3.38z";
}
