//! Carry Setting Maintenance 資料中心拋轉設定（TipTop aooi602 資料中心拋轉設定作業）
//! 版面比照 topcust/coo/4fd/aooi602.4fd（CS1）：上方拋轉設定清單（資料中心、資料類別、拋轉方式）；選定一筆後下方是拋轉營運中心；
//! 再選定一個營運中心，最下方是它的拋轉條件與預設值（TipTop 另開 aooi602_1、aooi602_2）。
//! 集團中心維護的「拋轉設定」以 `?center=` 帶入資料中心（GC7）。規則在後端 CarrySettingService。
//! 標準功能鈕（ADR 0013；CS9）：新增、修改（拋轉方式）、刪除、複製、查詢、列印、匯出 Excel、上下筆、異動記錄；失效、確認為例外。

use dioxus::prelude::*;
use ttrs_shared::{
    CarryCopyInput, CarryDefaultInput, CarrySettingDto, CarrySettingInput, CarryTargetInput,
    DataControlRuleInput, QbeCondition, QueryRequest, carry_setting_max_len as max_len,
    data_control_max_len,
};

use crate::api;
use crate::components::history::HistoryButton;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Select, Tabs, TextField,
    Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::qbe::{ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav};
use crate::components::status::{HasStatus, StatusCells, StatusHeaders, TAB_DATA, TAB_STATUS};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

/// 權限檢查用的程式代號
const PROGRAM: &str = "carry-setting-maintenance";

fn data_types() -> Vec<(String, String)> {
    (0..=8)
        .map(|i| (i.to_string(), data_type_label(&i.to_string())))
        .collect()
}

fn data_type_label(code: &str) -> String {
    t(&format!("group-center-data-type-{code}"))
}

fn modes() -> Vec<(String, String)> {
    (1..=3)
        .map(|i| (i.to_string(), mode_label(&i.to_string())))
        .collect()
}

fn mode_label(code: &str) -> String {
    t(&format!("carry-setting-mode-{code}"))
}

/// 拋轉條件可用欄位（CS6(c)）
fn condition_fields(data_type: &str) -> Vec<(String, String)> {
    let fields: &[&str] = match data_type {
        "1" => &[
            "item_no",
            "group_code",
            "material_category",
            "source_code",
            "product_class",
        ],
        _ => &[],
    };
    fields
        .iter()
        .map(|f| ((*f).to_owned(), field_label(f)))
        .collect()
}

/// 預設值可用欄位（CS7）
fn default_fields(data_type: &str) -> Vec<(String, String)> {
    let fields: &[&str] = match data_type {
        "1" => &["group_code", "material_category"],
        _ => &[],
    };
    fields
        .iter()
        .map(|f| ((*f).to_owned(), field_label(f)))
        .collect()
}

fn field_label(field: &str) -> String {
    t(&format!("data-control-field-{field}"))
}

fn conditions() -> Vec<(String, String)> {
    (1..=11)
        .map(|i| (i.to_string(), t(&format!("data-control-condition-{i}"))))
        .collect()
}

fn yes_no(v: bool) -> String {
    t(if v {
        "data-control-yes"
    } else {
        "data-control-no"
    })
}

fn yn(v: bool) -> String {
    if v { "Y" } else { "N" }.to_owned()
}

fn key_of(c: &CarrySettingDto) -> String {
    format!("{}/{}", c.center_code, c.data_type)
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String, String),
}

/// 單身小視窗：拋轉營運中心、拋轉條件、預設值（新增或修改）
#[derive(Clone, PartialEq)]
enum Part {
    Target(bool),
    Condition(bool),
    Default(bool),
}

#[component]
pub fn CarrySettingMaintenance(center: String) -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, CarryPage { center } }
    }
}

