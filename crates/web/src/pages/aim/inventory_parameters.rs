//! 庫存參數（TipTop asms230 / sma_file；specs/features/aim/inventory_parameters.feature）
//! 版面比照 tiptop/asm/4fd/asms230.4fd：「料件存放位置管制」「週轉率」「庫存帳務」「調撥」「雜收發」五組＋「狀態」頁籤（IP1）。
//! 工廠層：每個營運中心一份（IP2）。功能（ADR 0013、IP9）：修改、列印、匯出 Excel、異動記錄；其餘為例外（只有一筆）。

use dioxus::prelude::*;
use ttrs_shared::{InventoryParametersDto, InventoryParametersInput};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Dialog, Notification, NotificationKind, Select, Tabs,
    TextField, Tone, icons,
};
use crate::components::permission::ProgramGate;
use crate::components::plant::PlantPicker;
use crate::components::status::{self, HasStatus, StatusForm};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::t;

const PROGRAM: &str = "inventory-parameters";

#[component]
pub fn InventoryParameters() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ParametersPage {} }
    }
}

fn control_options() -> Vec<(String, String)> {
    ["1", "2", "3"]
        .iter()
        .map(|c| ((*c).to_owned(), t(&format!("ip-storage-control-{c}"))))
        .collect()
}

fn basis_options() -> Vec<(String, String)> {
    ["1", "2"]
        .iter()
        .map(|c| ((*c).to_owned(), t(&format!("ip-posting-date-basis-{c}"))))
        .collect()
}

/// 唯讀顯示與編輯共用的欄位
#[component]
fn Fields(
    v: InventoryParametersInput,
    readonly: bool,
    fast: String,
    average: String,
    onchange: EventHandler<InventoryParametersInput>,
    onfast: EventHandler<String>,
    onaverage: EventHandler<String>,
) -> Element {
    let flag = |id: &'static str,
                label: &'static str,
                checked: bool,
                set: fn(&mut InventoryParametersInput, bool)| {
        let v = v.clone();
        rsx! {
            Checkbox {
                id,
                label: t(label),
                checked,
                readonly,
                onchange: move |x| {
                    let mut n = v.clone();
                    set(&mut n, x);
                    onchange.call(n);
                },
            }
        }
    };
    rsx! {
        h3 { {t("ip-group-storage")} }
        div { class: "form-grid",
            Select {
                id: "sma42",
                label: t("ip-field-storage-control"),
                options: control_options(),
                value: v.storage_control.clone(),
                readonly,
                onchange: {
                    let v = v.clone();
                    move |x| onchange.call(InventoryParametersInput { storage_control: x, ..v.clone() })
                },
            }
        }
        h3 { {t("ip-group-turnover")} }
        div { class: "form-grid",
            TextField { id: "sma68", label: t("ip-field-fast-turnover"), input_type: "number", value: fast, readonly, oninput: move |x| onfast.call(x) }
            TextField { id: "sma69", label: t("ip-field-average-turnover"), input_type: "number", value: average, readonly, oninput: move |x| onaverage.call(x) }
        }
        h3 { {t("ip-group-posting")} }
        div { class: "form-grid",
            Select {
                id: "sma892_1",
                label: t("ip-field-posting-date-basis"),
                options: basis_options(),
                value: v.posting_date_basis.clone(),
                readonly,
                onchange: {
                    let v = v.clone();
                    move |x| onchange.call(InventoryParametersInput { posting_date_basis: x, ..v.clone() })
                },
            }
            {flag("sma892_2", "ip-field-misc-receipt-new-location", v.misc_receipt_new_location, |i, x| i.misc_receipt_new_location = x)}
            {flag("sma892_3", "ip-field-stock-query-window", v.stock_query_window, |i, x| i.stock_query_window = x)}
        }
        h3 { {t("ip-group-transfer")} }
        div { class: "form-grid",
            {flag("sma88", "ip-field-plant-transfer-same-no", v.plant_transfer_same_no, |i, x| i.plant_transfer_same_no = x)}
            {flag("sma884", "ip-field-warehouse-transfer-same-no", v.warehouse_transfer_same_no, |i, x| i.warehouse_transfer_same_no = x)}
        }
        h3 { {t("ip-group-misc")} }
        div { class: "form-grid",
            {flag("sma847_1", "ip-field-misc-issue-dept", v.misc_issue_dept, |i, x| i.misc_issue_dept = x)}
            {flag("sma847_2", "ip-field-misc-receipt-dept", v.misc_receipt_dept, |i, x| i.misc_receipt_dept = x)}
            {flag("sma39", "ip-field-add-storage-freely", v.add_storage_freely, |i, x| i.add_storage_freely = x)}
        }
        // 期間參數（sma51、sma52、sma53；misc_issue_posting.feature PT3）：未設定時過帳不檢查
        h3 { {t("ip-group-period")} }
        div { class: "form-grid",
            TextField {
                id: "sma51",
                label: t("ip-field-current-year"),
                input_type: "number",
                value: v.current_year.map(|x| x.to_string()).unwrap_or_default(),
                readonly,
                oninput: {
                    let v = v.clone();
                    move |x: String| onchange.call(InventoryParametersInput { current_year: x.trim().parse().ok(), ..v.clone() })
                },
            }
            TextField {
                id: "sma52",
                label: t("ip-field-current-period"),
                input_type: "number",
                value: v.current_period.map(|x| x.to_string()).unwrap_or_default(),
                readonly,
                oninput: {
                    let v = v.clone();
                    move |x: String| onchange.call(InventoryParametersInput { current_period: x.trim().parse().ok(), ..v.clone() })
                },
            }
            TextField {
                id: "sma53",
                label: t("ip-field-closing-date"),
                input_type: "date",
                value: v.closing_date.map(|x| x.to_string()).unwrap_or_default(),
                readonly,
                oninput: {
                    let v = v.clone();
                    move |x: String| onchange.call(InventoryParametersInput { closing_date: ttrs_shared::NaiveDate::parse_from_str(x.trim(), "%Y-%m-%d").ok(), ..v.clone() })
                },
            }
        }
        p { class: "muted", {t("ip-hint-period")} }
    }
}

