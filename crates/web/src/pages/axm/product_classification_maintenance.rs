//! Product Classification Maintenance 產品分類維護（TipTop axmi110）
//! 版面比照 tiptop/axm/4fd/axmi110.4fd（PC1）：多筆單身表格（產品分類碼、分類名稱、計劃期別格式、期數、成本中心、
//! 銷貨收入科目一、二），新增／修改以小視窗編輯。工廠層：用登入的營運中心（PC2）。規則在後端 ProductClassService。

use dioxus::prelude::*;
use ttrs_shared::{ProductClassDto, ProductClassInput, product_class_max_len as max_len};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Tabs, TextField, Tone,
    icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
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
const PROGRAM: &str = "product-classification-maintenance";

#[component]
pub fn ProductClassificationMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ProductClassPage {} }
    }
}

#[component]
fn ProductClassPage() -> Element {
    let perm = use_context::<Perm>();
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut classes = use_resource(move || {
        let p = plant();
        async move {
            if p.is_empty() {
                return Ok(Vec::new());
            }
            api::list_product_classes(&p).await
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(ProductClassInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);

    let list: Vec<ProductClassDto> = classes
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let cur: Option<ProductClassDto> = selected()
        .and_then(|c| list.iter().find(|x| x.class_code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab { 2 + status::ALL.len() } else { 7 };

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        classes.restart();
    };

    let save = move |_| {
        let p = plant();
        let target = editor();
        let req = form();
        spawn(async move {
            let result = match target {
                Some(Editor::Edit(code)) => api::update_product_class(&p, &code, &req).await,
                _ => api::create_product_class(&p, &req).await,
            };
            match result {
                Ok(c) => done(
                    tf("pclass-msg-saved", &[("code", c.class_code.as_str())]),
                    Some(c.class_code),
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

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("pclass-breadcrumb")} }
                h2 { {t("pclass-title")} }
            }
            PlantPicker { plant }

            div { class: "action-bar",
                if perm.add_row() {
                    Button {
                        icon: icons::ADD, tone: Tone::Success,
                        onclick: move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(ProductClassInput::default());
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
                            editor.set(Some(Editor::Edit(c.class_code)));
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
                            spawn(async move {
                                match api::product_class_history(&p, &c.class_code).await {
                                    Ok(h) => history.set(Some((c.class_code.clone(), h))),
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
                            spawn(async move {
                                match api::delete_product_class(&p, &c.class_code, c.version).await {
                                    Ok(()) => done(tf("pclass-msg-deleted", &[("code", c.class_code.as_str())]), None),
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
                            spawn(async move {
                                match api::set_product_class_active(&p, &c.class_code, !c.active, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "pclass-msg-activated" } else { "pclass-msg-deactivated" };
                                        done(tf(id, &[("code", r.class_code.as_str())]), Some(r.class_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        },
                        if inactive { {t("pclass-action-activate")} } else { {t("pclass-action-deactivate")} }
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
                    h3 { {t("pclass-list-title")} }
                    p { {tf("list-count-all", &[("total", list.len().to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("pclass-field-code")} }
                                th { {t("pclass-field-name")} }
                                if status_tab {
                                    StatusHeaders {}
                                } else {
                                    th { {t("pclass-field-period-format")} }
                                    th { {t("pclass-field-periods")} }
                                    th { {t("pclass-field-cost-center")} }
                                    th { {t("pclass-field-revenue-account")} }
                                    th { {t("pclass-field-revenue-account2")} }
                                }
                            }
                        }
                        tbody {
                            match &*classes.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
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
                                            td { {c.name.clone().unwrap_or_default()} }
                                            if status_tab {
                                                StatusCells { status: c.status() }
                                            } else {
                                                td { class: "mono", {c.period_format.clone().unwrap_or_default()} }
                                                td { class: "num", {c.periods.map(|n| n.to_string()).unwrap_or_default()} }
                                                td {
                                                    span { class: "mono", {c.cost_center.clone().unwrap_or_default()} }
                                                    " "
                                                    {c.cost_center_name.clone().unwrap_or_default()}
                                                }
                                                td { class: "mono", {c.revenue_account.clone().unwrap_or_default()} }
                                                td { class: "mono", {c.revenue_account2.clone().unwrap_or_default()} }
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
                        Editor::New => t("pclass-dialog-new"),
                        Editor::Edit(code) => tf("pclass-dialog-edit", &[("code", code.as_str())]),
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
                        TextField { id: "oba01", label: t("pclass-field-code"), maxlength: max_len::CODE, value: v.class_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().class_code = x }
                        TextField { id: "oba02", label: t("pclass-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                        TextField { id: "oba08", label: t("pclass-field-period-format"), maxlength: max_len::PERIOD_FORMAT, value: v.period_format.clone().unwrap_or_default(), oninput: move |x| form.write().period_format = opt(x) }
                        TextField {
                            id: "oba09",
                            label: t("pclass-field-periods"),
                            input_type: "number",
                            value: v.periods.map(|n| n.to_string()).unwrap_or_default(),
                            oninput: move |x: String| form.write().periods = x.trim().parse().ok(),
                        }
                        TextField { id: "oba10", label: t("pclass-field-cost-center"), maxlength: 10, value: v.cost_center.clone().unwrap_or_default(), oninput: move |x| form.write().cost_center = opt(x) }
                        TextField { id: "oba11", label: t("pclass-field-revenue-account"), maxlength: max_len::ACCOUNT, value: v.revenue_account.clone().unwrap_or_default(), oninput: move |x| form.write().revenue_account = opt(x) }
                        TextField { id: "oba111", label: t("pclass-field-revenue-account2"), maxlength: max_len::ACCOUNT, value: v.revenue_account2.clone().unwrap_or_default(), oninput: move |x| form.write().revenue_account2 = opt(x) }
                    }
                    p { class: "muted", {t("pclass-hint-cost-center")} }
                    p { class: "muted", {t("pclass-hint-account")} }
                }
            }
        }
    }
}
