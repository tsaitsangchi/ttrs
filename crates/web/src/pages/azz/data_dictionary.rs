//! Data Dictionary Maintenance 資料字典維護（TipTop p_zta 檔案架構修整記錄維護作業）
//! 版面比照 tiptop/azz/4fd/p_zta.4fd，視覺採 Material 3（components::material）：
//!   動作列（查詢 / 新增 / 修改 / 刪除）＋ 上下筆（4fd 底部 cnt）
//!   ＋ 上方頁籤 pc712（Main：左 zta01/gat06/gat07、右 gat03/gat04/zta11/zta12；Info：zta06/zta05）
//!   ＋ 下方頁籤 pc2（Field s_ztb / Index s_ztc / Constraint s_ztd＋限制欄位＋限制條件；另加變更紀錄）
//!   ＋ 資料字典清單。
//! 表名稱、目的等多語言欄位以翻譯圖示開窗輸入各語系（ADR 0005）。
//! 4fd 中 TipTop 專屬欄位（zta02 Database、zta07 Synonym、zta17 SynonymDB、lockmode、zta10 Create）不移植。
//! 結構唯讀、不執行 DDL（結構只由 migrations/ 變更）；規則在後端 DataDictionaryService。

use dioxus::prelude::*;
use ttrs_shared::{
    ColumnInfoDto, ColumnTextDto, ConstraintDto, ConstraintKindDto, TableDictionaryDto,
    TableDictionaryInput, TableTextDto, TableTypeDto,
};

use crate::api;
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, IconButton, Notification, NotificationKind, Select, Tabs,
    TextArea, TextField, Tone, icons,
};
use crate::components::multilingual::{
    LangField, LangValues, MultilingualDialogField, MultilingualField,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{self, UiLang, tf};
use crate::pages::azz::legacy_catalog::{LegacyCatalog, LegacyMappingTab};

const UPPER_TABS: [&str; 2] = ["dictionary-tab-main", "dictionary-tab-info"];
const LOWER_TABS: [&str; 5] = [
    "dictionary-tab-columns",
    "dictionary-tab-indexes",
    "dictionary-tab-constraints",
    "dictionary-tab-history",
    "dictionary-tab-legacy",
];

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    View,
    New,
    Edit,
    /// 輸入查詢條件
    Query,
}

/// 查詢條件（資料表代碼、模組、類型）；空白為不限
#[derive(Clone, Default, PartialEq)]
struct DictQuery {
    table: String,
    module: String,
    kind: String,
}

/// 表頭輸入（gat06 模組、gat07 類型、zta11/zta12 上游/下游）
#[derive(Clone, Default, PartialEq)]
struct Header {
    table: String,
    module: String,
    kind: String,
    upstream: String,
    downstream: String,
    /// 版本號（ADR 0012 §4）；新增時為 0
    version: i32,
}

impl Header {
    fn of(d: &TableDictionaryDto) -> Self {
        Self {
            table: d.table_name.clone(),
            module: d.module_code.clone(),
            kind: type_value(d.table_type),
            upstream: d.upstream.clone().unwrap_or_default(),
            downstream: d.downstream.clone().unwrap_or_default(),
            version: d.version,
        }
    }

    fn input(&self) -> TableDictionaryInput {
        TableDictionaryInput {
            table_name: self.table.clone(),
            module_code: self.module.clone(),
            table_type: parse_type(&self.kind),
            upstream: opt(&self.upstream),
            downstream: opt(&self.downstream),
            version: Some(self.version),
        }
    }
}

/// 權限檢查用的程式代號（TipTop 對照目錄也歸在 p_zta，authorization.feature AQ3）
const PROGRAM: &str = "data-dictionary";

/// 資料字典維護；工具列可切換到 TipTop 對照目錄（legacy_catalog.feature C1）
#[component]
pub fn DataDictionary() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, DataDictionaryPage {} }
    }
}

#[component]
fn DataDictionaryPage() -> Element {
    let mut catalog_mode = use_signal(|| false);
    rsx! {
        if catalog_mode() {
            div { class: "page",
                div { class: "page-header",
                    div { class: "program-id", {i18n::t("dictionary-breadcrumb")} }
                    h2 { {i18n::t("legacy-title")} }
                }
                LegacyCatalog { onclose: move |_| catalog_mode.set(false) }
            }
        } else {
            DictionaryMain { on_catalog: move |_| catalog_mode.set(true) }
        }
    }
}

