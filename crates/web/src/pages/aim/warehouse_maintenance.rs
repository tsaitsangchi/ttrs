//! Warehouse Maintenance 倉庫儲位維護（TipTop aimi201、aimi200）
//! 版面比照 tiptop/aim/4fd/aimi201.4fd：單頭倉庫（imd_file）、單身儲位（ime_file）。工廠層：用登入的營運中心。
//! 倉庫新增／修改以小視窗編輯；儲位在下方卡片維護（detail）。規則在後端 WarehouseService。

use dioxus::prelude::*;
use ttrs_shared::{
    QbeCondition, QueryRequest, WarehouseDto, WarehouseInput, WarehouseLocationDto,
    WarehouseLocationInput, warehouse_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Dialog, Notification, NotificationKind, Select, Tabs,
    TextField, Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::components::qbe::{
    HINT_FLAG, ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav,
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

fn txt(v: &Option<String>) -> String {
    v.clone().unwrap_or_default()
}

fn yn(v: bool) -> String {
    if v { "Y" } else { "N" }.to_owned()
}

/// 倉儲類別 imd10／ime04：S 一般倉、W 在製品倉
fn class_options() -> Vec<(String, String)> {
    ["S", "W"]
        .iter()
        .map(|c| {
            (
                (*c).to_owned(),
                format!(
                    "{c} {}",
                    t(&format!("warehouse-class-{}", c.to_lowercase()))
                ),
            )
        })
        .collect()
}

/// 會計科目方式 ime10：0 依料件、1 依倉庫、2 依儲位
fn method_options() -> Vec<(String, String)> {
    ["0", "1", "2"]
        .iter()
        .map(|c| {
            (
                (*c).to_owned(),
                format!("{c} {}", t(&format!("warehouse-method-{c}"))),
            )
        })
        .collect()
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 儲位小視窗：None 為新增，Some 為修改該儲位
#[derive(Clone, PartialEq)]
struct LocEditor(Option<String>);

/// 查詢列的欄位（aimi201.4fd 順序）：（標題翻譯 ID, 查詢欄位鍵值, 查詢提示翻譯 ID）
const Q_COLUMNS: [(&str, &str, &str); 8] = [
    ("warehouse-field-code", "warehouse_code", ""),
    ("warehouse-field-name", "name", ""),
    ("warehouse-field-class", "storage_class", ""),
    ("warehouse-field-usable", "usable", HINT_FLAG),
    ("warehouse-field-mrp", "mrp_usable", HINT_FLAG),
    ("warehouse-field-bonded", "bonded", HINT_FLAG),
    ("warehouse-field-cost-center", "cost_center", ""),
    ("warehouse-field-location", "location_code", ""),
];

/// 權限檢查用的程式代號
const PROGRAM: &str = "warehouse-maintenance";

#[component]
pub fn WarehouseMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, WarehousePage {} }
    }
}

