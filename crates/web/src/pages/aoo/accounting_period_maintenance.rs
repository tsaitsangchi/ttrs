//! 會計期間維護（TipTop aoos020 / azm_file；specs/features/aoo/accounting_periods.feature）
//! 版面比照 tiptop/aoo/4fd/aoos020.4fd：年度、期數與各期起訖日、季別；下方併入週期期間查詢（aooq010，PD3）。
//! 功能（ADR 0013、PD9）：新增、修改、刪除、查詢、複製為另一個年度、確認／取消確認、列印、匯出 Excel、異動記錄；失效為例外。

use dioxus::prelude::*;
use ttrs_shared::{
    AccountingDayDto, AccountingPeriodLine, AccountingYearDto, AccountingYearInput, QbeCondition,
    QueryRequest,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Tabs, TextField, Tone,
    icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::qbe::{ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav};
use crate::components::status::{self, HasStatus, StatusForm};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

const PROGRAM: &str = "accounting-period-maintenance";

#[component]
pub fn AccountingPeriodMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, PeriodPage {} }
    }
}

/// 編輯中的一期：日期與季別以字串保存
#[derive(Clone, Default, PartialEq)]
struct LineForm {
    period: i16,
    start: String,
    end: String,
    quarter: String,
}

#[derive(Clone, Default, PartialEq)]
struct Form {
    year: String,
    first_start: String,
    lines: Vec<LineForm>,
    version: Option<i32>,
}

impl Form {
    fn lines_of(periods: &[AccountingPeriodLine]) -> Vec<LineForm> {
        periods
            .iter()
            .map(|p| LineForm {
                period: p.period,
                start: p.start_date.to_string(),
                end: p.end_date.to_string(),
                quarter: p.quarter.to_string(),
            })
            .collect()
    }

    fn input(&self) -> Result<AccountingYearInput, String> {
        let year: i32 = self
            .year
            .trim()
            .parse()
            .map_err(|_| t("period-msg-year-required"))?;
        let mut periods = Vec::new();
        for l in &self.lines {
            let bad = || {
                tf(
                    "period-msg-line-invalid",
                    &[("period", l.period.to_string().as_str())],
                )
            };
            periods.push(AccountingPeriodLine {
                period: l.period,
                start_date: l.start.trim().parse().map_err(|_| bad())?,
                end_date: l.end.trim().parse().map_err(|_| bad())?,
                quarter: l.quarter.trim().parse().map_err(|_| bad())?,
            });
        }
        if periods.is_empty() {
            return Err(t("period-msg-defaults-first"));
        }
        Ok(AccountingYearInput {
            year,
            periods,
            version: self.version,
        })
    }
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(i32),
}

fn key(y: &AccountingYearDto) -> String {
    y.year.to_string()
}

