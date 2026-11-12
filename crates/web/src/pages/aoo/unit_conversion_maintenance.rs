//! 單位換算維護（TipTop aooi102 / smc_file；specs/features/aoo/unit_conversion.feature）
//! 版面比照 tiptop/aoo/4fd/aooi102.4fd：單一表格（來源單位、來源數量、目的單位、目的數量、說明、有效）；新增、修改以小視窗編輯。
//! 一組單位只存一筆，不分方向（UC3）。功能（ADR 0013、UC9）：比照單位維護。

use dioxus::prelude::*;
use rust_decimal::Decimal;
use ttrs_shared::{QbeCondition, QueryRequest, UnitConversionDto, UnitConversionInput};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Select, Tabs, TextField,
    Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::qbe::{ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav};
use crate::components::status::{
    ConfirmCells, ConfirmHeaders, HasStatus, StatusCells, StatusHeaders, TAB_DATA, TAB_STATUS,
};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

const PROGRAM: &str = "unit-conversion-maintenance";

#[component]
pub fn UnitConversionMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ConversionPage {} }
    }
}

fn key(c: &UnitConversionDto) -> String {
    format!("{} {}", c.from_unit, c.to_unit)
}

/// 編輯中的表單：數量以字串保存
#[derive(Clone, Default, PartialEq)]
pub(super) struct ConvForm {
    pub from_unit: String,
    pub to_unit: String,
    pub from_quantity: String,
    pub to_quantity: String,
    pub note: String,
    pub version: Option<i32>,
}

impl ConvForm {
    pub fn new_default() -> Self {
        Self {
            from_quantity: "1".into(),
            to_quantity: "1".into(),
            ..Default::default()
        }
    }

    pub fn input(&self) -> Result<UnitConversionInput, String> {
        let num = |v: &str| {
            v.trim()
                .parse::<Decimal>()
                .map_err(|_| t("conversion-msg-quantity-invalid"))
        };
        Ok(UnitConversionInput {
            from_unit: self.from_unit.trim().to_owned(),
            to_unit: self.to_unit.trim().to_owned(),
            from_quantity: num(&self.from_quantity)?,
            to_quantity: num(&self.to_quantity)?,
            note: Some(self.note.trim().to_owned()).filter(|n| !n.is_empty()),
            version: self.version,
        })
    }
}

/// 有效單位的下拉選單
pub(super) fn use_unit_options() -> Resource<Vec<(String, String)>> {
    use_resource(|| async {
        let req = QueryRequest {
            conditions: Vec::new(),
            page: Some(1),
            page_size: Some(500),
            lang: None,
        };
        let units = api::query_units(&req)
            .await
            .map(|p| p.items)
            .unwrap_or_default();
        std::iter::once((String::new(), String::new()))
            .chain(units.into_iter().filter(|u| u.active).map(|u| {
                let label = format!("{} {}", u.unit_code, u.name.clone().unwrap_or_default()); // i18n-allow
                (u.unit_code, label)
            }))
            .collect()
    })
}

