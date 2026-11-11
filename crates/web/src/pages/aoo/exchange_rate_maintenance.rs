//! 每月匯率維護（TipTop aooi060 ★）、每日匯率維護（aooi070 ★）
//! 版面比照 tiptop/aoo/4fd/aooi060.4fd、topcust/coo/4fd/aooi070.4fd（MR1、DR1）：單筆表單（幣別、年月或日期、各匯率）＋上下筆，
//! 下方列出查詢結果供選取；新增／修改以小視窗編輯。共用 schema（ER1）。規則在後端 ExchangeRateService。
//! 功能（ADR 0013）：insert、modify、delete、query、reproduce、invalid、output，另加匯出 Excel、異動記錄、確認／取消確認；
//! 每月匯率另有月底重評價 re_rate（MR7），每日匯率另有統一匯率 unify_rate（DR7）與新增時的預設匯率（DR4）。
//! 計算方式為「除」的幣別提供匯率換算（s_rate，MR9）。

use dioxus::prelude::*;
use rust_decimal::Decimal;
use ttrs_shared::{
    CurrencyDto, DailyRateDto, DailyRateInput, MonthlyRateDto, MonthlyRateInput, QbeCondition,
    QueryRequest, RateBatchResult, RatesDto,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Select, Tabs, TextField,
    Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::qbe::{
    HINT_DATE, ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav,
};
use crate::components::status::{
    self, ConfirmCells, ConfirmHeaders, ConfirmQbeCells, HasStatus, RecordStatus, StatusCells,
    StatusHeaders, StatusQbeCells, TAB_DATA, TAB_STATUS,
};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

/// 六種匯率：（翻譯 ID, 查詢欄位鍵值）
const RATES: [(&str, &str); 6] = [
    ("rate-field-bank-buy", "bank_buy"),
    ("rate-field-bank-sell", "bank_sell"),
    ("rate-field-bank-mid", "bank_mid"),
    ("rate-field-customs-period", "customs_period"),
    ("rate-field-customs-buy", "customs_buy"),
    ("rate-field-customs-sell", "customs_sell"),
];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Monthly,
    Daily,
}

impl Kind {
    fn program(self) -> &'static str {
        match self {
            Self::Monthly => "monthly-exchange-rate-maintenance",
            Self::Daily => "daily-exchange-rate-maintenance",
        }
    }

    fn id(self, suffix: &str) -> String {
        let p = match self {
            Self::Monthly => "monthly-rate",
            Self::Daily => "daily-rate",
        };
        format!("{p}-{suffix}")
    }

    fn key_label(self) -> &'static str {
        match self {
            Self::Monthly => "rate-field-period",
            Self::Daily => "rate-field-date",
        }
    }

    fn key_field(self) -> &'static str {
        match self {
            Self::Monthly => "period",
            Self::Daily => "rate_date",
        }
    }

    fn api_path(self) -> &'static str {
        match self {
            Self::Monthly => "/api/monthly-rates",
            Self::Daily => "/api/daily-rates",
        }
    }
}

/// 每月、每日匯率共用的列
#[derive(Clone, PartialEq)]
struct Row {
    currency: String,
    name: String,
    /// 年月 YYYYMM 或日期 YYYY-MM-DD
    key: String,
    rates: RatesDto,
    revaluation: Option<Decimal>,
    active: bool,
    confirmed: bool,
    confirmed_by: Option<String>,
    version: i32,
    status: RecordStatus,
}

impl Row {
    fn id(&self) -> String {
        format!("{} {}", self.currency, self.key)
    }
}

impl From<MonthlyRateDto> for Row {
    fn from(d: MonthlyRateDto) -> Self {
        Self {
            status: d.status(),
            currency: d.currency_code,
            name: d.currency_name.unwrap_or_default(),
            key: d.period,
            rates: d.rates,
            revaluation: Some(d.revaluation),
            active: d.active,
            confirmed: d.confirmed,
            confirmed_by: d.confirmed_by,
            version: d.version,
        }
    }
}

impl From<DailyRateDto> for Row {
    fn from(d: DailyRateDto) -> Self {
        Self {
            status: d.status(),
            currency: d.currency_code,
            name: d.currency_name.unwrap_or_default(),
            key: d.rate_date.to_string(),
            rates: d.rates,
            revaluation: None,
            active: d.active,
            confirmed: d.confirmed,
            confirmed_by: d.confirmed_by,
            version: d.version,
        }
    }
}

