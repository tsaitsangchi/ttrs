//! 基本代碼維護（共用畫面）：區域別 aooi100、國別 aooi090、地區 aooi110、交運方式 aooi160、報表抬頭 aooi210。
//! 版面比照各作業的 4fd（RC1、SM1、RH1）：多筆單身表格（代號、名稱、隸屬），新增／修改以小視窗編輯。
//! 共用層：全公司共用。規則在後端 BasicCodeService。
//! 標準功能鈕（ADR 0013）：查詢（表格第一列輸入條件）、新增、修改、複製、刪除、確認、停用／啟用、列印、匯出 Excel、
//! 異動記錄、上下筆（移動選取列）。報表抬頭的名稱為多語言（RH4）。

use dioxus::prelude::*;
use ttrs_shared::{
    BasicCodeDto, BasicCodeInput, BasicCodeKindDto, BasicCodeNameDto, QbeCondition, QueryRequest,
    basic_code_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Select, Tabs, TextField,
    Tone, icons,
};
use crate::components::multilingual::{LangField, LangValues, MultilingualDialogField};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::qbe::{ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav};
use crate::components::status::{
    self, ConfirmCells, ConfirmHeaders, ConfirmQbeCells, HasStatus, StatusCells, StatusHeaders,
    StatusQbeCells, TAB_DATA, TAB_STATUS,
};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{self, UiLang, t, tf};

/// 各作業的畫面文字 ID（locales/<語系>/basic_code.ftl）
struct Texts {
    breadcrumb: &'static str,
    title: &'static str,
    list_title: &'static str,
    code: &'static str,
    name: &'static str,
    parent: &'static str,
    dialog_new: &'static str,
    dialog_edit: &'static str,
    /// 下載檔名
    file: &'static str,
}

fn texts(kind: BasicCodeKindDto) -> Texts {
    match kind {
        BasicCodeKindDto::Region => Texts {
            breadcrumb: "region-breadcrumb",
            title: "region-title",
            list_title: "region-list-title",
            code: "region-field-code",
            name: "region-field-name",
            parent: "",
            dialog_new: "region-dialog-new",
            dialog_edit: "region-dialog-edit",
            file: "regions",
        },
        BasicCodeKindDto::Country => Texts {
            breadcrumb: "country-breadcrumb",
            title: "country-title",
            list_title: "country-list-title",
            code: "country-field-code",
            name: "country-field-name",
            parent: "country-field-parent",
            dialog_new: "country-dialog-new",
            dialog_edit: "country-dialog-edit",
            file: "countries",
        },
        BasicCodeKindDto::Area => Texts {
            breadcrumb: "area-breadcrumb",
            title: "area-title",
            list_title: "area-list-title",
            code: "area-field-code",
            name: "area-field-name",
            parent: "area-field-parent",
            dialog_new: "area-dialog-new",
            dialog_edit: "area-dialog-edit",
            file: "areas",
        },
        BasicCodeKindDto::ShippingMethod => Texts {
            breadcrumb: "shipping-method-breadcrumb",
            title: "shipping-method-title",
            list_title: "shipping-method-list-title",
            code: "shipping-method-field-code",
            name: "shipping-method-field-name",
            parent: "",
            dialog_new: "shipping-method-dialog-new",
            dialog_edit: "shipping-method-dialog-edit",
            file: "shipping-methods",
        },
        BasicCodeKindDto::ReportTitle => Texts {
            breadcrumb: "report-title-breadcrumb",
            title: "report-title-title",
            list_title: "report-title-list-title",
            code: "report-title-field-code",
            name: "report-title-field-name",
            parent: "",
            dialog_new: "report-title-dialog-new",
            dialog_edit: "report-title-dialog-edit",
            file: "report-titles",
        },
    }
}

/// 區域別維護（aooi100）
#[component]
pub fn RegionMaintenance() -> Element {
    rsx! { BasicCodeMaintenance { kind: BasicCodeKindDto::Region } }
}

/// 國別維護（aooi090）
#[component]
pub fn CountryMaintenance() -> Element {
    rsx! { BasicCodeMaintenance { kind: BasicCodeKindDto::Country } }
}

/// 地區維護（aooi110）
#[component]
pub fn AreaMaintenance() -> Element {
    rsx! { BasicCodeMaintenance { kind: BasicCodeKindDto::Area } }
}

/// 交運方式維護（aooi160）
#[component]
pub fn ShippingMethodMaintenance() -> Element {
    rsx! { BasicCodeMaintenance { kind: BasicCodeKindDto::ShippingMethod } }
}

