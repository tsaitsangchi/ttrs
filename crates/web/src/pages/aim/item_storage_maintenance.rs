//! Item Storage Maintenance 料件預設倉儲維護（TipTop aimi202）
//! 版面比照 tiptop/aim/4fd/aimi202.4fd：單頭料件（料號、品名、規格、版本、來源碼、庫存單位，唯讀），單身倉庫、儲位、最高存量。
//! 上方是料件清單（沒有條件時只列已有預設倉儲的料件；以料號查詢可找到尚未設定的料件，IS1）。
//! 標準功能例外（IS6）：單頭新增、刪除由料件維護；失效、確認不適用。規則在後端 ItemStorageService。

use dioxus::prelude::*;
use ttrs_shared::{
    ItemStorageBatchRequest, ItemStorageDto, ItemStorageInput, ItemStorageItemDto, QbeCondition,
    QueryRequest, item_max_len, warehouse_max_len,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, TextField, Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::components::qbe::{
    HINT_NUMBER, ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav,
};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

fn txt(v: &Option<String>) -> String {
    v.clone().unwrap_or_default()
}

fn qty_text(v: Option<rust_decimal::Decimal>) -> String {
    v.map(|q| q.normalize().to_string()).unwrap_or_default()
}

/// 單身小視窗：None 為新增，Some 為修改該筆（倉庫, 儲位）
#[derive(Clone, PartialEq)]
struct Editor(Option<(String, String)>);

/// 查詢列的欄位（aimi202.4fd）：（標題翻譯 ID, 查詢欄位鍵值, 查詢提示翻譯 ID）
const Q_COLUMNS: [(&str, &str, &str); 5] = [
    ("istorage-field-item", "item_no", ""),
    ("istorage-field-name", "name", ""),
    ("istorage-field-warehouse", "warehouse_code", ""),
    ("istorage-field-location", "location_code", ""),
    ("istorage-field-max-qty", "max_qty", HINT_NUMBER),
];

/// 權限檢查用的程式代號
const PROGRAM: &str = "item-storage-maintenance";

#[component]
pub fn ItemStorageMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ItemStoragePage {} }
    }
}

