//! Encoding Year Maintenance 編碼年對照維護（TipTop aooi404 年－編碼維護）
//! 版面比照 tiptop/aoo/4fd/aooi404.4fd（EY1）：多筆單身表格（年、長度、編碼編號），新增／修改以小視窗編輯。
//! 共用層：全公司共用（EY2）。鍵值為年＋長度，長度由編碼編號的字數帶出（EY3）。規則在後端 EncodingYearService。
//! 標準功能鈕（ADR 0013；EY8）：查詢（表格第一列輸入條件）、複製到新的年、確認、列印、匯出 Excel、上下筆。

use dioxus::prelude::*;
use ttrs_shared::{
    EncodingYearDto, EncodingYearInput, QbeCondition, QueryRequest,
    encoding_year_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Tabs, TextField, Tone,
    icons,
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

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "encoding-year-maintenance";

#[component]
pub fn EncodingYearMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, YearPage {} }
    }
}

#[component]
fn YearPage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    // 查詢：qbe 為查詢列正在輸入的條件，conditions 為目前生效的條件（QBE4：預設查全部）
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut years = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move { api::query_encoding_years(&req).await }
    });
    let selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        years
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|u| u.key()).collect())
            .unwrap_or_default()
    });
    let mut selected = selected;
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(EncodingYearInput::default);
    let mut dialog_error = use_signal(|| None::<String>);

    let page_data = years.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<EncodingYearDto> = page_data
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
    let cur: Option<EncodingYearDto> = selected()
        .and_then(|c| list.iter().find(|x| x.key() == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::ALL.len() + 2
    } else {
        3
    };
    let keys: Vec<String> = list.iter().map(|u| u.key()).collect();

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        years.restart();
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
            match api::query_encoding_years(&req).await {
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
                Some(Editor::Edit(code)) => api::update_encoding_year(&code, &req).await,
                _ => api::create_encoding_year(&req).await,
            };
            match result {
                Ok(u) => done(
                    tf("encoding-year-msg-saved", &[("code", u.key().as_str())]),
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
                "/api/encoding-years",
                "encoding-years",
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
                div { class: "program-id", {t("encoding-year-breadcrumb")} }
                h2 { {t("encoding-year-title")} }
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
                        // aooi404 是單身式作業：新增、修改、刪除、停用都用 detail
                        detail_action: "detail",
                        insert: if perm.add_row() { Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(EncodingYearInput::default());
                            editor.set(Some(Editor::New));
                        }) } else { None },
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.key())));
                        }),
                        // 複製到新的年（ADR 0013 SA5；EY8）：新的年輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let new_code = copy_code();
                            spawn(async move {
                                match api::copy_encoding_year(&c.key(), new_code.trim()).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("encoding-year-msg-copied", &[("from", c.key().as_str()), ("to", r.key().as_str())]), Some(r.key()));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）
                        delete: if perm.delete_row() { Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match api::delete_encoding_year(&c.key(), c.version).await {
                                    Ok(()) => done(tf("encoding-year-msg-deleted", &[("code", c.key().as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }) } else { None },
                        // 確認／取消確認（ADR 0013 SA9）
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            spawn(async move {
                                match api::set_encoding_year_state(&c.key(), if c.confirmed { "unconfirm" } else { "confirm" }, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "encoding-year-msg-confirmed" } else { "encoding-year-msg-unconfirmed" };
                                        done(tf(id, &[("code", r.key().as_str())]), Some(r.key()));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            spawn(async move {
                                match api::set_encoding_year_state(&c.key(), if c.active { "deactivate" } else { "activate" }, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "encoding-year-msg-activated" } else { "encoding-year-msg-deactivated" };
                                        done(tf(id, &[("code", r.key().as_str())]), Some(r.key()));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        invalid_action: "detail",
                        inactive,
                        invalid_label: Some(("encoding-year-action-deactivate", "encoding-year-action-activate")),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        // 異動記錄（ADR 0012 §8，功能 history）
                        if perm.can("history") {
                            HistoryButton { program: PROGRAM, record: cur.as_ref().map(|c| c.year.to_string()) }
                        }
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("encoding-year-field-copy-as"), input_type: "number", value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
                        }
                    }
                }
            }

            // 上下筆：移動選取列，跨頁自動換頁（ADR 0013 SA2）
            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.key()).as_deref()),
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
                    h3 { {t("encoding-year-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("encoding-year-field-year")} }
                                th { {t("encoding-year-field-length")} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else {
                                    th { {t("encoding-year-field-code")} }
                                }
                            }
                            // 查詢列（在表格第一列輸入條件）
                            if querying() {
                                tr {
                                    for (id, field, hint) in [("encoding-year-field-year", "year", HINT_NUMBER), ("encoding-year-field-length", "length", HINT_NUMBER), ("encoding-year-field-code", "code_value", "")].into_iter().take(if status_tab { 2 } else { 3 }) {
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
                            match &*years.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for u in list.iter().cloned() {
                                        tr {
                                            key: "{u.year}-{u.length}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.key() == u.key()) { "md-table__row--selected" },
                                            onclick: {
                                                let code = u.key();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "num", "{u.year}" }
                                            td { class: "num", "{u.length}" }
                                            if status_tab {
                                                StatusCells { status: u.status() }
                                                ConfirmCells { confirmed: u.confirmed, by: u.confirmed_by.clone() }
                                            } else {
                                                td { class: "mono", "{u.code_value}" }
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
                        Editor::New => t("encoding-year-dialog-new"),
                        Editor::Edit(code) => tf("encoding-year-dialog-edit", &[("code", code.as_str())]),
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
                        TextField {
                            id: "gff01",
                            label: t("encoding-year-field-year"),
                            input_type: "number",
                            readonly: ed != Editor::New,
                            value: v.year.map(|n| n.to_string()).unwrap_or_default(),
                            oninput: move |x: String| form.write().year = x.trim().parse().ok(),
                        }
                        TextField { id: "gff03", label: t("encoding-year-field-code"), maxlength: max_len::CODE, value: v.code_value.clone(), oninput: move |x| form.write().code_value = x }
                        // 長度＝編碼編號的字數，自動帶出、不可輸入（aooi404.4gl:229-234）
                        TextField { id: "gff02", label: t("encoding-year-field-length"), readonly: true, value: v.code_value.trim().chars().count().to_string() }
                    }
                    p { class: "muted", {t("encoding-year-hint")} }
                }
            }
        }
    }
}
