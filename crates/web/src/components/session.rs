//! 登入與連線（TipTop weblogin、webpasswd；specs/features/azz/sign_in.feature、web_password.feature，ADR 0009）
//! 外框依連線狀態顯示登入畫面或作業；須改密碼時登入畫面改為變更密碼表單。

use dioxus::prelude::*;
use ttrs_shared::{ChangePasswordInput, SessionDto};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Notification, NotificationKind, TextField,
};
use crate::i18n::t;

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

    let submit = move || async move {
        if busy() {
            return;
        }
        busy.set(true);
        match api::sign_in(&user_id(), &password()).await {
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
                if must_change() {
                    h2 { {t("password-title")} }
                    p { class: "note", {t("password-hint-required")} }
                    ChangePasswordForm {
                        user_id: user_id(),
                        old_password: password(),
                        ondone: move |new_password: String| async move {
                            // 改好後以新密碼登入
                            match api::sign_in(&user_id(), &new_password).await {
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
                        TextField { id: "zx01", label: t("signin-field-user"), maxlength: 10usize, value: user_id(), oninput: move |v| user_id.set(v) }
                        TextField { id: "zx10", label: t("signin-field-password"), input_type: "password", maxlength: 64usize, value: password(), oninput: move |v| password.set(v) }
                        button { r#type: "submit", class: "md-button md-state md-button--filled", disabled: busy() || user_id().trim().is_empty(),
                            {t("signin-action-sign-in")}
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
                Button { variant: ButtonVariant::Text, onclick: move |_| oncancel.call(()), {t("common-action-cancel")} }
            }
            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
        }
    }
}
