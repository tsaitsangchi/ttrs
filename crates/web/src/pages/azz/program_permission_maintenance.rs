//! Program Permission Maintenance 程式執行權限維護（TipTop p_zy 執行權限設定作業）
//! 版面比照 tiptop/azz/4fd/p_zy.4fd（Y12）：表頭（權限類別、說明、有效）＋類別切換
//!   ＋ 程式權限表格（程式、名稱、允許執行功能、他人資料權限、他部門資料權限、單身權限）
//!   ＋ 小視窗：新增、修改（勾選允許執行功能，s_act_define）、複製、依程式／依選單整批產生、依選單整批刪除、刪除全部。
//! 查詢比照 TipTop Q.查詢（p_zy_curs：單頭權限類別＋單身第一列 CONSTRUCT）：查詢結果為權限類別，
//! 單身只列出符合條件的程式（program_permission_maintenance_query.feature YQ1–YQ5）。
//! 規則在後端 ProgramPermissionService；權限的實際檢查見 ADR 0006。

use dioxus::prelude::*;
use ttrs_shared::{
    PermissionClassDto, ProgramDto, ProgramPermissionDto, ProgramPermissionInput, QbeCondition,
    QueryRequest,
};

use crate::api;
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Chip, Dialog, IconButton, Notification,
    NotificationKind, Select, Tabs, TextField, Tone, active_field_class, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::qbe::{Qbe, conditions_of};
use crate::components::status::{
    self, HasStatus, StatusCells, StatusForm, StatusHeaders, TAB_DATA, TAB_STATUS,
};
use crate::i18n::{self, t, tf};

/// 查詢結果的權限類別一次取回（數量有限；每頁上限同後端 500 筆）
const MAX_CLASSES: u32 = 500;

/// 選單程式的模組代號（與 ttrs_domain::program::MENU_MODULE 一致）
const MENU_MODULE: &str = "MENU";
pub(crate) const USER_LEVELS: [&str; 5] = ["0", "1", "2", "3", "4"];
pub(crate) const DEPT_LEVELS: [&str; 9] = ["0", "1", "2", "3", "4", "5", "6", "7", "8"];
pub(crate) const DETAIL_LEVELS: [&str; 4] = ["0", "1", "2", "3"];

/// 目前開啟的小視窗
#[derive(Clone, PartialEq)]
enum Open {
    Add,
    Edit(Box<ProgramPermissionDto>),
    Copy,
    ByProgram,
    ByMenu,
    DeleteByMenu,
    DeleteAll,
}

pub(crate) fn level_label(kind: &str, code: &str) -> String {
    t(&format!("perm-{kind}-{code}"))
}

pub(crate) fn level_options(kind: &str, codes: &[&str]) -> Vec<(String, String)> {
    codes
        .iter()
        .map(|c| ((*c).to_owned(), format!("{c}  {}", level_label(kind, c))))
        .collect()
}

/// 常見的 TipTop 動作代號有翻譯（Y2）；其他代號照原樣顯示
pub(crate) fn action_label(id: &str) -> String {
    let key = format!("perm-act-{id}");
    let text = t(&key);
    if text == key { id.to_owned() } else { text }
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "program-permission-maintenance";

#[component]
pub fn ProgramPermissions() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, PermissionMain { initial: None } }
    }
}

/// 由權限類別維護（p_zw「維護群組執行權限」）帶入權限類別
#[component]
pub fn ProgramPermissionsFor(class: String) -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, PermissionMain { initial: Some(class) } }
    }
}