#[component]
fn CarryPage(center: String) -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    // 從集團中心維護帶入資料中心時，預設以該資料中心查詢（aooi602 g_argv1）
    let initial = center.clone();
    let mut conditions_sig = use_signal(move || {
        if initial.trim().is_empty() {
            Vec::new()
        } else {
            vec![QbeCondition {
                field: "center_code".into(),
                expr: initial.trim().to_owned(),
            }]
        }
    });
    let mut page = use_signal(|| 1u32);
    let mut settings = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions_sig(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move { api::query_carry_settings(&req).await }
    });
    let plants = use_resource(|| async { api::list_plants().await });
    let selected = use_signal(|| None::<String>);
    let nav = use_row_nav(page, selected, move || {
        settings
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(key_of).collect())
            .unwrap_or_default()
    });
    let mut selected = selected;
    let mut selected_plant = use_signal(|| None::<String>);
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(CarrySettingInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut copy_form = use_signal(CarryCopyInput::default);
    let mut part = use_signal(|| None::<Part>);
    let mut target_form = use_signal(CarryTargetInput::default);
    let mut rule_form = use_signal(DataControlRuleInput::default);
    let mut default_form = use_signal(CarryDefaultInput::default);
    let mut part_error = use_signal(|| None::<String>);

    let page_data = settings
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let list: Vec<CarrySettingDto> = page_data
        .as_ref()
        .map(|p| p.items.clone())
        .unwrap_or_default();
    let total = page_data.as_ref().map_or(0, |p| p.total);
    let pages = page_data.as_ref().map_or(1, |p| {
        u32::try_from(p.total.div_ceil(u64::from(p.page_size.max(1))))
            .unwrap_or(1)
            .max(1)
    });
    let filtered = !conditions_sig.read().is_empty();
    let cur: Option<CarrySettingDto> = selected()
        .and_then(|k| list.iter().find(|x| key_of(x) == k).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let keys: Vec<String> = list.iter().map(key_of).collect();
    let cur_target = cur.as_ref().and_then(|c| {
        selected_plant()
            .and_then(|p| c.targets.iter().find(|x| x.plant == p).cloned())
            .or_else(|| c.targets.first().cloned())
    });
    let basic = cur.as_ref().is_some_and(|c| c.data_type == "0");
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let plant_options: Option<Vec<(String, String)>> = plants
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|ps| {
            std::iter::once((String::new(), String::new()))
                .chain(
                    ps.iter()
                        .map(|p| (p.plant_id.clone(), format!("{} {}", p.plant_id, p.name))),
                )
                .collect()
        });

    let mut done = move |msg: String, key: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        part.set(None);
        part_error.set(None);
        selected.set(key);
        settings.restart();
    };

    let run_query = move |_| {
        let conds = conditions_of(&qbe.read());
        let req = QueryRequest {
            conditions: conds.clone(),
            page: Some(1),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        spawn(async move {
            match api::query_carry_settings(&req).await {
                Ok(result) => {
                    error.set(None);
                    querying.set(false);
                    selected.set(None);
                    conditions_sig.set(conds);
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
        let req = form();
        spawn(async move {
            let result = match target {
                Some(Editor::Edit(c, d)) => api::update_carry_setting(&c, &d, &req).await,
                _ => api::create_carry_setting(&req).await,
            };
            match result {
                Ok(r) => done(
                    tf(
                        "carry-setting-msg-saved",
                        &[("center", r.center_code.as_str())],
                    ),
                    Some(key_of(&r)),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let part_cur = cur.clone();
    let part_plant = cur_target.as_ref().map(|g| g.plant.clone());
    let save_part = move |_| {
        let (Some(p), Some(c)) = (part(), part_cur.clone()) else {
            return;
        };
        let plant = part_plant.clone().unwrap_or_default();
        let (tf_, rf, df) = (target_form(), rule_form(), default_form());
        spawn(async move {
            let r = match p {
                Part::Target(edit) => {
                    api::save_carry_target(&c.center_code, &c.data_type, edit, &tf_).await
                }
                Part::Condition(edit) => {
                    api::save_carry_condition(&c.center_code, &c.data_type, &plant, edit, &rf).await
                }
                Part::Default(edit) => {
                    api::save_carry_default(&c.center_code, &c.data_type, &plant, edit, &df).await
                }
            };
            match r {
                Ok(r) => {
                    if let Part::Target(false) = p {
                        selected_plant.set(Some(tf_.plant.trim().to_uppercase()));
                    }
                    done(
                        tf(
                            "carry-setting-msg-saved",
                            &[("center", r.center_code.as_str())],
                        ),
                        Some(key_of(&r)),
                    );
                }
                Err(e) => part_error.set(Some(e)),
            }
        });
    };

    let report = move |format: api::ReportFormat| {
        let conds = conditions_sig.read().clone();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            if let Err(e) = api::system_list_report(
                "/api/carry-settings",
                "carry-settings",
                format,
                &conds,
                crate::i18n::current().code(),
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
    let cur_copy = cur.clone();
    let insert_center = center.clone();
    let remove = move |c: CarrySettingDto, suffix: String| {
        spawn(async move {
            match api::remove_carry_part(&c.center_code, &c.data_type, &suffix, c.version).await {
                Ok(r) => done(
                    tf(
                        "carry-setting-msg-saved",
                        &[("center", r.center_code.as_str())],
                    ),
                    Some(key_of(&r)),
                ),
                Err(e) => error.set(Some(e)),
            }
        });
    };
    let v = form();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("carry-setting-breadcrumb")} }
                h2 { {t("carry-setting-title")} }
            }

            if querying() {
                div { class: "action-bar",
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { error.set(None); querying.set(false); }, {t("common-action-cancel")} }
                }
                p { class: "query-hint", {t("query-hint")} }
                div { class: "form-grid",
                    for (id, field) in [("carry-setting-field-center", "center_code"), ("carry-setting-field-data-type", "data_type"), ("carry-setting-field-mode", "mode"), ("carry-setting-field-plant", "target_plant"), ("carry-setting-field-notify", "notify")] {
                        TextField {
                            key: "{field}",
                            id: "q-{field}",
                            label: t(id),
                            value: qbe.read().get(field).cloned().unwrap_or_default(),
                            oninput: move |v: String| { qbe.write().insert(field, v); },
                        }
                    }
                }
            } else {
                div { class: "action-bar",
                    MaintenanceToolbar {
                        query: Act::new(true, move |_| { message.set(None); error.set(None); qbe.set(Default::default()); querying.set(true); }),
                        insert: Act::new(true, move |_| {
                            message.set(None);
                            dialog_error.set(None);
                            form.set(CarrySettingInput { center_code: insert_center.trim().to_owned(), data_type: "1".into(), mode: "3".into(), version: None });
                            editor.set(Some(Editor::New));
                        }),
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(CarrySettingInput { center_code: c.center_code.clone(), data_type: c.data_type.clone(), mode: c.mode.clone(), version: Some(c.version) });
                            editor.set(Some(Editor::Edit(c.center_code, c.data_type)));
                        }),
                        // 複製（CS9）：連同單身複製到另一個資料中心＋資料類別
                        copy: Act::new(!no_cur && !copy_form().center_code.trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let req = copy_form();
                            spawn(async move {
                                match api::copy_carry_setting(&c.center_code, &c.data_type, &req).await {
                                    Ok(r) => {
                                        copy_form.set(CarryCopyInput::default());
                                        done(tf("carry-setting-msg-copied", &[("from", key_of(&c).as_str()), ("to", key_of(&r).as_str())]), Some(key_of(&r)));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        delete: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match api::delete_carry_setting(&c.center_code, &c.data_type, c.version).await {
                                    Ok(()) => done(tf("carry-setting-msg-deleted", &[("center", c.center_code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        if perm.can("history") {
                            HistoryButton { program: PROGRAM, record: cur.as_ref().map(|c| c.center_code.clone()) }
                        }
                        if perm.can("reproduce") {
                            TextField { id: "copy-center", label: t("carry-setting-field-copy-center"), value: copy_form().center_code, disabled: no_cur, oninput: move |v| copy_form.write().center_code = v }
                            Select { id: "copy-type", label: t("carry-setting-field-copy-type"), options: data_types(), value: copy_form().data_type, disabled: no_cur, onchange: move |v| copy_form.write().data_type = v }
                        }
                    }
                }
            }

            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(key_of).as_deref()),
                    ongo: {
                        let keys = keys.clone();
                        move |i| { message.set(None); selected_plant.set(None); nav.go(i, &keys); }
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
                Tabs { tabs: vec![t(TAB_DATA), t(TAB_STATUS)], active: col_tab }
                div { class: "table-header",
                    h3 { {t("carry-setting-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("carry-setting-field-center")} }
                                th { {t("carry-setting-field-data-type")} }
                                if status_tab {
                                    StatusHeaders {}
                                } else {
                                    th { {t("carry-setting-field-mode")} }
                                    th { {t("carry-setting-field-plant-count")} }
                                }
                            }
                        }
                        tbody {
                            match &*settings.read() {
                                None => rsx! { tr { td { colspan: 4, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 4, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: 4, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for c in list.iter().cloned() {
                                        tr {
                                            key: "{key_of(&c)}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| key_of(x) == key_of(&c)) { "md-table__row--selected" },
                                            onclick: {
                                                let k = key_of(&c);
                                                move |_| { message.set(None); error.set(None); selected_plant.set(None); selected.set(Some(k.clone())); }
                                            },
                                            td { class: "mono", {format!("{} {}", c.center_code, c.center_name.clone().unwrap_or_default())} }
                                            td { {data_type_label(&c.data_type)} }
                                            if status_tab {
                                                StatusCells { status: c.status() }
                                            } else {
                                                td { {mode_label(&c.mode)} }
                                                td { class: "num", "{c.targets.len()}" }
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

            // 拋轉營運中心（aooi602 單身）
            if let Some(c) = cur.clone() {
                Card {
                    div { class: "table-header",
                        h3 { {tf("carry-setting-targets-title", &[("center", c.center_code.as_str()), ("type", data_type_label(&c.data_type).as_str())])} }
                        if perm.can("detail") && perm.add_row() {
                            Button { icon: icons::ADD, tone: Tone::Primary, variant: ButtonVariant::Tonal, onclick: {
                                let version = c.version;
                                move |_| {
                                    part_error.set(None);
                                    target_form.set(CarryTargetInput { plant: String::new(), deletable: true, update_existing: true, notify: None, version: Some(version) });
                                    part.set(Some(Part::Target(false)));
                                }
                            }, {t("carry-setting-action-add-target")} }
                        }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("carry-setting-field-plant")} }
                                    th { {t("carry-setting-field-deletable")} }
                                    th { {t("carry-setting-field-update-existing")} }
                                    th { {t("carry-setting-field-notify")} }
                                    th {}
                                }
                            }
                            tbody {
                                if c.targets.is_empty() {
                                    tr { td { colspan: 5, class: "muted", {t("common-msg-no-data")} } }
                                }
                                for g in c.targets.iter().cloned() {
                                    tr {
                                        key: "{g.plant}",
                                        class: "md-table__row--clickable",
                                        class: if cur_target.as_ref().is_some_and(|x| x.plant == g.plant) { "md-table__row--selected" },
                                        onclick: {
                                            let p = g.plant.clone();
                                            move |_| selected_plant.set(Some(p.clone()))
                                        },
                                        td { class: "mono", "{g.plant}" }
                                        td { {yes_no(g.deletable)} }
                                        td { {yes_no(g.update_existing)} }
                                        // 使用者輸入的電子郵件不翻譯
                                        td { {g.notify.clone().unwrap_or_default()} }
                                        td {
                                            if perm.can("detail") {
                                                Button { icon: icons::EDIT, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: {
                                                    let (g, version) = (g.clone(), c.version);
                                                    move |_| {
                                                        part_error.set(None);
                                                        target_form.set(CarryTargetInput { plant: g.plant.clone(), deletable: g.deletable, update_existing: g.update_existing, notify: g.notify.clone(), version: Some(version) });
                                                        part.set(Some(Part::Target(true)));
                                                    }
                                                }, {t("common-action-edit")} }
                                            }
                                            if perm.can("detail") && perm.delete_row() {
                                                Button { icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Text, onclick: {
                                                    let (c, p) = (c.clone(), g.plant.clone());
                                                    move |_| { selected_plant.set(None); remove(c.clone(), format!("/targets/{p}")); }
                                                }, {t("common-action-delete")} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 拋轉條件與預設值（aooi602_1、aooi602_2）
            if let (Some(c), Some(g)) = (cur.clone(), cur_target.clone()) {
                if !basic {
                    Card {
                        div { class: "table-header",
                            h3 { {tf("carry-setting-conditions-title", &[("plant", g.plant.as_str())])} }
                            if perm.can("detail") && perm.add_row() {
                                Button { icon: icons::ADD, tone: Tone::Primary, variant: ButtonVariant::Tonal, disabled: condition_fields(&c.data_type).is_empty(), onclick: {
                                    let (dt, version) = (c.data_type.clone(), c.version);
                                    move |_| {
                                        part_error.set(None);
                                        rule_form.set(DataControlRuleInput { field: condition_fields(&dt).first().map(|f| f.0.clone()).unwrap_or_default(), condition: "1".into(), values: String::new(), version: Some(version) });
                                        part.set(Some(Part::Condition(false)));
                                    }
                                }, {t("carry-setting-action-add-condition")} }
                            }
                        }
                        table { class: "md-table",
                            thead { tr { th { {t("data-control-field-field")} } th { {t("data-control-field-condition")} } th { {t("data-control-field-values")} } th {} } }
                            tbody {
                                if g.conditions.is_empty() {
                                    tr { td { colspan: 4, class: "muted", {t("carry-setting-msg-all")} } }
                                }
                                for r in g.conditions.iter().cloned() {
                                    tr { key: "{r.field}",
                                        td { {field_label(&r.field)} }
                                        td { {t(&format!("data-control-condition-{}", r.condition))} }
                                        td { class: "mono", "{r.values}" }
                                        td {
                                            if perm.can("detail") {
                                                Button { icon: icons::EDIT, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: {
                                                    let (r, version) = (r.clone(), c.version);
                                                    move |_| {
                                                        part_error.set(None);
                                                        rule_form.set(DataControlRuleInput { field: r.field.clone(), condition: r.condition.clone(), values: r.values.clone(), version: Some(version) });
                                                        part.set(Some(Part::Condition(true)));
                                                    }
                                                }, {t("common-action-edit")} }
                                            }
                                            if perm.can("detail") && perm.delete_row() {
                                                Button { icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Text, onclick: {
                                                    let (c, p, f) = (c.clone(), g.plant.clone(), r.field.clone());
                                                    move |_| remove(c.clone(), format!("/targets/{p}/conditions/{f}"))
                                                }, {t("common-action-delete")} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Card {
                        div { class: "table-header",
                            h3 { {tf("carry-setting-defaults-title", &[("plant", g.plant.as_str())])} }
                            if perm.can("set_default_value") {
                                Button { icon: icons::ADD, tone: Tone::Primary, variant: ButtonVariant::Tonal, disabled: default_fields(&c.data_type).is_empty(), onclick: {
                                    let (dt, version) = (c.data_type.clone(), c.version);
                                    move |_| {
                                        part_error.set(None);
                                        default_form.set(CarryDefaultInput { field: default_fields(&dt).first().map(|f| f.0.clone()).unwrap_or_default(), value: String::new(), version: Some(version) });
                                        part.set(Some(Part::Default(false)));
                                    }
                                }, {t("carry-setting-action-add-default")} }
                            }
                        }
                        table { class: "md-table",
                            thead { tr { th { {t("data-control-field-field")} } th { {t("carry-setting-field-default-value")} } th {} } }
                            tbody {
                                if g.defaults.is_empty() {
                                    tr { td { colspan: 3, class: "muted", {t("common-msg-no-data")} } }
                                }
                                for d in g.defaults.iter().cloned() {
                                    tr { key: "{d.field}",
                                        td { {field_label(&d.field)} }
                                        td { class: "mono", "{d.value}" }
                                        td {
                                            if perm.can("set_default_value") {
                                                Button { icon: icons::EDIT, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: {
                                                    let (d, version) = (d.clone(), c.version);
                                                    move |_| {
                                                        part_error.set(None);
                                                        default_form.set(CarryDefaultInput { field: d.field.clone(), value: d.value.clone(), version: Some(version) });
                                                        part.set(Some(Part::Default(true)));
                                                    }
                                                }, {t("common-action-edit")} }
                                                Button { icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Text, onclick: {
                                                    let (c, p, f) = (c.clone(), g.plant.clone(), d.field.clone());
                                                    move |_| remove(c.clone(), format!("/targets/{p}/defaults/{f}"))
                                                }, {t("common-action-delete")} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    p { class: "query-hint", {t("carry-setting-hint-basic")} }
                }
            }

            if let Some(p) = part() {
                Dialog {
                    title: t(match p {
                        Part::Target(false) => "carry-setting-dialog-new-target",
                        Part::Target(true) => "carry-setting-dialog-edit-target",
                        Part::Condition(_) => "carry-setting-dialog-condition",
                        Part::Default(_) => "carry-setting-dialog-default",
                    }),
                    oncancel: move |_| { part.set(None); part_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { part.set(None); part_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save_part, {t("common-action-ok")} }
                    },
                    if let Some(msg) = part_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| part_error.set(None) }
                    }
                    div { class: "form-grid",
                        match p {
                            Part::Target(edit) => rsx! {
                                if let (Some(options), false) = (plant_options.clone(), edit) {
                                    Select { id: "gew04", label: t("carry-setting-field-plant"), options, value: target_form().plant, onchange: move |x| target_form.write().plant = x }
                                } else {
                                    TextField { id: "gew04", label: t("carry-setting-field-plant"), value: target_form().plant, readonly: edit, oninput: move |x| target_form.write().plant = x }
                                }
                                Select { id: "gew06", label: t("carry-setting-field-deletable"), options: vec![("Y".to_owned(), yes_no(true)), ("N".to_owned(), yes_no(false))], value: yn(target_form().deletable), readonly: basic, onchange: move |x: String| target_form.write().deletable = x == "Y" }
                                Select { id: "gew07", label: t("carry-setting-field-update-existing"), options: vec![("Y".to_owned(), yes_no(true)), ("N".to_owned(), yes_no(false))], value: yn(target_form().update_existing), onchange: move |x: String| target_form.write().update_existing = x == "Y" }
                                TextField { id: "gew08", label: t("carry-setting-field-notify"), maxlength: max_len::NOTIFY, value: target_form().notify.unwrap_or_default(), oninput: move |x: String| target_form.write().notify = Some(x) }
                            },
                            Part::Condition(edit) => rsx! {
                                Select { id: "field", label: t("data-control-field-field"), options: condition_fields(cur.as_ref().map(|c| c.data_type.as_str()).unwrap_or_default()), value: rule_form().field, readonly: edit, onchange: move |x| rule_form.write().field = x }
                                Select { id: "condition", label: t("data-control-field-condition"), options: conditions(), value: rule_form().condition, onchange: move |x| rule_form.write().condition = x }
                                TextField { id: "value", label: t("data-control-field-values"), maxlength: data_control_max_len::VALUES, value: rule_form().values, oninput: move |x| rule_form.write().values = x }
                            },
                            Part::Default(edit) => rsx! {
                                Select { id: "gez04", label: t("data-control-field-field"), options: default_fields(cur.as_ref().map(|c| c.data_type.as_str()).unwrap_or_default()), value: default_form().field, readonly: edit, onchange: move |x| default_form.write().field = x }
                                TextField { id: "gez05", label: t("carry-setting-field-default-value"), maxlength: max_len::DEFAULT, value: default_form().value, oninput: move |x| default_form.write().value = x }
                            },
                        }
                    }
                    p { class: "muted", {t(match p {
                        Part::Target(_) => "carry-setting-hint-target",
                        Part::Condition(_) => "data-control-hint-rule",
                        Part::Default(_) => "carry-setting-hint-default",
                    })} }
                }
            }

            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("carry-setting-dialog-new"),
                        Editor::Edit(c, _) => tf("carry-setting-dialog-edit", &[("center", c.as_str())]),
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
                        TextField { id: "gew01", label: t("carry-setting-field-center"), value: v.center_code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().center_code = x }
                        Select { id: "gew02", label: t("carry-setting-field-data-type"), options: data_types(), value: v.data_type.clone(), readonly: ed != Editor::New, onchange: move |x| form.write().data_type = x }
                        Select { id: "gew03", label: t("carry-setting-field-mode"), options: modes(), value: v.mode.clone(), onchange: move |x| form.write().mode = x }
                    }
                    p { class: "muted", {t("carry-setting-hint")} }
                }
            }
        }
    }
}
