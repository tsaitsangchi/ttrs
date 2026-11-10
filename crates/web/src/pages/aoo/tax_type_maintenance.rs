//! Tax Type Maintenance 稅別維護（TipTop aooi150 稅別資料維護）
//! 版面比照 tiptop/aoo/4fd/aooi150.4fd（TX1）：多筆單身表格（稅別碼、名稱、進銷項、會計科目、會計科目二、發票聯數、
//! 課稅別、稅率、單價含稅否、申報格式），新增／修改以小視窗編輯。鍵值為稅別碼＋進銷項。
//! 共用層：全公司共用（TX2）；只做台灣版（TX3）。規則在後端 TaxTypeService。
//! 標準功能鈕（ADR 0013）：查詢、新增、修改、複製、刪除、確認、停用／啟用、列印、匯出 Excel、異動記錄、上下筆。

use dioxus::prelude::*;
use ttrs_shared::{
    QbeCondition, QueryRequest, TaxTypeDto, TaxTypeInput, tax_type_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Select, Tabs, TextField,
    Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::qbe::{
    HINT_FLAG, HINT_NUMBER, ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav,
};
use crate::components::status::{
    self, ConfirmCells, ConfirmHeaders, ConfirmQbeCells, HasStatus, StatusCells, StatusHeaders,
    StatusQbeCells, TAB_DATA, TAB_STATUS,
};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

/// 代碼的顯示文字：前綴＋代碼（如 tax-class-1）；空白為空字串
fn label(prefix: &str, code: Option<&str>) -> String {
    code.map(|c| t(&format!("{prefix}{}", c.to_lowercase())))
        .unwrap_or_default()
}

/// 下拉選單選項：（代碼, 文字 ID 前綴）
fn options(prefix: &str, codes: &[&str], blank: bool) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = if blank {
        vec![(String::new(), String::new())]
    } else {
        Vec::new()
    };
    v.extend(
        codes
            .iter()
            .map(|c| ((*c).to_owned(), t(&format!("{prefix}{}", c.to_lowercase())))),
    );
    v
}

/// 媒體申報格式選項（aooi150.4fd）：進項 21–29、銷項 31–38、XX
fn filing_options(direction: &str) -> Vec<(String, String)> {
    let range = if direction == "1" { 21..=29 } else { 31..=38 };
    std::iter::once(("XX".to_owned(), t("tax-filing-none")))
        .chain(range.map(|n: u8| (n.to_string(), n.to_string())))
        .collect()
}

/// 複製目標：「新稅別碼」或「新稅別碼 進銷項代碼」；未指定進銷項時沿用原資料
fn copy_target(input: &str, direction: &str) -> (String, String) {
    let mut parts = input.split_whitespace();
    let code = parts.next().unwrap_or_default().to_owned();
    let dir = parts.next().unwrap_or(direction).to_owned();
    (code, dir)
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "tax-type-maintenance";

#[component]
pub fn TaxTypeMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, TaxTypePage {} }
    }
}

