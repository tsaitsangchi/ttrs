//! Data Control Maintenance 資料控管原則維護（TipTop aooi601 資料控管原則維護作業）
//! 版面比照 tiptop/aoo/4fd/aooi601.4fd（DC1）：上方資料類別清單；選定資料類別後顯示參與控管的營運中心
//! （資料中心的營運中心唯讀，受控營運中心可新增、刪除）；再選定受控營運中心，下方維護它的欄位條件。
//! 共用層、集團層（DC2）。規則在後端 DataControlService。
//! 標準功能鈕（ADR 0013；DC9）：查詢、新增／刪除受控營運中心（detail）、欄位條件（condition_detail）、
//! 複製欄位條件（reproduce）、列印、匯出 Excel、上下筆、異動記錄；失效、確認為例外。

use dioxus::prelude::*;
use ttrs_shared::{
    DataControlDto, DataControlPlantInput, DataControlRuleInput, QbeCondition, QueryRequest,
    data_control_max_len as max_len,
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
const PROGRAM: &str = "data-control-maintenance";

/// 資料類別 1–8（0 基本資料不可維護，DC3）
fn data_types() -> Vec<(String, String)> {
    (1..=8)
        .map(|i| (i.to_string(), data_type_label(&i.to_string())))
        .collect()
}

fn data_type_label(code: &str) -> String {
    t(&format!("group-center-data-type-{code}"))
}

/// 目前開放的欄位（DC5(b)：只有料件）
fn fields_of(data_type: &str) -> Vec<(String, String)> {
    match data_type {
        "1" => ["item_no", "group_code", "material_category"]
            .into_iter()
            .map(|f| (f.to_owned(), field_label(f)))
            .collect(),
        _ => Vec::new(),
    }
}

fn field_label(field: &str) -> String {
    t(&format!("data-control-field-{field}"))
}

fn conditions() -> Vec<(String, String)> {
    (1..=11)
        .map(|i| (i.to_string(), condition_label(&i.to_string())))
        .collect()
}

fn condition_label(code: &str) -> String {
    t(&format!("data-control-condition-{code}"))
}

#[component]
pub fn DataControlMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ControlPage {} }
    }
}

