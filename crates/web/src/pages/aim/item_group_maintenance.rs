//! Item Group Maintenance 料件分群維護（TipTop aimi110 料件分群碼維護作業）
//! 版面比照 tiptop/aim/4fd/aimi110.4fd（IG1）：動作列＋上下筆＋單筆表單，分頁「主畫面」「工程／計劃／庫存」
//! 「會計／狀態／分群」「狀態」（ADR 0012 §9），下方為分群清單。第一階段 41 欄（IG3）。
//! 工廠層：用登入的營運中心（IG2）。規則在後端 ItemGroupService。

use dioxus::prelude::*;
use rust_decimal::Decimal;
use ttrs_shared::{ItemGroupDto, ItemGroupInput};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Chip, Dialog, Notification, NotificationKind, Select,
    Tabs, TextField, Tone, active_tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::components::qbe::RecordNav;
use crate::components::status::{HasStatus, StatusForm, TAB_STATUS};
use crate::i18n::{t, tf};

/// 權限檢查用的程式代號
const PROGRAM: &str = "item-group-maintenance";

/// 來源碼 imz08（mfg1001，aimi110.per:267）
const SOURCES: [&str; 13] = [
    "C", "T", "D", "A", "M", "P", "X", "K", "U", "V", "R", "Z", "S",
];
/// 補貨策略碼 imz37（aimi110.per:303）
const REPLENISH: [&str; 6] = ["0", "1", "2", "3", "4", "5"];
/// 保稅料件型態 imz106（aimi110.per:343）
const BONDED_TYPES: [&str; 3] = ["1", "2", "3"];

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    View,
    New,
    Edit,
}

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

fn txt(v: &Option<String>) -> String {
    v.clone().unwrap_or_default()
}

fn dec_text(v: Option<Decimal>) -> String {
    v.map(|d| d.normalize().to_string()).unwrap_or_default()
}

fn dec(v: String) -> Option<Decimal> {
    v.trim().parse().ok()
}

/// 下拉選項：空白＋「代碼 說明」
fn options(codes: &[&str], prefix: &str) -> Vec<(String, String)> {
    let mut v = vec![(String::new(), String::new())];
    v.extend(codes.iter().map(|c| {
        (
            (*c).to_owned(),
            format!("{c} {}", t(&format!("{prefix}-{}", c.to_lowercase()))),
        )
    }));
    v
}

#[component]
pub fn ItemGroupMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ItemGroupPage {} }
    }
}

