//! Item Maintenance 料件基本資料維護（TipTop aimi100 料件基本資料建立，以客製 topcust/cim 為準）
//! 版面比照 topcust/cim/4fd/aimi100.4fd（IM1）：動作列＋上下筆＋單筆表單，分頁「主畫面」「工程／計劃／庫存」
//! 「會計／狀態／分群」「料件狀態」「參考日期」「狀態」（ADR 0012 §9），下方為料件清單。
//! 查詢（item_maintenance_query.feature）：進入時畫面空白，按「查詢」輸入條件後才載入，清單分頁（IQ6）；
//! 網址帶 `?item_no=` 時直接查出該筆（IQ7）。
//! 品名、規格多語言（IM7）；分群碼帶入預設值由使用者按鈕帶入（IM25）。規則在後端 ItemService。

use dioxus::prelude::*;
use rust_decimal::Decimal;
use ttrs_shared::{
    CopyItemRequest, ItemDto, ItemInput, ItemTextDto, Page, QbeCondition, item_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Chip, Dialog, Notification, NotificationKind, Select,
    Tabs, TextField, Tone, icons,
};
use crate::components::multilingual::{LangField, LangValues, MultilingualDialogField};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::components::qbe::{
    HINT_DATE, HINT_FLAG, HINT_NUMBER, ListPager, Q, Qbe, RecordNav, conditions_of,
    use_query_records_on_demand,
};
use crate::components::status::{HasStatus, StatusForm, TAB_STATUS};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{UiLang, t, tf};

/// 權限檢查用的程式代號
const PROGRAM: &str = "item-maintenance";

/// 來源碼 ima08（mfg1001）
const SOURCES: [&str; 13] = [
    "C", "T", "D", "A", "M", "P", "X", "K", "U", "V", "R", "Z", "S",
];
/// 補貨策略碼 ima37（IM23：0–5）
const REPLENISH: [&str; 6] = ["0", "1", "2", "3", "4", "5"];
/// 供應商管制 ima915
const SUPPLIER_CONTROL: [&str; 4] = ["0", "1", "2", "3"];
/// 產品週期狀態 ima1014
const LIFE_CYCLE: [&str; 6] = ["1", "2", "3", "4", "5", "6"];

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    View,
    Query,
    New,
    Edit,
}

