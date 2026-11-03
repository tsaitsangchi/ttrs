//! User Special Permission Maintenance 使用者特殊權限維護（TipTop p_zxw 使用者權限類別設定作業）
//! 版面比照 tiptop/azz/4fd/p_zxw.4fd（X11）：表頭（使用者、名稱）＋使用者切換
//!   ＋ 特殊權限表格（項次、分類、權限類別或程式、名稱、允許執行功能、他人／他部門資料權限、單身權限）
//!   ＋ 小視窗：新增（第一次可加入原權限類別，X1）、修改程式列、複製、刪除全部。
//! 有特殊權限的使用者只用特殊權限；規則在後端 UserPermissionService，實際檢查見 ADR 0006。

use dioxus::prelude::*;
use ttrs_shared::{SpecialPermissionKindDto, UserDto, UserPermissionDto, UserPermissionInput};

use super::program_permission_maintenance::{
    DEPT_LEVELS, DETAIL_LEVELS, USER_LEVELS, action_label, level_label, level_options,
};
use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Dialog, IconButton, Notification, NotificationKind,
    Select, TextField, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::i18n::{t, tf};

/// 目前開啟的小視窗
#[derive(Clone, PartialEq)]
enum Open {
    Add,
    Edit(Box<UserPermissionDto>),
    Copy,
    DeleteAll,
}

fn kind_label(kind: SpecialPermissionKindDto) -> String {
    match kind {
        SpecialPermissionKindDto::Class => t("uperm-kind-class"),
        SpecialPermissionKindDto::Program => t("uperm-kind-program"),
    }
}

fn level_cell(kind: &str, code: &Option<String>) -> String {
    code.as_deref()
        .map(|c| format!("{c}  {}", level_label(kind, c)))
        .unwrap_or_default()
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "p_zxw";

#[component]
pub fn UserPermissions() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, UserPermissionMain { initial: None } }
    }
}

/// 由使用者維護（p_zx「維護使用者特殊權限」）帶入使用者
#[component]
pub fn UserPermissionsFor(user: String) -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, UserPermissionMain { initial: Some(user) } }
    }
}

