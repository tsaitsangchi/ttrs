//! p_zz 程式資料建立
//! 版面比照 tiptop/azz/4fd/p_zz.4fd：
//!   上方頁籤（基本資料 / 資料擁有者）＋ 下方頁籤（資料、權限設定 / 列印設定 / 閒置設定 / 其他設定）。
//! 4fd 為英文標籤，中文由此翻譯。規則都在後端（ttrs-app ProgramService），畫面只負責輸入與顯示。

use dioxus::prelude::*;
use ttrs_shared::{
    IdleActionDto, IdleControlDto, ProgramDto, ProgramInput, ReportExtModeDto, ReportTypeDto,
};

use crate::api;

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
    let count = programs
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map_or(0, Vec::len);

    rsx! {
        h2 { "p_zz 程式資料建立" }

        // ----- 動作列 -----
        div { class: "toolbar",
            if editing {
                button { onclick: save, "確定" }
                button {
                    class: "secondary",
                    onclick: move |_| { error.set(None); show(current()); },
                    "放棄"
                }
            } else {
                button {
                    onclick: move |_| {
                        message.set(None);
                        error.set(None);
                        form.set(ProgramInput::default());
                        mode.set(Mode::New);
                        upper_tab.set(0);
                    },
                    "新增"
                }
                button { disabled: cur.is_none(), onclick: move |_| mode.set(Mode::Edit), "修改" }
                button { class: "danger", disabled: cur.is_none(), onclick: delete, "刪除" }
                span { class: "sep" }
                input {
                    class: "short",
                    placeholder: "新程式代號",
                    disabled: cur.is_none(),
                    value: "{new_no}",
                    oninput: move |e| new_no.set(e.value()),
                }
                button { disabled: cur.is_none(), onclick: copy, "複製" }
                button { disabled: cur.is_none(), onclick: rename, "改代號" }
            }
        }

        if let Some(msg) = error() {
            p { class: "error", "{msg}" }
        }
        if let Some(msg) = message() {
            p { class: "notice", "{msg}" }
        }

        // ----- 上方頁籤 pc700 -----
        Tabs { tabs: UPPER_TABS.to_vec(), active: upper_tab }
        div { class: "tab-body",
            if upper_tab() == 0 {
                div { class: "fd-grid cols-4",
                    label { "程式代號" }
                    input {
                        value: "{f.program_no}",
                        disabled: mode() != Mode::New,
                        oninput: move |e| form.write().program_no = e.value(),
                    }
                    label { "模組代號" }
                    input {
                        value: "{f.module_code}",
                        disabled: ro,
                        oninput: move |e| form.write().module_code = e.value(),
                    }
                    label { "程式名稱" }
                    input {
                        class: "span-3",
                        value: f.name.clone().unwrap_or_default(),
                        disabled: ro,
                        oninput: move |e| form.write().name = non_empty(e.value()),
                    }
                }
            } else {
                div { class: "fd-grid cols-4",
                    label { "資料所有者" }
                    input { disabled: true, value: cur.as_ref().map(|p| p.created_by.clone()).unwrap_or_default() }
                    label { "最後修改者" }
                    input { disabled: true, value: cur.as_ref().and_then(|p| p.updated_by.clone()).unwrap_or_default() }
                    label { "所屬部門" }
                    input { disabled: true, value: cur.as_ref().map(|p| p.created_dept.clone()).unwrap_or_default() }
                    label { "建立/修改日期" }
                    input { disabled: true, value: cur.as_ref().map(|p| p.updated_on.to_string()).unwrap_or_default() }
                }
            }
        }

        // ----- 下方頁籤 pc701 -----
        Tabs { tabs: LOWER_TABS.to_vec(), active: lower_tab }
        div { class: "tab-body",
            match lower_tab() {
                0 => rsx! {
                    div { class: "fd-columns",
                        fieldset { class: "group",
                            legend { "可執行功能" }
                            textarea {
                                rows: 12,
                                disabled: ro,
                                value: f.default_actions.clone().unwrap_or_default(),
                                oninput: move |e| form.write().default_actions = non_empty(e.value()),
                            }
                        }
                        div {
                            fieldset { class: "group",
                                legend { "設定" }
                                div { class: "fd-grid",
                                    label { "程式群組" }
                                    input {
                                        value: f.program_group.clone().unwrap_or_default(),
                                        disabled: ro,
                                        oninput: move |e| form.write().program_group = non_empty(e.value()),
                                    }
                                    label { "視窗型態" }
                                    OptionSelect {
                                        options: WINDOW_STYLES.iter().map(|s| (s.to_string(), s.to_string())).collect::<Vec<_>>(),
                                        value: f.window_style.clone(),
                                        disabled: ro,
                                        onchange: move |v| form.write().window_style = v,
                                    }
                                    label { "程式類別" }
                                    OptionSelect {
                                        options: PROGRAM_TYPES.iter().map(|(c, l)| (c.to_string(), l.to_string())).collect::<Vec<_>>(),
                                        value: f.program_type.clone(),
                                        placeholder: "依代號自動帶入",
                                        disabled: ro,
                                        onchange: move |v| form.write().program_type = v,
                                    }
                                }
                            }
                            fieldset { class: "group",
                                legend { "執行設定" }
                                div { class: "fd-grid",
                                    label { "UNIX 指令" }
                                    input {
                                        class: "mono",
                                        placeholder: "標準代號自動產生",
                                        value: f.run_command.clone().unwrap_or_default(),
                                        disabled: ro,
                                        oninput: move |e| form.write().run_command = non_empty(e.value()),
                                    }
                                    label { "固定執行條件" }
                                    input {
                                        value: f.fixed_run_condition.clone().unwrap_or_default(),
                                        disabled: ro,
                                        oninput: move |e| form.write().fixed_run_condition = non_empty(e.value()),
                                    }
                                }
                            }
                            fieldset { class: "group",
                                div { class: "fd-grid",
                                    label { "設計日期" }
                                    input {
                                        r#type: "date",
                                        disabled: ro,
                                        value: f.designed_on.map(|d| d.to_string()).unwrap_or_default(),
                                        oninput: move |e| form.write().designed_on = e.value().parse().ok(),
                                    }
                                }
                            }
                            fieldset { class: "group",
                                label { class: "check",
                                    input {
                                        r#type: "checkbox",
                                        checked: f.pass_book_no,
                                        disabled: ro || !is_ledger,
                                        onchange: move |e| form.write().pass_book_no = e.checked(),
                                    }
                                    "傳遞帳別參數給程式"
                                    if !is_ledger { span { class: "muted", "（僅總帳模組）" } }
                                }
                            }
                        }
                    }
                    if let Some(p) = cur.as_ref().filter(|_| !editing) {
                        fieldset { class: "group",
                            legend { "群組權限" }
                            div { class: "form-row",
                                match &*permissions.read() {
                                    Some(Ok(list)) if list.is_empty() => rsx! { span { class: "muted", "（無）" } },
                                    Some(Ok(list)) => rsx! { span { {list.join("、")} } },
                                    Some(Err(e)) => rsx! { span { class: "error", "{e}" } },
                                    None => rsx! { span { "載入中…" } },
                                }
                            }
                            if p.module_code != "MENU" {
                                div { class: "form-row",
                                    input { class: "short", placeholder: "權限類別", value: "{grant_class}", oninput: move |e| grant_class.set(e.value()) }
                                    button { onclick: grant, "設定權限" }
                                }
                            }
                        }
                    }
                },
                1 => rsx! {
                    fieldset { class: "group",
                        div { class: "fd-grid",
                            label { "報表抬頭" }
                            input {
                                value: f.report_title.clone().unwrap_or_default(),
                                disabled: ro,
                                oninput: move |e| form.write().report_title = non_empty(e.value()),
                            }
                            label { "列印方式" }
                            input {
                                value: f.print_method.clone().unwrap_or_default(),
                                disabled: ro,
                                oninput: move |e| form.write().print_method = non_empty(e.value()),
                            }
                            label { "報表類型" }
                            EnumSelect {
                                options: labels(&ReportTypeDto::ALL, ReportTypeDto::label),
                                selected: index_of(&ReportTypeDto::ALL, f.report_type.unwrap_or_default()),
                                disabled: ro,
                                onchange: move |i: usize| form.write().report_type = Some(ReportTypeDto::ALL[i]),
                            }
                            label { "報表副檔名" }
                            EnumSelect {
                                options: vec!["固定副檔名（*.out）".to_string(), "動態副檔名（*.xxr）".to_string()],
                                selected: index_of(&ReportExtModeDto::ALL, f.report_ext_mode.unwrap_or(ReportExtModeDto::Fixed)),
                                disabled: ro,
                                onchange: move |i: usize| form.write().report_ext_mode = Some(ReportExtModeDto::ALL[i]),
                            }
                            label { "報表寬度" }
                            NumberInput { value: f.report_width, disabled: ro, onchange: move |v| form.write().report_width = v }
                            label { "已列印次數" }
                            NumberInput { value: f.current_print_count, disabled: ro, onchange: move |v| form.write().current_print_count = v }
                            label { "延後列印次數" }
                            NumberInput { value: f.deferred_print_count, disabled: ro, onchange: move |v| form.write().deferred_print_count = v }
                        }
                        label { class: "check",
                            input {
                                r#type: "checkbox",
                                checked: f.print_option,
                                disabled: ro,
                                onchange: move |e| form.write().print_option = e.checked(),
                            }
                            "列印選項"
                        }
                    }
                },
                2 => rsx! {
                    fieldset { class: "group",
                        div { class: "fd-grid",
                            label { "閒置控制" }
                            EnumSelect {
                                options: vec![
                                    "不控制".to_string(),
                                    "由程式設定閒置時間".to_string(),
                                    "依整體系統設定".to_string(),
                                ],
                                selected: index_of(&IdleControlDto::ALL, f.idle_control.unwrap_or_default()),
                                disabled: ro,
                                onchange: move |i: usize| form.write().idle_control = Some(IdleControlDto::ALL[i]),
                            }
                            label { "閒置時間（秒）" }
                            NumberInput {
                                value: f.idle_seconds,
                                disabled: ro || !by_program,
                                onchange: move |v| form.write().idle_seconds = v,
                            }
                        }
                    }
                    fieldset { class: "group",
                        legend { "閒置處理" }
                        for (i, action) in IdleActionDto::ALL.iter().copied().enumerate() {
                            label { class: "check", key: "{i}",
                                input {
                                    r#type: "radio",
                                    name: "idle_action",
                                    checked: f.idle_action == Some(action),
                                    disabled: ro || !by_program,
                                    onchange: move |_| form.write().idle_action = Some(action),
                                }
                                {idle_action_text(action)}
                            }
                        }
                    }
                },
                _ => rsx! {
                    fieldset { class: "group",
                        label { class: "check",
                            input {
                                r#type: "checkbox",
                                checked: f.log_errors,
                                disabled: ro,
                                onchange: move |e| form.write().log_errors = e.checked(),
                            }
                            "錯誤訊息寫入記錄檔"
                        }
                        label { class: "check",
                            input {
                                r#type: "checkbox",
                                checked: f.allow_key_update,
                                disabled: ro,
                                onchange: move |e| form.write().allow_key_update = e.checked(),
                            }
                            "維護作業可修改鍵值"
                        }
                        label { class: "check",
                            input {
                                r#type: "checkbox",
                                checked: f.help_enabled,
                                disabled: ro,
                                onchange: move |e| form.write().help_enabled = e.checked(),
                            }
                            "開啟說明"
                        }
                    }
                    fieldset { class: "group",
                        legend { "其他備註" }
                        textarea {
                            rows: 6,
                            disabled: ro,
                            value: f.remark.clone().unwrap_or_default(),
                            oninput: move |e| form.write().remark = non_empty(e.value()),
                        }
                    }
                },
            }
        }

        // ----- 瀏覽清單（4fd 底部 cnt 筆數） -----
        p { class: "muted", "筆數：{count}" }
        match &*programs.read() {
            None => rsx! { p { "載入中…" } },
            Some(Err(e)) => rsx! { p { class: "error", "無法載入程式：{e}" } },
            Some(Ok(list)) if list.is_empty() => rsx! { p { "尚無程式資料" } },
            Some(Ok(list)) => rsx! {
                table {
                    thead {
                        tr {
                            th { "程式代號" }
                            th { "程式名稱" }
                            th { "模組" }
                            th { "類別" }
                            th { "報表類型" }
                        }
                    }
                    tbody {
                        for p in list.iter().cloned() {
                            tr {
                                key: "{p.program_no}",
                                class: if cur.as_ref().is_some_and(|c| c.program_no == p.program_no) { "selected" },
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
                            }
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn Tabs(tabs: Vec<&'static str>, active: Signal<usize>) -> Element {
    rsx! {
        div { class: "tabs",
            for (i, t) in tabs.iter().enumerate() {
                button {
                    key: "{i}",
                    class: if active() == i { "tab active" } else { "tab" },
                    onclick: move |_| active.set(i),
                    "{t}"
                }
            }
        }
    }
}

/// 以索引選取的下拉選單
#[component]
fn EnumSelect(
    options: Vec<String>,
    selected: usize,
    disabled: bool,
    onchange: EventHandler<usize>,
) -> Element {
    rsx! {
        select {
            disabled,
            onchange: move |e| {
                if let Ok(i) = e.value().parse() {
                    onchange.call(i);
                }
            },
            for (i, label) in options.iter().enumerate() {
                option { key: "{i}", value: "{i}", selected: i == selected, "{label}" }
            }
        }
    }
}

/// 以代碼選取、可留空的下拉選單（選項為 (代碼, 顯示文字)）
#[component]
fn OptionSelect(
    options: Vec<(String, String)>,
    value: Option<String>,
    #[props(default)] placeholder: String,
    disabled: bool,
    onchange: EventHandler<Option<String>>,
) -> Element {
    rsx! {
        select {
            disabled,
            onchange: move |e| onchange.call(non_empty(e.value())),
            option { value: "", selected: value.is_none(), "{placeholder}" }
            for (code, label) in options.iter() {
                option { key: "{code}", value: "{code}", selected: value.as_deref() == Some(code.as_str()), "{label}" }
            }
        }
    }
}

#[component]
fn NumberInput(value: Option<i32>, disabled: bool, onchange: EventHandler<Option<i32>>) -> Element {
    rsx! {
        input {
            r#type: "number",
            disabled,
            value: value.map(|v| v.to_string()).unwrap_or_default(),
            oninput: move |e| onchange.call(e.value().trim().parse().ok()),
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

fn labels<T: Copy>(all: &[T], label: fn(T) -> &'static str) -> Vec<String> {
    all.iter().map(|v| label(*v).to_owned()).collect()
}

fn index_of<T: Copy + PartialEq>(all: &[T], v: T) -> usize {
    all.iter().position(|x| *x == v).unwrap_or(0)
}

fn non_empty(s: String) -> Option<String> {
    Some(s).filter(|s| !s.trim().is_empty())
}
