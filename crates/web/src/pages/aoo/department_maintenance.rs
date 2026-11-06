//! Department Maintenance 部門維護（TipTop aooi030 部門名稱資料維護）
//! 版面比照 tiptop/aoo/4fd/aooi030.4fd（DQ9）：單一表格（部門編號、簡稱、全稱、會計部門、費用類別、管理類別、
//!   對應成本中心、資料有效）；新增／修改基本欄位與「會計資料維護」各以小視窗編輯（DQ4）。
//! 工廠層：上方選擇營運中心（ADR 0007）。查詢在表格第一列輸入條件（DQ8）。規則在後端 DepartmentService。

use dioxus::prelude::*;
use ttrs_shared::{
    DepartmentAccountingInput, DepartmentDto, DepartmentInput, QbeCondition, QueryRequest,
    department_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Select, Tabs, TextField,
    Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::components::qbe::{HINT_FLAG, ListPager, PAGE_SIZE, Qbe, conditions_of};
use crate::components::status::{
    self, HasStatus, StatusCells, StatusField, StatusHeaders, StatusQbeCells, TAB_DATA, TAB_STATUS,
};
use crate::i18n::{t, tf};

/// 表格欄位（aooi030.4fd s_gem 順序）：（標題翻譯 ID, 查詢欄位鍵值, 查詢提示翻譯 ID）
const COLUMNS: [(&str, &str, &str); 7] = [
    ("dept-field-code", "dept_code", ""),
    ("dept-field-name", "name", ""),
    ("dept-field-full-name", "full_name", ""),
    ("dept-field-accounting", "accounting_dept", HINT_FLAG),
    (
        "dept-field-expense",
        "expense_type",
        "dept-hint-qbe-expense",
    ),
    (
        "dept-field-control",
        "control_type",
        "dept-hint-qbe-control",
    ),
    ("dept-field-cost-center", "cost_center", ""),
];

/// 「狀態」頁仍顯示的識別欄位（部門編號、簡稱）
const KEY_COLUMNS: usize = 2;

const EXPENSES: [(&str, &str); 4] = [
    ("M", "dept-opt-expense-m"),
    ("P", "dept-opt-expense-p"),
    ("R", "dept-opt-expense-r"),
    ("S", "dept-opt-expense-s"),
];

const CONTROLS: [(&str, &str); 3] = [
    ("1", "dept-opt-control-1"),
    ("2", "dept-opt-control-2"),
    ("3", "dept-opt-control-3"),
];

fn label(options: &[(&str, &str)], code: &str) -> String {
    options
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, id)| t(id))
        .unwrap_or_default()
}

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

/// 編輯視窗：新增、修改基本欄位、會計資料維護
#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
    Accounting(String),
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "department-maintenance";

#[component]
pub fn DepartmentMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, DepartmentMaintenancePage {} }
    }
}

