//! Item Group Maintenance 料件分群維護（TipTop aimi110 料件分群碼維護作業）
//! 版面比照 tiptop/aim/4fd/aimi110.4fd（IG1）：動作列＋上下筆＋單筆表單，分頁「主畫面」「工程／計劃／庫存」
//! 「會計／狀態／分群」「狀態」（ADR 0012 §9），下方為分群清單。第一階段 41 欄（IG3）。
//! 工廠層：用登入的營運中心（IG2）。規則在後端 ItemGroupService。
//! 查詢（item_group_query.feature）：進入時畫面空白，按「查詢」輸入條件後才載入，清單分頁（GQ2）。

use dioxus::prelude::*;
use rust_decimal::Decimal;
use ttrs_shared::{ItemGroupDto, ItemGroupInput, Page};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::history::local_time;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Chip, Dialog, Notification, NotificationKind, Select,
    Tabs, TextField, Tone, active_tone, icons,
};
use crate::components::permission::ProgramGate;
use crate::components::plant::PlantPicker;
use crate::components::qbe::{
    HINT_FLAG, HINT_NUMBER, ListPager, Q, Qbe, RecordNav, conditions_of,
    use_query_records_on_demand,
};
use crate::components::status::{HasStatus, StatusForm, TAB_STATUS};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

/// 權限檢查用的程式代號
const PROGRAM: &str = "item-group-maintenance";

/// 來源碼 imz08（mfg1001，aimi110.per:267；名稱見 ima_file.sch ima08）
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
    Query,
    New,
    Edit,
}

