//! Data Dictionary Maintenance 資料字典維護（TipTop p_zta 檔案架構修整記錄維護作業）
//! 版面參考 tiptop/azz/4fd/p_zta.4fd：表頭（模組、類型、上游/下游、產生者/日期）＋
//! 單身頁籤（欄位 Field / 索引 Index / 限制 Constraint）；另加多語言表名稱與變更紀錄。
//! 結構唯讀、不執行 DDL（結構只由 migrations/ 變更）；規則在後端 DataDictionaryService。

use dioxus::prelude::*;
use ttrs_shared::{
    ColumnTextDto, ConstraintKindDto, TableDictionaryDto, TableDictionaryInput, TableTextDto,
    TableTypeDto,
};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Notification, NotificationKind, Select, Tabs, TextField,
};
use crate::components::multilingual::{LangField, LangValues, MultilingualField};
use crate::i18n::{self, UiLang, tf};

const DETAIL_TABS: [&str; 4] = [
    "dictionary-tab-columns",
    "dictionary-tab-indexes",
    "dictionary-tab-constraints",
    "dictionary-tab-history",
];

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
                div { class: "program-id", {i18n::t("dictionary-breadcrumb")} }
                h2 { {i18n::t("dictionary-title")} }
            }
            p { class: "query-hint",
                {i18n::t("dictionary-hint")}
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
                        TextField { id: "dict-filter", label: i18n::t("dictionary-field-filter"), value: filter(), oninput: move |v| filter.set(v) }
                        match &*tables.read() {
                            None => rsx! { p { class: "muted", {i18n::t("common-msg-loading")} } },
                            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
                            Some(Ok(list)) if list.is_empty() => rsx! {
                                p { class: "muted", {i18n::t("dictionary-msg-no-tables")} }
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
                                                span { class: "md-chip dict-chip", {i18n::t("dictionary-chip-documented")} }
                                            }
                                        }
                                    }
                                }
                            },
                        }
                        Button {
                            variant: ButtonVariant::Text,
                            onclick: move |_| { tables.restart(); dictionaries.restart(); },
                            {i18n::t("dictionary-action-refresh")}
                        }
                    }
                }

                // ----- 右：明細 -----
                div {
                    match selected() {
                        None => rsx! { Card { div { class: "card-body muted", {i18n::t("dictionary-msg-select-table")} } } },
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
    let detail_tab = use_signal(|| 0usize);

    let loaded = dict.read().clone();
    rsx! {
        match loaded {
            None => rsx! { Card { div { class: "card-body muted", {i18n::t("common-msg-loading")} } } },
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
                    TableTextForm {
                        table: table.clone(),
                        texts: d.texts.clone(),
                        on_ok,
                        on_error,
                    }
                }
                Card {
                    Tabs { tabs: DETAIL_TABS.iter().map(|s| i18n::t(s)).collect::<Vec<_>>(), active: detail_tab }
                    div { class: "card-body",
                        match detail_tab() {
                            0 => rsx! {
                                ColumnsTab { table: table.clone(), on_ok, on_error }
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
    std::iter::once((String::new(), i18n::t("dictionary-opt-type-none")))
        .chain(
            TableTypeDto::ALL
                .iter()
                .enumerate()
                .map(|(i, k)| (i.to_string(), i18n::t(type_id(*k)))),
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

/// gat03/gat04/gat05 表名稱、目的、備註（各語系）
fn table_values(texts: &[TableTextDto]) -> LangValues {
    LangValues::from_rows(
        texts,
        |x| &x.lang,
        &[
            ("name", |x| x.name.clone()),
            ("purpose", |x| x.purpose.clone()),
            ("remark", |x| x.remark.clone()),
        ],
    )
}

/// gaq03/gaq04/gaq05 欄位名稱、說明、額外說明（某欄位的各語系）
fn column_values(texts: &[ColumnTextDto], column: &str) -> LangValues {
    let own: Vec<&ColumnTextDto> = texts.iter().filter(|x| x.column_name == column).collect();
    LangValues::from_rows(
        &own,
        |x| &x.lang,
        &[
            ("name", |x| x.name.clone()),
            ("description", |x| x.description.clone()),
            ("extra", |x| x.extra.clone()),
        ],
    )
}

/// 欄位說明在介面語系的顯示文字；無則退回繁中（ADR 0005）
fn column_display(
    texts: &[ColumnTextDto],
    column: &str,
    get: fn(&ColumnTextDto) -> Option<String>,
) -> String {
    let pick = |lang: &str| {
        texts
            .iter()
            .find(|x| x.column_name == column && x.lang == lang)
            .and_then(get)
    };
    pick(i18n::current().code())
        .or_else(|| pick(UiLang::ZhTw.code()))
        .unwrap_or_default()
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
                h3 { class: "form-section-title", {tf("dictionary-msg-not-documented", &[("table", table.as_str())])} }
                div { class: "form-grid",
                    TextField { id: "new-module", label: i18n::t("dictionary-field-module"), value: module(), oninput: move |v| module.set(v) }
                    Select { id: "new-type", label: i18n::t("dictionary-field-type"), options: type_options(), value: kind(), onchange: move |v| kind.set(v) }
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
                                    Ok(d) => on_created.call(tf("dictionary-msg-created", &[("table", d.table_name.as_str())])),
                                    Err(e) => on_error.call(e),
                                }
                            });
                        },
                        {i18n::t("dictionary-action-create")}
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
                    TextField { id: "zta01", label: i18n::t("dictionary-field-table-name"), value: dict.table_name.clone(), readonly: true }
                    TextField { id: "gat06", label: i18n::t("dictionary-field-module"), value: module(), oninput: move |v| module.set(v) }
                    Select { id: "gat07", label: i18n::t("dictionary-field-type"), options: type_options(), value: kind(), onchange: move |v| kind.set(v) }
                    TextField { id: "zta05", label: i18n::t("dictionary-field-created"), value: format!("{} / {}", dict.created_by, dict.created_on), readonly: true }
                    TextField { id: "zta11", class: "span-2", label: i18n::t("dictionary-field-upstream"), maxlength: 80, value: upstream(), oninput: move |v| upstream.set(v) }
                    TextField { id: "zta12", class: "span-2", label: i18n::t("dictionary-field-downstream"), maxlength: 60, value: downstream(), oninput: move |v| downstream.set(v) }
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
                                    Ok(_) => on_saved.call(tf("dictionary-msg-saved", &[("table", t.as_str())])),
                                    Err(e) => on_error.call(e),
                                }
                            });
                        },
                        {i18n::t("dictionary-action-save")}
                    }
                    Button {
                        variant: ButtonVariant::Error,
                        onclick: move |_| {
                            let t = table2.clone();
                            spawn(async move {
                                match api::delete_dictionary(&t).await {
                                    Ok(()) => on_saved.call(tf("dictionary-msg-deleted", &[("table", t.as_str())])),
                                    Err(e) => on_error.call(e),
                                }
                            });
                        },
                        {i18n::t("dictionary-action-delete")}
                    }
                }
            }
        }
    }
}

