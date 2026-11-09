//! TTRS 前端入口。畫面依 TipTop 模組分資料夾：pages/aim（庫存/料件）、pages/azz（系統管理）…

mod api;
mod components;
mod i18n;
mod pages;

use components::material::{Dialog, MenuButton, Tone, icons};
use components::session::{
    ChangePasswordForm, ResetPasswordPage, SessionState, SignInPage, UserMenu, use_session,
};
use dioxus::prelude::*;
use i18n::{UiLang, t};
use pages::{
    aim::ItemList,
    aoo::{
        ClassificationCodeMaintenance, DepartmentMaintenance, EmployeeMaintenance, PlantMaintenance,
    },
    azz::{
        DataDictionary, MenuMaintenance, PermissionClassMaintenance, ProgramMaintenance,
        ProgramPermissions, ProgramPermissionsFor, UserMaintenance, UserPermissions,
        UserPermissionsFor,
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
        // 網址一律 /<模組>/<程式代號>（menu_maintenance.feature MN12）；舊網址轉到新網址
        /// aimi100 料件基本資料
        #[route("/aim/item-maintenance")]
        ItemList {},
        /// Plant Maintenance 營運中心維護（TipTop aooi901）
        #[route("/aoo/plant-maintenance")]
        PlantMaintenance {},
        #[route("/aoo/department-maintenance")]
        DepartmentMaintenance {},
        #[route("/aoo/employee-maintenance")]
        EmployeeMaintenance {},
        /// Classification Code Maintenance 分類碼維護（TipTop aooi300）
        #[route("/aoo/classification-code-maintenance")]
        ClassificationCodeMaintenance {},
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
        #[route("/azz/user-permission-maintenance")]
        UserPermissions {},
        /// 由使用者維護帶入使用者
        #[route("/azz/user-permission-maintenance/:user")]
        UserPermissionsFor { user: String },
        /// Permission Class Maintenance 權限類別維護（TipTop p_zw）
        #[route("/azz/permission-class-maintenance")]
        PermissionClassMaintenance {},
        /// Program Permission Maintenance 程式執行權限維護（TipTop p_zy）
        #[route("/azz/program-permission-maintenance")]
        ProgramPermissions {},
        /// 由權限類別維護帶入權限類別
        #[route("/azz/program-permission-maintenance/:class")]
        ProgramPermissionsFor { class: String },
        /// Menu Maintenance 選單維護（TipTop p_zm）
        #[route("/azz/menu-maintenance")]
        MenuMaintenance {},
    #[end_layout]
    #[redirect("/aim/items", || Route::ItemList {})]
    #[redirect("/aoo/classification-codes", || Route::ClassificationCodeMaintenance {})]
    #[redirect("/azz/user-permissions", || Route::UserPermissions {})]
    #[redirect("/azz/user-permissions/:user", |user: String| Route::UserPermissionsFor { user })]
    #[redirect("/azz/permission-classes", || Route::PermissionClassMaintenance {})]
    #[redirect("/azz/program-permissions", || Route::ProgramPermissions {})]
    #[redirect("/azz/program-permissions/:class", |class: String| Route::ProgramPermissionsFor { class })]
    #[route("/:..segments")]
    PageNotFound { segments: Vec<String> },
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
    // 選單維護變更後側欄即時更新（MN15）
    components::menu::provide_menu_version();
    rsx! {
        // 不要由瀏覽器翻譯（Chrome）：譯文會蓋掉畫面更新；語系由 i18n::set 設在 <html lang>
        document::Meta { name: "google", content: "notranslate" }
        document::Link { rel: "stylesheet", href: M3_COLORS_CSS }
        document::Link { rel: "stylesheet", href: M3_CSS }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

/// 側欄節點的圖示：模組群組與作業依模組著色（aim 第三色、aoo 綠、azz 藍），選取中的作業與文字同色
#[component]
fn NavIcon(path: &'static str, tone: Tone) -> Element {
    rsx! {
        svg { class: "nav-icon {tone.icon_class()}", view_box: "0 0 24 24", fill: "currentColor", "aria-hidden": "true",
            path { d: path }
        }
    }
}

/// 程式代號 → 畫面、圖示、色調（MN12、MN13）。不在此表的程式（尚未移植）不顯示
fn screen(program: &str) -> Option<(Route, &'static str, Tone)> {
    Some(match program {
        "item-maintenance" => (Route::ItemList {}, icons::CATEGORY, Tone::Tertiary),
        "plant-maintenance" => (Route::PlantMaintenance {}, icons::FACTORY, Tone::Success),
        "department-maintenance" => (
            Route::DepartmentMaintenance {},
            icons::ACCOUNT_TREE,
            Tone::Success,
        ),
        "employee-maintenance" => (
            Route::EmployeeMaintenance {},
            icons::GROUP_PEOPLE,
            Tone::Success,
        ),
        "classification-code-maintenance" => (
            Route::ClassificationCodeMaintenance {},
            icons::LABEL,
            Tone::Success,
        ),
        "program-maintenance" => (Route::ProgramMaintenance {}, icons::APPS, Tone::Primary),
        "data-dictionary" => (Route::DataDictionary {}, icons::MENU_BOOK, Tone::Primary),
        "user-maintenance" => (Route::UserMaintenance {}, icons::PERSON, Tone::Primary),
        "user-permission-maintenance" => (
            Route::UserPermissions {},
            icons::VERIFIED_USER,
            Tone::Primary,
        ),
        "permission-class-maintenance" => (
            Route::PermissionClassMaintenance {},
            icons::SHIELD,
            Tone::Primary,
        ),
        "program-permission-maintenance" => {
            (Route::ProgramPermissions {}, icons::KEY, Tone::Primary)
        }
        "menu-maintenance" => (Route::MenuMaintenance {}, icons::LIST, Tone::Primary),
        _ => return None,
    })
}

/// 目錄節點的圖示與色調：依第一個程式的模組（aim 第三色、aoo 綠、azz 藍）；混合或無法判斷時用資料夾
fn group_icon(node: &ttrs_shared::MenuNodeDto) -> (&'static str, Tone) {
    fn first_module(n: &ttrs_shared::MenuNodeDto) -> Option<String> {
        if !n.is_menu() {
            return Some(n.module_code.to_uppercase());
        }
        n.children.iter().find_map(first_module)
    }
    match first_module(node).as_deref() {
        Some("AIM") => (icons::INVENTORY, Tone::Tertiary),
        Some("AOO") => (icons::PLANT, Tone::Success),
        Some("AZZ") => (icons::SETTINGS, Tone::Primary),
        _ => (icons::FOLDER, Tone::Neutral),
    }
}

/// 側欄樹狀選單（cl_create_4sm 的 StartMenu）：由 API 依使用者的選單與權限組成
#[component]
fn SideMenu() -> Element {
    let version = use_context::<components::menu::MenuVersion>().0;
    // 選單維護變更時版本加 1，重新取得（MN15）；重新載入期間保留上一份，側欄不閃爍
    let tree = use_resource(move || {
        let _ = version();
        api::menu_tree()
    });
    let mut last = use_signal(Vec::<ttrs_shared::MenuNodeDto>::new);
    // 收合的目錄（記在瀏覽器）
    use_context_provider(|| {
        components::menu::Collapsed(Signal::new(components::menu::load_collapsed()))
    });
    let mut error = use_signal(|| None::<String>);
    use_effect(move || match &*tree.read() {
        Some(Ok(nodes)) => {
            error.set(None);
            last.set(nodes.clone());
        }
        Some(Err(e)) => error.set(Some(e.clone())),
        None => {}
    });
    rsx! {
        if let Some(e) = error() {
            p { class: "session-error", "{e}" }
        }
        MenuNodes { nodes: last(), level: 0 }
    }
}

/// 目錄底下（含下層）是否有目前所在的作業
fn contains_route(node: &ttrs_shared::MenuNodeDto, current: &Route) -> bool {
    node.children.iter().any(|c| {
        if c.is_menu() {
            contains_route(c, current)
        } else {
            screen(&c.program_no).is_some_and(|(r, ..)| &r == current)
        }
    })
}

/// 樹狀節點：目錄可點選展開／收合（記在瀏覽器）；目前所在的作業所屬目錄一律展開
#[component]
fn MenuNodes(nodes: Vec<ttrs_shared::MenuNodeDto>, level: u8) -> Element {
    let mut collapsed = use_context::<components::menu::Collapsed>().0;
    let current = use_route::<Route>();
    rsx! {
        for n in nodes {
            if n.is_menu() {
                {
                    let (path, tone) = group_icon(&n);
                    let open = !collapsed.read().contains(&n.program_no) || contains_route(&n, &current);
                    let no = n.program_no.clone();
                    rsx! {
                        button {
                            key: "{n.program_no}",
                            r#type: "button",
                            class: "group group-toggle nav-level-{level}",
                            aria_expanded: open,
                            onclick: move |_| {
                                let mut set = collapsed.write();
                                if !set.remove(&no) {
                                    set.insert(no.clone());
                                }
                                components::menu::save_collapsed(&set);
                            },
                            NavIcon { path, tone }
                            span { "{n.name}" }
                            svg { class: if open { "chevron chevron--open" } else { "chevron" }, view_box: "0 0 24 24", fill: "currentColor", "aria-hidden": "true",
                                path { d: icons::CHEVRON_RIGHT }
                            }
                        }
                        if open {
                            MenuNodes { nodes: n.children.clone(), level: level.saturating_add(1) }
                        }
                    }
                }
            } else if let Some((to, path, tone)) = screen(&n.program_no) {
                Link { key: "{n.program_no}", class: "nav-level-{level}", active_class: "active", to, NavIcon { path, tone } span { "{n.name}" } }
            }
        }
    }
}

/// 找不到的網址回首頁
#[component]
fn PageNotFound(segments: Vec<String>) -> Element {
    let _ = segments;
    let nav = use_navigator();
    nav.replace(Route::Home {});
    rsx! {}
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
                // 首頁固定在最上方（MN15）；其餘依目錄資料組成（menu_maintenance.feature MN9–MN11）
                Link { active_class: "active", to: Route::Home {}, NavIcon { path: icons::HOME, tone: Tone::Primary } span { {t("shell-nav-home")} } }
                SideMenu { key: "{current.code()}" }
            }
            // 以語系為 key：切換語系時重新載入畫面，所有文字改用新語系
            for l in [current] {
                main { key: "{l.code()}", class: "content", Outlet::<Route> {} }
            }
        }
    }
}
