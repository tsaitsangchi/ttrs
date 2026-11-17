//! Encoding Class Maintenance 編碼類別維護（TipTop aooi402 編碼類別資料維護作業）
//! 版面比照 tiptop/aoo/4fd/aooi402.4fd（EC1）：上方類別清單（編碼類別、名稱、性質、總段數、總長度），選定一筆後下方顯示各段
//! （段次、說明、長度、型態、固定值／獨立段、名稱顯示）。共用層：全公司共用（EC2）。規則在後端 EncodingClassService。
//! 標準功能鈕（ADR 0013；EC11）：沿用 TipTop 的 insert、modify、delete、invalid、reproduce、detail，
//! 加上查詢、確認、列印、匯出 Excel、上下筆、異動記錄；單身另有「插入一段」「段次重排」（EC4）。

use dioxus::prelude::*;
use ttrs_shared::{
    EncodingClassDto, EncodingClassInput, EncodingClassSegmentInput, QbeCondition, QueryRequest,
    encoding_class_max_len as max_len,
};

use crate::api::{self, SegmentEdit};
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Dialog, Notification, NotificationKind, Select, Tabs,
    TextField, Tone, icons,
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

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "encoding-class-maintenance";

/// 型態 gel04 的選項（aooi402.per）：1–9；型態 4（欄位值）第一版不開放（EC6）
fn types() -> Vec<(String, String)> {
    [1, 2, 3, 5, 6, 7, 8, 9]
        .into_iter()
        .map(|i| (i.to_string(), t(&format!("encoding-class-type-{i}"))))
        .collect()
}

/// 固定值、獨立段才需要輸入 gel05；季、月、週、日長度固定 2、獨立段帶獨立段的長度（EC5）
fn needs_value(kind: &str) -> bool {
    matches!(kind, "1" | "2")
}

fn needs_length(kind: &str) -> bool {
    !matches!(kind, "2" | "6" | "7" | "8" | "9")
}

#[component]
pub fn EncodingClassMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ClassPage {} }
    }
}

