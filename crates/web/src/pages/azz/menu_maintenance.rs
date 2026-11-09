//! Menu Maintenance 選單維護（TipTop p_zm 系統目錄資料建立作業）
//! 版面比照 tiptop/azz/4fd/p_zm.4fd（MN1）：單頭目錄代號＋名稱（以下拉切換目錄），單身序號、程式代號、程式名稱。
//! 單身新增以小視窗輸入；上移／下移、重新編號、移除、複製、刪除目錄、異動記錄。規則在後端 MenuService。
//! 側欄依目錄組成，目錄異動後重新整理頁面即生效（MN15）。

use dioxus::prelude::*;
use ttrs_shared::{MenuItemDto, MenuItemInput};

use crate::api;
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Chip, Dialog, IconButton, Notification, NotificationKind, Select,
    TextField, Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::i18n::{t, tf};

/// 權限檢查用的程式代號
const PROGRAM: &str = "menu-maintenance";

/// 小視窗：加入單身、複製目錄
#[derive(Clone, PartialEq)]
enum Open {
    Add,
    Copy,
}

#[component]
pub fn MenuMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, MenuMaintenancePage {} }
    }
}

#[component]
fn MenuMaintenancePage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut menu = use_signal(String::new);
    let menus = use_resource(api::list_menus);
    let mut items = use_resource(move || {
        let m = menu();
        async move {
            if m.is_empty() {
                Ok(Vec::new())
            } else {
                api::menu_items(&m).await
            }
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut open = use_signal(|| None::<Open>);
    let mut add_program = use_signal(String::new);
    let mut add_seq = use_signal(String::new);
    let mut copy_to = use_signal(String::new);
    let mut dialog_error = use_signal(|| None::<String>);

    // 預設選第一個目錄
    let menu_list = menus
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    if menu().is_empty()
        && let Some(first) = menu_list.first()
    {
        menu.set(first.menu_no.clone());
    }
    let rows: Vec<MenuItemDto> = items
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let cur = selected()
        .and_then(|p| rows.iter().find(|r| r.program_no == p).cloned())
        .or_else(|| rows.first().cloned());
    let no_cur = cur.is_none();
    let no_menu = menu().is_empty();
    let menu_options: Vec<(String, String)> = menu_list
        .iter()
        .map(|m| {
            let label = match &m.name {
                Some(n) => format!("{} {n}", m.menu_no),
                None => m.menu_no.clone(),
            };
            (m.menu_no.clone(), label)
        })
        .collect();

    let mut done = move |msg: String| {
        error.set(None);
        dialog_error.set(None);
        open.set(None);
        message.set(Some(msg));
        items.restart();
    };
    let mut close = move || {
        open.set(None);
        dialog_error.set(None);
    };

    let cur_remove = cur.clone();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("menu-breadcrumb")} }
                h2 { {t("menu-title")} }
            }

            div { class: "action-bar",
                if perm.add_row() {
                    Button {
                        icon: icons::ADD, tone: Tone::Success, disabled: no_menu,
                        onclick: move |_| { add_program.set(String::new()); add_seq.set(String::new()); dialog_error.set(None); open.set(Some(Open::Add)); },
                        {t("menu-action-add")}
                    }
                }
                if perm.delete_row() {
                    Button {
                        icon: icons::REMOVE, tone: Tone::Error, variant: ButtonVariant::Outlined, disabled: no_cur,
                        onclick: move |_| {
                            let Some(r) = cur_remove.clone() else { return };
                            let m = menu();
                            spawn(async move {
                                match api::remove_menu_item(&m, &r.program_no).await {
                                    Ok(()) => { selected.set(None); done(tf("menu-msg-removed", &[("program", r.program_no.as_str())])); }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        },
                        {t("menu-action-remove")}
                    }
                }
                if perm.can("detail") {
                    Button {
                        icon: icons::SYNC, tone: Tone::Primary, variant: ButtonVariant::Outlined, disabled: no_menu,
                        onclick: move |_| {
                            let m = menu();
                            spawn(async move {
                                match api::renumber_menu(&m).await {
                                    Ok(_) => done(t("menu-msg-renumbered")),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        },
                        {t("menu-action-renumber")}
                    }
                }
                if perm.can("reproduce") {
                    Button {
                        icon: icons::COPY, tone: Tone::Primary, variant: ButtonVariant::Outlined, disabled: no_menu,
                        onclick: move |_| { copy_to.set(String::new()); dialog_error.set(None); open.set(Some(Open::Copy)); },
                        {t("common-action-copy")}
                    }
                }
                if perm.can("delete") {
                    Button {
                        icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Error, disabled: no_menu,
                        onclick: move |_| {
                            let m = menu();
                            spawn(async move {
                                match api::delete_menu(&m).await {
                                    Ok(()) => done(tf("menu-msg-deleted", &[("menu", m.as_str())])),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        },
                        {t("menu-action-delete")}
                    }
                }
                if perm.can("history") {
                    HistoryButton { program: PROGRAM, record: Some(menu()).filter(|m| !m.is_empty()) }
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            // ----- 單頭（p_zm.4fd grid01：目錄代號 zm01、名稱 gaz03h） -----
            Card {
                div { class: "card-body",
                    div { class: "form-grid",
                        Select {
                            id: "zm01",
                            label: t("menu-field-menu"),
                            options: menu_options,
                            value: menu(),
                            onchange: move |v: String| { selected.set(None); message.set(None); menu.set(v); },
                        }
                    }
                    p { class: "muted", {t("menu-hint-menu")} }
                }
            }

            // ----- 單身（p_zm.4fd s_zm：序號 zm03、程式代號 zm04、名稱 gaz03） -----
            Card {
                div { class: "table-header",
                    h3 { {t("menu-list-title")} }
                    p { {tf("list-count-all", &[("total", rows.len().to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            th { {t("menu-field-seq")} }
                            th { {t("menu-field-program")} }
                            th { {t("menu-field-name")} }
                            th { {t("menu-field-kind")} }
                            th { {t("menu-field-order")} }
                        } }
                        tbody {
                            if rows.is_empty() {
                                tr { td { colspan: 5, class: "muted", {t("common-msg-no-data")} } }
                            }
                            for r in rows.iter().cloned() {
                                tr {
                                    key: "{r.program_no}",
                                    class: "md-table__row--clickable",
                                    class: if cur.as_ref().is_some_and(|c| c.program_no == r.program_no) { "md-table__row--selected" },
                                    onclick: {
                                        let p = r.program_no.clone();
                                        move |_| { message.set(None); error.set(None); selected.set(Some(p.clone())); }
                                    },
                                    td { class: "mono", "{r.seq}" }
                                    td { class: "mono", "{r.program_no}" }
                                    td { {r.name.clone().unwrap_or_default()} }
                                    td {
                                        if r.is_menu {
                                            Chip { label: t("menu-kind-menu"), tone: Tone::Primary }
                                        } else {
                                            Chip { label: t("menu-kind-program"), tone: Tone::Neutral }
                                        }
                                    }
                                    td {
                                        if perm.can("detail") {
                                            IconButton {
                                                label: t("menu-action-up"), path: icons::ARROW_UPWARD,
                                                onclick: {
                                                    let p = r.program_no.clone();
                                                    move |_| {
                                                        let (m, p) = (menu(), p.clone());
                                                        spawn(async move {
                                                            match api::shift_menu_item(&m, &p, true).await {
                                                                Ok(_) => { selected.set(Some(p)); items.restart(); }
                                                                Err(e) => error.set(Some(e)),
                                                            }
                                                        });
                                                    }
                                                },
                                            }
                                            IconButton {
                                                label: t("menu-action-down"), path: icons::ARROW_DOWNWARD,
                                                onclick: {
                                                    let p = r.program_no.clone();
                                                    move |_| {
                                                        let (m, p) = (menu(), p.clone());
                                                        spawn(async move {
                                                            match api::shift_menu_item(&m, &p, false).await {
                                                                Ok(_) => { selected.set(Some(p)); items.restart(); }
                                                                Err(e) => error.set(Some(e)),
                                                            }
                                                        });
                                                    }
                                                },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            p { class: "note", {t("menu-note")} }
            match open() {
                None => rsx! {},
                Some(Open::Add) => rsx! {
                    Dialog {
                        title: tf("menu-dialog-add", &[("menu", menu().as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                icon: icons::CHECK, tone: Tone::Success,
                                onclick: move |_| {
                                    let m = menu();
                                    let seq = add_seq().trim().parse::<i32>().ok();
                                    let req = MenuItemInput { program_no: add_program().trim().to_owned(), seq };
                                    spawn(async move {
                                        match api::add_menu_item(&m, &req).await {
                                            Ok(_) => { selected.set(Some(req.program_no.clone())); done(tf("menu-msg-added", &[("program", req.program_no.as_str())])); }
                                            Err(e) => dialog_error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("common-action-ok")}
                            }
                        },
                        if let Some(msg) = dialog_error() {
                            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                        }
                        div { class: "form-grid",
                            TextField { id: "zm04", label: t("menu-field-program"), value: add_program(), oninput: move |x| add_program.set(x) }
                            TextField { id: "zm03", label: t("menu-field-seq"), input_type: "number", placeholder: t("menu-hint-seq"), value: add_seq(), oninput: move |x| add_seq.set(x) }
                        }
                    }
                },
                Some(Open::Copy) => rsx! {
                    Dialog {
                        title: tf("menu-dialog-copy", &[("menu", menu().as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                icon: icons::CHECK, tone: Tone::Success,
                                onclick: move |_| {
                                    let (from, to) = (menu(), copy_to().trim().to_owned());
                                    spawn(async move {
                                        match api::copy_menu(&from, &to).await {
                                            Ok(()) => { done(tf("menu-msg-copied", &[("menu", to.as_str())])); menu.set(to); }
                                            Err(e) => dialog_error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("common-action-ok")}
                            }
                        },
                        if let Some(msg) = dialog_error() {
                            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                        }
                        TextField { id: "new-zm01", label: t("menu-field-new-menu"), value: copy_to(), oninput: move |x| copy_to.set(x) }
                        p { class: "muted", {t("menu-hint-copy")} }
                    }
                },
            }
        }
    }
}
