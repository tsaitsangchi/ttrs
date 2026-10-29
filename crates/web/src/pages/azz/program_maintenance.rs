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
    IdleActionDto, IdleControlDto, ProgramDto, ProgramInput, QbeCondition, QueryRequest,
    ReportExtModeDto, ReportTypeDto,
};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, IconButton, Notification, NotificationKind, RadioGroup,
    Select, Tabs, TextArea, TextField, icons,
};

const LEDGER_MODULES: [&str; 4] = ["AGL", "CGL", "GGL", "CGGL"];

/// zz03 程式類別（4fd ComboBox 選項）
const PROGRAM_TYPES: [(&str, &str); 9] = [
    ("I", "I：輸入/建立"),
    ("M", "M：選單"),
    ("O", "O：其他"),
    ("P", "P：處理/批次"),
    ("Q", "Q：查詢"),
    ("R", "R：報表"),
    ("S", "S：系統參數"),
    ("T", "T：交易"),
    ("U", "U：更新"),
];

/// zz27 視窗型態（4fd ComboBox 選項）
const WINDOW_STYLES: [&str; 13] = [
    "sm1", "sm2", "sm3", "popup", "main", "dialog", "naked", "viewer", "report", "err01", "login",
    "view", "lib",
];

const UPPER_TABS: [&str; 2] = ["基本資料", "資料擁有者"];
const LOWER_TABS: [&str; 4] = ["資料、權限設定", "列印設定", "閒置設定", "其他設定"];

/// 清單每頁筆數（QBE7）
const PAGE_SIZE: u32 = 50;

const HINT_FLAG: &str = "Y 或 N";
const HINT_DATE: &str = "2026/10/29 或 2026/10/01:2026/10/31";
const HINT_NUMBER: &str = "例：132、100:200、>80";

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    View,
    New,
    Edit,
    /// 輸入 QBE 條件
    Query,
}

type Qbe = Signal<BTreeMap<&'static str, String>>;