#[component]
fn ConversionPage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut rows = use_resource(move || {
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move { api::query_unit_conversions(&req).await }
    });
    let units = use_unit_options();
    let mut selected = use_signal(|| None::<String>);
    let nav = use_row_nav(page, selected, move || {
        rows.read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(key).collect())
            .unwrap_or_default()
    });
    let mut editing = use_signal(|| None::<Option<UnitConversionDto>>);
    let mut form = use_signal(ConvForm::default);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);
    let mut copy_from = use_signal(String::new);
    let mut copy_to = use_signal(String::new);
    let col_tab = use_signal(|| 0usize);

    let page_data = rows.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<UnitConversionDto> = page_data
        .as_ref()
        .map(|p| p.items.clone())
        .unwrap_or_default();
    let total = page_data.as_ref().map_or(0, |p| p.total);
    let pages = page_data.as_ref().map_or(1, |p| {
        u32::try_from(p.total.div_ceil(u64::from(p.page_size.max(1))))
            .unwrap_or(1)
            .max(1)
    });
    let cur: Option<UnitConversionDto> = selected()
        .and_then(|k| list.iter().find(|x| key(x) == k).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let confirmed = cur.as_ref().is_some_and(|c| c.confirmed);
    let inactive = cur.as_ref().is_some_and(|c| !c.active);
    let keys: Vec<String> = list.iter().map(key).collect();
    let status_tab = col_tab() == 1;
    let unit_options = units.read().clone().unwrap_or_default();

    let mut done = move |msg: String, k: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editing.set(None);
        dialog_error.set(None);
        selected.set(k);
        rows.restart();
    };

    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            if let Err(e) = api::system_list_report(
                "/api/unit-conversions",
                PROGRAM,
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

    let save = move |_| {
        let ed = editing();
        let input = match form().input() {
            Ok(i) => i,
            Err(e) => {
                dialog_error.set(Some(e));
                return;
            }
        };
        spawn(async move {
            let r = match ed {
                Some(Some(c)) => {
                    api::update_unit_conversion(&c.from_unit, &c.to_unit, &input).await
                }
                _ => api::create_unit_conversion(&input).await,
            };
            match r {
                Ok(c) => done(
                    tf("conversion-msg-saved", &[("key", key(&c).as_str())]),
                    Some(key(&c)),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let (c1, c2, c3, c4, c5, c6) = (
        cur.clone(),
        cur.clone(),
        cur.clone(),
        cur.clone(),
        cur.clone(),
        cur.clone(),
    );

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("conversion-breadcrumb")} }
                h2 { {t("conversion-title")} }
            }

            if querying() {
                div { class: "action-bar",
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: move |_| { let c = conditions_of(&qbe.read()); querying.set(false); selected.set(None); page.set(1); conditions.set(c); }, {t("common-action-ok")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| querying.set(false), {t("common-action-cancel")} }
                    TextField { id: "q-from_unit", label: t("conversion-field-from-unit"), value: qbe.read().get("from_unit").cloned().unwrap_or_default(), oninput: move |v: String| { qbe.write().insert("from_unit", v); } }
                    TextField { id: "q-to_unit", label: t("conversion-field-to-unit"), value: qbe.read().get("to_unit").cloned().unwrap_or_default(), oninput: move |v: String| { qbe.write().insert("to_unit", v); } }
                }
                p { class: "query-hint", {t("query-hint")} }
            } else {
                div { class: "action-bar",
                    MaintenanceToolbar {
                        detail_action: "detail",
                        invalid_action: "detail",
                        query: Act::new(true, move |_| { message.set(None); qbe.set(Default::default()); querying.set(true); }),
                        insert: Act::new(true, move |_| { message.set(None); dialog_error.set(None); form.set(ConvForm::new_default()); editing.set(Some(None)); }),
                        modify: Act::new(!no_cur && !inactive, move |_| {
                            let Some(c) = c1.clone() else { return };
                            dialog_error.set(None);
                            form.set(ConvForm {
                                from_unit: c.from_unit.clone(),
                                to_unit: c.to_unit.clone(),
                                from_quantity: c.from_quantity.normalize().to_string(),
                                to_quantity: c.to_quantity.normalize().to_string(),
                                note: c.note.clone().unwrap_or_default(),
                                version: Some(c.version),
                            });
                            editing.set(Some(Some(c)));
                        }),
                        copy: Act::new(!no_cur && !copy_from().trim().is_empty() && !copy_to().trim().is_empty(), move |_| {
                            let Some(c) = c2.clone() else { return };
                            let (f, t2) = (copy_from(), copy_to());
                            spawn(async move {
                                match api::copy_unit_conversion(&c.from_unit, &c.to_unit, f.trim(), t2.trim()).await {
                                    Ok(n) => { copy_from.set(String::new()); copy_to.set(String::new()); done(tf("conversion-msg-copied", &[("from", key(&c).as_str()), ("to", key(&n).as_str())]), Some(key(&n))); }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        delete: Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = c3.clone() else { return };
                            spawn(async move {
                                match api::delete_unit_conversion(&c.from_unit, &c.to_unit, c.version).await {
                                    Ok(()) => done(tf("conversion-msg-deleted", &[("key", key(&c).as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = c4.clone() else { return };
                            spawn(async move {
                                let state = if c.confirmed { "unconfirm" } else { "confirm" };
                                match api::set_unit_conversion_state(&c.from_unit, &c.to_unit, state, c.version).await {
                                    Ok(n) => done(tf(if n.confirmed { "conversion-msg-confirmed" } else { "conversion-msg-unconfirmed" }, &[("key", key(&n).as_str())]), Some(key(&n))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = c5.clone() else { return };
                            spawn(async move {
                                let state = if c.active { "deactivate" } else { "activate" };
                                match api::set_unit_conversion_state(&c.from_unit, &c.to_unit, state, c.version).await {
                                    Ok(n) => done(tf(if n.active { "conversion-msg-activated" } else { "conversion-msg-deactivated" }, &[("key", key(&n).as_str())]), Some(key(&n))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        inactive,
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        history: Act::new(!no_cur, move |_| {
                            let Some(c) = c6.clone() else { return };
                            spawn(async move {
                                match api::shared_history(PROGRAM, &c.from_unit).await {
                                    Ok(h) => history.set(Some((c.from_unit.clone(), h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        if perm.can("reproduce") {
                            TextField { id: "copy-from", label: t("conversion-field-copy-from"), maxlength: 4, value: copy_from(), disabled: no_cur, oninput: move |v| copy_from.set(v) }
                            TextField { id: "copy-to", label: t("conversion-field-copy-to"), maxlength: 4, value: copy_to(), disabled: no_cur, oninput: move |v| copy_to.set(v) }
                        }
                    }
                }
            }

            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(key).as_deref()),
                    ongo: { let keys = keys.clone(); move |i| { message.set(None); nav.go(i, &keys); } },
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
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            th { {t("conversion-field-from-unit")} }
                            th { {t("conversion-field-from-quantity")} }
                            th { {t("conversion-field-to-unit")} }
                            th { {t("conversion-field-to-quantity")} }
                            if status_tab {
                                StatusHeaders {}
                                ConfirmHeaders {}
                            } else {
                                th { {t("conversion-field-note")} }
                            }
                        } }
                        tbody {
                            for c in list.iter().cloned() {
                                tr {
                                    key: "{key(&c)}",
                                    class: "md-table__row--clickable",
                                    class: if cur.as_ref().is_some_and(|x| key(x) == key(&c)) { "md-table__row--selected" },
                                    onclick: { let k = key(&c); move |_| { message.set(None); selected.set(Some(k.clone())); } },
                                    td { class: "mono", "{c.from_unit}" }
                                    td { class: "num", {c.from_quantity.normalize().to_string()} }
                                    td { class: "mono", "{c.to_unit}" }
                                    td { class: "num", {c.to_quantity.normalize().to_string()} }
                                    if status_tab {
                                        StatusCells { status: c.status() }
                                        ConfirmCells { confirmed: c.confirmed, by: c.confirmed_by.clone() }
                                    } else {
                                        td { {c.note.clone().unwrap_or_default()} }
                                    }
                                }
                            }
                        }
                    }
                }
                ListPager { page, pages }
            }

            if let Some((k, changes)) = history() {
                HistoryDialog { record: k, changes, onclose: move |_| history.set(None) }
            }
            if let Some(ed) = editing() {
                Dialog {
                    title: match &ed { None => t("conversion-dialog-new"), Some(c) => tf("conversion-dialog-edit", &[("key", key(c).as_str())]) },
                    oncancel: move |_| { editing.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { editing.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    ConvFields { form, units: unit_options.clone(), fixed_units: ed.is_some() }
                    p { class: "muted", {t("conversion-dialog-hint")} }
                }
            }
        }
    }
}

/// 換算的輸入欄位（一般與料件共用）
#[component]
pub(super) fn ConvFields(
    form: Signal<ConvForm>,
    units: Vec<(String, String)>,
    fixed_units: bool,
) -> Element {
    let mut form = form;
    let v = form();
    rsx! {
        div { class: "form-grid",
            if fixed_units {
                TextField { id: "conv-from-unit", label: t("conversion-field-from-unit"), value: v.from_unit.clone(), readonly: true }
                TextField { id: "conv-to-unit", label: t("conversion-field-to-unit"), value: v.to_unit.clone(), readonly: true }
            } else {
                Select { id: "conv-from-unit", label: t("conversion-field-from-unit"), options: units.clone(), value: v.from_unit.clone(), onchange: move |x| form.write().from_unit = x }
                Select { id: "conv-to-unit", label: t("conversion-field-to-unit"), options: units.clone(), value: v.to_unit.clone(), onchange: move |x| form.write().to_unit = x }
            }
            TextField { id: "conv-from-quantity", label: t("conversion-field-from-quantity"), input_type: "number", value: v.from_quantity.clone(), oninput: move |x| form.write().from_quantity = x }
            TextField { id: "conv-to-quantity", label: t("conversion-field-to-quantity"), input_type: "number", value: v.to_quantity.clone(), oninput: move |x| form.write().to_quantity = x }
            TextField { id: "conv-note", label: t("conversion-field-note"), maxlength: 80, value: v.note.clone(), oninput: move |x| form.write().note = x }
        }
    }
}
