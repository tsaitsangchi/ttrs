//! 登入與連線（TipTop weblogin、webpasswd；specs/features/azz/sign_in.feature、web_password.feature，ADR 0009）
//! 外框依連線狀態顯示登入畫面或作業；須改密碼時登入畫面改為變更密碼表單。

use dioxus::prelude::*;
use ttrs_shared::{ChangePasswordInput, PasswordResetInput, SessionDto, SignInPlantsDto};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Notification, NotificationKind, Select, TextField, Tone, icons,
};
use crate::i18n::{t, tf};

/// 目前的連線（以 context 提供給外框與工廠層作業）
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Loading,
    SignedOut,
    SignedIn(Box<SessionDto>),
}

impl SessionState {
    pub fn session(&self) -> Option<&SessionDto> {
        match self {
            Self::SignedIn(s) => Some(s),
            _ => None,
        }
    }
}

pub fn use_session() -> Signal<SessionState> {
    use_context::<Signal<SessionState>>()
}

/// 登入畫面（weblogin.per：使用者代號、密碼）
#[component]
pub fn SignInPage() -> Element {
    let mut session = use_session();
    let mut user_id = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    // 須改密碼（PASSWORD_CHANGE_REQUIRED）：改用變更密碼表單，舊密碼為剛輸入的密碼
    let mut must_change = use_signal(|| false);
    let mut busy = use_signal(|| false);
    // 忘記密碼（ADR 0010 §A）：未設定寄信伺服器時不顯示入口（R9）
    let reset_available = use_resource(api::password_reset_available);
    let mut forgot = use_signal(|| false);
    // 營運中心（sign_in.feature SL1–SL8）：輸入完使用者代號（離開欄位）後載入有效的營運中心並自動選取
    let mut plants = use_signal(SignInPlantsDto::default);
    let mut plants_for = use_signal(String::new);
    let mut plant = use_signal(String::new);
    let load_plants = move |_| async move {
        let id = user_id().trim().to_owned();
        if id.is_empty() || id == plants_for() {
            return;
        }
        let options = api::sign_in_plants(&id).await;
        // 載入期間又改了代號：丟棄舊結果
        if user_id().trim() != id {
            return;
        }
        plant.set(options.selected.clone().unwrap_or_default());
        plants.set(options);
        plants_for.set(id);
    };
    // 已載入此代號的選項時送出選的營運中心，否則由伺服器自動選取（SQ12）
    let chosen_plant = move || {
        let p = plant();
        (plants_for() == user_id().trim() && !p.is_empty()).then_some(p)
    };

    let submit = move || async move {
        if busy() {
            return;
        }
        busy.set(true);
        match api::sign_in(&user_id(), &password(), chosen_plant().as_deref()).await {
            Ok(s) => session.set(SessionState::SignedIn(Box::new(s))),
            Err(e) if e.code == "PASSWORD_CHANGE_REQUIRED" => {
                error.set(None);
                must_change.set(true);
            }
            Err(e) => error.set(Some(e.message)),
        }
        busy.set(false);
    };

    rsx! {
        div { class: "sign-in md-theme-light",
            div { class: "sign-in-card",
                h1 { "TTRS ERP" }
                if forgot() {
                    ForgotPasswordForm {
                        user_id: user_id(),
                        onback: move |_| forgot.set(false),
                    }
                } else if must_change() {
                    h2 { {t("password-title")} }
                    p { class: "note", {t("password-hint-required")} }
                    ChangePasswordForm {
                        user_id: user_id(),
                        old_password: password(),
                        ondone: move |new_password: String| async move {
                            // 改好後以新密碼登入
                            match api::sign_in(&user_id(), &new_password, chosen_plant().as_deref()).await {
                                Ok(s) => session.set(SessionState::SignedIn(Box::new(s))),
                                Err(e) => {
                                    must_change.set(false);
                                    error.set(Some(e.message));
                                }
                            }
                        },
                        oncancel: move |_| {
                            must_change.set(false);
                            password.set(String::new());
                        },
                    }
                } else {
                    h2 { {t("signin-title")} }
                    form {
                        class: "sign-in-form",
                        onsubmit: move |e| async move {
                            e.prevent_default();
                            submit().await;
                        },
                        TextField { id: "zx01", label: t("signin-field-user"), maxlength: 10usize, value: user_id(), oninput: move |v| user_id.set(v), onblur: load_plants }
                        TextField { id: "zx10", label: t("signin-field-password"), input_type: "password", maxlength: 64usize, value: password(), oninput: move |v| password.set(v) }
                        Select {
                            id: "plant",
                            label: t("signin-field-plant"),
                            options: if plants_for() == user_id().trim() {
                                plants().plants.iter().map(|p| (p.plant_id.clone(), format!("{} {}", p.plant_id, p.name))).collect::<Vec<_>>()
                            } else {
                                Vec::new()
                            },
                            value: plant(),
                            disabled: plants_for() != user_id().trim() || plants().plants.is_empty() || plants().fixed,
                            onchange: move |v| plant.set(v),
                        }
                        button { r#type: "submit", class: "md-button md-state md-button--filled md-button--with-icon", disabled: busy() || user_id().trim().is_empty(),
                            svg { class: "md-button__icon", width: "18", height: "18", view_box: "0 0 24 24", fill: "currentColor", "aria-hidden": "true", path { d: icons::LOGIN } }
                            {t("signin-action-sign-in")}
                        }
                    }
                    if reset_available() == Some(true) {
                        Button {
                            icon: icons::HELP, tone: Tone::Primary,
                            variant: ButtonVariant::Text,
                            onclick: move |_| {
                                error.set(None);
                                forgot.set(true);
                            },
                            {t("forgot-action-open")}
                        }
                    }
                }
                if let Some(msg) = error() {
                    Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
                }
            }
        }
    }
}

