//! 料件單位換算維護（TipTop aooi103 / smd_file；specs/features/aoo/unit_conversion.feature）
//! 版面比照 tiptop/aoo/4fd/aooi103.4fd：單頭為料件（料號、品名、規格、庫存單位，唯讀），單身為換算。工廠層（UC2）。
//! 功能（ADR 0013、UC9）：查詢（輸入料號）、單身新增／修改／刪除、失效／啟用、列印、匯出 Excel、異動記錄；單頭新增、刪除、複製、確認為例外。

use dioxus::prelude::*;
use ttrs_shared::{ItemDto, ItemUnitConversionDto};

use super::unit_conversion_maintenance::{ConvFields, ConvForm, use_unit_options};
use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, TextField, Tone, icons,
};
use crate::components::permission::ProgramGate;
use crate::components::plant::PlantPicker;
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

const PROGRAM: &str = "item-unit-conversion-maintenance";

#[component]
pub fn ItemUnitConversionMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ItemConversionPage {} }
    }
}

fn key(c: &ItemUnitConversionDto) -> String {
    format!("{} {}", c.from_unit, c.to_unit)
}

#[component]
fn ItemConversionPage() -> Element {
    let plant = use_signal(String::new);
    let mut item_no = use_signal(String::new);
    let mut item = use_signal(|| None::<ItemDto>);
    let mut lines = use_signal(Vec::<ItemUnitConversionDto>::new);
    let mut selected = use_signal(|| None::<String>);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut editing = use_signal(|| None::<Option<ItemUnitConversionDto>>);
    let mut form = use_signal(ConvForm::default);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);
    let units = use_unit_options();

    // 依料號載入單頭與單身
    let load = move |no: String| {
        let p = plant();
        spawn(async move {
            match api::get_item(&p, no.trim()).await {
                Ok(it) => {
                    match api::list_item_unit_conversions(&p, &it.fields.item_no).await {
                        Ok(l) => lines.set(l),
                        Err(e) => error.set(Some(e)),
                    }
                    item.set(Some(it));
                }
                Err(e) => {
                    item.set(None);
                    lines.set(Vec::new());
                    error.set(Some(e));
                }
            }
        });
    };

    let list = lines();
    let cur: Option<ItemUnitConversionDto> = selected()
        .and_then(|k| list.iter().find(|x| key(x) == k).cloned())
        .or_else(|| list.first().cloned());
    let has_item = item().is_some();
    let no_cur = cur.is_none();
    let inactive = cur.as_ref().is_some_and(|c| !c.active);
    let unit_options = units.read().clone().unwrap_or_default();

    let mut done = move |msg: String, k: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editing.set(None);
        dialog_error.set(None);
        selected.set(k);
        if let Some(it) = item() {
            load(it.fields.item_no);
        }
    };

    let save = move |_| {
        let Some(it) = item() else { return };
        let ed = editing();
        let input = match form().input() {
            Ok(i) => i,
            Err(e) => {
                dialog_error.set(Some(e));
                return;
            }
        };
        let p = plant();
        spawn(async move {
            let r = match ed {
                Some(Some(c)) => {
                    api::update_item_unit_conversion(
                        &p,
                        &it.fields.item_no,
                        &c.from_unit,
                        &c.to_unit,
                        &input,
                    )
                    .await
                }
                _ => api::create_item_unit_conversion(&p, &it.fields.item_no, &input).await,
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

    let report = move |format: api::ReportFormat| {
        let p = plant();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            let path = format!("/api/plants/{p}/item-unit-conversions");
            if let Err(e) = api::system_list_report(
                &path,
                PROGRAM,
                format,
                &[],
                crate::i18n::current().code(),
                tab,
            )
            .await
            {
                error.set(Some(e));
            }
        });
    };

    let (c1, c2, c3) = (cur.clone(), cur.clone(), cur.clone());

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("item-conversion-breadcrumb")} }
                h2 { {t("item-conversion-title")} }
            }
            PlantPicker { plant }
            div { class: "action-bar",
                TextField { id: "item-no", label: t("conversion-field-item"), value: item_no(), oninput: move |v| item_no.set(v) }
                Button {
                    icon: icons::SEARCH,
                    tone: Tone::Primary,
                    variant: ButtonVariant::Tonal,
                    disabled: item_no().trim().is_empty(),
                    onclick: move |_| { message.set(None); error.set(None); selected.set(None); load(item_no()); },
                    {t("common-action-query")}
                }
                MaintenanceToolbar {
                    detail_action: "detail",
                    invalid_action: "detail",
                    insert: Act::new(has_item, move |_| { message.set(None); dialog_error.set(None); form.set(ConvForm::new_default()); editing.set(Some(None)); }),
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
                    delete: Act::new(!no_cur, move |_| {
                        let Some(c) = c2.clone() else { return };
                        let p = plant();
                        spawn(async move {
                            match api::delete_item_unit_conversion(&p, &c.item_no, &c.from_unit, &c.to_unit, c.version).await {
                                Ok(()) => done(tf("conversion-msg-deleted", &[("key", key(&c).as_str())]), None),
                                Err(e) => error.set(Some(e)),
                            }
                        });
                    }),
                    invalid: Act::new(!no_cur, move |_| {
                        let Some(c) = c3.clone() else { return };
                        let p = plant();
                        spawn(async move {
                            let state = if c.active { "deactivate" } else { "activate" };
                            match api::set_item_unit_conversion_state(&p, &c.item_no, &c.from_unit, &c.to_unit, state, c.version).await {
                                Ok(n) => done(tf(if n.active { "conversion-msg-activated" } else { "conversion-msg-deactivated" }, &[("key", key(&n).as_str())]), Some(key(&n))),
                                Err(e) => error.set(Some(e)),
                            }
                        });
                    }),
                    inactive,
                    print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                    export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                    history: Act::new(has_item, move |_| {
                        let Some(it) = item() else { return };
                        let p = plant();
                        spawn(async move {
                            match api::item_unit_conversion_history(&p, &it.fields.item_no).await {
                                Ok(h) => history.set(Some((it.fields.item_no.clone(), h))),
                                Err(e) => error.set(Some(e)),
                            }
                        });
                    }),
                }
            }
            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            if let Some(it) = item() {
                Card {
                    div { class: "card-body",
                        div { class: "form-grid cols-4",
                            TextField { id: "ima01", label: t("conversion-field-item"), value: it.fields.item_no.clone(), readonly: true }
                            TextField { id: "ima02", label: t("conversion-field-item-name"), value: it.fields.name.clone().unwrap_or_default(), readonly: true }
                            TextField { id: "ima021", label: t("conversion-field-item-spec"), value: it.fields.spec.clone().unwrap_or_default(), readonly: true }
                            TextField { id: "ima25", label: t("conversion-field-stock-unit"), value: it.fields.stock_unit.clone().unwrap_or_default(), readonly: true }
                        }
                        div { class: "table-scroll",
                            table { class: "md-table",
                                thead { tr {
                                    th { {t("conversion-field-from-unit")} }
                                    th { {t("conversion-field-from-quantity")} }
                                    th { {t("conversion-field-to-unit")} }
                                    th { {t("conversion-field-to-quantity")} }
                                    th { {t("conversion-field-note")} }
                                    th { {t("conversion-field-active")} }
                                } }
                                tbody {
                                    if list.is_empty() {
                                        tr { td { colspan: 6, class: "muted", {t("common-msg-no-data")} } }
                                    }
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
                                            td { {c.note.clone().unwrap_or_default()} }
                                            td { if c.active { {t("common-yes")} } else { {t("common-no")} } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
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
                    p { class: "muted", {t("item-conversion-dialog-hint")} }
                }
            }
        }
    }
}