#[component]
fn ControlPage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions_sig = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut types = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions_sig(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move { api::query_data_controls(&req).await }
    });
    // 營運中心選單（沒有營運中心維護的查詢權限時改為直接輸入代碼）
    let plants = use_resource(|| async { api::list_plants().await });
    let selected = use_signal(|| None::<String>);
    let nav = use_row_nav(page, selected, move || {
        types
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|u| u.data_type.clone()).collect())
            .unwrap_or_default()
    });
    let mut selected = selected;
    let mut selected_plant = use_signal(|| None::<String>);
    let mut copy_to = use_signal(String::new);
    // 新增受控營運中心
    let mut plant_open = use_signal(|| false);
    let mut plant_form = use_signal(DataControlPlantInput::default);
    let mut plant_error = use_signal(|| None::<String>);
    // 欄位條件編輯：None 未開啟；Some(false) 新增；Some(true) 修改
    let mut rule_editor = use_signal(|| None::<bool>);
    let mut rule_form = use_signal(DataControlRuleInput::default);
    let mut rule_error = use_signal(|| None::<String>);

    let page_data = types.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<DataControlDto> = page_data
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
    let cur: Option<DataControlDto> = selected()
        .and_then(|c| list.iter().find(|x| x.data_type == c).cloned())
        .or_else(|| list.first().cloned());
    let keys: Vec<String> = list.iter().map(|u| u.data_type.clone()).collect();
    // 選定的受控營運中心（資料中心那列不可選來設定條件，DC5(a)）
    let cur_plant = cur.as_ref().and_then(|c| {
        selected_plant()
            .and_then(|p| c.plants.iter().find(|x| x.plant == p).cloned())
            .or_else(|| c.plants.iter().find(|x| !x.is_data_center).cloned())
            .filter(|p| !p.is_data_center)
    });
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

    let mut done = move |msg: String, data_type: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        selected.set(data_type);
        types.restart();
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
            match api::query_data_controls(&req).await {
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

    let save_plant = move |_| {
        let req = plant_form();
        spawn(async move {
            match api::add_data_control_plant(&req).await {
                Ok(r) => {
                    plant_open.set(false);
                    plant_error.set(None);
                    selected_plant.set(Some(req.plant.trim().to_uppercase()));
                    done(
                        tf(
                            "data-control-msg-plant-added",
                            &[("plant", req.plant.as_str())],
                        ),
                        Some(r.data_type),
                    );
                }
                Err(e) => plant_error.set(Some(e)),
            }
        });
    };

    let rule_target = cur.as_ref().map(|c| c.data_type.clone());
    let rule_plant = cur_plant.as_ref().map(|p| p.plant.clone());
    let save_rule = move |_| {
        let (Some(edit), Some(dt), Some(pl)) =
            (rule_editor(), rule_target.clone(), rule_plant.clone())
        else {
            return;
        };
        let req = rule_form();
        spawn(async move {
            match api::save_data_control_rule(&dt, &pl, edit, &req).await {
                Ok(r) => {
                    rule_editor.set(None);
                    rule_error.set(None);
                    done(
                        tf("data-control-msg-rule-saved", &[("plant", pl.as_str())]),
                        Some(r.data_type),
                    );
                }
                Err(e) => rule_error.set(Some(e)),
            }
        });
    };

    let report = move |format: api::ReportFormat| {
        let conds = conditions_sig.read().clone();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            if let Err(e) = api::system_list_report(
                "/api/data-controls",
                "data-controls",
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

    let cur_insert = cur.clone();
    let del_target = cur
        .as_ref()
        .map(|c| c.data_type.clone())
        .zip(cur_plant.clone());
    let copy_target = del_target.clone();
    let has_plant = cur_plant.is_some();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("data-control-breadcrumb")} }
                h2 { {t("data-control-title")} }
            }

            if querying() {
                div { class: "action-bar",
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { error.set(None); querying.set(false); }, {t("common-action-cancel")} }
                }
                p { class: "query-hint", {t("query-hint")} }
                div { class: "form-grid",
                    for (id, field) in [("data-control-field-data-type", "data_type"), ("data-control-field-plant", "plant"), ("data-control-field-field", "field"), ("data-control-field-condition", "condition"), ("data-control-field-values", "values")] {
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
                        // 受控營運中心的新增、刪除用 TipTop 的 detail（DC9）
                        detail_action: Some("detail"),
                        insert: Act::new(true, move |_| {
                            message.set(None);
                            plant_error.set(None);
                            plant_form.set(DataControlPlantInput {
                                data_type: cur_insert.as_ref().map(|c| c.data_type.clone()).unwrap_or_else(|| "1".into()),
                                plant: String::new(),
                            });
                            plant_open.set(true);
                        }),
                        delete: Act::new(has_plant, move |_| {
                            let Some((dt, p)) = del_target.clone() else { return };
                            spawn(async move {
                                match api::remove_data_control_plant(&dt, &p.plant, p.version).await {
                                    Ok(r) => {
                                        selected_plant.set(None);
                                        done(tf("data-control-msg-plant-deleted", &[("plant", p.plant.as_str())]), Some(r.data_type));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 複製（DC9）：把選定受控營運中心的欄位條件複製到另一個營運中心
                        copy: Act::new(has_plant && !copy_to().trim().is_empty(), move |_| {
                            let Some((dt, p)) = copy_target.clone() else { return };
                            let to = copy_to();
                            spawn(async move {
                                match api::copy_data_control_rules(&dt, &p.plant, to.trim()).await {
                                    Ok(r) => {
                                        copy_to.set(String::new());
                                        selected_plant.set(Some(to.trim().to_uppercase()));
                                        done(tf("data-control-msg-copied", &[("from", p.plant.as_str()), ("to", to.trim())]), Some(r.data_type));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        if perm.can("history") {
                            HistoryButton { program: PROGRAM, record: cur.as_ref().map(|c| c.data_type.clone()) }
                        }
                        if perm.can("reproduce") {
                            TextField { id: "copy-to", label: t("data-control-field-copy-to"), value: copy_to(), disabled: !has_plant, oninput: move |v| copy_to.set(v) }
                        }
                    }
                }
            }

            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.data_type.as_str())),
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

            // 資料類別清單（aooi601 單頭）
            Card {
                div { class: "table-header",
                    h3 { {t("data-control-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("data-control-field-data-type")} }
                                th { {t("data-control-field-plant-count")} }
                            }
                        }
                        tbody {
                            match &*types.read() {
                                None => rsx! { tr { td { colspan: 2, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 2, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: 2, class: "muted", {t("data-control-msg-empty")} } } },
                                Some(Ok(_)) => rsx! {
                                    for d in list.iter().cloned() {
                                        tr {
                                            key: "{d.data_type}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.data_type == d.data_type) { "md-table__row--selected" },
                                            onclick: {
                                                let dt = d.data_type.clone();
                                                move |_| { message.set(None); error.set(None); selected_plant.set(None); selected.set(Some(dt.clone())); }
                                            },
                                            td { {data_type_label(&d.data_type)} }
                                            td { class: "num", "{d.plants.len()}" }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
                ListPager { page, pages }
            }

            // 參與控管的營運中心（aooi601 單身一）
            if let Some(c) = cur.clone() {
                Card {
                    Tabs { tabs: vec![t(TAB_DATA), t(TAB_STATUS)], active: col_tab }
                    div { class: "table-header",
                        h3 { {tf("data-control-plants-title", &[("type", data_type_label(&c.data_type).as_str())])} }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("data-control-field-plant")} }
                                    if status_tab {
                                        StatusHeaders {}
                                    } else {
                                        th { {t("data-control-field-is-data-center")} }
                                        th { {t("data-control-field-center")} }
                                        th { {t("data-control-field-rule-count")} }
                                    }
                                }
                            }
                            tbody {
                                for p in c.plants.iter().cloned() {
                                    tr {
                                        key: "{p.plant}",
                                        class: if !p.is_data_center { "md-table__row--clickable" },
                                        class: if cur_plant.as_ref().is_some_and(|x| x.plant == p.plant) { "md-table__row--selected" },
                                        onclick: {
                                            let (code, center) = (p.plant.clone(), p.is_data_center);
                                            move |_| { if !center { selected_plant.set(Some(code.clone())); } }
                                        },
                                        td { class: "mono", "{p.plant}" }
                                        if status_tab {
                                            StatusCells { status: p.status() }
                                        } else {
                                            td { {t(if p.is_data_center { "data-control-yes" } else { "data-control-no" })} }
                                            td { {p.center_code.clone().map(|code| format!("{code} {}", p.center_name.clone().unwrap_or_default())).unwrap_or_default()} }
                                            td { class: "num", if p.is_data_center { "" } else { "{p.rules.len()}" } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    p { class: "query-hint", {t("data-control-hint-plants")} }
                }
            }

            // 欄位條件（aooi601 單身二）：只有受控營運中心
            if let (Some(c), Some(p)) = (cur.clone(), cur_plant.clone()) {
                Card {
                    div { class: "table-header",
                        h3 { {tf("data-control-rules-title", &[("plant", p.plant.as_str())])} }
                        if perm.can("condition_detail") && perm.add_row() {
                            Button { icon: icons::ADD, tone: Tone::Primary, variant: ButtonVariant::Tonal, disabled: fields_of(&c.data_type).is_empty(), onclick: {
                                let (dt, version) = (c.data_type.clone(), p.version);
                                move |_| {
                                    rule_error.set(None);
                                    rule_form.set(DataControlRuleInput {
                                        field: fields_of(&dt).first().map(|f| f.0.clone()).unwrap_or_default(),
                                        condition: "1".into(),
                                        values: String::new(),
                                        version: Some(version),
                                    });
                                    rule_editor.set(Some(false));
                                }
                            }, {t("data-control-action-add-rule")} }
                        }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("data-control-field-field")} }
                                    th { {t("data-control-field-condition")} }
                                    th { {t("data-control-field-values")} }
                                    th {}
                                }
                            }
                            tbody {
                                if p.rules.is_empty() {
                                    tr { td { colspan: 4, class: "muted", {t("common-msg-no-data")} } }
                                }
                                for r in p.rules.iter().cloned() {
                                    tr { key: "{r.field}",
                                        td { {field_label(&r.field)} }
                                        td { {condition_label(&r.condition)} }
                                        // 使用者輸入的條件值不翻譯
                                        td { class: "mono", "{r.values}" }
                                        td {
                                            if perm.can("condition_detail") {
                                                Button { icon: icons::EDIT, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: {
                                                    let (r, version) = (r.clone(), p.version);
                                                    move |_| {
                                                        rule_error.set(None);
                                                        rule_form.set(DataControlRuleInput { field: r.field.clone(), condition: r.condition.clone(), values: r.values.clone(), version: Some(version) });
                                                        rule_editor.set(Some(true));
                                                    }
                                                }, {t("common-action-edit")} }
                                            }
                                            if perm.can("condition_detail") && perm.delete_row() {
                                                Button { icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Text, onclick: {
                                                    let (dt, pl, field, version) = (c.data_type.clone(), p.plant.clone(), r.field.clone(), p.version);
                                                    move |_| {
                                                        let (dt, pl, field) = (dt.clone(), pl.clone(), field.clone());
                                                        spawn(async move {
                                                            match api::remove_data_control_rule(&dt, &pl, &field, version).await {
                                                                Ok(r) => done(tf("data-control-msg-rule-deleted", &[("plant", pl.as_str())]), Some(r.data_type)),
                                                                Err(e) => error.set(Some(e)),
                                                            }
                                                        });
                                                    }
                                                }, {t("common-action-delete")} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    p { class: "query-hint", {t("data-control-hint-rules")} }
                }
            }

            if plant_open() {
                Dialog {
                    title: t("data-control-dialog-new-plant"),
                    oncancel: move |_| { plant_open.set(false); plant_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { plant_open.set(false); plant_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save_plant, {t("common-action-ok")} }
                    },
                    if let Some(msg) = plant_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| plant_error.set(None) }
                    }
                    div { class: "form-grid",
                        Select {
                            id: "gev01",
                            label: t("data-control-field-data-type"),
                            options: data_types(),
                            value: plant_form().data_type,
                            onchange: move |x| plant_form.write().data_type = x,
                        }
                        if let Some(options) = plant_options.clone() {
                            Select {
                                id: "gev02",
                                label: t("data-control-field-plant"),
                                options,
                                value: plant_form().plant,
                                onchange: move |x| plant_form.write().plant = x,
                            }
                        } else {
                            TextField { id: "gev02", label: t("data-control-field-plant"), value: plant_form().plant, oninput: move |x| plant_form.write().plant = x }
                        }
                    }
                    p { class: "muted", {t("data-control-hint-new-plant")} }
                }
            }

            if let Some(edit) = rule_editor() {
                Dialog {
                    title: t(if edit { "data-control-dialog-edit-rule" } else { "data-control-dialog-new-rule" }),
                    oncancel: move |_| { rule_editor.set(None); rule_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { rule_editor.set(None); rule_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save_rule, {t("common-action-ok")} }
                    },
                    if let Some(msg) = rule_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| rule_error.set(None) }
                    }
                    div { class: "form-grid",
                        Select {
                            id: "gey03",
                            label: t("data-control-field-field"),
                            options: fields_of(cur.as_ref().map(|c| c.data_type.as_str()).unwrap_or_default()),
                            value: rule_form().field,
                            readonly: edit,
                            onchange: move |x| rule_form.write().field = x,
                        }
                        Select {
                            id: "gey04",
                            label: t("data-control-field-condition"),
                            options: conditions(),
                            value: rule_form().condition,
                            onchange: move |x| rule_form.write().condition = x,
                        }
                        TextField { id: "gey05", label: t("data-control-field-values"), maxlength: max_len::VALUES, value: rule_form().values, oninput: move |x| rule_form.write().values = x }
                    }
                    p { class: "muted", {t("data-control-hint-rule")} }
                }
            }
        }
    }
}