#[component]
pub fn ProgramMaintenance() -> Element {
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);

    let mut mode = use_signal(|| Mode::View);
    let mut form = use_signal(ProgramInput::default);
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
        };
        async move { api::query_programs(&req).await }
    });
    // 目前顯示的是查詢結果的第幾筆（0 起算）；sync 表示要在該頁載入後顯示這一筆
    let mut pos = use_signal(|| None::<u64>);
    let mut sync = use_signal(|| false);
    let mut jump = use_signal(String::new);

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
            let req = form();
            let result = match (mode(), current()) {
                (Mode::Edit, Some(p)) => api::update_program(&p.program_no, &req).await,
                _ => api::create_program(&req).await,
            };
            match result {
                Ok(p) => done(format!("已儲存 {}", p.program_no), Some(p)),
                Err(e) => error.set(Some(e)),
            }
        });
    };

    // 執行查詢：先試查第 1 頁，條件有誤時留在查詢模式顯示錯誤
    let run_query = move |_| {
        let conds: Vec<QbeCondition> = qbe
            .read()
            .iter()
            .filter(|(_, v)| !v.trim().is_empty())
            .map(|(k, v)| QbeCondition {
                field: (*k).to_owned(),
                expr: v.clone(),
            })
            .collect();
        spawn(async move {
            let req = QueryRequest {
                conditions: conds.clone(),
                page: Some(1),
                page_size: Some(PAGE_SIZE),
            };
            match api::query_programs(&req).await {
                Ok(page) => {
                    error.set(None);
                    conditions.set(conds);
                    list_page.set(1);
                    if page.total == 0 {
                        pos.set(None);
                        show(None);
                        message.set(Some("查無符合條件的資料".to_owned()));
                    } else {
                        message.set(Some(format!("共 {} 筆符合條件", page.total)));
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
            match api::delete_program(&p.program_no).await {
                Ok(()) => {
                    pos.set(None);
                    done(
                        format!("已刪除 {}（含名稱、權限、選單）", p.program_no),
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
                        format!("已將 {} 複製為 {}", p.program_no, n.program_no),
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
                    done(format!("已為 {} 設定權限", p.program_no), Some(p));
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
                div { class: "program-id", "系統管理 / Program Maintenance" }
                h2 { "程式維護" }
            }

            // ----- 動作列 -----
            div { class: "action-bar",
                match mode() {
                    Mode::New | Mode::Edit => rsx! {
                        Button { onclick: save, "確定" }
                        Button {
                            variant: ButtonVariant::Text,
                            onclick: move |_| {
                                error.set(None);
                                show(current());
                            },
                            "放棄"
                        }
                    },
                    Mode::Query => rsx! {
                        Button { onclick: run_query, "確定" }
                        Button {
                            variant: ButtonVariant::Text,
                            onclick: move |_| {
                                error.set(None);
                                show(current());
                            },
                            "放棄"
                        }
                    },
                    Mode::View => rsx! {
                        Button {
                            variant: ButtonVariant::Tonal,
                            onclick: move |_| {
                                message.set(None);
                                error.set(None);
                                qbe.set(BTreeMap::new());
                                mode.set(Mode::Query);
                                upper_tab.set(0);
                            },
                            "查詢"
                        }
                        Button {
                            onclick: move |_| {
                                message.set(None);
                                error.set(None);
                                form.set(ProgramInput::default());
                                mode.set(Mode::New);
                                upper_tab.set(0);
                            },
                            "新增"
                        }
                        Button {
                            variant: ButtonVariant::Tonal,
                            disabled: no_cur,
                            onclick: move |_| mode.set(Mode::Edit),
                            "修改"
                        }
                        Button {
                            variant: ButtonVariant::Error,
                            disabled: no_cur,
                            onclick: delete,
                            "刪除"
                        }
                        div { class: "spacer" }
                        TextField {
                            id: "new-no",
                            label: "複製為新代號",
                            value: new_no(),
                            disabled: no_cur,
                            oninput: move |v| new_no.set(v),
                        }
                        Button {
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: copy,
                            "複製"
                        }
                    },
                }
            }

            if q {
                p { class: "query-hint",
                    "查詢模式：在欄位輸入條件後按「確定」。例：aim*（開頭）、aimi10?（單一字元）、a|b（或）、100:200（範圍）、>x、<>x、=（空值）；不輸入即查全部。"
                }
            }

            // ----- 筆數與上下筆（4fd 底部 cnt；:383-395 F/P/N/L/跳筆） -----
            if mode() == Mode::View && total > 0 {
                div { class: "record-nav",
                    IconButton {
                        label: "第一筆",
                        path: icons::FIRST_PAGE,
                        disabled: pos() == Some(0),
                        onclick: move |_| go(0),
                    }
                    IconButton {
                        label: "上一筆",
                        path: icons::CHEVRON_LEFT,
                        disabled: pos().is_none_or(|i| i == 0),
                        onclick: move |_| {
                            if let Some(i) = pos().filter(|i| *i > 0) {
                                go(i - 1);
                            }
                        },
                    }
                    span { class: "record-nav__count",
                        match pos() {
                            Some(i) => rsx! { "第 {i + 1} / {total} 筆" },
                            None => rsx! { "共 {total} 筆" },
                        }
                    }
                    IconButton {
                        label: "下一筆",
                        path: icons::CHEVRON_RIGHT,
                        disabled: pos().is_some_and(|i| i + 1 >= total),
                        onclick: move |_| go(pos().map_or(0, |i| i + 1)),
                    }
                    IconButton {
                        label: "最後一筆",
                        path: icons::LAST_PAGE,
                        disabled: pos() == Some(total - 1),
                        onclick: move |_| go(total - 1),
                    }
                    TextField {
                        id: "jump",
                        class: "record-nav__jump",
                        label: "跳至第幾筆",
                        input_type: "number",
                        value: jump(),
                        oninput: move |v| jump.set(v),
                    }
                    Button {
                        variant: ButtonVariant::Outlined,
                        onclick: move |_| {
                            match jump().trim().parse::<u64>() {
                                Ok(n) if (1..=total).contains(&n) => {
                                    jump.set(String::new());
                                    go(n - 1);
                                }
                                _ => error.set(Some(format!("請輸入 1 到 {total} 之間的筆數"))),
                            }
                        },
                        "跳至"
                    }
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            // ----- 上方頁籤 pc700 -----
            Card {
                Tabs { tabs: UPPER_TABS.to_vec(), active: upper_tab }
                div { class: "card-body",
                    if upper_tab() == 0 {
                        div { class: "form-grid cols-4",
                            Q { field: "program_no", label: "程式代號", query: q, qbe,
                                TextField {
                                    id: "zz01",
                                    label: "程式代號",
                                    value: f.program_no.clone(),
                                    // 修改時也可改代號（:1124 g_chkey；Q2 一律允許），存檔時連動選單與權限
                                    readonly: ro,
                                    oninput: move |v| form.write().program_no = v,
                                }
                            }
                            Q { field: "module_code", label: "模組代號", query: q, qbe,
                                TextField {
                                    id: "zz011",
                                    label: "模組代號",
                                    value: f.module_code.clone(),
                                    readonly: ro,
                                    oninput: move |v| form.write().module_code = v,
                                }
                            }
                            Q { field: "name", label: "程式名稱", class: "span-2", query: q, qbe,
                                TextField {
                                    id: "gaz03",
                                    class: "span-2",
                                    label: "程式名稱",
                                    value: f.name.clone().unwrap_or_default(),
                                    readonly: ro,
                                    oninput: move |v| form.write().name = non_empty(v),
                                }
                            }
                        }
                    } else {
                        div { class: "form-grid cols-4",
                            Q { field: "created_by", label: "資料所有者", query: q, qbe,
                                TextField { id: "zzuser", label: "資料所有者", readonly: true, value: cur.as_ref().map(|p| p.created_by.clone()).unwrap_or_default() }
                            }
                            Q { field: "updated_by", label: "最後修改者", query: q, qbe,
                                TextField { id: "zzmodu", label: "最後修改者", readonly: true, value: cur.as_ref().and_then(|p| p.updated_by.clone()).unwrap_or_default() }
                            }
                            Q { field: "created_dept", label: "所屬部門", query: q, qbe,
                                TextField { id: "zzgrup", label: "所屬部門", readonly: true, value: cur.as_ref().map(|p| p.created_dept.clone()).unwrap_or_default() }
                            }
                            Q { field: "updated_on", label: "建立/修改日期", hint: HINT_DATE, query: q, qbe,
                                TextField { id: "zzdate", label: "建立/修改日期", readonly: true, value: cur.as_ref().map(|p| p.updated_on.to_string()).unwrap_or_default() }
                            }
                        }
                    }
                }
            }

            // ----- 下方頁籤 pc701 -----
            Card {
                Tabs { tabs: LOWER_TABS.to_vec(), active: lower_tab }
                div { class: "card-body",
                    match lower_tab() {
                        0 => rsx! {
                            div { class: "group-columns",
                                section {
                                    h3 { class: "form-section-title", "可執行功能" }
                                    // zz04 不可查詢（:292）
                                    TextArea {
                                        id: "zz04",
                                        label: if q { "功能代碼（不可查詢）" } else { "功能代碼" },
                                        rows: 12,
                                        readonly: ro,
                                        value: if q { String::new() } else { f.default_actions.clone().unwrap_or_default() },
                                        oninput: move |v| form.write().default_actions = non_empty(v),
                                    }
                                }
                                div {
                                    section { class: "form-section",
                                        h3 { class: "form-section-title", "設定" }
                                        div { class: "form-grid",
                                            Q { field: "program_group", label: "程式群組", query: q, qbe,
                                                TextField {
                                                    id: "zz10",
                                                    label: "程式群組",
                                                    readonly: ro,
                                                    value: f.program_group.clone().unwrap_or_default(),
                                                    oninput: move |v| form.write().program_group = non_empty(v),
                                                }
                                            }
                                            Q { field: "window_style", label: "視窗型態", hint: "例：sm1|main", query: q, qbe,
                                                Select {
                                                    id: "zz27",
                                                    label: "視窗型態",
                                                    options: with_blank("", WINDOW_STYLES.iter().map(|s| (s.to_string(), s.to_string()))),
                                                    value: f.window_style.clone().unwrap_or_default(),
                                                    readonly: ro,
                                                    onchange: move |v| form.write().window_style = non_empty(v),
                                                }
                                            }
                                            Q { field: "program_type", label: "程式類別", hint: "I M O P Q R S T U", query: q, qbe,
                                                Select {
                                                    id: "zz03",
                                                    label: "程式類別",
                                                    options: with_blank("依代號自動帶入", PROGRAM_TYPES.iter().map(|(c, l)| (c.to_string(), l.to_string()))),
                                                    value: f.program_type.clone().unwrap_or_default(),
                                                    readonly: ro,
                                                    onchange: move |v| form.write().program_type = non_empty(v),
                                                }
                                            }
                                            Q { field: "designed_on", label: "設計日期", hint: HINT_DATE, query: q, qbe,
                                                TextField {
                                                    id: "zz09",
                                                    label: "設計日期",
                                                    input_type: "date",
                                                    readonly: ro,
                                                    value: f.designed_on.map(|d| d.to_string()).unwrap_or_default(),
                                                    oninput: move |v: String| form.write().designed_on = v.parse().ok(),
                                                }
                                            }
                                        }
                                    }
                                    section { class: "form-section",
                                        h3 { class: "form-section-title", "執行設定" }
                                        div { class: "form-grid",
                                            Q { field: "run_command", label: "UNIX 指令", class: "span-all", query: q, qbe,
                                                TextField {
                                                    id: "zz08",
                                                    class: "span-all mono",
                                                    label: "UNIX 指令",
                                                    placeholder: "標準代號自動產生",
                                                    readonly: ro,
                                                    value: f.run_command.clone().unwrap_or_default(),
                                                    oninput: move |v| form.write().run_command = non_empty(v),
                                                }
                                            }
                                            Q { field: "fixed_run_condition", label: "固定執行條件", class: "span-all", query: q, qbe,
                                                TextField {
                                                    id: "zz21",
                                                    class: "span-all",
                                                    label: "固定執行條件",
                                                    readonly: ro,
                                                    value: f.fixed_run_condition.clone().unwrap_or_default(),
                                                    oninput: move |v| form.write().fixed_run_condition = non_empty(v),
                                                }
                                            }
                                            if !q {
                                                div { class: "span-all",
                                                    Checkbox {
                                                        id: "zz15",
                                                        label: if is_ledger { "傳遞帳別參數給程式" } else { "傳遞帳別參數給程式（僅總帳模組）" },
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
                                    h3 { class: "form-section-title", "群組權限" }
                                    div { class: "inline-items",
                                        match &*permissions.read() {
                                            Some(Ok(list)) if list.is_empty() => rsx! { span { class: "muted", "尚未授權任何權限類別" } },
                                            Some(Ok(list)) => rsx! {
                                                for c in list.iter() {
                                                    span { key: "{c}", class: "md-chip", "{c}" }
                                                }
                                            },
                                            Some(Err(e)) => rsx! { span { class: "muted", "{e}" } },
                                            None => rsx! { span { class: "muted", "載入中…" } },
                                        }
                                    }
                                    if p.module_code != "MENU" {
                                        div { class: "action-bar grant-bar",
                                            TextField {
                                                id: "grant-class",
                                                label: "權限類別",
                                                value: grant_class(),
                                                oninput: move |v| grant_class.set(v),
                                            }
                                            Button { variant: ButtonVariant::Tonal, onclick: grant, "設定權限" }
                                        }
                                    }
                                }
                            }
                        },
                        1 => rsx! {
                            div { class: "form-grid cols-4",
                                Q { field: "report_type", label: "報表類型", hint: "憑證|明細表|統計表|追蹤表|無", query: q, qbe,
                                    Select {
                                        id: "zz14",
                                        label: "報表類型",
                                        options: enum_options(&ReportTypeDto::ALL, ReportTypeDto::label),
                                        value: index_of(&ReportTypeDto::ALL, f.report_type.unwrap_or_default()).to_string(),
                                        readonly: ro,
                                        onchange: move |v: String| {
                                            form.write().report_type = v.parse::<usize>().ok().map(|i| ReportTypeDto::ALL[i]);
                                        },
                                    }
                                }
                                // gaz06 報表抬頭不在 CONSTRUCT 欄位內，不可查詢
                                if !q {
                                    TextField {
                                        id: "gaz06",
                                        label: "報表抬頭",
                                        readonly: ro,
                                        value: f.report_title.clone().unwrap_or_default(),
                                        oninput: move |v| form.write().report_title = non_empty(v),
                                    }
                                }
                                Q { field: "print_method", label: "列印方式", query: q, qbe,
                                    TextField {
                                        id: "zz22",
                                        label: "列印方式",
                                        readonly: ro,
                                        value: f.print_method.clone().unwrap_or_default(),
                                        oninput: move |v| form.write().print_method = non_empty(v),
                                    }
                                }
                                Q { field: "report_ext_mode", label: "報表副檔名", hint: "固定|動態", query: q, qbe,
                                    Select {
                                        id: "zz06",
                                        label: "報表副檔名",
                                        options: vec![
                                            ("0".to_string(), "固定副檔名（*.out）".to_string()),
                                            ("1".to_string(), "動態副檔名（*.xxr）".to_string()),
                                        ],
                                        value: index_of(&ReportExtModeDto::ALL, f.report_ext_mode.unwrap_or(ReportExtModeDto::Fixed)).to_string(),
                                        readonly: ro,
                                        onchange: move |v: String| {
                                            form.write().report_ext_mode = v.parse::<usize>().ok().map(|i| ReportExtModeDto::ALL[i]);
                                        },
                                    }
                                }
                                Q { field: "report_width", label: "報表寬度", hint: HINT_NUMBER, query: q, qbe,
                                    NumberField { id: "zz17", label: "報表寬度", value: f.report_width, readonly: ro, onchange: move |v| form.write().report_width = v }
                                }
                                Q { field: "current_print_count", label: "已列印次數", hint: HINT_NUMBER, query: q, qbe,
                                    NumberField { id: "zz16", label: "已列印次數", value: f.current_print_count, readonly: ro, onchange: move |v| form.write().current_print_count = v }
                                }
                                Q { field: "deferred_print_count", label: "延後列印次數", hint: HINT_NUMBER, query: q, qbe,
                                    NumberField { id: "zz07", label: "延後列印次數", value: f.deferred_print_count, readonly: ro, onchange: move |v| form.write().deferred_print_count = v }
                                }
                                Q { field: "print_option", label: "列印選項", hint: HINT_FLAG, query: q, qbe,
                                    div { class: "span-all",
                                        Checkbox {
                                            id: "zz05",
                                            label: "列印選項",
                                            checked: f.print_option,
                                            readonly: ro,
                                            onchange: move |v| form.write().print_option = v,
                                        }
                                    }
                                }
                            }
                        },
                        2 => rsx! {
                            div { class: "form-grid cols-4",
                                Q { field: "idle_control", label: "閒置控制", hint: "不控制|由程式|依系統", class: "span-2", query: q, qbe,
                                    div { class: "span-2",
                                        Select {
                                            id: "zz28",
                                            label: "閒置控制",
                                            options: vec![
                                                ("0".to_string(), "不控制".to_string()),
                                                ("1".to_string(), "由程式設定閒置時間".to_string()),
                                                ("2".to_string(), "依整體系統設定".to_string()),
                                            ],
                                            value: index_of(&IdleControlDto::ALL, f.idle_control.unwrap_or_default()).to_string(),
                                            readonly: ro,
                                            onchange: move |v: String| {
                                                form.write().idle_control = v.parse::<usize>().ok().map(|i| IdleControlDto::ALL[i]);
                                            },
                                        }
                                    }
                                }
                                Q { field: "idle_seconds", label: "閒置時間（秒）", hint: HINT_NUMBER, query: q, qbe,
                                    NumberField {
                                        id: "zz30",
                                        label: "閒置時間（秒）",
                                        value: f.idle_seconds,
                                        readonly: ro,
                                        disabled: !by_program,
                                        onchange: move |v| form.write().idle_seconds = v,
                                    }
                                }
                                Q { field: "idle_action", label: "閒置處理", hint: "僅警告|警告後關閉|直接關閉", class: "span-all", query: q, qbe,
                                    div { class: "span-all",
                                        RadioGroup {
                                            name: "zz29",
                                            legend: "閒置處理",
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
                                    h3 { class: "form-section-title", "執行選項" }
                                    div { class: if q { "form-grid" } else { "check-list" },
                                        Q { field: "log_errors", label: "錯誤訊息寫入記錄檔", hint: HINT_FLAG, query: q, qbe,
                                            Checkbox {
                                                id: "zz26",
                                                label: "錯誤訊息寫入記錄檔",
                                                checked: f.log_errors,
                                                readonly: ro,
                                                onchange: move |v| form.write().log_errors = v,
                                            }
                                        }
                                        Q { field: "allow_key_update", label: "維護作業可修改鍵值", hint: HINT_FLAG, query: q, qbe,
                                            Checkbox {
                                                id: "zz13",
                                                label: "維護作業可修改鍵值",
                                                checked: f.allow_key_update,
                                                readonly: ro,
                                                onchange: move |v| form.write().allow_key_update = v,
                                            }
                                        }
                                        Q { field: "help_enabled", label: "開啟說明", hint: HINT_FLAG, query: q, qbe,
                                            Checkbox {
                                                id: "zz25",
                                                label: "開啟說明",
                                                checked: f.help_enabled,
                                                readonly: ro,
                                                onchange: move |v| form.write().help_enabled = v,
                                            }
                                        }
                                    }
                                }
                                section {
                                    h3 { class: "form-section-title", "其他備註" }
                                    Q { field: "remark", label: "備註", query: q, qbe,
                                        TextArea {
                                            id: "zz31",
                                            label: "備註",
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
                    h3 { "程式清單" }
                    p {
                        if filtered { "依查詢條件共 {total} 筆" } else { "全部共 {total} 筆" }
                        "，點選一筆以檢視或修改"
                    }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                for h in ["程式代號", "程式名稱", "模組", "類別", "報表類型", "UNIX 指令", "最後修改日期"] {
                                    th { key: "{h}", "{h}" }
                                }
                            }
                        }
                        tbody {
                            match &*programs.read() {
                                None => rsx! { tr { td { colspan: 7, "載入中…" } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 7, "無法載入程式：{e}" } } },
                                Some(Ok(page)) if page.items.is_empty() => rsx! { tr { td { colspan: 7, class: "muted", "查無資料" } } },
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
                                                td { {p.report_type.label()} }
                                                td { class: "mono", {p.run_command.clone().unwrap_or_default()} }
                                                td { "{p.updated_on}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "table-footer",
                    span { "第 {list_page} / {pages} 頁" }
                    IconButton {
                        label: "上一頁",
                        path: icons::CHEVRON_LEFT,
                        disabled: list_page() <= 1,
                        onclick: move |_| list_page -= 1,
                    }
                    IconButton {
                        label: "下一頁",
                        path: icons::CHEVRON_RIGHT,
                        disabled: list_page() >= pages,
                        onclick: move |_| list_page += 1,
                    }
                }
            }
        }
    }
}

/// 查詢模式下把欄位換成 QBE 條件輸入框（文字），其餘模式照常顯示原本的欄位
#[component]
fn Q(
    field: &'static str,
    label: &'static str,
    query: bool,
    qbe: Qbe,
    #[props(default)] hint: &'static str,
    #[props(default)] class: &'static str,
    children: Element,
) -> Element {
    if !query {
        return children;
    }
    rsx! {
        TextField {
            id: "q-{field}",
            label,
            class,
            placeholder: hint,
            value: qbe.read().get(field).cloned().unwrap_or_default(),
            oninput: move |v: String| {
                qbe.write().insert(field, v);
            },
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

fn idle_action_text(a: IdleActionDto) -> &'static str {
    match a {
        IdleActionDto::WarnOnly => "僅提出警告",
        IdleActionDto::WarnThenClose => "警告後關閉程式",
        IdleActionDto::Close => "直接關閉程式",
    }
}

/// 列舉轉成 Select 選項，值為索引
fn enum_options<T: Copy>(all: &[T], label: fn(T) -> &'static str) -> Vec<(String, String)> {
    all.iter()
        .enumerate()
        .map(|(i, v)| (i.to_string(), label(*v).to_owned()))
        .collect()
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
