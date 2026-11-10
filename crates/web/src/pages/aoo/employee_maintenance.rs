//! Employee Maintenance 員工維護（TipTop aooi040 員工姓名資料維護）
//! 版面比照 tiptop/aoo/4fd/aooi040.4fd（EQ8）：單一表格（員工編號、姓名、部門與部門名稱、職稱、分機、E-mail、資料有效）；
//!   新增／修改以小視窗編輯，部門從本營運中心有效的部門選擇（aooi040.4gl:608-622）。
//! 工廠層：上方選擇營運中心（ADR 0007）。查詢在表格第一列輸入條件。規則在後端 EmployeeService。

use dioxus::prelude::*;
use ttrs_shared::{
    EmployeeDto, EmployeeInput, QbeCondition, QueryRequest, employee_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Select, Tabs, TextField,
    Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::components::qbe::{ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav};
use crate::components::status::{
    self, ConfirmCells, ConfirmHeaders, ConfirmQbeCells, HasStatus, StatusCells, StatusHeaders,
    StatusQbeCells, TAB_DATA, TAB_STATUS,
};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

/// 表格欄位（aooi040.4fd s_gen 順序）：（標題翻譯 ID, 查詢欄位鍵值, 查詢提示翻譯 ID）
const COLUMNS: [(&str, &str, &str); 6] = [
    ("emp-field-id", "employee_id", ""),
    ("emp-field-name", "name", ""),
    ("emp-field-dept", "dept_code", ""),
    ("emp-field-title", "title", ""),
    ("emp-field-extension", "extension", ""),
    ("emp-field-email", "email", ""),
];

/// 「狀態」頁仍顯示的識別欄位（代號、名稱）
const KEY_COLUMNS: usize = 2;

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "employee-maintenance";

#[component]
pub fn EmployeeMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, EmployeeMaintenancePage {} }
    }
}