fn get(r: &RatesDto, field: &str) -> Decimal {
    match field {
        "bank_buy" => r.bank_buy,
        "bank_sell" => r.bank_sell,
        "bank_mid" => r.bank_mid,
        "customs_period" => r.customs_period,
        "customs_buy" => r.customs_buy,
        _ => r.customs_sell,
    }
}

fn set(r: &mut RatesDto, field: &str, v: Decimal) {
    match field {
        "bank_buy" => r.bank_buy = v,
        "bank_sell" => r.bank_sell = v,
        "bank_mid" => r.bank_mid = v,
        "customs_period" => r.customs_period = v,
        "customs_buy" => r.customs_buy = v,
        _ => r.customs_sell = v,
    }
}

/// 編輯中的表單：數值以字串保存，存檔時才轉 Decimal
#[derive(Clone, Default, PartialEq)]
struct Form {
    currency: String,
    key: String,
    rates: [String; 6],
    revaluation: String,
    version: Option<i32>,
}

impl Form {
    fn of(r: &RatesDto) -> [String; 6] {
        RATES.map(|(_, f)| get(r, f).normalize().to_string())
    }

    fn from_row(r: &Row) -> Self {
        Self {
            currency: r.currency.clone(),
            key: r.key.clone(),
            rates: Self::of(&r.rates),
            revaluation: r
                .revaluation
                .map(|v| v.normalize().to_string())
                .unwrap_or_default(),
            version: Some(r.version),
        }
    }

    /// 空白視為 0；不是數字時回錯誤（欄位名稱）
    fn parsed(&self) -> Result<(RatesDto, Decimal), String> {
        let num = |label: &str, v: &str| -> Result<Decimal, String> {
            let v = v.trim();
            if v.is_empty() {
                return Ok(Decimal::ZERO);
            }
            v.parse()
                .map_err(|_| tf("rate-msg-not-number", &[("field", t(label).as_str())]))
        };
        let mut rates = RatesDto::default();
        for (i, (label, field)) in RATES.iter().enumerate() {
            set(&mut rates, field, num(label, &self.rates[i])?);
        }
        Ok((rates, num("rate-field-revaluation", &self.revaluation)?))
    }
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(Box<Row>),
}

#[component]
pub fn MonthlyRateMaintenance() -> Element {
    rsx! {
        ProgramGate { program: Kind::Monthly.program(), RatePage { kind: Kind::Monthly } }
    }
}

#[component]
pub fn DailyRateMaintenance() -> Element {
    rsx! {
        ProgramGate { program: Kind::Daily.program(), RatePage { kind: Kind::Daily } }
    }
}

async fn query(kind: Kind, req: QueryRequest) -> Result<ttrs_shared::Page<Row>, String> {
    Ok(match kind {
        Kind::Monthly => {
            let p = api::query_monthly_rates(&req).await?;
            ttrs_shared::Page {
                items: p.items.into_iter().map(Row::from).collect(),
                total: p.total,
                page: p.page,
                page_size: p.page_size,
            }
        }
        Kind::Daily => {
            let p = api::query_daily_rates(&req).await?;
            ttrs_shared::Page {
                items: p.items.into_iter().map(Row::from).collect(),
                total: p.total,
                page: p.page,
                page_size: p.page_size,
            }
        }
    })
}

fn date_of(key: &str) -> Result<ttrs_shared::NaiveDate, String> {
    key.trim().parse().map_err(|_| t("rate-msg-date-required"))
}

async fn save(kind: Kind, editing: Option<Row>, f: Form) -> Result<Row, String> {
    let (rates, revaluation) = f.parsed()?;
    match kind {
        Kind::Monthly => {
            let req = MonthlyRateInput {
                currency_code: f.currency.trim().to_owned(),
                period: f.key.trim().to_owned(),
                rates,
                revaluation,
                version: f.version,
            };
            let r = match editing {
                Some(r) => api::update_monthly_rate(&r.currency, &r.key, &req).await?,
                None => api::create_monthly_rate(&req).await?,
            };
            Ok(r.into())
        }
        Kind::Daily => {
            let req = DailyRateInput {
                currency_code: f.currency.trim().to_owned(),
                rate_date: date_of(&f.key)?,
                rates,
                version: f.version,
            };
            let r = match editing {
                Some(r) => api::update_daily_rate(&r.currency, &r.key, &req).await?,
                None => api::create_daily_rate(&req).await?,
            };
            Ok(r.into())
        }
    }
}

