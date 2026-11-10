//! Unit Maintenance 單位維護（TipTop aooi101 單位資料維護）
//! 版面比照 tiptop/aoo/4fd/aooi101.4fd（UN1）：多筆單身表格（單位、說明、小數位數），新增／修改以小視窗編輯。
//! 共用層：全公司共用（UN2）。規則在後端 UnitService。
//! 標準功能鈕（ADR 0013；unit_maintenance_standard_actions.feature）：查詢（表格第一列輸入條件）、複製、確認、列印、
//! 匯出 Excel、上下筆（移動選取列）。

use dioxus::prelude::*;
use ttrs_shared::{QbeCondition, QueryRequest, UnitDto, UnitInput, unit_max_len as max_len};

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

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "unit-maintenance";

#[component]
pub fn UnitMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, UnitPage {} }
    }
}

#[component]
fn UnitPage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    // 查詢：qbe 為查詢列正在輸入的條件，conditions 為目前生效的條件（QBE4：預設查全部）
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut units = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move { api::query_units(&req).await }
    });
    let selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        units
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|u| u.unit_code.clone()).collect())
            .unwrap_or_default()
    });
    let mut selected = selected;
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(UnitInput::default);
    let mut dialog_error = use_signal(|| None::<String>);

    let page_data = units.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<UnitDto> = page_data
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
    let cur: Option<UnitDto> = selected()
        .and_then(|c| list.iter().find(|x| x.unit_code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::ALL.len() + 2
    } else {
        3
    };
    let keys: Vec<String> = list.iter().map(|u| u.unit_code.clone()).collect();

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        units.restart();
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
            match api::query_units(&req).await {
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
                Some(Editor::Edit(code)) => api::update_unit(&code, &req).await,
                _ => api::create_unit(&req).await,
            };
            match result {
                Ok(u) => done(
                    tf("unit-msg-saved", &[("code", u.unit_code.as_str())]),
                    Some(u.unit_code),
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
                "/api/units",
                "units",
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
                div { class: "program-id", {t("unit-breadcrumb")} }
                h2 { {t("unit-title")} }
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
                        // aooi101 是單身式作業：新增、修改、刪除、停用都用 detail
                        detail_action: "detail",
                        insert: if perm.add_row() { Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(UnitInput::default());
                            editor.set(Some(Editor::New));
                        }) } else { None },
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.unit_code)));
                        }),
                        // 複製（ADR 0013 SA5）：新單位代碼輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let new_code = copy_code();
                            spawn(async move {
                                match api::copy_unit(&c.unit_code, new_code.trim()).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("unit-msg-copied", &[("from", c.unit_code.as_str()), ("to", r.unit_code.as_str())]), Some(r.unit_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）
                        delete: if perm.delete_row() { Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match api::delete_unit(&c.unit_code, c.version).await {
                                    Ok(()) => done(tf("unit-msg-deleted", &[("code", c.unit_code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }) } else { None },
                        // 確認／取消確認（ADR 0013 SA9）
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            spawn(async move {
                                match api::confirm_unit(&c.unit_code, !c.confirmed, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "unit-msg-confirmed" } else { "unit-msg-unconfirmed" };
                                        done(tf(id, &[("code", r.unit_code.as_str())]), Some(r.unit_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            spawn(async move {
                                match api::set_unit_active(&c.unit_code, !c.active, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "unit-msg-activated" } else { "unit-msg-deactivated" };
                                        done(tf(id, &[("code", r.unit_code.as_str())]), Some(r.unit_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        invalid_action: "detail",
                        inactive,
                        invalid_label: Some(("unit-action-deactivate", "unit-action-activate")),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        // 異動記錄（ADR 0012 §8，功能 history）
                        if perm.can("history") {
                            HistoryButton { program: PROGRAM, record: cur.as_ref().map(|c| c.unit_code.clone()) }
                        }
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("unit-field-copy-as"), maxlength: max_len::CODE, value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
                        }
                    }
                }
            }

            // 上下筆：移動選取列，跨頁自動換頁（ADR 0013 SA2）
            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.unit_code.as_str())),
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
                    h3 { {t("unit-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("unit-field-code")} }
                                th { {t("unit-field-name")} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else {
                                    th { {t("unit-field-decimals")} }
                                }
                            }
                            // 查詢列（在表格第一列輸入條件）
                            if querying() {
                                tr {
                                    for (id, field, hint) in [("unit-field-code", "unit_code", ""), ("unit-field-name", "name", ""), ("unit-field-decimals", "decimals", HINT_NUMBER)].into_iter().take(if status_tab { 2 } else { 3 }) {
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
                            match &*units.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for u in list.iter().cloned() {
                                        tr {
                                            key: "{u.unit_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.unit_code == u.unit_code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = u.unit_code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{u.unit_code}" }
                                            td { {u.name.clone().unwrap_or_default()} }
                                            if status_tab {
                                                StatusCells { status: u.status() }
                                                ConfirmCells { confirmed: u.confirmed, by: u.confirmed_by.clone() }
                                            } else {
                                                td { class: "num", "{u.decimals}" }
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
                        Editor::New => t("unit-dialog-new"),
                        Editor::Edit(code) => tf("unit-dialog-edit", &[("code", code.as_str())]),
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
                        TextField { id: "gfe01", label: t("unit-field-code"), maxlength: max_len::CODE, value: v.unit_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().unit_code = x }
                        TextField { id: "gfe02", label: t("unit-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                        TextField {
                            id: "gfe03",
                            label: t("unit-field-decimals"),
                            input_type: "number",
                            value: v.decimals.map(|n| n.to_string()).unwrap_or_default(),
                            oninput: move |x: String| form.write().decimals = x.trim().parse().ok(),
                        }
                    }
                    p { class: "muted", {t("unit-hint-decimals")} }
                }
            }
        }
    }
}
