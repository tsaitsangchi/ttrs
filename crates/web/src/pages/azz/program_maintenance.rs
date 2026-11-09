//! Program Maintenance 程式維護（TipTop p_zz 程式資料建立）
//! 版面比照 tiptop/azz/4fd/p_zz.4fd，視覺採 Material 3（components::material）：
//!   上方頁籤 pc700（基本資料 / 資料擁有者）＋ 下方頁籤 pc701（資料、權限設定 / 列印設定 / 閒置設定 / 其他設定）
//!   ＋ 瀏覽清單（4fd 底部 cnt 筆數）。
//! 查詢比照 TipTop Q.查詢（CONSTRUCT）：在同一個表單的欄位上輸入 QBE 條件，
//! 查詢後以 第一筆 / 上一筆 / 下一筆 / 最後一筆 / 跳筆 瀏覽（p_zz.4gl :383-395）。
//! 4fd 為英文標籤，中文由此翻譯。規則都在後端（ttrs-app ProgramService），畫面只負責輸入與顯示。

use std::collections::BTreeMap;

use dioxus::prelude::*;
use ttrs_shared::{
    IdleActionDto, IdleControlDto, ProgramDto, ProgramInput, ProgramNameDto, QbeCondition,
    QueryRequest, ReportExtModeDto, ReportTypeDto,
};

use ttrs_shared::program_max_len as max_len;

use crate::api;
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Notification, NotificationKind, RadioGroup, Select,
    Tabs, TextArea, TextField, Tone, icons,
};
use crate::components::multilingual::{
    LangField, LangValues, MultilingualDialogField, MultilingualField,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::qbe::{
    HINT_DATE, HINT_FLAG, HINT_NUMBER, ListPager, PAGE_SIZE, Q, Qbe, RecordNav, conditions_of,
};
use crate::components::status::{self, HasStatus, StatusForm};
use crate::i18n::{self, UiLang, t, tf};

const LEDGER_MODULES: [&str; 4] = ["AGL", "CGL", "GGL", "CGGL"];

/// zz03 程式類別（4fd ComboBox 選項）
const PROGRAM_TYPES: [(&str, &str); 9] = [
    ("I", "program-opt-type-i"),
    ("M", "program-opt-type-m"),
    ("O", "program-opt-type-o"),
    ("P", "program-opt-type-p"),
    ("Q", "program-opt-type-q"),
    ("R", "program-opt-type-r"),
    ("S", "program-opt-type-s"),
    ("T", "program-opt-type-t"),
    ("U", "program-opt-type-u"),
];

/// zz27 視窗型態（4fd ComboBox 選項）
const WINDOW_STYLES: [&str; 13] = [
    "sm1", "sm2", "sm3", "popup", "main", "dialog", "naked", "viewer", "report", "err01", "login",
    "view", "lib",
];

const UPPER_TABS: [&str; 2] = ["program-tab-basic", status::TAB_STATUS];
const LOWER_TABS: [&str; 4] = [
    "program-tab-data",
    "program-tab-print",
    "program-tab-idle",
    "program-tab-other",
];

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    View,
    New,
    Edit,
    /// 輸入 QBE 條件
    Query,
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "program-maintenance";

#[component]
pub fn ProgramMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ProgramMaintenancePage {} }
    }
}