/// 查詢模式各分頁的欄位（GQ1）：(查詢鍵值, 標籤翻譯 ID, 提示：qbe-hint-* 為翻譯 ID，其餘照原文顯示)
type QField = (&'static str, &'static str, &'static str);

const Q_MAIN: &[QField] = &[
    ("group_code", "igroup-field-code", ""),
    ("description", "igroup-field-description", ""),
    ("source_code", "igroup-field-source", "P|M"),
    ("stock_unit", "igroup-field-stock-unit", ""),
    ("catalog_ref", "igroup-field-catalog-ref", ""),
    (
        "supplier_control",
        "igroup-field-supplier-control",
        "0|1|2|3",
    ),
    ("drawing_no", "igroup-field-drawing-no", ""),
    ("product_class", "item-field-product-class", ""),
];

const Q_PLANNING: &[QField] = &[
    ("software", "igroup-field-software", HINT_FLAG),
    ("engineering", "igroup-field-engineering", HINT_FLAG),
    (
        "insert_qpa_check",
        "igroup-field-insert-qpa-check",
        HINT_FLAG,
    ),
    ("insert_position", "igroup-field-insert-position", HINT_FLAG),
    ("coproduct", "igroup-field-coproduct", HINT_FLAG),
    ("inspection", "igroup-field-inspection", HINT_FLAG),
    ("repetitive", "igroup-field-repetitive", HINT_FLAG),
    ("consumable", "igroup-field-consumable", HINT_FLAG),
    ("material_category", "igroup-field-material-category", ""),
    ("abc_code", "igroup-field-abc", "A|B|C"),
    ("replenish_policy", "igroup-field-replenish", "0:5"),
    ("reorder_point", "igroup-field-reorder-point", HINT_NUMBER),
    ("reorder_qty", "igroup-field-reorder-qty", HINT_NUMBER),
    (
        "economic_order_qty",
        "item-field-economic-order-qty",
        HINT_NUMBER,
    ),
    (
        "average_order_qty",
        "item-field-average-order-qty",
        HINT_NUMBER,
    ),
    ("safety_stock", "igroup-field-safety-stock", HINT_NUMBER),
    ("safety_period", "igroup-field-safety-period", HINT_NUMBER),
    ("shelf_life_days", "igroup-field-shelf-life", HINT_NUMBER),
    ("mrp_bucket_days", "igroup-field-mrp-bucket", HINT_NUMBER),
    ("warehouse", "igroup-field-warehouse", ""),
    ("location", "igroup-field-location", ""),
    ("stock_keeper", "igroup-field-stock-keeper", ""),
    ("sales_stat_unit", "igroup-field-sales-stat-unit", ""),
    ("second_unit", "igroup-field-second-unit", ""),
    ("pricing_unit", "igroup-field-pricing-unit", ""),
];

const Q_ACCOUNTING: &[QField] = &[
    ("other_group1", "igroup-field-other-group1", ""),
    ("other_group2", "igroup-field-other-group2", ""),
    ("other_group3", "igroup-field-other-group3", ""),
    ("other_group4", "igroup-field-other-group4", ""),
    ("account", "igroup-field-account", ""),
    ("account2", "igroup-field-account2", ""),
    ("bonded", "igroup-field-bonded", HINT_FLAG),
    ("bonded_class", "igroup-field-bonded-class", ""),
    ("tariff_no", "igroup-field-tariff-no", ""),
    ("bonded_type", "igroup-field-bonded-type", "1|2|3"),
];

/// 查詢模式的條件輸入（一個分頁）
#[component]
fn QueryGrid(fields: &'static [QField], qbe: Qbe) -> Element {
    rsx! {
        div { class: "form-grid cols-4",
            for (field, label, hint) in fields.iter().copied() {
                Q {
                    key: "{field}",
                    field,
                    label: t(label),
                    hint_text: if hint.starts_with("qbe-hint") { t(hint) } else { hint.to_owned() },
                    query: true,
                    qbe,
                    div {}
                }
            }
        }
    }
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
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut rec = use_query_records_on_demand(move |req| {
        let p = plant();
        async move {
            if p.is_empty() {
                return Ok(Page {
                    items: Vec::new(),
                    total: 0,
                    page: 1,
                    page_size: crate::components::qbe::PAGE_SIZE,
                });
            }
            api::query_item_groups(&p, &req).await
        }
    });
    let mut mode = use_signal(|| Mode::View);
    let mut form = use_signal(ItemGroupInput::default);
    let tab = use_signal(|| 0usize);
    let mut copying = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let mut copy_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);

    // 換營運中心：回到空白畫面（GQ2）
    use_effect(move || {
        if plant().is_empty() {
            return;
        }
        mode.set(Mode::View);
        rec.reset(true);
    });

    // 執行查詢：先試查第 1 頁，條件有誤時留在查詢模式顯示錯誤
    let run_query = move |_| {
        let p = plant();
        let conds = conditions_of(&rec.qbe.read());
        let req = rec.request(conds.clone(), 1);
        spawn(async move {
            match api::query_item_groups(&p, &req).await {
                Ok(page) => {
                    error.set(None);
                    mode.set(Mode::View);
                    rec.apply(conds, page.total);
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

    let cur: Option<ItemGroupDto> = rec.current.read().clone();
    let no_cur = cur.is_none();
    let (total, pages) = rec.totals();
    let offset = rec.offset();
    let queried = (rec.active)();
    let qbe = rec.qbe;
    let m = mode();
    let q = m == Mode::Query;
    let ro = matches!(m, Mode::View | Mode::Query);
    let v = if ro {
        cur.as_ref().map(|c| c.fields.clone()).unwrap_or_default()
    } else {
        form()
    };
    let inactive = cur.as_ref().is_some_and(|c| !c.active);

    // 存檔或其他異動後：顯示該筆並重新載入清單
    let mut done = move |msg: String, g: Option<ItemGroupDto>| {
        error.set(None);
        message.set(Some(msg));
        mode.set(Mode::View);
        if g.is_none() {
            rec.pos.set(None);
        }
        rec.show(g);
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
                    done(tf("igroup-msg-saved", &[("code", code.as_str())]), Some(g));
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
                        Some(g),
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
    let cur_confirm = cur.clone();
    // 列印、匯出 Excel（aimi110_out）：已查詢時用查詢條件；尚未查詢但有目前這筆時印這一筆；都沒有時停用（取代 9057）
    let report_conditions: Option<Vec<ttrs_shared::QbeCondition>> = if queried {
        Some(rec.conditions.read().clone())
    } else {
        cur.as_ref().map(|c| {
            vec![ttrs_shared::QbeCondition {
                field: "group_code".into(),
                expr: format!("={}", c.fields.group_code),
            }]
        })
    };
    let can_print = report_conditions.is_some();
    let report = move |format: api::ReportFormat| {
        let Some(conds) = report_conditions.clone() else {
            return;
        };
        let tab = api::ReportTab::open(format);
        let p = plant();
        spawn(async move {
            if let Err(e) = api::list_report(
                &p,
                "item-groups",
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
    let units_hint = t("igroup-hint-unit");

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("igroup-breadcrumb")} }
                h2 { {t("igroup-title")} }
            }
            PlantPicker { plant }

            div { class: "action-bar",
                if q {
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                    Button {
                        icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text,
                        onclick: move |_| { error.set(None); mode.set(Mode::View); },
                        {t("common-action-cancel")}
                    }
                } else if ro {
                    MaintenanceToolbar {
                        query: Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            rec.qbe.set(Default::default());
                            mode.set(Mode::Query);
                        }),
                        insert: Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            form.set(ItemGroupInput {
                                source_code: Some("P".into()),
                                replenish_policy: Some("0".into()),
                                ..ItemGroupInput::default()
                            });
                            mode.set(Mode::New);
                        }),
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            message.set(None);
                            error.set(None);
                            form.set(c.to_input());
                            mode.set(Mode::Edit);
                        }),
                        copy: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            copy_code.set(String::new());
                            copy_error.set(None);
                            copying.set(Some(c.fields.group_code));
                        }),
                        // 已確認不可刪除（SA9-2）
                        delete: Act::new(!no_cur && !cur.as_ref().is_some_and(|c| c.confirmed), move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                let code = c.fields.group_code.clone();
                                match api::delete_item_group(&p, &code, c.version).await {
                                    Ok(()) => done(tf("igroup-msg-deleted", &[("code", code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::set_item_group_active(&p, &c.fields.group_code, !c.active, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "igroup-msg-activated" } else { "igroup-msg-deactivated" };
                                        let code = r.fields.group_code.clone();
                                        done(tf(id, &[("code", code.as_str())]), Some(r));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        inactive,
                        invalid_label: Some(("igroup-action-deactivate", "igroup-action-activate")),
                        // 確認／取消確認（ADR 0013 SA9）
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                match api::confirm_item_group(&p, &c.fields.group_code, !c.confirmed, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "igroup-msg-confirmed" } else { "igroup-msg-unconfirmed" };
                                        let code = r.fields.group_code.clone();
                                        done(tf(id, &[("code", code.as_str())]), Some(r));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed: cur.as_ref().is_some_and(|c| c.confirmed),
                        print: Act::new(can_print, {
                            let report = report.clone();
                            move |_| report(api::ReportFormat::Pdf)
                        }),
                        export: Act::new(can_print, move |_| report(api::ReportFormat::Excel)),
                        history: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_history.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                let code = c.fields.group_code.clone();
                                match api::item_group_history(&p, &code).await {
                                    Ok(h) => history.set(Some((code, h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
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

            if q {
                p { class: "query-hint", {t("query-hint")} }
            }

            if m == Mode::View && total > 0 {
                RecordNav { total, pos: (rec.pos)(), ongo: move |i| { message.set(None); rec.go(i); }, onerror: move |e| error.set(Some(e)) }
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
                    0 if q => rsx! { QueryGrid { fields: Q_MAIN, qbe } },
                    1 if q => rsx! { QueryGrid { fields: Q_PLANNING, qbe } },
                    2 if q => rsx! { QueryGrid { fields: Q_ACCOUNTING, qbe } },
                    0 => rsx! {
                        div { class: "form-grid",
                            TextField { id: "imz01", label: t("igroup-field-code"), maxlength: 10usize, value: v.group_code.clone(), readonly: m != Mode::New, oninput: move |x| form.write().group_code = x }
                            TextField { id: "imz02", label: t("igroup-field-description"), maxlength: 80usize, value: txt(&v.description), readonly: ro, oninput: move |x| form.write().description = opt(x) }
                            Select { id: "imz08", label: t("igroup-field-source"), options: options(&SOURCES, "igroup-source"), value: txt(&v.source_code), readonly: ro, onchange: move |x| form.write().source_code = opt(x) }
                            TextField { id: "imz25", label: t("igroup-field-stock-unit"), maxlength: 4usize, value: txt(&v.stock_unit), readonly: ro, oninput: move |x| form.write().stock_unit = opt(x) }
                            TextField { id: "imz03", label: t("igroup-field-catalog-ref"), maxlength: 20usize, value: txt(&v.catalog_ref), readonly: ro, oninput: move |x| form.write().catalog_ref = opt(x) }
                            TextField { id: "imz72", label: t("igroup-field-supplier-control"), maxlength: 1usize, value: txt(&v.supplier_control), readonly: ro, oninput: move |x| form.write().supplier_control = opt(x) }
                            TextField { id: "imz04", label: t("igroup-field-drawing-no"), maxlength: 20usize, value: txt(&v.drawing_no), readonly: ro, oninput: move |x| form.write().drawing_no = opt(x) }
                            TextField { id: "imz131", label: t("item-field-product-class"), maxlength: 10usize, value: txt(&v.product_class), readonly: ro, oninput: move |x| form.write().product_class = opt(x) }
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
                            TextField { id: "imz51", label: t("item-field-economic-order-qty"), input_type: "number", value: dec_text(v.economic_order_qty), readonly: ro, oninput: move |x| form.write().economic_order_qty = dec(x) }
                            TextField { id: "imz52", label: t("item-field-average-order-qty"), input_type: "number", value: dec_text(v.average_order_qty), readonly: ro, oninput: move |x| form.write().average_order_qty = dec(x) }
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
                        StatusForm { status: cur.as_ref().filter(|_| m != Mode::New).map(|c| c.status()), queryable: crate::components::status::ALL, query: q, qbe, prefix: "imz",
                            // 確認狀態（ADR 0013 SA9）
                            TextField { id: "imz_confirmed", label: t("common-field-confirmed"), readonly: true, value: match cur.as_ref().filter(|_| m != Mode::New) { Some(c) if c.confirmed => t("common-yes"), Some(_) => t("common-no"), None => String::new() } }
                            TextField { id: "imz_confirmed_by", label: t("common-field-confirmed-by"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).and_then(|c| c.confirmed_by.clone()).unwrap_or_default() }
                            TextField { id: "imz_confirmed_at", label: t("common-field-confirmed-at"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).and_then(|c| c.confirmed_at).map(local_time).unwrap_or_default() }
                        }
                        if q {
                            div { class: "form-grid cols-4",
                                Q { field: "confirmed", label: t("common-field-confirmed"), hint: HINT_FLAG, query: true, qbe, div {} }
                                Q { field: "confirmed_by", label: t("common-field-confirmed-by"), query: true, qbe, div {} }
                            }
                        }
                    },
                  }
                }
            }

            // ----- 分群清單 -----
            Card {
                div { class: "table-header",
                    h3 { {t("igroup-list-title")} }
                    if queried {
                        p { {tf("list-count-filtered", &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                    } else {
                        p { class: "muted", {t("list-msg-not-queried")} }
                    }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for id in ["igroup-field-code", "igroup-field-description", "igroup-field-source", "igroup-field-stock-unit", "common-field-active"] {
                                th { key: "{id}", {t(id)} }
                            }
                        } }
                        tbody {
                            match &*rec.list.read() {
                                None => rsx! { tr { td { colspan: 5, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 5, "{e}" } } },
                                Some(Ok(page)) if page.items.is_empty() => rsx! { tr { td { colspan: 5, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(page)) => rsx! {
                                    for (k, g) in page.items.iter().cloned().enumerate() {
                                        tr {
                                            key: "{g.fields.group_code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|c| c.fields.group_code == g.fields.group_code) { "md-table__row--selected" },
                                            onclick: move |_| if mode() == Mode::View { message.set(None); error.set(None); rec.go(offset + k as u64); },
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
                if queried {
                    ListPager { page: rec.page, pages }
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