/// 查詢模式各分頁的欄位（IQ1）：(查詢鍵值, 標籤翻譯 ID, 提示：qbe-hint-* 為翻譯 ID，其餘照原文顯示)
type QField = (&'static str, &'static str, &'static str);

const Q_MAIN: &[QField] = &[
    ("item_no", "item-field-item-no", ""),
    ("active_code", "item-field-active-code", "P|Y|N"),
    ("name", "item-field-name", ""),
    ("spec", "item-field-spec", ""),
    ("group_code", "item-field-group", ""),
    ("source_code", "item-field-source", "P|M"),
    ("configurable_item_no", "item-field-configurable", ""),
    ("current_version", "item-field-current-version", ""),
    ("catalog_ref", "item-field-catalog-ref", ""),
    ("supplier_control", "item-field-supplier-control", "0|1|2|3"),
    ("product_class", "item-field-product-class", ""),
    ("drawing_no", "item-field-drawing-no", ""),
    ("apply_dept", "item-field-apply-dept", ""),
    ("status_code", "item-field-status-code", "0|1"),
    ("order_invalid", "item-field-order-invalid", HINT_FLAG),
    ("phase_out", "item-field-phase-out", HINT_FLAG),
    ("new_part_status", "item-field-new-part-status", ""),
    ("data_source", "item-field-data-source", ""),
];

const Q_PLANNING: &[QField] = &[
    ("software", "igroup-field-software", HINT_FLAG),
    ("engineering", "igroup-field-engineering", HINT_FLAG),
    ("insert_position", "igroup-field-insert-position", HINT_FLAG),
    (
        "insert_qpa_check",
        "igroup-field-insert-qpa-check",
        HINT_FLAG,
    ),
    ("coproduct", "igroup-field-coproduct", HINT_FLAG),
    ("inspection", "igroup-field-inspection", HINT_FLAG),
    ("repetitive", "igroup-field-repetitive", HINT_FLAG),
    ("consumable", "igroup-field-consumable", HINT_FLAG),
    ("material_category", "igroup-field-material-category", ""),
    ("pcn", "item-field-pcn", ""),
    ("abc_code", "igroup-field-abc", "A|B|C"),
    ("replenish_policy", "igroup-field-replenish", "0:5"),
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
    ("max_stock_qty", "item-field-max-stock-qty", HINT_NUMBER),
    ("shelf_life_days", "igroup-field-shelf-life", HINT_NUMBER),
    ("stock_unit", "igroup-field-stock-unit", ""),
    ("warehouse", "igroup-field-warehouse", ""),
    ("location", "igroup-field-location", ""),
    ("stock_keeper", "igroup-field-stock-keeper", ""),
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
    ("bom_changed", "item-field-bom-changed", ""),
    ("low_level_code", "item-field-low-level-code", HINT_NUMBER),
    (
        "coproduct_this_month",
        "item-field-coproduct-this-month",
        HINT_FLAG,
    ),
];

const Q_ITEM_STATUS: &[QField] = &[
    ("full_name", "item-field-full-name", ""),
    ("english_name", "item-field-english-name", ""),
    ("first_shipped_on", "item-field-first-shipped-on", HINT_DATE),
    (
        "first_received_on",
        "item-field-first-received-on",
        HINT_DATE,
    ),
    (
        "planned_discontinue_on",
        "item-field-planned-discontinue-on",
        HINT_DATE,
    ),
    ("life_cycle", "item-field-life-cycle", "1:6"),
    ("carton_barcode", "item-field-carton-barcode", ""),
];

const Q_DATES: &[QField] = &[
    ("item_created_on", "item-field-item-created-on", HINT_DATE),
    ("slow_moving_on", "item-field-slow-moving-on", HINT_DATE),
    (
        "last_period_purchase_on",
        "item-field-last-period-purchase-on",
        HINT_DATE,
    ),
    ("last_received_on", "item-field-last-received-on", HINT_DATE),
    ("last_issued_on", "item-field-last-issued-on", HINT_DATE),
    ("last_txn_on", "item-field-last-txn-on", HINT_DATE),
    ("last_count_on", "item-field-last-count-on", HINT_DATE),
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

/// 清單顯示的品名、規格：目前介面語系 → 繁中（ADR 0005）
fn shown(d: &ItemDto, pick: fn(&ItemTextDto) -> Option<String>, base: &Option<String>) -> String {
    let lang = crate::i18n::current().code();
    d.texts
        .iter()
        .find(|x| x.lang == lang)
        .and_then(pick)
        .or_else(|| base.clone())
        .unwrap_or_default()
}

/// 料號當作字面值查詢：QBE 的特殊字元都加上跳脫（ttrs_domain::qbe）
fn qbe_literal(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        if matches!(c, '\\' | '*' | '?' | '|' | ':' | '<' | '>' | '=' | '!') {
            out.push('\\');
        }
        out.push(c);
    }
    out
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

fn date_text(v: Option<ttrs_shared::NaiveDate>) -> String {
    v.map(|d| d.format("%Y/%m/%d").to_string())
        .unwrap_or_default()
}

/// 下拉選項：（可選的空白）＋「代碼 說明」；說明的翻譯 ID 為 `<prefix>-<代碼小寫>`
fn options(codes: &[&str], prefix: &str, blank: bool) -> Vec<(String, String)> {
    let mut v = Vec::new();
    if blank {
        v.push((String::new(), String::new()));
    }
    v.extend(codes.iter().map(|c| {
        (
            (*c).to_owned(),
            format!("{c} {}", t(&format!("{prefix}-{}", c.to_lowercase()))),
        )
    }));
    v
}

/// 資料有效碼的翻譯 ID
fn active_id(code: &str) -> &'static str {
    match code {
        "Y" => "item-active-confirmed",
        "N" => "item-active-invalid",
        _ => "item-active-processing",
    }
}

fn active_tone(code: &str) -> Tone {
    match code {
        "Y" => Tone::Success,
        "N" => Tone::Error,
        _ => Tone::Tertiary,
    }
}

/// 各語系的品名、規格（zh-TW 為料件本身）
fn text_values(d: &ItemDto) -> LangValues {
    let mut v = LangValues::default();
    v.set(UiLang::ZhTw, "name", txt(&d.fields.name));
    v.set(UiLang::ZhTw, "spec", txt(&d.fields.spec));
    for x in &d.texts {
        if let Some(l) = UiLang::from_code(&x.lang) {
            v.set(l, "name", txt(&x.name));
            v.set(l, "spec", txt(&x.spec));
        }
    }
    v
}

#[component]
pub fn ItemMaintenance(item_no: String) -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ItemPage { item_no } }
    }
}

