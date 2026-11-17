//! Encoding Segment Maintenance 編碼獨立段維護（TipTop aooi403 編碼獨立段資料維護作業）
//! 版面比照 tiptop/aoo/4fd/aooi403.4fd（IS1）：上方獨立段清單（獨立段編號、名稱、長度），選定一筆後下方顯示單身
//! （項次、編碼編號、編碼名稱）。共用層：全公司共用（IS2）。規則在後端 EncodingSegmentService。
//! 標準功能鈕（ADR 0013；IS8）：沿用 TipTop 的 insert、modify、delete、invalid、reproduce、detail，
//! 加上查詢（表格第一列輸入條件）、確認、列印、匯出 Excel、上下筆、異動記錄。

use dioxus::prelude::*;
use ttrs_shared::{
    EncodingSegmentDto, EncodingSegmentInput, EncodingSegmentValueInput, QbeCondition,
    QueryRequest, encoding_segment_max_len as max_len,
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

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "encoding-segment-maintenance";

#[component]
pub fn EncodingSegmentMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, SegmentPage {} }
    }
}

#[component]
fn SegmentPage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    // 查詢：qbe 為查詢列正在輸入的條件，conditions 為目前生效的條件（QBE4：預設查全部）
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut segments = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move { api::query_encoding_segments(&req).await }
    });
    let selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        segments
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|u| u.segment_code.clone()).collect())
            .unwrap_or_default()
    });
    let mut selected = selected;
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(EncodingSegmentInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    // 單身編輯：None 為未開啟；Some(None) 新增一列；Some(Some(項次)) 修改該列
    let mut value_editor = use_signal(|| None::<Option<i32>>);
    let mut value_form = use_signal(EncodingSegmentValueInput::default);
    let mut value_error = use_signal(|| None::<String>);

    let page_data = segments
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let list: Vec<EncodingSegmentDto> = page_data
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
    let cur: Option<EncodingSegmentDto> = selected()
        .and_then(|c| list.iter().find(|x| x.segment_code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let first_code = cur.as_ref().map(|c| c.segment_code.clone());
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::ALL.len() + 2
    } else {
        3
    };
    let keys: Vec<String> = list.iter().map(|u| u.segment_code.clone()).collect();

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        segments.restart();
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
            match api::query_encoding_segments(&req).await {
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
                Some(Editor::Edit(code)) => api::update_encoding_segment(&code, &req).await,
                _ => api::create_encoding_segment(&req).await,
            };
            match result {
                Ok(u) => done(
                    tf(
                        "encoding-segment-msg-saved",
                        &[("code", u.segment_code.as_str())],
                    ),
                    Some(u.segment_code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    // 單身存檔：新增一列或修改一列；成功後關閉小視窗並重新讀取
    let save_value = move |_| {
        let Some(line) = value_editor() else { return };
        let Some(code) = selected().or_else(|| first_code.clone()) else {
            return;
        };
        let req = value_form();
        spawn(async move {
            match api::save_encoding_segment_value(&code, line, &req).await {
                Ok(r) => {
                    value_editor.set(None);
                    value_error.set(None);
                    done(
                        tf(
                            "encoding-segment-msg-value-saved",
                            &[("code", r.segment_code.as_str())],
                        ),
                        Some(r.segment_code),
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
                "/api/encoding-segments",
                "encoding-segments",
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
                div { class: "program-id", {t("encoding-segment-breadcrumb")} }
                h2 { {t("encoding-segment-title")} }
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
                        // aooi403 是單頭單身作業：新增、修改、刪除、失效各用 insert、modify、delete、invalid
                        insert: Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(EncodingSegmentInput::default());
                            editor.set(Some(Editor::New));
                        }),
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.segment_code)));
                        }),
                        // 複製（ADR 0013 SA5；連同單身）：新獨立段編號輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let new_code = copy_code();
                            spawn(async move {
                                match api::copy_encoding_segment(&c.segment_code, new_code.trim()).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("encoding-segment-msg-copied", &[("from", c.segment_code.as_str()), ("to", r.segment_code.as_str())]), Some(r.segment_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）
                        delete: Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match api::delete_encoding_segment(&c.segment_code, c.version).await {
                                    Ok(()) => done(tf("encoding-segment-msg-deleted", &[("code", c.segment_code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 確認／取消確認（ADR 0013 SA9）
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            spawn(async move {
                                match api::set_encoding_segment_state(&c.segment_code, if c.confirmed { "unconfirm" } else { "confirm" }, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "encoding-segment-msg-confirmed" } else { "encoding-segment-msg-unconfirmed" };
                                        done(tf(id, &[("code", r.segment_code.as_str())]), Some(r.segment_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            spawn(async move {
                                match api::set_encoding_segment_state(&c.segment_code, if c.active { "deactivate" } else { "activate" }, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "encoding-segment-msg-activated" } else { "encoding-segment-msg-deactivated" };
                                        done(tf(id, &[("code", r.segment_code.as_str())]), Some(r.segment_code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        inactive,
                        invalid_label: Some(("encoding-segment-action-deactivate", "encoding-segment-action-activate")),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        // 異動記錄（ADR 0012 §8，功能 history）
                        if perm.can("history") {
                            HistoryButton { program: PROGRAM, record: cur.as_ref().map(|c| c.segment_code.clone()) }
                        }
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("encoding-segment-field-copy-as"), maxlength: max_len::CODE, value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
                        }
                    }
                }
            }

            // 上下筆：移動選取列，跨頁自動換頁（ADR 0013 SA2）
            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.segment_code.as_str())),
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
                    h3 { {t("encoding-segment-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("encoding-segment-field-code")} }
                                th { {t("encoding-segment-field-name")} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else {
                                    th { {t("encoding-segment-field-length")} }
                                }
                            }
                            // 查詢列（在表格第一列輸入條件）
                            if querying() {
                                tr {
                                    for (id, field, hint) in [("encoding-segment-field-code", "segment_code", ""), ("encoding-segment-field-name", "name", ""), ("encoding-segment-field-length", "length", HINT_NUMBER)].into_iter().take(if status_tab { 2 } else { 3 }) {
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
                            match &*segments.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for u in list.iter().cloned() {
                                        tr {
                                            key: "{u.segment_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.segment_code == u.segment_code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = u.segment_code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{u.segment_code}" }
                                            td { {u.name.clone().unwrap_or_default()} }
                                            if status_tab {
                                                StatusCells { status: u.status() }
                                                ConfirmCells { confirmed: u.confirmed, by: u.confirmed_by.clone() }
                                            } else {
                                                td { class: "num", "{u.length}" }
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

            // 單身（aooi403.4fd 下半部）：選定的獨立段的編碼編號
            if let Some(c) = cur.clone() {
                Card {
                    div { class: "table-header",
                        h3 { {tf("encoding-segment-values-title", &[("code", c.segment_code.as_str())])} }
                        if perm.can("detail") && perm.add_row() {
                            Button { icon: icons::ADD, tone: Tone::Primary, variant: ButtonVariant::Tonal, disabled: !c.active, onclick: {
                                let c = c.clone();
                                move |_| {
                                    value_error.set(None);
                                    value_form.set(EncodingSegmentValueInput { version: Some(c.version), ..Default::default() });
                                    value_editor.set(Some(None));
                                }
                            }, {t("encoding-segment-action-add-value")} }
                        }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("encoding-segment-field-line")} }
                                    th { {t("encoding-segment-field-value")} }
                                    th { {t("encoding-segment-field-value-name")} }
                                    th {}
                                }
                            }
                            tbody {
                                if c.values.is_empty() {
                                    tr { td { colspan: 4, class: "muted", {t("common-msg-no-data")} } }
                                }
                                for row in c.values.iter().cloned() {
                                    tr { key: "{row.line}",
                                        td { class: "num", "{row.line}" }
                                        td { class: "mono", "{row.value}" }
                                        td { {row.name.clone().unwrap_or_default()} }
                                        td {
                                            if perm.can("detail") {
                                                Button { icon: icons::EDIT, tone: Tone::Neutral, variant: ButtonVariant::Text, disabled: !c.active, onclick: {
                                                    let (row, version) = (row.clone(), c.version);
                                                    move |_| {
                                                        value_error.set(None);
                                                        value_form.set(EncodingSegmentValueInput { line: Some(row.line), value: row.value.clone(), name: row.name.clone(), version: Some(version) });
                                                        value_editor.set(Some(Some(row.line)));
                                                    }
                                                }, {t("common-action-edit")} }
                                            }
                                            if perm.can("detail") && perm.delete_row() {
                                                Button { icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Text, disabled: !c.active, onclick: {
                                                    let (code, line, version) = (c.segment_code.clone(), row.line, c.version);
                                                    move |_| {
                                                        let code = code.clone();
                                                        spawn(async move {
                                                            match api::remove_encoding_segment_value(&code, line, version).await {
                                                                Ok(r) => done(tf("encoding-segment-msg-value-deleted", &[("line", line.to_string().as_str())]), Some(r.segment_code)),
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
                        p { class: "query-hint", {t("encoding-segment-hint-inactive")} }
                    }
                }
            }

            if let Some(line) = value_editor() {
                Dialog {
                    title: match line {
                        None => t("encoding-segment-dialog-new-value"),
                        Some(l) => tf("encoding-segment-dialog-edit-value", &[("line", l.to_string().as_str())]),
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
                            id: "gek02",
                            label: t("encoding-segment-field-line"),
                            input_type: "number",
                            readonly: line.is_some(),
                            value: value_form().line.map(|n| n.to_string()).unwrap_or_default(),
                            oninput: move |x: String| value_form.write().line = x.trim().parse().ok(),
                        }
                        TextField { id: "gek03", label: t("encoding-segment-field-value"), maxlength: max_len::VALUE, value: value_form().value, oninput: move |x| value_form.write().value = x }
                        TextField { id: "gek04", label: t("encoding-segment-field-value-name"), maxlength: max_len::NAME, value: value_form().name.unwrap_or_default(), oninput: move |x| value_form.write().name = opt(x) }
                    }
                    p { class: "muted", {tf("encoding-segment-hint-value", &[("length", cur.as_ref().map(|c| c.length.to_string()).unwrap_or_default().as_str())])} }
                }
            }

            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("encoding-segment-dialog-new"),
                        Editor::Edit(code) => tf("encoding-segment-dialog-edit", &[("code", code.as_str())]),
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
                        TextField { id: "gej01", label: t("encoding-segment-field-code"), maxlength: max_len::CODE, value: v.segment_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().segment_code = x }
                        TextField { id: "gej02", label: t("encoding-segment-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                        TextField {
                            id: "gej03",
                            label: t("encoding-segment-field-length"),
                            input_type: "number",
                            value: v.length.map(|n| n.to_string()).unwrap_or_default(),
                            oninput: move |x: String| form.write().length = x.trim().parse().ok(),
                        }
                    }
                    p { class: "muted", {t("encoding-segment-hint")} }
                }
            }
        }
    }
}
