//! p_zz 程式資料建立
//! 規則都在後端（ttrs-app ProgramService），畫面只負責輸入與顯示；
//! 欄位顯示與否比照 p_zz：閒置設定只在「由程式」時可輸入，傳遞帳別只在總帳模組顯示。

use dioxus::prelude::*;
use ttrs_shared::{
    IdleActionDto, IdleControlDto, ProgramDto, ProgramInput, ReportExtModeDto, ReportTypeDto,
};

use crate::api;

const LEDGER_MODULES: [&str; 4] = ["AGL", "CGL", "GGL", "CGGL"];

#[component]
pub fn ProgramList() -> Element {
    let mut programs = use_resource(api::list_programs);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);

    // 表單：editing 為 None 表示新增，Some(代號) 表示修改該筆
    let mut form = use_signal(ProgramInput::default);
    let mut editing = use_signal(|| None::<String>);
    // 選取的程式：改代號、複製、群組權限
    let mut selected = use_signal(|| None::<String>);
    let mut new_no = use_signal(String::new);
    let mut grant_class = use_signal(String::new);
    let mut permissions = use_resource(move || async move {
        match selected() {
            Some(no) => api::program_permissions(&no).await,
            None => Ok(Vec::new()),
        }
    });

    let mut done = move |msg: String| {
        error.set(None);
        message.set(Some(msg));
        programs.restart();
        permissions.restart();
    };

    let save = move |_| {
        spawn(async move {
            let req = form();
            let result = match editing() {
                Some(no) => api::update_program(&no, &req).await,
                None => api::create_program(&req).await,
            };
            match result {
                Ok(p) => {
                    form.set(ProgramInput::default());
                    editing.set(None);
                    done(format!("已儲存 {}", p.program_no));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let rename = move |_| {
        let Some(no) = selected() else { return };
        spawn(async move {
            match api::rename_program(&no, &new_no()).await {
                Ok(p) => {
                    selected.set(Some(p.program_no.clone()));
                    new_no.set(String::new());
                    done(format!("{no} 已改為 {}", p.program_no));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let copy = move |_| {
        let Some(no) = selected() else { return };
        spawn(async move {
            match api::copy_program(&no, &new_no()).await {
                Ok(p) => {
                    new_no.set(String::new());
                    done(format!("已將 {no} 複製為 {}", p.program_no));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let grant = move |_| {
        let Some(no) = selected() else { return };
        spawn(async move {
            match api::grant_permission(&no, &grant_class()).await {
                Ok(()) => {
                    grant_class.set(String::new());
                    done(format!("已為 {no} 設定權限"));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let f = form();
    let by_program = f.idle_control == Some(IdleControlDto::ByProgram);
    let is_ledger = LEDGER_MODULES.contains(&f.module_code.trim().to_uppercase().as_str());

    rsx! {
        h2 { "p_zz 程式資料建立" }

        fieldset { class: "panel",
            legend {
                match editing() {
                    Some(no) => rsx! { "修改程式 {no}" },
                    None => rsx! { "新增程式" },
                }
            }
            div { class: "form-grid",
                label { "程式代號"
                    input {
                        value: "{f.program_no}",
                        disabled: editing().is_some(),
                        oninput: move |e| form.write().program_no = e.value(),
                    }
                }
                label { "所屬模組"
                    input { value: "{f.module_code}", oninput: move |e| form.write().module_code = e.value() }
                }
                label { "程式名稱"
                    input {
                        value: f.name.clone().unwrap_or_default(),
                        oninput: move |e| form.write().name = non_empty(e.value()),
                    }
                }
                label { "程式類別"
                    input {
                        placeholder: "依代號自動帶入",
                        value: f.program_type.clone().unwrap_or_default(),
                        oninput: move |e| form.write().program_type = non_empty(e.value()),
                    }
                }
                label { "報表類型"
                    EnumSelect {
                        options: labels(&ReportTypeDto::ALL, ReportTypeDto::label),
                        selected: index_of(&ReportTypeDto::ALL, f.report_type.unwrap_or_default()),
                        onchange: move |i: usize| form.write().report_type = Some(ReportTypeDto::ALL[i]),
                    }
                }
                label { "報表寬度"
                    NumberInput { value: f.report_width, onchange: move |v| form.write().report_width = v }
                }
                label { "延後列印次數"
                    NumberInput { value: f.deferred_print_count, onchange: move |v| form.write().deferred_print_count = v }
                }
                label { "目前列印次數"
                    NumberInput { value: f.current_print_count, onchange: move |v| form.write().current_print_count = v }
                }
                label { "閒置控制"
                    EnumSelect {
                        options: labels(&IdleControlDto::ALL, IdleControlDto::label),
                        selected: index_of(&IdleControlDto::ALL, f.idle_control.unwrap_or_default()),
                        onchange: move |i: usize| form.write().idle_control = Some(IdleControlDto::ALL[i]),
                    }
                }
                if by_program {
                    label { "閒置秒數"
                        NumberInput { value: f.idle_seconds, onchange: move |v| form.write().idle_seconds = v }
                    }
                    label { "逾時處理"
                        EnumSelect {
                            options: labels(&IdleActionDto::ALL, IdleActionDto::label),
                            selected: index_of(&IdleActionDto::ALL, f.idle_action.unwrap_or(IdleActionDto::WarnOnly)),
                            onchange: move |i: usize| form.write().idle_action = Some(IdleActionDto::ALL[i]),
                        }
                    }
                }
                if is_ledger {
                    label { class: "check",
                        input {
                            r#type: "checkbox",
                            checked: f.pass_book_no,
                            onchange: move |e| form.write().pass_book_no = e.checked(),
                        }
                        "傳遞帳別"
                    }
                }
            }

            details {
                summary { "執行環境（Genero）" }
                div { class: "form-grid",
                    label { "執行指令"
                        input {
                            placeholder: "標準代號自動產生",
                            value: f.run_command.clone().unwrap_or_default(),
                            oninput: move |e| form.write().run_command = non_empty(e.value()),
                        }
                    }
                    label { "視窗型態"
                        input {
                            value: f.window_style.clone().unwrap_or_default(),
                            oninput: move |e| form.write().window_style = non_empty(e.value()),
                        }
                    }
                    label { "報表副檔名"
                        EnumSelect {
                            options: std::iter::once("（未設定）".to_string())
                                .chain(labels(&ReportExtModeDto::ALL, ReportExtModeDto::label))
                                .collect::<Vec<_>>(),
                            selected: f.report_ext_mode.map_or(0, |m| index_of(&ReportExtModeDto::ALL, m) + 1),
                            onchange: move |i: usize| {
                                form.write().report_ext_mode = i.checked_sub(1).map(|i| ReportExtModeDto::ALL[i]);
                            },
                        }
                    }
                    label { "列印設定"
                        input {
                            value: f.print_setting.clone().unwrap_or_default(),
                            oninput: move |e| form.write().print_setting = non_empty(e.value()),
                        }
                    }
                    label { "列印預設"
                        input {
                            value: f.print_default.clone().unwrap_or_default(),
                            oninput: move |e| form.write().print_default = non_empty(e.value()),
                        }
                    }
                    label { "快速執行資料"
                        input {
                            value: f.quick_run_data.clone().unwrap_or_default(),
                            oninput: move |e| form.write().quick_run_data = non_empty(e.value()),
                        }
                    }
                    label { class: "check",
                        input {
                            r#type: "checkbox",
                            checked: f.help_enabled,
                            onchange: move |e| form.write().help_enabled = e.checked(),
                        }
                        "說明開關"
                    }
                    label { class: "check",
                        input {
                            r#type: "checkbox",
                            checked: f.log_errors,
                            onchange: move |e| form.write().log_errors = e.checked(),
                        }
                        "錯誤寫入記錄"
                    }
                }
            }

            div { class: "form-row",
                button { onclick: save, if editing().is_some() { "儲存修改" } else { "新增" } }
                if editing().is_some() {
                    button {
                        class: "secondary",
                        onclick: move |_| {
                            editing.set(None);
                            form.set(ProgramInput::default());
                        },
                        "取消"
                    }
                }
            }
        }

        if let Some(msg) = error() {
            p { class: "error", "{msg}" }
        }
        if let Some(msg) = message() {
            p { class: "notice", "{msg}" }
        }

        if let Some(no) = selected() {
            fieldset { class: "panel",
                legend { "{no}" }
                div { class: "form-row",
                    input { placeholder: "新程式代號", value: "{new_no}", oninput: move |e| new_no.set(e.value()) }
                    button { onclick: rename, "改代號" }
                    button { onclick: copy, "複製" }
                }
                div { class: "form-row",
                    span { "群組權限：" }
                    match &*permissions.read() {
                        Some(Ok(list)) if list.is_empty() => rsx! { span { class: "muted", "（無）" } },
                        Some(Ok(list)) => rsx! { span { {list.join("、")} } },
                        Some(Err(e)) => rsx! { span { class: "error", "{e}" } },
                        None => rsx! { span { "載入中…" } },
                    }
                }
                div { class: "form-row",
                    input { placeholder: "權限類別", value: "{grant_class}", oninput: move |e| grant_class.set(e.value()) }
                    button { onclick: grant, "設定權限" }
                }
            }
        }

        match &*programs.read() {
            None => rsx! { p { "載入中…" } },
            Some(Err(e)) => rsx! { p { class: "error", "無法載入程式：{e}" } },
            Some(Ok(list)) if list.is_empty() => rsx! { p { "尚無程式資料" } },
            Some(Ok(list)) => rsx! {
                table {
                    thead {
                        tr {
                            th { "程式代號" }
                            th { "名稱" }
                            th { "模組" }
                            th { "類別" }
                            th { "報表類型" }
                            th { "執行指令" }
                            th { "建立者" }
                            th { "最後修改者" }
                            th { "" }
                        }
                    }
                    tbody {
                        for p in list.iter().cloned() {
                            ProgramRow {
                                key: "{p.program_no}",
                                selected: selected().as_deref() == Some(p.program_no.as_str()),
                                program: p.clone(),
                                on_select: move |no: String| {
                                    message.set(None);
                                    selected.set(Some(no));
                                },
                                on_edit: move |p: ProgramDto| {
                                    message.set(None);
                                    editing.set(Some(p.program_no.clone()));
                                    form.set(p.to_input());
                                },
                                on_delete: move |no: String| {
                                    spawn(async move {
                                        match api::delete_program(&no).await {
                                            Ok(()) => {
                                                if selected().as_deref() == Some(no.as_str()) {
                                                    selected.set(None);
                                                }
                                                done(format!("已刪除 {no}（含名稱、權限、選單）"));
                                            }
                                            Err(e) => error.set(Some(e)),
                                        }
                                    });
                                },
                            }
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn ProgramRow(
    program: ProgramDto,
    selected: bool,
    on_select: EventHandler<String>,
    on_edit: EventHandler<ProgramDto>,
    on_delete: EventHandler<String>,
) -> Element {
    let p = program;
    let no = p.program_no.clone();
    rsx! {
        tr {
            class: if selected { "selected" },
            onclick: {
                let no = no.clone();
                move |_| on_select.call(no.clone())
            },
            td { "{p.program_no}" }
            td { {p.name.clone().unwrap_or_default()} }
            td { "{p.module_code}" }
            td { {p.program_type.clone().unwrap_or_default()} }
            td { {p.report_type.label()} }
            td { class: "mono", {p.run_command.clone().unwrap_or_default()} }
            td { "{p.created_by}" }
            td { {p.updated_by.clone().unwrap_or_default()} }
            td {
                button {
                    onclick: {
                        let p = p.clone();
                        move |e: MouseEvent| {
                            e.stop_propagation();
                            on_edit.call(p.clone());
                        }
                    },
                    "修改"
                }
                button {
                    class: "danger",
                    onclick: move |e: MouseEvent| {
                        e.stop_propagation();
                        on_delete.call(no.clone());
                    },
                    "刪除"
                }
            }
        }
    }
}

/// 以索引選取的下拉選單（選項標籤來自 ttrs_shared 的 `label()`）
#[component]
fn EnumSelect(options: Vec<String>, selected: usize, onchange: EventHandler<usize>) -> Element {
    rsx! {
        select {
            onchange: move |e| {
                if let Ok(i) = e.value().parse() {
                    onchange.call(i);
                }
            },
            for (i, label) in options.iter().enumerate() {
                option { value: "{i}", selected: i == selected, "{label}" }
            }
        }
    }
}

#[component]
fn NumberInput(value: Option<i32>, onchange: EventHandler<Option<i32>>) -> Element {
    rsx! {
        input {
            r#type: "number",
            value: value.map(|v| v.to_string()).unwrap_or_default(),
            oninput: move |e| onchange.call(e.value().trim().parse().ok()),
        }
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
