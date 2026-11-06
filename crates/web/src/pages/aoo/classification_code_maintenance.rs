//! Classification Code Maintenance 分類碼維護（TipTop aooi300 分類碼共用程式）
//! 版面比照 tiptop/aoo/4fd/aooi300.4fd（CC7）：多筆單身表格（代號、說明、資料有效；類別 G 另有存貨差異科目與成本性質），
//! 上方切換碼類別（CC1）。新增／修改以小視窗編輯。工廠層：用登入的營運中心（ADR 0007）。規則在後端 ClassificationCodeService。

use dioxus::prelude::*;
use ttrs_shared::{
    ClassificationCodeDto, ClassificationCodeInput, classification_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Select, Tabs, TextField,
    Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::components::status::{
    self, HasStatus, StatusCells, StatusHeaders, TAB_DATA, TAB_STATUS,
};
use crate::i18n::{t, tf};

/// 碼類別（CC2：料件用到的類別）：（代碼, 翻譯 ID）
const KINDS: [(&str, &str); 5] = [
    ("8", "code-kind-8"),
    ("D", "code-kind-d"),
    ("E", "code-kind-e"),
    ("F", "code-kind-f"),
    ("G", "code-kind-g"),
];

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "classification-code-maintenance";

#[component]
pub fn ClassificationCodeMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ClassificationCodePage {} }
    }
}

#[component]
fn ClassificationCodePage() -> Element {
    let perm = use_context::<Perm>();
    let plant = use_signal(String::new);
    let mut kind = use_signal(|| "D".to_owned());
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut codes = use_resource(move || {
        let p = plant();
        let k = kind();
        async move {
            if p.is_empty() {
                return Ok(Vec::new());
            }
            api::list_codes(&p, &k).await
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(ClassificationCodeInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    // 異動記錄對話框（ADR 0012 §8）：（鍵值, 記錄）
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);

    let list: Vec<ClassificationCodeDto> = codes
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let cur: Option<ClassificationCodeDto> = selected()
        .and_then(|c| list.iter().find(|x| x.code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let cost = kind() == "G";
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::ALL.len()
    } else if cost {
        5
    } else {
        2
    };

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        codes.restart();
    };

    let save = move |_| {
        let p = plant();
        let k = kind();
        let target = editor();
        let req = form();
        spawn(async move {
            let result = match target {
                Some(Editor::Edit(code)) => api::update_code(&p, &k, &code, &req).await,
                _ => api::create_code(&p, &k, &req).await,
            };
            match result {
                Ok(c) => done(
                    tf("code-msg-saved", &[("code", c.code.as_str())]),
                    Some(c.code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_edit = cur.clone();
    let cur_delete = cur.clone();
    let cur_history = cur.clone();
    let cur_toggle = cur.clone();
    let inactive = cur.as_ref().is_some_and(|c| !c.active);
    let v = form();
    let kind_options: Vec<(String, String)> = KINDS
        .iter()
        .map(|(c, id)| ((*c).to_owned(), format!("{c} {}", t(id))))
        .collect();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("code-breadcrumb")} }
                h2 { {t("code-title")} }
            }
            PlantPicker { plant }
            div { class: "form-grid",
                Select {
                    id: "azf02",
                    label: t("code-field-kind"),
                    options: kind_options,
                    value: kind(),
                    onchange: move |k: String| {
                        message.set(None);
                        error.set(None);
                        selected.set(None);
                        kind.set(k);
                    },
                }
            }

            div { class: "action-bar",
                if perm.add_row() {
                    Button {
                        icon: icons::ADD, tone: Tone::Success,
                        onclick: move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(ClassificationCodeInput::default());
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
                            editor.set(Some(Editor::Edit(c.code)));
                        },
                        {t("common-action-edit")}
                    }
                }
                if perm.can("history") {
                    Button {
                        icon: icons::HISTORY, tone: Tone::Primary,
                        variant: ButtonVariant::Outlined,
                        disabled: no_cur,
                        onclick: move |_| {
                            let Some(c) = cur_history.clone() else { return };
                            let p = plant();
                                            let k = kind();
                            spawn(async move {
                                match api::code_history(&p, &k, &c.code).await {
                                    Ok(h) => history.set(Some((c.code.clone(), h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        },
                        {t("common-action-history")}
                    }
                }
                if perm.delete_row() {
                    Button {
                        icon: icons::DELETE, tone: Tone::Error,
                        variant: ButtonVariant::Error,
                        disabled: no_cur,
                        onclick: move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            let p = plant();
                            let k = kind();
                            spawn(async move {
                                match api::delete_code(&p, &k, &c.code, c.version).await {
                                    Ok(()) => done(tf("code-msg-deleted", &[("code", c.code.as_str())]), None),
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
                            let p = plant();
                            let k = kind();
                            spawn(async move {
                                match api::set_code_active(&p, &k, &c.code, !c.active, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "code-msg-activated" } else { "code-msg-deactivated" };
                                        done(tf(id, &[("code", r.code.as_str())]), Some(r.code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        },
                        if inactive { {t("code-action-activate")} } else { {t("code-action-deactivate")} }
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
                    h3 { {t("code-list-title")} }
                    p { {tf("list-count-all", &[("total", list.len().to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("code-field-code")} }
                                th { {t("code-field-description")} }
                                if status_tab {
                                    StatusHeaders {}
                                } else if cost {
                                    th { {t("code-field-variance-account")} }
                                    th { {t("code-field-variance-account2")} }
                                    th { {t("code-field-cost-nature")} }
                                }
                            }
                        }
                        tbody {
                            match &*codes.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for c in list.iter().cloned() {
                                        tr {
                                            key: "{c.code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.code == c.code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = c.code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{c.code}" }
                                            td { {c.description.clone().unwrap_or_default()} }
                                            if status_tab {
                                                StatusCells { status: c.status() }
                                            } else if cost {
                                                td { class: "mono", {c.variance_account.clone().unwrap_or_default()} }
                                                td { class: "mono", {c.variance_account2.clone().unwrap_or_default()} }
                                                td { {c.cost_nature.map(|n| n.to_string()).unwrap_or_default()} }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }

            if let Some((key, changes)) = history() {
                HistoryDialog { record: key, changes, onclose: move |_| history.set(None) }
            }
            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("code-dialog-new"),
                        Editor::Edit(code) => tf("code-dialog-edit", &[("code", code.as_str())]),
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
                        TextField { id: "azf01", label: t("code-field-code"), maxlength: max_len::CODE, value: v.code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().code = x }
                        TextField { id: "azf03", label: t("code-field-description"), maxlength: max_len::DESCRIPTION, value: v.description.clone().unwrap_or_default(), oninput: move |x| form.write().description = opt(x) }
                        if cost {
                            TextField { id: "azf05", label: t("code-field-variance-account"), maxlength: max_len::ACCOUNT, value: v.variance_account.clone().unwrap_or_default(), oninput: move |x| form.write().variance_account = opt(x) }
                            TextField { id: "azf051", label: t("code-field-variance-account2"), maxlength: max_len::ACCOUNT, value: v.variance_account2.clone().unwrap_or_default(), oninput: move |x| form.write().variance_account2 = opt(x) }
                            TextField {
                                id: "azf06",
                                label: t("code-field-cost-nature"),
                                input_type: "number",
                                value: v.cost_nature.map(|n| n.to_string()).unwrap_or_default(),
                                oninput: move |x: String| form.write().cost_nature = x.trim().parse().ok(),
                            }
                        }
                    }
                    if cost {
                        p { class: "muted", {t("code-hint-account")} }
                    }
                }
            }
        }
    }
}