#[component]
fn ItemGroupPage() -> Element {
    let perm = use_context::<Perm>();
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut groups = use_resource(move || {
        let p = plant();
        async move {
            if p.is_empty() {
                return Ok(Vec::new());
            }
            api::list_item_groups(&p).await
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut mode = use_signal(|| Mode::View);
    let mut form = use_signal(ItemGroupInput::default);
    let tab = use_signal(|| 0usize);
    let mut copying = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let mut copy_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);

    let list: Vec<ItemGroupDto> = groups
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let pos = selected()
        .and_then(|c| list.iter().position(|x| x.fields.group_code == c))
        .or(if list.is_empty() { None } else { Some(0) });
    let cur: Option<ItemGroupDto> = pos.map(|i| list[i].clone());
    let no_cur = cur.is_none();
    let total = list.len() as u64;
    let m = mode();
    let ro = m == Mode::View;
    let v = if ro {
        cur.as_ref().map(|c| c.fields.clone()).unwrap_or_default()
    } else {
        form()
    };
    let inactive = cur.as_ref().is_some_and(|c| !c.active);

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        mode.set(Mode::View);
        selected.set(code);
        groups.restart();
    };

    let save = move |_| {
        let p = plant();
        let req = form();
        let m = mode();
        spawn(async move {
            let result = if m == Mode::Edit {
                api::update_item_group(&p, &req.group_code.clone(), &req).await
            } else {
                api::create_item_group(&p, &req).await
            };
            match result {
                Ok(g) => {
                    let code = g.fields.group_code.clone();
                    done(
                        tf("igroup-msg-saved", &[("code", code.as_str())]),
                        Some(code),
                    );
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let do_copy = move |_| {
        let p = plant();
        let Some(from) = copying() else { return };
        let to = copy_code();
        spawn(async move {
            match api::copy_item_group(&p, &from, to.trim()).await {
                Ok(g) => {
                    copying.set(None);
                    copy_error.set(None);
                    let code = g.fields.group_code.clone();
                    done(
                        tf(
                            "igroup-msg-copied",
                            &[("from", from.as_str()), ("to", code.as_str())],
                        ),
                        Some(code),
                    );
                }
                Err(e) => copy_error.set(Some(e)),
            }
        });
    };

    let cur_edit = cur.clone();
    let cur_copy = cur.clone();
    let cur_delete = cur.clone();
    let cur_history = cur.clone();
    let cur_toggle = cur.clone();
    let list_rows = list.clone();
    let units_hint = t("igroup-hint-unit");

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("igroup-breadcrumb")} }
                h2 { {t("igroup-title")} }
            }
            PlantPicker { plant }

            div { class: "action-bar",
                if ro {
                    if perm.can("insert") {
                        Button {
                            icon: icons::ADD, tone: Tone::Success,
                            onclick: move |_| {
                                message.set(None);
                                error.set(None);
                                form.set(ItemGroupInput {
                                    source_code: Some("P".into()),
                                    replenish_policy: Some("0".into()),
                                    ..ItemGroupInput::default()
                                });
                                mode.set(Mode::New);
                            },
                            {t("common-action-new")}
                        }
                    }
                    if perm.can("modify") {
                        Button {
                            icon: icons::EDIT, tone: Tone::Tertiary,
                            variant: ButtonVariant::Tonal,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(c) = cur_edit.clone() else { return };
                                message.set(None);
                                error.set(None);
                                form.set(c.to_input());
                                mode.set(Mode::Edit);
                            },
                            {t("common-action-edit")}
                        }
                    }
                    if perm.can("reproduce") {
                        Button {
                            icon: icons::COPY, tone: Tone::Primary,
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(c) = cur_copy.clone() else { return };
                                copy_code.set(String::new());
                                copy_error.set(None);
                                copying.set(Some(c.fields.group_code));
                            },
                            {t("common-action-copy")}
                        }
                    }
                    if perm.can("history") {
                        Button {
                            icon: icons::HISTORY, tone: Tone::Primary,
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(c) = cur_history.clone() else { return };
                                let p = plant();
                                spawn(async move {
                                    let code = c.fields.group_code.clone();
                                    match api::item_group_history(&p, &code).await {
                                        Ok(h) => history.set(Some((code, h))),
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            {t("common-action-history")}
                        }
                    }
                    if perm.can("invalid") {
                        Button {
                            icon: if inactive { icons::CHECK_CIRCLE } else { icons::BLOCK },
                            tone: if inactive { Tone::Success } else { Tone::Error },
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(c) = cur_toggle.clone() else { return };
                                let p = plant();
                                spawn(async move {
                                    match api::set_item_group_active(&p, &c.fields.group_code, !c.active, c.version).await {
                                        Ok(r) => {
                                            let id = if r.active { "igroup-msg-activated" } else { "igroup-msg-deactivated" };
                                            let code = r.fields.group_code.clone();
                                            done(tf(id, &[("code", code.as_str())]), Some(code));
                                        }
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            if inactive { {t("igroup-action-activate")} } else { {t("igroup-action-deactivate")} }
                        }
                    }
                    if perm.can("delete") {
                        Button {
                            icon: icons::DELETE, tone: Tone::Error,
                            variant: ButtonVariant::Error,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(c) = cur_delete.clone() else { return };
                                let p = plant();
                                spawn(async move {
                                    let code = c.fields.group_code.clone();
                                    match api::delete_item_group(&p, &code, c.version).await {
                                        Ok(()) => done(tf("igroup-msg-deleted", &[("code", code.as_str())]), None),
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            {t("common-action-delete")}
                        }
                    }
                } else {
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {t("common-action-ok")} }
                    Button {
                        icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text,
                        onclick: move |_| { error.set(None); mode.set(Mode::View); },
                        {t("common-action-cancel")}
                    }
                }
            }

            if ro && total > 0 {
                RecordNav {
                    total,
                    pos: pos.map(|p| p as u64),
                    ongo: {
                        let codes: Vec<String> = list.iter().map(|g| g.fields.group_code.clone()).collect();
                        move |i: u64| {
                            if let Some(c) = codes.get(i as usize) {
                                message.set(None);
                                selected.set(Some(c.clone()));
                            }
                        }
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

            // ----- 單筆表單（aimi110.4fd） -----
            Card {
                Tabs { tabs: vec![t("igroup-tab-main"), t("igroup-tab-planning"), t("igroup-tab-accounting"), t(TAB_STATUS)], active: tab }
                div { class: "card-body",
                  match tab() {
                    0 => rsx! {
                        div { class: "form-grid",
                            TextField { id: "imz01", label: t("igroup-field-code"), maxlength: 10usize, value: v.group_code.clone(), readonly: m != Mode::New, oninput: move |x| form.write().group_code = x }
                            TextField { id: "imz02", label: t("igroup-field-description"), maxlength: 80usize, value: txt(&v.description), readonly: ro, oninput: move |x| form.write().description = opt(x) }
                            Select { id: "imz08", label: t("igroup-field-source"), options: options(&SOURCES, "igroup-source"), value: txt(&v.source_code), readonly: ro, onchange: move |x| form.write().source_code = opt(x) }
                            TextField { id: "imz25", label: t("igroup-field-stock-unit"), maxlength: 4usize, value: txt(&v.stock_unit), readonly: ro, oninput: move |x| form.write().stock_unit = opt(x) }
                            TextField { id: "imz03", label: t("igroup-field-catalog-ref"), maxlength: 20usize, value: txt(&v.catalog_ref), readonly: ro, oninput: move |x| form.write().catalog_ref = opt(x) }
                            TextField { id: "imz72", label: t("igroup-field-supplier-control"), maxlength: 1usize, value: txt(&v.supplier_control), readonly: ro, oninput: move |x| form.write().supplier_control = opt(x) }
                            TextField { id: "imz04", label: t("igroup-field-drawing-no"), maxlength: 20usize, value: txt(&v.drawing_no), readonly: ro, oninput: move |x| form.write().drawing_no = opt(x) }
                        }
                        p { class: "muted", "{units_hint}" }
                    },
                    1 => rsx! {
                        div { class: "form-grid cols-4",
                            Checkbox { id: "imz105", label: t("igroup-field-software"), checked: v.software, readonly: ro, onchange: move |b| form.write().software = b }
                            Checkbox { id: "imz14", label: t("igroup-field-engineering"), checked: v.engineering, readonly: ro, onchange: move |b| form.write().engineering = b }
                            Checkbox { id: "imz147", label: t("igroup-field-insert-qpa-check"), checked: v.insert_qpa_check, readonly: ro, onchange: move |b| form.write().insert_qpa_check = b }
                            Checkbox { id: "imz107", label: t("igroup-field-insert-position"), checked: v.insert_position, readonly: ro, onchange: move |b| form.write().insert_position = b }
                            Checkbox { id: "imz903", label: t("igroup-field-coproduct"), checked: v.coproduct, readonly: ro, onchange: move |b| form.write().coproduct = b }
                            Checkbox { id: "imz24", label: t("igroup-field-inspection"), checked: v.inspection, readonly: ro, onchange: move |b| form.write().inspection = b }
                            Checkbox { id: "imz911", label: t("igroup-field-repetitive"), checked: v.repetitive, readonly: ro, onchange: move |b| form.write().repetitive = b }
                            Checkbox { id: "imz70", label: t("igroup-field-consumable"), checked: v.consumable, readonly: ro, onchange: move |b| form.write().consumable = b }
                        }
                        div { class: "form-grid cols-4",
                            TextField { id: "imz109", label: t("igroup-field-material-category"), maxlength: 10usize, value: txt(&v.material_category), readonly: ro, oninput: move |x| form.write().material_category = opt(x) }
                            Select { id: "imz07", label: t("igroup-field-abc"), options: vec![(String::new(), String::new()), ("A".into(), "A".into()), ("B".into(), "B".into()), ("C".into(), "C".into())], value: txt(&v.abc_code), readonly: ro, onchange: move |x| form.write().abc_code = opt(x) }
                            Select { id: "imz37", label: t("igroup-field-replenish"), options: options(&REPLENISH, "igroup-replenish"), value: txt(&v.replenish_policy), readonly: ro, onchange: move |x| form.write().replenish_policy = opt(x) }
                            TextField { id: "imz38", label: t("igroup-field-reorder-point"), input_type: "number", value: dec_text(v.reorder_point), readonly: ro, oninput: move |x| form.write().reorder_point = dec(x) }
                            TextField { id: "imz99", label: t("igroup-field-reorder-qty"), input_type: "number", value: dec_text(v.reorder_qty), readonly: ro, oninput: move |x| form.write().reorder_qty = dec(x) }
                            TextField { id: "imz27", label: t("igroup-field-safety-stock"), input_type: "number", value: dec_text(v.safety_stock), readonly: ro, oninput: move |x| form.write().safety_stock = dec(x) }
                            TextField { id: "imz28", label: t("igroup-field-safety-period"), input_type: "number", value: dec_text(v.safety_period), readonly: ro, oninput: move |x| form.write().safety_period = dec(x) }
                            TextField { id: "imz71", label: t("igroup-field-shelf-life"), input_type: "number", value: v.shelf_life_days.map(|n| n.to_string()).unwrap_or_default(), readonly: ro, oninput: move |x: String| form.write().shelf_life_days = x.trim().parse().ok() }
                            TextField { id: "imz909", label: t("igroup-field-mrp-bucket"), input_type: "number", value: v.mrp_bucket_days.map(|n| n.to_string()).unwrap_or_default(), readonly: ro, oninput: move |x: String| form.write().mrp_bucket_days = x.trim().parse().ok() }
                            TextField { id: "imz35", label: t("igroup-field-warehouse"), maxlength: 10usize, value: txt(&v.warehouse), readonly: ro, oninput: move |x| form.write().warehouse = opt(x) }
                            TextField { id: "imz36", label: t("igroup-field-location"), maxlength: 10usize, value: txt(&v.location), readonly: ro, oninput: move |x| form.write().location = opt(x) }
                            TextField { id: "imz23", label: t("igroup-field-stock-keeper"), maxlength: 10usize, value: txt(&v.stock_keeper), readonly: ro, oninput: move |x| form.write().stock_keeper = opt(x) }
                            TextField { id: "gen02", label: t("igroup-field-stock-keeper-name"), value: if ro { cur.as_ref().and_then(|c| c.stock_keeper_name.clone()).unwrap_or_default() } else { String::new() }, readonly: true }
                            TextField { id: "imz17", label: t("igroup-field-sales-stat-unit"), maxlength: 4usize, value: txt(&v.sales_stat_unit), readonly: ro, oninput: move |x| form.write().sales_stat_unit = opt(x) }
                            TextField { id: "imz907", label: t("igroup-field-second-unit"), maxlength: 4usize, value: txt(&v.second_unit), readonly: ro, oninput: move |x| form.write().second_unit = opt(x) }
                            TextField { id: "imz908", label: t("igroup-field-pricing-unit"), maxlength: 4usize, value: txt(&v.pricing_unit), readonly: ro, oninput: move |x| form.write().pricing_unit = opt(x) }
                        }
                        p { class: "muted", {t("igroup-hint-planning")} }
                    },
                    2 => rsx! {
                        div { class: "form-grid cols-4",
                            TextField { id: "imz09", label: t("igroup-field-other-group1"), maxlength: 10usize, value: txt(&v.other_group1), readonly: ro, oninput: move |x| form.write().other_group1 = opt(x) }
                            TextField { id: "imz10", label: t("igroup-field-other-group2"), maxlength: 10usize, value: txt(&v.other_group2), readonly: ro, oninput: move |x| form.write().other_group2 = opt(x) }
                            TextField { id: "imz11", label: t("igroup-field-other-group3"), maxlength: 10usize, value: txt(&v.other_group3), readonly: ro, oninput: move |x| form.write().other_group3 = opt(x) }
                            TextField { id: "imz12", label: t("igroup-field-other-group4"), maxlength: 10usize, value: txt(&v.other_group4), readonly: ro, oninput: move |x| form.write().other_group4 = opt(x) }
                            TextField { id: "imz39", label: t("igroup-field-account"), maxlength: 24usize, value: txt(&v.account), readonly: ro, oninput: move |x| form.write().account = opt(x) }
                            TextField { id: "imz391", label: t("igroup-field-account2"), maxlength: 24usize, value: txt(&v.account2), readonly: ro, oninput: move |x| form.write().account2 = opt(x) }
                            Checkbox { id: "imz15", label: t("igroup-field-bonded"), checked: v.bonded, readonly: ro, onchange: move |b| form.write().bonded = b }
                            TextField { id: "imz19", label: t("igroup-field-bonded-class"), maxlength: 4usize, value: txt(&v.bonded_class), readonly: ro, oninput: move |x| form.write().bonded_class = opt(x) }
                            TextField { id: "imz21", label: t("igroup-field-tariff-no"), maxlength: 4usize, value: txt(&v.tariff_no), readonly: ro, oninput: move |x| form.write().tariff_no = opt(x) }
                            Select { id: "imz106", label: t("igroup-field-bonded-type"), options: options(&BONDED_TYPES, "igroup-bonded-type"), value: txt(&v.bonded_type), readonly: ro, onchange: move |x| form.write().bonded_type = opt(x) }
                        }
                        p { class: "muted", {t("igroup-hint-accounting")} }
                    },
                    _ => rsx! {
                        StatusForm { status: cur.as_ref().filter(|_| m != Mode::New).map(|c| c.status()), query: false, qbe: None, prefix: "imz" }
                    },
                  }
                }
            }

            // ----- 分群清單 -----
            Card {
                div { class: "table-header",
                    h3 { {t("igroup-list-title")} }
                    p { {tf("list-count-all", &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for id in ["igroup-field-code", "igroup-field-description", "igroup-field-source", "igroup-field-stock-unit", "common-field-active"] {
                                th { key: "{id}", {t(id)} }
                            }
                        } }
                        tbody {
                            match &*groups.read() {
                                None => rsx! { tr { td { colspan: 5, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 5, "{e}" } } },
                                Some(Ok(_)) if list_rows.is_empty() => rsx! { tr { td { colspan: 5, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for g in list_rows.iter().cloned() {
                                        tr {
                                            key: "{g.fields.group_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|c| c.fields.group_code == g.fields.group_code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = g.fields.group_code.clone();
                                                move |_| if mode() == Mode::View { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{g.fields.group_code}" }
                                            td { {txt(&g.fields.description)} }
                                            td { class: "mono", {txt(&g.fields.source_code)} }
                                            td { class: "mono", {txt(&g.fields.stock_unit)} }
                                            td {
                                                Chip { label: if g.active { t("igroup-status-active") } else { t("igroup-status-inactive") }, tone: active_tone(g.active) }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }

            if let Some((key, changes)) = history() {
                HistoryDialog { record: key, changes, onclose: move |_| history.set(None) }
            }
            if let Some(from) = copying() {
                Dialog {
                    title: tf("igroup-dialog-copy", &[("code", from.as_str())]),
                    oncancel: move |_| copying.set(None),
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| copying.set(None), {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: do_copy, {t("common-action-ok")} }
                    },
                    if let Some(msg) = copy_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| copy_error.set(None) }
                    }
                    div { class: "form-grid",
                        TextField { id: "imz01_new", label: t("igroup-field-new-code"), maxlength: 10usize, value: copy_code(), oninput: move |x| copy_code.set(x) }
                    }
                }
            }
        }
    }
}