async fn set_state(kind: Kind, r: &Row, state: &str) -> Result<Row, String> {
    Ok(match kind {
        Kind::Monthly => api::set_monthly_rate_state(&r.currency, &r.key, state, r.version)
            .await?
            .into(),
        Kind::Daily => api::set_daily_rate_state(&r.currency, &r.key, state, r.version)
            .await?
            .into(),
    })
}

async fn copy(kind: Kind, r: &Row, currency: &str, key: &str) -> Result<Row, String> {
    Ok(match kind {
        Kind::Monthly => api::copy_monthly_rate(&r.currency, &r.key, currency, key)
            .await?
            .into(),
        Kind::Daily => api::copy_daily_rate(&r.currency, &r.key, currency, key)
            .await?
            .into(),
    })
}

async fn delete(kind: Kind, r: &Row) -> Result<(), String> {
    match kind {
        Kind::Monthly => api::delete_monthly_rate(&r.currency, &r.key, r.version).await,
        Kind::Daily => api::delete_daily_rate(&r.currency, &r.key, r.version).await,
    }
}

fn batch_message(kind: Kind, r: &RateBatchResult) -> String {
    let done = r.processed.join(", ");
    let skipped = r.skipped.join(", ");
    let id = if r.skipped.is_empty() {
        "msg-batch-done"
    } else {
        "msg-batch-skipped"
    };
    tf(
        &kind.id(id),
        &[
            ("count", r.processed.len().to_string().as_str()),
            ("done", done.as_str()),
            ("skipped", skipped.as_str()),
        ],
    )
}

/// 今天（YYYY-MM-DD；每日匯率新增時的預設日期，aooi070.4gl:423）
pub(super) fn today() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let d = js_sys::Date::new_0();
        format!(
            "{:04}-{:02}-{:02}",
            d.get_full_year(),
            d.get_month() + 1,
            d.get_date()
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        String::new()
    }
}

fn round10(v: Decimal) -> Decimal {
    v.round_dp(10).normalize()
}

