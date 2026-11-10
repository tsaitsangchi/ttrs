//! Product Classification Maintenance 產品分類維護（TipTop axmi110）
//! 版面比照 tiptop/axm/4fd/axmi110.4fd（PC1）：多筆單身表格（產品分類碼、分類名稱、計劃期別格式、期數、成本中心、
//! 銷貨收入科目一、二），新增／修改以小視窗編輯。工廠層：用登入的營運中心（PC2）。規則在後端 ProductClassService。

use dioxus::prelude::*;
use ttrs_shared::{
    ProductClassDto, ProductClassInput, QbeCondition, QueryRequest,
    product_class_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Tabs, TextField, Tone,
    icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::components::qbe::{
    HINT_NUMBER, ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav,
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

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 查詢列的欄位（axmi110.4fd 順序）：（標題翻譯 ID, 查詢欄位鍵值, 查詢提示翻譯 ID）
const Q_COLUMNS: [(&str, &str, &str); 7] = [
    ("pclass-field-code", "class_code", ""),
    ("pclass-field-name", "name", ""),
    ("pclass-field-period-format", "period_format", ""),
    ("pclass-field-periods", "periods", HINT_NUMBER),
    ("pclass-field-cost-center", "cost_center", ""),
    ("pclass-field-revenue-account", "revenue_account", ""),
    ("pclass-field-revenue-account2", "revenue_account2", ""),
];

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
    // 查詢：qbe 為查詢列正在輸入的條件，conditions 為目前生效的條件（QBE4：預設查全部）
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut classes = use_resource(move || {
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
            api::query_product_classes(&p, &req).await
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        classes
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|c| c.class_code.clone()).collect())
            .unwrap_or_default()
    });
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(ProductClassInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);

    let page_data = classes
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let list: Vec<ProductClassDto> = page_data
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
    let cur: Option<ProductClassDto> = selected()
        .and_then(|c| list.iter().find(|x| x.class_code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let keys: Vec<String> = list.iter().map(|c| c.class_code.clone()).collect();
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
        dialog_error.set(None);
        selected.set(code);
        classes.restart();
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
        let p = plant();
        spawn(async move {
            match api::query_product_classes(&p, &req).await {
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

    // 列印、匯出 Excel（axmi110 output／exporttoexcel；ADR 0013 SA3）：依目前生效的查詢條件
    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        let p = plant();
        spawn(async move {
            if let Err(e) = api::list_report(
                &p,
                "product-classes",
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
    let cur_copy = cur.clone();
    let cur_confirm = cur.clone();
    let inactive = cur.as_ref().is_some_and(|c| !c.active);
    let confirmed = cur.as_ref().is_some_and(|c| c.confirmed);
    let v = form();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("pclass-breadcrumb")} }
                h2 { {t("pclass-title")} }
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
                        // axmi110 是單身式作業：新增、修改、刪除、停用都用 detail
                        detail_action: "detail",
                        insert: if perm.add_row() { Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(ProductClassInput::default());
                            editor.set(Some(Editor::New));
                        }) } else { None },
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.class_code)));
                        }),
                        // 複製（ADR 0013 SA5）：新分類碼輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let p = plant();
                            let new_code = copy_code();
                            spawn(async move {
                                match api::copy_product_class(&p, &c.class_code, new_code.trim()).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("pclass-msg-copied", &[("from", c.class_code.as_str()), ("to", r.class_code.as_str())]), Some(r.class_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）
                        delete: if perm.delete_row() { Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::delete_product_class(&p, &c.class_code, c.version).await {
                                    Ok(()) => done(tf("pclass-msg-deleted", &[("code", c.class_code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }) } else { None },
                        // 確認／取消確認（ADR 0013 SA9）
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::confirm_product_class(&p, &c.class_code, !c.confirmed, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "pclass-msg-confirmed" } else { "pclass-msg-unconfirmed" };
                                        done(tf(id, &[("code", r.class_code.as_str())]), Some(r.class_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
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
                        }),
                        invalid_action: "detail",
                        inactive,
                        invalid_label: Some(("pclass-action-deactivate", "pclass-action-activate")),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        history: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_history.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::product_class_history(&p, &c.class_code).await {
                                    Ok(h) => history.set(Some((c.class_code.clone(), h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("pclass-field-copy-as"), maxlength: max_len::CODE, value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
                        }
                    }
                }
            }

            // 上下筆：移動選取列，跨頁自動換頁（ADR 0013 SA2）
            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.class_code.as_str())),
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
                    h3 { {t("pclass-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("pclass-field-code")} }
                                th { {t("pclass-field-name")} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else {
                                    th { {t("pclass-field-period-format")} }
                                    th { {t("pclass-field-periods")} }
                                    th { {t("pclass-field-cost-center")} }
                                    th { {t("pclass-field-revenue-account")} }
                                    th { {t("pclass-field-revenue-account2")} }
                                }
                            }
                            // 查詢列（在表格第一列輸入條件）
                            if querying() {
                                tr {
                                    for (id, field, hint) in Q_COLUMNS.into_iter().take(if status_tab { 2 } else { Q_COLUMNS.len() }) {
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
                                                ConfirmCells { confirmed: c.confirmed, by: c.confirmed_by.clone() }
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
                ListPager { page, pages }
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