#[component]
fn ParametersPage() -> Element {
    let plant = use_signal(String::new);
    let mut params = use_resource(move || {
        let p = plant();
        async move {
            if p.is_empty() {
                return Err(String::new());
            }
            api::get_inventory_parameters(&p).await
        }
    });
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut editing = use_signal(|| false);
    let mut form = use_signal(InventoryParametersInput::default);
    let mut fast = use_signal(String::new);
    let mut average = use_signal(String::new);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<Vec<ttrs_shared::ChangeDto>>);
    let tab = use_signal(|| 0usize);

    let cur: Option<InventoryParametersDto> = params
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let load_error: Option<String> = params
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().err())
        .filter(|e| !e.is_empty())
        .cloned();

    let report = move |format: api::ReportFormat| {
        let tab = api::ReportTab::open(format);
        let p = plant();
        spawn(async move {
            if let Err(e) = api::list_report(
                &p,
                "inventory-parameters",
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

    let save = move |_| {
        let mut input = form();
        for (raw, slot) in [
            (fast(), &mut input.fast_turnover),
            (average(), &mut input.average_turnover),
        ] {
            match raw.trim().parse() {
                Ok(v) => *slot = v,
                Err(_) => {
                    dialog_error.set(Some(t("error-INVENTORY_TURNOVER_INVALID")));
                    return;
                }
            }
        }
        let p = plant();
        spawn(async move {
            match api::update_inventory_parameters(&p, &input).await {
                Ok(_) => {
                    editing.set(false);
                    dialog_error.set(None);
                    error.set(None);
                    message.set(Some(t("ip-msg-saved")));
                    params.restart();
                }
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_edit = cur.clone();
    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("ip-breadcrumb")} }
                h2 { {t("ip-title")} }
            }
            PlantPicker { plant }
            div { class: "action-bar",
                MaintenanceToolbar {
                    modify: Act::new(cur_edit.is_some(), move |_| {
                        let Some(c) = cur_edit.clone() else { return };
                        message.set(None);
                        dialog_error.set(None);
                        fast.set(c.settings.fast_turnover.normalize().to_string());
                        average.set(c.settings.average_turnover.normalize().to_string());
                        form.set(c.to_input());
                        editing.set(true);
                    }),
                    print: Act::new(cur.is_some(), move |_| report(api::ReportFormat::Pdf)),
                    export: Act::new(cur.is_some(), move |_| report(api::ReportFormat::Excel)),
                    history: Act::new(cur.is_some(), move |_| {
                        let p = plant();
                        spawn(async move {
                            match api::inventory_parameters_history(&p).await {
                                Ok(h) => history.set(Some(h)),
                                Err(e) => error.set(Some(e)),
                            }
                        });
                    }),
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = load_error {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| params.restart() }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            Card {
                Tabs { tabs: vec![t("ip-tab-basic"), t(status::TAB_STATUS)], active: tab }
                div { class: "card-body",
                    match cur.as_ref() {
                        None => rsx! { p { class: "muted", {t("common-msg-loading")} } },
                        Some(p) if tab() == 1 => rsx! {
                            StatusForm { status: Some(p.status()), fields: status::NO_ACTIVE, prefix: "sma" }
                        },
                        Some(p) => rsx! {
                            Fields {
                                v: p.settings.clone(),
                                readonly: true,
                                fast: p.settings.fast_turnover.normalize().to_string(),
                                average: p.settings.average_turnover.normalize().to_string(),
                                onchange: |_| {},
                                onfast: |_| {},
                                onaverage: |_| {},
                            }
                            p { class: "muted", {t("ip-hint")} }
                        },
                    }
                }
            }

            if let Some(changes) = history() {
                HistoryDialog { record: t("ip-title"), changes, onclose: move |_| history.set(None) }
            }
            if editing() {
                Dialog {
                    title: t("ip-dialog-edit"),
                    oncancel: move |_| { editing.set(false); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { editing.set(false); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    Fields {
                        v: form(),
                        readonly: false,
                        fast: fast(),
                        average: average(),
                        onchange: move |n| form.set(n),
                        onfast: move |x| fast.set(x),
                        onaverage: move |x| average.set(x),
                    }
                    p { class: "muted", {t("ip-dialog-hint")} }
                }
            }
        }
    }
}
