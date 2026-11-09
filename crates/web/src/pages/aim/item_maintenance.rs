//! Item Maintenance 料件基本資料維護（TipTop aimi100 料件基本資料建立，以客製 topcust/cim 為準）
//! 版面比照 topcust/cim/4fd/aimi100.4fd（IM1）：動作列＋上下筆＋單筆表單，分頁「主畫面」「工程／計劃／庫存」
//! 「會計／狀態／分群」「料件狀態」「參考日期」「狀態」（ADR 0012 §9），下方為料件清單。
//! 品名、規格多語言（IM7）；分群碼帶入預設值由使用者按鈕帶入（IM25）。規則在後端 ItemService。

use dioxus::prelude::*;
use rust_decimal::Decimal;
use ttrs_shared::{CopyItemRequest, ItemDto, ItemInput, ItemTextDto, item_max_len as max_len};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Chip, Dialog, Notification, NotificationKind, Select,
    Tabs, TextField, Tone, icons,
};
use crate::components::multilingual::{LangField, LangValues, MultilingualDialogField};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::components::qbe::RecordNav;
use crate::components::status::{HasStatus, StatusForm, TAB_STATUS};
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
pub fn ItemMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ItemPage {} }
    }
}

#[component]
fn ItemPage() -> Element {
    let perm = use_context::<Perm>();
    let plant = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut items = use_resource(move || {
        let p = plant();
        async move {
            if p.is_empty() {
                return Ok(Vec::new());
            }
            api::list_items(&p).await
        }
    });
    let mut selected = use_signal(|| None::<String>);
    // 目前這一筆的完整資料（含各語系品名）
    let mut detail = use_signal(|| None::<ItemDto>);
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

    let list: Vec<ItemDto> = items
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let pos = selected()
        .and_then(|c| list.iter().position(|x| x.fields.item_no == c))
        .or(if list.is_empty() { None } else { Some(0) });
    let cur_no = pos.map(|i| list[i].fields.item_no.clone());

    // 選到的料件改變時載入完整資料（各語系品名）
    use_effect(move || {
        let p = plant();
        let no = selected().or_else(|| {
            items
                .read()
                .as_ref()
                .and_then(|r| r.as_ref().ok())
                .and_then(|l| l.first().map(|i| i.fields.item_no.clone()))
        });
        let _ = items.read();
        spawn(async move {
            let Some(no) = no.filter(|_| !p.is_empty()) else {
                detail.set(None);
                return;
            };
            match api::get_item(&p, &no).await {
                Ok(d) => {
                    let v = text_values(&d);
                    texts.set(v.clone());
                    loaded_texts.set(v);
                    detail.set(Some(d));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    });

    let cur: Option<ItemDto> = detail().filter(|d| Some(&d.fields.item_no) == cur_no.as_ref());
    let no_cur = cur.is_none();
    let total = list.len() as u64;
    let m = mode();
    let ro = m == Mode::View;
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

    let mut done = move |msg: String, no: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        mode.set(Mode::View);
        selected.set(no);
        items.restart();
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
        let Some(c) = detail() else { return };
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
    let list_rows = list.clone();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("item-breadcrumb")} }
                h2 { {t("item-title")} }
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
                                form.set(ItemInput::default());
                                texts.set(LangValues::default());
                                loaded_texts.set(LangValues::default());
                                mode.set(Mode::New);
                            },
                            {t("common-action-new")}
                        }
                    }
                    if perm.can("modify") {
                        Button {
                            icon: icons::EDIT, tone: Tone::Tertiary,
                            variant: ButtonVariant::Tonal,
                            disabled: no_cur || invalid,
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
                                copy_no.set(String::new());
                                copy_name.set(txt(&c.fields.name));
                                copy_error.set(None);
                                copying.set(Some(c.fields.item_no));
                            },
                            {t("common-action-copy")}
                        }
                    }
                    if perm.can("confirm") && !confirmed {
                        Button {
                            icon: icons::CHECK_CIRCLE, tone: Tone::Success,
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur || invalid,
                            onclick: move |_| act("confirm", "item-msg-confirmed"),
                            {t("item-action-confirm")}
                        }
                    }
                    if perm.can("notconfirm") && confirmed {
                        Button {
                            icon: icons::UNDO, tone: Tone::Tertiary,
                            variant: ButtonVariant::Outlined,
                            onclick: move |_| act("unconfirm", "item-msg-unconfirmed"),
                            {t("item-action-unconfirm")}
                        }
                    }
                    if perm.can("invalid") {
                        Button {
                            icon: if invalid { icons::CHECK_CIRCLE } else { icons::BLOCK },
                            tone: if invalid { Tone::Success } else { Tone::Error },
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: move |_| act("void", if invalid { "item-msg-restored" } else { "item-msg-voided" }),
                            if invalid { {t("item-action-restore")} } else { {t("item-action-void")} }
                        }
                    }
                    if perm.can("order_invalid") {
                        Button {
                            icon: icons::SYNC, tone: Tone::Primary,
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: move |_| act("order-invalid", "item-msg-order-invalid"),
                            {t("item-action-order-invalid")}
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
                                    let no = c.fields.item_no.clone();
                                    match api::item_history(&p, &no).await {
                                        Ok(h) => history.set(Some((no, h))),
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            {t("common-action-history")}
                        }
                    }
                    if perm.can("delete") {
                        Button {
                            icon: icons::DELETE, tone: Tone::Error,
                            variant: ButtonVariant::Error,
                            disabled: no_cur || active_code != "P",
                            onclick: move |_| {
                                let Some(c) = cur_delete.clone() else { return };
                                let p = plant();
                                spawn(async move {
                                    let no = c.fields.item_no.clone();
                                    match api::delete_item(&p, &no, c.version).await {
                                        Ok(()) => done(tf("item-msg-deleted", &[("no", no.as_str())]), None),
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
                        onclick: move |_| {
                            error.set(None);
                            texts.set(loaded_texts());
                            mode.set(Mode::View);
                        },
                        {t("common-action-cancel")}
                    }
                }
            }

            if ro && total > 0 {
                RecordNav {
                    total,
                    pos: pos.map(|p| p as u64),
                    ongo: {
                        let nos: Vec<String> = list.iter().map(|i| i.fields.item_no.clone()).collect();
                        move |i: u64| {
                            if let Some(n) = nos.get(i as usize) {
                                message.set(None);
                                selected.set(Some(n.clone()));
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
                    0 => rsx! {
                        div { class: "form-grid",
                            TextField { id: "ima01", label: t("item-field-item-no"), maxlength: max_len::ITEM_NO, value: v.item_no.clone(), readonly: m != Mode::New, oninput: move |x| form.write().item_no = x }
                            TextField {
                                id: "imaacti",
                                label: t("item-field-active-code"),
                                readonly: true,
                                value: if m == Mode::New { t("item-active-processing") } else { t(active_id(&active_code)) },
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
                        StatusForm { status: cur.as_ref().filter(|_| m != Mode::New).map(|c| c.status()), query: false, qbe: None, prefix: "ima" }
                    },
                  }
                }
            }

            // ----- 料件清單 -----
            Card {
                div { class: "table-header",
                    h3 { {t("item-list-title")} }
                    p { {tf("list-count-all", &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for id in ["item-field-item-no", "item-field-name", "item-field-spec", "item-field-group", "item-field-source", "igroup-field-stock-unit", "item-field-active-code"] {
                                th { key: "{id}", {t(id)} }
                            }
                        } }
                        tbody {
                            match &*items.read() {
                                None => rsx! { tr { td { colspan: 7, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 7, "{e}" } } },
                                Some(Ok(_)) if list_rows.is_empty() => rsx! { tr { td { colspan: 7, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for i in list_rows.iter().cloned() {
                                        tr {
                                            key: "{i.fields.item_no}",
                                            class: "md-table__row--clickable",
                                            class: if cur_no.as_ref() == Some(&i.fields.item_no) { "md-table__row--selected" },
                                            onclick: {
                                                let no = i.fields.item_no.clone();
                                                move |_| if mode() == Mode::View { message.set(None); error.set(None); selected.set(Some(no.clone())); }
                                            },
                                            td { class: "mono", "{i.fields.item_no}" }
                                            td { {txt(&i.fields.name)} }
                                            td { {txt(&i.fields.spec)} }
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
