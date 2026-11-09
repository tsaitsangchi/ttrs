//! Unit Maintenance 單位維護（TipTop aooi101 單位資料維護）
//! 版面比照 tiptop/aoo/4fd/aooi101.4fd（UN1）：多筆單身表格（單位、說明、小數位數），新增／修改以小視窗編輯。
//! 共用層：全公司共用（UN2）。規則在後端 UnitService。

use dioxus::prelude::*;
use ttrs_shared::{UnitDto, UnitInput, unit_max_len as max_len};

use crate::api;
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Tabs, TextField, Tone,
    icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::status::{
    self, HasStatus, StatusCells, StatusHeaders, TAB_DATA, TAB_STATUS,
};
use crate::i18n::{t, tf};

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "unit-maintenance";

#[component]
pub fn UnitMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, UnitPage {} }
    }
}

#[component]
fn UnitPage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut units = use_resource(api::list_units);
    let mut selected = use_signal(|| None::<String>);
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(UnitInput::default);
    let mut dialog_error = use_signal(|| None::<String>);

    let list: Vec<UnitDto> = units
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let cur: Option<UnitDto> = selected()
        .and_then(|c| list.iter().find(|x| x.unit_code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab { 2 + status::ALL.len() } else { 3 };

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        units.restart();
    };

    let save = move |_| {
        let target = editor();
        let req = form();
        spawn(async move {
            let result = match target {
                Some(Editor::Edit(code)) => api::update_unit(&code, &req).await,
                _ => api::create_unit(&req).await,
            };
            match result {
                Ok(u) => done(
                    tf("unit-msg-saved", &[("code", u.unit_code.as_str())]),
                    Some(u.unit_code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_edit = cur.clone();
    let cur_delete = cur.clone();
    let cur_toggle = cur.clone();
    let inactive = cur.as_ref().is_some_and(|c| !c.active);
    let v = form();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("unit-breadcrumb")} }
                h2 { {t("unit-title")} }
            }

            div { class: "action-bar",
                if perm.add_row() {
                    Button {
                        icon: icons::ADD, tone: Tone::Success,
                        onclick: move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(UnitInput::default());
                            editor.set(Some(Editor::New));
                        },
                        {t("common-action-new")}
                    }
                }
                if perm.can("detail") {
                    Button {
                        icon: icons::EDIT, tone: Tone::Tertiary,
                        variant: ButtonVariant::Tonal,
                        disabled: no_cur,
                        onclick: move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.unit_code)));
                        },
                        {t("common-action-edit")}
                    }
                }
                if perm.can("history") {
                    HistoryButton { program: PROGRAM, record: cur.as_ref().map(|c| c.unit_code.clone()) }
                }
                if perm.delete_row() {
                    Button {
                        icon: icons::DELETE, tone: Tone::Error,
                        variant: ButtonVariant::Error,
                        disabled: no_cur,
                        onclick: move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match api::delete_unit(&c.unit_code, c.version).await {
                                    Ok(()) => done(tf("unit-msg-deleted", &[("code", c.unit_code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        },
                        {t("common-action-delete")}
                    }
                }
                if perm.can("detail") {
                    Button {
                        icon: if inactive { icons::CHECK_CIRCLE } else { icons::BLOCK },
                        tone: if inactive { Tone::Success } else { Tone::Error },
                        variant: ButtonVariant::Outlined,
                        disabled: no_cur,
                        onclick: move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            spawn(async move {
                                match api::set_unit_active(&c.unit_code, !c.active, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "unit-msg-activated" } else { "unit-msg-deactivated" };
                                        done(tf(id, &[("code", r.unit_code.as_str())]), Some(r.unit_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        },
                        if inactive { {t("unit-action-activate")} } else { {t("unit-action-deactivate")} }
                    }
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            Card {
                // 資料／狀態頁籤（ADR 0012 §9）
                Tabs { tabs: vec![t(TAB_DATA), t(TAB_STATUS)], active: col_tab }
                div { class: "table-header",
                    h3 { {t("unit-list-title")} }
                    p { {tf("list-count-all", &[("total", list.len().to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("unit-field-code")} }
                                th { {t("unit-field-name")} }
                                if status_tab {
                                    StatusHeaders {}
                                } else {
                                    th { {t("unit-field-decimals")} }
                                }
                            }
                        }
                        tbody {
                            match &*units.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for u in list.iter().cloned() {
                                        tr {
                                            key: "{u.unit_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.unit_code == u.unit_code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = u.unit_code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{u.unit_code}" }
                                            td { {u.name.clone().unwrap_or_default()} }
                                            if status_tab {
                                                StatusCells { status: u.status() }
                                            } else {
                                                td { class: "num", "{u.decimals}" }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }

            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("unit-dialog-new"),
                        Editor::Edit(code) => tf("unit-dialog-edit", &[("code", code.as_str())]),
                    },
                    oncancel: move |_| { editor.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { editor.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    div { class: "form-grid",
                        TextField { id: "gfe01", label: t("unit-field-code"), maxlength: max_len::CODE, value: v.unit_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().unit_code = x }
                        TextField { id: "gfe02", label: t("unit-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                        TextField {
                            id: "gfe03",
                            label: t("unit-field-decimals"),
                            input_type: "number",
                            value: v.decimals.map(|n| n.to_string()).unwrap_or_default(),
                            oninput: move |x: String| form.write().decimals = x.trim().parse().ok(),
                        }
                    }
                    p { class: "muted", {t("unit-hint-decimals")} }
                }
            }
        }
    }
}
