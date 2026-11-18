//! Group Center Maintenance 集團中心維護（TipTop aooi600 集團中心資料維護）
//! 版面比照 topcust/coo/4fd/aooi600.4fd（GC1）：單一表格（類別、集團中心代碼、名稱），新增／修改以小視窗編輯；
//! 選定一筆類別 1 的資料中心後，下方卡片維護它負責的資料類別（aooi600_1：資料類別、營運中心）。
//! 共用層、集團層（GC2）。規則在後端 GroupCenterService。
//! 標準功能鈕（ADR 0013；GC10）：新增、修改、刪除、失效、複製，加上查詢（表格第一列輸入條件）、確認、列印、
//! 匯出 Excel、上下筆、異動記錄；資料類別設定用 TipTop 的 data_type_setting。拋轉設定在 aooi602 移植後才顯示（GC7）。

use dioxus::prelude::*;
use ttrs_shared::{
    DataCenterTypeInput, GroupCenterDto, GroupCenterInput, QbeCondition, QueryRequest,
    group_center_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Select, Tabs, TextField,
    Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::qbe::{ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav};
use crate::components::status::{
    self, ConfirmCells, ConfirmHeaders, ConfirmQbeCells, HasStatus, StatusCells, StatusHeaders,
    StatusQbeCells, TAB_DATA, TAB_STATUS,
};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

/// 類別 1–7（GC4）
fn categories() -> Vec<(String, String)> {
    (1..=7)
        .map(|i| (i.to_string(), t(&format!("group-center-category-{i}"))))
        .collect()
}

fn category_label(code: &str) -> String {
    t(&format!("group-center-category-{code}"))
}

/// 資料類別 0–8（GC5）
fn data_types() -> Vec<(String, String)> {
    (0..=8)
        .map(|i| (i.to_string(), t(&format!("group-center-data-type-{i}"))))
        .collect()
}

fn data_type_label(code: &str) -> String {
    t(&format!("group-center-data-type-{code}"))
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "group-center-maintenance";

#[component]
pub fn GroupCenterMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, CenterPage {} }
    }
}