#[component]
fn EmployeeMaintenancePage() -> Element {
    let perm = use_context::<Perm>();
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut employees = use_resource(move || {
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
            api::query_employees(&p, &req).await
        }
    });
    // 部門選項：本營運中心有效的部門
    let depts = use_resource(move || {
        let p = plant();
        async move {
            if p.is_empty() {
                return Vec::new();
            }
            let req = QueryRequest {
                page_size: Some(500),
                ..Default::default()
            };
            api::query_departments(&p, &req)
                .await
                .map(|page| page.items)
                .unwrap_or_default()
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut copy_id = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        employees
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|e| e.employee_id.clone()).collect())
            .unwrap_or_default()
    });
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(EmployeeInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    // 異動記錄對話框（ADR 0012 §8）：（鍵值, 記錄）
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        KEY_COLUMNS + status::ALL.len() + 2
    } else {
        COLUMNS.len()
    };

    let page_data = employees
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let list: Vec<EmployeeDto> = page_data
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
    let cur: Option<EmployeeDto> = selected()
        .and_then(|id| list.iter().find(|e| e.employee_id == id).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();

    let mut done = move |msg: String, id: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(id);
        employees.restart();
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
            match api::query_employees(&p, &req).await {
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
        let req = form();
        let target = editor();
        spawn(async move {
            let result = match target {
                Some(Editor::Edit(id)) => api::update_employee(&p, &id, &req).await,
                _ => api::create_employee(&p, &req).await,
            };
            match result {
                Ok(e) => done(
                    tf("emp-msg-saved", &[("id", e.employee_id.as_str())]),
                    Some(e.employee_id),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_edit = cur.clone();
    let cur_delete = cur.clone();
    let cur_history = cur.clone();
    let cur_toggle = cur.clone();
    let cur_copy = cur.clone();
    let cur_confirm = cur.clone();
    let confirmed = cur.as_ref().is_some_and(|e| e.confirmed);
    let keys: Vec<String> = list.iter().map(|e| e.employee_id.clone()).collect();
    let report = move |format: api::ReportFormat| {
        let tab = api::ReportTab::open(format);
        let p = plant();
        let conds = conditions();
        spawn(async move {
            if let Err(e) = api::list_report(
                &p,
                "employees",
                format,
                &conds,
                crate::i18n::current().code(),
                tab,
            )
            .await
            {
                error.set(Some(e));
            }
        });
    };
    let v = form();
    let current_dept = v.dept_code.clone().unwrap_or_default();
    let dept_options: Vec<(String, String)> =
        std::iter::once((String::new(), t("emp-opt-no-dept")))
            .chain(
                depts
                    .read()
                    .as_ref()
                    .map(|ds| {
                        ds.iter()
                            .filter(|d| d.active || d.dept_code == current_dept)
                            .map(|d| (d.dept_code.clone(), format!("{} {}", d.dept_code, d.name)))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
            )
            .collect();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("emp-breadcrumb")} }
                h2 { {t("emp-title")} }
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
                    MaintenanceToolbar {
                        query: Act::new(true, move |_| { message.set(None); error.set(None); qbe.set(Default::default()); querying.set(true); }),
                        // aooi040 是單身式作業：新增、修改、刪除、停用都用 detail
                        detail_action: "detail",
                        insert: if perm.add_row() { Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(EmployeeInput::default());
                            editor.set(Some(Editor::New));
                        }) } else { None },
                        modify: Act::new(!no_cur, move |_| {
                            let Some(e) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(e.to_input());
                            editor.set(Some(Editor::Edit(e.employee_id)));
                        }),
                        // 複製（ADR 0013 SA5）：新員工編號輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_id().trim().is_empty(), move |_| {
                            let Some(e) = cur_copy.clone() else { return };
                            let p = plant();
                            let new_id = copy_id();
                            spawn(async move {
                                match api::copy_employee(&p, &e.employee_id, new_id.trim()).await {
                                    Ok(r) => {
                                        copy_id.set(String::new());
                                        done(tf("emp-msg-copied", &[("from", e.employee_id.as_str()), ("to", r.employee_id.as_str())]), Some(r.employee_id));
                                    }
                                    Err(err) => error.set(Some(err)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）
                        delete: if perm.delete_row() { Act::new(!no_cur && !confirmed, move |_| {
                            let Some(e) = cur_delete.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::delete_employee(&p, &e.employee_id, e.version).await {
                                    Ok(()) => done(tf("emp-msg-deleted", &[("id", e.employee_id.as_str())]), None),
                                    Err(err) => error.set(Some(err)),
                                }
                            });
                        }) } else { None },
                        // 確認／取消確認（ADR 0013 SA9）
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(e) = cur_confirm.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::confirm_employee(&p, &e.employee_id, !e.confirmed, e.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "emp-msg-confirmed" } else { "emp-msg-unconfirmed" };
                                        done(tf(id, &[("id", r.employee_id.as_str())]), Some(r.employee_id));
                                    }
                                    Err(err) => error.set(Some(err)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(e) = cur_toggle.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::set_employee_active(&p, &e.employee_id, !e.active, e.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "emp-msg-activated" } else { "emp-msg-deactivated" };
                                        done(tf(id, &[("id", r.employee_id.as_str())]), Some(r.employee_id));
                                    }
                                    Err(err) => error.set(Some(err)),
                                }
                            });
                        }),
                        invalid_action: "detail",
                        inactive: cur.as_ref().is_some_and(|e| !e.active),
                        invalid_label: Some(("emp-action-deactivate", "emp-action-activate")),
                        // 列印、匯出 Excel（ADR 0013 SA3）：依目前的查詢條件
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        history: Act::new(!no_cur, move |_| {
                            let Some(e) = cur_history.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::history(&p, "employees", &e.employee_id).await {
                                    Ok(h) => history.set(Some((e.employee_id.clone(), h))),
                                    Err(err) => error.set(Some(err)),
                                }
                            });
                        }),
                        if perm.can("reproduce") {
                            TextField { id: "copy-id", label: t("emp-field-copy-as"), value: copy_id(), disabled: no_cur, oninput: move |v| copy_id.set(v) }
                        }
                    }
                }
            }

            // 上下筆：移動選取列，跨頁自動換頁（ADR 0013 SA2）
            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|e| e.employee_id.as_str())),
                    ongo: {
                        let keys = keys.clone();
                        move |i| { message.set(None); nav.go(i, &keys); }
                    },
                    onerror: move |e| error.set(Some(e)),
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
                    h3 { {t("emp-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                for (id, _, _) in COLUMNS.iter().take(if status_tab { KEY_COLUMNS } else { COLUMNS.len() }) {
                                    th { key: "{id}", {t(id)} }
                                }
                                if status_tab { StatusHeaders {} ConfirmHeaders {} }
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
                                    if status_tab { StatusQbeCells { queryable: status::ALL, qbe } ConfirmQbeCells { qbe } }
                                }
                            }
                        }
                        tbody {
                            match &*employees.read() {
                                None => rsx! { tr { td { colspan: "{cols}", {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: "{cols}", "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: "{cols}", class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for e in list.iter().cloned() {
                                        tr {
                                            key: "{e.employee_id}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.employee_id == e.employee_id) { "md-table__row--selected" },
                                            onclick: {
                                                let id = e.employee_id.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(id.clone())); }
                                            },
                                            td { class: "mono", "{e.employee_id}" }
                                            td { "{e.name}" }
                                            if status_tab {
                                                StatusCells { status: e.status() }
                                                ConfirmCells { confirmed: e.confirmed, by: e.confirmed_by.clone() }
                                            } else {
                                            td { {e.dept_code.clone().map(|d| format!("{d} {}", e.dept_name.clone().unwrap_or_default())).unwrap_or_default()} }
                                            td { {e.title.clone().unwrap_or_default()} }
                                            td { {e.extension.clone().unwrap_or_default()} }
                                            td { {e.email.clone().unwrap_or_default()} }
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
                        Editor::New => t("emp-dialog-new"),
                        Editor::Edit(id) => tf("emp-dialog-edit", &[("id", id.as_str())]),
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
                        TextField { id: "gen01", label: t("emp-field-id"), maxlength: max_len::ID, value: v.employee_id.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().employee_id = x }
                        TextField { id: "gen02", label: t("emp-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                        Select { id: "gen03", label: t("emp-field-dept"), options: dept_options.clone(), value: current_dept.clone(), onchange: move |x| form.write().dept_code = opt(x) }
                        TextField { id: "gen04", label: t("emp-field-title"), maxlength: max_len::TITLE, value: v.title.clone().unwrap_or_default(), oninput: move |x| form.write().title = opt(x) }
                        TextField { id: "gen05", label: t("emp-field-extension"), maxlength: max_len::EXTENSION, value: v.extension.clone().unwrap_or_default(), oninput: move |x| form.write().extension = opt(x) }
                        TextField { id: "gen06", label: t("emp-field-email"), maxlength: max_len::EMAIL, value: v.email.clone().unwrap_or_default(), oninput: move |x| form.write().email = opt(x) }
                    }
                }
            }
        }
    }
}
