//! p_zz 程式資料建立
//! 版面比照 tiptop/azz/4fd/p_zz.4fd，視覺採 Material 3（components::material）：
//!   上方頁籤 pc700（基本資料 / 資料擁有者）＋ 下方頁籤 pc701（資料、權限設定 / 列印設定 / 閒置設定 / 其他設定）
//!   ＋ 瀏覽清單（4fd 底部 cnt 筆數）。
//! 4fd 為英文標籤，中文由此翻譯。規則都在後端（ttrs-app ProgramService），畫面只負責輸入與顯示。

use dioxus::prelude::*;
use ttrs_shared::{
    IdleActionDto, IdleControlDto, ProgramDto, ProgramInput, ReportExtModeDto, ReportTypeDto,
};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Notification, NotificationKind, RadioGroup, Select,
    Tabs, TextArea, TextField,
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

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    View,
    New,
    Edit,
}

#[component]
pub fn ProgramList() -> Element {
    let mut programs = use_resource(api::list_programs);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);

    let mut mode = use_signal(|| Mode::View);
    let mut form = use_signal(ProgramInput::default);
    // 目前顯示的程式（瀏覽模式）
    let mut current = use_signal(|| None::<ProgramDto>);
    let mut upper_tab = use_signal(|| 0usize);
    let lower_tab = use_signal(|| 0usize);

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

    let delete = move |_| {
        let Some(p) = current() else { return };
        spawn(async move {
            match api::delete_program(&p.program_no).await {
                Ok(()) => done(
                    format!("已刪除 {}（含名稱、權限、選單）", p.program_no),
                    None,
                ),
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let rename = move |_| {
        let Some(p) = current() else { return };
        spawn(async move {
            match api::rename_program(&p.program_no, &new_no()).await {
                Ok(n) => {
                    new_no.set(String::new());
                    done(format!("{} 已改為 {}", p.program_no, n.program_no), Some(n));
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

    let editing = mode() != Mode::View;
    let ro = !editing;
    let f = form();
    let by_program = f.idle_control == Some(IdleControlDto::ByProgram);
    let is_ledger = LEDGER_MODULES.contains(&f.module_code.trim().to_uppercase().as_str());
    let cur = current();
    let no_cur = cur.is_none();
    let count = programs
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map_or(0, Vec::len);

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", "系統管理 / p_zz" }
                h2 { "程式資料建立" }
            }

            // ----- 動作列 -----
            div { class: "action-bar",
                if editing {
                    Button { onclick: save, "確定" }
                    Button {
                        variant: ButtonVariant::Text,
                        onclick: move |_| {
                            error.set(None);
                            show(current());
                        },
                        "放棄"
                    }
                } else {
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
                    Button { variant: ButtonVariant::Tonal, disabled: no_cur, onclick: move |_| mode.set(Mode::Edit), "修改" }
                    Button { variant: ButtonVariant::Error, disabled: no_cur, onclick: delete, "刪除" }
                    div { class: "spacer" }
                    TextField {
                        id: "new-no",
                        label: "新程式代號",
                        value: new_no(),
                        disabled: no_cur,
                        oninput: move |v| new_no.set(v),
                    }
                    Button { variant: ButtonVariant::Outlined, disabled: no_cur, onclick: copy, "複製" }
                    Button { variant: ButtonVariant::Outlined, disabled: no_cur, onclick: rename, "改代號" }
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
                        TextField {
                            id: "zz01",
                            label: "程式代號",
                            value: f.program_no.clone(),
                            readonly: mode() != Mode::New,
                            oninput: move |v| form.write().program_no = v,
                        }
                        TextField {
                            id: "zz011",
                            label: "模組代號",
                            value: f.module_code.clone(),
                            readonly: ro,
                            oninput: move |v| form.write().module_code = v,
                        }
                        TextField {
                            id: "gaz03",
                            class: "span-2",
                            label: "程式名稱",
                            value: f.name.clone().unwrap_or_default(),
                            readonly: ro,
                            oninput: move |v| form.write().name = non_empty(v),
                        }
                    }
                } else {
                    div { class: "form-grid cols-4",
                        TextField { id: "zzuser", label: "資料所有者", readonly: true, value: cur.as_ref().map(|p| p.created_by.clone()).unwrap_or_default() }
                        TextField { id: "zzmodu", label: "最後修改者", readonly: true, value: cur.as_ref().and_then(|p| p.updated_by.clone()).unwrap_or_default() }
                        TextField { id: "zzgrup", label: "所屬部門", readonly: true, value: cur.as_ref().map(|p| p.created_dept.clone()).unwrap_or_default() }
                        TextField { id: "zzdate", label: "建立/修改日期", readonly: true, value: cur.as_ref().map(|p| p.updated_on.to_string()).unwrap_or_default() }
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
                                TextArea {
                                    id: "zz04",
                                    label: "功能代碼",
                                    rows: 12,
                                    readonly: ro,
                                    value: f.default_actions.clone().unwrap_or_default(),
                                    oninput: move |v| form.write().default_actions = non_empty(v),
                                }
                            }
                            div {
                                section { class: "form-section",
                                    h3 { class: "form-section-title", "設定" }
                                    div { class: "form-grid",
                                        TextField {
                                            id: "zz10",
                                            label: "程式群組",
                                            readonly: ro,
                                            value: f.program_group.clone().unwrap_or_default(),
                                            oninput: move |v| form.write().program_group = non_empty(v),
                                        }
                                        Select {
                                            id: "zz27",
                                            label: "視窗型態",
                                            options: with_blank("", WINDOW_STYLES.iter().map(|s| (s.to_string(), s.to_string()))),
                                            value: f.window_style.clone().unwrap_or_default(),
                                            readonly: ro,
                                            onchange: move |v| form.write().window_style = non_empty(v),
                                        }
                                        Select {
                                            id: "zz03",
                                            label: "程式類別",
                                            options: with_blank("依代號自動帶入", PROGRAM_TYPES.iter().map(|(c, l)| (c.to_string(), l.to_string()))),
                                            value: f.program_type.clone().unwrap_or_default(),
                                            readonly: ro,
                                            onchange: move |v| form.write().program_type = non_empty(v),
                                        }
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
                                section { class: "form-section",
                                    h3 { class: "form-section-title", "執行設定" }
                                    div { class: "form-grid",
                                        TextField {
                                            id: "zz08",
                                            class: "span-all mono",
                                            label: "UNIX 指令",
                                            placeholder: "標準代號自動產生",
                                            readonly: ro,
                                            value: f.run_command.clone().unwrap_or_default(),
                                            oninput: move |v| form.write().run_command = non_empty(v),
                                        }
                                        TextField {
                                            id: "zz21",
                                            class: "span-all",
                                            label: "固定執行條件",
                                            readonly: ro,
                                            value: f.fixed_run_condition.clone().unwrap_or_default(),
                                            oninput: move |v| form.write().fixed_run_condition = non_empty(v),
                                        }
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
                        if let Some(p) = cur.as_ref().filter(|_| !editing) {
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
                            TextField {
                                id: "gaz06",
                                class: "span-2",
                                label: "報表抬頭",
                                readonly: ro,
                                value: f.report_title.clone().unwrap_or_default(),
                                oninput: move |v| form.write().report_title = non_empty(v),
                            }
                            TextField {
                                id: "zz22",
                                label: "列印方式",
                                readonly: ro,
                                value: f.print_method.clone().unwrap_or_default(),
                                oninput: move |v| form.write().print_method = non_empty(v),
                            }
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
                            NumberField { id: "zz17", label: "報表寬度", value: f.report_width, readonly: ro, onchange: move |v| form.write().report_width = v }
                            NumberField { id: "zz16", label: "已列印次數", value: f.current_print_count, readonly: ro, onchange: move |v| form.write().current_print_count = v }
                            NumberField { id: "zz07", label: "延後列印次數", value: f.deferred_print_count, readonly: ro, onchange: move |v| form.write().deferred_print_count = v }
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
                    },
                    2 => rsx! {
                        div { class: "form-grid cols-4",
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
                            NumberField {
                                id: "zz30",
                                label: "閒置時間（秒）",
                                value: f.idle_seconds,
                                readonly: ro,
                                disabled: !by_program,
                                onchange: move |v| form.write().idle_seconds = v,
                            }
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
                    },
                    _ => rsx! {
                        div { class: "group-columns",
                            section {
                                h3 { class: "form-section-title", "執行選項" }
                                div { class: "check-list",
                                    Checkbox {
                                        id: "zz26",
                                        label: "錯誤訊息寫入記錄檔",
                                        checked: f.log_errors,
                                        readonly: ro,
                                        onchange: move |v| form.write().log_errors = v,
                                    }
                                    Checkbox {
                                        id: "zz13",
                                        label: "維護作業可修改鍵值",
                                        checked: f.allow_key_update,
                                        readonly: ro,
                                        onchange: move |v| form.write().allow_key_update = v,
                                    }
                                    Checkbox {
                                        id: "zz25",
                                        label: "開啟說明",
                                        checked: f.help_enabled,
                                        readonly: ro,
                                        onchange: move |v| form.write().help_enabled = v,
                                    }
                                }
                            }
                            section {
                                h3 { class: "form-section-title", "其他備註" }
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
                    },
                }
            }

            }

            // ----- 瀏覽清單（4fd 底部 cnt 筆數） -----
            Card {
                div { class: "table-header",
                    h3 { "程式清單" }
                    p { "共 {count} 筆，點選一筆以檢視或修改" }
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
                                Some(Ok(list)) if list.is_empty() => rsx! { tr { td { colspan: 7, class: "muted", "尚無程式資料" } } },
                                Some(Ok(list)) => rsx! {
                                    for p in list.iter().cloned() {
                                        tr {
                                            key: "{p.program_no}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|c| c.program_no == p.program_no) { "md-table__row--selected" },
                                            onclick: {
                                                let p = p.clone();
                                                move |_| {
                                                    if mode() == Mode::View {
                                                        message.set(None);
                                                        error.set(None);
                                                        show(Some(p.clone()));
                                                    }
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
                                },
                            }
                        }
                    }
                }
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