#[component]
fn RatePage(kind: Kind) -> Element {
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
        async move { query(kind, req).await }
    });
    // 幣別選單與計算方式（匯率換算 s_rate 用）
    let currencies = use_resource(move || async move {
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
    let mut selected = use_signal(|| None::<String>);
    let mut copy_currency = use_signal(String::new);
    let mut copy_key = use_signal(String::new);
    let mut batch_date = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        rows.read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(Row::id).collect())
            .unwrap_or_default()
    });
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(Form::default);
    let mut dialog_error = use_signal(|| None::<String>);
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);
    // 匯率換算（s_rate）：「1 本國幣 = ? 外幣」與要填入的欄位
    let mut convert_value = use_signal(String::new);
    let mut convert_target = use_signal(|| "bank_mid".to_owned());

    let page_data = rows.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<Row> = page_data
        .as_ref()
        .map(|p| p.items.clone())
        .unwrap_or_default();
    let total = page_data.as_ref().map_or(0, |p| p.total);
    let pages = page_data.as_ref().map_or(1, |p| {
        u32::try_from(p.total.div_ceil(u64::from(p.page_size.max(1))))
            .unwrap_or(1)
            .max(1)
    });
    let filtered = !conditions.read().is_empty();
    let cur: Option<Row> = selected()
        .and_then(|k| list.iter().find(|x| x.id() == k).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let keys: Vec<String> = list.iter().map(Row::id).collect();
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let monthly = kind == Kind::Monthly;
    let rate_cols = RATES.len() + usize::from(monthly);
    let cols = if status_tab {
        3 + status::ALL.len() + 2
    } else {
        3 + rate_cols
    };
    let currency_list: Vec<CurrencyDto> = currencies.read().clone().unwrap_or_default();
    let currency_options: Vec<(String, String)> = std::iter::once((String::new(), String::new()))
        .chain(currency_list.iter().filter(|c| c.active).map(|c| {
            let label = match &c.name {
                Some(n) => format!("{} {n}", c.currency_code), // i18n-allow
                None => c.currency_code.clone(),
            };
            (c.currency_code.clone(), label)
        }))
        .collect();

    let mut done = move |msg: String, key: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(key);
        rows.restart();
    };

    let run_query = move |_| {
        let conds = conditions_of(&qbe.read());
        let req = QueryRequest {
            conditions: conds.clone(),
            page: Some(1),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        spawn(async move {
            match query(kind, req).await {
                Ok(result) => {
                    error.set(None);
                    querying.set(false);
                    selected.set(None);
                    conditions.set(conds);
                    page.set(1);
                    message.set(Some(if result.total == 0 {
                        t("query-msg-none")
                    } else {
                        tf(
                            "query-msg-matched",
                            &[("count", result.total.to_string().as_str())],
                        )
                    }));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    // 列印、匯出 Excel（output／exporttoexcel；ADR 0013 SA3）：依目前生效的查詢條件
    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        spawn(async move {
            let name = kind.program();
            if let Err(e) = api::system_list_report(
                kind.api_path(),
                name,
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

    let save_form = move |_| {
        let editing = match editor() {
            Some(Editor::Edit(r)) => Some(*r),
            _ => None,
        };
        let f = form();
        spawn(async move {
            match save(kind, editing, f).await {
                Ok(r) => done(
                    tf("rate-msg-saved", &[("key", r.id().as_str())]),
                    Some(r.id()),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    // 選幣別後帶入預設匯率（每日匯率新增，DR4）
    let pick_currency = move |code: String| {
        form.write().currency = code.clone();
        if kind != Kind::Daily || editor() != Some(Editor::New) || code.is_empty() {
            return;
        }
        spawn(async move {
            match api::daily_rate_defaults(&code).await {
                Ok(d) => form.write().rates = Form::of(&d),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_edit = cur.clone();
    let cur_delete = cur.clone();
    let cur_history = cur.clone();
    let cur_toggle = cur.clone();
    let cur_copy = cur.clone();
    let cur_confirm = cur.clone();
    let cur_unify = cur.clone();
    let inactive = cur.as_ref().is_some_and(|c| !c.active);
    let confirmed = cur.as_ref().is_some_and(|c| c.confirmed);
    let v = form();
    // 匯率換算只給計算方式為「除」的幣別（s_rate，MR9）
    let divide = currency_list
        .iter()
        .any(|c| c.currency_code == v.currency && c.rate_method == "2");

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t(&kind.id("breadcrumb"))} }
                h2 { {t(&kind.id("title"))} }
            }

            if querying() {
                div { class: "action-bar",
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { error.set(None); querying.set(false); }, {t("common-action-cancel")} }
                }
                p { class: "query-hint", {t("query-hint")} }
            } else {
                div { class: "action-bar",
                    MaintenanceToolbar {
                        query: Act::new(true, move |_| { message.set(None); error.set(None); qbe.set(Default::default()); querying.set(true); }),
                        insert: Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            convert_value.set(String::new());
                            let mut f = Form::default();
                            if kind == Kind::Daily {
                                f.key = today();
                            }
                            form.set(f);
                            editor.set(Some(Editor::New));
                        }),
                        // 停用的不可修改（MR6）
                        modify: Act::new(!no_cur && !inactive, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            convert_value.set(String::new());
                            form.set(Form::from_row(&c));
                            editor.set(Some(Editor::Edit(Box::new(c))));
                        }),
                        // 複製（ADR 0013 SA5）：新幣別＋新年月／日期輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_currency().trim().is_empty() && !copy_key().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let (to, key) = (copy_currency(), copy_key());
                            spawn(async move {
                                match copy(kind, &c, to.trim(), key.trim()).await {
                                    Ok(r) => {
                                        copy_currency.set(String::new());
                                        copy_key.set(String::new());
                                        done(tf("rate-msg-copied", &[("from", c.id().as_str()), ("to", r.id().as_str())]), Some(r.id()));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）
                        delete: Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            spawn(async move {
                                match delete(kind, &c).await {
                                    Ok(()) => done(tf("rate-msg-deleted", &[("key", c.id().as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            spawn(async move {
                                let state = if c.confirmed { "unconfirm" } else { "confirm" };
                                match set_state(kind, &c, state).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "rate-msg-confirmed" } else { "rate-msg-unconfirmed" };
                                        done(tf(id, &[("key", r.id().as_str())]), Some(r.id()));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            spawn(async move {
                                let state = if c.active { "deactivate" } else { "activate" };
                                match set_state(kind, &c, state).await {
                                    Ok(r) => {
                                        let id = if r.active { "rate-msg-activated" } else { "rate-msg-deactivated" };
                                        done(tf(id, &[("key", r.id().as_str())]), Some(r.id()));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        invalid_action: "invalid",
                        inactive,
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        history: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_history.clone() else { return };
                            spawn(async move {
                                match api::shared_history(kind.program(), &c.currency).await {
                                    Ok(h) => history.set(Some((c.currency.clone(), h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        if perm.can("reproduce") {
                            TextField { id: "copy-currency", label: t("rate-field-copy-currency"), maxlength: 4, value: copy_currency(), disabled: no_cur, oninput: move |v| copy_currency.set(v) }
                            TextField {
                                id: "copy-key",
                                label: t(&kind.id("field-copy-key")),
                                input_type: if monthly { "text" } else { "date" },
                                maxlength: if monthly { Some(6) } else { None },
                                value: copy_key(),
                                disabled: no_cur,
                                oninput: move |v| copy_key.set(v),
                            }
                        }
                        // 月底重評價（re_rate，MR7）：輸入日期後執行
                        if monthly && perm.can("re_rate") {
                            TextField { id: "reval-date", label: t("monthly-rate-field-reval-date"), input_type: "date", value: batch_date(), oninput: move |v| batch_date.set(v) }
                            Button {
                                icon: icons::SYNC,
                                tone: Tone::Primary,
                                variant: ButtonVariant::Tonal,
                                disabled: batch_date().trim().is_empty(),
                                onclick: move |_| {
                                    let d = batch_date();
                                    spawn(async move {
                                        match api::revaluate_rates(d.trim()).await {
                                            Ok(r) => done(batch_message(kind, &r), None),
                                            Err(e) => error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("monthly-rate-action-revaluate")}
                            }
                        }
                        // 統一匯率（unify_rate，DR7）：以目前這筆的日期
                        if !monthly && perm.can("unify_rate") {
                            Button {
                                icon: icons::SYNC,
                                tone: Tone::Primary,
                                variant: ButtonVariant::Tonal,
                                disabled: no_cur,
                                onclick: move |_| {
                                    let Some(c) = cur_unify.clone() else { return };
                                    spawn(async move {
                                        match api::unify_rates(&c.key).await {
                                            Ok(r) => done(batch_message(kind, &r), Some(c.id())),
                                            Err(e) => error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("daily-rate-action-unify")}
                            }
                        }
                    }
                }
            }

            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.id()).as_deref()),
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

            // 單筆表單：目前這筆
            if let Some(c) = cur.clone() {
                Card {
                    div { class: "form-grid",
                        TextField { id: "cur-currency", label: t("rate-field-currency"), value: c.currency.clone(), readonly: true }
                        TextField { id: "cur-name", label: t("rate-field-currency-name"), value: c.name.clone(), readonly: true }
                        TextField { id: "cur-key", label: t(kind.key_label()), value: c.key.clone(), readonly: true }
                        for (label, field) in RATES {
                            TextField { key: "{field}", id: "cur-{field}", label: t(label), value: get(&c.rates, field).normalize().to_string(), readonly: true }
                        }
                        if let Some(r) = c.revaluation {
                            TextField { id: "cur-revaluation", label: t("rate-field-revaluation"), value: r.normalize().to_string(), readonly: true }
                        }
                    }
                }
            }

            Card {
                Tabs { tabs: vec![t(TAB_DATA), t(TAB_STATUS)], active: col_tab }
                div { class: "table-header",
                    h3 { {t(&kind.id("list-title"))} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("rate-field-currency")} }
                                th { {t("rate-field-currency-name")} }
                                th { {t(kind.key_label())} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else {
                                    for (label, field) in RATES {
                                        th { key: "{field}", {t(label)} }
                                    }
                                    if monthly {
                                        th { {t("rate-field-revaluation")} }
                                    }
                                }
                            }
                            if querying() {
                                tr {
                                    th {
                                        TextField { id: "q-currency_code", label: t("rate-field-currency"), value: qbe.read().get("currency_code").cloned().unwrap_or_default(), oninput: move |v: String| { qbe.write().insert("currency_code", v); } }
                                    }
                                    th {}
                                    th {
                                        TextField {
                                            id: "q-key",
                                            label: t(kind.key_label()),
                                            placeholder: if monthly { String::new() } else { t(HINT_DATE) },
                                            value: qbe.read().get(kind.key_field()).cloned().unwrap_or_default(),
                                            oninput: move |v: String| { qbe.write().insert(kind.key_field(), v); },
                                        }
                                    }
                                    if status_tab {
                                        StatusQbeCells { queryable: status::ALL, qbe }
                                        ConfirmQbeCells { qbe }
                                    } else {
                                        for (label, field) in RATES {
                                            th { key: "{field}",
                                                TextField { id: "q-{field}", label: t(label), value: qbe.read().get(field).cloned().unwrap_or_default(), oninput: move |v: String| { qbe.write().insert(field, v); } }
                                            }
                                        }
                                        if monthly {
                                            th {
                                                TextField { id: "q-revaluation", label: t("rate-field-revaluation"), value: qbe.read().get("revaluation").cloned().unwrap_or_default(), oninput: move |v: String| { qbe.write().insert("revaluation", v); } }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        tbody {
                            match &*rows.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for c in list.iter().cloned() {
                                        tr {
                                            key: "{c.id()}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.id() == c.id()) { "md-table__row--selected" },
                                            onclick: {
                                                let k = c.id();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(k.clone())); }
                                            },
                                            td { class: "mono", "{c.currency}" }
                                            td { "{c.name}" }
                                            td { class: "mono", "{c.key}" }
                                            if status_tab {
                                                StatusCells { status: c.status.clone() }
                                                ConfirmCells { confirmed: c.confirmed, by: c.confirmed_by.clone() }
                                            } else {
                                                for (_, field) in RATES {
                                                    td { key: "{field}", class: "num", {get(&c.rates, field).normalize().to_string()} }
                                                }
                                                if let Some(r) = c.revaluation {
                                                    td { class: "num", {r.normalize().to_string()} }
                                                }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
                ListPager { page, pages }
            }

            if let Some((key, changes)) = history() {
                HistoryDialog { record: key, changes, onclose: move |_| history.set(None) }
            }
            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t(&kind.id("dialog-new")),
                        Editor::Edit(r) => tf(&kind.id("dialog-edit"), &[("key", r.id().as_str())]),
                    },
                    oncancel: move |_| { editor.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { editor.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save_form, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    div { class: "form-grid",
                        if ed == Editor::New {
                            Select { id: "rate-currency", label: t("rate-field-currency"), options: currency_options.clone(), value: v.currency.clone(), onchange: pick_currency }
                        } else {
                            TextField { id: "rate-currency", label: t("rate-field-currency"), value: v.currency.clone(), readonly: true }
                        }
                        TextField {
                            id: "rate-key",
                            label: t(kind.key_label()),
                            input_type: if monthly { "text" } else { "date" },
                            maxlength: if monthly { Some(6) } else { None },
                            placeholder: if monthly { t("monthly-rate-hint-period") } else { String::new() },
                            value: v.key.clone(),
                            readonly: ed != Editor::New,
                            oninput: move |x| form.write().key = x,
                        }
                        for (i, (label, field)) in RATES.into_iter().enumerate() {
                            // 每月匯率的海關旬是月平均，只由月底重評價寫入（MR10）
                            TextField {
                                key: "{field}",
                                id: "rate-{field}",
                                label: t(label),
                                input_type: "number",
                                value: v.rates[i].clone(),
                                readonly: monthly && field == "customs_period",
                                oninput: move |x| form.write().rates[i] = x,
                            }
                        }
                        if monthly {
                            TextField { id: "rate-revaluation", label: t("rate-field-revaluation"), input_type: "number", value: v.revaluation.clone(), oninput: move |x| form.write().revaluation = x }
                        }
                    }
                    // 匯率換算（s_rate，MR9）：輸入「1 本國幣 = ? 外幣」，填入 1 / 數值
                    if divide {
                        div { class: "form-grid",
                            TextField { id: "convert-value", label: t("rate-field-convert"), input_type: "number", value: convert_value(), oninput: move |x| convert_value.set(x) }
                            Select {
                                id: "convert-target",
                                label: t("rate-field-convert-target"),
                                options: RATES.iter().filter(|(_, f)| !(monthly && *f == "customs_period")).map(|(l, f)| ((*f).to_owned(), t(l))).collect::<Vec<_>>(),
                                value: convert_target(),
                                onchange: move |x| convert_target.set(x),
                            }
                            Button {
                                icon: icons::SYNC,
                                tone: Tone::Primary,
                                variant: ButtonVariant::Tonal,
                                onclick: move |_| {
                                    match convert_value().trim().parse::<Decimal>() {
                                        Ok(x) if !x.is_zero() => {
                                            let target = convert_target();
                                            if let Some(i) = RATES.iter().position(|(_, f)| *f == target) {
                                                form.write().rates[i] = round10(Decimal::ONE / x).to_string();
                                            }
                                        }
                                        _ => dialog_error.set(Some(t("rate-msg-convert-invalid"))),
                                    }
                                },
                                {t("rate-action-convert")}
                            }
                        }
                    }
                    p { class: "muted", {t(&kind.id("hint"))} }
                }
            }
        }
    }
}
