//! TTRS 前端入口。畫面依 TipTop 模組分資料夾：pages/aim（庫存/料件）、pages/azz（系統管理）…

mod api;
mod components;
mod i18n;
mod pages;

use components::material::{Dialog, MenuButton, icons};
use components::session::{
    ChangePasswordForm, ResetPasswordPage, SessionState, SignInPage, UserMenu, use_session,
};
use dioxus::prelude::*;
use i18n::{UiLang, t};
use pages::{
    aim::ItemList,
    aoo::{DepartmentMaintenance, EmployeeMaintenance, PlantMaintenance},
    azz::{
        DataDictionary, PermissionClassMaintenance, ProgramMaintenance, ProgramPermissions,
        ProgramPermissionsFor, UserMaintenance, UserPermissions, UserPermissionsFor,
    },
    home::Home,
};

/// Material 3：色彩 token（自動產生）→ 元件樣式 → 版面，順序不可調換
const M3_COLORS_CSS: Asset = asset!("/assets/material/m3-colors.css");
const M3_CSS: Asset = asset!("/assets/material/m3.css");
const MAIN_CSS: Asset = asset!("/assets/main.css");

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    /// 忘記密碼的重設連結（ADR 0010 §A）：不需登入，不在外框內
    #[route("/reset-password?:token")]
    ResetPassword { token: String },
    #[layout(Shell)]
        #[route("/")]
        Home {},
        /// aimi100 料件基本資料
        #[route("/aim/items")]
        ItemList {},
        /// Plant Maintenance 營運中心維護（TipTop aooi901）
        #[route("/aoo/plant-maintenance")]
        PlantMaintenance {},
        #[route("/aoo/department-maintenance")]
        DepartmentMaintenance {},
        #[route("/aoo/employee-maintenance")]
        EmployeeMaintenance {},
        /// Program Maintenance 程式維護（TipTop p_zz）
        #[route("/azz/program-maintenance")]
        ProgramMaintenance {},
        /// Data Dictionary 資料字典維護（TipTop p_zta）
        #[route("/azz/data-dictionary")]
        DataDictionary {},
        /// User Maintenance 使用者維護（TipTop p_zx）
        #[route("/azz/user-maintenance")]
        UserMaintenance {},
        /// User Special Permission Maintenance 使用者特殊權限維護（TipTop p_zxw）
        #[route("/azz/user-permissions")]
        UserPermissions {},
        /// 由使用者維護帶入使用者
        #[route("/azz/user-permissions/:user")]
        UserPermissionsFor { user: String },
        /// Permission Class Maintenance 權限類別維護（TipTop p_zw）
        #[route("/azz/permission-classes")]
        PermissionClassMaintenance {},
        /// Program Permission Maintenance 程式執行權限維護（TipTop p_zy）
        #[route("/azz/program-permissions")]
        ProgramPermissions {},
        /// 由權限類別維護帶入權限類別
        #[route("/azz/program-permissions/:class")]
        ProgramPermissionsFor { class: String },
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // 介面語系（L3）：啟動時讀取瀏覽器記住的語系；供外框切換
    use_context_provider(|| Signal::new(i18n::load()));
    // 目前的連線（sign_in.feature）：登入者與目前營運中心，供外框與工廠層作業使用
    use_context_provider(|| Signal::new(SessionState::Loading));
    rsx! {
        // 不要由瀏覽器翻譯（Chrome）：譯文會蓋掉畫面更新；語系由 i18n::set 設在 <html lang>
        document::Meta { name: "google", content: "notranslate" }
        document::Link { rel: "stylesheet", href: M3_COLORS_CSS }
        document::Link { rel: "stylesheet", href: M3_CSS }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
fn ResetPassword(token: String) -> Element {
    rsx! { ResetPasswordPage { token } }
}

#[component]
fn Shell() -> Element {
    let mut lang = use_context::<Signal<UiLang>>();
    let current = lang();
    let mut session = use_session();
    let mut session_error = use_signal(|| None::<String>);
    use_future(move || async move {
        match api::session().await {
            Ok(Some(s)) => session.set(SessionState::SignedIn(Box::new(s))),
            Ok(None) => session.set(SessionState::SignedOut),
            Err(e) => {
                session_error.set(Some(e));
                session.set(SessionState::SignedOut);
            }
        }
    });
    let mut switch_error = use_signal(|| None::<String>);
    let mut changing_password = use_signal(|| false);
    // 未登入：只顯示登入畫面（weblogin）
    match session() {
        SessionState::Loading => return rsx! {},
        SessionState::SignedOut => {
            return rsx! {
                SignInPage {}
                if let Some(e) = session_error() {
                    p { class: "session-error", "{e}" }
                }
            };
        }
        SessionState::SignedIn(_) => {}
    }
    rsx! {
        // M3 淺色主題；深色改為 md-theme-dark
        div { class: "shell md-theme-light", lang: current.code(),
            nav { class: "sidebar",
                div { class: "sidebar-header",
                    // 介面語系（L3）：最左上方的地球圖示，點開選單切換
                    MenuButton {
                        label: t("shell-language"),
                        path: icons::LANGUAGE,
                        items: UiLang::ALL.iter().map(|l| (l.code().to_owned(), l.native_name().to_owned())).collect::<Vec<_>>(),
                        selected: current.code().to_owned(),
                        onselect: move |code: String| {
                            if let Some(l) = UiLang::from_code(&code) {
                                i18n::set(l);
                                lang.set(l);
                            }
                        },
                    }
                    // 登入者選單：語系圖示右邊（使用者、營運中心、切換營運中心、變更密碼、登出）
                    if let SessionState::SignedIn(s) = session() {
                        UserMenu {
                            session: (*s).clone(),
                            onswitch: move |plant: String| async move {
                                match api::switch_plant(&plant).await {
                                    Ok(s) => { switch_error.set(None); session.set(SessionState::SignedIn(Box::new(s))); }
                                    Err(e) => switch_error.set(Some(e)),
                                }
                            },
                            onchangepassword: move |_| changing_password.set(true),
                            onsignout: move |_| async move {
                                let _ = api::sign_out().await;
                                session.set(SessionState::SignedOut);
                            },
                        }
                    }
                    h1 { "TTRS ERP" }
                }
                if let SessionState::SignedIn(s) = session() {
                    if let Some(e) = switch_error() {
                        p { class: "session-error", "{e}" }
                    }
                    if changing_password() {
                        Dialog {
                            title: t("password-title"),
                            oncancel: move |_| changing_password.set(false),
                            actions: rsx! {},
                            ChangePasswordForm {
                                user_id: s.user_id.clone(),
                                ondone: move |_| {
                                    changing_password.set(false);
                                    switch_error.set(Some(t("password-msg-changed")));
                                },
                                oncancel: move |_| changing_password.set(false),
                            }
                        }
                    }
                }
                Link { active_class: "active", to: Route::Home {}, {t("shell-nav-home")} }
                p { class: "group", {t("shell-group-aim")} }
                Link { active_class: "active", to: Route::ItemList {}, {t("shell-nav-items")} }
                p { class: "group", {t("shell-group-aoo")} }
                Link { active_class: "active", to: Route::PlantMaintenance {}, {t("shell-nav-plants")} }
                Link { active_class: "active", to: Route::DepartmentMaintenance {}, {t("shell-nav-departments")} }
                Link { active_class: "active", to: Route::EmployeeMaintenance {}, {t("shell-nav-employees")} }
                p { class: "group", {t("shell-group-azz")} }
                Link { active_class: "active", to: Route::ProgramMaintenance {}, {t("shell-nav-program")} }
                Link { active_class: "active", to: Route::DataDictionary {}, {t("shell-nav-dictionary")} }
                Link { active_class: "active", to: Route::UserMaintenance {}, {t("shell-nav-users")} }
                Link { active_class: "active", to: Route::UserPermissions {}, {t("shell-nav-user-permissions")} }
                Link { active_class: "active", to: Route::PermissionClassMaintenance {}, {t("shell-nav-classes")} }
                Link { active_class: "active", to: Route::ProgramPermissions {}, {t("shell-nav-permissions")} }
            }
            // 以語系為 key：切換語系時重新載入畫面，所有文字改用新語系
            for l in [current] {
                main { key: "{l.code()}", class: "content", Outlet::<Route> {} }
            }
        }
    }
}
