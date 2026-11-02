//! Permission Class Maintenance 權限類別維護（TipTop p_zw 權限類別建立作業）
//! 版面比照 tiptop/azz/4fd/p_zw.4fd（W11）：單一表格，欄位與順序同 4fd
//!   （權限類別、權限說明、使用目錄編號、閒置設定、閒置處理、閒置時間、有效）＋筆數；
//!   新增／修改以小視窗編輯；動作列另有停用・啟用與「更新所有使用者目錄編號」（需確認）。
//! 規則在後端 PermissionClassService。

use dioxus::prelude::*;
use ttrs_shared::{
    ClassIdleControlDto, IdleActionDto, PermissionClassDto, PermissionClassInput,
    permission_class_max_len as max_len,
};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Chip, Dialog, Notification, NotificationKind, Select, TextField,
};
use crate::i18n::{t, tf};

/// 編輯視窗：新增或修改某代號
#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

const IDLE_CONTROLS: [(ClassIdleControlDto, &str, &str); 3] = [
    (ClassIdleControlDto::Off, "off", "class-opt-idle-off"),
    (
        ClassIdleControlDto::ByClass,
        "class",
        "class-opt-idle-class",
    ),
    (
        ClassIdleControlDto::BySystem,
        "system",
        "class-opt-idle-system",
    ),
];

const IDLE_ACTIONS: [(IdleActionDto, &str, &str); 3] = [
    (IdleActionDto::WarnOnly, "warn", "class-opt-idle-warn"),
    (
        IdleActionDto::WarnThenClose,
        "warn_close",
        "class-opt-idle-warn-close",
    ),
    (IdleActionDto::Close, "close", "class-opt-idle-close"),
];

fn control_key(v: ClassIdleControlDto) -> &'static str {
    IDLE_CONTROLS
        .iter()
        .find(|c| c.0 == v)
        .map_or("off", |c| c.1)
}

fn control_label(v: ClassIdleControlDto) -> String {
    IDLE_CONTROLS
        .iter()
        .find(|c| c.0 == v)
        .map(|c| t(c.2))
        .unwrap_or_default()
}

fn action_key(v: Option<IdleActionDto>) -> &'static str {
    v.and_then(|v| IDLE_ACTIONS.iter().find(|a| a.0 == v))
        .map_or("", |a| a.1)
}

fn action_label(v: Option<IdleActionDto>) -> String {
    v.and_then(|v| IDLE_ACTIONS.iter().find(|a| a.0 == v))
        .map(|a| t(a.2))
        .unwrap_or_default()
}

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