#[component]
fn TaxTypePage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    // 查詢：qbe 為查詢列正在輸入的條件，conditions 為目前生效的條件（QBE4：預設查全部）
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut taxes = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move { api::query_tax_types(&req).await }
    });
    let selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        taxes
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|u| u.key()).collect())
            .unwrap_or_default()
    });
    let mut selected = selected;
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(TaxTypeInput::default);
    let mut dialog_error = use_signal(|| None::<String>);

    let page_data = taxes.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<TaxTypeDto> = page_data
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
    let cur: Option<TaxTypeDto> = selected()
        .and_then(|c| list.iter().find(|x| x.key() == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::ALL.len() + 2
    } else {
        9
    };
    let keys: Vec<String> = list.iter().map(|u| u.key()).collect();

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        taxes.restart();
    };

    // 執行查詢：先試查第 1 頁，條件有誤時留在查詢列顯示錯誤
    let run_query = move |_| {
        let conds = conditions_of(&qbe.read());
        let req = QueryRequest {
            conditions: conds.clone(),
            page: Some(1),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        spawn(async move {
            match api::query_tax_types(&req).await {
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
        let target = editor();
        let req = form();
        spawn(async move {
            let result = match target {
                Some(Editor::Edit(key)) => api::update_tax_type(&key, &req).await,
                _ => api::create_tax_type(&req).await,
            };
            match result {
                Ok(u) => done(
                    tf("tax-msg-saved", &[("code", u.tax_code.as_str())]),
                    Some(u.key()),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    // 列印、匯出 Excel（ADR 0013 SA3）：依目前生效的查詢條件
    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            if let Err(e) = api::system_list_report(
                "/api/tax-types",
                "tax-types",
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

    let cur_edit = cur.clone();
    let cur_delete = cur.clone();
    let cur_toggle = cur.clone();
    let cur_copy = cur.clone();
    let cur_confirm = cur.clone();
    let inactive = cur.as_ref().is_some_and(|c| !c.active);
    let confirmed = cur.as_ref().is_some_and(|c| c.confirmed);
    let v = form();
    // 零稅率、免稅：稅率與單價含稅否不可輸入（aooi150.4gl:1000-1012）
    let fixed_rate = matches!(v.tax_class.as_deref(), Some("2" | "3"));

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("tax-breadcrumb")} }
                h2 { {t("tax-title")} }
            }

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
                        // aooi150 是單身式作業：新增、修改、刪除、停用都用 detail
                        detail_action: "detail",
                        insert: if perm.add_row() { Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            // 新增預設值（aooi150.4gl:260-273、TX6）：銷項、不含稅、不申報
                            form.set(TaxTypeInput {
                                direction: "2".to_owned(),
                                include_tax: Some(false),
                                filing_format: Some("XX".to_owned()),
                                ..Default::default()
                            });
                            editor.set(Some(Editor::New));
                        }) } else { None },
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.key())));
                        }),
                        // 複製（ADR 0013 SA5）：新單位代碼輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let new_code = copy_code();
                            spawn(async move {
                                let (to_code, to_dir) = copy_target(&new_code, &c.direction);
                                match api::copy_tax_type(&c.tax_code, &c.direction, &to_code, &to_dir).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("tax-msg-copied", &[("from", c.tax_code.as_str()), ("to", r.tax_code.as_str())]), Some(r.key()));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）
                        delete: if perm.delete_row() { Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match api::delete_tax_type(&c.tax_code, &c.direction, c.version).await {
                                    Ok(()) => done(tf("tax-msg-deleted", &[("code", c.tax_code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }) } else { None },
                        // 確認／取消確認（ADR 0013 SA9）
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            spawn(async move {
                                match api::confirm_tax_type(&c.tax_code, &c.direction, !c.confirmed, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "tax-msg-confirmed" } else { "tax-msg-unconfirmed" };
                                        done(tf(id, &[("code", r.tax_code.as_str())]), Some(r.key()));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            spawn(async move {
                                match api::set_tax_type_active(&c.tax_code, &c.direction, !c.active, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "tax-msg-activated" } else { "tax-msg-deactivated" };
                                        done(tf(id, &[("code", r.tax_code.as_str())]), Some(r.key()));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        invalid_action: "detail",
                        inactive,
                        invalid_label: Some(("tax-action-deactivate", "tax-action-activate")),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        // 異動記錄（ADR 0012 §8，功能 history）
                        if perm.can("history") {
                            // 異動記錄以稅別碼查詢（含進項與銷項）
                            HistoryButton { program: PROGRAM, record: cur.as_ref().map(|c| c.tax_code.clone()) }
                        }
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("tax-field-copy-as"), maxlength: max_len::CODE, value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
                        }
                    }
                }
            }

            // 上下筆：移動選取列，跨頁自動換頁（ADR 0013 SA2）
            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.tax_code.as_str())),
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
                    h3 { {t("tax-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("tax-field-code")} }
                                th { {t("tax-field-name")} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else {
                                    th { {t("tax-field-direction")} }
                                    th { {t("tax-field-account")} }
                                    th { {t("tax-field-account2")} }
                                    th { {t("tax-field-invoice-copies")} }
                                    th { {t("tax-field-class")} }
                                    th { {t("tax-field-rate")} }
                                    th { {t("tax-field-include-tax")} }
                                    th { {t("tax-field-filing-format")} }
                                }
                            }
                            // 查詢列（在表格第一列輸入條件）
                            if querying() {
                                tr {
                                    for (id, field, hint) in [
                                        ("tax-field-code", "tax_code", ""),
                                        ("tax-field-name", "name", ""),
                                        ("tax-field-direction", "direction", "tax-hint-direction"),
                                        ("tax-field-account", "account", ""),
                                        ("tax-field-account2", "account2", ""),
                                        ("tax-field-invoice-copies", "invoice_copies", ""),
                                        ("tax-field-class", "tax_class", "tax-hint-class"),
                                        ("tax-field-rate", "rate", HINT_NUMBER),
                                        ("tax-field-include-tax", "include_tax", HINT_FLAG),
                                        ("tax-field-filing-format", "filing_format", ""),
                                    ].into_iter().take(if status_tab { 2 } else { 10 }) {
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
                                    if status_tab {
                                        StatusQbeCells { queryable: status::ALL, qbe }
                                        ConfirmQbeCells { qbe }
                                    }
                                }
                            }
                        }
                        tbody {
                            match &*taxes.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for u in list.iter().cloned() {
                                        tr {
                                            key: "{u.key()}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.key() == u.key()) { "md-table__row--selected" },
                                            onclick: {
                                                let code = u.key();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{u.tax_code}" }
                                            td { {u.name.clone().unwrap_or_default()} }
                                            if status_tab {
                                                StatusCells { status: u.status() }
                                                ConfirmCells { confirmed: u.confirmed, by: u.confirmed_by.clone() }
                                            } else {
                                                td { {label("tax-direction-", Some(&u.direction))} }
                                                td { class: "mono", {u.account.clone().unwrap_or_default()} }
                                                td { class: "mono", {u.account2.clone().unwrap_or_default()} }
                                                td { {label("tax-copies-", u.invoice_copies.as_deref())} }
                                                td { {label("tax-class-", u.tax_class.as_deref())} }
                                                td { class: "num", "{u.rate}" }
                                                td { {t(if u.include_tax { "common-yes" } else { "common-no" })} }
                                                td { class: "mono", "{u.filing_format}" }
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

            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("tax-dialog-new"),
                        Editor::Edit(code) => tf("tax-dialog-edit", &[("code", code.as_str())]),
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
                        TextField { id: "gec01", label: t("tax-field-code"), maxlength: max_len::CODE, value: v.tax_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().tax_code = x }
                        Select {
                            id: "gec011",
                            label: t("tax-field-direction"),
                            options: options("tax-direction-", &["1", "2"], false),
                            value: v.direction.clone(),
                            readonly: ed != Editor::New,
                            onchange: move |x: String| form.write().direction = x,
                        }
                        TextField { id: "gec02", label: t("tax-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                        TextField { id: "gec03", label: t("tax-field-account"), maxlength: max_len::ACCOUNT, value: v.account.clone().unwrap_or_default(), oninput: move |x| form.write().account = opt(x) }
                        TextField { id: "gec031", label: t("tax-field-account2"), maxlength: max_len::ACCOUNT, value: v.account2.clone().unwrap_or_default(), oninput: move |x| form.write().account2 = opt(x) }
                        Select {
                            id: "gec05",
                            label: t("tax-field-invoice-copies"),
                            options: options("tax-copies-", &["0", "2", "3", "4", "X"], true),
                            value: v.invoice_copies.clone().unwrap_or_default(),
                            onchange: move |x: String| form.write().invoice_copies = opt(x),
                        }
                        Select {
                            id: "gec06",
                            label: t("tax-field-class"),
                            options: options("tax-class-", &["1", "2", "3"], true),
                            value: v.tax_class.clone().unwrap_or_default(),
                            onchange: move |x: String| form.write().tax_class = opt(x),
                        }
                        // 零稅率、免稅的稅率固定為 0、不含稅（aooi150.4gl:1000-1012）
                        TextField {
                            id: "gec04",
                            label: t("tax-field-rate"),
                            input_type: "number",
                            disabled: fixed_rate,
                            value: v.rate.map(|r| r.to_string()).unwrap_or_default(),
                            oninput: move |x: String| form.write().rate = x.trim().parse().ok(),
                        }
                        Select {
                            id: "gec07",
                            label: t("tax-field-include-tax"),
                            options: vec![("N".to_owned(), t("common-no")), ("Y".to_owned(), t("common-yes"))],
                            value: if v.include_tax.unwrap_or(false) { "Y".to_owned() } else { "N".to_owned() },
                            disabled: fixed_rate,
                            onchange: move |x: String| form.write().include_tax = Some(x == "Y"),
                        }
                        Select {
                            id: "gec08",
                            label: t("tax-field-filing-format"),
                            options: filing_options(&v.direction),
                            value: v.filing_format.clone().unwrap_or_else(|| "XX".to_owned()),
                            onchange: move |x: String| form.write().filing_format = Some(x),
                        }
                    }
                    p { class: "muted", {t("tax-hint-rules")} }
                }
            }
        }
    }
}
