//! 整體管理系統參數（TipTop aoos010 / aza_file；specs/features/aoo/general_parameters.feature）
//! 版面比照 tiptop/aoo/4fd/aoos010.4fd：單筆表單分頁籤「基本設定」「狀態」（GP1）；只放有移植的參數（GP3）。
//! 功能（ADR 0013、GP8）：修改、列印、匯出 Excel、異動記錄；新增、刪除、複製、查詢、上下筆、失效、確認為例外（只有一筆）。

use dioxus::prelude::*;
use ttrs_shared::{GeneralParametersDto, GeneralParametersInput, QueryRequest};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Dialog, Notification, NotificationKind, Select, Tabs,
    TextField, Tone, icons,
};
use crate::components::permission::ProgramGate;
use crate::components::status::{self, HasStatus, StatusForm};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::t;

const PROGRAM: &str = "general-management-parameters";
/// 參數只有一筆，異動記錄的鍵值固定 0（aza01）
const KEY: &str = "0";

#[component]
pub fn GeneralParameters() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ParametersPage {} }
    }
}

fn periods_label(n: i16) -> String {
    if n == 13 {
        t("gp-periods-13")
    } else {
        t("gp-periods-12")
    }
}

fn basis_label(code: &str) -> String {
    if code == "monthly" {
        t("gp-rate-monthly")
    } else {
        t("gp-rate-daily")
    }
}

/// 編輯中的表單
#[derive(Clone, Default, PartialEq)]
struct Form {
    periods: String,
    base_currency: String,
    rate_basis: String,
    /// aza28 料件編號自動編碼（item_auto_number.feature AN1）
    auto_item_number: bool,
}

#[component]
fn ParametersPage() -> Element {
    let mut params = use_resource(|| async { api::get_general_parameters().await });
    // 本國幣別選單：有效的幣別
    let currencies = use_resource(|| async {
        let req = QueryRequest {
            conditions: Vec::new(),
            page: Some(1),
            page_size: Some(500),
            lang: None,
        };
        api::query_currencies(&req)
            .await
            .map(|p| p.items)
            .unwrap_or_default()
    });
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut editing = use_signal(|| false);
    let mut form = use_signal(Form::default);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<Vec<ttrs_shared::ChangeDto>>);
    let tab = use_signal(|| 0usize);

    let cur: Option<GeneralParametersDto> = params
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let load_error: Option<String> = params
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().err())
        .cloned();
    let currency_options: Vec<(String, String)> = currencies
        .read()
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.active)
        .map(|c| {
            let label = match &c.name {
                Some(n) => format!("{} {n}", c.currency_code), // i18n-allow
                None => c.currency_code.clone(),
            };
            (c.currency_code, label)
        })
        .collect();

    let cur_edit = cur.clone();
    let report = move |format: api::ReportFormat| {
        let tab = api::ReportTab::open(format);
        spawn(async move {
            if let Err(e) = api::system_list_report(
                "/api/general-parameters",
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

    let cur_save = cur.clone();
    let save = move |_| {
        let Some(c) = cur_save.clone() else { return };
        let f = form();
        let Ok(periods) = f.periods.trim().parse::<i16>() else {
            dialog_error.set(Some(t("gp-msg-periods-invalid")));
            return;
        };
        let input = GeneralParametersInput {
            accounting_periods: periods,
            base_currency: f.base_currency.trim().to_owned(),
            rate_basis: f.rate_basis.clone(),
            auto_item_number: Some(f.auto_item_number),
            version: Some(c.version),
        };
        spawn(async move {
            match api::update_general_parameters(&input).await {
                Ok(_) => {
                    editing.set(false);
                    dialog_error.set(None);
                    error.set(None);
                    message.set(Some(t("gp-msg-saved")));
                    params.restart();
                }
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("gp-breadcrumb")} }
                h2 { {t("gp-title")} }
            }
            div { class: "action-bar",
                MaintenanceToolbar {
                    modify: Act::new(cur_edit.is_some(), move |_| {
                        let Some(c) = cur_edit.clone() else { return };
                        message.set(None);
                        dialog_error.set(None);
                        form.set(Form {
                            periods: c.accounting_periods.to_string(),
                            base_currency: c.base_currency.clone(),
                            rate_basis: c.rate_basis.clone(),
                            auto_item_number: c.auto_item_number,
                        });
                        editing.set(true);
                    }),
                    print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                    export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                    history: Act::new(true, move |_| {
                        spawn(async move {
                            match api::shared_history(PROGRAM, KEY).await {
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
                Tabs { tabs: vec![t("gp-tab-basic"), t(status::TAB_STATUS)], active: tab }
                div { class: "card-body",
                    match cur.as_ref() {
                        None => rsx! { p { class: "muted", {t("common-msg-loading")} } },
                        Some(p) if tab() == 1 => rsx! {
                            StatusForm { status: Some(p.status()), fields: status::NO_ACTIVE, prefix: "aza" }
                        },
                        Some(p) => rsx! {
                            div { class: "form-grid",
                                TextField { id: "aza02", label: t("gp-field-accounting-periods"), value: periods_label(p.accounting_periods), readonly: true }
                                TextField {
                                    id: "aza17",
                                    label: t("gp-field-base-currency"),
                                    value: match &p.base_currency_name {
                                        Some(n) => format!("{} {n}", p.base_currency), // i18n-allow
                                        None => p.base_currency.clone(),
                                    },
                                    readonly: true,
                                }
                                TextField { id: "aza19", label: t("gp-field-rate-basis"), value: basis_label(&p.rate_basis), readonly: true }
                                Checkbox { id: "aza28", label: t("gp-field-auto-item-number"), checked: p.auto_item_number, readonly: true }
                            }
                            p { class: "muted", {t("gp-hint")} }
                        },
                    }
                }
            }

            if let Some(changes) = history() {
                HistoryDialog { record: t("gp-title"), changes, onclose: move |_| history.set(None) }
            }
            if editing() {
                Dialog {
                    title: t("gp-dialog-edit"),
                    oncancel: move |_| { editing.set(false); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { editing.set(false); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    div { class: "form-grid",
                        Select {
                            id: "gp-periods",
                            label: t("gp-field-accounting-periods"),
                            options: vec![("12".to_owned(), t("gp-periods-12")), ("13".to_owned(), t("gp-periods-13"))],
                            value: form().periods,
                            onchange: move |v| form.write().periods = v,
                        }
                        Select {
                            id: "gp-base-currency",
                            label: t("gp-field-base-currency"),
                            // 目前的本國幣別不在有效幣別清單時（停用、尚未建立）仍列出，存檔時由伺服器檢查（GP5）
                            options: {
                                let mut o = currency_options.clone();
                                let b = form().base_currency;
                                if !b.is_empty() && !o.iter().any(|(c, _)| *c == b) {
                                    o.insert(0, (b.clone(), b));
                                }
                                o
                            },
                            value: form().base_currency,
                            onchange: move |v| form.write().base_currency = v,
                        }
                        Select {
                            id: "gp-rate-basis",
                            label: t("gp-field-rate-basis"),
                            options: vec![("daily".to_owned(), t("gp-rate-daily")), ("monthly".to_owned(), t("gp-rate-monthly"))],
                            value: form().rate_basis,
                            onchange: move |v| form.write().rate_basis = v,
                        }
                        Checkbox { id: "gp-auto-item-number", label: t("gp-field-auto-item-number"), checked: form().auto_item_number, onchange: move |v| form.write().auto_item_number = v }
                    }
                    p { class: "muted", {t("gp-dialog-hint")} }
                }
            }
        }
    }
}