#[component]
fn ProgramMaintenancePage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);

    let mut mode = use_signal(|| Mode::View);
    let mut form = use_signal(ProgramInput::default);
    // 各語系的程式名稱（gaz03）與報表抬頭（gaz06），一次儲存；loaded 為載入時的內容
    let mut names = use_signal(LangValues::default);
    let mut loaded_names = use_signal(LangValues::default);
    // 目前顯示的程式（瀏覽模式）
    let mut current = use_signal(|| None::<ProgramDto>);
    let mut upper_tab = use_signal(|| 0usize);
    let lower_tab = use_signal(|| 0usize);

    // 查詢：qbe 為查詢模式中正在輸入的條件，conditions 為目前生效的條件（QBE4：預設查全部）
    let mut qbe: Qbe = use_signal(BTreeMap::new);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut list_page = use_signal(|| 1u32);
    let mut programs = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(list_page()),
            page_size: Some(PAGE_SIZE),
            // 清單中的程式名稱以目前介面語系顯示（沒有時退回繁中，L2）
            lang: Some(i18n::current().code().to_owned()),
        };
        async move { api::query_programs(&req).await }
    });
    // 目前顯示的是查詢結果的第幾筆（0 起算）；sync 表示要在該頁載入後顯示這一筆
    let mut pos = use_signal(|| None::<u64>);
    let mut sync = use_signal(|| false);

    let mut new_no = use_signal(String::new);
    let mut grant_class = use_signal(String::new);
    let mut permissions = use_resource(move || async move {
        match current() {
            Some(p) => api::program_permissions(&p.program_no).await,
            None => Ok(Vec::new()),
        }
    });

    let mut show = move |p: Option<ProgramDto>| {
        form.set(p.as_ref().map(ProgramDto::to_input).unwrap_or_default());
        let n = p.as_ref().map(name_values).unwrap_or_default();
        names.set(n.clone());
        loaded_names.set(n);
        current.set(p);
        mode.set(Mode::View);
    };
    let mut done = move |msg: String, p: Option<ProgramDto>| {
        error.set(None);
        message.set(Some(msg));
        show(p);
        programs.restart();
        permissions.restart();
    };
    // 移到查詢結果的第 i 筆（0 起算），必要時換頁
    let mut go = move |i: u64| {
        pos.set(Some(i));
        list_page.set(u32::try_from(i / u64::from(PAGE_SIZE)).unwrap_or(0) + 1);
        sync.set(true);
    };

    // 換頁或跳筆後，等該頁載入完成再顯示目標那一筆
    use_effect(move || {
        if !sync() {
            return;
        }
        let Some(i) = pos() else { return };
        let picked = match &*programs.read() {
            Some(Ok(page)) if page.page == list_page() => {
                let offset = u64::from(page.page - 1) * u64::from(page.page_size);
                Some(
                    i.checked_sub(offset)
                        .and_then(|k| page.items.get(usize::try_from(k).ok()?))
                        .cloned(),
                )
            }
            _ => None,
        };
        if let Some(p) = picked {
            sync.set(false);
            show(p);
        }
    });

    let save = move |_| {
        spawn(async move {
            let mut req = form();
            req.names = names_input(&names.read(), &loaded_names.read());
            let result = match (mode(), current()) {
                (Mode::Edit, Some(p)) => api::update_program(&p.program_no, &req).await,
                _ => api::create_program(&req).await,
            };
            match result {
                Ok(p) => done(
                    tf("program-msg-saved", &[("no", p.program_no.as_str())]),
                    Some(p),
                ),
                Err(e) => error.set(Some(e)),
            }
        });
    };

    // 執行查詢：先試查第 1 頁，條件有誤時留在查詢模式顯示錯誤
    let run_query = move |_| {
        let conds = conditions_of(&qbe.read());
        spawn(async move {
            let req = QueryRequest {
                conditions: conds.clone(),
                page: Some(1),
                page_size: Some(PAGE_SIZE),
                lang: Some(i18n::current().code().to_owned()),
            };
            match api::query_programs(&req).await {
                Ok(page) => {
                    error.set(None);
                    // 清掉舊條件的結果，避免新結果載入前誤取舊清單的第一筆
                    programs.clear();
                    conditions.set(conds);
                    list_page.set(1);
                    if page.total == 0 {
                        pos.set(None);
                        show(None);
                        message.set(Some(t("query-msg-none").to_owned()));
                    } else {
                        message.set(Some(tf(
                            "query-msg-matched",
                            &[("count", page.total.to_string().as_str())],
                        )));
                        mode.set(Mode::View);
                        go(0);
                    }
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let delete = move |_| {
        let Some(p) = current() else { return };
        spawn(async move {
            match api::delete_program(&p.program_no, p.version).await {
                Ok(()) => {
                    pos.set(None);
                    done(
                        tf("program-msg-deleted", &[("no", p.program_no.as_str())]),
                        None,
                    );
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let copy = move |_| {
        let Some(p) = current() else { return };
        spawn(async move {
            match api::copy_program(&p.program_no, &new_no()).await {
                Ok(n) => {
                    new_no.set(String::new());
                    done(
                        tf(
                            "program-msg-copied",
                            &[
                                ("from", p.program_no.as_str()),
                                ("to", n.program_no.as_str()),
                            ],
                        ),
                        Some(n),
                    );
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let grant = move |_| {
        let Some(p) = current() else { return };
        spawn(async move {
            match api::grant_permission(&p.program_no, &grant_class()).await {
                Ok(()) => {
                    grant_class.set(String::new());
                    done(
                        tf("program-msg-granted", &[("no", p.program_no.as_str())]),
                        Some(p),
                    );
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let q = mode() == Mode::Query;
    let editing = matches!(mode(), Mode::New | Mode::Edit);
    let ro = !editing;
    let f = form();
    let by_program = f.idle_control == Some(IdleControlDto::ByProgram);
    let is_ledger = LEDGER_MODULES.contains(&f.module_code.trim().to_uppercase().as_str());
    let cur = current();
    let no_cur = cur.is_none();
    let (total, pages) = programs
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map_or((0, 1), |p| {
            (
                p.total,
                u32::try_from(p.total.div_ceil(u64::from(p.page_size)))
                    .unwrap_or(1)
                    .max(1),
            )
        });
    let filtered = !conditions.read().is_empty();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("program-breadcrumb")} }
                h2 { {t("program-title")} }
            }

            // ----- 動作列 -----
            div { class: "action-bar",
                match mode() {
                    Mode::New | Mode::Edit => rsx! {
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {t("common-action-ok")} }
                        Button {
                            icon: icons::CLOSE, tone: Tone::Neutral,
                            variant: ButtonVariant::Text,
                            onclick: move |_| {
                                error.set(None);
                                show(current());
                            },
                            {t("common-action-cancel")}
                        }
                    },
                    Mode::Query => rsx! {
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                        Button {
                            icon: icons::CLOSE, tone: Tone::Neutral,
                            variant: ButtonVariant::Text,
                            onclick: move |_| {
                                error.set(None);
                                show(current());
                            },
                            {t("common-action-cancel")}
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
                                    qbe.set(BTreeMap::new());
                                    mode.set(Mode::Query);
                                    upper_tab.set(0);
                                },
                                {t("common-action-query")}
                            }
                        }
                        if perm.can("insert") {
                            Button {
                                icon: icons::ADD, tone: Tone::Success,
                                onclick: move |_| {
                                    message.set(None);
                                    error.set(None);
                                    form.set(ProgramInput::default());
                                    names.set(LangValues::default());
                                    loaded_names.set(LangValues::default());
                                    mode.set(Mode::New);
                                    upper_tab.set(0);
                                },
                                {t("common-action-new")}
                            }
                        }
                        if perm.can("modify") {
                            Button {
                                icon: icons::EDIT, tone: Tone::Tertiary,
                                variant: ButtonVariant::Tonal,
                                disabled: no_cur,
                                onclick: move |_| mode.set(Mode::Edit),
                                {t("common-action-edit")}
                            }
                        }
                        // 異動記錄（ADR 0012 §8，功能 history）
                        if perm.can("history") {
                            HistoryButton { program: PROGRAM, record: cur.as_ref().map(|p| p.program_no.clone()) }
                        }
                        if perm.can("delete") {
                            Button {
                                icon: icons::DELETE, tone: Tone::Error,
                                variant: ButtonVariant::Error,
                                disabled: no_cur,
                                onclick: delete,
                                {t("common-action-delete")}
                            }
                        }
                        div { class: "spacer" }
                        if perm.can("reproduce") {
                            TextField {
                                id: "new-no",
                                label: t("program-field-copy-as"),
                                value: new_no(),
                                disabled: no_cur,
                                oninput: move |v| new_no.set(v),
                            }
                            Button {
                                icon: icons::COPY, tone: Tone::Tertiary,
                                variant: ButtonVariant::Outlined,
                                disabled: no_cur,
                                onclick: copy,
                                {t("common-action-copy")}
                            }
                        }
                    },
                }
            }

            if q {
                p { class: "query-hint",
                    {t("query-hint")}
                }
            }

            // ----- 筆數與上下筆（4fd 底部 cnt；:383-395 F/P/N/L/跳筆） -----
            if mode() == Mode::View && total > 0 {
                RecordNav { total, pos: pos(), ongo: go, onerror: move |e| error.set(Some(e)) }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            // ----- 上方頁籤 pc700 -----
            Card {
                Tabs { tabs: UPPER_TABS.iter().map(|s| t(s)).collect::<Vec<_>>(), active: upper_tab }
                div { class: "card-body",
                    if upper_tab() == 0 {
                        div { class: "form-grid cols-4",
                            Q { field: "program_no", label: t("program-field-program-no"), query: q, qbe,
                                TextField {
                                    id: "zz01",
                                    maxlength: Some(max_len::PROGRAM_NO),
                                    label: t("program-field-program-no"),
                                    value: f.program_no.clone(),
                                    // 修改時也可改代號（:1124 g_chkey；Q2 一律允許），存檔時連動選單與權限
                                    readonly: ro,
                                    oninput: move |v| form.write().program_no = v,
                                }
                            }
                            Q { field: "module_code", label: t("program-field-module"), query: q, qbe,
                                TextField {
                                    id: "zz011",
                                    maxlength: Some(max_len::MODULE_CODE),
                                    label: t("program-field-module"),
                                    value: f.module_code.clone(),
                                    readonly: ro,
                                    oninput: move |v| form.write().module_code = v,
                                }
                            }
                            // gaz03 程式名稱：表單顯示介面語系，翻譯圖示開窗輸入各語系（ADR 0005）；
                            // 查詢時依介面語系的名稱查
                            Q { field: "name", label: t("program-field-name"), class: "span-2", query: q, qbe,
                                MultilingualDialogField {
                                    id: "gaz03",
                                    class: "span-2",
                                    field: LangField::text("name", t("program-field-name"), max_len::NAME),
                                    values: names,
                                    readonly: ro,
                                }
                            }
                            // ADR 0008：TipTop 程式代號（zz01），只供轉檔與對照
                            Q { field: "tiptop_program_no", label: t("program-field-tiptop-no"), query: q, qbe,
                                TextField {
                                    id: "tiptop_program_no",
                                    maxlength: Some(max_len::TIPTOP_PROGRAM_NO),
                                    label: t("program-field-tiptop-no"),
                                    readonly: ro,
                                    value: f.tiptop_program_no.clone().unwrap_or_default(),
                                    oninput: move |v| form.write().tiptop_program_no = non_empty(v),
                                }
                            }
                        }
                    } else {
                        // p_zz.4fd 資料擁有者頁：zzuser、zzgrup、zzmodu、zzdate（zz_file 無資料有效碼與建立日期；ADR 0012 §9）
                        StatusForm { status: cur.as_ref().map(|p| p.status()), fields: status::PROGRAM, queryable: status::PROGRAM, query: q, qbe, prefix: "zz" }
                    }
                }
            }

            // ----- 下方頁籤 pc701 -----
            Card {
                Tabs { tabs: LOWER_TABS.iter().map(|s| t(s)).collect::<Vec<_>>(), active: lower_tab }
                div { class: "card-body",
                    match lower_tab() {
                        0 => rsx! {
                            div { class: "group-columns",
                                section {
                                    h3 { class: "form-section-title", {t("program-section-actions")} }
                                    // zz04 不可查詢（:292）
                                    TextArea {
                                        id: "zz04",
                                        maxlength: Some(max_len::DEFAULT_ACTIONS),
                                        label: if q { {t("program-field-action-codes-noquery")} } else { {t("program-field-action-codes")} },
                                        rows: 12,
                                        readonly: ro,
                                        value: if q { String::new() } else { f.default_actions.clone().unwrap_or_default() },
                                        oninput: move |v| form.write().default_actions = non_empty(v),
                                    }
                                }
                                div {
                                    section { class: "form-section",
                                        h3 { class: "form-section-title", {t("program-section-settings")} }
                                        div { class: "form-grid",
                                            Q { field: "program_group", label: t("program-field-group"), query: q, qbe,
                                                TextField {
                                                    id: "zz10",
                                                    maxlength: Some(max_len::PROGRAM_GROUP),
                                                    label: t("program-field-group"),
                                                    readonly: ro,
                                                    value: f.program_group.clone().unwrap_or_default(),
                                                    oninput: move |v| form.write().program_group = non_empty(v),
                                                }
                                            }
                                            Q { field: "window_style", label: t("program-field-window-style"), hint: "例：sm1|main", query: q, qbe, // i18n-allow：QBE 條件值（資料）
                                                Select {
                                                    id: "zz27",
                                                    label: t("program-field-window-style"),
                                                    options: with_blank("", WINDOW_STYLES.iter().map(|s| (s.to_string(), s.to_string()))),
                                                    value: f.window_style.clone().unwrap_or_default(),
                                                    readonly: ro,
                                                    onchange: move |v| form.write().window_style = non_empty(v),
                                                }
                                            }
                                            Q { field: "program_type", label: t("program-field-type"), hint: "I M O P Q R S T U", query: q, qbe,
                                                Select {
                                                    id: "zz03",
                                                    label: t("program-field-type"),
                                                    options: with_blank(&t("program-hint-type-menu"), PROGRAM_TYPES.iter().map(|(c, l)| (c.to_string(), t(l).to_string()))),
                                                    value: f.program_type.clone().unwrap_or_default(),
                                                    readonly: ro,
                                                    onchange: move |v| form.write().program_type = non_empty(v),
                                                }
                                            }
                                            Q { field: "designed_on", label: t("program-field-designed-on"), hint: HINT_DATE, query: q, qbe,
                                                TextField {
                                                    id: "zz09",
                                                    label: t("program-field-designed-on"),
                                                    input_type: "date",
                                                    readonly: ro,
                                                    value: f.designed_on.map(|d| d.to_string()).unwrap_or_default(),
                                                    oninput: move |v: String| form.write().designed_on = v.parse().ok(),
                                                }
                                            }
                                        }
                                    }
                                    section { class: "form-section",
                                        h3 { class: "form-section-title", {t("program-section-execution")} }
                                        div { class: "form-grid",
                                            Q { field: "run_command", label: t("program-field-run-command"), class: "span-all", query: q, qbe,
                                                TextField {
                                                    id: "zz08",
                                                    maxlength: Some(max_len::RUN_COMMAND),
                                                    class: "span-all mono",
                                                    label: t("program-field-run-command"),
                                                    placeholder: t("program-hint-run-command"),
                                                    readonly: ro,
                                                    value: f.run_command.clone().unwrap_or_default(),
                                                    oninput: move |v| form.write().run_command = non_empty(v),
                                                }
                                            }
                                            Q { field: "fixed_print_condition", label: t("program-field-fixed-print-condition"), class: "span-all", query: q, qbe,
                                                TextField {
                                                    id: "zz21",
                                                    maxlength: Some(max_len::FIXED_PRINT_CONDITION),
                                                    class: "span-all",
                                                    label: t("program-field-fixed-print-condition"),
                                                    readonly: ro,
                                                    value: f.fixed_print_condition.clone().unwrap_or_default(),
                                                    oninput: move |v| form.write().fixed_print_condition = non_empty(v),
                                                }
                                            }
                                            if !q {
                                                div { class: "span-all",
                                                    Checkbox {
                                                        id: "zz15",
                                                        label: if is_ledger { {t("program-field-pass-book")} } else { {t("program-field-pass-book-gl-only")} },
                                                        checked: f.pass_book_no,
                                                        readonly: ro,
                                                        disabled: !is_ledger,
                                                        onchange: move |v| form.write().pass_book_no = v,
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(p) = cur.as_ref().filter(|_| mode() == Mode::View) {
                                section { class: "form-section",
                                    h3 { class: "form-section-title", {t("program-section-permissions")} }
                                    div { class: "inline-items",
                                        match &*permissions.read() {
                                            Some(Ok(list)) if list.is_empty() => rsx! { span { class: "muted", {t("program-msg-no-grant")} } },
                                            Some(Ok(list)) => rsx! {
                                                for c in list.iter() {
                                                    span { key: "{c}", class: "md-chip", "{c}" }
                                                }
                                            },
                                            Some(Err(e)) => rsx! { span { class: "muted", "{e}" } },
                                            None => rsx! { span { class: "muted", {t("common-msg-loading")} } },
                                        }
                                    }
                                    if p.module_code != "MENU" && perm.can("authorization") {
                                        div { class: "action-bar grant-bar",
                                            TextField {
                                                id: "grant-class",
                                                label: t("program-field-permission-class"),
                                                value: grant_class(),
                                                oninput: move |v| grant_class.set(v),
                                            }
                                            Button { icon: icons::VERIFIED_USER, tone: Tone::Success, variant: ButtonVariant::Tonal, onclick: grant, {t("program-action-grant")} }
                                        }
                                    }
                                }
                            }
                        },
                        1 => rsx! {
                            div { class: "form-grid cols-4",
                                Q { field: "report_type", label: t("program-field-report-type"), hint: "憑證|明細表|統計表|追蹤表|無", query: q, qbe, // i18n-allow：QBE 條件值（資料）
                                    Select {
                                        id: "zz14",
                                        label: t("program-field-report-type"),
                                        options: enum_options(&ReportTypeDto::ALL, |v| t(report_type_id(v))),
                                        value: index_of(&ReportTypeDto::ALL, f.report_type.unwrap_or_default()).to_string(),
                                        readonly: ro,
                                        onchange: move |v: String| {
                                            form.write().report_type = v.parse::<usize>().ok().map(|i| ReportTypeDto::ALL[i]);
                                        },
                                    }
                                }
                                // gaz06 報表抬頭不在 CONSTRUCT 欄位內，不可查詢
                                if !q {
                                    div { class: "span-all",
                                        MultilingualField {
                                            id: "gaz06",
                                            fields: vec![LangField::text("report_title", t("program-field-report-title"), max_len::REPORT_TITLE)],
                                            values: names,
                                            readonly: ro,
                                        }
                                    }
                                }
                                Q { field: "fixed_print_method", label: t("program-field-fixed-print-method"), query: q, qbe,
                                    TextField {
                                        id: "zz22",
                                        maxlength: Some(max_len::FIXED_PRINT_METHOD),
                                        label: t("program-field-fixed-print-method"),
                                        readonly: ro,
                                        value: f.fixed_print_method.clone().unwrap_or_default(),
                                        oninput: move |v| form.write().fixed_print_method = non_empty(v),
                                    }
                                }
                                Q { field: "report_ext_mode", label: t("program-field-report-ext"), hint: "固定|動態", query: q, qbe, // i18n-allow：QBE 條件值（資料）
                                    Select {
                                        id: "zz06",
                                        label: t("program-field-report-ext"),
                                        options: vec![
                                            ("0".to_string(), t("program-opt-ext-fixed").to_string()),
                                            ("1".to_string(), t("program-opt-ext-dynamic").to_string()),
                                        ],
                                        value: index_of(&ReportExtModeDto::ALL, f.report_ext_mode.unwrap_or(ReportExtModeDto::Fixed)).to_string(),
                                        readonly: ro,
                                        onchange: move |v: String| {
                                            form.write().report_ext_mode = v.parse::<usize>().ok().map(|i| ReportExtModeDto::ALL[i]);
                                        },
                                    }
                                }
                                Q { field: "report_width", label: t("program-field-report-width"), hint: HINT_NUMBER, query: q, qbe,
                                    NumberField { id: "zz17", label: t("program-field-report-width"), value: f.report_width, readonly: ro, onchange: move |v| form.write().report_width = v }
                                }
                                Q { field: "current_print_count", label: t("program-field-printed"), hint: HINT_NUMBER, query: q, qbe,
                                    NumberField { id: "zz16", label: t("program-field-printed"), value: f.current_print_count, readonly: ro, onchange: move |v| form.write().current_print_count = v }
                                }
                                Q { field: "deferred_print_count", label: t("program-field-deferred"), hint: HINT_NUMBER, query: q, qbe,
                                    NumberField { id: "zz07", label: t("program-field-deferred"), value: f.deferred_print_count, readonly: ro, onchange: move |v| form.write().deferred_print_count = v }
                                }
                                Q { field: "print_conditions", label: t("program-field-print-conditions"), hint: HINT_FLAG, query: q, qbe,
                                    div { class: "span-all",
                                        Checkbox {
                                            id: "zz05",
                                            label: t("program-field-print-conditions"),
                                            checked: f.print_conditions,
                                            readonly: ro,
                                            onchange: move |v| form.write().print_conditions = v,
                                        }
                                    }
                                }
                            }
                        },
                        2 => rsx! {
                            div { class: "form-grid cols-4",
                                Q { field: "idle_control", label: t("program-field-idle-control"), hint: "不控制|由程式|依系統", class: "span-2", query: q, qbe, // i18n-allow：QBE 條件值（資料）
                                    div { class: "span-2",
                                        Select {
                                            id: "zz28",
                                            label: t("program-field-idle-control"),
                                            options: vec![
                                                ("0".to_string(), t("program-opt-idle-off").to_string()),
                                                ("1".to_string(), t("program-opt-idle-program").to_string()),
                                                ("2".to_string(), t("program-opt-idle-system").to_string()),
                                            ],
                                            value: index_of(&IdleControlDto::ALL, f.idle_control.unwrap_or_default()).to_string(),
                                            readonly: ro,
                                            onchange: move |v: String| {
                                                form.write().idle_control = v.parse::<usize>().ok().map(|i| IdleControlDto::ALL[i]);
                                            },
                                        }
                                    }
                                }
                                Q { field: "idle_seconds", label: t("program-field-idle-seconds"), hint: HINT_NUMBER, query: q, qbe,
                                    NumberField {
                                        id: "zz30",
                                        label: t("program-field-idle-seconds"),
                                        value: f.idle_seconds,
                                        readonly: ro,
                                        disabled: !by_program,
                                        onchange: move |v| form.write().idle_seconds = v,
                                    }
                                }
                                Q { field: "idle_action", label: t("program-field-idle-action"), hint: "僅警告|警告後關閉|直接關閉", class: "span-all", query: q, qbe, // i18n-allow：QBE 條件值（資料）
                                    div { class: "span-all",
                                        RadioGroup {
                                            name: "zz29",
                                            legend: t("program-field-idle-action"),
                                            options: IdleActionDto::ALL.iter().enumerate().map(|(i, a)| (i.to_string(), idle_action_text(*a).to_string())).collect::<Vec<_>>(),
                                            value: f.idle_action.map(|a| index_of(&IdleActionDto::ALL, a).to_string()),
                                            readonly: ro,
                                            disabled: !by_program,
                                            onchange: move |v: String| {
                                                form.write().idle_action = v.parse::<usize>().ok().map(|i| IdleActionDto::ALL[i]);
                                            },
                                        }
                                    }
                                }
                            }
                        },
                        _ => rsx! {
                            div { class: "group-columns",
                                section {
                                    h3 { class: "form-section-title", {t("program-section-options")} }
                                    div { class: if q { "form-grid" } else { "check-list" },
                                        Q { field: "log_errors", label: t("program-field-log-errors"), hint: HINT_FLAG, query: q, qbe,
                                            Checkbox {
                                                id: "zz26",
                                                label: t("program-field-log-errors"),
                                                checked: f.log_errors,
                                                readonly: ro,
                                                onchange: move |v| form.write().log_errors = v,
                                            }
                                        }
                                        Q { field: "allow_key_update", label: t("program-field-allow-key-update"), hint: HINT_FLAG, query: q, qbe,
                                            Checkbox {
                                                id: "zz13",
                                                label: t("program-field-allow-key-update"),
                                                checked: f.allow_key_update,
                                                readonly: ro,
                                                onchange: move |v| form.write().allow_key_update = v,
                                            }
                                        }
                                        Q { field: "sql_explain", label: t("program-field-sql-explain"), hint: HINT_FLAG, query: q, qbe,
                                            Checkbox {
                                                id: "zz25",
                                                label: t("program-field-sql-explain"),
                                                checked: f.sql_explain,
                                                readonly: ro,
                                                onchange: move |v| form.write().sql_explain = v,
                                            }
                                        }
                                    }
                                }
                                section {
                                    h3 { class: "form-section-title", {t("program-section-memo")} }
                                    Q { field: "remark", label: t("program-field-remark"), query: q, qbe,
                                        TextArea {
                                            id: "zz31",
                                            maxlength: Some(max_len::REMARK),
                                            label: t("program-field-remark"),
                                            rows: 6,
                                            readonly: ro,
                                            value: f.remark.clone().unwrap_or_default(),
                                            oninput: move |v| form.write().remark = non_empty(v),
                                        }
                                    }
                                }
                            }
                        },
                    }
                }
            }

            // ----- 瀏覽清單（查詢結果，分頁） -----
            Card {
                div { class: "table-header",
                    h3 { {t("program-list-title")} }
                    p {
                        if filtered { {tf("list-count-filtered", &[("total", total.to_string().as_str())])} } else { {tf("list-count-all", &[("total", total.to_string().as_str())])} }
                        {t("list-hint-click")}
                    }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                for h in [
                                    "program-field-program-no",
                                    "program-field-name",
                                    "program-col-module",
                                    "program-col-type",
                                    "program-field-report-type",
                                    "program-field-run-command",
                                    "program-col-updated-on",
                                ]
                                .map(t)
                                {
                                    th { key: "{h}", "{h}" }
                                }
                            }
                        }
                        tbody {
                            match &*programs.read() {
                                None => rsx! { tr { td { colspan: 7, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 7, {tf("program-msg-load-failed", &[("error", e.as_str())])} } } },
                                Some(Ok(page)) if page.items.is_empty() => rsx! { tr { td { colspan: 7, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(page)) => {
                                    let offset = u64::from(page.page - 1) * u64::from(page.page_size);
                                    rsx! {
                                        for (i, p) in page.items.iter().cloned().enumerate() {
                                            tr {
                                                key: "{p.program_no}",
                                                class: "md-table__row--clickable",
                                                class: if cur.as_ref().is_some_and(|c| c.program_no == p.program_no) { "md-table__row--selected" },
                                                onclick: move |_| {
                                                    if mode() == Mode::View {
                                                        message.set(None);
                                                        error.set(None);
                                                        go(offset + i as u64);
                                                    }
                                                },
                                                td { "{p.program_no}" }
                                                td { {p.name.clone().unwrap_or_default()} }
                                                td { "{p.module_code}" }
                                                td { {p.program_type.clone().unwrap_or_default()} }
                                                td { {t(report_type_id(p.report_type))} }
                                                td { class: "mono", {p.run_command.clone().unwrap_or_default()} }
                                                td { class: "mono", {p.updated_at.map(crate::components::history::local_time).unwrap_or_else(|| p.updated_on.format("%Y/%m/%d 00:00:00").to_string())} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                ListPager { page: list_page, pages }
            }
        }
    }
}

/// 數字欄位（type=number 的 outlined text field）
#[component]
fn NumberField(
    id: String,
    label: String,
    value: Option<i32>,
    readonly: bool,
    #[props(default)] disabled: bool,
    onchange: EventHandler<Option<i32>>,
) -> Element {
    rsx! {
        TextField {
            id,
            label,
            input_type: "number",
            readonly,
            disabled,
            value: value.map(|v| v.to_string()).unwrap_or_default(),
            oninput: move |v: String| onchange.call(v.trim().parse().ok()),
        }
    }
}

fn idle_action_text(a: IdleActionDto) -> String {
    match a {
        IdleActionDto::WarnOnly => t("program-opt-idle-warn"),
        IdleActionDto::WarnThenClose => t("program-opt-idle-warn-close"),
        IdleActionDto::Close => t("program-opt-idle-close"),
    }
}

/// 列舉轉成 Select 選項，值為索引
fn enum_options<T: Copy>(all: &[T], label: impl Fn(T) -> String) -> Vec<(String, String)> {
    all.iter()
        .enumerate()
        .map(|(i, v)| (i.to_string(), label(*v)))
        .collect()
}

/// 報表類型的翻譯 ID（顯示用；QBE 條件值仍為繁中名稱或代碼）
fn report_type_id(v: ReportTypeDto) -> &'static str {
    match v {
        ReportTypeDto::Voucher => "report-type-voucher",
        ReportTypeDto::Detail => "report-type-detail",
        ReportTypeDto::Statistics => "report-type-statistics",
        ReportTypeDto::Tracking => "report-type-tracking",
        ReportTypeDto::None => "report-type-none",
    }
}

/// 在選項前加一個空白值（代表未設定）
fn with_blank(blank: &str, rest: impl Iterator<Item = (String, String)>) -> Vec<(String, String)> {
    std::iter::once((String::new(), blank.to_owned()))
        .chain(rest)
        .collect()
}

fn index_of<T: Copy + PartialEq>(all: &[T], v: T) -> usize {
    all.iter().position(|x| *x == v).unwrap_or(0)
}

fn non_empty(s: String) -> Option<String> {
    Some(s).filter(|s| !s.trim().is_empty())
}

/// 各語系名稱與報表抬頭（只取各語系自己的內容，不退回繁中，避免把繁中存成其他語系）
fn name_values(p: &ProgramDto) -> LangValues {
    LangValues::from_rows(
        &p.names,
        |n| &n.lang,
        &[
            ("name", |n| Some(n.name.clone())),
            ("report_title", |n| n.report_title.clone()),
        ],
    )
}

/// 要儲存的語系：有內容的語系，加上原本有資料、這次清空的語系（後端以空白清除）
fn names_input(values: &LangValues, loaded: &LangValues) -> Vec<ProgramNameDto> {
    UiLang::ALL
        .into_iter()
        .filter(|l| values.has(*l) || loaded.has(*l))
        .map(|l| ProgramNameDto {
            lang: l.code().to_owned(),
            name: values.opt(l, "name").unwrap_or_default(),
            report_title: values.opt(l, "report_title"),
        })
        .collect()
}
