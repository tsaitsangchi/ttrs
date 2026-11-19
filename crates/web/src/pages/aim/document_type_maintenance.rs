//! Document Type Maintenance 單別維護（TipTop asmi300 / smy_file、smz_file、smu_file、smv_file）
//! 版面比照 topcust/csm/4fd/asmi300.4fd：上方單別清單（資料／狀態頁籤），選定後下方是倉庫設限、使用者與部門設限（DT1）。
//! 工廠層：用登入的營運中心（DT2）。規則在後端 DocumentTypeService；本批只開放系統別 aim（DT3）。

use dioxus::prelude::*;
use ttrs_shared::{
    DocTypeAccessInput, DocTypeStorageDto, DocTypeStorageInput, DocumentTypeDto, DocumentTypeInput,
    QbeCondition, QueryRequest,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Dialog, Notification, NotificationKind, Select, Tabs,
    TextField, Tone, icons,
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

/// aim 的單據性質（DT4）
const AIM_KINDS: [&str; 17] = [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F", "H", "Z",
];

fn kind_label(system: &str, kind: &str) -> String {
    format!(
        "{kind} {}",
        t(&format!("dt-kind-{system}-{}", kind.to_lowercase()))
    )
}

fn coded(prefix: &str, codes: &[&str]) -> Vec<(String, String)> {
    codes
        .iter()
        .map(|c| {
            (
                (*c).to_owned(),
                format!("{c} {}", t(&format!("{prefix}-{}", c.to_lowercase()))),
            )
        })
        .collect()
}

fn yn(v: bool) -> String {
    if v { "Y" } else { "N" }.to_owned()
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

const Q_COLUMNS: [(&str, &str); 5] = [
    ("dt-field-code", "doc_type_code"),
    ("dt-field-name", "name"),
    ("dt-field-kind", "kind"),
    ("dt-field-numbering", "numbering"),
    ("dt-field-restriction", "restriction"),
];

const PROGRAM: &str = "document-type-maintenance";

#[component]
pub fn DocumentTypeMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, DocumentTypePage {} }
    }
}