#[component]
fn TableTextForm(
    table: String,
    texts: Vec<TableTextDto>,
    on_ok: EventHandler<String>,
    on_error: EventHandler<String>,
) -> Element {
    let values = use_signal(|| table_values(&texts));
    let mut loaded = use_signal(|| table_values(&texts));
    rsx! {
        div { class: "card-body",
            h3 { class: "form-section-title", {i18n::t("dictionary-section-table-text")} }
            MultilingualField {
                id: "gat",
                fields: vec![
                    LangField::text("name", i18n::t("dictionary-field-text-name"), 120),
                    LangField::area("purpose", i18n::t("dictionary-field-text-purpose"), 255, 2),
                    LangField::text("remark", i18n::t("dictionary-field-text-remark"), 255),
                ],
                values,
            }
            div { class: "action-bar grant-bar",
                Button {
                    variant: ButtonVariant::Tonal,
                    onclick: move |_| {
                        let now = values();
                        let langs = now.changed(&loaded());
                        let t = table.clone();
                        spawn(async move {
                            // 每個有異動的語系各存一次（PUT …/texts 一次一個語系）
                            for l in langs {
                                let text = TableTextDto {
                                    lang: l.code().to_owned(),
                                    name: now.opt(l, "name"),
                                    purpose: now.opt(l, "purpose"),
                                    remark: now.opt(l, "remark"),
                                };
                                if let Err(e) = api::set_table_text(&t, &text).await {
                                    on_error.call(e);
                                    return;
                                }
                            }
                            loaded.set(now);
                            on_ok.call(tf("dictionary-msg-table-text-saved", &[("table", t.as_str())]));
                        });
                    },
                    {i18n::t("dictionary-action-save-table-text")}
                }
            }
        }
    }
}