#[component]
fn CenterPage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut centers = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move { api::query_group_centers(&req).await }
    });
    // 營運中心選單（沒有營運中心維護的查詢權限時改為直接輸入代碼）
    let plants = use_resource(|| async { api::list_plants().await });
    let selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        centers
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|u| u.center_code.clone()).collect())
            .unwrap_or_default()
    });
    let mut selected = selected;
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(GroupCenterInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut type_open = use_signal(|| false);
    let mut type_form = use_signal(DataCenterTypeInput::default);
    let mut type_error = use_signal(|| None::<String>);

    let page_data = centers
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let list: Vec<GroupCenterDto> = page_data
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
    let cur: Option<GroupCenterDto> = selected()
        .and_then(|c| list.iter().find(|x| x.center_code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::ALL.len() + 2
    } else {
        3
    };
    let keys: Vec<String> = list.iter().map(|u| u.center_code.clone()).collect();
    let plant_options: Option<Vec<(String, String)>> = plants
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|ps| {
            ps.iter()
                .map(|p| (p.plant_id.clone(), format!("{} {}", p.plant_id, p.name)))
                .collect()
        });

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        centers.restart();
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
            match api::query_group_centers(&req).await {
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
                Some(Editor::Edit(code)) => api::update_group_center(&code, &req).await,
                _ => api::create_group_center(&req).await,
            };
            match result {
                Ok(u) => done(
                    tf(
                        "group-center-msg-saved",
                        &[("code", u.center_code.as_str())],
                    ),
                    Some(u.center_code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    // 資料類別新增一列（data_type_setting）
    let cur_type = cur.clone();
    let save_type = move |_| {
        let Some(c) = cur_type.clone() else { return };
        let req = type_form();
        spawn(async move {
            match api::add_group_center_data_type(&c.center_code, &req).await {
                Ok(r) => {
                    type_open.set(false);
                    type_error.set(None);
                    done(
                        tf(
                            "group-center-msg-type-saved",
                            &[("code", r.center_code.as_str())],
                        ),
                        Some(r.center_code),
                    );
                }
                Err(e) => type_error.set(Some(e)),
            }
        });
    };

    // 列印、匯出 Excel（ADR 0013 SA3）：依目前生效的查詢條件
    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            if let Err(e) = api::system_list_report(
                "/api/group-centers",
                "group-centers",
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
                div { class: "program-id", {t("group-center-breadcrumb")} }
                h2 { {t("group-center-title")} }
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
                        insert: Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(GroupCenterInput { category: Some("1".into()), ..Default::default() });
                            editor.set(Some(Editor::New));
                        }),
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.center_code)));
                        }),
                        // 複製（ADR 0013 SA5；不複製資料類別設定，GC10）：新代碼輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let new_code = copy_code();
                            spawn(async move {
                                match api::copy_group_center(&c.center_code, new_code.trim()).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("group-center-msg-copied", &[("from", c.center_code.as_str()), ("to", r.center_code.as_str())]), Some(r.center_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）；連同資料類別設定一起刪除（GC8）
                        delete: Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match api::delete_group_center(&c.center_code, c.version).await {
                                    Ok(()) => done(tf("group-center-msg-deleted", &[("code", c.center_code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            spawn(async move {
                                match api::set_group_center_state(&c.center_code, if c.confirmed { "unconfirm" } else { "confirm" }, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "group-center-msg-confirmed" } else { "group-center-msg-unconfirmed" };
                                        done(tf(id, &[("code", r.center_code.as_str())]), Some(r.center_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            spawn(async move {
                                match api::set_group_center_state(&c.center_code, if c.active { "deactivate" } else { "activate" }, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "group-center-msg-activated" } else { "group-center-msg-deactivated" };
                                        done(tf(id, &[("code", r.center_code.as_str())]), Some(r.center_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        inactive,
                        invalid_label: Some(("group-center-action-deactivate", "group-center-action-activate")),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        if perm.can("history") {
                            HistoryButton { program: PROGRAM, record: cur.as_ref().map(|c| c.center_code.clone()) }
                        }
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("group-center-field-copy-as"), maxlength: max_len::CODE, value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
                        }
                    }
                }
            }

            // 上下筆：移動選取列，跨頁自動換頁（ADR 0013 SA2）
            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.center_code.as_str())),
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
                    h3 { {t("group-center-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                if !status_tab {
                                    th { {t("group-center-field-category")} }
                                }
                                th { {t("group-center-field-code")} }
                                th { {t("group-center-field-name")} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                }
                            }
                            if querying() {
                                tr {
                                    for (id, field) in [("group-center-field-category", "category"), ("group-center-field-code", "center_code"), ("group-center-field-name", "name")].into_iter().skip(usize::from(status_tab)) {
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
                                    }
                                }
                            }
                        }
                        tbody {
                            match &*centers.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for u in list.iter().cloned() {
                                        tr {
                                            key: "{u.center_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.center_code == u.center_code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = u.center_code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            if !status_tab {
                                                td { {category_label(&u.category)} }
                                            }
                                            td { class: "mono", "{u.center_code}" }
                                            td { {u.name.clone().unwrap_or_default()} }
                                            if status_tab {
                                                StatusCells { status: u.status() }
                                                ConfirmCells { confirmed: u.confirmed, by: u.confirmed_by.clone() }
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

            // 資料類別設定（aooi600_1）：只有資料中心（類別 1）才顯示（GC1、GC5）
            if let Some(c) = cur.clone().filter(|c| c.category == "1") {
                Card {
                    div { class: "table-header",
                        h3 { {tf("group-center-types-title", &[("code", c.center_code.as_str())])} }
                        if perm.can("data_type_setting") {
                            Button { icon: icons::ADD, tone: Tone::Primary, variant: ButtonVariant::Tonal, disabled: !c.active, onclick: {
                                let c = c.clone();
                                move |_| {
                                    type_error.set(None);
                                    type_form.set(DataCenterTypeInput { data_type: "1".into(), version: Some(c.version), ..Default::default() });
                                    type_open.set(true);
                                }
                            }, {t("group-center-action-add-type")} }
                        }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("group-center-field-data-type")} }
                                    th { {t("group-center-field-plant")} }
                                    th {}
                                }
                            }
                            tbody {
                                if c.data_types.is_empty() {
                                    tr { td { colspan: 3, class: "muted", {t("common-msg-no-data")} } }
                                }
                                for row in c.data_types.iter().cloned() {
                                    tr { key: "{row.data_type}-{row.plant}",
                                        td { {data_type_label(&row.data_type)} }
                                        td { class: "mono", "{row.plant}" }
                                        td {
                                            if perm.can("data_type_setting") {
                                                Button { icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Text, disabled: !c.active, onclick: {
                                                    let (code, row, version) = (c.center_code.clone(), row.clone(), c.version);
                                                    move |_| {
                                                        let (code, row) = (code.clone(), row.clone());
                                                        spawn(async move {
                                                            match api::remove_group_center_data_type(&code, &row.data_type, &row.plant, version).await {
                                                                Ok(r) => done(tf("group-center-msg-type-deleted", &[("code", r.center_code.as_str())]), Some(r.center_code)),
                                                                Err(e) => error.set(Some(e)),
                                                            }
                                                        });
                                                    }
                                                }, {t("common-action-delete")} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    p { class: "query-hint",
                        if c.active { {t("group-center-hint-types")} } else { {t("group-center-hint-inactive")} }
                    }
                }
            }

            if type_open() {
                Dialog {
                    title: t("group-center-dialog-new-type"),
                    oncancel: move |_| { type_open.set(false); type_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { type_open.set(false); type_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save_type, {t("common-action-ok")} }
                    },
                    if let Some(msg) = type_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| type_error.set(None) }
                    }
                    div { class: "form-grid",
                        Select {
                            id: "gev01",
                            label: t("group-center-field-data-type"),
                            options: data_types(),
                            value: type_form().data_type,
                            onchange: move |x| type_form.write().data_type = x,
                        }
                        if let Some(options) = plant_options.clone() {
                            Select {
                                id: "gev02",
                                label: t("group-center-field-plant"),
                                options: std::iter::once((String::new(), String::new())).chain(options).collect::<Vec<_>>(),
                                value: type_form().plant,
                                onchange: move |x| type_form.write().plant = x,
                            }
                        } else {
                            TextField { id: "gev02", label: t("group-center-field-plant"), value: type_form().plant, oninput: move |x| type_form.write().plant = x }
                        }
                    }
                }
            }

            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("group-center-dialog-new"),
                        Editor::Edit(code) => tf("group-center-dialog-edit", &[("code", code.as_str())]),
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
                        Select {
                            id: "geu00",
                            label: t("group-center-field-category"),
                            options: categories(),
                            value: v.category.clone().unwrap_or_default(),
                            onchange: move |x| form.write().category = Some(x),
                        }
                        TextField { id: "geu01", label: t("group-center-field-code"), maxlength: max_len::CODE, value: v.center_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().center_code = x }
                        TextField { id: "geu02", label: t("group-center-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                    }
                    p { class: "muted", {t("group-center-hint")} }
                }
            }
        }
    }
}