#[component]
fn DictionaryMain(on_catalog: EventHandler<()>) -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut mode = use_signal(|| Mode::View);
    let mut upper_tab = use_signal(|| 0usize);
    let lower_tab = use_signal(|| 0usize);

    // 已建字典的資料表（表名稱依介面語系，無則退回繁中）
    let mut dictionaries = use_resource(api::list_dictionaries);
    let schema_tables = use_resource(api::schema_tables);
    let mut query = use_signal(DictQuery::default);
    let mut draft_query = use_signal(DictQuery::default);
    // 目前顯示查詢結果的第幾筆（0 起算）
    let mut pos = use_signal(|| None::<usize>);

    let mut header = use_signal(Header::default);
    let mut texts = use_signal(LangValues::default);
    let mut loaded_texts = use_signal(LangValues::default);

    let all: Vec<TableDictionaryDto> = dictionaries
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let list: Vec<TableDictionaryDto> = all
        .iter()
        .filter(|d| matches(d, &query()))
        .cloned()
        .collect();
    let total = list.len();
    let cur: Option<TableDictionaryDto> = pos().and_then(|i| list.get(i).cloned());
    let filtered = query() != DictQuery::default();

    // 顯示某一筆（或清空），回到瀏覽模式
    let mut show = move |d: Option<TableDictionaryDto>| {
        header.set(d.as_ref().map(Header::of).unwrap_or_default());
        let v = d
            .as_ref()
            .map(|d| table_values(&d.texts))
            .unwrap_or_default();
        texts.set(v.clone());
        loaded_texts.set(v);
        mode.set(Mode::View);
    };
    // 清單載入、查詢或換筆後，表單跟著目前這一筆（預設停在第一筆）
    use_effect(move || {
        let list: Vec<TableDictionaryDto> = match &*dictionaries.read() {
            Some(Ok(all)) => all
                .iter()
                .filter(|d| matches(d, &query()))
                .cloned()
                .collect(),
            _ => return,
        };
        let i = match pos() {
            None if !list.is_empty() => {
                pos.set(Some(0));
                return;
            }
            i => i,
        };
        if *mode.peek() == Mode::View {
            show(i.and_then(|i| list.get(i).cloned()));
        }
    });

    let mut done = move |msg: String, table: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        mode.set(Mode::View);
        query.set(DictQuery::default());
        dictionaries.restart();
        // 重新載入後定位到該表；刪除則回到第一筆
        spawn(async move {
            if let Ok(list) = api::list_dictionaries().await {
                pos.set(match table {
                    Some(t) => list.iter().position(|d| d.table_name == t),
                    None => (!list.is_empty()).then_some(0),
                });
                if let Some(d) = pos().and_then(|i| list.get(i).cloned()) {
                    show(Some(d));
                }
            }
        });
    };

    let save = move |_| {
        let h = header();
        let now = texts();
        let langs = now.changed(&loaded_texts());
        let creating = mode() == Mode::New;
        spawn(async move {
            let saved = if creating {
                api::create_dictionary(&h.input()).await
            } else {
                api::update_dictionary(&h.table, &h.input()).await
            };
            if let Err(e) = saved {
                error.set(Some(e));
                return;
            }
            // 每個有異動的語系各存一次（PUT …/texts 一次一個語系）
            for l in langs {
                let text = TableTextDto {
                    lang: l.code().to_owned(),
                    name: now.opt(l, "name"),
                    purpose: now.opt(l, "purpose"),
                    remark: now.opt(l, "remark"),
                };
                if let Err(e) = api::set_table_text(&h.table, &text).await {
                    error.set(Some(e));
                    return;
                }
            }
            let msg = if creating {
                tf("dictionary-msg-created", &[("table", h.table.as_str())])
            } else {
                tf("dictionary-msg-saved", &[("table", h.table.as_str())])
            };
            done(msg, Some(h.table));
        });
    };

    let delete = move |_| {
        let table = header.read().table.clone();
        let version = header.read().version;
        spawn(async move {
            match api::delete_dictionary(&table, version).await {
                Ok(()) => done(
                    tf("dictionary-msg-deleted", &[("table", table.as_str())]),
                    None,
                ),
                Err(e) => error.set(Some(e)),
            }
        });
    };

    // 新增時可選的資料表：資料庫中有、但尚未建立字典者（D1）
    let undocumented: Vec<String> = schema_tables
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|tables| {
            tables
                .iter()
                .filter(|t| !all.iter().any(|d| &d.table_name == *t))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    let cur_cancel = cur.clone();
    let m = mode();
    let ro = m == Mode::View;
    let q = m == Mode::Query;
    let h = header();
    let no_cur = cur.is_none();
    let table = h.table.clone();
    // 下方頁籤顯示的資料表：目前這一筆；新增時為已選的資料表（尚未建字典也可先看結構）
    let has_table = !table.is_empty() && !q;

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {i18n::t("dictionary-breadcrumb")} }
                h2 { {i18n::t("dictionary-title")} }
            }

            // ----- 動作列 -----
            div { class: "action-bar",
                match m {
                    Mode::New | Mode::Edit => rsx! {
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {i18n::t("common-action-ok")} }
                        Button {
                            icon: icons::CLOSE, tone: Tone::Neutral,
                            variant: ButtonVariant::Text,
                            onclick: move |_| { error.set(None); show(cur_cancel.clone()); },
                            {i18n::t("common-action-cancel")}
                        }
                    },
                    Mode::Query => rsx! {
                        Button {
                            icon: icons::CHECK, tone: Tone::Success,
                            onclick: move |_| {
                                error.set(None);
                                query.set(draft_query());
                                let n = all.iter().filter(|d| matches(d, &draft_query())).count();
                                message.set(Some(if n == 0 {
                                    i18n::t("query-msg-none")
                                } else {
                                    tf("query-msg-matched", &[("count", n.to_string().as_str())])
                                }));
                                pos.set((n > 0).then_some(0));
                                mode.set(Mode::View);
                            },
                            {i18n::t("common-action-ok")}
                        }
                        Button {
                            icon: icons::CLOSE, tone: Tone::Neutral,
                            variant: ButtonVariant::Text,
                            onclick: move |_| { error.set(None); mode.set(Mode::View); },
                            {i18n::t("common-action-cancel")}
                        }
                    },
                    Mode::View => rsx! {
                        if perm.can("query") {
                            Button {
                                icon: icons::SEARCH, tone: Tone::Primary,
                                variant: ButtonVariant::Tonal,
                                onclick: move |_| {
                                    message.set(None);
                                    error.set(None);
                                    draft_query.set(DictQuery::default());
                                    upper_tab.set(0);
                                    mode.set(Mode::Query);
                                },
                                {i18n::t("common-action-query")}
                            }
                        }
                        if perm.can("insert") {
                            Button {
                                icon: icons::ADD, tone: Tone::Success,
                                onclick: move |_| {
                                    message.set(None);
                                    error.set(None);
                                    header.set(Header::default());
                                    texts.set(LangValues::default());
                                    loaded_texts.set(LangValues::default());
                                    upper_tab.set(0);
                                    mode.set(Mode::New);
                                },
                                {i18n::t("common-action-new")}
                            }
                        }
                        if perm.can("modify") {
                            Button {
                                icon: icons::EDIT, tone: Tone::Tertiary,
                                variant: ButtonVariant::Tonal,
                                disabled: no_cur,
                                onclick: move |_| mode.set(Mode::Edit),
                                {i18n::t("common-action-edit")}
                            }
                        }
                        // 異動記錄（ADR 0012 §8，功能 history）
                        if perm.can("history") {
                            HistoryButton { program: PROGRAM, record: cur.as_ref().map(|d| d.table_name.clone()) }
                        }
                        if perm.can("delete") {
                            Button {
                                icon: icons::DELETE, tone: Tone::Error,
                                variant: ButtonVariant::Error,
                                disabled: no_cur,
                                onclick: delete,
                                {i18n::t("common-action-delete")}
                            }
                        }
                        div { class: "spacer" }
                        Button {
                            icon: icons::OPEN_IN_NEW, tone: Tone::Primary,
                            variant: ButtonVariant::Outlined,
                            onclick: move |_| {
                                message.set(None);
                                error.set(None);
                                on_catalog.call(());
                            },
                            {i18n::t("legacy-action-open")}
                        }
                        if !no_cur && perm.can("exporttoexcel") {
                            a {
                                class: "md-button md-button--outlined md-state md-button--with-icon",
                                href: api::export_url(&table, i18n::current().code()),
                                svg { class: "md-button__icon md-icon--primary", width: "18", height: "18", view_box: "0 0 24 24", fill: "currentColor", "aria-hidden": "true", path { d: icons::DOWNLOAD } }
                                {i18n::t("dictionary-action-export")}
                            }
                        }
                        // 列印（PDF）、匯出 Excel（XLSX）：資料表的欄位字典（ADR 0013 SA3）
                        MaintenanceToolbar {
                            print: Act::new(!no_cur, {
                                let table = table.clone();
                                move |_| {
                                    let tab = api::ReportTab::open(api::ReportFormat::Pdf);
                                    let table = table.clone();
                                    spawn(async move {
                                        if let Err(e) = api::system_list_report(&format!("/api/dictionary/tables/{table}"), &table, api::ReportFormat::Pdf, &[], i18n::current().code(), tab).await {
                                            error.set(Some(e));
                                        }
                                    });
                                }
                            }),
                            export: Act::new(!no_cur, {
                                let table = table.clone();
                                move |_| {
                                    let tab = api::ReportTab::open(api::ReportFormat::Excel);
                                    let table = table.clone();
                                    spawn(async move {
                                        if let Err(e) = api::system_list_report(&format!("/api/dictionary/tables/{table}"), &table, api::ReportFormat::Excel, &[], i18n::current().code(), tab).await {
                                            error.set(Some(e));
                                        }
                                    });
                                }
                            }),
                        }
                    },
                }
            }

            if q {
                p { class: "query-hint", {i18n::t("dictionary-query-hint")} }
            }

            // ----- 筆數與上下筆（4fd 底部 cnt） -----
            if m == Mode::View && total > 0 {
                div { class: "record-nav",
                    IconButton { label: i18n::t("nav-first"), path: icons::FIRST_PAGE, disabled: pos() == Some(0), onclick: move |_| pos.set(Some(0)) }
                    IconButton {
                        label: i18n::t("nav-previous"),
                        path: icons::CHEVRON_LEFT,
                        disabled: pos().is_none_or(|i| i == 0),
                        onclick: move |_| pos.set(pos().map(|i| i.saturating_sub(1))),
                    }
                    span { class: "record-nav__count",
                        match pos() {
                            Some(i) => rsx! { {tf("nav-record-position", &[("pos", (i + 1).to_string().as_str()), ("total", total.to_string().as_str())])} },
                            None => rsx! { {tf("nav-record-count", &[("total", total.to_string().as_str())])} },
                        }
                    }
                    IconButton {
                        label: i18n::t("nav-next"),
                        path: icons::CHEVRON_RIGHT,
                        disabled: pos().is_some_and(|i| i + 1 >= total),
                        onclick: move |_| pos.set(Some(pos().map_or(0, |i| (i + 1).min(total - 1)))),
                    }
                    IconButton { label: i18n::t("nav-last"), path: icons::LAST_PAGE, disabled: pos() == Some(total - 1), onclick: move |_| pos.set(Some(total - 1)) }
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            // ----- 上方頁籤 pc712（Main / Info） -----
            Card {
                Tabs { tabs: UPPER_TABS.iter().map(|s| i18n::t(s)).collect::<Vec<_>>(), active: upper_tab }
                div { class: "card-body",
                    if q {
                        div { class: "form-grid cols-4",
                            TextField { id: "q-zta01", label: i18n::t("dictionary-field-table-name"), value: draft_query().table, placeholder: i18n::t("dictionary-hint-table-query"), oninput: move |v| draft_query.write().table = v }
                            TextField { id: "q-gat06", label: i18n::t("dictionary-field-module"), value: draft_query().module, oninput: move |v| draft_query.write().module = v }
                            Select { id: "q-gat07", label: i18n::t("dictionary-field-type"), options: type_options(), value: draft_query().kind, onchange: move |v| draft_query.write().kind = v }
                        }
                    } else if upper_tab() == 0 {
                        div { class: "dict-main",
                            // 4fd gr3074：資料表代碼、模組、資料表類型
                            div { class: "dict-main__col",
                                if m == Mode::New {
                                    Select {
                                        id: "zta01",
                                        label: i18n::t("dictionary-field-table-name"),
                                        options: std::iter::once((String::new(), i18n::t("dictionary-opt-select-table")))
                                            .chain(undocumented.iter().map(|t| (t.clone(), t.clone())))
                                            .collect::<Vec<_>>(),
                                        value: h.table.clone(),
                                        onchange: move |v| header.write().table = v,
                                    }
                                } else {
                                    TextField { id: "zta01", label: i18n::t("dictionary-field-table-name"), value: h.table.clone(), readonly: true }
                                }
                                TextField { id: "gat06", label: i18n::t("dictionary-field-module"), maxlength: 10, value: h.module.clone(), readonly: ro, oninput: move |v| header.write().module = v }
                                Select { id: "gat07", label: i18n::t("dictionary-field-type"), options: type_options(), value: h.kind.clone(), readonly: ro, onchange: move |v| header.write().kind = v }
                            }
                            // 4fd gr3075：表名稱、表目的、上游/下游檔案（另加備註 gat05）
                            div { class: "dict-main__col",
                                MultilingualDialogField { id: "gat03", field: LangField::text("name", i18n::t("dictionary-field-text-name"), 120), values: texts, readonly: ro }
                                MultilingualDialogField { id: "gat04", field: LangField::area("purpose", i18n::t("dictionary-field-text-purpose"), 255, 2), values: texts, readonly: ro }
                                TextField { id: "zta11", label: i18n::t("dictionary-field-upstream"), maxlength: 80, value: h.upstream.clone(), readonly: ro, oninput: move |v| header.write().upstream = v }
                                TextField { id: "zta12", label: i18n::t("dictionary-field-downstream"), maxlength: 60, value: h.downstream.clone(), readonly: ro, oninput: move |v| header.write().downstream = v }
                                MultilingualDialogField { id: "gat05", field: LangField::text("remark", i18n::t("dictionary-field-text-remark"), 255), values: texts, readonly: ro }
                            }
                        }
                    } else {
                        // 4fd Info：產生者 zta06、產生日期 zta05
                        div { class: "form-grid cols-4",
                            TextField { id: "zta06", label: i18n::t("dictionary-field-created-by"), readonly: true, value: cur.as_ref().map(|d| d.created_by.clone()).unwrap_or_default() }
                            TextField { id: "zta05", label: i18n::t("dictionary-field-created-on"), readonly: true, value: cur.as_ref().map(|d| d.created_on.format("%Y/%m/%d").to_string()).unwrap_or_default() }
                            // 最後修改時間（到秒，ADR 0012 §9）
                            TextField { id: "zta-updated-at", label: i18n::t("common-field-updated-at"), readonly: true, value: cur.as_ref().and_then(|d| d.updated_at).map(crate::components::history::local_time).unwrap_or_default() }
                        }
                    }
                }
            }

            // ----- 下方頁籤 pc2（Field / Index / Constraint / 變更紀錄），比照 4fd 一律顯示 -----
            Card {
                Tabs { tabs: LOWER_TABS.iter().map(|s| i18n::t(s)).collect::<Vec<_>>(), active: lower_tab }
                div { class: "card-body",
                    if !has_table {
                        p { class: "muted", {i18n::t("dictionary-msg-no-table")} }
                    }
                    // 以資料表為 key：換筆時重新載入
                    if has_table {
                        for t in [table.clone()] {
                            match lower_tab() {
                                0 => rsx! { ColumnsTab { key: "{t}", table: t.clone(), editable: m == Mode::Edit && perm.can("maintain_field_description"), on_error: move |e| error.set(Some(e)) } },
                                1 => rsx! { IndexesTab { key: "{t}", table: t.clone() } },
                                2 => rsx! { ConstraintsTab { key: "{t}", table: t.clone() } },
                                3 => rsx! { HistoryTab { key: "{t}", table: t.clone() } },
                                _ => rsx! { LegacyMappingTab { key: "{t}", table: t.clone() } },
                            }
                        }
                    }
                }
            }

            // ----- 資料字典清單 -----
            Card {
                div { class: "table-header",
                    h3 { {i18n::t("dictionary-list-title")} }
                    p {
                        if filtered { {tf("list-count-filtered", &[("total", total.to_string().as_str())])} } else { {tf("list-count-all", &[("total", total.to_string().as_str())])} }
                        {i18n::t("list-hint-click")}
                    }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for id in ["dictionary-field-table-name", "dictionary-field-text-name", "dictionary-field-module", "dictionary-field-type", "dictionary-field-created-by", "dictionary-field-created-on"] {
                                th { key: "{id}", {i18n::t(id)} }
                            }
                        } }
                        tbody {
                            match &*dictionaries.read() {
                                None => rsx! { tr { td { colspan: 6, {i18n::t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 6, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: 6, class: "muted", {i18n::t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for (i, d) in list.iter().cloned().enumerate() {
                                        tr {
                                            key: "{d.table_name}",
                                            class: "md-table__row--clickable",
                                            class: if pos() == Some(i) { "md-table__row--selected" },
                                            onclick: move |_| {
                                                if mode() == Mode::View {
                                                    message.set(None);
                                                    error.set(None);
                                                    pos.set(Some(i));
                                                }
                                            },
                                            td { class: "mono", "{d.table_name}" }
                                            td { {d.name.clone().unwrap_or_default()} }
                                            td { "{d.module_code}" }
                                            td { {d.table_type.map(|k| i18n::t(type_id(k))).unwrap_or_default()} }
                                            td { "{d.created_by}" }
                                            td { {d.created_on.format("%Y/%m/%d").to_string()} }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 查詢條件比對：資料表代碼支援 `*`（任意字元）、`?`（單一字元）、`|`（或）；空白為不限
fn matches(d: &TableDictionaryDto, q: &DictQuery) -> bool {
    let table = q.table.trim();
    let module = q.module.trim();
    (table.is_empty() || table.split('|').any(|p| glob(p.trim(), &d.table_name)))
        && (module.is_empty()
            || module
                .split('|')
                .any(|p| glob(&p.trim().to_uppercase(), &d.module_code)))
        && (q.kind.is_empty() || parse_type(&q.kind) == d.table_type)
}

fn glob(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some('*'), _) => go(&p[1..], t) || (!t.is_empty() && go(p, &t[1..])),
            (Some('?'), Some(_)) => go(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &t[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    go(&p, &t)
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
    v.parse::<usize>()
        .ok()
        .and_then(|i| TableTypeDto::ALL.get(i).copied())
}

fn opt(v: &str) -> Option<String> {
    Some(v.trim().to_owned()).filter(|s| !s.is_empty())
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

/// 型別與長度分開顯示（4fd ztb04 Type、ztb08 length），如 varchar(20) → varchar、20
fn split_type(data_type: &str) -> (String, String) {
    match data_type.split_once('(') {
        Some((t, rest)) => (t.to_owned(), rest.trim_end_matches(')').to_owned()),
        None => (data_type.to_owned(), String::new()),
    }
}

/// 4fd s_ztb：欄位名稱 ztb03、型態 ztb04、長度 ztb08、不允許null ztb09、預設值 ztb11、
/// 屬性 ztb05、欄位說明 gaq03、額外說明 gaq05。
/// 點選欄位開窗：修改模式可編輯各語系說明，否則唯讀。
#[component]
fn ColumnsTab(table: String, editable: bool, on_error: EventHandler<String>) -> Element {
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
    let values = use_signal(LangValues::default);
    let loaded = use_signal(LangValues::default);

    let text_list: Vec<ColumnTextDto> = texts
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();

    rsx! {
        p { class: "muted",
            if editable { {i18n::t("dictionary-hint-columns-edit")} } else { {i18n::t("dictionary-hint-columns")} }
        }
        match &*columns.read() {
            None => rsx! { p { class: "muted", {i18n::t("common-msg-loading")} } },
            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
            Some(Ok(cols)) => rsx! {
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for h in ["dictionary-col-column", "dictionary-col-type", "dictionary-col-length", "dictionary-col-not-null", "dictionary-col-default", "dictionary-col-attr", "dictionary-col-field-desc", "dictionary-col-more-desc"] {
                                th { key: "{h}", {i18n::t(h)} }
                            }
                        } }
                        tbody {
                            for c in cols.iter().cloned() {
                                {column_row(&table, c, &text_list, editing, values, loaded)}
                            }
                        }
                    }
                }
            },
        }
        if let Some(col) = editing() {
            Dialog {
                title: tf("dictionary-section-column-text", &[("column", col.as_str())]),
                oncancel: move |_| editing.set(None),
                actions: rsx! {
                    if editable {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| editing.set(None), {i18n::t("common-action-cancel")} }
                        Button {
                            icon: icons::CHECK, tone: Tone::Success,
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
                                            return;
                                        }
                                    }
                                    editing.set(None);
                                    texts.restart();
                                });
                            },
                            {i18n::t("common-action-ok")}
                        }
                    } else {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| editing.set(None), {i18n::t("common-action-close")} }
                    }
                },
                MultilingualField {
                    id: "gaq",
                    fields: vec![
                        LangField::text("name", i18n::t("dictionary-field-column-name"), 255),
                        LangField::text("description", i18n::t("dictionary-field-column-description"), 500),
                        LangField::area("extra", i18n::t("dictionary-field-column-extra"), 2000, 2),
                    ],
                    values,
                    readonly: !editable,
                }
            }
        }
    }
}

fn column_row(
    table: &str,
    c: ColumnInfoDto,
    all: &[ColumnTextDto],
    mut editing: Signal<Option<String>>,
    mut values: Signal<LangValues>,
    mut loaded: Signal<LangValues>,
) -> Element {
    let (kind, length) = split_type(&c.data_type);
    let name = column_display(all, &c.name, |x| x.name.clone());
    let extra = column_display(all, &c.name, |x| x.extra.clone());
    let v = column_values(all, &c.name);
    let attr = attr_id(column_is_custom(table, &c.name));
    let default = c
        .default
        .as_deref()
        .map(display_default)
        .unwrap_or_default();
    let col = c.name.clone();
    rsx! {
        tr {
            key: "{c.name}",
            class: "md-table__row--clickable",
            onclick: move |_| {
                values.set(v.clone());
                loaded.set(v.clone());
                editing.set(Some(col.clone()));
            },
            td { class: "mono", "{c.name}" }
            td { class: "mono", "{kind}" }
            td { class: "mono num", "{length}" }
            td { if c.not_null { "✓" } }
            td { class: "mono", "{default}" }
            td { {i18n::t(attr)} }
            td { "{name}" }
            td { class: "dict-extra", "{extra}" }
        }
    }
}

/// 屬性（ztb05/ztc05/ztd12）：TipTop 存在 zte_file，新系統不移植 zte_file，
/// 一律依 TipTop 預設規則推導（p_zta.4gl:2260 欄位、:1997 索引、:2964 限制）：
/// 客製開頭 → c:客製，其餘 p:標準
fn attr_id(custom: bool) -> &'static str {
    if custom {
        "dictionary-opt-attr-c"
    } else {
        "dictionary-opt-attr-p"
    }
}

/// 欄位屬性：欄位以 tc_/ta_ 開頭，或資料表以 tc_ 開頭為客製（p_zta.4gl:2263）
fn column_is_custom(table: &str, column: &str) -> bool {
    column.starts_with("tc_") || column.starts_with("ta_") || table.starts_with("tc_")
}

/// 預設值去掉字串引號與型別轉換（比照 TQC-860023，p_zta.4gl:2241）：`'5'::text` → `5`；
/// 函式（now()）、常數（false、CURRENT_DATE）照原樣
fn display_default(raw: &str) -> String {
    let body = raw.trim();
    let Some(rest) = body.strip_prefix('\'') else {
        return body.to_owned();
    };
    // 找結尾引號（'' 為跳脫的單引號），之後只允許 ::型別
    let mut out = String::new();
    let mut chars = rest.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            if chars.peek() == Some(&'\'') {
                chars.next();
                out.push('\'');
                continue;
            }
            let tail: String = chars.collect();
            let tail = tail.trim();
            return if tail.is_empty() || tail.starts_with("::") {
                out
            } else {
                body.to_owned()
            };
        }
        out.push(ch);
    }
    body.to_owned()
}