#[component]
fn PermissionMain(initial: Option<String>) -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    // 查詢：qbe 為查詢模式正在輸入的條件，conditions 為目前生效的條件（QBE4：預設查全部）
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    // 全部權限類別（複製目標）；found 為符合查詢條件的權限類別（切換用，YQ2）
    let classes = use_resource(api::list_classes);
    let found = use_resource(move || {
        let req = class_query(conditions());
        async move { api::query_program_permissions(&req).await.map(|p| p.items) }
    });
    let mut class = use_signal(|| initial.clone());
    // 有查詢條件時單身只列出符合的程式（YQ3）
    let mut perms = use_resource(move || {
        let code = class();
        let conds = conditions();
        async move {
            match code {
                Some(c) if conds.is_empty() => api::list_class_permissions(&c).await,
                Some(c) => {
                    let req = QueryRequest {
                        conditions: conds,
                        lang: Some(i18n::current().code().to_owned()),
                        ..Default::default()
                    };
                    api::query_class_permissions(&c, &req).await
                }
                None => Ok(Vec::new()),
            }
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::NO_ACTIVE.len()
    } else {
        6
    };
    let mut open = use_signal(|| None::<Open>);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut add_program = use_signal(String::new);
    let mut form = use_signal(ProgramPermissionInput::default);
    let mut target = use_signal(String::new);
    let mut programs = use_signal(Vec::<ProgramDto>::new);
    let mut program_filter = use_signal(String::new);
    let mut ticked = use_signal(Vec::<String>::new);
    let mut menu = use_signal(String::new);
    let mut reset_existing = use_signal(|| false);

    let all_classes: Vec<PermissionClassDto> = classes
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let found_classes: Vec<PermissionClassDto> = found
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    // 未指定權限類別時停在第一個
    use_effect(move || {
        if class.peek().is_none()
            && let Some(Ok(list)) = &*found.read()
            && let Some(first) = list.first()
        {
            class.set(Some(first.class_code.clone()));
        }
    });

    let code = class().unwrap_or_default();
    let cur_class = all_classes.iter().find(|c| c.class_code == code).cloned();
    let pos = found_classes.iter().position(|c| c.class_code == code);
    let total = found_classes.len();
    let filtered = !conditions.read().is_empty();
    let rows: Vec<ProgramPermissionDto> = perms
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let cur: Option<ProgramPermissionDto> = selected()
        .and_then(|no| rows.iter().find(|r| r.program_no == no).cloned())
        .or_else(|| rows.first().cloned());
    let no_class = cur_class.is_none();
    let no_cur = cur.is_none();

    let mut close = move || {
        open.set(None);
        dialog_error.set(None);
    };
    let mut done = move |msg: String| {
        open.set(None);
        dialog_error.set(None);
        error.set(None);
        message.set(Some(msg));
        perms.restart();
    };
    // 整批作業需要程式清單（依程式／依選單）
    let load_programs = move || {
        spawn(async move {
            let req = QueryRequest {
                page_size: Some(500),
                lang: Some(i18n::current().code().to_owned()),
                ..Default::default()
            };
            match api::query_programs(&req).await {
                Ok(page) => programs.set(page.items),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };
    let mut go = move |i: usize, list: Vec<PermissionClassDto>| {
        if let Some(c) = list.get(i) {
            selected.set(None);
            message.set(None);
            error.set(None);
            class.set(Some(c.class_code.clone()));
        }
    };

    let granted: Vec<String> = rows.iter().map(|r| r.program_no.clone()).collect();
    let f = program_filter().trim().to_lowercase();
    let candidates: Vec<ProgramDto> = programs()
        .into_iter()
        .filter(|p| p.module_code != MENU_MODULE && !granted.contains(&p.program_no))
        .filter(|p| {
            f.is_empty()
                || p.program_no.to_lowercase().contains(&f)
                || p.module_code.to_lowercase().contains(&f)
                || p.name.as_deref().unwrap_or("").to_lowercase().contains(&f)
        })
        .collect();
    let menu_options: Vec<(String, String)> =
        std::iter::once((String::new(), t("perm-opt-choose")))
            .chain(
                programs()
                    .into_iter()
                    .filter(|p| p.module_code == MENU_MODULE)
                    .map(|p| {
                        let label = match &p.name {
                            Some(n) => format!("{}  {n}", p.program_no),
                            None => p.program_no.clone(),
                        };
                        (p.program_no, label)
                    }),
            )
            .collect();
    let class_options: Vec<(String, String)> = found_classes
        .iter()
        .map(|c| {
            (
                c.class_code.clone(),
                format!("{}  {}", c.class_code, c.name),
            )
        })
        .collect();
    let target_options: Vec<(String, String)> =
        std::iter::once((String::new(), t("perm-opt-choose")))
            .chain(
                all_classes
                    .iter()
                    .filter(|c| c.class_code != code && c.active)
                    .map(|c| {
                        (
                            c.class_code.clone(),
                            format!("{}  {}", c.class_code, c.name),
                        )
                    }),
            )
            .collect();

    // 執行查詢：條件有誤時留在查詢模式顯示錯誤；成功時停在第一個權限類別
    let run_query = move |_| {
        let conds = conditions_of(&qbe.read());
        let req = class_query(conds.clone());
        spawn(async move {
            match api::query_program_permissions(&req).await {
                Ok(page) => {
                    error.set(None);
                    querying.set(false);
                    selected.set(None);
                    class.set(page.items.first().map(|c| c.class_code.clone()));
                    conditions.set(conds);
                    message.set(Some(if page.total == 0 {
                        t("query-msg-none")
                    } else {
                        tf(
                            "query-msg-matched",
                            &[("count", page.total.to_string().as_str())],
                        )
                    }));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };
    let level_legend = |kind: &str, field: &str, codes: &[&str]| {
        tf(
            "perm-hint-qbe-levels",
            &[
                ("field", t(field).as_str()),
                (
                    "codes",
                    codes
                        .iter()
                        .map(|c| format!("{c} {}", level_label(kind, c)))
                        .collect::<Vec<_>>()
                        .join(" / ")
                        .as_str(),
                ),
            ],
        )
    };

    let all_nav = found_classes.clone();
    let all_prev = found_classes.clone();
    let all_next = found_classes.clone();
    let all_last = found_classes.clone();
    let cur_edit = cur.clone();
    let cur_remove = cur.clone();
    let v = form();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("perm-breadcrumb")} }
                h2 { {t("perm-title")} }
            }

            // ----- 動作列 -----
            if querying() {
                div { class: "action-bar",
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { error.set(None); querying.set(false); }, {t("common-action-cancel")} }
                }
                p { class: "query-hint", {t("query-hint")} }
                // 單頭權限類別＋單身第一列（p_zy.4gl:154）
                Card {
                    div { class: "card-body",
                        div { class: "form-grid cols-3",
                            for (field, label, hint) in [
                                ("class_code", "perm-field-class", String::new()),
                                ("program_no", "perm-field-program", String::new()),
                                ("actions", "perm-field-actions", t("perm-hint-qbe-actions")),
                                ("other_user_data", "perm-field-other-user", "0:4".to_owned()),
                                ("other_dept_data", "perm-field-other-dept", "0:8".to_owned()),
                                ("detail_rows", "perm-field-detail", "0:3".to_owned()),
                            ] {
                                TextField {
                                    key: "{field}",
                                    id: "q-{field}",
                                    label: t(label),
                                    placeholder: hint,
                                    value: qbe.read().get(field).cloned().unwrap_or_default(),
                                    oninput: move |v: String| { qbe.write().insert(field, v); },
                                }
                            }
                        }
                        // 單身的狀態欄位（ADR 0012 §9）
                        StatusForm { status: None, fields: status::NO_ACTIVE, queryable: status::NO_ACTIVE, query: true, qbe: Some(qbe), prefix: "zy" }
                        p { class: "muted", {level_legend("user", "perm-field-other-user", &USER_LEVELS)} }
                        p { class: "muted", {level_legend("dept", "perm-field-other-dept", &DEPT_LEVELS)} }
                        p { class: "muted", {level_legend("detail", "perm-field-detail", &DETAIL_LEVELS)} }
                    }
                }
            } else {
            div { class: "action-bar",
                if perm.can("query") {
                    Button {
                        icon: icons::SEARCH, tone: Tone::Primary,
                        variant: ButtonVariant::Tonal,
                        onclick: move |_| {
                            message.set(None);
                            error.set(None);
                            qbe.set(Default::default());
                            querying.set(true);
                        },
                        {t("common-action-query")}
                    }
                }
                if filtered {
                    Button {
                        icon: icons::CLEAR_ALL, tone: Tone::Neutral,
                        variant: ButtonVariant::Outlined,
                        onclick: move |_| {
                            message.set(None);
                            selected.set(None);
                            conditions.set(Vec::new());
                        },
                        {t("query-action-clear")}
                    }
                }
                if perm.add_row() {
                    Button {
                        icon: icons::ADD, tone: Tone::Success,
                        disabled: no_class,
                        onclick: move |_| {
                            message.set(None);
                            add_program.set(String::new());
                            open.set(Some(Open::Add));
                        },
                        {t("common-action-new")}
                    }
                }
                if perm.can("detail") {
                    Button {
                        icon: icons::EDIT, tone: Tone::Tertiary,
                        variant: ButtonVariant::Tonal,
                        disabled: no_cur,
                        onclick: move |_| {
                            let Some(r) = cur_edit.clone() else { return };
                            message.set(None);
                            form.set(r.to_input());
                            open.set(Some(Open::Edit(Box::new(r))));
                        },
                        {t("common-action-edit")}
                    }
                }
                if perm.delete_row() {
                    Button {
                        icon: icons::REMOVE, tone: Tone::Error,
                        variant: ButtonVariant::Error,
                        disabled: no_cur,
                        onclick: move |_| {
                            let Some(r) = cur_remove.clone() else { return };
                            spawn(async move {
                                match api::revoke_class_permission(&r.class_code, &r.program_no).await {
                                    Ok(()) => {
                                        selected.set(None);
                                        done(tf("perm-msg-removed", &[("program", r.program_no.as_str())]));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        },
                        {t("perm-action-remove")}
                    }
                }
                // 異動記錄（ADR 0012 §8，功能 history）
                if perm.can("history") {
                    HistoryButton { program: PROGRAM, record: cur_class.as_ref().map(|c| c.class_code.clone()) }
                }
                if perm.can("delete") {
                    Button { icon: icons::DELETE_SWEEP, tone: Tone::Error, variant: ButtonVariant::Outlined, disabled: no_class, onclick: move |_| { message.set(None); open.set(Some(Open::DeleteAll)); }, {t("perm-action-delete-all")} }
                }
                div { class: "spacer" }
                if perm.can("reproduce") {
                    Button { icon: icons::COPY, tone: Tone::Tertiary, variant: ButtonVariant::Outlined, disabled: no_class, onclick: move |_| { message.set(None); target.set(String::new()); open.set(Some(Open::Copy)); }, {t("common-action-copy")} }
                }
                if perm.can("batch_gen_by_prog") {
                    Button {
                        icon: icons::PLAYLIST_ADD, tone: Tone::Success,
                        variant: ButtonVariant::Outlined,
                        disabled: no_class,
                        onclick: move |_| {
                            message.set(None);
                            ticked.set(Vec::new());
                            program_filter.set(String::new());
                            load_programs();
                            open.set(Some(Open::ByProgram));
                        },
                        {t("perm-action-gen-by-program")}
                    }
                }
                if perm.can("batch_gen_by_menu") {
                    Button {
                        icon: icons::PLAYLIST_ADD, tone: Tone::Success,
                        variant: ButtonVariant::Outlined,
                        disabled: no_class,
                        onclick: move |_| {
                            message.set(None);
                            menu.set(String::new());
                            reset_existing.set(false);
                            load_programs();
                            open.set(Some(Open::ByMenu));
                        },
                        {t("perm-action-gen-by-menu")}
                    }
                }
                if perm.can("batch_del_by_menu") {
                    Button {
                        icon: icons::PLAYLIST_REMOVE, tone: Tone::Error,
                        variant: ButtonVariant::Outlined,
                        disabled: no_class,
                        onclick: move |_| {
                            message.set(None);
                            menu.set(String::new());
                            load_programs();
                            open.set(Some(Open::DeleteByMenu));
                        },
                        {t("perm-action-del-by-menu")}
                    }
                }
            }
            }

            // ----- 權限類別切換 -----
            if total > 0 && !querying() {
                div { class: "record-nav",
                    IconButton { label: t("nav-first"), path: icons::FIRST_PAGE, disabled: pos == Some(0), onclick: move |_| go(0, all_nav.clone()) }
                    IconButton { label: t("nav-previous"), path: icons::CHEVRON_LEFT, disabled: pos.is_none_or(|i| i == 0), onclick: move |_| go(pos.unwrap_or(0).saturating_sub(1), all_prev.clone()) }
                    span { class: "record-nav__count",
                        match pos {
                            Some(i) => rsx! { {tf("nav-record-position", &[("pos", (i + 1).to_string().as_str()), ("total", total.to_string().as_str())])} },
                            None => rsx! { {tf("nav-record-count", &[("total", total.to_string().as_str())])} },
                        }
                    }
                    IconButton { label: t("nav-next"), path: icons::CHEVRON_RIGHT, disabled: pos.is_some_and(|i| i + 1 >= total), onclick: move |_| go(pos.map_or(0, |i| i + 1), all_next.clone()) }
                    IconButton { label: t("nav-last"), path: icons::LAST_PAGE, disabled: pos == Some(total.saturating_sub(1)), onclick: move |_| go(total.saturating_sub(1), all_last.clone()) }
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            // ----- 表頭（p_zy.4fd group01） -----
            Card {
                div { class: "card-body",
                    div { class: "form-grid cols-4",
                        Select { id: "zy01", label: t("perm-field-class"), options: class_options, value: code.clone(), onchange: move |x: String| { selected.set(None); message.set(None); class.set(Some(x)); } }
                        TextField { id: "zw02", class: "span-2", label: t("perm-field-class-name"), readonly: true, value: cur_class.as_ref().map(|c| c.name.clone()).unwrap_or_default() }
                        TextField { id: "zwacti", label: t("perm-field-class-active"), readonly: true, class: active_field_class(cur_class.as_ref().map(|c| c.active)), value: cur_class.as_ref().map(|c| if c.active { t("perm-status-active") } else { t("perm-status-inactive") }).unwrap_or_default() }
                    }
                }
            }

            // ----- 程式權限（p_zy.4fd s_zy） -----
            Card {
                // 資料／狀態頁籤（ADR 0012 §9）
                Tabs { tabs: vec![t(TAB_DATA), t(TAB_STATUS)], active: col_tab }
                div { class: "table-header",
                    h3 { {t("perm-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", rows.len().to_string().as_str())])} {t("list-hint-click")} }
                    if filtered {
                        Chip { label: t("perm-msg-filtered") }
                    }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for id in ["perm-field-program", "perm-field-program-name", "perm-field-actions", "perm-field-other-user", "perm-field-other-dept", "perm-field-detail"].into_iter().take(if status_tab { 2 } else { 6 }) {
                                th { key: "{id}", {t(id)} }
                            }
                            if status_tab { StatusHeaders { fields: status::NO_ACTIVE } }
                        } }
                        tbody {
                            match &*perms.read() {
                                None => rsx! { tr { td { colspan: "{cols}", {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: "{cols}", "{e}" } } },
                                Some(Ok(_)) if rows.is_empty() => rsx! { tr { td { colspan: "{cols}", class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for r in rows.iter().cloned() {
                                        tr {
                                            key: "{r.program_no}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|c| c.program_no == r.program_no) { "md-table__row--selected" },
                                            onclick: {
                                                let no = r.program_no.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(no.clone())); }
                                            },
                                            td { class: "mono", "{r.program_no}" }
                                            td { {r.program_name.clone().unwrap_or_default()} }
                                            if status_tab {
                                                StatusCells { status: r.status(), fields: status::NO_ACTIVE }
                                            } else {
                                            td { {r.actions.iter().map(|a| action_label(a)).collect::<Vec<_>>().join(", ")} }
                                            td { {format!("{}  {}", r.other_user_data, level_label("user", &r.other_user_data))} }
                                            td { {format!("{}  {}", r.other_dept_data, level_label("dept", &r.other_dept_data))} }
                                            td { {format!("{}  {}", r.detail_rows, level_label("detail", &r.detail_rows))} }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }
            p { class: "note", {t("perm-note")} }

            // ----- 小視窗 -----
            match open() {
                None => rsx! {},
                Some(Open::Add) => rsx! {
                    Dialog {
                        title: tf("perm-dialog-add", &[("class", code.as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                icon: icons::CHECK, tone: Tone::Success,
                                onclick: move |_| {
                                    let class_code = class().unwrap_or_default();
                                    let req = ProgramPermissionInput { program_no: add_program(), ..Default::default() };
                                    spawn(async move {
                                        match api::grant_class_permission(&class_code, &req).await {
                                            Ok(r) => {
                                                selected.set(Some(r.program_no.clone()));
                                                done(tf("perm-msg-added", &[("program", r.program_no.as_str())]));
                                            }
                                            Err(e) => dialog_error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("common-action-ok")}
                            }
                        },
                        if let Some(msg) = dialog_error() {
                            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                        }
                        TextField { id: "zy02", label: t("perm-field-program"), value: add_program(), oninput: move |x| add_program.set(x) }
                        p { class: "muted", {t("perm-hint-add")} }
                    }
                },
                Some(Open::Edit(r)) => rsx! {
                    Dialog {
                        title: tf("perm-dialog-edit", &[("class", r.class_code.as_str()), ("program", r.program_no.as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                icon: icons::CHECK, tone: Tone::Success,
                                onclick: {
                                    let r = r.clone();
                                    move |_| {
                                        let r = r.clone();
                                        let req = ttrs_shared::ProgramPermissionInput { version: Some(r.version), ..form() };
                                        spawn(async move {
                                            match api::update_class_permission(&r.class_code, &r.program_no, &req).await {
                                                Ok(_) => done(tf("perm-msg-saved", &[("program", r.program_no.as_str())])),
                                                Err(e) => dialog_error.set(Some(e)),
                                            }
                                        });
                                    }
                                },
                                {t("common-action-ok")}
                            }
                        },
                        if let Some(msg) = dialog_error() {
                            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                        }
                        h3 { class: "dialog-section", {t("perm-field-actions")} }
                        if r.available_actions.is_empty() {
                            p { class: "muted", {t("perm-hint-no-actions")} }
                        } else {
                            div { class: "action-checks",
                                for a in r.available_actions.iter().cloned() {
                                    Checkbox {
                                        key: "{a}",
                                        id: "act-{a}",
                                        label: format!("{}  ({a})", action_label(&a)),
                                        checked: v.actions.as_ref().is_some_and(|x| x.contains(&a)),
                                        onchange: {
                                            let a = a.clone();
                                            move |on: bool| {
                                                let mut f = form.write();
                                                let list = f.actions.get_or_insert_with(Vec::new);
                                                list.retain(|x| x != &a);
                                                if on {
                                                    list.push(a.clone());
                                                }
                                            }
                                        },
                                    }
                                }
                            }
                            div { class: "action-bar",
                                Button { icon: icons::CHECK_BOX, tone: Tone::Primary, variant: ButtonVariant::Text, onclick: { let all = r.available_actions.clone(); move |_| form.write().actions = Some(all.clone()) }, {t("perm-action-select-all")} }
                                Button { icon: icons::CHECK_BOX_BLANK, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| form.write().actions = Some(Vec::new()), {t("perm-action-select-none")} }
                            }
                        }
                        div { class: "form-grid",
                            Select { id: "zy04", label: t("perm-field-other-user"), options: level_options("user", &USER_LEVELS), value: v.other_user_data.clone().unwrap_or_default(), onchange: move |x| form.write().other_user_data = Some(x) }
                            Select { id: "zy05", label: t("perm-field-other-dept"), options: level_options("dept", &DEPT_LEVELS), value: v.other_dept_data.clone().unwrap_or_default(), onchange: move |x| form.write().other_dept_data = Some(x) }
                            Select { id: "zy07", label: t("perm-field-detail"), options: level_options("detail", &DETAIL_LEVELS), value: v.detail_rows.clone().unwrap_or_default(), onchange: move |x| form.write().detail_rows = Some(x) }
                        }
                    }
                },
                Some(Open::Copy) => rsx! {
                    Dialog {
                        title: tf("perm-dialog-copy", &[("class", code.as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                icon: icons::CHECK, tone: Tone::Success,
                                onclick: move |_| {
                                    let from = class().unwrap_or_default();
                                    let to = target();
                                    spawn(async move {
                                        match api::copy_permissions(&from, &to).await {
                                            Ok(r) => done(tf("perm-msg-copied", &[("count", r.added.to_string().as_str()), ("target", to.as_str())])),
                                            Err(e) => dialog_error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("common-action-ok")}
                            }
                        },
                        if let Some(msg) = dialog_error() {
                            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                        }
                        Select { id: "copy-target", label: t("perm-field-target-class"), options: target_options.clone(), value: target(), onchange: move |x| target.set(x) }
                        p { class: "muted", {t("perm-hint-copy")} }
                    }
                },
                Some(Open::ByProgram) => rsx! {
                    Dialog {
                        title: tf("perm-dialog-gen-by-program", &[("class", code.as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                icon: icons::CHECK, tone: Tone::Success,
                                disabled: ticked().is_empty(),
                                onclick: move |_| {
                                    let class_code = class().unwrap_or_default();
                                    let list = ticked();
                                    spawn(async move {
                                        match api::generate_permissions(&class_code, &list).await {
                                            Ok(r) => done(tf("perm-msg-generated", &[("count", r.added.to_string().as_str())])),
                                            Err(e) => dialog_error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("common-action-ok")}
                            }
                        },
                        if let Some(msg) = dialog_error() {
                            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                        }
                        TextField { id: "gen-filter", label: t("perm-field-program-filter"), value: program_filter(), oninput: move |x| program_filter.set(x) }
                        div { class: "program-picks",
                            if candidates.is_empty() {
                                p { class: "muted", {t("common-msg-no-data")} }
                            }
                            for p in candidates.iter().cloned() {
                                Checkbox {
                                    key: "{p.program_no}",
                                    id: "pick-{p.program_no}",
                                    label: format!("{}  {}  ({})", p.program_no, p.name.clone().unwrap_or_default(), p.module_code),
                                    checked: ticked().contains(&p.program_no),
                                    onchange: {
                                        let no = p.program_no.clone();
                                        move |on: bool| {
                                            let mut list = ticked.write();
                                            list.retain(|x| x != &no);
                                            if on {
                                                list.push(no.clone());
                                            }
                                        }
                                    },
                                }
                            }
                        }
                        p { class: "muted", {tf("perm-hint-picked", &[("count", ticked().len().to_string().as_str())])} }
                    }
                },
                Some(Open::ByMenu) => rsx! {
                    Dialog {
                        title: tf("perm-dialog-gen-by-menu", &[("class", code.as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                icon: icons::CHECK, tone: Tone::Success,
                                onclick: move |_| {
                                    let class_code = class().unwrap_or_default();
                                    let m = menu();
                                    let reset = reset_existing();
                                    spawn(async move {
                                        match api::generate_permissions_by_menu(&class_code, &m, reset).await {
                                            Ok(r) => done(tf("perm-msg-generated-menu", &[("added", r.added.to_string().as_str()), ("reset", r.reset.to_string().as_str())])),
                                            Err(e) => dialog_error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("common-action-ok")}
                            }
                        },
                        if let Some(msg) = dialog_error() {
                            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                        }
                        Select { id: "gen-menu", label: t("perm-field-menu"), options: menu_options.clone(), value: menu(), onchange: move |x| menu.set(x) }
                        Checkbox { id: "reset-existing", label: t("perm-field-reset-existing"), checked: reset_existing(), onchange: move |x| reset_existing.set(x) }
                        p { class: "muted", {t("perm-hint-gen-by-menu")} }
                    }
                },
                Some(Open::DeleteByMenu) => rsx! {
                    Dialog {
                        title: tf("perm-dialog-del-by-menu", &[("class", code.as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                icon: icons::DELETE, tone: Tone::Error,
                                variant: ButtonVariant::Error,
                                onclick: move |_| {
                                    let class_code = class().unwrap_or_default();
                                    let m = menu();
                                    spawn(async move {
                                        match api::delete_permissions_by_menu(&class_code, &m).await {
                                            Ok(r) => done(tf("perm-msg-removed-count", &[("count", r.removed.to_string().as_str())])),
                                            Err(e) => dialog_error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("common-action-delete")}
                            }
                        },
                        if let Some(msg) = dialog_error() {
                            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                        }
                        Select { id: "del-menu", label: t("perm-field-menu"), options: menu_options.clone(), value: menu(), onchange: move |x| menu.set(x) }
                        p { class: "muted", {t("perm-confirm-del-by-menu")} }
                    }
                },
                Some(Open::DeleteAll) => rsx! {
                    Dialog {
                        title: tf("perm-dialog-delete-all", &[("class", code.as_str())]),
                        oncancel: move |_| close(),
                        actions: rsx! {
                            Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| close(), {t("common-action-cancel")} }
                            Button {
                                icon: icons::DELETE, tone: Tone::Error,
                                variant: ButtonVariant::Error,
                                onclick: move |_| {
                                    let class_code = class().unwrap_or_default();
                                    spawn(async move {
                                        match api::revoke_all_permissions(&class_code).await {
                                            Ok(r) => {
                                                selected.set(None);
                                                done(tf("perm-msg-removed-count", &[("count", r.removed.to_string().as_str())]));
                                            }
                                            Err(e) => dialog_error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("common-action-delete")}
                            }
                        },
                        if let Some(msg) = dialog_error() {
                            Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                        }
                        p { {t("perm-confirm-delete-all")} }
                    }
                },
            }
        }
    }
}

/// 權限類別查詢（p_zy 查詢）：一次取回全部符合的權限類別
fn class_query(conditions: Vec<QbeCondition>) -> QueryRequest {
    QueryRequest {
        conditions,
        page: Some(1),
        page_size: Some(MAX_CLASSES),
        lang: None,
    }
}
