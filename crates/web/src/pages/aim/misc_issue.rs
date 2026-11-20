//! Misc Issue 雜項發料（TipTop aimt301 → saimt370 參數 1 / ina_file、inb_file → misc_issue、misc_issue_line）
//! 版面比照 topcust/cim/4fd/aimt370.4fd：上方單據清單（資料／狀態頁籤），選定後下方是單身（MT1）。
//! 與雜項收料、雜項報廢各自獨立（Key User 2026-11-20）。規則在後端 MiscIssueService。

use dioxus::prelude::*;
use ttrs_shared::{
    InventoryTxnCopyInput, InventoryTxnDto, InventoryTxnInput, InventoryTxnLineInput, QbeCondition,
    QueryRequest,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Chip, Dialog, Notification, NotificationKind, Tabs, TextField,
    Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::components::qbe::{ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav};
use crate::components::status::{
    self, HasStatus, StatusCells, StatusHeaders, StatusQbeCells, TAB_DATA, TAB_STATUS,
};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

const PROGRAM: &str = "misc-issue";

const Q_COLUMNS: [(&str, &str); 5] = [
    ("misc-field-doc-no", "doc_no"),
    ("misc-field-posting-date", "posting_date"),
    ("misc-field-dept", "dept"),
    ("misc-field-applicant", "applicant"),
    ("misc-field-memo", "memo"),
];

/// 單身編輯列（畫面上全部是文字，存檔時再轉換）
#[derive(Clone, Default, PartialEq)]
struct LineRow {
    line_no: String,
    item_no: String,
    warehouse: String,
    location: String,
    lot: String,
    qty: String,
    reason: String,
}

#[derive(Clone, Default, PartialEq)]
struct Form {
    doc_no: String,
    input_date: String,
    posting_date: String,
    dept: String,
    applicant: String,
    memo: String,
    lines: Vec<LineRow>,
    version: Option<i32>,
}

impl Form {
    fn of(d: &InventoryTxnDto) -> Self {
        Self {
            doc_no: d.doc_no.clone(),
            input_date: d.input_date.to_string(),
            posting_date: d.posting_date.to_string(),
            dept: d.dept.clone().unwrap_or_default(),
            applicant: d.applicant.clone().unwrap_or_default(),
            memo: d.memo.clone().unwrap_or_default(),
            lines: d
                .lines
                .iter()
                .map(|l| LineRow {
                    line_no: l.line_no.to_string(),
                    item_no: l.item_no.clone(),
                    warehouse: l.warehouse.clone(),
                    location: l.location.clone(),
                    lot: l.lot.clone(),
                    qty: l.qty.normalize().to_string(),
                    reason: l.reason.clone().unwrap_or_default(),
                })
                .collect(),
            version: Some(d.version),
        }
    }

    fn input(&self, new: bool) -> Result<InventoryTxnInput, String> {
        let date = |s: &str| -> Result<Option<ttrs_shared::NaiveDate>, String> {
            if s.trim().is_empty() {
                return Ok(None);
            }
            ttrs_shared::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
                .map(Some)
                .map_err(|_| t("misc-msg-date-invalid"))
        };
        let mut lines = Vec::new();
        for r in self.lines.iter().filter(|r| !r.item_no.trim().is_empty()) {
            lines.push(InventoryTxnLineInput {
                line_no: r.line_no.trim().parse().unwrap_or(0),
                item_no: r.item_no.trim().to_owned(),
                warehouse: Some(r.warehouse.trim().to_owned()),
                location: Some(r.location.trim().to_owned()),
                lot: Some(r.lot.trim().to_owned()),
                qty: r
                    .qty
                    .trim()
                    .parse()
                    .map_err(|_| tf("misc-msg-qty-invalid", &[("item", r.item_no.as_str())]))?,
                reason: Some(r.reason.trim().to_owned()).filter(|x| !x.is_empty()),
            });
        }
        Ok(InventoryTxnInput {
            doc_no: self.doc_no.trim().to_owned(),
            input_date: date(&self.input_date)?,
            posting_date: date(&self.posting_date)?,
            // 新增時空白交給後端帶預設（申請人、部門）；修改時照畫面
            dept: (!new || !self.dept.trim().is_empty()).then(|| self.dept.trim().to_owned()),
            applicant: (!new || !self.applicant.trim().is_empty())
                .then(|| self.applicant.trim().to_owned()),
            memo: Some(self.memo.clone()),
            lines: Some(lines),
            version: self.version,
        })
    }
}

fn status_label(code: &str) -> String {
    t(&format!("misc-status-{}", code.to_lowercase()))
}

fn status_tone(code: &str) -> Tone {
    match code {
        "Y" => Tone::Success,
        "X" => Tone::Error,
        _ => Tone::Neutral,
    }
}

#[component]
pub fn MiscIssue() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, MiscIssuePage {} }
    }
}