#[component]
fn DocumentTypePage() -> Element {
    let perm = use_context::<Perm>();
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut docs = use_resource(move || {
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
            api::query_document_types(&p, &req).await
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        docs.read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|c| c.doc_type_code.clone()).collect())
            .unwrap_or_default()
    });
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(DocumentTypeInput::default);
    let mut storage_form = use_signal(|| None::<DocTypeStorageInput>);
    let mut access_form = use_signal(|| None::<(String, String, i32)>);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);

    let page_data = docs.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<DocumentTypeDto> = page_data
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
    let cur: Option<DocumentTypeDto> = selected()
        .and_then(|c| list.iter().find(|x| x.doc_type_code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let keys: Vec<String> = list.iter().map(|c| c.doc_type_code.clone()).collect();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::ALL.len() + 2
    } else {
        7
    };

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        storage_form.set(None);
        access_form.set(None);
        dialog_error.set(None);
        selected.set(code);
        docs.restart();
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
            match api::query_document_types(&p, &req).await {
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

    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        let p = plant();
        spawn(async move {
            if let Err(e) = api::list_report(
                &p,
                "document-types",
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

    let save = move |_| {
        let p = plant();
        let target = editor();
        let req = form();
        spawn(async move {
            let code = match &target {
                Some(Editor::Edit(c)) => Some(c.as_str()),
                _ => None,
            };
            match api::save_document_type(&p, code, &req).await {
                Ok(d) => done(
                    tf("dt-msg-saved", &[("code", d.doc_type_code.as_str())]),
                    Some(d.doc_type_code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_storage = cur.clone();
    let save_storages = move |_| {
        let (Some(c), Some(mut req)) = (cur_storage.clone(), storage_form()) else {
            return;
        };
        req.version = Some(c.version);
        let p = plant();
        spawn(async move {
            match api::set_document_type_storages(&p, &c.doc_type_code, &req).await {
                Ok(d) => done(
                    tf("dt-msg-saved", &[("code", d.doc_type_code.as_str())]),
                    Some(d.doc_type_code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_access = cur.clone();
    let save_access = move |_| {
        let (Some(c), Some((users, depts, version))) = (cur_access.clone(), access_form()) else {
            return;
        };
        let split = |s: &str| -> Vec<String> {
            s.split([',', '，', ' ', '、'])
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(str::to_owned)
                .collect()
        };
        let req = DocTypeAccessInput {
            // 只送有權限修改的清單（authorization、dept_authorization）
            users: perm.can("authorization").then(|| split(&users)),
            depts: perm.can("dept_authorization").then(|| split(&depts)),
            version: Some(version),
        };
        let p = plant();
        spawn(async move {
            match api::set_document_type_access(&p, &c.doc_type_code, &req).await {
                Ok(d) => done(
                    tf("dt-msg-saved", &[("code", d.doc_type_code.as_str())]),
                    Some(d.doc_type_code),
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
    let inactive = cur.as_ref().is_some_and(|c| !c.active);
    let confirmed = cur.as_ref().is_some_and(|c| c.confirmed);
    let v = form();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("dt-breadcrumb")} }
                h2 { {t("dt-title")} }
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
                        insert: Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(DocumentTypeInput { system: "aim".into(), kind: "1".into(), ..Default::default() });
                            editor.set(Some(Editor::New));
                        }),
                        modify: Act::new(!no_cur && !inactive, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.doc_type_code)));
                        }),
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let p = plant();
                            let new_code = copy_code();
                            spawn(async move {
                                match api::copy_document_type(&p, &c.doc_type_code, new_code.trim()).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("dt-msg-copied", &[("from", c.doc_type_code.as_str()), ("to", r.doc_type_code.as_str())]), Some(r.doc_type_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        delete: Act::new(!no_cur && !confirmed && !inactive, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::delete_document_type(&p, &c.doc_type_code, c.version).await {
                                    Ok(()) => done(tf("dt-msg-deleted", &[("code", c.doc_type_code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            let p = plant();
                            let state = if c.confirmed { "unconfirm" } else { "confirm" };
                            spawn(async move {
                                match api::set_document_type_state(&p, &c.doc_type_code, state, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "dt-msg-confirmed" } else { "dt-msg-unconfirmed" };
                                        done(tf(id, &[("code", r.doc_type_code.as_str())]), Some(r.doc_type_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            let p = plant();
                            let state = if c.active { "deactivate" } else { "activate" };
                            spawn(async move {
                                match api::set_document_type_state(&p, &c.doc_type_code, state, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "dt-msg-activated" } else { "dt-msg-deactivated" };
                                        done(tf(id, &[("code", r.doc_type_code.as_str())]), Some(r.doc_type_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        inactive,
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        history: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_history.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::document_type_history(&p, &c.doc_type_code).await {
                                    Ok(h) => history.set(Some((c.doc_type_code.clone(), h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("dt-field-copy-as"), maxlength: 5usize, value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
                        }
                    }
                }
            }

            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.doc_type_code.as_str())),
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
                Tabs { tabs: vec![t(TAB_DATA), t(TAB_STATUS)], active: col_tab }
                div { class: "table-header",
                    h3 { {t("dt-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("dt-field-code")} }
                                th { {t("dt-field-name")} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else {
                                    th { {t("dt-field-kind")} }
                                    th { {t("dt-field-numbering")} }
                                    th { {t("dt-field-auto-number")} }
                                    th { {t("dt-field-cost-class")} }
                                    th { {t("dt-field-restriction")} }
                                }
                            }
                            if querying() {
                                tr {
                                    for (id, field) in Q_COLUMNS.into_iter().take(if status_tab { 2 } else { Q_COLUMNS.len() }) {
                                        th { key: "{field}",
                                            TextField {
                                                id: "q-{field}",
                                                label: t(id),
                                                value: qbe.read().get(field).cloned().unwrap_or_default(),
                                                oninput: move |v: String| { qbe.write().insert(field, v); },
                                            }
                                        }
                                    }
                                    if status_tab {
                                        StatusQbeCells { queryable: status::ALL, qbe }
                                        ConfirmQbeCells { qbe }
                                    } else {
                                        th {}
                                        th {}
                                    }
                                }
                            }
                        }
                        tbody {
                            match &*docs.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for c in list.iter().cloned() {
                                        tr {
                                            key: "{c.doc_type_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.doc_type_code == c.doc_type_code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = c.doc_type_code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{c.doc_type_code}" }
                                            td { "{c.name}" }
                                            if status_tab {
                                                StatusCells { status: c.status() }
                                                ConfirmCells { confirmed: c.confirmed, by: c.confirmed_by.clone() }
                                            } else {
                                                td { {kind_label(&c.system, &c.kind)} }
                                                td { {t(&format!("dt-numbering-{}", c.numbering))} }
                                                td { class: "mono", {yn(c.auto_number)} }
                                                td { {t(&format!("dt-cost-class-{}", c.cost_class.to_lowercase()))} }
                                                td { {t(&format!("dt-restriction-{}", c.restriction))} }
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

            // 單身：倉庫設限、使用者與部門設限
            if let Some(c) = cur.clone() {
                Card {
                    div { class: "table-header",
                        h3 { {tf("dt-storage-title", &[("code", c.doc_type_code.as_str()), ("restriction", t(&format!("dt-restriction-{}", c.restriction)).as_str())])} }
                        if perm.can("detail") && c.active {
                            Button {
                                icon: icons::EDIT,
                                variant: ButtonVariant::Tonal,
                                onclick: {
                                    let c = c.clone();
                                    move |_| {
                                        dialog_error.set(None);
                                        let mut rows = c.storages.clone();
                                        rows.push(DocTypeStorageDto::default());
                                        storage_form.set(Some(DocTypeStorageInput { restriction: c.restriction.clone(), storages: rows, version: Some(c.version) }));
                                    }
                                },
                                {t("dt-action-storages")}
                            }
                        }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead { tr { th { {t("dt-field-warehouse")} } th { {t("dt-field-location")} } } }
                            tbody {
                                if c.storages.is_empty() {
                                    tr { td { colspan: 2, class: "muted", {t("dt-msg-no-storage")} } }
                                }
                                for (i, s) in c.storages.iter().cloned().enumerate() {
                                    tr { key: "{i}",
                                        td { class: "mono", "{s.warehouse}" }
                                        td { class: "mono", "{s.location}" }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "table-header",
                        h3 { {t("dt-access-title")} }
                        if (perm.can("authorization") || perm.can("dept_authorization")) && c.active {
                            Button {
                                icon: icons::EDIT,
                                variant: ButtonVariant::Tonal,
                                onclick: {
                                    let c = c.clone();
                                    move |_| {
                                        dialog_error.set(None);
                                        access_form.set(Some((c.users.join(", "), c.depts.join(", "), c.version))); // i18n-allow
                                    }
                                },
                                {t("dt-action-access")}
                            }
                        }
                    }
                    div { class: "form-grid",
                        TextField { id: "smu02", label: t("dt-field-users"), value: c.users.join(", "), readonly: true } // i18n-allow
                        TextField { id: "smv02", label: t("dt-field-depts"), value: c.depts.join(", "), readonly: true } // i18n-allow
                    }
                    p { class: "muted", {t("dt-hint-access")} }
                }
            }

            if let Some((key, changes)) = history() {
                HistoryDialog { record: key, changes, onclose: move |_| history.set(None) }
            }
            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("dt-dialog-new"),
                        Editor::Edit(code) => tf("dt-dialog-edit", &[("code", code.as_str())]),
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
                        TextField { id: "smyslip", label: t("dt-field-code"), maxlength: 5usize, value: v.doc_type_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().doc_type_code = x }
                        TextField { id: "smydesc", label: t("dt-field-name"), maxlength: 80usize, value: v.name.clone(), oninput: move |x| form.write().name = x }
                        Select { id: "smysys", label: t("dt-field-system"), options: vec![("aim".to_owned(), t("dt-system-aim"))], value: v.system.clone(), onchange: move |x| form.write().system = x }
                        Select {
                            id: "smykind",
                            label: t("dt-field-kind"),
                            options: AIM_KINDS.iter().map(|k| ((*k).to_owned(), kind_label("aim", k))).collect(),
                            value: v.kind.clone(),
                            onchange: move |x| { let mut f = form.write(); f.kind = x; f.cost_class = None; },
                        }
                        Select { id: "smydmy5", label: t("dt-field-numbering"), options: coded("dt-numbering", &["1", "2", "3", "4"]), value: v.numbering.clone().unwrap_or_else(|| "2".into()), onchange: move |x| form.write().numbering = Some(x) }
                        Select {
                            id: "smydmy2",
                            label: t("dt-field-cost-class"),
                            options: { let mut o = vec![(String::new(), t("dt-cost-class-default"))]; o.extend(coded("dt-cost-class", &["1", "2", "3", "4", "5", "X"])); o },
                            value: v.cost_class.clone().unwrap_or_default(),
                            onchange: move |x: String| form.write().cost_class = Some(x).filter(|s| !s.is_empty()),
                        }
                        Checkbox { id: "smyauno", label: t("dt-field-auto-number"), checked: v.auto_number.unwrap_or(true), onchange: move |x| form.write().auto_number = Some(x) }
                        Checkbox { id: "smyprint", label: t("dt-field-print-now"), checked: v.print_now.unwrap_or(false), onchange: move |x| form.write().print_now = Some(x) }
                        Checkbox { id: "smydmy4", label: t("dt-field-confirm-now"), checked: v.confirm_now.unwrap_or(false), onchange: move |x| form.write().confirm_now = Some(x) }
                        Checkbox { id: "smydmy1", label: t("dt-field-cost-enabled"), checked: v.cost_enabled.unwrap_or(true), onchange: move |x| form.write().cost_enabled = Some(x) }
                        Checkbox { id: "smy56", label: t("dt-field-idle-date"), checked: v.affects_idle_date.unwrap_or(true), onchange: move |x| form.write().affects_idle_date = Some(x) }
                        Checkbox { id: "smy53", label: t("dt-field-internal-trade"), checked: v.internal_trade.unwrap_or(false), onchange: move |x| form.write().internal_trade = Some(x) }
                        Checkbox { id: "smy59", label: t("dt-field-budget"), checked: v.budget_control.unwrap_or(false), onchange: move |x| form.write().budget_control = Some(x) }
                        TextField { id: "smymemo1", label: t("dt-field-memo"), maxlength: 255usize, value: v.memo.clone().unwrap_or_default(), oninput: move |x: String| form.write().memo = Some(x) }
                    }
                    p { class: "muted", {t("dt-hint-code")} }
                    p { class: "muted", {t("dt-hint-cost")} }
                }
            }
            if let Some(sf) = storage_form() {
                Dialog {
                    title: t("dt-action-storages"),
                    oncancel: move |_| { storage_form.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { storage_form.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save_storages, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    Select {
                        id: "smyware",
                        label: t("dt-field-restriction"),
                        options: coded("dt-restriction", &["0", "1", "2", "3", "4"]),
                        value: sf.restriction.clone(),
                        onchange: move |x| if let Some(f) = storage_form.write().as_mut() { f.restriction = x; },
                    }
                    for (i, r) in sf.storages.iter().cloned().enumerate() {
                        div { key: "{i}", class: "form-grid",
                            TextField { id: "smz021-{i}", label: t("dt-field-warehouse"), maxlength: 10usize, value: r.warehouse.clone(), oninput: move |x| if let Some(f) = storage_form.write().as_mut() { f.storages[i].warehouse = x; } }
                            TextField { id: "smz031-{i}", label: t("dt-field-location"), maxlength: 10usize, value: r.location.clone(), oninput: move |x| if let Some(f) = storage_form.write().as_mut() { f.storages[i].location = x; } }
                        }
                    }
                    Button {
                        icon: icons::ADD,
                        variant: ButtonVariant::Text,
                        onclick: move |_| if let Some(f) = storage_form.write().as_mut() { f.storages.push(DocTypeStorageDto::default()); },
                        {t("dt-action-add-row")}
                    }
                    p { class: "muted", {t("dt-hint-storage")} }
                }
            }
            if let Some((users, depts, _)) = access_form() {
                Dialog {
                    title: t("dt-action-access"),
                    oncancel: move |_| { access_form.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { access_form.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save_access, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    div { class: "form-grid",
                        TextField { id: "smu02-edit", label: t("dt-field-users"), value: users, disabled: !perm.can("authorization"), oninput: move |x| if let Some(f) = access_form.write().as_mut() { f.0 = x; } }
                        TextField { id: "smv02-edit", label: t("dt-field-depts"), value: depts, disabled: !perm.can("dept_authorization"), oninput: move |x| if let Some(f) = access_form.write().as_mut() { f.1 = x; } }
                    }
                    p { class: "muted", {t("dt-hint-access")} }
                }
            }
        }
    }
}