#[component]
pub fn PermissionClassMaintenance() -> Element {
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut classes = use_resource(api::list_classes);
    let mut filter = use_signal(String::new);
    let mut selected = use_signal(|| None::<String>);
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(PermissionClassInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut confirm_menus = use_signal(|| false);

    let all: Vec<PermissionClassDto> = classes
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let f = filter().trim().to_lowercase();
    let list: Vec<PermissionClassDto> = all
        .iter()
        .filter(|c| {
            f.is_empty()
                || c.class_code.to_lowercase().contains(&f)
                || c.name.to_lowercase().contains(&f)
        })
        .cloned()
        .collect();
    let total = list.len();
    let cur: Option<PermissionClassDto> = selected()
        .and_then(|code| list.iter().find(|c| c.class_code == code).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();

    // 異動後重新載入並停在該代號
    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        classes.restart();
    };

    let save = move |_| {
        let req = form();
        let target = editor();
        spawn(async move {
            let result = match target {
                Some(Editor::Edit(code)) => api::update_class(&code, &req).await,
                _ => api::create_class(&req).await,
            };
            match result {
                Ok(c) => done(
                    tf("class-msg-saved", &[("code", c.class_code.as_str())]),
                    Some(c.class_code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_edit = cur.clone();
    let cur_delete = cur.clone();
    let cur_toggle = cur.clone();
    let v = form();
    let by_class = v.idle_control == Some(ClassIdleControlDto::ByClass);
    let control_options: Vec<(String, String)> = IDLE_CONTROLS
        .iter()
        .map(|c| (c.1.to_owned(), t(c.2)))
        .collect();
    let action_options: Vec<(String, String)> =
        std::iter::once((String::new(), t("class-opt-none")))
            .chain(IDLE_ACTIONS.iter().map(|a| (a.1.to_owned(), t(a.2))))
            .collect();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("class-breadcrumb")} }
                h2 { {t("class-title")} }
            }

            // ----- 動作列 -----
            div { class: "action-bar",
                Button {
                    onclick: move |_| {
                        message.set(None);
                        error.set(None);
                        dialog_error.set(None);
                        form.set(PermissionClassInput { idle_control: Some(ClassIdleControlDto::Off), ..Default::default() });
                        editor.set(Some(Editor::New));
                    },
                    {t("common-action-new")}
                }
                Button {
                    variant: ButtonVariant::Tonal,
                    disabled: no_cur,
                    onclick: move |_| {
                        let Some(c) = cur_edit.clone() else { return };
                        dialog_error.set(None);
                        form.set(c.to_input());
                        editor.set(Some(Editor::Edit(c.class_code)));
                    },
                    {t("common-action-edit")}
                }
                Button {
                    variant: ButtonVariant::Error,
                    disabled: no_cur,
                    onclick: move |_| {
                        let Some(c) = cur_delete.clone() else { return };
                        spawn(async move {
                            match api::delete_class(&c.class_code).await {
                                Ok(()) => done(tf("class-msg-deleted", &[("code", c.class_code.as_str())]), None),
                                Err(e) => error.set(Some(e)),
                            }
                        });
                    },
                    {t("common-action-delete")}
                }
                // TipTop 有效欄位：有效 → 停用、無效 → 啟用
                Button {
                    variant: ButtonVariant::Outlined,
                    disabled: no_cur,
                    onclick: move |_| {
                        let Some(c) = cur_toggle.clone() else { return };
                        spawn(async move {
                            match api::set_class_active(&c.class_code, !c.active).await {
                                Ok(r) => {
                                    let id = if r.active { "class-msg-activated" } else { "class-msg-deactivated" };
                                    done(tf(id, &[("code", r.class_code.as_str())]), Some(r.class_code));
                                }
                                Err(e) => error.set(Some(e)),
                            }
                        });
                    },
                    if cur.as_ref().is_some_and(|c| !c.active) { {t("class-action-activate")} } else { {t("class-action-deactivate")} }
                }
                // 串接 p_zy「維護群組執行權限」（p_zw.4gl:130-138）
                if let Some(c) = cur.as_ref() {
                    Link {
                        class: "md-button md-state md-button--outlined",
                        to: crate::Route::ProgramPermissionsFor { class: c.class_code.clone() },
                        {t("class-action-permissions")}
                    }
                }
                div { class: "spacer" }
                Button {
                    variant: ButtonVariant::Outlined,
                    onclick: move |_| { message.set(None); error.set(None); confirm_menus.set(true); },
                    {t("class-action-update-menus")}
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            // ----- 權限類別表格（p_zw.4fd s_zw） -----
            Card {
                div { class: "table-header",
                    h3 { {t("class-list-title")} }
                    p { {tf("list-count-all", &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "card-body",
                    TextField { id: "class-filter", class: "legacy-search", label: t("class-field-filter"), value: filter(), oninput: move |x| filter.set(x) }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for id in ["class-field-code", "class-field-name", "class-field-menu", "class-field-idle-control", "class-field-idle-action", "class-field-idle-seconds", "class-field-active"] {
                                th { key: "{id}", {t(id)} }
                            }
                        } }
                        tbody {
                            match &*classes.read() {
                                None => rsx! { tr { td { colspan: 7, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 7, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: 7, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for c in list.iter().cloned() {
                                        tr {
                                            key: "{c.class_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.class_code == c.class_code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = c.class_code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{c.class_code}" }
                                            td { "{c.name}" }
                                            td { class: "mono", {c.default_menu.clone().unwrap_or_default()} }
                                            td { {control_label(c.idle_control)} }
                                            td { {action_label(c.idle_action)} }
                                            td { {c.idle_seconds.map(|s| s.to_string()).unwrap_or_default()} }
                                            td {
                                                if c.active { Chip { label: t("class-status-active") } } else { Chip { label: t("class-status-inactive"), warning: true } }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }
            p { class: "note", {t("class-note-permissions")} }

            // ----- 新增／修改視窗 -----
            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("class-dialog-new"),
                        Editor::Edit(code) => tf("class-dialog-edit", &[("code", code.as_str())]),
                    },
                    oncancel: move |_| { editor.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { variant: ButtonVariant::Text, onclick: move |_| { editor.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { onclick: save, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    div { class: "form-grid",
                        TextField { id: "zw01", label: t("class-field-code"), maxlength: max_len::CODE, value: v.class_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().class_code = x }
                        TextField { id: "zw02", label: t("class-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                        TextField { id: "zw03", label: t("class-field-menu"), value: v.default_menu.clone().unwrap_or_default(), oninput: move |x| form.write().default_menu = opt(x) }
                        Select {
                            id: "zw04",
                            label: t("class-field-idle-control"),
                            options: control_options.clone(),
                            value: control_key(v.idle_control.unwrap_or_default()),
                            onchange: move |x: String| {
                                let control = IDLE_CONTROLS.iter().find(|c| c.1 == x).map_or(ClassIdleControlDto::Off, |c| c.0);
                                let mut f = form.write();
                                f.idle_control = Some(control);
                                // 非「依權限類別」時清空閒置處理與時間（p_zw.4gl:503-509）
                                if control != ClassIdleControlDto::ByClass {
                                    f.idle_action = None;
                                    f.idle_seconds = None;
                                }
                            },
                        }
                        Select {
                            id: "zw05",
                            label: t("class-field-idle-action"),
                            options: action_options.clone(),
                            value: action_key(v.idle_action),
                            disabled: !by_class,
                            onchange: move |x: String| form.write().idle_action = IDLE_ACTIONS.iter().find(|a| a.1 == x).map(|a| a.0),
                        }
                        TextField {
                            id: "zw06",
                            label: t("class-field-idle-seconds"),
                            input_type: "number",
                            value: v.idle_seconds.map(|s| s.to_string()).unwrap_or_default(),
                            disabled: !by_class,
                            oninput: move |x: String| form.write().idle_seconds = x.trim().parse().ok(),
                        }
                    }
                    p { class: "muted", {t("class-hint-idle")} }
                }
            }

            // ----- 更新所有使用者目錄編號（TipTop azz-001 確認） -----
            if confirm_menus() {
                Dialog {
                    title: t("class-dialog-update-menus"),
                    oncancel: move |_| confirm_menus.set(false),
                    actions: rsx! {
                        Button { variant: ButtonVariant::Text, onclick: move |_| confirm_menus.set(false), {t("common-action-cancel")} }
                        Button {
                            onclick: move |_| {
                                confirm_menus.set(false);
                                spawn(async move {
                                    match api::update_user_menus().await {
                                        Ok(n) => {
                                            error.set(None);
                                            message.set(Some(tf("class-msg-menus-updated", &[("count", n.to_string().as_str())])));
                                        }
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            {t("common-action-ok")}
                        }
                    },
                    p { {t("class-confirm-update-menus")} }
                }
            }
        }
    }
}