/// 4fd s_ztc：唯一、索引名稱、索引欄位、屬性
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
                    thead { tr {
                        th { {i18n::t("dictionary-col-unique")} }
                        th { {i18n::t("dictionary-col-index-name")} }
                        th { {i18n::t("dictionary-col-index-columns")} }
                        th { {i18n::t("dictionary-col-attr")} }
                    } }
                    tbody {
                        for i in list.iter().cloned() {
                            tr { key: "{i.name}",
                                td { if i.unique { "✓" } }
                                td { class: "mono", "{i.name}" }
                                td { class: "mono", {i.columns.join(", ")} }
                                td { {i18n::t(attr_id(i.name.starts_with("tic_")))} }
                            }
                        }
                    }
                }
            },
        }
    }
}

fn kind_id(k: ConstraintKindDto) -> &'static str {
    match k {
        ConstraintKindDto::PrimaryKey => "dictionary-opt-kind-pk",
        ConstraintKindDto::ForeignKey => "dictionary-opt-kind-fk",
        ConstraintKindDto::Unique => "dictionary-opt-kind-unique",
        ConstraintKindDto::Check => "dictionary-opt-kind-check",
    }
}

/// 4fd Constraint 頁：限制清單 s_ztd ＋ 選取限制的「限制欄位」s_ref ＋「限制條件」condition
#[component]
fn ConstraintsTab(table: String) -> Element {
    let constraints = use_resource(move || {
        let t = table.clone();
        async move { api::table_constraints(&t).await }
    });
    let mut selected = use_signal(|| 0usize);
    let list: Vec<ConstraintDto> = constraints
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let cur = list.get(selected()).cloned();
    rsx! {
        match &*constraints.read() {
            None => rsx! { p { class: "muted", {i18n::t("common-msg-loading")} } },
            Some(Err(e)) => rsx! { p { class: "muted", "{e}" } },
            Some(Ok(_)) => rsx! {
                table { class: "md-table",
                    thead { tr {
                        th { {i18n::t("dictionary-col-constraint-name")} }
                        th { {i18n::t("dictionary-col-constraint-kind")} }
                        th { {i18n::t("dictionary-col-attr")} }
                    } }
                    tbody {
                        for (i, c) in list.iter().cloned().enumerate() {
                            tr {
                                key: "{c.name}",
                                class: "md-table__row--clickable",
                                class: if selected() == i { "md-table__row--selected" },
                                onclick: move |_| selected.set(i),
                                td { class: "mono", "{c.name}" }
                                td { {i18n::t(kind_id(c.kind))} }
                                td { {i18n::t(attr_id(c.name.starts_with("tpc_")))} }
                            }
                        }
                    }
                }
                if let Some(c) = cur {
                    div { class: "dict-constraint",
                        section { class: "dict-group",
                            h4 { class: "form-section-title", {i18n::t("dictionary-group-constraint-columns")} }
                            table { class: "md-table",
                                thead { tr {
                                    th { {i18n::t("dictionary-col-position")} }
                                    th { {i18n::t("dictionary-col-column")} }
                                    th { {i18n::t("dictionary-col-ref-constraint")} }
                                    th { {i18n::t("dictionary-col-ref-table")} }
                                    th { {i18n::t("dictionary-col-ref-column")} }
                                } }
                                tbody {
                                    for (n, col) in c.columns.iter().cloned().enumerate() {
                                        tr { key: "{col}",
                                            td { class: "num", "{n + 1}" }
                                            td { class: "mono", "{col}" }
                                            td { class: "mono", {c.ref_constraint.clone().unwrap_or_default()} }
                                            td { class: "mono", {c.ref_table.clone().unwrap_or_default()} }
                                            td { class: "mono", {c.ref_columns.get(n).cloned().unwrap_or_default()} }
                                        }
                                    }
                                }
                            }
                        }
                        section { class: "dict-group",
                            h4 { class: "form-section-title", {i18n::t("dictionary-group-constraint-text")} }
                            TextArea {
                                id: "condition",
                                label: i18n::t("dictionary-col-check"),
                                rows: 4,
                                readonly: true,
                                value: c.check_clause.clone().unwrap_or_default(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matches_tiptop_style_patterns() {
        assert!(glob("program*", "program_name"));
        assert!(glob("program", "program"));
        assert!(!glob("program", "program_name"), "無萬用字元時須完全相同"); // i18n-allow：測試訊息
        assert!(glob("?tem", "item"));
        assert!(glob("*_text", "table_dictionary_text"));
    }

    #[test]
    fn default_without_quotes_and_casts() {
        assert_eq!(display_default("'5'::text"), "5");
        assert_eq!(display_default("'Y'::bpchar"), "Y");
        assert_eq!(display_default("'it''s'::text"), "it's");
        assert_eq!(display_default("now()"), "now()");
        assert_eq!(display_default("false"), "false");
        assert_eq!(
            display_default("nextval('seq'::regclass)"),
            "nextval('seq'::regclass)"
        );
    }

    #[test]
    fn attribute_follows_tiptop_default_rule() {
        assert!(column_is_custom("program", "tc_abc01"));
        assert!(column_is_custom("program", "ta_abc01"));
        assert!(column_is_custom("tc_aaa", "aaa01"));
        assert!(!column_is_custom("program", "program_no"));
    }

    #[test]
    fn splits_type_and_length() {
        assert_eq!(split_type("varchar(20)"), ("varchar".into(), "20".into()));
        assert_eq!(
            split_type("numeric(15,3)"),
            ("numeric".into(), "15,3".into())
        );
        assert_eq!(split_type("text"), ("text".into(), String::new()));
    }
}