#[component]
fn WarehousePage() -> Element {
    let perm = use_context::<Perm>();
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut warehouses = use_resource(move || {
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
            api::query_warehouses(&p, &req).await
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        warehouses
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|c| c.warehouse_code.clone()).collect())
            .unwrap_or_default()
    });
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(WarehouseInput::default);
    let mut loc_editor = use_signal(|| None::<LocEditor>);
    let mut loc_form = use_signal(WarehouseLocationInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);

    let page_data = warehouses
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let list: Vec<WarehouseDto> = page_data
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
    let cur: Option<WarehouseDto> = selected()
        .and_then(|c| list.iter().find(|x| x.warehouse_code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let keys: Vec<String> = list.iter().map(|c| c.warehouse_code.clone()).collect();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::ALL.len() + 2
    } else {
        8
    };

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        loc_editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        warehouses.restart();
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
            match api::query_warehouses(&p, &req).await {
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

    // 列印、匯出 Excel（ADR 0013 SA3）：依目前生效的查詢條件
    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        let p = plant();
        spawn(async move {
            if let Err(e) = api::list_report(
                &p,
                "warehouses",
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
                Some(Editor::Edit(code)) => api::update_warehouse(&p, &code, &req).await,
                _ => api::create_warehouse(&p, &req).await,
            };
            match result {
                Ok(w) => done(
                    tf(
                        "warehouse-msg-saved",
                        &[("code", w.warehouse_code.as_str())],
                    ),
                    Some(w.warehouse_code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_loc_save = cur.clone();
    let save_location = move |_| {
        let Some(w) = cur_loc_save.clone() else {
            return;
        };
        let p = plant();
        let edit = loc_editor().is_some_and(|e| e.0.is_some());
        let mut req = loc_form();
        req.version = Some(w.version);
        spawn(async move {
            match api::save_warehouse_location(&p, &w.warehouse_code, edit, &req).await {
                Ok(r) => done(
                    tf(
                        "warehouse-msg-location-saved",
                        &[
                            ("code", r.warehouse_code.as_str()),
                            ("location", req.location_code.as_str()),
                        ],
                    ),
                    Some(r.warehouse_code),
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
    let lv = loc_form();
    let can_detail = perm.can("detail");
    let add_row = perm.add_row();
    let delete_row = perm.delete_row();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("warehouse-breadcrumb")} }
                h2 { {t("warehouse-title")} }
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
                            form.set(WarehouseInput { storage_class: "S".into(), ..Default::default() });
                            editor.set(Some(Editor::New));
                        }),
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.warehouse_code)));
                        }),
                        // 複製（ADR 0013 SA5）：新倉庫代號輸入在功能列後方，儲位一併複製
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let p = plant();
                            let new_code = copy_code();
                            spawn(async move {
                                match api::copy_warehouse(&p, &c.warehouse_code, new_code.trim()).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("warehouse-msg-copied", &[("from", c.warehouse_code.as_str()), ("to", r.warehouse_code.as_str())]), Some(r.warehouse_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）；料件或分群使用中也不可刪
                        delete: Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::delete_warehouse(&p, &c.warehouse_code, c.version).await {
                                    Ok(()) => done(tf("warehouse-msg-deleted", &[("code", c.warehouse_code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            let p = plant();
                            let state = if c.confirmed { "unconfirm" } else { "confirm" };
                            spawn(async move {
                                match api::set_warehouse_state(&p, &c.warehouse_code, state, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "warehouse-msg-confirmed" } else { "warehouse-msg-unconfirmed" };
                                        done(tf(id, &[("code", r.warehouse_code.as_str())]), Some(r.warehouse_code));
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
                                match api::set_warehouse_state(&p, &c.warehouse_code, state, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "warehouse-msg-activated" } else { "warehouse-msg-deactivated" };
                                        done(tf(id, &[("code", r.warehouse_code.as_str())]), Some(r.warehouse_code));
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
                                match api::warehouse_history(&p, &c.warehouse_code).await {
                                    Ok(h) => history.set(Some((c.warehouse_code.clone(), h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("warehouse-field-copy-as"), maxlength: max_len::CODE, value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
                        }
                    }
                }
            }

            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.warehouse_code.as_str())),
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
                    h3 { {t("warehouse-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("warehouse-field-code")} }
                                th { {t("warehouse-field-name")} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else {
                                    th { {t("warehouse-field-class")} }
                                    th { {t("warehouse-field-usable")} }
                                    th { {t("warehouse-field-mrp")} }
                                    th { {t("warehouse-field-bonded")} }
                                    th { {t("warehouse-field-cost-center")} }
                                    th { {t("warehouse-field-location-count")} }
                                }
                            }
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
                            match &*warehouses.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for c in list.iter().cloned() {
                                        tr {
                                            key: "{c.warehouse_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.warehouse_code == c.warehouse_code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = c.warehouse_code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{c.warehouse_code}" }
                                            td { {txt(&c.name)} }
                                            if status_tab {
                                                StatusCells { status: c.status() }
                                                ConfirmCells { confirmed: c.confirmed, by: c.confirmed_by.clone() }
                                            } else {
                                                td { class: "mono", "{c.storage_class}" }
                                                td { class: "mono", {yn(c.usable)} }
                                                td { class: "mono", {yn(c.mrp_usable)} }
                                                td { class: "mono", {yn(c.bonded)} }
                                                td {
                                                    span { class: "mono", {txt(&c.cost_center)} }
                                                    " "
                                                    {txt(&c.cost_center_name)}
                                                }
                                                td { class: "num", "{c.locations.len()}" }
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

            // 單身：儲位（ime_file）
            if let Some(w) = cur.clone() {
                Card {
                    div { class: "table-header",
                        h3 { {tf("warehouse-locations-title", &[("code", w.warehouse_code.as_str())])} }
                        if add_row {
                            Button {
                                icon: icons::ADD,
                                variant: ButtonVariant::Tonal,
                                onclick: {
                                    let w = w.clone();
                                    move |_| {
                                        dialog_error.set(None);
                                        // 新增儲位時預設帶入倉庫的設定（WH4(c)），後端未給的欄位也從倉庫帶入
                                        loc_form.set(WarehouseLocationInput {
                                            storage_class: Some(w.storage_class.clone()),
                                            usable: Some(w.usable),
                                            mrp_usable: Some(w.mrp_usable),
                                            bonded: Some(w.bonded),
                                            issue_priority: Some(w.issue_priority),
                                            ship_priority: Some(w.ship_priority),
                                            account_method: Some("0".into()),
                                            ..Default::default()
                                        });
                                        loc_editor.set(Some(LocEditor(None)));
                                    }
                                },
                                {t("warehouse-action-add-location")}
                            }
                        }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("warehouse-field-location")} }
                                    th { {t("warehouse-field-location-name")} }
                                    th { {t("warehouse-field-class")} }
                                    th { {t("warehouse-field-usable")} }
                                    th { {t("warehouse-field-mrp")} }
                                    th { {t("warehouse-field-bonded")} }
                                    th { {t("warehouse-field-issue-priority")} }
                                    th { {t("warehouse-field-ship-priority")} }
                                    th { {t("warehouse-field-account-method")} }
                                    th { {t("warehouse-field-account")} }
                                    th { {t("warehouse-field-account2")} }
                                    th {}
                                }
                            }
                            tbody {
                                if w.locations.is_empty() {
                                    tr { td { colspan: 12, class: "muted", {t("warehouse-msg-no-location")} } }
                                }
                                for l in w.locations.iter().cloned() {
                                    LocationRow {
                                        key: "{l.location_code}",
                                        location: l.clone(),
                                        can_edit: can_detail,
                                        can_delete: delete_row,
                                        onedit: {
                                            let l = l.clone();
                                            let version = w.version;
                                            move |_| {
                                                dialog_error.set(None);
                                                loc_form.set(l.to_input(version));
                                                loc_editor.set(Some(LocEditor(Some(l.location_code.clone()))));
                                            }
                                        },
                                        ondelete: {
                                            let w = w.clone();
                                            let loc = l.location_code.clone();
                                            move |_| {
                                                let p = plant();
                                                let w = w.clone();
                                                let loc = loc.clone();
                                                spawn(async move {
                                                    match api::remove_warehouse_location(&p, &w.warehouse_code, &loc, w.version).await {
                                                        Ok(r) => done(tf("warehouse-msg-location-deleted", &[("code", r.warehouse_code.as_str()), ("location", loc.as_str())]), Some(r.warehouse_code)),
                                                        Err(e) => error.set(Some(e)),
                                                    }
                                                });
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                    p { class: "muted", {t("warehouse-hint-location")} }
                }
            }

            if let Some((key, changes)) = history() {
                HistoryDialog { record: key, changes, onclose: move |_| history.set(None) }
            }
            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("warehouse-dialog-new"),
                        Editor::Edit(code) => tf("warehouse-dialog-edit", &[("code", code.as_str())]),
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
                        TextField { id: "imd01", label: t("warehouse-field-code"), maxlength: max_len::CODE, value: v.warehouse_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().warehouse_code = x }
                        TextField { id: "imd02", label: t("warehouse-field-name"), maxlength: max_len::NAME, value: txt(&v.name), oninput: move |x| form.write().name = opt(x) }
                        Select { id: "imd10", label: t("warehouse-field-class"), options: class_options(), value: v.storage_class.clone(), onchange: move |x| form.write().storage_class = x }
                        TextField { id: "imd03", label: t("warehouse-field-address"), maxlength: max_len::ADDRESS, value: txt(&v.address), oninput: move |x| form.write().address = opt(x) }
                        TextField { id: "imd06", label: t("warehouse-field-contact"), maxlength: max_len::CONTACT, value: txt(&v.contact), oninput: move |x| form.write().contact = opt(x) }
                        TextField { id: "imd07", label: t("warehouse-field-phone"), maxlength: max_len::CONTACT, value: txt(&v.phone), oninput: move |x| form.write().phone = opt(x) }
                        TextField { id: "imd08", label: t("warehouse-field-account"), maxlength: max_len::ACCOUNT, value: txt(&v.account), oninput: move |x| form.write().account = opt(x) }
                        TextField { id: "imd081", label: t("warehouse-field-account2"), maxlength: max_len::ACCOUNT, value: txt(&v.account2), oninput: move |x| form.write().account2 = opt(x) }
                        TextField { id: "imd09", label: t("warehouse-field-cost-group"), maxlength: 1usize, value: txt(&v.cost_group), oninput: move |x| form.write().cost_group = opt(x) }
                        TextField { id: "imd16", label: t("warehouse-field-cost-center"), maxlength: max_len::CODE, value: txt(&v.cost_center), oninput: move |x| form.write().cost_center = opt(x) }
                        TextField {
                            id: "imd14",
                            label: t("warehouse-field-issue-priority"),
                            input_type: "number",
                            value: v.issue_priority.unwrap_or(0).to_string(),
                            oninput: move |x: String| form.write().issue_priority = x.trim().parse().ok(),
                        }
                        TextField {
                            id: "imd15",
                            label: t("warehouse-field-ship-priority"),
                            input_type: "number",
                            value: v.ship_priority.unwrap_or(0).to_string(),
                            oninput: move |x: String| form.write().ship_priority = x.trim().parse().ok(),
                        }
                        Checkbox { id: "imd11", label: t("warehouse-field-usable"), checked: v.usable.unwrap_or(true), onchange: move |x| form.write().usable = Some(x) }
                        Checkbox { id: "imd12", label: t("warehouse-field-mrp"), checked: v.mrp_usable.unwrap_or(true), onchange: move |x| form.write().mrp_usable = Some(x) }
                        Checkbox { id: "imd13", label: t("warehouse-field-bonded"), checked: v.bonded.unwrap_or(false), onchange: move |x| form.write().bonded = Some(x) }
                    }
                    p { class: "muted", {t("warehouse-hint-flags")} }
                    p { class: "muted", {t("warehouse-hint-cost-center")} }
                }
            }
            if let Some(le) = loc_editor() {
                Dialog {
                    title: match &le.0 {
                        None => t("warehouse-dialog-new-location"),
                        Some(code) => tf("warehouse-dialog-edit-location", &[("location", code.as_str())]),
                    },
                    oncancel: move |_| { loc_editor.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { loc_editor.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save_location, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    div { class: "form-grid",
                        TextField { id: "ime02", label: t("warehouse-field-location"), maxlength: max_len::CODE, value: lv.location_code.clone(), readonly: le.0.is_some(), oninput: move |x| loc_form.write().location_code = x }
                        TextField { id: "ime03", label: t("warehouse-field-location-name"), maxlength: max_len::NAME, value: txt(&lv.name), oninput: move |x| loc_form.write().name = opt(x) }
                        Select { id: "ime04", label: t("warehouse-field-class"), options: class_options(), value: txt(&lv.storage_class), onchange: move |x| loc_form.write().storage_class = Some(x) }
                        TextField {
                            id: "ime10",
                            label: t("warehouse-field-issue-priority"),
                            input_type: "number",
                            value: lv.issue_priority.unwrap_or(0).to_string(),
                            oninput: move |x: String| loc_form.write().issue_priority = x.trim().parse().ok(),
                        }
                        TextField {
                            id: "ime11",
                            label: t("warehouse-field-ship-priority"),
                            input_type: "number",
                            value: lv.ship_priority.unwrap_or(0).to_string(),
                            oninput: move |x: String| loc_form.write().ship_priority = x.trim().parse().ok(),
                        }
                        Select { id: "ime08", label: t("warehouse-field-account-method"), options: method_options(), value: lv.account_method.clone().unwrap_or_else(|| "0".into()), onchange: move |x| loc_form.write().account_method = Some(x) }
                        TextField { id: "ime09", label: t("warehouse-field-account"), maxlength: max_len::ACCOUNT, value: txt(&lv.account), disabled: lv.account_method.as_deref() != Some("2"), oninput: move |x| loc_form.write().account = opt(x) }
                        TextField { id: "ime091", label: t("warehouse-field-account2"), maxlength: max_len::ACCOUNT, value: txt(&lv.account2), disabled: lv.account_method.as_deref() != Some("2"), oninput: move |x| loc_form.write().account2 = opt(x) }
                        Checkbox { id: "ime05", label: t("warehouse-field-usable"), checked: lv.usable.unwrap_or(true), onchange: move |x| loc_form.write().usable = Some(x) }
                        Checkbox { id: "ime06", label: t("warehouse-field-mrp"), checked: lv.mrp_usable.unwrap_or(true), onchange: move |x| loc_form.write().mrp_usable = Some(x) }
                        Checkbox { id: "ime07", label: t("warehouse-field-bonded"), checked: lv.bonded.unwrap_or(false), onchange: move |x| loc_form.write().bonded = Some(x) }
                    }
                    p { class: "muted", {t("warehouse-hint-account-method")} }
                }
            }
        }
    }
}

#[component]
fn LocationRow(
    location: WarehouseLocationDto,
    can_edit: bool,
    can_delete: bool,
    onedit: EventHandler<()>,
    ondelete: EventHandler<()>,
) -> Element {
    let l = location;
    rsx! {
        tr {
            td { class: "mono", "{l.location_code}" }
            td { {txt(&l.name)} }
            td { class: "mono", "{l.storage_class}" }
            td { class: "mono", {yn(l.usable)} }
            td { class: "mono", {yn(l.mrp_usable)} }
            td { class: "mono", {yn(l.bonded)} }
            td { class: "num", "{l.issue_priority}" }
            td { class: "num", "{l.ship_priority}" }
            td { class: "mono", "{l.account_method}" }
            td { class: "mono", {txt(&l.account)} }
            td { class: "mono", {txt(&l.account2)} }
            td {
                if can_edit {
                    Button { icon: icons::EDIT, variant: ButtonVariant::Text, onclick: move |_| onedit.call(()), {t("common-action-edit")} }
                }
                if can_delete {
                    Button { icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Text, onclick: move |_| ondelete.call(()), {t("common-action-delete")} }
                }
            }
        }
    }
}
