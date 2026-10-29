//! Data Dictionary Maintenance 資料字典維護（TipTop p_zta 檔案架構修整記錄維護作業）
//! 版面參考 tiptop/azz/4fd/p_zta.4fd：表頭（模組、類型、上游/下游、產生者/日期）＋
//! 單身頁籤（欄位 Field / 索引 Index / 限制 Constraint）；另加多語言表名稱與變更紀錄。
//! 結構唯讀、不執行 DDL（結構只由 migrations/ 變更）；規則在後端 DataDictionaryService。

use dioxus::prelude::*;
use ttrs_shared::{
    ColumnTextDto, ConstraintKindDto, DICTIONARY_LANGS, TableDictionaryDto, TableDictionaryInput,
    TableTextDto, TableTypeDto,
};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Notification, NotificationKind, Select, Tabs, TextArea, TextField,
};

const LANG_TABS: [&str; 3] = ["繁體中文", "English", "简体中文"];
const DETAIL_TABS: [&str; 4] = ["欄位", "索引", "限制", "變更紀錄"];

#[component]
pub fn DataDictionary() -> Element {
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut filter = use_signal(String::new);
    let mut selected = use_signal(|| None::<String>);

    let mut tables = use_resource(api::schema_tables);
    let mut dictionaries = use_resource(api::list_dictionaries);

    let mut ok = move |msg: String| {
        error.set(None);
        message.set(Some(msg));
        dictionaries.restart();
    };

    let documented: Vec<String> = dictionaries
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|l| l.iter().map(|d| d.table_name.clone()).collect())
        .unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", "系統管理 / Data Dictionary" }
                h2 { "資料字典維護" }
            }
            p { class: "query-hint",
                "維護資料表與欄位的名稱與多語言說明。欄位、索引、限制取自資料庫實際結構（唯讀）；結構變更請透過 migration。"
            }
            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            div { class: "dict-layout",
                // ----- 左：資料庫中的資料表 -----
                Card {
                    div { class: "card-body dict-list",
                        TextField { id: "dict-filter", label: "篩選資料表", value: filter(), oninput: move |v| filter.set(v) }
                        match &*tables.read() {
                            None => rsx! { p { class: "muted", "載入中…" } },
                            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
                            Some(Ok(list)) if list.is_empty() => rsx! {
                                p { class: "muted", "沒有可顯示的資料表（記憶體模式沒有實際資料庫結構，請設定 DATABASE_URL）" }
                            },
                            Some(Ok(list)) => rsx! {
                                ul { class: "dict-tables",
                                    for t in list.iter().filter(|t| t.contains(filter().trim())).cloned() {
                                        li {
                                            key: "{t}",
                                            class: if selected().as_deref() == Some(t.as_str()) { "active" },
                                            onclick: {
                                                let t = t.clone();
                                                move |_| {
                                                    error.set(None);
                                                    message.set(None);
                                                    selected.set(Some(t.clone()));
                                                }
                                            },
                                            span { class: "mono", "{t}" }
                                            if documented.contains(&t) {
                                                span { class: "md-chip dict-chip", "已建字典" }
                                            }
                                        }
                                    }
                                }
                            },
                        }
                        Button {
                            variant: ButtonVariant::Text,
                            onclick: move |_| { tables.restart(); dictionaries.restart(); },
                            "重新整理"
                        }
                    }
                }

                // ----- 右：明細 -----
                div {
                    match selected() {
                        None => rsx! { Card { div { class: "card-body muted", "請從左側選擇資料表" } } },
                        Some(t) => rsx! {
                            TableDetail {
                                key: "{t}",
                                table: t,
                                on_ok: move |m: String| ok(m),
                                on_error: move |e: String| error.set(Some(e)),
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn TableDetail(
    table: String,
    on_ok: EventHandler<String>,
    on_error: EventHandler<String>,
) -> Element {
    let t = table.clone();
    let mut dict = use_resource(move || {
        let t = t.clone();
        async move { api::get_dictionary(&t).await }
    });
    let lang_tab = use_signal(|| 0usize);
    let detail_tab = use_signal(|| 0usize);

    let loaded = dict.read().clone();
    rsx! {
        match loaded {
            None => rsx! { Card { div { class: "card-body muted", "載入中…" } } },
            Some(Err(e)) => rsx! { Card { div { class: "card-body muted", "{e}" } } },
            Some(Ok(None)) => rsx! {
                CreateForm {
                    table: table.clone(),
                    on_created: move |m: String| { on_ok.call(m); dict.restart(); },
                    on_error,
                }
            },
            Some(Ok(Some(d))) => rsx! {
                HeaderForm {
                    dict: d.clone(),
                    on_saved: move |m: String| { on_ok.call(m); dict.restart(); },
                    on_error,
                }
                Card {
                    Tabs { tabs: LANG_TABS.to_vec(), active: lang_tab }
                    // 以語系為 key：切換語系時重新載入該語系的說明
                    for l in [lang_tab()] {
                        TableTextForm {
                            key: "{l}",
                            table: table.clone(),
                            lang: DICTIONARY_LANGS[l].to_owned(),
                            on_ok,
                            on_error,
                        }
                    }
                }
                Card {
                    Tabs { tabs: DETAIL_TABS.to_vec(), active: detail_tab }
                    div { class: "card-body",
                        match detail_tab() {
                            0 => rsx! {
                                ColumnsTab { table: table.clone(), lang: DICTIONARY_LANGS[lang_tab()].to_owned(), on_ok, on_error }
                            },
                            1 => rsx! { IndexesTab { table: table.clone() } },
                            2 => rsx! { ConstraintsTab { table: table.clone() } },
                            _ => rsx! { HistoryTab { table: table.clone() } },
                        }
                    }
                }
            },
        }
    }
}

fn type_options() -> Vec<(String, String)> {
    std::iter::once((String::new(), "（未指定）".to_owned()))
        .chain(
            TableTypeDto::ALL
                .iter()
                .enumerate()
                .map(|(i, t)| (i.to_string(), t.label().to_owned())),
        )
        .collect()
}

fn type_value(t: Option<TableTypeDto>) -> String {
    t.and_then(|t| TableTypeDto::ALL.iter().position(|x| *x == t))
        .map(|i| i.to_string())
        .unwrap_or_default()
}

fn parse_type(v: &str) -> Option<TableTypeDto> {
    v.parse::<usize>().ok().map(|i| TableTypeDto::ALL[i])
}

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

#[component]
fn CreateForm(
    table: String,
    on_created: EventHandler<String>,
    on_error: EventHandler<String>,
) -> Element {
    let mut module = use_signal(String::new);
    let mut kind = use_signal(String::new);
    let t = table.clone();
    rsx! {
        Card {
            div { class: "card-body",
                h3 { class: "form-section-title", "{table}：尚未建立字典" }
                div { class: "form-grid",
                    TextField { id: "new-module", label: "模組", value: module(), oninput: move |v| module.set(v) }
                    Select { id: "new-type", label: "類型", options: type_options(), value: kind(), onchange: move |v| kind.set(v) }
                }
                div { class: "action-bar grant-bar",
                    Button {
                        onclick: move |_| {
                            let req = TableDictionaryInput {
                                table_name: t.clone(),
                                module_code: module(),
                                table_type: parse_type(&kind()),
                                ..Default::default()
                            };
                            spawn(async move {
                                match api::create_dictionary(&req).await {
                                    Ok(d) => on_created.call(format!("已建立 {} 的資料字典", d.table_name)),
                                    Err(e) => on_error.call(e),
                                }
                            });
                        },
                        "建立字典"
                    }
                }
            }
        }
    }
}

#[component]
fn HeaderForm(
    dict: TableDictionaryDto,
    on_saved: EventHandler<String>,
    on_error: EventHandler<String>,
) -> Element {
    let d = dict.clone();
    let mut module = use_signal(|| d.module_code.clone());
    let mut kind = use_signal(|| type_value(d.table_type));
    let mut upstream = use_signal(|| d.upstream.clone().unwrap_or_default());
    let mut downstream = use_signal(|| d.downstream.clone().unwrap_or_default());
    let table = dict.table_name.clone();
    let table2 = table.clone();
    rsx! {
        Card {
            div { class: "card-body",
                div { class: "form-grid cols-4",
                    TextField { id: "zta01", label: "資料表代碼", value: dict.table_name.clone(), readonly: true }
                    TextField { id: "gat06", label: "模組", value: module(), oninput: move |v| module.set(v) }
                    Select { id: "gat07", label: "類型", options: type_options(), value: kind(), onchange: move |v| kind.set(v) }
                    TextField { id: "zta05", label: "建立者／日期", value: format!("{} / {}", dict.created_by, dict.created_on), readonly: true }
                    TextField { id: "zta11", class: "span-2", label: "上游檔案", maxlength: 80, value: upstream(), oninput: move |v| upstream.set(v) }
                    TextField { id: "zta12", class: "span-2", label: "下游檔案", maxlength: 60, value: downstream(), oninput: move |v| downstream.set(v) }
                }
                div { class: "action-bar grant-bar",
                    Button {
                        onclick: move |_| {
                            let req = TableDictionaryInput {
                                table_name: table.clone(),
                                module_code: module(),
                                table_type: parse_type(&kind()),
                                upstream: opt(upstream()),
                                downstream: opt(downstream()),
                            };
                            let t = table.clone();
                            spawn(async move {
                                match api::update_dictionary(&t, &req).await {
                                    Ok(_) => on_saved.call(format!("已儲存 {t}")),
                                    Err(e) => on_error.call(e),
                                }
                            });
                        },
                        "儲存"
                    }
                    Button {
                        variant: ButtonVariant::Error,
                        onclick: move |_| {
                            let t = table2.clone();
                            spawn(async move {
                                match api::delete_dictionary(&t).await {
                                    Ok(()) => on_saved.call(format!("已刪除 {t} 的資料字典（含多語言說明；資料表本身不受影響）")),
                                    Err(e) => on_error.call(e),
                                }
                            });
                        },
                        "刪除字典"
                    }
                }
            }
        }
    }
}

#[component]
fn TableTextForm(
    table: String,
    lang: String,
    on_ok: EventHandler<String>,
    on_error: EventHandler<String>,
) -> Element {
    let t = table.clone();
    let l = lang.clone();
    let texts = use_resource(move || {
        let (t, l) = (t.clone(), l.clone());
        async move {
            api::table_texts(&t)
                .await
                .map(|v| v.into_iter().find(|x| x.lang == l).unwrap_or_default())
        }
    });
    let mut name = use_signal(String::new);
    let mut purpose = use_signal(String::new);
    let mut remark = use_signal(String::new);
    use_effect(move || {
        if let Some(Ok(x)) = &*texts.read() {
            name.set(x.name.clone().unwrap_or_default());
            purpose.set(x.purpose.clone().unwrap_or_default());
            remark.set(x.remark.clone().unwrap_or_default());
        }
    });
    rsx! {
        div { class: "card-body",
            div { class: "form-grid",
                TextField { id: "gat03", label: "表名稱", maxlength: 120, value: name(), oninput: move |v| name.set(v) }
                TextField { id: "gat05", label: "備註", maxlength: 255, value: remark(), oninput: move |v| remark.set(v) }
                div { class: "span-all",
                    TextArea { id: "gat04", label: "表目的", rows: 2, maxlength: 255, value: purpose(), oninput: move |v| purpose.set(v) }
                }
            }
            div { class: "action-bar grant-bar",
                Button {
                    variant: ButtonVariant::Tonal,
                    onclick: move |_| {
                        let text = TableTextDto { lang: lang.clone(), name: opt(name()), purpose: opt(purpose()), remark: opt(remark()) };
                        let t = table.clone();
                        spawn(async move {
                            match api::set_table_text(&t, &text).await {
                                Ok(()) => on_ok.call(format!("已儲存 {t} 的 {} 說明", text.lang)),
                                Err(e) => on_error.call(e),
                            }
                        });
                    },
                    "儲存表說明"
                }
            }
        }
    }
}

#[component]
fn ColumnsTab(
    table: String,
    lang: String,
    on_ok: EventHandler<String>,
    on_error: EventHandler<String>,
) -> Element {
    let t1 = table.clone();
    let columns = use_resource(move || {
        let t = t1.clone();
        async move { api::table_columns(&t).await }
    });
    let t2 = table.clone();
    let mut texts = use_resource(move || {
        let t = t2.clone();
        async move { api::column_texts(&t).await }
    });
    let mut editing = use_signal(|| None::<String>);
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut extra = use_signal(String::new);

    let export_href = api::export_url(&table, &lang);
    let text_list: Vec<ColumnTextDto> = texts
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let l = lang.clone();
    let text_of = move |col: &str| {
        text_list
            .iter()
            .find(|x| x.column_name == col && x.lang == l)
            .cloned()
            .unwrap_or_default()
    };

    rsx! {
        div { class: "action-bar",
            span { class: "muted", "語系：{lang}　點選欄位以編輯說明" }
            div { class: "spacer" }
            a { class: "md-button md-button--outlined md-state", href: "{export_href}", "匯出 CSV" }
        }
        match &*columns.read() {
            None => rsx! { p { class: "muted", "載入中…" } },
            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
            Some(Ok(cols)) => rsx! {
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for h in ["欄位代碼", "型別", "不可空白", "主鍵", "預設值", "名稱", "說明"] { th { key: "{h}", "{h}" } }
                        } }
                        tbody {
                            for c in cols.iter().cloned() {
                                {
                                    let x = text_of(&c.name);
                                    let col = c.name.clone();
                                    rsx! {
                                        tr {
                                            key: "{c.name}",
                                            class: "md-table__row--clickable",
                                            class: if editing().as_deref() == Some(c.name.as_str()) { "md-table__row--selected" },
                                            onclick: {
                                                let x = x.clone();
                                                move |_| {
                                                    editing.set(Some(col.clone()));
                                                    name.set(x.name.clone().unwrap_or_default());
                                                    description.set(x.description.clone().unwrap_or_default());
                                                    extra.set(x.extra.clone().unwrap_or_default());
                                                }
                                            },
                                            td { class: "mono", "{c.name}" }
                                            td { class: "mono", "{c.data_type}" }
                                            td { if c.not_null { "✓" } }
                                            td { if c.primary_key { "PK" } }
                                            td { class: "mono", {c.default.clone().unwrap_or_default()} }
                                            td { {x.name.clone().unwrap_or_default()} }
                                            td { {x.description.clone().unwrap_or_default()} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
        }
        if let Some(col) = editing() {
            section { class: "form-section",
                h3 { class: "form-section-title", "欄位說明：{col}（{lang}）" }
                div { class: "form-grid",
                    TextField { id: "gaq03", label: "欄位名稱", maxlength: 255, value: name(), oninput: move |v| name.set(v) }
                    TextField { id: "gaq04", label: "欄位說明", maxlength: 500, value: description(), oninput: move |v| description.set(v) }
                    div { class: "span-all",
                        TextArea { id: "gaq05", label: "額外說明", rows: 3, maxlength: 2000, value: extra(), oninput: move |v| extra.set(v) }
                    }
                }
                div { class: "action-bar grant-bar",
                    Button {
                        variant: ButtonVariant::Tonal,
                        onclick: move |_| {
                            let text = ColumnTextDto {
                                column_name: col.clone(),
                                lang: lang.clone(),
                                name: opt(name()),
                                description: opt(description()),
                                extra: opt(extra()),
                            };
                            let t = table.clone();
                            spawn(async move {
                                match api::set_column_text(&t, &text).await {
                                    Ok(()) => { on_ok.call(format!("已儲存欄位 {} 的說明", text.column_name)); texts.restart(); }
                                    Err(e) => on_error.call(e),
                                }
                            });
                        },
                        "儲存欄位說明"
                    }
                    Button { variant: ButtonVariant::Text, onclick: move |_| editing.set(None), "取消" }
                }
            }
        }
    }
}

#[component]
fn IndexesTab(table: String) -> Element {
    let indexes = use_resource(move || {
        let t = table.clone();
        async move { api::table_indexes(&t).await }
    });
    rsx! {
        match &*indexes.read() {
            None => rsx! { p { class: "muted", "載入中…" } },
            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
            Some(Ok(list)) if list.is_empty() => rsx! { p { class: "muted", "沒有一般索引（主鍵、唯一限制的索引列在「限制」）" } },
            Some(Ok(list)) => rsx! {
                table { class: "md-table",
                    thead { tr { th { "索引名稱" } th { "唯一" } th { "欄位" } } }
                    tbody {
                        for i in list.iter().cloned() {
                            tr { key: "{i.name}",
                                td { class: "mono", "{i.name}" }
                                td { if i.unique { "✓" } }
                                td { class: "mono", {i.columns.join(", ")} }
                            }
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn ConstraintsTab(table: String) -> Element {
    let constraints = use_resource(move || {
        let t = table.clone();
        async move { api::table_constraints(&t).await }
    });
    rsx! {
        match &*constraints.read() {
            None => rsx! { p { class: "muted", "載入中…" } },
            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
            Some(Ok(list)) => rsx! {
                table { class: "md-table",
                    thead { tr { th { "限制名稱" } th { "類型" } th { "欄位" } th { "參照" } th { "條件" } } }
                    tbody {
                        for c in list.iter().cloned() {
                            tr { key: "{c.name}",
                                td { class: "mono", "{c.name}" }
                                td {
                                    match c.kind {
                                        ConstraintKindDto::PrimaryKey => "主鍵",
                                        ConstraintKindDto::ForeignKey => "外鍵",
                                        ConstraintKindDto::Unique => "唯一",
                                        ConstraintKindDto::Check => "檢查",
                                    }
                                }
                                td { class: "mono", {c.columns.join(", ")} }
                                td { class: "mono",
                                    if let Some(rt) = c.ref_table.clone() { "{rt}({c.ref_columns.join(\", \")})" }
                                }
                                td { class: "mono", {c.check_clause.clone().unwrap_or_default()} }
                            }
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn HistoryTab(table: String) -> Element {
    let history = use_resource(move || {
        let t = table.clone();
        async move { api::table_history(&t).await }
    });
    rsx! {
        match &*history.read() {
            None => rsx! { p { class: "muted", "載入中…" } },
            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
            Some(Ok(list)) => rsx! {
                p { class: "muted", "內容涉及此資料表的 migration（結構變更紀錄）" }
                table { class: "md-table",
                    thead { tr { th { "版本" } th { "說明" } } }
                    tbody {
                        for m in list.iter().cloned() {
                            tr { key: "{m.version}",
                                td { class: "mono num", "{m.version:04}" }
                                td { "{m.description}" }
                            }
                        }
                    }
                }
            },
        }
    }
}