#[component]
fn UserPermissionMain(initial: Option<String>) -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let users = use_resource(api::list_users);
    let mut user = use_signal(|| initial.clone());
    let mut rows_res = use_resource(move || {
        let id = user();
        async move {
            match id {
                Some(u) => api::list_user_permissions(&u).await,
                None => Ok(Vec::new()),
            }
        }
    });
    let mut selected = use_signal(|| None::<i32>);
    let mut open = use_signal(|| None::<Open>);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut add_form = use_signal(|| UserPermissionInput {
        include_own_class: true,
        ..Default::default()
    });
    let mut form = use_signal(UserPermissionInput::default);
    let mut target_user = use_signal(String::new);

    let all_users: Vec<UserDto> = users
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    // 未指定使用者時停在第一位
    use_effect(move || {
        if user.peek().is_none()
            && let Some(Ok(list)) = &*users.read()
            && let Some(first) = list.first()
        {
            user.set(Some(first.user_id.clone()));
        }
    });

    let id = user().unwrap_or_default();
    let cur_user = all_users.iter().find(|u| u.user_id == id).cloned();
    let pos = all_users.iter().position(|u| u.user_id == id);
    let total = all_users.len();
    let rows: Vec<UserPermissionDto> = rows_res
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let cur: Option<UserPermissionDto> = selected()
        .and_then(|n| rows.iter().find(|r| r.line_no == n).cloned())
        .or_else(|| rows.first().cloned());
    let no_user = cur_user.is_none();
    let no_cur = cur.is_none();
    let cur_is_program = cur
        .as_ref()
        .is_some_and(|r| r.kind == SpecialPermissionKindDto::Program);
    let first_row = rows.is_empty();

    let mut close = move || {
        open.set(None);
        dialog_error.set(None);
    };
    let mut done = move |msg: String| {
        open.set(None);
        dialog_error.set(None);
        error.set(None);
        message.set(Some(msg));
        rows_res.restart();
    };
    let mut go = move |i: usize, list: Vec<UserDto>| {
        if let Some(u) = list.get(i) {
            selected.set(None);
            message.set(None);
            error.set(None);
            user.set(Some(u.user_id.clone()));
        }
    };

    let user_options: Vec<(String, String)> = all_users
        .iter()
        .map(|u| {
            (
                u.user_id.clone(),
                format!("{}  {}", u.user_id, u.name.clone().unwrap_or_default()),
            )
        })
        .collect();
    let target_options: Vec<(String, String)> =
        std::iter::once((String::new(), t("perm-opt-choose")))
            .chain(all_users.iter().filter(|u| u.user_id != id).map(|u| {
                (
                    u.user_id.clone(),
                    format!("{}  {}", u.user_id, u.name.clone().unwrap_or_default()),
                )
            }))
            .collect();
    let kind_options = vec![
        ("class".to_owned(), t("uperm-kind-class")),
        ("program".to_owned(), t("uperm-kind-program")),
    ];

    let all_nav = all_users.clone();
    let all_prev = all_users.clone();
    let all_next = all_users.clone();
    let all_last = all_users.clone();
    let cur_edit = cur.clone();
    let cur_remove = cur.clone();
    let a = add_form();
    let v = form();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("uperm-breadcrumb")} }
                h2 { {t("uperm-title")} }
            }

            // ----- 動作列 -----
            div { class: "action-bar",
                if perm.add_row() {
                    Button {
                        disabled: no_user,
                        onclick: move |_| {
                            message.set(None);
                            add_form.set(UserPermissionInput { include_own_class: true, ..Default::default() });
                            open.set(Some(Open::Add));
                        },
                        {t("common-action-new")}
                    }
                }
                if perm.can("detail") {
                    Button {
                        variant: ButtonVariant::Tonal,
                        disabled: !cur_is_program,
                        onclick: move |_| {
                            let Some(r) = cur_edit.clone() else { return };
                            message.set(None);
                            form.set(r.to_input());
                            open.set(Some(Open::Edit(Box::new(r))));
                        },
                        {t("common-action-edit")}
                    }
                }
                if perm.delete_row() {
                    Button {
                        variant: ButtonVariant::Error,
                        disabled: no_cur,
                        onclick: move |_| {
                            let Some(r) = cur_remove.clone() else { return };
                            spawn(async move {
                                match api::remove_user_permission(&r.user_id, r.line_no).await {
                                    Ok(()) => {
                                        selected.set(None);
                                        done(tf("uperm-msg-removed", &[("line", r.line_no.to_string().as_str())]));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        },
                        {t("perm-action-remove")}
                    }
                }
                if perm.can("delete") {
                    Button { variant: ButtonVariant::Outlined, disabled: no_user, onclick: move |_| { message.set(None); open.set(Some(Open::DeleteAll)); }, {t("perm-action-delete-all")} }
                }
                div { class: "spacer" }
                if perm.can("reproduce") {
                    Button { variant: ButtonVariant::Outlined, disabled: no_user, onclick: move |_| { message.set(None); target_user.set(String::new()); open.set(Some(Open::Copy)); }, {t("common-action-copy")} }
                }
            }

            // ----- 使用者切換 -----
            if total > 0 {
                div { class: "record-nav",
                    IconButton { label: t("nav-first"), path: icons::FIRST_PAGE, disabled: pos == Some(0), onclick: move |_| go(0, all_nav.clone()) }
                    IconButton { label: t("nav-previous"), path: icons::CHEVRON_LEFT, disabled: pos.is_none_or(|i| i == 0), onclick: move |_| go(pos.unwrap_or(0).saturating_sub(1), all_prev.clone()) }
                    span { class: "record-nav__count",
                        match pos {
                            Some(i) => rsx! { {tf("nav-record-position", &[("pos", (i + 1).to_string().as_str()), ("total", total.to_string().as_str())])} },
                            None => rsx! { {tf("nav-record-count", &[("total", total.to_string().as_str())])} },
                        }
                    }
                    IconButton { label: t("nav-next"), path: icons::CHEVRON_RIGHT, disabled: pos.is_some_and(|i| i + 1 >= total), onclick: move |_| go(pos.map_or(0, |i| i + 1), all_next.clone()) }
                    IconButton { label: t("nav-last"), path: icons::LAST_PAGE, disabled: pos == Some(total.saturating_sub(1)), onclick: move |_| go(total.saturating_sub(1), all_last.clone()) }
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            // ----- 表頭（p_zxw.4fd group01） -----
            Card {
                div { class: "card-body",
                    div { class: "form-grid cols-4",
                        Select { id: "zxw01", label: t("uperm-field-user"), options: user_options, value: id.clone(), onchange: move |x: String| { selected.set(None); message.set(None); user.set(Some(x)); } }
                        TextField { id: "zx02", label: t("uperm-field-user-name"), readonly: true, value: cur_user.as_ref().and_then(|u| u.name.clone()).unwrap_or_default() }
                        TextField { id: "zx04", label: t("uperm-field-user-class"), readonly: true, value: cur_user.as_ref().map(|u| u.permission_class.clone()).unwrap_or_default() }
                        TextField { id: "uperm-state", label: t("uperm-field-state"), readonly: true, value: if rows.is_empty() { t("uperm-state-class") } else { t("uperm-state-special") } }
                    }
                }
            }

            // ----- 特殊權限（p_zxw.4fd s_zxw） -----
            Card {
                div { class: "table-header",
                    h3 { {t("uperm-list-title")} }
                    p { {tf("list-count-all", &[("total", rows.len().to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for col in ["uperm-field-line", "uperm-field-kind", "uperm-field-target", "uperm-field-target-name", "perm-field-actions", "perm-field-other-user", "perm-field-other-dept", "perm-field-detail"] {
                                th { key: "{col}", {t(col)} }
                            }
                        } }
                        tbody {
                            match &*rows_res.read() {
                                None => rsx! { tr { td { colspan: 8, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 8, "{e}" } } },
                                Some(Ok(_)) if rows.is_empty() => rsx! { tr { td { colspan: 8, class: "muted", {t("uperm-msg-none")} } } },
                                Some(Ok(_)) => rsx! {
                                    for r in rows.iter().cloned() {
                                        tr {
                                            key: "{r.line_no}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|c| c.line_no == r.line_no) { "md-table__row--selected" },
                                            onclick: {
                                                let n = r.line_no;
                                                move |_| { message.set(None); error.set(None); selected.set(Some(n)); }
                                            },
                                            td { "{r.line_no}" }
                                            td { {kind_label(r.kind)} }
                                            td { class: "mono", "{r.target}" }
                                            td { {r.target_name.clone().unwrap_or_default()} }
                                            td { {r.actions.iter().map(|a| action_label(a)).collect::<Vec<_>>().join(", ")} }
                                            td { {level_cell("user", &r.other_user_data)} }
                                            td { {level_cell("dept", &r.other_dept_data)} }
                                            td { {level_cell("detail", &r.detail_rows)} }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }
            p { class: "note", {t("uperm-note")} }

            // ----- 小視窗 -----
            match open() {
                None => rsx! {},
                Some(Open::Add) => rsx! {
                    Dialog {
                        title: tf("uperm-dialog-add", &[("user", id.as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                onclick: move |_| {
                                    let uid = user().unwrap_or_default();
                                    let req = add_form();
                                    spawn(async move {
                                        match api::add_user_permission(&uid, &req).await {
                                            Ok(list) => {
                                                selected.set(list.last().map(|r| r.line_no));
                                                done(tf("uperm-msg-added", &[("target", req.target.as_str())]));
                                            }
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
                            Select {
                                id: "zxw03",
                                label: t("uperm-field-kind"),
                                options: kind_options.clone(),
                                value: if a.kind == SpecialPermissionKindDto::Program { "program" } else { "class" },
                                onchange: move |x: String| add_form.write().kind = if x == "program" { SpecialPermissionKindDto::Program } else { SpecialPermissionKindDto::Class },
                            }
                            TextField { id: "zxw04", label: t("uperm-field-target"), value: a.target.clone(), oninput: move |x| add_form.write().target = x }
                        }
                        if first_row {
                            Checkbox { id: "include-own", label: tf("uperm-field-include-own", &[("class", cur_user.as_ref().map(|u| u.permission_class.as_str()).unwrap_or(""))]), checked: a.include_own_class, onchange: move |x| add_form.write().include_own_class = x }
                            p { class: "muted", {t("uperm-hint-first")} }
                        }
                        p { class: "muted", {t("uperm-hint-add")} }
                    }
                },
                Some(Open::Edit(r)) => rsx! {
                    Dialog {
                        title: tf("uperm-dialog-edit", &[("user", r.user_id.as_str()), ("program", r.target.as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                onclick: {
                                    let r = r.clone();
                                    move |_| {
                                        let r = r.clone();
                                        let req = form();
                                        spawn(async move {
                                            match api::update_user_program_permission(&r.user_id, &r.target, &req).await {
                                                Ok(_) => done(tf("perm-msg-saved", &[("program", r.target.as_str())])),
                                                Err(e) => dialog_error.set(Some(e)),
                                            }
                                        });
                                    }
                                },
                                {t("common-action-ok")}
                            }
                        },
                        if let Some(msg) = dialog_error() {
                            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                        }
                        h3 { class: "dialog-section", {t("perm-field-actions")} }
                        if r.available_actions.is_empty() {
                            p { class: "muted", {t("perm-hint-no-actions")} }
                        } else {
                            div { class: "action-checks",
                                for act in r.available_actions.iter().cloned() {
                                    Checkbox {
                                        key: "{act}",
                                        id: "uact-{act}",
                                        label: format!("{}  ({act})", action_label(&act)),
                                        checked: v.actions.as_ref().is_some_and(|x| x.contains(&act)),
                                        onchange: {
                                            let act = act.clone();
                                            move |on: bool| {
                                                let mut f = form.write();
                                                let list = f.actions.get_or_insert_with(Vec::new);
                                                list.retain(|x| x != &act);
                                                if on {
                                                    list.push(act.clone());
                                                }
                                            }
                                        },
                                    }
                                }
                            }
                            div { class: "action-bar",
                                Button { variant: ButtonVariant::Text, onclick: { let all = r.available_actions.clone(); move |_| form.write().actions = Some(all.clone()) }, {t("perm-action-select-all")} }
                                Button { variant: ButtonVariant::Text, onclick: move |_| form.write().actions = Some(Vec::new()), {t("perm-action-select-none")} }
                            }
                        }
                        div { class: "form-grid",
                            Select { id: "zxw06", label: t("perm-field-other-user"), options: level_options("user", &USER_LEVELS), value: v.other_user_data.clone().unwrap_or_default(), onchange: move |x| form.write().other_user_data = Some(x) }
                            Select { id: "zxw07", label: t("perm-field-other-dept"), options: level_options("dept", &DEPT_LEVELS), value: v.other_dept_data.clone().unwrap_or_default(), onchange: move |x| form.write().other_dept_data = Some(x) }
                            Select { id: "zxw08", label: t("perm-field-detail"), options: level_options("detail", &DETAIL_LEVELS), value: v.detail_rows.clone().unwrap_or_default(), onchange: move |x| form.write().detail_rows = Some(x) }
                        }
                    }
                },
                Some(Open::Copy) => rsx! {
                    Dialog {
                        title: tf("uperm-dialog-copy", &[("user", id.as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                onclick: move |_| {
                                    let from = user().unwrap_or_default();
                                    let to = target_user();
                                    spawn(async move {
                                        match api::copy_user_permissions(&from, &to).await {
                                            Ok(r) => done(tf("uperm-msg-copied", &[("count", r.added.to_string().as_str()), ("target", to.as_str())])),
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
                        Select { id: "copy-user", label: t("uperm-field-target-user"), options: target_options.clone(), value: target_user(), onchange: move |x| target_user.set(x) }
                        p { class: "muted", {t("uperm-hint-copy")} }
                    }
                },
                Some(Open::DeleteAll) => rsx! {
                    Dialog {
                        title: tf("uperm-dialog-delete-all", &[("user", id.as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                variant: ButtonVariant::Error,
                                onclick: move |_| {
                                    let uid = user().unwrap_or_default();
                                    spawn(async move {
                                        match api::remove_all_user_permissions(&uid).await {
                                            Ok(r) => {
                                                selected.set(None);
                                                done(tf("perm-msg-removed-count", &[("count", r.removed.to_string().as_str())]));
                                            }
                                            Err(e) => dialog_error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("common-action-delete")}
                            }
                        },
                        if let Some(msg) = dialog_error() {
                            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                        }
                        p { {t("uperm-confirm-delete-all")} }
                    }
                },
            }
        }
    }
}