/// 變更密碼表單（webpasswd.per：舊密碼、新密碼、確認新密碼）；成功時回傳新密碼
#[component]
pub fn ChangePasswordForm(
    user_id: String,
    /// 須改密碼時帶入登入時輸入的密碼；一般變更時空白，由使用者輸入
    #[props(default)]
    old_password: String,
    ondone: EventHandler<String>,
    oncancel: EventHandler<()>,
) -> Element {
    let ask_old = old_password.is_empty();
    let mut old = use_signal(|| old_password.clone());
    let mut new = use_signal(String::new);
    let mut confirm = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let id = user_id.clone();
    rsx! {
        div { class: "sign-in-form",
            if ask_old {
                TextField { id: "zx10_old", label: t("password-field-old"), input_type: "password", maxlength: 64usize, value: old(), oninput: move |v| old.set(v) }
            }
            TextField { id: "zx10_new", label: t("password-field-new"), input_type: "password", maxlength: 64usize, value: new(), oninput: move |v| new.set(v) }
            TextField { id: "zx10_ver", label: t("password-field-confirm"), input_type: "password", maxlength: 64usize, value: confirm(), oninput: move |v| confirm.set(v) }
            div { class: "action-bar",
                Button {
                    icon: icons::CHECK, tone: Tone::Success,
                    onclick: move |_| {
                        let input = ChangePasswordInput {
                            user_id: id.clone(),
                            old_password: old(),
                            new_password: new(),
                            confirm_password: confirm(),
                        };
                        spawn(async move {
                            match api::change_password(&input).await {
                                Ok(()) => ondone.call(input.new_password),
                                Err(e) => error.set(Some(e)),
                            }
                        });
                    },
                    {t("common-action-ok")}
                }
                Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| oncancel.call(()), {t("common-action-cancel")} }
            }
            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
        }
    }
}