#[component]
fn ClassPage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    // 查詢：qbe 為查詢列正在輸入的條件，conditions 為目前生效的條件（QBE4：預設查全部）
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut classes = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move { api::query_encoding_classes(&req).await }
    });
    let selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        classes
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|u| u.class_code.clone()).collect())
            .unwrap_or_default()
    });
    let mut selected = selected;
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(EncodingClassInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    // 單身編輯：None 為未開啟；Some(新增／插入／修改)
    let mut value_editor = use_signal(|| None::<SegmentEdit>);
    let mut value_form = use_signal(EncodingClassSegmentInput::default);
    let mut value_error = use_signal(|| None::<String>);

    let page_data = classes
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let list: Vec<EncodingClassDto> = page_data
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
    let cur: Option<EncodingClassDto> = selected()
        .and_then(|c| list.iter().find(|x| x.class_code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let first_code = cur.as_ref().map(|c| c.class_code.clone());
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::ALL.len() + 2
    } else {
        5
    };
    let keys: Vec<String> = list.iter().map(|u| u.class_code.clone()).collect();

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
        spawn(async move {
            match api::query_encoding_classes(&req).await {
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
                Some(Editor::Edit(code)) => api::update_encoding_class(&code, &req).await,
                _ => api::create_encoding_class(&req).await,
            };
            match result {
                Ok(u) => done(
                    tf(
                        "encoding-class-msg-saved",
                        &[("code", u.class_code.as_str())],
                    ),
                    Some(u.class_code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    // 單身存檔：新增一列或修改一列；成功後關閉小視窗並重新讀取
    let save_value = move |_| {
        let Some(edit) = value_editor() else { return };
        let Some(code) = selected().or_else(|| first_code.clone()) else {
            return;
        };
        let req = value_form();
        spawn(async move {
            match api::save_encoding_class_segment(&code, edit, &req).await {
                Ok(r) => {
                    value_editor.set(None);
                    value_error.set(None);
                    done(
                        tf(
                            "encoding-class-msg-segment-saved",
                            &[("code", r.class_code.as_str())],
                        ),
                        Some(r.class_code),
                    );
                }
                Err(e) => value_error.set(Some(e)),
            }
        });
    };

    // 列印、匯出 Excel（ADR 0013 SA3）：依目前生效的查詢條件
    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            if let Err(e) = api::system_list_report(
                "/api/encoding-classes",
                "encoding-classes",
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
                div { class: "program-id", {t("encoding-class-breadcrumb")} }
                h2 { {t("encoding-class-title")} }
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
                        // aooi402 是單頭單身作業：新增、修改、刪除、失效各用 insert、modify、delete、invalid
                        insert: Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(EncodingClassInput::default());
                            editor.set(Some(Editor::New));
                        }),
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.class_code)));
                        }),
                        // 複製（ADR 0013 SA5；連同單身）：新類別輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let new_code = copy_code();
                            spawn(async move {
                                match api::copy_encoding_class(&c.class_code, new_code.trim()).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("encoding-class-msg-copied", &[("from", c.class_code.as_str()), ("to", r.class_code.as_str())]), Some(r.class_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）
                        delete: Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match api::delete_encoding_class(&c.class_code, c.version).await {
                                    Ok(()) => done(tf("encoding-class-msg-deleted", &[("code", c.class_code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 確認／取消確認（ADR 0013 SA9）
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            spawn(async move {
                                match api::set_encoding_class_state(&c.class_code, if c.confirmed { "unconfirm" } else { "confirm" }, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "encoding-class-msg-confirmed" } else { "encoding-class-msg-unconfirmed" };
                                        done(tf(id, &[("code", r.class_code.as_str())]), Some(r.class_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            spawn(async move {
                                match api::set_encoding_class_state(&c.class_code, if c.active { "deactivate" } else { "activate" }, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "encoding-class-msg-activated" } else { "encoding-class-msg-deactivated" };
                                        done(tf(id, &[("code", r.class_code.as_str())]), Some(r.class_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        inactive,
                        invalid_label: Some(("encoding-class-action-deactivate", "encoding-class-action-activate")),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        // 異動記錄（ADR 0012 §8，功能 history）
                        if perm.can("history") {
                            HistoryButton { program: PROGRAM, record: cur.as_ref().map(|c| c.class_code.clone()) }
                        }
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("encoding-class-field-copy-as"), maxlength: max_len::CODE, value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
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
                    h3 { {t("encoding-class-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("encoding-class-field-code")} }
                                th { {t("encoding-class-field-name")} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else {
                                    th { {t("encoding-class-field-nature")} }
                                    th { {t("encoding-class-field-count")} }
                                    th { {t("encoding-class-field-total")} }
                                }
                            }
                            // 查詢列（在表格第一列輸入條件）
                            if querying() {
                                tr {
                                    for (id, field) in [("encoding-class-field-code", "class_code"), ("encoding-class-field-name", "name"), ("encoding-class-field-nature", "nature_code"), ("encoding-class-field-type", "segment_type"), ("encoding-class-field-value", "value")].into_iter().take(if status_tab { 2 } else { 5 }) {
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
                            match &*classes.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for u in list.iter().cloned() {
                                        tr {
                                            key: "{u.class_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.class_code == u.class_code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = u.class_code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{u.class_code}" }
                                            td { "{u.name}" }
                                            if status_tab {
                                                StatusCells { status: u.status() }
                                                ConfirmCells { confirmed: u.confirmed, by: u.confirmed_by.clone() }
                                            } else {
                                                td { class: "mono", "{u.nature_code}" }
                                                td { class: "num", "{u.segment_count}" }
                                                td { class: "num", "{u.total_length}" }
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

            // 單身（aooi402.4fd 下半部）：選定的類別的各段
            if let Some(c) = cur.clone() {
                Card {
                    div { class: "table-header",
                        h3 { {tf("encoding-class-segments-title", &[("code", c.class_code.as_str())])} }
                        div { class: "action-bar",
                            if perm.can("detail") && perm.add_row() {
                                Button { icon: icons::ADD, tone: Tone::Primary, variant: ButtonVariant::Tonal, disabled: !c.active, onclick: {
                                    let c = c.clone();
                                    move |_| {
                                        value_error.set(None);
                                        value_form.set(EncodingClassSegmentInput { segment_type: "1".to_owned(), version: Some(c.version), ..Default::default() });
                                        value_editor.set(Some(SegmentEdit::Add));
                                    }
                                }, {t("encoding-class-action-add-segment")} }
                            }
                            // 段次重排（lineno_from_1_resort）
                            if perm.can("detail") {
                                Button { icon: icons::SYNC, tone: Tone::Neutral, variant: ButtonVariant::Text, disabled: !c.active || c.segments.is_empty(), onclick: {
                                    let (code, version) = (c.class_code.clone(), c.version);
                                    move |_| {
                                        let code = code.clone();
                                        spawn(async move {
                                            match api::renumber_encoding_class(&code, version).await {
                                                Ok(r) => done(tf("encoding-class-msg-renumbered", &[("code", r.class_code.as_str())]), Some(r.class_code)),
                                                Err(e) => error.set(Some(e)),
                                            }
                                        });
                                    }
                                }, {t("encoding-class-action-renumber")} }
                            }
                        }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("encoding-class-field-seq")} }
                                    th { {t("encoding-class-field-description")} }
                                    th { {t("encoding-class-field-length")} }
                                    th { {t("encoding-class-field-type")} }
                                    th { {t("encoding-class-field-value")} }
                                    th { {t("encoding-class-field-show-name")} }
                                    th {}
                                }
                            }
                            tbody {
                                if c.segments.is_empty() {
                                    tr { td { colspan: 7, class: "muted", {t("common-msg-no-data")} } }
                                }
                                for row in c.segments.iter().cloned() {
                                    tr { key: "{row.seq_no}",
                                        td { class: "num", "{row.seq_no}" }
                                        td { {row.description.clone().unwrap_or_default()} }
                                        td { class: "num", "{row.length}" }
                                        td { {t(&format!("encoding-class-type-{}", row.segment_type))} }
                                        td { class: "mono", {row.value.clone().unwrap_or_default()} }
                                        td { {t(if row.show_name { "encoding-class-yes" } else { "encoding-class-no" })} }
                                        td {
                                            if perm.can("detail") {
                                                Button { icon: icons::EDIT, tone: Tone::Neutral, variant: ButtonVariant::Text, disabled: !c.active, onclick: {
                                                    let (row, version) = (row.clone(), c.version);
                                                    move |_| {
                                                        value_error.set(None);
                                                        value_form.set(EncodingClassSegmentInput { seq_no: Some(row.seq_no), description: row.description.clone(), length: Some(row.length), segment_type: row.segment_type.clone(), value: row.value.clone(), show_name: row.show_name, version: Some(version) });
                                                        value_editor.set(Some(SegmentEdit::Update(row.seq_no)));
                                                    }
                                                }, {t("common-action-edit")} }
                                            }
                                            // 插入一段（addno）：插在這一段之前，之後的段次加 1
                                            if perm.can("detail") && perm.add_row() {
                                                Button { icon: icons::PLAYLIST_ADD, tone: Tone::Neutral, variant: ButtonVariant::Text, disabled: !c.active, onclick: {
                                                    let (no, version) = (row.seq_no, c.version);
                                                    move |_| {
                                                        value_error.set(None);
                                                        value_form.set(EncodingClassSegmentInput { segment_type: "1".to_owned(), version: Some(version), ..Default::default() });
                                                        value_editor.set(Some(SegmentEdit::Insert(no)));
                                                    }
                                                }, {t("encoding-class-action-insert-segment")} }
                                            }
                                            if perm.can("detail") && perm.delete_row() {
                                                Button { icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Text, disabled: !c.active, onclick: {
                                                    let (code, no, version) = (c.class_code.clone(), row.seq_no, c.version);
                                                    move |_| {
                                                        let code = code.clone();
                                                        spawn(async move {
                                                            match api::remove_encoding_class_segment(&code, no, version).await {
                                                                Ok(r) => done(tf("encoding-class-msg-segment-deleted", &[("no", no.to_string().as_str())]), Some(r.class_code)),
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
                    if !c.active {
                        p { class: "query-hint", {t("encoding-class-hint-inactive")} }
                    }
                }
            }

            if let Some(edit) = value_editor() {
                Dialog {
                    title: match edit {
                        SegmentEdit::Add => t("encoding-class-dialog-new-segment"),
                        SegmentEdit::Insert(n) => tf("encoding-class-dialog-insert-segment", &[("no", n.to_string().as_str())]),
                        SegmentEdit::Update(n) => tf("encoding-class-dialog-edit-segment", &[("no", n.to_string().as_str())]),
                    },
                    oncancel: move |_| { value_editor.set(None); value_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { value_editor.set(None); value_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save_value, {t("common-action-ok")} }
                    },
                    if let Some(msg) = value_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| value_error.set(None) }
                    }
                    div { class: "form-grid",
                        TextField {
                            id: "gel02",
                            label: t("encoding-class-field-seq"),
                            input_type: "number",
                            readonly: edit != SegmentEdit::Add,
                            value: match edit { SegmentEdit::Insert(n) => n.to_string(), _ => value_form().seq_no.map(|n| n.to_string()).unwrap_or_default() },
                            oninput: move |x: String| value_form.write().seq_no = x.trim().parse().ok(),
                        }
                        TextField { id: "gel06", label: t("encoding-class-field-description"), maxlength: max_len::NAME, value: value_form().description.unwrap_or_default(), oninput: move |x| value_form.write().description = opt(x) }
                        Select {
                            id: "gel04",
                            label: t("encoding-class-field-type"),
                            options: types(),
                            value: value_form().segment_type,
                            onchange: move |x: String| {
                                let mut f = value_form.write();
                                if !needs_value(&x) { f.value = None; }
                                f.segment_type = x;
                            },
                        }
                        if needs_length(&value_form().segment_type) {
                            TextField {
                                id: "gel03",
                                label: t("encoding-class-field-length"),
                                input_type: "number",
                                value: value_form().length.map(|n| n.to_string()).unwrap_or_default(),
                                oninput: move |x: String| value_form.write().length = x.trim().parse().ok(),
                            }
                        }
                        if needs_value(&value_form().segment_type) {
                            TextField { id: "gel05", label: t("encoding-class-field-value"), maxlength: max_len::VALUE, value: value_form().value.unwrap_or_default(), oninput: move |x| value_form.write().value = opt(x) }
                        }
                        Checkbox { id: "gel07", label: t("encoding-class-field-show-name"), checked: value_form().show_name, onchange: move |v| value_form.write().show_name = v }
                    }
                    p { class: "muted", {t("encoding-class-hint-segment")} }
                }
            }

            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("encoding-class-dialog-new"),
                        Editor::Edit(code) => tf("encoding-class-dialog-edit", &[("code", code.as_str())]),
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
                        TextField { id: "gei01", label: t("encoding-class-field-code"), maxlength: max_len::CODE, value: v.class_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().class_code = x }
                        TextField { id: "gei02", label: t("encoding-class-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                        TextField { id: "gei03", label: t("encoding-class-field-nature"), maxlength: 2, value: v.nature_code.clone(), oninput: move |x| form.write().nature_code = x }
                    }
                    p { class: "muted", {t("encoding-class-hint")} }
                }
            }
        }
    }
}
