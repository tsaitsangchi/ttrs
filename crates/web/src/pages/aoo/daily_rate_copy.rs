//! 每日匯率複製（TipTop aoop010 ★ 每日匯率複製作業；specs/features/aoo/exchange_rate_daily_copy.feature）
//! 批次作業（CP1、CP7）：輸入「補到哪一天」（預設今天，CP2）後執行，列出複製結果，可列印、匯出 Excel。
//! 規則在後端 ExchangeRateService::fill。

use dioxus::prelude::*;
use ttrs_shared::DailyRateFillLine;

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Notification, NotificationKind, TextField, Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::i18n::{t, tf};

use super::exchange_rate_maintenance::today;

const PROGRAM: &str = "daily-exchange-rate-copy";

/// 結果表的匯率欄（翻譯 ID）
const RATE_LABELS: [&str; 6] = [
    "rate-field-bank-buy",
    "rate-field-bank-sell",
    "rate-field-bank-mid",
    "rate-field-customs-period",
    "rate-field-customs-buy",
    "rate-field-customs-sell",
];

#[component]
pub fn DailyRateCopy() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, CopyPage {} }
    }
}

#[component]
fn CopyPage() -> Element {
    let perm = use_context::<Perm>();
    let mut until = use_signal(today);
    let mut running = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut lines = use_signal(|| None::<Vec<DailyRateFillLine>>);

    let run = move |_| {
        let d = until();
        running.set(true);
        spawn(async move {
            match api::fill_daily_rates(d.trim()).await {
                Ok(r) => {
                    error.set(None);
                    message.set(Some(if r.is_empty() {
                        t("rate-fill-msg-none")
                    } else {
                        tf(
                            "rate-fill-msg-done",
                            &[("count", r.len().to_string().as_str())],
                        )
                    }));
                    lines.set(Some(r));
                }
                Err(e) => error.set(Some(e)),
            }
            running.set(false);
        });
    };

    let report = move |format: api::ReportFormat| {
        let rows = lines().unwrap_or_default();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            if let Err(e) =
                api::fill_report(&rows, format, crate::i18n::current().code(), tab).await
            {
                error.set(Some(e));
            }
        });
    };

    let result = lines();
    let has_rows = result.as_ref().is_some_and(|r| !r.is_empty());

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("rate-fill-breadcrumb")} }
                h2 { {t("rate-fill-title")} }
            }
            Card {
                p { class: "muted", {t("rate-fill-hint")} }
                div { class: "action-bar",
                    TextField { id: "fill-until", label: t("rate-fill-field-until"), input_type: "date", value: until(), oninput: move |v| until.set(v) }
                    if perm.can("run") {
                        Button { icon: icons::SYNC, tone: Tone::Success, disabled: running() || until().trim().is_empty(), onclick: run, {t("rate-fill-action-run")} }
                    }
                    if perm.can("output") {
                        Button { icon: icons::PRINT, tone: Tone::Primary, variant: ButtonVariant::Outlined, disabled: !has_rows, onclick: move |_| report(api::ReportFormat::Pdf), {t("common-action-print")} }
                    }
                    if perm.can("exporttoexcel") {
                        Button { icon: icons::DOWNLOAD, tone: Tone::Primary, variant: ButtonVariant::Outlined, disabled: !has_rows, onclick: move |_| report(api::ReportFormat::Excel), {t("common-action-export-excel")} }
                    }
                }
            }
            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }
            if let Some(rows) = result {
                Card {
                    div { class: "table-header",
                        h3 { {t("rate-fill-list-title")} }
                        p { {tf("list-count-all", &[("total", rows.len().to_string().as_str())])} }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead {
                                tr {
                                    th { {t("rate-field-currency")} }
                                    th { {t("rate-field-date")} }
                                    for label in RATE_LABELS {
                                        th { key: "{label}", {t(label)} }
                                    }
                                }
                            }
                            tbody {
                                if rows.is_empty() {
                                    tr { td { colspan: 2 + RATE_LABELS.len(), class: "muted", {t("rate-fill-msg-none")} } }
                                }
                                for l in rows.iter() {
                                    tr { key: "{l.currency_code}-{l.rate_date}",
                                        td { class: "mono", "{l.currency_code}" }
                                        td { class: "mono", "{l.rate_date}" }
                                        for v in [l.rates.bank_buy, l.rates.bank_sell, l.rates.bank_mid, l.rates.customs_period, l.rates.customs_buy, l.rates.customs_sell] {
                                            td { class: "num", {v.normalize().to_string()} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
