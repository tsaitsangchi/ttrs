//! Currency Maintenance 幣別維護（TipTop aooi050 幣別資料維護）
//! 版面比照 tiptop/aoo/4fd/aooi050.4fd（CY1）：多筆單身表格（幣別、說明、單價／金額／小計總計／匯率小數位數、計算方式），
//! 新增／修改以小視窗編輯。共用層：全公司共用（CY2）。規則在後端 CurrencyService。
//! 標準功能鈕（ADR 0013）：查詢（表格第一列輸入條件）、新增、修改、複製、刪除、確認、停用／啟用、列印、匯出 Excel、
//! 異動記錄、上下筆（移動選取列）。

use dioxus::prelude::*;
use ttrs_shared::{
    CurrencyDto, CurrencyInput, QbeCondition, QueryRequest, currency_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Select, Tabs, TextField,
    Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
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

fn num(v: Option<i16>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

/// 計算方式（azi10）的顯示文字 ID
fn method_label(code: &str) -> &'static str {
    if code == "2" {
        "currency-rate-divide"
    } else {
        "currency-rate-multiply"
    }
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "currency-maintenance";

#[component]
pub fn CurrencyMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, CurrencyPage {} }
    }
}

#[component]
fn CurrencyPage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    // 查詢：qbe 為查詢列正在輸入的條件，conditions 為目前生效的條件（QBE4：預設查全部）
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut currencies = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move { api::query_currencies(&req).await }
    });
    let selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        currencies
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|u| u.currency_code.clone()).collect())
            .unwrap_or_default()
    });
    let mut selected = selected;
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(CurrencyInput::default);
    let mut dialog_error = use_signal(|| None::<String>);

    let page_data = currencies
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let list: Vec<CurrencyDto> = page_data
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
    let cur: Option<CurrencyDto> = selected()
        .and_then(|c| list.iter().find(|x| x.currency_code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::ALL.len() + 2
    } else {
        7
    };
    let keys: Vec<String> = list.iter().map(|u| u.currency_code.clone()).collect();

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        currencies.restart();
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
            match api::query_currencies(&req).await {
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
                Some(Editor::Edit(code)) => api::update_currency(&code, &req).await,
                _ => api::create_currency(&req).await,
            };
            match result {
                Ok(u) => done(
                    tf("currency-msg-saved", &[("code", u.currency_code.as_str())]),
                    Some(u.currency_code),
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
                "/api/currencies",
                "currencies",
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

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("currency-breadcrumb")} }
                h2 { {t("currency-title")} }
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
                        // aooi050 是單身式作業：新增、修改、刪除、停用都用 detail
                        detail_action: "detail",
                        insert: if perm.add_row() { Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            // 新增預設值（CY3、CY4）
                            form.set(CurrencyInput {
                                price_decimals: Some(4),
                                amount_decimals: Some(2),
                                total_decimals: Some(2),
                                rate_decimals: Some(6),
                                rate_method: Some("1".to_owned()),
                                ..Default::default()
                            });
                            editor.set(Some(Editor::New));
                        }) } else { None },
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.currency_code)));
                        }),
                        // 複製（ADR 0013 SA5）：新單位代碼輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let new_code = copy_code();
                            spawn(async move {
                                match api::copy_currency(&c.currency_code, new_code.trim()).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("currency-msg-copied", &[("from", c.currency_code.as_str()), ("to", r.currency_code.as_str())]), Some(r.currency_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）
                        delete: if perm.delete_row() { Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match api::delete_currency(&c.currency_code, c.version).await {
                                    Ok(()) => done(tf("currency-msg-deleted", &[("code", c.currency_code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }) } else { None },
                        // 確認／取消確認（ADR 0013 SA9）
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            spawn(async move {
                                match api::confirm_currency(&c.currency_code, !c.confirmed, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "currency-msg-confirmed" } else { "currency-msg-unconfirmed" };
                                        done(tf(id, &[("code", r.currency_code.as_str())]), Some(r.currency_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            spawn(async move {
                                match api::set_currency_active(&c.currency_code, !c.active, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "currency-msg-activated" } else { "currency-msg-deactivated" };
                                        done(tf(id, &[("code", r.currency_code.as_str())]), Some(r.currency_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        invalid_action: "detail",
                        inactive,
                        invalid_label: Some(("currency-action-deactivate", "currency-action-activate")),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        // 異動記錄（ADR 0012 §8，功能 history）
                        if perm.can("history") {
                            HistoryButton { program: PROGRAM, record: cur.as_ref().map(|c| c.currency_code.clone()) }
                        }
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("currency-field-copy-as"), maxlength: max_len::CODE, value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
                        }
                    }
                }
            }

            // 上下筆：移動選取列，跨頁自動換頁（ADR 0013 SA2）
            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.currency_code.as_str())),
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
                    h3 { {t("currency-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("currency-field-code")} }
                                th { {t("currency-field-name")} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else {
                                    th { {t("currency-field-price-decimals")} }
                                    th { {t("currency-field-amount-decimals")} }
                                    th { {t("currency-field-total-decimals")} }
                                    th { {t("currency-field-rate-decimals")} }
                                    th { {t("currency-field-rate-method")} }
                                }
                            }
                            // 查詢列（在表格第一列輸入條件）
                            if querying() {
                                tr {
                                    for (id, field, hint) in [
                                        ("currency-field-code", "currency_code", ""),
                                        ("currency-field-name", "name", ""),
                                        ("currency-field-price-decimals", "price_decimals", HINT_NUMBER),
                                        ("currency-field-amount-decimals", "amount_decimals", HINT_NUMBER),
                                        ("currency-field-total-decimals", "total_decimals", HINT_NUMBER),
                                        ("currency-field-rate-decimals", "rate_decimals", HINT_NUMBER),
                                        ("currency-field-rate-method", "rate_method", "currency-hint-rate-method"),
                                    ].into_iter().take(if status_tab { 2 } else { 7 }) {
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
                            match &*currencies.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for u in list.iter().cloned() {
                                        tr {
                                            key: "{u.currency_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.currency_code == u.currency_code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = u.currency_code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{u.currency_code}" }
                                            td { {u.name.clone().unwrap_or_default()} }
                                            if status_tab {
                                                StatusCells { status: u.status() }
                                                ConfirmCells { confirmed: u.confirmed, by: u.confirmed_by.clone() }
                                            } else {
                                                td { class: "num", "{u.price_decimals}" }
                                                td { class: "num", "{u.amount_decimals}" }
                                                td { class: "num", "{u.total_decimals}" }
                                                td { class: "num", "{u.rate_decimals}" }
                                                td { {t(method_label(&u.rate_method))} }
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
                        Editor::New => t("currency-dialog-new"),
                        Editor::Edit(code) => tf("currency-dialog-edit", &[("code", code.as_str())]),
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
                        TextField { id: "azi01", label: t("currency-field-code"), maxlength: max_len::CODE, value: v.currency_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().currency_code = x }
                        TextField { id: "azi02", label: t("currency-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                        TextField { id: "azi03", label: t("currency-field-price-decimals"), input_type: "number", value: num(v.price_decimals), oninput: move |x: String| form.write().price_decimals = x.trim().parse().ok() }
                        TextField { id: "azi04", label: t("currency-field-amount-decimals"), input_type: "number", value: num(v.amount_decimals), oninput: move |x: String| form.write().amount_decimals = x.trim().parse().ok() }
                        TextField { id: "azi05", label: t("currency-field-total-decimals"), input_type: "number", value: num(v.total_decimals), oninput: move |x: String| form.write().total_decimals = x.trim().parse().ok() }
                        TextField { id: "azi07", label: t("currency-field-rate-decimals"), input_type: "number", value: num(v.rate_decimals), oninput: move |x: String| form.write().rate_decimals = x.trim().parse().ok() }
                        Select {
                            id: "azi10",
                            label: t("currency-field-rate-method"),
                            options: vec![("1".to_owned(), t("currency-rate-multiply")), ("2".to_owned(), t("currency-rate-divide"))],
                            value: v.rate_method.clone().unwrap_or_else(|| "1".to_owned()),
                            onchange: move |x: String| form.write().rate_method = Some(x),
                        }
                        // 匯率自動匯入的來源代號（IM6）
                        TextField { id: "import-code", label: t("currency-field-import-code"), maxlength: max_len::CODE, value: v.import_code.clone().unwrap_or_default(), oninput: move |x: String| form.write().import_code = opt(x) }
                    }
                    p { class: "muted", {t("currency-hint-decimals")} }
                }
            }
        }
    }
}