/// 忘記密碼（ADR 0010 §A）：輸入使用者代號寄出重設連結；不論帳號是否存在都顯示同一個訊息（R2）
#[component]
fn ForgotPasswordForm(user_id: String, onback: EventHandler<()>) -> Element {
    let mut id = use_signal(|| user_id.clone());
    let mut sent = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        h2 { {t("forgot-title")} }
        if sent() {
            Notification { kind: NotificationKind::Success, title: t("forgot-msg-sent"), onclose: move |_| {} }
        } else {
            p { class: "note", {t("forgot-hint")} }
            form {
                class: "sign-in-form",
                onsubmit: move |e| async move {
                    e.prevent_default();
                    if busy() {
                        return;
                    }
                    busy.set(true);
                    match api::request_password_reset(&id()).await {
                        Ok(()) => sent.set(true),
                        Err(e) => error.set(Some(e)),
                    }
                    busy.set(false);
                },
                TextField { id: "zx01_forgot", label: t("signin-field-user"), maxlength: 10usize, value: id(), oninput: move |v| id.set(v) }
                button { r#type: "submit", class: "md-button md-state md-button--filled md-button--with-icon", disabled: busy() || id().trim().is_empty(),
                    svg { class: "md-button__icon", width: "18", height: "18", view_box: "0 0 24 24", fill: "currentColor", "aria-hidden": "true", path { d: icons::SEND } }
                    {t("forgot-action-send")}
                }
            }
        }
        Button { icon: icons::ARROW_BACK, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| onback.call(()), {t("forgot-action-back")} }
        if let Some(msg) = error() {
            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
        }
    }
}

/// 重設連結開啟的畫面（/reset-password?token=…）：不需登入；成功後回登入畫面以新密碼登入
#[component]
pub fn ResetPasswordPage(token: String) -> Element {
    let mut new = use_signal(String::new);
    let mut confirm = use_signal(String::new);
    let mut done = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    // 回到首頁：網址不再帶權杖，外框重新取得連線狀態（原連線已失效）
    let back = move |_| {
        navigator().replace("/");
    };
    rsx! {
        div { class: "sign-in md-theme-light",
            div { class: "sign-in-card",
                h1 { "TTRS ERP" }
                h2 { {t("reset-title")} }
                if done() {
                    Notification { kind: NotificationKind::Success, title: t("reset-msg-done"), onclose: move |_| {} }
                    Button { icon: icons::ARROW_BACK, tone: Tone::Neutral, onclick: back, {t("forgot-action-back")} }
                } else {
                    p { class: "note", {t("reset-hint")} }
                    form {
                        class: "sign-in-form",
                        onsubmit: move |e| {
                            e.prevent_default();
                            let token = token.clone();
                            async move {
                                if busy() {
                                    return;
                                }
                                busy.set(true);
                                let input = PasswordResetInput {
                                    token,
                                    new_password: new(),
                                    confirm_password: confirm(),
                                };
                                match api::confirm_password_reset(&input).await {
                                    Ok(()) => done.set(true),
                                    Err(e) => error.set(Some(e)),
                                }
                                busy.set(false);
                            }
                        },
                        TextField { id: "zx10_new", label: t("password-field-new"), input_type: "password", maxlength: 64usize, value: new(), oninput: move |v| new.set(v) }
                        TextField { id: "zx10_ver", label: t("password-field-confirm"), input_type: "password", maxlength: 64usize, value: confirm(), oninput: move |v| confirm.set(v) }
                        button { r#type: "submit", class: "md-button md-state md-button--filled md-button--with-icon", disabled: busy() || new().is_empty(),
                            svg { class: "md-button__icon", width: "18", height: "18", view_box: "0 0 24 24", fill: "currentColor", "aria-hidden": "true", path { d: icons::CHECK } }
                            {t("common-action-ok")}
                        }
                    }
                    Button { icon: icons::ARROW_BACK, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: back, {t("forgot-action-back")} }
                }
                if let Some(msg) = error() {
                    Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
                }
            }
        }
    }
}