#[component]
fn ColumnsTab(
    table: String,
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
    let mut values = use_signal(LangValues::default);
    let mut loaded = use_signal(LangValues::default);

    // 匯出與清單顯示都用介面語系（無說明時退回繁中）
    let export_href = api::export_url(&table, i18n::current().code());
    let text_list: Vec<ColumnTextDto> = texts
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();

    rsx! {
        div { class: "action-bar",
            span { class: "muted", {i18n::t("dictionary-hint-columns")} }
            div { class: "spacer" }
            a { class: "md-button md-button--outlined md-state", href: "{export_href}", {i18n::t("dictionary-action-export")} }
        }
        match &*columns.read() {
            None => rsx! { p { class: "muted", {i18n::t("common-msg-loading")} } },
            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
            Some(Ok(cols)) => rsx! {
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for h in ["dictionary-col-column", "dictionary-col-type", "dictionary-col-not-null", "dictionary-col-pk", "dictionary-col-default", "dictionary-col-name", "dictionary-col-description"] { th { key: "{h}", {i18n::t(h)} } }
                        } }
                        tbody {
                            for c in cols.iter().cloned() {
                                {
                                    let col = c.name.clone();
                                    let all = text_list.clone();
                                    rsx! {
                                        tr {
                                            key: "{c.name}",
                                            class: "md-table__row--clickable",
                                            class: if editing().as_deref() == Some(c.name.as_str()) { "md-table__row--selected" },
                                            onclick: move |_| {
                                                let v = column_values(&all, &col);
                                                values.set(v.clone());
                                                loaded.set(v);
                                                editing.set(Some(col.clone()));
                                            },
                                            td { class: "mono", "{c.name}" }
                                            td { class: "mono", "{c.data_type}" }
                                            td { if c.not_null { "✓" } }
                                            td { if c.primary_key { "PK" } }
                                            td { class: "mono", {c.default.clone().unwrap_or_default()} }
                                            td { {column_display(&text_list, &c.name, |x| x.name.clone())} }
                                            td { {column_display(&text_list, &c.name, |x| x.description.clone())} }
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
                h3 { class: "form-section-title", {tf("dictionary-section-column-text", &[("column", col.as_str())])} }
                MultilingualField {
                    id: "gaq",
                    fields: vec![
                        LangField::text("name", i18n::t("dictionary-field-column-name"), 255),
                        LangField::text("description", i18n::t("dictionary-field-column-description"), 500),
                        LangField::area("extra", i18n::t("dictionary-field-column-extra"), 2000, 2),
                    ],
                    values,
                }
                div { class: "action-bar grant-bar",
                    Button {
                        variant: ButtonVariant::Tonal,
                        onclick: move |_| {
                            let now = values();
                            let langs = now.changed(&loaded());
                            let (t, col) = (table.clone(), col.clone());
                            spawn(async move {
                                for l in langs {
                                    let text = ColumnTextDto {
                                        column_name: col.clone(),
                                        lang: l.code().to_owned(),
                                        name: now.opt(l, "name"),
                                        description: now.opt(l, "description"),
                                        extra: now.opt(l, "extra"),
                                    };
                                    if let Err(e) = api::set_column_text(&t, &text).await {
                                        on_error.call(e);
                                        texts.restart();
                                        return;
                                    }
                                }
                                loaded.set(now);
                                on_ok.call(tf("dictionary-msg-column-text-saved", &[("column", col.as_str())]));
                                texts.restart();
                            });
                        },
                        {i18n::t("dictionary-action-save-column-text")}
                    }
                    Button { variant: ButtonVariant::Text, onclick: move |_| editing.set(None), {i18n::t("common-action-cancel")} }
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
            None => rsx! { p { class: "muted", {i18n::t("common-msg-loading")} } },
            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
            Some(Ok(list)) if list.is_empty() => rsx! { p { class: "muted", {i18n::t("dictionary-msg-no-indexes")} } },
            Some(Ok(list)) => rsx! {
                table { class: "md-table",
                    thead { tr { th { {i18n::t("dictionary-col-index-name")} } th { {i18n::t("dictionary-col-unique")} } th { {i18n::t("dictionary-col-columns")} } } }
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
            None => rsx! { p { class: "muted", {i18n::t("common-msg-loading")} } },
            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
            Some(Ok(list)) => rsx! {
                table { class: "md-table",
                    thead { tr { th { {i18n::t("dictionary-col-constraint-name")} } th { {i18n::t("dictionary-col-constraint-kind")} } th { {i18n::t("dictionary-col-columns")} } th { {i18n::t("dictionary-col-references")} } th { {i18n::t("dictionary-col-check")} } } }
                    tbody {
                        for c in list.iter().cloned() {
                            tr { key: "{c.name}",
                                td { class: "mono", "{c.name}" }
                                td {
                                    {i18n::t(match c.kind {
                                        ConstraintKindDto::PrimaryKey => "dictionary-opt-kind-pk",
                                        ConstraintKindDto::ForeignKey => "dictionary-opt-kind-fk",
                                        ConstraintKindDto::Unique => "dictionary-opt-kind-unique",
                                        ConstraintKindDto::Check => "dictionary-opt-kind-check",
                                    })}
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
            None => rsx! { p { class: "muted", {i18n::t("common-msg-loading")} } },
            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
            Some(Ok(list)) => rsx! {
                p { class: "muted", {i18n::t("dictionary-hint-history")} }
                table { class: "md-table",
                    thead { tr { th { {i18n::t("dictionary-col-version")} } th { {i18n::t("dictionary-col-migration")} } } }
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

fn type_id(k: TableTypeDto) -> &'static str {
    match k {
        TableTypeDto::Parameter => "dictionary-opt-type-parameter",
        TableTypeDto::Master => "dictionary-opt-type-master",
        TableTypeDto::Transaction => "dictionary-opt-type-transaction",
        TableTypeDto::Statistics => "dictionary-opt-type-statistics",
        TableTypeDto::Base => "dictionary-opt-type-base",
    }
}