#[component]
fn DepartmentMaintenancePage() -> Element {
    let perm = use_context::<Perm>();
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut depts = use_resource(move || {
        let p = plant();
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move {
            if p.is_empty() {
                return Ok(ttrs_shared::Page {
                    items: Vec::new(),
                    total: 0,
                    page: 1,
                    page_size: PAGE_SIZE,
                });
            }
            api::query_departments(&p, &req).await
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(DepartmentInput::default);
    let mut acct = use_signal(DepartmentAccountingInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    // 異動記錄對話框（ADR 0012 §8）：（鍵值, 記錄）
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        KEY_COLUMNS + status::ALL.len()
    } else {
        COLUMNS.len()
    };

    let page_data = depts.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<DepartmentDto> = page_data
        .as_ref()
        .map(|p| p.items.clone())
        .unwrap_or_default();
    let total = page_data.as_ref().map_or(0, |p| p.total);
    let pages = page_data.as_ref().map_or(1, |p| {
        u32::try_from(p.total.div_ceil(u64::from(p.page_size.max(1))))
            .unwrap_or(1)
            .max(1)
    });
    let filtered = !conditions.read().is_empty();
    let cur: Option<DepartmentDto> = selected()
        .and_then(|code| list.iter().find(|d| d.dept_code == code).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        depts.restart();
    };

    let run_query = move |_| {
        let conds = conditions_of(&qbe.read());
        let req = QueryRequest {
            conditions: conds.clone(),
            page: Some(1),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        let p = plant();
        spawn(async move {
            match api::query_departments(&p, &req).await {
                Ok(result) => {
                    error.set(None);
                    querying.set(false);
                    selected.set(None);
                    conditions.set(conds);
                    page.set(1);
                    message.set(Some(if result.total == 0 {
                        t("query-msg-none")
                    } else {
                        tf(
                            "query-msg-matched",
                            &[("count", result.total.to_string().as_str())],
                        )
                    }));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let save = move |_| {
        let p = plant();
        let target = editor();
        let basic = form();
        let accounting = acct();
        spawn(async move {
            let result = match target {
                Some(Editor::Edit(code)) => api::update_department(&p, &code, &basic).await,
                Some(Editor::Accounting(code)) => {
                    api::set_department_accounting(&p, &code, &accounting).await
                }
                _ => api::create_department(&p, &basic).await,
            };
            match result {
                Ok(d) => done(
                    tf("dept-msg-saved", &[("code", d.dept_code.as_str())]),
                    Some(d.dept_code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_edit = cur.clone();
    let cur_acct = cur.clone();
    let cur_delete = cur.clone();
    let cur_history = cur.clone();
    let cur_toggle = cur.clone();
    let v = form();
    let av = acct();
    let center = av.control_type != "3";
    let expense_options: Vec<(String, String)> =
        std::iter::once((String::new(), t("dept-opt-none")))
            .chain(EXPENSES.iter().map(|(c, id)| ((*c).to_owned(), t(id))))
            .collect();
    let control_options: Vec<(String, String)> = CONTROLS
        .iter()
        .map(|(c, id)| ((*c).to_owned(), t(id)))
        .collect();
    let yes_no: Vec<(String, String)> = vec![
        ("Y".to_owned(), t("dept-opt-yes")),
        ("N".to_owned(), t("dept-opt-no")),
    ];

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("dept-breadcrumb")} }
                h2 { {t("dept-title")} }
            }
            PlantPicker { plant }

            if querying() {
                div { class: "action-bar",
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { error.set(None); querying.set(false); }, {t("common-action-cancel")} }
                }
                p { class: "query-hint", {t("query-hint")} }
            } else {
                div { class: "action-bar",
                    if perm.can("query") {
                        Button {
                            icon: icons::SEARCH, tone: Tone::Primary,
                            variant: ButtonVariant::Tonal,
                            onclick: move |_| { message.set(None); error.set(None); qbe.set(Default::default()); querying.set(true); },
                            {t("common-action-query")}
                        }
                    }
                    if perm.add_row() {
                        Button {
                            icon: icons::ADD, tone: Tone::Success,
                            onclick: move |_| {
                                message.set(None);
                                error.set(None);
                                dialog_error.set(None);
                                form.set(DepartmentInput::default());
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
                                let Some(d) = cur_edit.clone() else { return };
                                dialog_error.set(None);
                                form.set(d.to_input());
                                editor.set(Some(Editor::Edit(d.dept_code)));
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
                                let Some(d) = cur_history.clone() else { return };
                                let p = plant();
                                spawn(async move {
                                    match api::history(&p, "departments", &d.dept_code).await {
                                        Ok(h) => history.set(Some((d.dept_code.clone(), h))),
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
                                let Some(d) = cur_delete.clone() else { return };
                                let p = plant();
                                spawn(async move {
                                    match api::delete_department(&p, &d.dept_code, d.version).await {
                                        Ok(()) => done(tf("dept-msg-deleted", &[("code", d.dept_code.as_str())]), None),
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            {t("common-action-delete")}
                        }
                    }
                    if perm.can("detail") {
                        Button {
                            icon: if cur.as_ref().is_some_and(|d| !d.active) { icons::CHECK_CIRCLE } else { icons::BLOCK }, tone: if cur.as_ref().is_some_and(|d| !d.active) { Tone::Success } else { Tone::Error },
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(d) = cur_toggle.clone() else { return };
                                let p = plant();
                                spawn(async move {
                                    match api::set_department_active(&p, &d.dept_code, !d.active, d.version).await {
                                        Ok(r) => {
                                            let id = if r.active { "dept-msg-activated" } else { "dept-msg-deactivated" };
                                            done(tf(id, &[("code", r.dept_code.as_str())]), Some(r.dept_code));
                                        }
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            if cur.as_ref().is_some_and(|d| !d.active) { {t("dept-action-activate")} } else { {t("dept-action-deactivate")} }
                        }
                    }
                    // 列印、匯出 Excel（aooi030 output／exporttoexcel；department_report.feature）：依目前的查詢條件
                    if perm.can("output") {
                        Button {
                            icon: icons::PRINT, tone: Tone::Primary,
                            variant: ButtonVariant::Outlined,
                            onclick: move |_| {
                                let tab = api::ReportTab::open(api::ReportFormat::Pdf);
                                let p = plant();
                                let conds = conditions();
                                spawn(async move {
                                    if let Err(e) = api::department_report(&p, api::ReportFormat::Pdf, &conds, crate::i18n::current().code(), tab).await {
                                        error.set(Some(e));
                                    }
                                });
                            },
                            {t("common-action-print")}
                        }
                    }
                    if perm.can("exporttoexcel") {
                        Button {
                            icon: icons::DOWNLOAD, tone: Tone::Primary,
                            variant: ButtonVariant::Outlined,
                            onclick: move |_| {
                                let tab = api::ReportTab::open(api::ReportFormat::Excel);
                                let p = plant();
                                let conds = conditions();
                                spawn(async move {
                                    if let Err(e) = api::department_report(&p, api::ReportFormat::Excel, &conds, crate::i18n::current().code(), tab).await {
                                        error.set(Some(e));
                                    }
                                });
                            },
                            {t("common-action-export-excel")}
                        }
                    }
                    if perm.can("account") {
                        Button {
                            icon: icons::ACCOUNT_BALANCE, tone: Tone::Tertiary,
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(d) = cur_acct.clone() else { return };
                                dialog_error.set(None);
                                acct.set(d.to_accounting());
                                editor.set(Some(Editor::Accounting(d.dept_code)));
                            },
                            {t("dept-action-accounting")}
                        }
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
                // 資料／狀態頁籤（ADR 0012 §9）：狀態頁顯示資料有效、所有者、所有部門、建立與修改資訊
                Tabs { tabs: vec![t(TAB_DATA), t(TAB_STATUS)], active: col_tab }
                div { class: "table-header",
                    h3 { {t("dept-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                for (id, _, _) in COLUMNS.iter().take(if status_tab { KEY_COLUMNS } else { COLUMNS.len() }) {
                                    th { key: "{id}", {t(id)} }
                                }
                                if status_tab { StatusHeaders {} }
                            }
                            if querying() {
                                tr {
                                    for &(id, field, hint) in COLUMNS.iter().take(if status_tab { KEY_COLUMNS } else { COLUMNS.len() }) {
                                        th { key: "{field}",
                                            TextField {
                                                id: "q-{field}",
                                                label: t(id),
                                                placeholder: if hint.is_empty() { String::new() } else { t(hint) },
                                                value: qbe.read().get(field).cloned().unwrap_or_default(),
                                                oninput: move |v: String| { qbe.write().insert(field, v); },
                                            }
                                        }
                                    }
                                    if status_tab { StatusQbeCells { queryable: &[StatusField::Active], qbe } }
                                }
                            }
                        }
                        tbody {
                            match &*depts.read() {
                                None => rsx! { tr { td { colspan: "{cols}", {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: "{cols}", "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: "{cols}", class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for d in list.iter().cloned() {
                                        tr {
                                            key: "{d.dept_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.dept_code == d.dept_code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = d.dept_code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{d.dept_code}" }
                                            td { "{d.name}" }
                                            if status_tab {
                                                StatusCells { status: d.status() }
                                            } else {
                                            td { {d.full_name.clone().unwrap_or_default()} }
                                            td { if d.accounting_dept { {t("dept-opt-yes")} } else { {t("dept-opt-no")} } }
                                            td { {d.expense_type.as_deref().map(|e| label(&EXPENSES, e)).unwrap_or_default()} }
                                            td { {label(&CONTROLS, &d.control_type)} }
                                            td { {d.cost_center.clone().map(|c| format!("{c} {}", d.cost_center_name.clone().unwrap_or_default())).unwrap_or_default()} }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
                ListPager { page, pages }
            }

            if let Some((key, changes)) = history() {
                HistoryDialog { record: key, changes, onclose: move |_| history.set(None) }
            }
            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("dept-dialog-new"),
                        Editor::Edit(code) => tf("dept-dialog-edit", &[("code", code.as_str())]),
                        Editor::Accounting(code) => tf("dept-dialog-accounting", &[("code", code.as_str())]),
                    },
                    oncancel: move |_| { editor.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { editor.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    if matches!(ed, Editor::Accounting(_)) {
                        div { class: "form-grid",
                            Select { id: "gem05", label: t("dept-field-accounting"), options: yes_no.clone(), value: if av.accounting_dept { "Y" } else { "N" }, onchange: move |x: String| acct.write().accounting_dept = x == "Y" }
                            Select { id: "gem07", label: t("dept-field-expense"), options: expense_options.clone(), value: av.expense_type.clone().unwrap_or_default(), onchange: move |x| acct.write().expense_type = opt(x) }
                            Select {
                                id: "gem09",
                                label: t("dept-field-control"),
                                options: control_options.clone(),
                                value: av.control_type.clone(),
                                onchange: move |x: String| {
                                    let mut a = acct.write();
                                    a.control_type = x;
                                    a.cost_center = None;
                                },
                            }
                            TextField { id: "gem10", label: t("dept-field-cost-center"), maxlength: max_len::CODE, value: av.cost_center.clone().unwrap_or_default(), disabled: !center, oninput: move |x| acct.write().cost_center = opt(x) }
                        }
                        p { class: "muted", {t("dept-hint-cost-center")} }
                    } else {
                        div { class: "form-grid",
                            TextField { id: "gem01", label: t("dept-field-code"), maxlength: max_len::CODE, value: v.dept_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().dept_code = x }
                            TextField { id: "gem02", label: t("dept-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                            TextField { id: "gem03", label: t("dept-field-full-name"), maxlength: max_len::FULL_NAME, value: v.full_name.clone().unwrap_or_default(), oninput: move |x| form.write().full_name = opt(x) }
                        }
                    }
                }
            }
        }
    }
}