/// 登入者選單（外框左上角、語系圖示右邊）：使用者、目前營運中心、切換營運中心（aoos901）、
/// 密碼即將到期提示、變更密碼（webpasswd）、登出。有密碼提示時圖示加上提示點
#[component]
pub fn UserMenu(
    session: SessionDto,
    onswitch: EventHandler<String>,
    onchangepassword: EventHandler<()>,
    onsignout: EventHandler<()>,
) -> Element {
    let mut open = use_signal(|| false);
    let label = t("shell-user-menu");
    let mut notices = Vec::new();
    if let Some(days) = session.password_expires_in_days {
        notices.push(tf(
            "shell-password-expiring",
            &[("days", days.to_string().as_str())],
        ));
    }
    if let Some(uses) = session.password_uses_left {
        notices.push(tf(
            "shell-password-uses-left",
            &[("uses", uses.to_string().as_str())],
        ));
    }
    let has_notice = !notices.is_empty();
    let switchable = session.multi_plant && session.plants.len() > 1;
    let name = session.name.clone().unwrap_or_default();
    rsx! {
        div { class: "md-menu-anchor",
            button {
                r#type: "button",
                class: "md-icon-button md-state user-menu-button",
                class: if open() { "md-icon-button--selected" },
                title: "{session.user_id} {name}",
                aria_label: "{label}",
                aria_haspopup: "menu",
                aria_expanded: open(),
                onclick: move |_| open.toggle(),
                svg { width: "24", height: "24", view_box: "0 0 24 24", fill: "currentColor",
                    path { d: icons::ACCOUNT }
                }
                if has_notice {
                    span { class: "user-menu-badge" }
                }
            }
            if open() {
                div { class: "md-menu-scrim", onclick: move |_| open.set(false) }
                div { class: "md-menu user-menu", role: "menu", aria_label: "{label}",
                    div { class: "user-menu__header",
                        div { class: "session-user", "{session.user_id} {name}" }
                        div { class: "session-plant", "{session.plant} {session.plant_name}" }
                        for n in notices {
                            div { class: "session-notice", "{n}" }
                        }
                    }
                    if switchable {
                        div { class: "user-menu__divider" }
                        div { class: "user-menu__label",
                            svg { width: "16", height: "16", view_box: "0 0 24 24", fill: "currentColor",
                                path { d: icons::PLANT }
                            }
                            span { {t("shell-switch-plant")} }
                        }
                        for p in session.plants.clone() {
                            div {
                                key: "{p.plant_id}",
                                role: "menuitemradio",
                                aria_checked: p.plant_id == session.plant,
                                class: "md-menu__item md-state",
                                class: if p.plant_id == session.plant { "md-menu__item--selected" },
                                onclick: {
                                    let id = p.plant_id.clone();
                                    move |_| {
                                        open.set(false);
                                        onswitch.call(id.clone());
                                    }
                                },
                                span { class: "md-menu__check",
                                    if p.plant_id == session.plant {
                                        svg { width: "18", height: "18", view_box: "0 0 24 24", fill: "currentColor",
                                            path { d: icons::CHECK }
                                        }
                                    }
                                }
                                span { "{p.plant_id} {p.name}" }
                            }
                        }
                    }
                    div { class: "user-menu__divider" }
                    div {
                        role: "menuitem",
                        class: "md-menu__item md-state",
                        onclick: move |_| {
                            open.set(false);
                            onchangepassword.call(());
                        },
                        svg { width: "18", height: "18", view_box: "0 0 24 24", fill: "currentColor",
                            path { d: icons::KEY }
                        }
                        span { {t("shell-change-password")} }
                    }
                    div {
                        role: "menuitem",
                        class: "md-menu__item md-state",
                        onclick: move |_| {
                            open.set(false);
                            onsignout.call(());
                        },
                        svg { width: "18", height: "18", view_box: "0 0 24 24", fill: "currentColor",
                            path { d: icons::LOGOUT }
                        }
                        span { {t("shell-sign-out")} }
                    }
                }
            }
        }
    }
}
