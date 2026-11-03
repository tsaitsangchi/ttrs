//! 目前使用者在各作業的有效權限（authorization.feature AQ13；ADR 0006 §4）。
//! 畫面隱藏沒有權限的按鈕只是方便，伺服器一定會檢查；資料權限（他人、他部門）照 TipTop 按下後才提示。

use dioxus::prelude::*;
use ttrs_shared::MyPermissionDto;

use crate::api;
use crate::components::material::{Notification, NotificationKind};
use crate::i18n::{t, tf};

/// 作業的有效權限；由 [`ProgramGate`] 提供，頁面以 `use_context::<Perm>()` 取得
#[derive(Clone, Copy, PartialEq)]
pub struct Perm(Resource<Result<MyPermissionDto, String>>);

impl Perm {
    fn with<R>(&self, f: impl FnOnce(&MyPermissionDto) -> R) -> Option<R> {
        self.0.read().as_ref().and_then(|r| r.as_ref().ok()).map(f)
    }

    /// 功能在允許清單中（cl_chk_act_auth）
    pub fn can(&self, action: &str) -> bool {
        self.with(|p| p.allowed && p.allows(action))
            .unwrap_or(false)
    }

    /// 單身可新增列（zy07 = 0、1）
    pub fn add_row(&self) -> bool {
        self.can("detail")
            && self.with(|p| matches!(p.detail_rows.as_str(), "0" | "1")) == Some(true)
    }

    /// 單身可刪除列（zy07 = 0、2）
    pub fn delete_row(&self) -> bool {
        self.can("detail")
            && self.with(|p| matches!(p.detail_rows.as_str(), "0" | "2")) == Some(true)
    }
}

/// 先取得目前使用者在此作業的有效權限：沒有本作業權限時顯示訊息、不載入畫面內容
#[component]
pub fn ProgramGate(program: &'static str, children: Element) -> Element {
    let perm = Perm(use_resource(move || async move {
        api::my_permission(program).await
    }));
    use_context_provider(|| perm);
    let mut dismissed = use_signal(|| false);
    let state = perm.0.read();
    match &*state {
        None => rsx! { p { class: "page", {t("common-msg-loading")} } },
        Some(Err(e)) => rsx! {
            div { class: "page",
                if !dismissed() {
                    Notification { kind: NotificationKind::Error, title: e.clone(), onclose: move |_| dismissed.set(true) }
                }
            }
        },
        Some(Ok(p)) if !p.allowed => rsx! {
            div { class: "page",
                Notification {
                    kind: NotificationKind::Error,
                    title: tf("common-msg-no-program-permission", &[("program", program)]),
                    onclose: move |_| {},
                }
            }
        },
        Some(Ok(_)) => children,
    }
}