#[component]
fn MiscIssuePage() -> Element {
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
            api::query_misc_issues(&p, &req).await
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let nav = use_row_nav(page, selected, move || {
        docs.read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|c| c.doc_no.clone()).collect())
            .unwrap_or_default()
    });
    // 編輯中的單據：None 未開啟；Some(None) 新增；Some(Some(單號)) 修改
    let mut editor = use_signal(|| None::<Option<String>>);
    let mut form = use_signal(Form::default);
    let mut copy_form = use_signal(|| None::<InventoryTxnCopyInput>);
    let mut copy_text = use_signal(|| (String::new(), String::new(), String::new()));
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);
    let mut ask_print = use_signal(|| false);
    // 過帳（PT2：可改扣帳日）與庫存異動記錄明細（PT6）
    let mut post_date = use_signal(|| None::<String>);
    let mut txn_records = use_signal(|| None::<(String, Vec<ttrs_shared::InventoryTxnRecordDto>)>);

    let page_data = docs.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<InventoryTxnDto> = page_data
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
    let cur: Option<InventoryTxnDto> = selected()
        .and_then(|c| list.iter().find(|x| x.doc_no == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let keys: Vec<String> = list.iter().map(|c| c.doc_no.clone()).collect();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        1 + status::NO_ACTIVE.len()
    } else {
        6
    };

    let mut done = move |msg: String, no: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        copy_form.set(None);
        dialog_error.set(None);
        selected.set(no);
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
            match api::query_misc_issues(&p, &req).await {
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
                "misc-issues",
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
        let target = editor().flatten();
        let mut req = match form().input(target.is_none()) {
            Ok(r) => r,
            Err(e) => {
                dialog_error.set(Some(e));
                return;
            }
        };
        // 修改時只送有權限的部分：單頭 modify、單身 detail
        if target.is_some() {
            if !perm.can("detail") {
                req.lines = None;
            }
            if !perm.can("modify") {
                req.input_date = None;
                req.posting_date = None;
                req.dept = None;
                req.applicant = None;
                req.memo = None;
            }
        }
        spawn(async move {
            match api::save_misc_issue(&p, target.as_deref(), &req).await {
                Ok(d) => {
                    let msg = match &d.auto_confirm_error {
                        Some(code) => tf(
                            "misc-msg-auto-confirm-failed",
                            &[
                                ("no", d.doc_no.as_str()),
                                ("reason", t(&format!("error-{code}")).as_str()),
                            ],
                        ),
                        None => tf("misc-msg-saved", &[("no", d.doc_no.as_str())]),
                    };
                    // 單別「立即列印」（MT10）：新增後詢問是否列印
                    if target.is_none() && d.print_now && !d.lines.is_empty() {
                        ask_print.set(true);
                    }
                    done(msg, Some(d.doc_no));
                }
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_copy = cur.clone();
    let save_copy = move |_| {
        let Some(c) = cur_copy.clone() else { return };
        let (code, input_date, posting_date) = copy_text();
        let date = |s: &str| ttrs_shared::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok();
        let req = InventoryTxnCopyInput {
            doc_no: code.trim().to_owned(),
            input_date: date(&input_date),
            posting_date: date(&posting_date),
        };
        let p = plant();
        spawn(async move {
            match api::copy_misc_issue(&p, &c.doc_no, &req).await {
                Ok(r) => done(
                    tf(
                        "misc-msg-copied",
                        &[("from", c.doc_no.as_str()), ("to", r.doc_no.as_str())],
                    ),
                    Some(r.doc_no),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_edit = cur.clone();
    let cur_delete = cur.clone();
    let cur_history = cur.clone();
    let cur_void = cur.clone();
    let cur_confirm = cur.clone();
    let cur_post = cur.clone();
    let save_post = move |_| {
        let Some(c) = cur_post.clone() else { return };
        let text = post_date().unwrap_or_default();
        let date = if text.trim().is_empty() {
            None
        } else {
            match ttrs_shared::NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d") {
                Ok(d) => Some(d),
                Err(_) => {
                    dialog_error.set(Some(t("misc-msg-date-invalid")));
                    return;
                }
            }
        };
        let p = plant();
        spawn(async move {
            match api::post_misc_issue(&p, &c.doc_no, date, c.version).await {
                Ok(r) => {
                    post_date.set(None);
                    done(
                        tf("misc-msg-posted", &[("no", r.doc_no.as_str())]),
                        Some(r.doc_no),
                    );
                }
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };
    let cur_undo_post = cur.clone();
    let cur_records = cur.clone();
    let posted = cur.as_ref().is_some_and(|c| c.posted);
    let cur_status = cur.as_ref().map(|c| c.status.clone()).unwrap_or_default();
    let open = cur_status == "N";
    let confirmed = cur_status == "Y";
    let voided = cur_status == "X";
    let v = form();
    let ct = copy_text();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("misc-issue-breadcrumb")} }
                h2 { {t("misc-issue-title")} }
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
                            form.set(Form { lines: vec![LineRow::default()], ..Form::default() });
                            editor.set(Some(None));
                        }),
                        modify: Act::new(open, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            let mut f = Form::of(&c);
                            f.lines.push(LineRow::default());
                            form.set(f);
                            editor.set(Some(Some(c.doc_no)));
                        }),
                        copy: Act::new(!no_cur, move |_| {
                            dialog_error.set(None);
                            copy_text.set((String::new(), String::new(), String::new()));
                            copy_form.set(Some(InventoryTxnCopyInput::default()));
                        }),
                        delete: Act::new(open, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::delete_misc_issue(&p, &c.doc_no, c.version).await {
                                    Ok(()) => done(tf("misc-msg-deleted", &[("no", c.doc_no.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirm: Act::new(!no_cur && !voided && !posted, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            let p = plant();
                            let state = if c.status == "Y" { "undo-confirm" } else { "confirm" };
                            spawn(async move {
                                match api::set_misc_issue_state(&p, &c.doc_no, state, c.version).await {
                                    Ok(r) => {
                                        let id = if r.status == "Y" { "misc-msg-confirmed" } else { "misc-msg-unconfirmed" };
                                        done(tf(id, &[("no", r.doc_no.as_str())]), Some(r.doc_no));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        unconfirm_action: "undo_confirm",
                        invalid: Act::new(!no_cur && !confirmed && !posted, move |_| {
                            let Some(c) = cur_void.clone() else { return };
                            let p = plant();
                            let state = if c.status == "X" { "unvoid" } else { "void" };
                            spawn(async move {
                                match api::set_misc_issue_state(&p, &c.doc_no, state, c.version).await {
                                    Ok(r) => {
                                        let id = if r.status == "X" { "misc-msg-voided" } else { "misc-msg-unvoided" };
                                        done(tf(id, &[("no", r.doc_no.as_str())]), Some(r.doc_no));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        inactive: voided,
                        invalid_action: "void",
                        invalid_label: ("misc-action-void", "misc-action-unvoid"),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        history: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_history.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::misc_issue_history(&p, &c.doc_no).await {
                                    Ok(h) => history.set(Some((c.doc_no.clone(), h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        if perm.can("stock_post") {
                            Button {
                                icon: icons::CHECK_CIRCLE,
                                tone: Tone::Success,
                                variant: ButtonVariant::Outlined,
                                disabled: !(confirmed && !posted),
                                onclick: {
                                    let c = cur.clone();
                                    move |_| {
                                        dialog_error.set(None);
                                        post_date.set(Some(c.as_ref().map(|x| x.posting_date.to_string()).unwrap_or_default()));
                                    }
                                },
                                {t("misc-action-post")}
                            }
                        }
                        if perm.can("undo_post") {
                            Button {
                                icon: icons::UNDO,
                                tone: Tone::Tertiary,
                                variant: ButtonVariant::Outlined,
                                disabled: !posted,
                                onclick: move |_| {
                                    let Some(c) = cur_undo_post.clone() else { return };
                                    let p = plant();
                                    spawn(async move {
                                        match api::undo_post_misc_issue(&p, &c.doc_no, c.version).await {
                                            Ok(r) => done(tf("misc-msg-unposted", &[("no", r.doc_no.as_str())]), Some(r.doc_no)),
                                            Err(e) => error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("misc-action-undo-post")}
                            }
                        }
                        Button {
                            icon: icons::SEARCH,
                            variant: ButtonVariant::Text,
                            disabled: !posted,
                            onclick: move |_| {
                                let Some(c) = cur_records.clone() else { return };
                                let p = plant();
                                spawn(async move {
                                    match api::misc_issue_txn_records(&p, &c.doc_no).await {
                                        Ok(r) => txn_records.set(Some((c.doc_no.clone(), r))),
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            {t("misc-action-txn-records")}
                        }
                    }
                }
            }

            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.doc_no.as_str())),
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
                    h3 { {t("misc-issue-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("misc-field-doc-no")} }
                                if status_tab {
                                    StatusHeaders { fields: status::NO_ACTIVE }
                                } else {
                                    th { {t("misc-field-posting-date")} }
                                    th { {t("misc-field-dept")} }
                                    th { {t("misc-field-applicant")} }
                                    th { {t("misc-field-memo")} }
                                    th { {t("misc-field-status")} }
                                }
                            }
                            if querying() {
                                tr {
                                    for (id, field) in Q_COLUMNS.into_iter().take(if status_tab { 1 } else { Q_COLUMNS.len() }) {
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
                                        StatusQbeCells { fields: status::NO_ACTIVE, queryable: status::NO_ACTIVE, qbe }
                                    } else {
                                        th {
                                            TextField {
                                                id: "q-status",
                                                label: t("misc-field-status"),
                                                value: qbe.read().get("status").cloned().unwrap_or_default(),
                                                oninput: move |v: String| { qbe.write().insert("status", v); },
                                            }
                                        }
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
                                            key: "{c.doc_no}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.doc_no == c.doc_no) { "md-table__row--selected" },
                                            onclick: {
                                                let no = c.doc_no.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(no.clone())); }
                                            },
                                            td { class: "mono", "{c.doc_no}" }
                                            if status_tab {
                                                StatusCells { status: c.status(), fields: status::NO_ACTIVE }
                                            } else {
                                                td { "{c.posting_date}" }
                                                td { class: "mono", {c.dept.clone().unwrap_or_default()} }
                                                td { class: "mono", {c.applicant.clone().unwrap_or_default()} }
                                                td { {c.memo.clone().unwrap_or_default()} }
                                                td {
                                                    Chip { label: status_label(&c.status), tone: status_tone(&c.status) }
                                                    if c.posted {
                                                        Chip { label: t("misc-status-posted"), tone: Tone::Success }
                                                    }
                                                }
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

            // 單身
            if let Some(c) = cur.clone() {
                Card {
                    div { class: "table-header",
                        h3 { {tf("misc-lines-title", &[("no", c.doc_no.as_str())])} }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("misc-field-line-no")} }
                                    th { {t("misc-field-item")} }
                                    th { {t("misc-field-item-name")} }
                                    th { {t("misc-field-warehouse")} }
                                    th { {t("misc-field-location")} }
                                    th { {t("misc-field-lot")} }
                                    th { {t("misc-field-unit")} }
                                    th { {t("misc-field-qty")} }
                                    th { {t("misc-field-reason")} }
                                }
                            }
                            tbody {
                                if c.lines.is_empty() {
                                    tr { td { colspan: 9, class: "muted", {t("misc-msg-no-lines")} } }
                                }
                                for l in c.lines.iter().cloned() {
                                    tr { key: "{l.line_no}",
                                        td { class: "mono", "{l.line_no}" }
                                        td { class: "mono", "{l.item_no}" }
                                        td { {l.item_name.clone().unwrap_or_default()} }
                                        td { class: "mono", "{l.warehouse}" }
                                        td { class: "mono", "{l.location}" }
                                        td { class: "mono", "{l.lot}" }
                                        td { class: "mono", "{l.unit}" }
                                        td { class: "num", {l.qty.normalize().to_string()} }
                                        td { class: "mono", {l.reason.clone().unwrap_or_default()} }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(text) = post_date() {
                Dialog {
                    title: t("misc-dialog-post"),
                    oncancel: move |_| { post_date.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { post_date.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save_post, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    TextField { id: "post-ina02", label: t("misc-field-posting-date"), input_type: "date", value: text, oninput: move |x| post_date.set(Some(x)) }
                    p { class: "muted", {t("misc-hint-post")} }
                }
            }
            if let Some((no, rows)) = txn_records() {
                Dialog {
                    title: tf("misc-dialog-txn-records", &[("no", no.as_str())]),
                    oncancel: move |_| txn_records.set(None),
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| txn_records.set(None), {t("common-action-close")} }
                    },
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("misc-field-line-no")} }
                                    th { {t("misc-field-item")} }
                                    th { {t("misc-field-warehouse")} }
                                    th { {t("misc-field-location")} }
                                    th { {t("misc-field-lot")} }
                                    th { {t("misc-field-direction")} }
                                    th { {t("misc-field-qty")} }
                                    th { {t("misc-field-balance-after")} }
                                    th { {t("misc-field-posting-date")} }
                                }
                            }
                            tbody {
                                for r in rows.iter().cloned() {
                                    tr { key: "{r.doc_line}",
                                        td { class: "mono", "{r.doc_line}" }
                                        td { class: "mono", "{r.item_no}" }
                                        td { class: "mono", "{r.warehouse}" }
                                        td { class: "mono", "{r.location}" }
                                        td { class: "mono", "{r.lot}" }
                                        td { class: "mono", "{r.direction}" }
                                        td { class: "num", {r.qty.normalize().to_string()} }
                                        td { class: "num", {r.balance_after.normalize().to_string()} }
                                        td { "{r.txn_date}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some((key, changes)) = history() {
                HistoryDialog { record: key, changes, onclose: move |_| history.set(None) }
            }
            if ask_print() {
                Dialog {
                    title: t("misc-dialog-print"),
                    oncancel: move |_| ask_print.set(false),
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| ask_print.set(false), {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: move |_| { ask_print.set(false); report(api::ReportFormat::Pdf); }, {t("common-action-ok")} }
                    },
                    p { {t("misc-msg-print-now")} }
                }
            }
            if let Some(target) = editor() {
                Dialog {
                    title: match &target {
                        None => t("misc-issue-dialog-new"),
                        Some(no) => tf("misc-issue-dialog-edit", &[("no", no.as_str())]),
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
                        TextField { id: "ina01", label: t("misc-field-doc-no"), maxlength: 16usize, value: v.doc_no.clone(), readonly: target.is_some(), oninput: move |x| form.write().doc_no = x }
                        TextField { id: "ina03", label: t("misc-field-input-date"), input_type: "date", value: v.input_date.clone(), oninput: move |x| form.write().input_date = x }
                        TextField { id: "ina02", label: t("misc-field-posting-date"), input_type: "date", value: v.posting_date.clone(), oninput: move |x| form.write().posting_date = x }
                        TextField { id: "ina11", label: t("misc-field-applicant"), maxlength: 10usize, value: v.applicant.clone(), oninput: move |x| form.write().applicant = x }
                        TextField { id: "ina04", label: t("misc-field-dept"), maxlength: 10usize, value: v.dept.clone(), oninput: move |x| form.write().dept = x }
                        TextField { id: "ina07", label: t("misc-field-memo"), maxlength: 255usize, value: v.memo.clone(), oninput: move |x| form.write().memo = x }
                    }
                    p { class: "muted", {t("misc-hint-doc-no")} }
                    h3 { {t("misc-lines-edit-title")} }
                    for (i, r) in v.lines.iter().cloned().enumerate() {
                        div { key: "{i}", class: "form-grid",
                            TextField { id: "inb03-{i}", label: t("misc-field-line-no"), input_type: "number", value: r.line_no.clone(), oninput: move |x| form.write().lines[i].line_no = x }
                            TextField { id: "inb04-{i}", label: t("misc-field-item"), maxlength: 40usize, value: r.item_no.clone(), oninput: move |x| form.write().lines[i].item_no = x }
                            TextField { id: "inb05-{i}", label: t("misc-field-warehouse"), maxlength: 10usize, value: r.warehouse.clone(), oninput: move |x| form.write().lines[i].warehouse = x }
                            TextField { id: "inb06-{i}", label: t("misc-field-location"), maxlength: 10usize, value: r.location.clone(), oninput: move |x| form.write().lines[i].location = x }
                            TextField { id: "inb07-{i}", label: t("misc-field-lot"), maxlength: 24usize, value: r.lot.clone(), oninput: move |x| form.write().lines[i].lot = x }
                            TextField { id: "inb09-{i}", label: t("misc-field-qty"), input_type: "number", value: r.qty.clone(), oninput: move |x| form.write().lines[i].qty = x }
                            TextField { id: "inb15-{i}", label: t("misc-field-reason"), maxlength: 10usize, value: r.reason.clone(), oninput: move |x| form.write().lines[i].reason = x }
                            Button { icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Text, onclick: move |_| { form.write().lines.remove(i); }, {t("misc-action-remove-line")} }
                        }
                    }
                    Button {
                        icon: icons::ADD,
                        variant: ButtonVariant::Text,
                        onclick: move |_| form.write().lines.push(LineRow::default()),
                        {t("misc-action-add-line")}
                    }
                    p { class: "muted", {t("misc-issue-hint-lines")} }
                }
            }
            if copy_form().is_some() {
                Dialog {
                    title: tf("misc-dialog-copy", &[("no", cur.as_ref().map(|c| c.doc_no.clone()).unwrap_or_default().as_str())]),
                    oncancel: move |_| { copy_form.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { copy_form.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save_copy, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    div { class: "form-grid",
                        TextField { id: "copy-ina01", label: t("misc-field-doc-no"), maxlength: 16usize, value: ct.0.clone(), oninput: move |x| copy_text.write().0 = x }
                        TextField { id: "copy-ina03", label: t("misc-field-input-date"), input_type: "date", value: ct.1.clone(), oninput: move |x| copy_text.write().1 = x }
                        TextField { id: "copy-ina02", label: t("misc-field-posting-date"), input_type: "date", value: ct.2.clone(), oninput: move |x| copy_text.write().2 = x }
                    }
                    p { class: "muted", {t("misc-hint-doc-no")} }
                }
            }
        }
    }
}