/// 報表抬頭維護（aooi210）
#[component]
pub fn ReportTitleMaintenance() -> Element {
    rsx! { BasicCodeMaintenance { kind: BasicCodeKindDto::ReportTitle } }
}

#[component]
fn BasicCodeMaintenance(kind: BasicCodeKindDto) -> Element {
    rsx! {
        ProgramGate { program: kind.program(), BasicCodePage { key: "{kind.slug()}", kind } }
    }
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

/// 報表抬頭各語系名稱 → 輸入（zh-TW 存 name，其他語系存 names）
fn apply_names(input: &mut BasicCodeInput, values: &LangValues) {
    input.name = values.opt(UiLang::ZhTw, "name");
    input.names = UiLang::ALL
        .into_iter()
        .filter(|l| *l != UiLang::ZhTw)
        .filter_map(|l| {
            values.opt(l, "name").map(|name| BasicCodeNameDto {
                lang: l.code().to_owned(),
                name,
            })
        })
        .collect();
}

#[component]
fn BasicCodePage(kind: BasicCodeKindDto) -> Element {
    let tx = texts(kind);
    let program = kind.program();
    let has_parent = kind.parent().is_some();
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    // 查詢：qbe 為查詢列正在輸入的條件，conditions 為目前生效的條件（QBE4：預設查全部）
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut codes = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: Some(i18n::current().code().to_owned()),
        };
        async move { api::query_basic_codes(kind, &req).await }
    });
    // 隸屬欄位的選項（國別 → 區域、地區 → 國別；開窗 q_gea、q_geb）
    let parents = use_resource(move || async move {
        match kind.parent() {
            Some(pk) => api::list_basic_codes(pk, i18n::current().code())
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        }
    });
    let selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        codes
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|c| c.code.clone()).collect())
            .unwrap_or_default()
    });
    let mut selected = selected;
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(BasicCodeInput::default);
    let mut names = use_signal(LangValues::default);
    let mut dialog_error = use_signal(|| None::<String>);

    let page_data = codes.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<BasicCodeDto> = page_data
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
    let cur: Option<BasicCodeDto> = selected()
        .and_then(|c| list.iter().find(|x| x.code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let data_cols = if has_parent { 3 } else { 2 };
    let cols = if status_tab {
        2 + status::ALL.len() + 2
    } else {
        data_cols
    };
    let keys: Vec<String> = list.iter().map(|c| c.code.clone()).collect();

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        codes.restart();
    };

    // 執行查詢：先試查第 1 頁，條件有誤時留在查詢列顯示錯誤
    let run_query = move |_| {
        let conds = conditions_of(&qbe.read());
        let req = QueryRequest {
            conditions: conds.clone(),
            page: Some(1),
            page_size: Some(PAGE_SIZE),
            lang: Some(i18n::current().code().to_owned()),
        };
        spawn(async move {
            match api::query_basic_codes(kind, &req).await {
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
        let mut req = form();
        if kind.multilingual() {
            apply_names(&mut req, &names.read());
        }
        spawn(async move {
            let result = match target {
                Some(Editor::Edit(code)) => api::update_basic_code(kind, &code, &req).await,
                _ => api::create_basic_code(kind, &req).await,
            };
            match result {
                Ok(c) => done(
                    tf("basic-code-msg-saved", &[("code", c.code.as_str())]),
                    Some(c.code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    // 停用／啟用、確認／取消確認
    let set_state = move |c: BasicCodeDto, state: &'static str, msg: &'static str| {
        spawn(async move {
            match api::set_basic_code_state(kind, &c.code, state, c.version).await {
                Ok(r) => done(tf(msg, &[("code", r.code.as_str())]), Some(r.code)),
                Err(e) => error.set(Some(e)),
            }
        });
    };

    // 列印、匯出 Excel（ADR 0013 SA3）：依目前生效的查詢條件
    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            if let Err(e) = api::system_list_report(
                &api::basic_code_path(kind),
                texts(kind).file,
                format,
                &conds,
                i18n::current().code(),
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
    let parent_options: Vec<(String, String)> = std::iter::once((String::new(), String::new()))
        .chain(
            parents
                .read()
                .iter()
                .flatten()
                .filter(|p| p.active || v.parent.as_deref() == Some(p.code.as_str()))
                .map(|p| {
                    let label = match &p.name {
                        Some(n) => format!("{} {n}", p.code),
                        None => p.code.clone(),
                    };
                    (p.code.clone(), label)
                }),
        )
        .collect();
    let mut qbe_fields = vec![(tx.code, "code"), (tx.name, "name")];
    if has_parent {
        qbe_fields.push((tx.parent, "parent"));
    }

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t(tx.breadcrumb)} }
                h2 { {t(tx.title)} }
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
                        // 單身式作業：新增、修改、刪除、停用都用 detail
                        detail_action: "detail",
                        insert: if perm.add_row() { Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(BasicCodeInput::default());
                            names.set(LangValues::default());
                            editor.set(Some(Editor::New));
                        }) } else { None },
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            let input = c.to_input();
                            let mut values = LangValues::default();
                            values.set(UiLang::ZhTw, "name", input.name.clone().unwrap_or_default());
                            for n in &input.names {
                                if let Some(l) = UiLang::from_code(&n.lang) {
                                    values.set(l, "name", n.name.clone());
                                }
                            }
                            names.set(values);
                            form.set(input);
                            editor.set(Some(Editor::Edit(c.code)));
                        }),
                        // 複製（ADR 0013 SA5）：新代號輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let new_code = copy_code();
                            spawn(async move {
                                match api::copy_basic_code(kind, &c.code, new_code.trim()).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("basic-code-msg-copied", &[("from", c.code.as_str()), ("to", r.code.as_str())]), Some(r.code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）
                        delete: if perm.delete_row() { Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match api::delete_basic_code(kind, &c.code, c.version).await {
                                    Ok(()) => done(tf("basic-code-msg-deleted", &[("code", c.code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }) } else { None },
                        // 確認／取消確認（ADR 0013 SA9）
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            if c.confirmed {
                                set_state(c, "unconfirm", "basic-code-msg-unconfirmed");
                            } else {
                                set_state(c, "confirm", "basic-code-msg-confirmed");
                            }
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            if c.active {
                                set_state(c, "deactivate", "basic-code-msg-deactivated");
                            } else {
                                set_state(c, "activate", "basic-code-msg-activated");
                            }
                        }),
                        invalid_action: "detail",
                        inactive,
                        invalid_label: Some(("basic-code-action-deactivate", "basic-code-action-activate")),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        // 異動記錄（ADR 0012 §8，功能 history）
                        if perm.can("history") {
                            HistoryButton { program, record: cur.as_ref().map(|c| c.code.clone()) }
                        }
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("basic-code-field-copy-as"), maxlength: max_len::CODE, value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
                        }
                    }
                }
            }

            // 上下筆：移動選取列，跨頁自動換頁（ADR 0013 SA2）
            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.code.as_str())),
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
                    h3 { {t(tx.list_title)} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t(tx.code)} }
                                th { {t(tx.name)} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else if has_parent {
                                    th { {t(tx.parent)} }
                                }
                            }
                            // 查詢列（在表格第一列輸入條件）
                            if querying() {
                                tr {
                                    for (id, field) in qbe_fields.iter().copied().take(if status_tab { 2 } else { data_cols }) {
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
                            match &*codes.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for c in list.iter().cloned() {
                                        tr {
                                            key: "{c.code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.code == c.code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = c.code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{c.code}" }
                                            td { {c.name.clone().unwrap_or_default()} }
                                            if status_tab {
                                                StatusCells { status: c.status() }
                                                ConfirmCells { confirmed: c.confirmed, by: c.confirmed_by.clone() }
                                            } else if has_parent {
                                                td {
                                                    span { class: "mono", {c.parent.clone().unwrap_or_default()} }
                                                    " "
                                                    {c.parent_name.clone().unwrap_or_default()}
                                                }
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
                        Editor::New => t(tx.dialog_new),
                        Editor::Edit(code) => tf(tx.dialog_edit, &[("code", code.as_str())]),
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
                        TextField { id: "code", label: t(tx.code), maxlength: max_len::CODE, value: v.code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().code = x }
                        if kind.multilingual() {
                            // 名稱多語言（RH4）：表單顯示介面語系，翻譯圖示開窗輸入各語系
                            MultilingualDialogField {
                                id: "name",
                                field: LangField::text("name", t(tx.name), max_len::NAME),
                                values: names,
                            }
                        } else {
                            TextField { id: "name", label: t(tx.name), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), oninput: move |x| form.write().name = opt(x) }
                        }
                        if has_parent {
                            Select {
                                id: "parent",
                                label: t(tx.parent),
                                options: parent_options.clone(),
                                value: v.parent.clone().unwrap_or_default(),
                                onchange: move |x: String| form.write().parent = opt(x),
                            }
                        }
                    }
                    p { class: "muted", {t("basic-code-hint")} }
                }
            }
        }
    }
}