#[component]
fn ItemPage(item_no: String) -> Element {
    let perm = use_context::<Perm>();
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
            api::query_items(&p, &req).await
        }
    });
    let mut mode = use_signal(|| Mode::View);
    let mut form = use_signal(ItemInput::default);
    let mut texts = use_signal(LangValues::default);
    let mut loaded_texts = use_signal(LangValues::default);
    let tab = use_signal(|| 0usize);
    let mut copying = use_signal(|| None::<String>);
    let mut copy_no = use_signal(String::new);
    let mut copy_name = use_signal(String::new);
    let mut copy_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);
    // 網址帶入的料號，選定營運中心後查一次（IQ7）
    let mut pending_no = use_signal(move || Some(item_no).filter(|v| !v.is_empty()));

    // 執行查詢：先試查第 1 頁，條件有誤時留在查詢模式顯示錯誤
    let start_query = move |conds: Vec<QbeCondition>| {
        let p = plant();
        let req = rec.request(conds.clone(), 1);
        spawn(async move {
            match api::query_items(&p, &req).await {
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

    // 換營運中心：回到空白畫面；網址帶料號時直接查該筆（選擇器可能重設同一個營運中心，值沒變時不清空）
    let mut shown_plant = use_signal(String::new);
    use_effect(move || {
        let p = plant();
        if p.is_empty() || *shown_plant.peek() == p {
            return;
        }
        let changed = !shown_plant.peek().is_empty();
        shown_plant.set(p);
        if changed {
            mode.set(Mode::View);
            rec.reset(true);
        }
        if let Some(no) = pending_no.take() {
            start_query(vec![QbeCondition {
                field: "item_no".into(),
                expr: qbe_literal(&no),
            }]);
        }
    });

    // 換筆時各語系品名跟著目前這一筆
    use_effect(move || {
        let c = rec.current.read().clone();
        if *mode.peek() == Mode::View {
            let v = c.as_ref().map(text_values).unwrap_or_default();
            texts.set(v.clone());
            loaded_texts.set(v);
        }
    });

    let cur: Option<ItemDto> = rec.current.read().clone();
    let no_cur = cur.is_none();
    let (total, pages) = rec.totals();
    let offset = rec.offset();
    let queried = (rec.active)();
    let m = mode();
    let q = m == Mode::Query;
    let ro = matches!(m, Mode::View | Mode::Query);
    let qbe = rec.qbe;
    let v = if ro {
        cur.as_ref().map(|c| c.fields.clone()).unwrap_or_default()
    } else {
        form()
    };
    let active_code = cur
        .as_ref()
        .map(|c| c.active_code.clone())
        .unwrap_or_default();
    let confirmed = active_code == "Y";
    let invalid = active_code == "N";

    // 存檔或其他異動後：重新讀出該筆（含各語系品名）顯示，並重新載入清單
    let mut done = move |msg: String, no: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        mode.set(Mode::View);
        let Some(no) = no else {
            rec.pos.set(None);
            rec.show(None);
            return;
        };
        let p = plant();
        spawn(async move {
            match api::get_item(&p, &no).await {
                Ok(d) => {
                    let v = text_values(&d);
                    texts.set(v.clone());
                    loaded_texts.set(v);
                    rec.show(Some(d));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let save = move |_| {
        let p = plant();
        let m = mode();
        let values = texts();
        let before = loaded_texts();
        let mut req = form();
        req.name = values.opt(UiLang::ZhTw, "name");
        req.spec = values.opt(UiLang::ZhTw, "spec");
        spawn(async move {
            let result = if m == Mode::Edit {
                api::update_item(&p, &req.item_no.clone(), &req).await
            } else {
                api::create_item(&p, &req).await
            };
            let saved = match result {
                Ok(d) => d,
                Err(e) => {
                    error.set(Some(e));
                    return;
                }
            };
            let no = saved.fields.item_no.clone();
            // 其他語系有異動的才送（IM7）；zh-TW 已隨料件儲存
            for lang in values.changed(&before) {
                if lang == UiLang::ZhTw {
                    continue;
                }
                let t = ItemTextDto {
                    lang: lang.code().to_owned(),
                    name: values.opt(lang, "name"),
                    spec: values.opt(lang, "spec"),
                };
                if let Err(e) = api::set_item_text(&p, &no, &t).await {
                    error.set(Some(e));
                    return;
                }
            }
            done(tf("item-msg-saved", &[("no", no.as_str())]), Some(no));
        });
    };

    // 分群碼帶入預設值（IM25）：料號、品名、規格與版本保留
    let apply_group = move |_| {
        let p = plant();
        let Some(group) = form().group_code.clone() else {
            error.set(Some(t("item-msg-group-required")));
            return;
        };
        spawn(async move {
            match api::item_group_defaults(&p, &group).await {
                Ok(d) => {
                    let cur = form();
                    form.set(ItemInput {
                        item_no: cur.item_no,
                        name: cur.name,
                        spec: cur.spec,
                        current_version: cur.current_version,
                        version: cur.version,
                        ..d
                    });
                    message.set(Some(tf(
                        "item-msg-group-applied",
                        &[("group", group.as_str())],
                    )));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let act = move |action: &'static str, ok_id: &'static str| {
        let Some(c) = rec.current.peek().clone() else {
            return;
        };
        let p = plant();
        spawn(async move {
            match api::item_action(&p, &c.fields.item_no, action, c.version).await {
                Ok(r) => {
                    let no = r.fields.item_no.clone();
                    done(tf(ok_id, &[("no", no.as_str())]), Some(no));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let do_copy = move |_| {
        let p = plant();
        let Some(from) = copying() else { return };
        let req = CopyItemRequest {
            new_item_no: copy_no().trim().to_owned(),
            name: opt(copy_name()),
        };
        spawn(async move {
            match api::copy_item(&p, &from, &req).await {
                Ok(d) => {
                    copying.set(None);
                    copy_error.set(None);
                    let no = d.fields.item_no.clone();
                    done(
                        tf(
                            "item-msg-copied",
                            &[("from", from.as_str()), ("to", no.as_str())],
                        ),
                        Some(no),
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
    // 列印、匯出 Excel（i100_out）：已查詢時用查詢條件；尚未查詢但有目前這筆時印這一筆、排除無效料件（客製）；
    // 兩者都沒有時停用（取代 9057 請先查詢）
    let report_conditions: Option<Vec<QbeCondition>> = if queried {
        Some(rec.conditions.read().clone())
    } else {
        cur.as_ref().map(|c| {
            vec![
                QbeCondition {
                    field: "item_no".into(),
                    expr: qbe_literal(&c.fields.item_no),
                },
                QbeCondition {
                    field: "active_code".into(),
                    expr: "<>N".into(),
                },
            ]
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
                "items",
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

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("item-breadcrumb")} }
                h2 { {t("item-title")} }
            }
            PlantPicker { plant }

            div { class: "action-bar",
                if q {
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: move |_| start_query(conditions_of(&rec.qbe.read())), {t("common-action-ok")} }
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
                            form.set(ItemInput::default());
                            texts.set(LangValues::default());
                            loaded_texts.set(LangValues::default());
                            mode.set(Mode::New);
                        }),
                        modify: Act::new(!no_cur && !invalid, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            message.set(None);
                            error.set(None);
                            form.set(c.to_input());
                            mode.set(Mode::Edit);
                        }),
                        copy: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            copy_no.set(String::new());
                            copy_name.set(txt(&c.fields.name));
                            copy_error.set(None);
                            copying.set(Some(c.fields.item_no));
                        }),
                        // 只有處理中的料件可刪除
                        delete: Act::new(!no_cur && active_code == "P", move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                let no = c.fields.item_no.clone();
                                match api::delete_item(&p, &no, c.version).await {
                                    Ok(()) => done(tf("item-msg-deleted", &[("no", no.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 確認 confirm／取消確認 notconfirm（IM-16～IM-18）：無效的料件不能確認
                        confirm: Act::new(!no_cur && (confirmed || !invalid), move |_| {
                            if confirmed {
                                act("unconfirm", "item-msg-unconfirmed");
                            } else {
                                act("confirm", "item-msg-confirmed");
                            }
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| act("void", if invalid { "item-msg-restored" } else { "item-msg-voided" })),
                        inactive: invalid,
                        invalid_label: Some(("item-action-void", "item-action-restore")),
                        print: Act::new(can_print, {
                            let report = report.clone();
                            move |_| report(api::ReportFormat::Pdf)
                        }),
                        export: Act::new(can_print, move |_| report(api::ReportFormat::Excel)),
                        history: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_history.clone() else { return };
                            let p = plant();
                            spawn(async move {
                                let no = c.fields.item_no.clone();
                                match api::item_history(&p, &no).await {
                                    Ok(h) => history.set(Some((no, h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        if perm.can("order_invalid") {
                            Button {
                                icon: icons::SYNC, tone: Tone::Primary,
                                variant: ButtonVariant::Outlined,
                                disabled: no_cur,
                                onclick: move |_| act("order-invalid", "item-msg-order-invalid"),
                                {t("item-action-order-invalid")}
                            }
                        }
                    }
                } else {
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {t("common-action-ok")} }
                    Button {
                        icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text,
                        onclick: move |_| {
                            error.set(None);
                            texts.set(loaded_texts());
                            mode.set(Mode::View);
                        },
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

            // ----- 單筆表單（aimi100.4fd） -----
            Card {
                Tabs {
                    tabs: vec![
                        t("item-tab-main"),
                        t("item-tab-planning"),
                        t("item-tab-accounting"),
                        t("item-tab-item-status"),
                        t("item-tab-dates"),
                        t(TAB_STATUS),
                    ],
                    active: tab,
                }
                div { class: "card-body",
                  match tab() {
                    0 if q => rsx! { QueryGrid { fields: Q_MAIN, qbe } },
                    1 if q => rsx! { QueryGrid { fields: Q_PLANNING, qbe } },
                    2 if q => rsx! { QueryGrid { fields: Q_ACCOUNTING, qbe } },
                    3 if q => rsx! { QueryGrid { fields: Q_ITEM_STATUS, qbe } },
                    4 if q => rsx! { QueryGrid { fields: Q_DATES, qbe } },
                    0 => rsx! {
                        div { class: "form-grid",
                            TextField { id: "ima01", label: t("item-field-item-no"), maxlength: max_len::ITEM_NO, value: v.item_no.clone(), readonly: m != Mode::New, oninput: move |x| form.write().item_no = x }
                            TextField {
                                id: "imaacti",
                                label: t("item-field-active-code"),
                                readonly: true,
                                value: if m == Mode::New { t("item-active-processing") } else if no_cur { String::new() } else { t(active_id(&active_code)) },
                            }
                            MultilingualDialogField {
                                id: "ima02",
                                class: "span-2",
                                field: LangField::text("name", t("item-field-name"), max_len::NAME),
                                values: texts,
                                readonly: ro,
                            }
                            MultilingualDialogField {
                                id: "ima021",
                                class: "span-2",
                                field: LangField::text("spec", t("item-field-spec"), max_len::SPEC),
                                values: texts,
                                readonly: ro,
                            }
                            TextField { id: "ima06", label: t("item-field-group"), maxlength: max_len::GROUP, value: txt(&v.group_code), readonly: ro, oninput: move |x| form.write().group_code = opt(x) }
                            if ro {
                                TextField { id: "imz02", label: t("item-field-group-name"), readonly: true, value: cur.as_ref().and_then(|c| c.group_description.clone()).unwrap_or_default() }
                            } else {
                                div { class: "field-action",
                                    Button {
                                        icon: icons::DOWNLOAD, tone: Tone::Primary,
                                        variant: ButtonVariant::Tonal,
                                        onclick: apply_group,
                                        {t("item-action-apply-group")}
                                    }
                                }
                            }
                            Select { id: "ima08", label: t("item-field-source"), options: options(&SOURCES, "igroup-source", false), value: v.source_code.clone().unwrap_or_else(|| "P".into()), readonly: ro, onchange: move |x| form.write().source_code = opt(x) }
                            TextField { id: "ima13", label: t("item-field-configurable"), maxlength: max_len::ITEM_NO, value: txt(&v.configurable_item_no), readonly: ro || v.source_code.as_deref() != Some("T"), oninput: move |x| form.write().configurable_item_no = opt(x) }
                            TextField { id: "ima05", label: t("item-field-current-version"), maxlength: max_len::VERSION, value: txt(&v.current_version), readonly: ro, oninput: move |x| form.write().current_version = opt(x) }
                            TextField { id: "ima03", label: t("item-field-catalog-ref"), maxlength: max_len::CATALOG_REF, value: txt(&v.catalog_ref), readonly: ro, oninput: move |x| form.write().catalog_ref = opt(x) }
                            Select { id: "ima915", label: t("item-field-supplier-control"), options: options(&SUPPLIER_CONTROL, "item-supplier-control", false), value: v.supplier_control.clone().unwrap_or_else(|| "0".into()), readonly: ro, onchange: move |x| form.write().supplier_control = opt(x) }
                            TextField { id: "ima131", label: t("item-field-product-class"), maxlength: max_len::CODE10, value: txt(&v.product_class), readonly: ro, oninput: move |x| form.write().product_class = opt(x) }
                            TextField { id: "oba02", label: t("item-field-product-class-name"), readonly: true, value: if ro { cur.as_ref().and_then(|c| c.product_class_name.clone()).unwrap_or_default() } else { String::new() } }
                            TextField { id: "ima04", class: "span-2", label: t("item-field-drawing-no"), maxlength: max_len::DRAWING_NO, value: txt(&v.drawing_no), readonly: ro, oninput: move |x| form.write().drawing_no = opt(x) }
                            TextField { id: "ta_ima08", label: t("item-field-apply-dept"), maxlength: max_len::CODE10, value: txt(&v.apply_dept), readonly: ro, oninput: move |x| form.write().apply_dept = opt(x) }
                            TextField { id: "gem02", label: t("item-field-apply-dept-name"), readonly: true, value: if ro { cur.as_ref().and_then(|c| c.apply_dept_name.clone()).unwrap_or_default() } else { String::new() } }
                            TextField { id: "ima1010", label: t("item-field-status-code"), readonly: true, value: match cur.as_ref().filter(|_| m != Mode::New).map(|c| c.status_code.as_str()) { Some("1") => t("item-status-confirmed"), _ => t("item-status-open") } }
                            TextField { id: "ta_ima01", label: t("item-field-order-invalid"), readonly: true, value: if cur.as_ref().filter(|_| m != Mode::New).is_none_or(|c| c.order_invalid) { t("common-yes") } else { t("common-no") } }
                            TextField { id: "ima140", label: t("item-field-phase-out"), readonly: true, value: if cur.as_ref().is_some_and(|c| c.phase_out) && m != Mode::New { t("common-yes") } else { t("common-no") } }
                            TextField { id: "ima93", label: t("item-field-new-part-status"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).map(|c| c.new_part_status.clone()).unwrap_or_default() }
                            TextField { id: "ima916", label: t("item-field-data-source"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).map(|c| c.data_source.clone()).unwrap_or_default() }
                        }
                    },
                    1 => rsx! {
                        div { class: "form-grid cols-4",
                            Checkbox { id: "ima105", label: t("igroup-field-software"), checked: v.software, readonly: ro, onchange: move |b| form.write().software = b }
                            Checkbox { id: "ima14", label: t("igroup-field-engineering"), checked: v.engineering, readonly: ro, onchange: move |b| form.write().engineering = b }
                            Checkbox { id: "ima107", label: t("igroup-field-insert-position"), checked: v.insert_position, readonly: ro, onchange: move |b| form.write().insert_position = b }
                            Checkbox { id: "ima147", label: t("igroup-field-insert-qpa-check"), checked: v.insert_qpa_check, readonly: ro, onchange: move |b| form.write().insert_qpa_check = b }
                            Checkbox { id: "ima903", label: t("igroup-field-coproduct"), checked: v.coproduct, readonly: ro, onchange: move |b| form.write().coproduct = b }
                            Checkbox { id: "ima24", label: t("igroup-field-inspection"), checked: v.inspection, readonly: ro, onchange: move |b| form.write().inspection = b }
                            Checkbox { id: "ima911", label: t("igroup-field-repetitive"), checked: v.repetitive, readonly: ro, onchange: move |b| form.write().repetitive = b }
                            Checkbox { id: "ima70", label: t("igroup-field-consumable"), checked: v.consumable, readonly: ro, onchange: move |b| form.write().consumable = b }
                        }
                        div { class: "form-grid cols-4",
                            TextField { id: "ima109", label: t("igroup-field-material-category"), maxlength: max_len::CODE10, value: txt(&v.material_category), readonly: ro, oninput: move |x| form.write().material_category = opt(x) }
                            TextField { id: "ta_ima02", label: t("item-field-pcn"), maxlength: max_len::PCN, value: txt(&v.pcn), readonly: ro, oninput: move |x| form.write().pcn = opt(x) }
                            Select { id: "ima07", label: t("igroup-field-abc"), options: vec![("A".into(), "A".into()), ("B".into(), "B".into()), ("C".into(), "C".into())], value: v.abc_code.clone().unwrap_or_else(|| "A".into()), readonly: ro, onchange: move |x| form.write().abc_code = opt(x) }
                            Select { id: "ima37", label: t("igroup-field-replenish"), options: options(&REPLENISH, "igroup-replenish", true), value: txt(&v.replenish_policy), readonly: ro, onchange: move |x| form.write().replenish_policy = opt(x) }
                            TextField { id: "ima51", label: t("item-field-economic-order-qty"), input_type: "number", value: dec_text(v.economic_order_qty), readonly: ro, oninput: move |x| form.write().economic_order_qty = dec(x) }
                            TextField { id: "ima52", label: t("item-field-average-order-qty"), input_type: "number", value: dec_text(v.average_order_qty), readonly: ro, oninput: move |x| form.write().average_order_qty = dec(x) }
                            TextField { id: "ima27", label: t("igroup-field-safety-stock"), input_type: "number", value: dec_text(v.safety_stock), readonly: ro, oninput: move |x| form.write().safety_stock = dec(x) }
                            TextField { id: "ima28", label: t("igroup-field-safety-period"), input_type: "number", value: dec_text(v.safety_period), readonly: ro, oninput: move |x| form.write().safety_period = dec(x) }
                            TextField { id: "ima271", label: t("item-field-max-stock-qty"), input_type: "number", value: dec_text(v.max_stock_qty), readonly: ro, oninput: move |x| form.write().max_stock_qty = dec(x) }
                            TextField { id: "ima71", label: t("igroup-field-shelf-life"), input_type: "number", value: v.shelf_life_days.map(|n| n.to_string()).unwrap_or_default(), readonly: ro, oninput: move |x: String| form.write().shelf_life_days = x.trim().parse().ok() }
                            TextField { id: "ima25", label: t("igroup-field-stock-unit"), maxlength: max_len::UNIT, value: txt(&v.stock_unit), readonly: ro, oninput: move |x| form.write().stock_unit = opt(x) }
                            TextField { id: "ima35", label: t("igroup-field-warehouse"), maxlength: max_len::CODE10, value: txt(&v.warehouse), readonly: ro, oninput: move |x| form.write().warehouse = opt(x) }
                            TextField { id: "ima36", label: t("igroup-field-location"), maxlength: max_len::CODE10, value: txt(&v.location), readonly: ro, oninput: move |x| form.write().location = opt(x) }
                            TextField { id: "ima23", label: t("igroup-field-stock-keeper"), maxlength: max_len::CODE10, value: txt(&v.stock_keeper), readonly: ro, oninput: move |x| form.write().stock_keeper = opt(x) }
                            TextField { id: "gen02", label: t("igroup-field-stock-keeper-name"), readonly: true, value: if ro { cur.as_ref().and_then(|c| c.stock_keeper_name.clone()).unwrap_or_default() } else { String::new() } }
                            TextField { id: "ima906", label: t("item-field-unit-usage"), readonly: true, value: t("item-unit-usage-1") }
                            TextField { id: "ima907", label: t("igroup-field-second-unit"), maxlength: max_len::UNIT, value: txt(&v.second_unit), readonly: ro, oninput: move |x| form.write().second_unit = opt(x) }
                            TextField { id: "ima908", label: t("igroup-field-pricing-unit"), maxlength: max_len::UNIT, value: txt(&v.pricing_unit), readonly: ro, oninput: move |x| form.write().pricing_unit = opt(x) }
                        }
                        p { class: "muted", {t("item-hint-planning")} }
                    },
                    2 => rsx! {
                        div { class: "form-grid cols-4",
                            TextField { id: "ima09", label: t("igroup-field-other-group1"), maxlength: max_len::CODE10, value: txt(&v.other_group1), readonly: ro, oninput: move |x| form.write().other_group1 = opt(x) }
                            TextField { id: "ima10", label: t("igroup-field-other-group2"), maxlength: max_len::CODE10, value: txt(&v.other_group2), readonly: ro, oninput: move |x| form.write().other_group2 = opt(x) }
                            TextField { id: "ima11", label: t("igroup-field-other-group3"), maxlength: max_len::CODE10, value: txt(&v.other_group3), readonly: ro, oninput: move |x| form.write().other_group3 = opt(x) }
                            TextField { id: "ima12", label: t("igroup-field-other-group4"), maxlength: max_len::CODE10, value: txt(&v.other_group4), readonly: ro, oninput: move |x| form.write().other_group4 = opt(x) }
                            TextField { id: "ima39", label: t("igroup-field-account"), maxlength: max_len::ACCOUNT, value: txt(&v.account), readonly: ro, oninput: move |x| form.write().account = opt(x) }
                            TextField { id: "ima391", label: t("igroup-field-account2"), maxlength: max_len::ACCOUNT, value: txt(&v.account2), readonly: ro, oninput: move |x| form.write().account2 = opt(x) }
                            Checkbox { id: "ima15", label: t("igroup-field-bonded"), checked: v.bonded, readonly: ro, onchange: move |b| form.write().bonded = b }
                            TextField { id: "ima146", label: t("item-field-bom-changed"), readonly: true, value: cur.as_ref().and_then(|c| c.bom_changed.clone()).unwrap_or_default() }
                            TextField { id: "ima16", label: t("item-field-low-level-code"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).map(|c| c.low_level_code.to_string()).unwrap_or_default() }
                            TextField { id: "ima905", label: t("item-field-coproduct-this-month"), readonly: true, value: if cur.as_ref().is_some_and(|c| c.coproduct_this_month) && m != Mode::New { t("common-yes") } else { t("common-no") } }
                        }
                        p { class: "muted", {t("igroup-hint-accounting")} }
                    },
                    3 => rsx! {
                        div { class: "form-grid",
                            TextField { id: "ima1001", class: "span-2", label: t("item-field-full-name"), maxlength: max_len::FULL_NAME, value: txt(&v.full_name), readonly: ro, oninput: move |x| form.write().full_name = opt(x) }
                            TextField { id: "ima1002", class: "span-2", label: t("item-field-english-name"), maxlength: max_len::ENGLISH_NAME, value: txt(&v.english_name), readonly: ro, oninput: move |x| form.write().english_name = opt(x) }
                            TextField { id: "ima1012", label: t("item-field-first-shipped-on"), readonly: true, value: date_text(cur.as_ref().and_then(|c| c.first_shipped_on)) }
                            TextField { id: "ima1013", label: t("item-field-first-received-on"), readonly: true, value: date_text(cur.as_ref().and_then(|c| c.first_received_on)) }
                            TextField { id: "ima1015", label: t("item-field-planned-discontinue-on"), input_type: "date", value: v.planned_discontinue_on.map(|d| d.to_string()).unwrap_or_default(), readonly: ro, oninput: move |x: String| form.write().planned_discontinue_on = x.parse().ok() }
                            Select { id: "ima1014", label: t("item-field-life-cycle"), options: options(&LIFE_CYCLE, "item-life-cycle", true), value: txt(&v.life_cycle), readonly: ro, onchange: move |x| form.write().life_cycle = opt(x) }
                            TextField { id: "ima1016", label: t("item-field-carton-barcode"), maxlength: max_len::BARCODE, value: txt(&v.carton_barcode), readonly: ro, oninput: move |x| form.write().carton_barcode = opt(x) }
                        }
                    },
                    4 => rsx! {
                        div { class: "form-grid cols-4",
                            TextField { id: "ima901", label: t("item-field-item-created-on"), readonly: true, value: date_text(cur.as_ref().filter(|_| m != Mode::New).map(|c| c.item_created_on)) }
                            TextField { id: "ima902", label: t("item-field-slow-moving-on"), readonly: true, value: date_text(cur.as_ref().and_then(|c| c.slow_moving_on)) }
                            TextField { id: "ima881", label: t("item-field-last-period-purchase-on"), readonly: true, value: date_text(cur.as_ref().and_then(|c| c.last_period_purchase_on)) }
                            TextField { id: "ima73", label: t("item-field-last-received-on"), readonly: true, value: date_text(cur.as_ref().and_then(|c| c.last_received_on)) }
                            TextField { id: "ima74", label: t("item-field-last-issued-on"), readonly: true, value: date_text(cur.as_ref().and_then(|c| c.last_issued_on)) }
                            TextField { id: "ima29", label: t("item-field-last-txn-on"), readonly: true, value: date_text(cur.as_ref().and_then(|c| c.last_txn_on)) }
                            TextField { id: "ima30", label: t("item-field-last-count-on"), readonly: true, value: date_text(cur.as_ref().and_then(|c| c.last_count_on)) }
                        }
                        p { class: "muted", {t("item-hint-dates")} }
                    },
                    _ => rsx! {
                        StatusForm { status: cur.as_ref().filter(|_| m != Mode::New).map(|c| c.status()), queryable: crate::components::status::ALL, query: q, qbe, prefix: "ima" }
                    },
                  }
                }
            }

            // ----- 料件清單（目前這一頁，IQ6） -----
            Card {
                div { class: "table-header",
                    h3 { {t("item-list-title")} }
                    if queried {
                        p { {tf("list-count-filtered", &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                    } else {
                        p { class: "muted", {t("list-msg-not-queried")} }
                    }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for id in ["item-field-item-no", "item-field-name", "item-field-spec", "item-field-group", "item-field-source", "igroup-field-stock-unit", "item-field-active-code"] {
                                th { key: "{id}", {t(id)} }
                            }
                        } }
                        tbody {
                            match &*rec.list.read() {
                                None => rsx! { tr { td { colspan: 7, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 7, "{e}" } } },
                                Some(Ok(page)) if page.items.is_empty() => rsx! { tr { td { colspan: 7, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(page)) => rsx! {
                                    for (k, i) in page.items.iter().cloned().enumerate() {
                                        tr {
                                            key: "{i.fields.item_no}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|c| c.fields.item_no == i.fields.item_no) { "md-table__row--selected" },
                                            onclick: move |_| if mode() == Mode::View { message.set(None); error.set(None); rec.go(offset + k as u64); },
                                            td { class: "mono", "{i.fields.item_no}" }
                                            td { {shown(&i, |x| x.name.clone(), &i.fields.name)} }
                                            td { {shown(&i, |x| x.spec.clone(), &i.fields.spec)} }
                                            td { class: "mono", {txt(&i.fields.group_code)} }
                                            td { class: "mono", {txt(&i.fields.source_code)} }
                                            td { class: "mono", {txt(&i.fields.stock_unit)} }
                                            td {
                                                Chip { label: t(active_id(&i.active_code)), tone: active_tone(&i.active_code) }
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
                    title: tf("item-dialog-copy", &[("no", from.as_str())]),
                    oncancel: move |_| copying.set(None),
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| copying.set(None), {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: do_copy, {t("common-action-ok")} }
                    },
                    if let Some(msg) = copy_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| copy_error.set(None) }
                    }
                    div { class: "form-grid",
                        TextField { id: "ima01_new", label: t("item-field-new-item-no"), maxlength: max_len::ITEM_NO, value: copy_no(), oninput: move |x| copy_no.set(x) }
                        TextField { id: "ima02_new", label: t("item-field-name"), maxlength: max_len::NAME, value: copy_name(), oninput: move |x| copy_name.set(x) }
                    }
                }
            }
        }
    }
}