#[component]
fn ItemStoragePage() -> Element {
    let perm = use_context::<Perm>();
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut items = use_resource(move || {
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
            api::query_item_storages(&p, &req).await
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut copy_to = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        items
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|c| c.item_no.clone()).collect())
            .unwrap_or_default()
    });
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(ItemStorageInput::default);
    let mut max_text = use_signal(String::new);
    let mut batch = use_signal(|| None::<ItemStorageBatchRequest>);
    let mut batch_item = use_signal(String::new);
    let mut batch_group = use_signal(String::new);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);

    let page_data = items.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<ItemStorageItemDto> = page_data
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
    let cur: Option<ItemStorageItemDto> = selected()
        .and_then(|c| list.iter().find(|x| x.item_no == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let keys: Vec<String> = list.iter().map(|c| c.item_no.clone()).collect();

    let mut done = move |msg: String, item: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        batch.set(None);
        dialog_error.set(None);
        selected.set(item);
        items.restart();
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
            match api::query_item_storages(&p, &req).await {
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
                "item-storages",
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

    let cur_save = cur.clone();
    let save = move |_| {
        let Some(c) = cur_save.clone() else { return };
        let p = plant();
        let place = editor().and_then(|e| e.0);
        let mut req = form();
        let raw = max_text();
        req.max_qty = if raw.trim().is_empty() {
            None
        } else {
            match raw.trim().parse() {
                Ok(q) => Some(q),
                Err(_) => {
                    dialog_error.set(Some(t("error-ITEM_STORAGE_MAX_QTY_INVALID")));
                    return;
                }
            }
        };
        spawn(async move {
            let r = api::save_item_storage(
                &p,
                &c.item_no,
                place.as_ref().map(|(w, l)| (w.as_str(), l.as_str())),
                &req,
            )
            .await;
            match r {
                Ok(d) => done(
                    tf("istorage-msg-saved", &[("item", d.item_no.as_str())]),
                    Some(d.item_no),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let run_batch = move |_| {
        let Some(mut req) = batch() else { return };
        let mut conds = Vec::new();
        for (field, v) in [("item_no", batch_item()), ("group_code", batch_group())] {
            if !v.trim().is_empty() {
                conds.push(QbeCondition {
                    field: field.into(),
                    expr: v.trim().to_owned(),
                });
            }
        }
        req.conditions = conds;
        let p = plant();
        spawn(async move {
            match api::batch_item_storages(&p, &req).await {
                Ok(r) => done(
                    tf(
                        "istorage-msg-batch-done",
                        &[
                            ("inserted", r.inserted.to_string().as_str()),
                            ("skipped", r.skipped.to_string().as_str()),
                        ],
                    ),
                    None,
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_history = cur.clone();
    let cur_copy = cur.clone();
    let can_edit = perm.can("detail") && cur.as_ref().is_some_and(|c| c.active);
    let can_delete = perm.delete_row();
    let v = form();
    let b = batch();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("istorage-breadcrumb")} }
                h2 { {t("istorage-title")} }
            }
            PlantPicker { plant }

            if querying() {
                div { class: "action-bar",
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { error.set(None); querying.set(false); }, {t("common-action-cancel")} }
                }
                p { class: "query-hint", {t("istorage-hint-query")} }
            } else {
                div { class: "action-bar",
                    MaintenanceToolbar {
                        query: Act::new(true, move |_| { message.set(None); error.set(None); qbe.set(Default::default()); querying.set(true); }),
                        // aimi202 是單身式作業：新增預設倉儲用 detail
                        detail_action: "detail",
                        insert: if perm.add_row() { Act::new(can_edit, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(ItemStorageInput::default());
                            max_text.set(String::new());
                            editor.set(Some(Editor(None)));
                        }) } else { None },
                        // 複製（reproduce）：目的料號輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_to().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let p = plant();
                            let target = copy_to();
                            spawn(async move {
                                match api::copy_item_storage(&p, &c.item_no, target.trim()).await {
                                    Ok(r) => {
                                        copy_to.set(String::new());
                                        done(tf("istorage-msg-copied", &[("from", c.item_no.as_str()), ("to", r.item_no.as_str())]), Some(r.item_no));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        history: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_history.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::item_storage_history(&p, &c.item_no).await {
                                    Ok(h) => history.set(Some((c.item_no.clone(), h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        if perm.can("reproduce") {
                            TextField { id: "copy-to", label: t("istorage-field-copy-to"), maxlength: item_max_len::ITEM_NO, value: copy_to(), disabled: no_cur, oninput: move |v| copy_to.set(v) }
                        }
                        if perm.can("batch_generate") {
                            Button {
                                icon: icons::ADD,
                                variant: ButtonVariant::Tonal,
                                onclick: move |_| {
                                    message.set(None);
                                    dialog_error.set(None);
                                    batch_item.set(String::new());
                                    batch_group.set(String::new());
                                    batch.set(Some(ItemStorageBatchRequest::default()));
                                },
                                {t("istorage-action-batch")}
                            }
                        }
                    }
                }
            }

            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.item_no.as_str())),
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
                div { class: "table-header",
                    h3 { {t("istorage-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("istorage-field-item")} }
                                th { {t("istorage-field-name")} }
                                th { {t("istorage-field-warehouse")} }
                                th { {t("istorage-field-location")} }
                                th { {t("istorage-field-max-qty")} }
                            }
                            if querying() {
                                tr {
                                    for (id, field, hint) in Q_COLUMNS {
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
                                }
                            }
                        }
                        tbody {
                            match &*items.read() {
                                None => rsx! { tr { td { colspan: 5, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 5, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: 5, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for c in list.iter().cloned() {
                                        tr {
                                            key: "{c.item_no}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.item_no == c.item_no) { "md-table__row--selected" },
                                            onclick: {
                                                let item = c.item_no.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(item.clone())); }
                                            },
                                            td { class: "mono", "{c.item_no}" }
                                            td { {txt(&c.name)} }
                                            td { class: "mono", {c.storages.iter().map(|s| s.warehouse_code.clone()).collect::<Vec<_>>().join(", ")} }
                                            td { class: "mono", {c.storages.iter().filter(|s| !s.location_code.is_empty()).map(|s| s.location_code.clone()).collect::<Vec<_>>().join(", ")} }
                                            td { class: "num", "{c.storages.len()}" }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
                ListPager { page, pages }
            }

            // 單頭（料件，唯讀）與單身（預設倉儲）
            if let Some(c) = cur.clone() {
                Card {
                    div { class: "table-header",
                        h3 { {tf("istorage-detail-title", &[("item", c.item_no.as_str())])} }
                    }
                    div { class: "form-grid",
                        TextField { id: "imf01", label: t("istorage-field-item"), value: c.item_no.clone(), readonly: true }
                        TextField { id: "ima02", label: t("istorage-field-name"), value: txt(&c.name), readonly: true }
                        TextField { id: "ima021", label: t("istorage-field-spec"), value: txt(&c.spec), readonly: true }
                        TextField { id: "ima05", label: t("istorage-field-version"), value: txt(&c.current_version), readonly: true }
                        TextField { id: "ima08", label: t("istorage-field-source"), value: txt(&c.source_code), readonly: true }
                        TextField { id: "ima25", label: t("istorage-field-stock-unit"), value: txt(&c.stock_unit), readonly: true }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("istorage-field-warehouse")} }
                                    th { {t("istorage-field-warehouse-name")} }
                                    th { {t("istorage-field-location")} }
                                    th { {t("istorage-field-location-name")} }
                                    th { {t("istorage-field-max-qty")} }
                                    th {}
                                }
                            }
                            tbody {
                                if c.storages.is_empty() {
                                    tr { td { colspan: 6, class: "muted", {t("istorage-msg-no-storage")} } }
                                }
                                for s in c.storages.iter().cloned() {
                                    StorageRow {
                                        key: "{s.warehouse_code}/{s.location_code}",
                                        storage: s.clone(),
                                        can_edit,
                                        can_delete,
                                        onedit: {
                                            let s = s.clone();
                                            move |_| {
                                                dialog_error.set(None);
                                                form.set(ItemStorageInput {
                                                    warehouse_code: s.warehouse_code.clone(),
                                                    location_code: s.location_code.clone(),
                                                    max_qty: s.max_qty,
                                                    version: Some(s.version),
                                                });
                                                max_text.set(qty_text(s.max_qty));
                                                editor.set(Some(Editor(Some((s.warehouse_code.clone(), s.location_code.clone())))));
                                            }
                                        },
                                        ondelete: {
                                            let item = c.item_no.clone();
                                            let s = s.clone();
                                            move |_| {
                                                let p = plant();
                                                let item = item.clone();
                                                let s = s.clone();
                                                spawn(async move {
                                                    match api::delete_item_storage(&p, &item, (&s.warehouse_code, &s.location_code), s.version).await {
                                                        Ok(r) => done(tf("istorage-msg-deleted", &[("item", r.item_no.as_str())]), Some(r.item_no)),
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
                    p { class: "muted", {t("istorage-hint-detail")} }
                }
            }

            if let Some((key, changes)) = history() {
                HistoryDialog { record: key, changes, onclose: move |_| history.set(None) }
            }
            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed.0 {
                        None => t("istorage-dialog-new"),
                        Some((w, l)) => tf("istorage-dialog-edit", &[("warehouse", w.as_str()), ("location", l.as_str())]),
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
                        TextField { id: "imf02", label: t("istorage-field-warehouse"), maxlength: warehouse_max_len::CODE, value: v.warehouse_code.clone(), oninput: move |x| form.write().warehouse_code = x }
                        TextField { id: "imf03", label: t("istorage-field-location"), maxlength: warehouse_max_len::CODE, value: v.location_code.clone(), oninput: move |x| form.write().location_code = x }
                        TextField { id: "imf04", label: t("istorage-field-max-qty"), input_type: "number", value: max_text(), oninput: move |x| max_text.set(x) }
                    }
                    p { class: "muted", {t("istorage-hint-location")} }
                    p { class: "muted", {t("istorage-hint-max-qty")} }
                }
            }
            if let Some(req) = b {
                Dialog {
                    title: t("istorage-dialog-batch"),
                    oncancel: move |_| { batch.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { batch.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_batch, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    div { class: "form-grid",
                        TextField { id: "ima01", label: t("istorage-field-item"), placeholder: t("istorage-hint-pattern"), value: batch_item(), oninput: move |x| batch_item.set(x) }
                        TextField { id: "ima06", label: t("istorage-field-group"), value: batch_group(), oninput: move |x| batch_group.set(x) }
                        TextField { id: "g_stk", label: t("istorage-field-warehouse"), maxlength: warehouse_max_len::CODE, value: req.warehouse_code.clone(), oninput: move |x| if let Some(b) = batch.write().as_mut() { b.warehouse_code = x; } }
                        TextField { id: "g_loc", label: t("istorage-field-location"), maxlength: warehouse_max_len::CODE, value: req.location_code.clone(), oninput: move |x| if let Some(b) = batch.write().as_mut() { b.location_code = x; } }
                    }
                    p { class: "muted", {t("istorage-hint-batch")} }
                }
            }
        }
    }
}

#[component]
fn StorageRow(
    storage: ItemStorageDto,
    can_edit: bool,
    can_delete: bool,
    onedit: EventHandler<()>,
    ondelete: EventHandler<()>,
) -> Element {
    let s = storage;
    rsx! {
        tr {
            td { class: "mono", "{s.warehouse_code}" }
            td { {txt(&s.warehouse_name)} }
            td { class: "mono", "{s.location_code}" }
            td { {txt(&s.location_name)} }
            td { class: "num", {qty_text(s.max_qty)} }
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