#[component]
fn PeriodPage() -> Element {
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
        async move { api::query_accounting_years(&req).await }
    });
    let mut selected = use_signal(|| None::<String>);
    let nav = use_row_nav(page, selected, move || {
        rows.read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(key).collect())
            .unwrap_or_default()
    });
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(Form::default);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);
    let mut copy_to = use_signal(String::new);
    let tab = use_signal(|| 0usize);
    // 週期期間查詢（aooq010）
    let mut day_from = use_signal(String::new);
    let mut day_to = use_signal(String::new);
    let mut days = use_signal(|| None::<Vec<AccountingDayDto>>);

    let page_data = rows.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<AccountingYearDto> = page_data
        .as_ref()
        .map(|p| p.items.clone())
        .unwrap_or_default();
    let total = page_data.as_ref().map_or(0, |p| p.total);
    let pages = page_data.as_ref().map_or(1, |p| {
        u32::try_from(p.total.div_ceil(u64::from(p.page_size.max(1))))
            .unwrap_or(1)
            .max(1)
    });
    let cur: Option<AccountingYearDto> = selected()
        .and_then(|k| list.iter().find(|x| key(x) == k).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let confirmed = cur.as_ref().is_some_and(|c| c.confirmed);
    let keys: Vec<String> = list.iter().map(key).collect();

    let mut done = move |msg: String, k: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(k);
        rows.restart();
    };

    let run_query = move |_| {
        let conds = conditions_of(&qbe.read());
        querying.set(false);
        selected.set(None);
        page.set(1);
        conditions.set(conds);
    };

    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            if let Err(e) = api::system_list_report(
                "/api/accounting-years",
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

    // 輸入年度與第 1 期起始日後帶出預設各期（PD5）
    let fill_defaults = move |_| {
        let f = form();
        let Ok(year) = f.year.trim().parse::<i32>() else {
            dialog_error.set(Some(t("period-msg-year-required")));
            return;
        };
        let start = f.first_start.trim().to_owned();
        spawn(async move {
            match api::accounting_year_defaults(year, &start).await {
                Ok(p) => {
                    dialog_error.set(None);
                    form.write().lines = Form::lines_of(&p);
                }
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let save = move |_| {
        let ed = editor();
        let input = match form().input() {
            Ok(i) => i,
            Err(e) => {
                dialog_error.set(Some(e));
                return;
            }
        };
        spawn(async move {
            let r = match ed {
                Some(Editor::Edit(y)) => api::update_accounting_year(y, &input).await,
                _ => api::create_accounting_year(&input).await,
            };
            match r {
                Ok(y) => done(
                    tf("period-msg-saved", &[("year", key(&y).as_str())]),
                    Some(key(&y)),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_edit = cur.clone();
    let cur_delete = cur.clone();
    let cur_copy = cur.clone();
    let cur_confirm = cur.clone();
    let cur_history = cur.clone();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("period-breadcrumb")} }
                h2 { {t("period-title")} }
            }

            if querying() {
                div { class: "action-bar",
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| querying.set(false), {t("common-action-cancel")} }
                    TextField { id: "q-year", label: t("period-field-year"), value: qbe.read().get("year").cloned().unwrap_or_default(), oninput: move |v: String| { qbe.write().insert("year", v); } }
                }
                p { class: "query-hint", {t("query-hint")} }
            } else {
                div { class: "action-bar",
                    MaintenanceToolbar {
                        query: Act::new(true, move |_| { message.set(None); error.set(None); qbe.set(Default::default()); querying.set(true); }),
                        insert: Act::new(true, move |_| {
                            message.set(None);
                            dialog_error.set(None);
                            form.set(Form::default());
                            editor.set(Some(Editor::New));
                        }),
                        // 已確認不可修改（PD8）
                        modify: Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(Form {
                                year: c.year.to_string(),
                                first_start: String::new(),
                                lines: Form::lines_of(&c.periods),
                                version: Some(c.version),
                            });
                            editor.set(Some(Editor::Edit(c.year)));
                        }),
                        // 複製為另一個年度（PD9）
                        copy: Act::new(!no_cur && copy_to().trim().parse::<i32>().is_ok(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let Ok(to) = copy_to().trim().parse::<i32>() else { return };
                            spawn(async move {
                                match api::copy_accounting_year(c.year, to).await {
                                    Ok(y) => {
                                        copy_to.set(String::new());
                                        done(tf("period-msg-copied", &[("from", key(&c).as_str()), ("to", key(&y).as_str())]), Some(key(&y)));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        delete: Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match api::delete_accounting_year(c.year, c.version).await {
                                    Ok(()) => done(tf("period-msg-deleted", &[("year", key(&c).as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            spawn(async move {
                                let state = if c.confirmed { "unconfirm" } else { "confirm" };
                                match api::set_accounting_year_state(c.year, state, c.version).await {
                                    Ok(y) => {
                                        let id = if y.confirmed { "period-msg-confirmed" } else { "period-msg-unconfirmed" };
                                        done(tf(id, &[("year", key(&y).as_str())]), Some(key(&y)));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        history: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_history.clone() else { return };
                            spawn(async move {
                                match api::shared_history(PROGRAM, &key(&c)).await {
                                    Ok(h) => history.set(Some((key(&c), h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        if perm.can("reproduce") {
                            TextField { id: "copy-year", label: t("period-field-copy-year"), maxlength: 4, value: copy_to(), disabled: no_cur, oninput: move |v| copy_to.set(v) }
                        }
                    }
                }
            }

            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(key).as_deref()),
                    ongo: {
                        let keys = keys.clone();
                        move |i| { message.set(None); nav.go(i, &keys); }
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

            Card {
                Tabs { tabs: vec![t("common-tab-basic"), t(status::TAB_STATUS)], active: tab }
                div { class: "card-body",
                    match (cur.as_ref(), &*rows.read()) {
                        (_, None) => rsx! { p { class: "muted", {t("common-msg-loading")} } },
                        (_, Some(Err(e))) => rsx! { p { class: "muted", "{e}" } },
                        (None, _) => rsx! { p { class: "muted", {t("common-msg-no-data")} } },
                        (Some(c), _) if tab() == 1 => rsx! {
                            StatusForm { status: Some(c.status()), fields: status::NO_ACTIVE, prefix: "azm" }
                        },
                        (Some(c), _) => rsx! {
                            div { class: "form-grid cols-4",
                                TextField { id: "azm01", label: t("period-field-year"), value: c.year.to_string(), readonly: true }
                                TextField { id: "azm02", label: t("period-field-count"), value: tf("period-count-value", &[("count", c.period_count.to_string().as_str())]), readonly: true }
                                TextField { id: "azm-confirmed", label: t("period-field-confirmed"), value: if c.confirmed { t("common-yes") } else { t("common-no") }, readonly: true }
                            }
                            PeriodTable { periods: c.periods.clone() }
                        },
                    }
                }
                ListPager { page, pages }
            }

            // 週期期間查詢（aooq010，PD3）
            Card {
                div { class: "card-body",
                    h3 { {t("period-days-title")} }
                    div { class: "action-bar",
                        TextField { id: "day-from", label: t("period-field-from"), input_type: "date", value: day_from(), oninput: move |v| day_from.set(v) }
                        TextField { id: "day-to", label: t("period-field-to"), input_type: "date", value: day_to(), oninput: move |v| day_to.set(v) }
                        Button {
                            icon: icons::SEARCH,
                            tone: Tone::Primary,
                            variant: ButtonVariant::Tonal,
                            disabled: day_from().trim().is_empty(),
                            onclick: move |_| {
                                let (a, b) = (day_from(), day_to());
                                let b = if b.trim().is_empty() { a.clone() } else { b };
                                spawn(async move {
                                    match api::accounting_days(a.trim(), b.trim()).await {
                                        Ok(d) => { error.set(None); days.set(Some(d)); }
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            {t("common-action-query")}
                        }
                    }
                    if let Some(d) = days() {
                        div { class: "table-scroll",
                            table { class: "md-table",
                                thead { tr {
                                    th { {t("period-col-date")} }
                                    th { {t("period-field-year")} }
                                    th { {t("period-col-quarter")} }
                                    th { {t("period-col-period")} }
                                    th { {t("period-col-week")} }
                                } }
                                tbody {
                                    for x in d {
                                        tr { key: "{x.date}",
                                            td { class: "mono", "{x.date}" }
                                            td { class: "num", "{x.year}" }
                                            td { class: "num", "{x.quarter}" }
                                            td { class: "num", "{x.period}" }
                                            td { class: "num", "{x.week}" }
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
            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("period-dialog-new"),
                        Editor::Edit(y) => tf("period-dialog-edit", &[("year", y.to_string().as_str())]),
                    },
                    oncancel: move |_| { editor.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { editor.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    if ed == Editor::New {
                        div { class: "action-bar",
                            TextField { id: "edit-year", label: t("period-field-year"), maxlength: 4, value: form().year, oninput: move |v| form.write().year = v }
                            TextField { id: "edit-first-start", label: t("period-field-first-start"), input_type: "date", value: form().first_start, oninput: move |v| form.write().first_start = v }
                            Button { icon: icons::SYNC, tone: Tone::Primary, variant: ButtonVariant::Tonal, disabled: form().first_start.trim().is_empty(), onclick: fill_defaults, {t("period-action-defaults")} }
                        }
                    }
                    if !form().lines.is_empty() {
                        div { class: "table-scroll",
                            table { class: "md-table",
                                thead { tr {
                                    th { {t("period-col-period")} }
                                    th { {t("period-col-start")} }
                                    th { {t("period-col-end")} }
                                    th { {t("period-col-quarter")} }
                                } }
                                tbody {
                                    for (i, l) in form().lines.into_iter().enumerate() {
                                        tr { key: "{l.period}",
                                            td { class: "num", "{l.period}" }
                                            td { TextField { id: "line-start-{l.period}", label: t("period-col-start"), input_type: "date", value: l.start.clone(), oninput: move |v| form.write().lines[i].start = v } }
                                            td { TextField { id: "line-end-{l.period}", label: t("period-col-end"), input_type: "date", value: l.end.clone(), oninput: move |v| form.write().lines[i].end = v } }
                                            td { TextField { id: "line-quarter-{l.period}", label: t("period-col-quarter"), input_type: "number", value: l.quarter.clone(), oninput: move |v| form.write().lines[i].quarter = v } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    p { class: "muted", {t("period-dialog-hint")} }
                }
            }
        }
    }
}

/// 各期一覽（唯讀）
#[component]
fn PeriodTable(periods: Vec<AccountingPeriodLine>) -> Element {
    rsx! {
        div { class: "table-scroll",
            table { class: "md-table",
                thead { tr {
                    th { {t("period-col-period")} }
                    th { {t("period-col-start")} }
                    th { {t("period-col-end")} }
                    th { {t("period-col-quarter")} }
                } }
                tbody {
                    for p in periods {
                        tr { key: "{p.period}",
                            td { class: "num", "{p.period}" }
                            td { class: "mono", "{p.start_date}" }
                            td { class: "mono", "{p.end_date}" }
                            td { class: "num", "{p.quarter}" }
                        }
                    }
                }
            }
        }
    }
}
