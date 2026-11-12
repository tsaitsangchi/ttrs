//! 匯率 use case：每月匯率維護（aooi060 / azj_file）、每日匯率維護（aooi070 / azk_file）、
//! 每日匯率複製（aoop010）、匯率取用（s_exrate）
//! 規格：specs/features/aoo/exchange_rate_monthly.feature（MR1–MR10）、exchange_rate_daily.feature（ER1–ER5、DR1–DR9）、
//! exchange_rate_daily_copy.feature（CP1–CP7）
//!
//! 共用 schema（ER1）。單筆作業：新增 insert、修改 modify、刪除 delete、查詢 query、複製 reproduce、失效 invalid、
//! 列印 output；依 ADR 0013 補上匯出 Excel、異動記錄、確認／取消確認。修改、刪除、失效依資料擁有者套用資料權限（ADR 0006）。
//! 每日匯率異動後重算該月平均寫入每月匯率（i070_mth，DR5）。

use std::{collections::BTreeMap, sync::Arc};

use chrono::{Days, NaiveDate};
use rust_decimal::Decimal;
use ttrs_domain::exchange_rate::feed::{self, BankRateType, Quote};
use ttrs_domain::{
    DomainError,
    currency::{Currency, CurrencyCode},
    exchange_rate::{DailyRate, MonthlyRate, RateKind, RatePeriod, RateRecord, Rates, convert},
    general_parameter::RateBasis,
};
use ttrs_shared::{
    DailyRateDto, DailyRateFillLine, DailyRateInput, MonthlyRateDto, MonthlyRateInput, Page,
    QbeCondition, QueryRequest, RateBatchResult, RateImportResult, RateJobStep, RateSettingsDto,
    RatesDto,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    mail::{Mail, MailTemplate, Mailer},
    ports::{
        AuditEntry, AuditLog, CurrencyRepository, ExchangeRateRepository, RateFeed,
        SystemParameterRepository,
    },
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const MONTHLY: &str = "monthly-exchange-rate-maintenance";
pub const DAILY: &str = "daily-exchange-rate-maintenance";
pub const COPY: &str = "daily-exchange-rate-copy";

const RATE_COLUMNS: [(&str, &str); 6] = [
    ("bank_buy", "rate-col-bank-buy"),
    ("bank_sell", "rate-col-bank-sell"),
    ("bank_mid", "rate-col-bank-mid"),
    ("customs_period", "rate-col-customs-period"),
    ("customs_buy", "rate-col-customs-buy"),
    ("customs_sell", "rate-col-customs-sell"),
];

/// 每月匯率清單報表（aooi060 output；ADR 0013 SA3）
const MONTHLY_REPORT: ListSpec = ListSpec {
    program: MONTHLY,
    title: "monthly-rate-report-title",
    columns: &[
        ("currency_code", "rate-col-currency"),
        ("currency_name", "rate-col-currency-name"),
        ("period", "rate-col-period"),
        RATE_COLUMNS[0],
        RATE_COLUMNS[1],
        RATE_COLUMNS[2],
        RATE_COLUMNS[3],
        RATE_COLUMNS[4],
        RATE_COLUMNS[5],
        ("revaluation", "rate-col-revaluation"),
        ("active", "rate-col-active"),
    ],
};

/// 每日匯率清單報表（aooi070 output）
const DAILY_REPORT: ListSpec = ListSpec {
    program: DAILY,
    title: "daily-rate-report-title",
    columns: &[
        ("currency_code", "rate-col-currency"),
        ("currency_name", "rate-col-currency-name"),
        ("rate_date", "rate-col-date"),
        RATE_COLUMNS[0],
        RATE_COLUMNS[1],
        RATE_COLUMNS[2],
        RATE_COLUMNS[3],
        RATE_COLUMNS[4],
        RATE_COLUMNS[5],
        ("active", "rate-col-active"),
    ],
};

/// 每日匯率複製結果報表（aoop010 報表，CP1）
const FILL_REPORT: ListSpec = ListSpec {
    program: COPY,
    title: "rate-fill-report-title",
    columns: &[
        ("currency_code", "rate-col-currency"),
        ("rate_date", "rate-col-date"),
        RATE_COLUMNS[0],
        RATE_COLUMNS[1],
        RATE_COLUMNS[2],
        RATE_COLUMNS[3],
        RATE_COLUMNS[4],
        RATE_COLUMNS[5],
    ],
};

#[derive(Clone)]
pub struct ExchangeRateService {
    authz: Arc<dyn Authorizer>,
    rates: Arc<dyn ExchangeRateRepository>,
    currencies: Arc<dyn CurrencyRepository>,
    params: Arc<dyn SystemParameterRepository>,
    feed: Arc<dyn RateFeed>,
    audit: Arc<dyn AuditLog>,
    reports: ListReporter,
    /// 系統日期（TipTop g_today）；測試可指定（CP-5）
    today: Option<NaiveDate>,
    /// 每日匯率排程失敗時的通知：寄信工具與管理員信箱（IM8）
    job_mail: Option<(Arc<dyn Mailer>, String)>,
}

impl ExchangeRateService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        rates: Arc<dyn ExchangeRateRepository>,
        currencies: Arc<dyn CurrencyRepository>,
        params: Arc<dyn SystemParameterRepository>,
        feed: Arc<dyn RateFeed>,
        audit: Arc<dyn AuditLog>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            rates,
            currencies,
            params,
            feed,
            audit,
            reports,
            today: None,
            job_mail: None,
        }
    }

    /// 換掉匯率來源（BDD 對 PostgreSQL 也用記憶體來源，不連外網）
    pub fn with_feed(mut self, feed: Arc<dyn RateFeed>) -> Self {
        self.feed = feed;
        self
    }

    /// 每日匯率排程失敗時寄信給管理員（TTRS_ADMIN_MAIL，IM8）
    pub fn with_job_mail(mut self, mailer: Arc<dyn Mailer>, to: &str) -> Self {
        self.job_mail = Some((mailer, to.to_owned()));
        self
    }

    /// 測試用：指定今天
    pub fn with_today(mut self, today: NaiveDate) -> Self {
        self.today = Some(today);
        self
    }

    /// 測試用：調整同步產生報表的筆數上限
    pub fn with_report_limit(mut self, limit: u64) -> Self {
        self.reports.limit = limit;
        self
    }

    // ----- 每月匯率（aooi060） -----

    /// QBE 查詢（aooi060.4gl:201-264；ADR 0013 SA1）：依幣別、年月排序並分頁
    pub async fn query_monthly(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<MonthlyRateDto>, AppError> {
        self.authorize(actor, MONTHLY, "query").await?;
        let filters = query::monthly_rate::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.rates.list_monthly_rates().await.map_err(infra)?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::monthly_rate::value, p);
        let names = self.currency_names().await?;
        Ok(Page {
            items: rows.iter().map(|r| monthly_dto(r, &names)).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    pub async fn get_monthly(
        &self,
        actor: &Actor,
        currency: &str,
        period: &str,
    ) -> Result<MonthlyRateDto, AppError> {
        self.authorize(actor, MONTHLY, "query").await?;
        let r = self.load_monthly(currency, period).await?;
        self.monthly_out(&r).await
    }

    /// 列印 output、匯出 Excel exporttoexcel（aooi060.4gl:1465-1528；ADR 0013 SA3）
    pub async fn report_monthly(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, MONTHLY, format.action()).await?;
        let filters = query::monthly_rate::filters(&conditions)?;
        let all = self.rates.list_monthly_rates().await.map_err(infra)?;
        let (rows, total) =
            query::page_in_memory(&all, &filters, query::monthly_rate::value, self.all_rows());
        self.reports.check(total)?;
        let names = self.currency_names().await?;
        let lines = rows
            .iter()
            .map(|r| {
                let mut line = vec![
                    r.currency.as_str().to_owned(),
                    names.get(r.currency.as_str()).cloned().unwrap_or_default(),
                    r.period.to_string(),
                ];
                line.extend(rate_cells(&r.rates));
                line.push(r.revaluation.to_string());
                line.push(active_text(lang, r.record.active));
                line
            })
            .collect();
        self.reports
            .build(
                actor,
                None,
                &MONTHLY_REPORT,
                lines,
                &conditions,
                format,
                lang,
            )
            .await
    }

    /// 新增（aooi060.4gl:397-587）：幣別須存在且有效（MR5）、年月 YYYYMM（MR2）、不可重複（-239）
    pub async fn create_monthly(
        &self,
        actor: &Actor,
        input: MonthlyRateInput,
    ) -> Result<MonthlyRateDto, AppError> {
        crate::change_log::run(actor, MONTHLY, "insert", async {
            self.authorize(actor, MONTHLY, "insert").await?;
            let currency = self.active_currency(&input.currency_code).await?;
            let period = RatePeriod::parse(&input.period)?;
            let base = self.is_base(&currency.code).await?;
            let rates = Rates {
                customs_period: Decimal::ZERO,
                ..rates_of(&input.rates)
            };
            let r = MonthlyRate::new(
                currency.code,
                period,
                rates,
                input.revaluation,
                base,
                self.record(actor),
            )?;
            self.insert_monthly(&r).await?;
            self.monthly_out(&r).await
        })
        .await
    }

    /// 修改（aooi060.4gl:1019-1213）：鍵值不可改；停用的不可修改（aoo-062，MR6）
    pub async fn update_monthly(
        &self,
        actor: &Actor,
        currency: &str,
        period: &str,
        input: MonthlyRateInput,
    ) -> Result<MonthlyRateDto, AppError> {
        crate::change_log::run(actor, MONTHLY, "modify", async {
            let grant = self.authorize(actor, MONTHLY, "modify").await?;
            let mut r = self.load_monthly(currency, period).await?;
            grant.record(&r.record.created_by, &r.record.created_dept)?;
            r.record.stamp.check(input.version)?;
            r.record.check_modifiable()?;
            let base = self.is_base(&r.currency).await?;
            // 海關旬（月平均）只由月底重評價寫入（MR10）
            let rates = Rates {
                customs_period: r.rates.customs_period,
                ..rates_of(&input.rates)
            };
            r.set_rates(rates, input.revaluation, base)?;
            r.record.touch(&actor.user_id, self.today());
            self.save_monthly(&r).await?;
            self.monthly_out(&r).await
        })
        .await
    }

    /// 失效切換 invalid（aooi060.4gl:1237-1286）
    pub async fn set_monthly_active(
        &self,
        actor: &Actor,
        currency: &str,
        period: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<MonthlyRateDto, AppError> {
        crate::change_log::run(actor, MONTHLY, "invalid", async {
            let grant = self.authorize(actor, MONTHLY, "invalid").await?;
            let mut r = self.load_monthly(currency, period).await?;
            grant.record(&r.record.created_by, &r.record.created_dept)?;
            r.record.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            r.record.active = active;
            r.record.touch(&actor.user_id, self.today());
            self.save_monthly(&r).await?;
            self.monthly_out(&r).await
        })
        .await
    }

    /// 刪除（aooi060.4gl:1288-1342）：已確認不可刪除（SA9-2）
    pub async fn delete_monthly(
        &self,
        actor: &Actor,
        currency: &str,
        period: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, MONTHLY, "delete", async {
            let grant = self.authorize(actor, MONTHLY, "delete").await?;
            let r = self.load_monthly(currency, period).await?;
            grant.record(&r.record.created_by, &r.record.created_dept)?;
            r.record.stamp.check(version)?;
            r.record.confirmation.check_deletable()?;
            match self.rates.delete_monthly_rate(&r.currency, r.period).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::ExchangeRateNotFound.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 複製 reproduce（aooi060.4gl:1344-1463）：新幣別＋新年月
    pub async fn copy_monthly(
        &self,
        actor: &Actor,
        currency: &str,
        period: &str,
        new_currency: &str,
        new_period: &str,
    ) -> Result<MonthlyRateDto, AppError> {
        crate::change_log::run(actor, MONTHLY, "reproduce", async {
            self.authorize(actor, MONTHLY, "reproduce").await?;
            let source = self.load_monthly(currency, period).await?;
            let to = self.active_currency(new_currency).await?;
            let base = self.is_base(&to.code).await?;
            let r = source.copy_as(
                to.code,
                RatePeriod::parse(new_period)?,
                base,
                self.record(actor),
            )?;
            self.insert_monthly(&r).await?;
            self.monthly_out(&r).await
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm_monthly(
        &self,
        actor: &Actor,
        currency: &str,
        period: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<MonthlyRateDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, MONTHLY, action, async {
            self.authorize(actor, MONTHLY, action).await?;
            let mut r = self.load_monthly(currency, period).await?;
            r.record.stamp.check(version)?;
            r.record
                .set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save_monthly(&r).await?;
            self.monthly_out(&r).await
        })
        .await
    }

    /// 月底重評價 re_rate（aooi060.4gl:1589-1719，客製 S202604008；MR7、MR8）：
    /// 當天有有效每日匯率的每個幣別，六種匯率取該月有效每日匯率的平均，月底重評價取當天的銀行中價。
    /// 已確認或停用的每月匯率不覆寫（DR5 (b)），列在略過
    pub async fn revaluate(
        &self,
        actor: &Actor,
        date: NaiveDate,
    ) -> Result<RateBatchResult, AppError> {
        crate::change_log::run(actor, MONTHLY, "re_rate", async {
            self.authorize(actor, MONTHLY, "re_rate").await?;
            let period = RatePeriod::of(date);
            let (first, last) = period.range();
            let month = self
                .rates
                .list_daily_rates(None, Some(first), Some(last))
                .await
                .map_err(infra)?;
            let mut result = RateBatchResult::default();
            for day in month.iter().filter(|r| r.date == date && r.record.active) {
                let avg = Rates::average(
                    month
                        .iter()
                        .filter(|r| r.currency == day.currency && r.record.active)
                        .map(|r| &r.rates),
                )
                .unwrap_or(day.rates);
                let base = self.is_base(&day.currency).await?;
                let code = day.currency.as_str().to_owned();
                match self
                    .rates
                    .get_monthly_rate(&day.currency, period)
                    .await
                    .map_err(infra)?
                {
                    Some(mut m) if m.overwritable() => {
                        m.apply_revaluation(&avg, day.rates.bank_mid, base);
                        m.record.touch(&actor.user_id, self.today());
                        self.save_monthly(&m).await?;
                        result.processed.push(code);
                    }
                    Some(_) => result.skipped.push(code),
                    None => {
                        let mut m = MonthlyRate::new(
                            day.currency.clone(),
                            period,
                            Rates::default(),
                            Decimal::ZERO,
                            base,
                            self.record(actor),
                        )?;
                        m.apply_revaluation(&avg, day.rates.bank_mid, base);
                        self.insert_monthly(&m).await?;
                        result.processed.push(code);
                    }
                }
            }
            Ok(result)
        })
        .await
    }

    // ----- 每日匯率（aooi070） -----

    /// QBE 查詢（aooi070.4gl:201-274）：依幣別、日期排序並分頁
    pub async fn query_daily(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<DailyRateDto>, AppError> {
        self.authorize(actor, DAILY, "query").await?;
        let filters = query::daily_rate::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self
            .rates
            .list_daily_rates(None, None, None)
            .await
            .map_err(infra)?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::daily_rate::value, p);
        let names = self.currency_names().await?;
        Ok(Page {
            items: rows.iter().map(|r| daily_dto(r, &names)).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    pub async fn get_daily(
        &self,
        actor: &Actor,
        currency: &str,
        date: NaiveDate,
    ) -> Result<DailyRateDto, AppError> {
        self.authorize(actor, DAILY, "query").await?;
        let r = self.load_daily(currency, date).await?;
        self.daily_out(&r).await
    }

    /// 列印 output、匯出 Excel exporttoexcel（aooi070.4gl:1213-1286）
    pub async fn report_daily(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, DAILY, format.action()).await?;
        let filters = query::daily_rate::filters(&conditions)?;
        let all = self
            .rates
            .list_daily_rates(None, None, None)
            .await
            .map_err(infra)?;
        let (rows, total) =
            query::page_in_memory(&all, &filters, query::daily_rate::value, self.all_rows());
        self.reports.check(total)?;
        let names = self.currency_names().await?;
        let lines = rows
            .iter()
            .map(|r| {
                let mut line = vec![
                    r.currency.as_str().to_owned(),
                    names.get(r.currency.as_str()).cloned().unwrap_or_default(),
                    r.date.to_string(),
                ];
                line.extend(rate_cells(&r.rates));
                line.push(active_text(lang, r.record.active));
                line
            })
            .collect();
        self.reports
            .build(actor, None, &DAILY_REPORT, lines, &conditions, format, lang)
            .await
    }

    /// 新增時的預設匯率（aooi070.4gl:490-511；DR4）：本國幣別全部 1；
    /// 其他幣別取最近一筆海關買入、賣出都大於 0 的有效每日匯率，沒有就全部 1
    pub async fn daily_defaults(
        &self,
        actor: &Actor,
        currency: &str,
    ) -> Result<RatesDto, AppError> {
        self.authorize(actor, DAILY, "insert").await?;
        let code = CurrencyCode::parse(currency).map_err(|_| DomainError::CurrencyNotFound)?;
        if self.is_base(&code).await? {
            return Ok(rates_dto(&Rates::unit()));
        }
        let list = self
            .rates
            .list_daily_rates(Some(&code), None, None)
            .await
            .map_err(infra)?;
        let found = list.iter().rev().find(|r| {
            r.record.active
                && r.rates.customs_buy > Decimal::ZERO
                && r.rates.customs_sell > Decimal::ZERO
        });
        Ok(rates_dto(&found.map_or_else(Rates::unit, |r| r.rates)))
    }

    /// 新增（aooi070.4gl:415-459）：幣別須存在且有效（aoo-011）；不可重複（-239）；存檔後重算月平均（DR5）
    pub async fn create_daily(
        &self,
        actor: &Actor,
        input: DailyRateInput,
    ) -> Result<DailyRateDto, AppError> {
        crate::change_log::run(actor, DAILY, "insert", async {
            self.authorize(actor, DAILY, "insert").await?;
            let currency = self.active_currency(&input.currency_code).await?;
            let base = self.is_base(&currency.code).await?;
            let r = DailyRate::new(
                currency.code,
                input.rate_date,
                rates_of(&input.rates),
                base,
                self.record(actor),
            )?;
            self.insert_daily(&r).await?;
            self.sync_month(actor, &r.currency, RatePeriod::of(r.date))
                .await?;
            self.daily_out(&r).await
        })
        .await
    }

    /// 修改（aooi070.4gl:967-1027）：鍵值不可改；停用的不可修改（MR6 同規則）；存檔後重算月平均
    pub async fn update_daily(
        &self,
        actor: &Actor,
        currency: &str,
        date: NaiveDate,
        input: DailyRateInput,
    ) -> Result<DailyRateDto, AppError> {
        crate::change_log::run(actor, DAILY, "modify", async {
            let grant = self.authorize(actor, DAILY, "modify").await?;
            let mut r = self.load_daily(currency, date).await?;
            grant.record(&r.record.created_by, &r.record.created_dept)?;
            r.record.stamp.check(input.version)?;
            r.record.check_modifiable()?;
            let base = self.is_base(&r.currency).await?;
            r.set_rates(rates_of(&input.rates), base)?;
            r.record.touch(&actor.user_id, self.today());
            self.save_daily(&r).await?;
            self.sync_month(actor, &r.currency, RatePeriod::of(r.date))
                .await?;
            self.daily_out(&r).await
        })
        .await
    }

    /// 失效切換 invalid（DR3：azk_file 沒有資料有效碼，依 ADR 0013 補上）；停用的不算進月平均
    pub async fn set_daily_active(
        &self,
        actor: &Actor,
        currency: &str,
        date: NaiveDate,
        active: bool,
        version: Option<i32>,
    ) -> Result<DailyRateDto, AppError> {
        crate::change_log::run(actor, DAILY, "invalid", async {
            let grant = self.authorize(actor, DAILY, "invalid").await?;
            let mut r = self.load_daily(currency, date).await?;
            grant.record(&r.record.created_by, &r.record.created_dept)?;
            r.record.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            r.record.active = active;
            r.record.touch(&actor.user_id, self.today());
            self.save_daily(&r).await?;
            self.sync_month(actor, &r.currency, RatePeriod::of(r.date))
                .await?;
            self.daily_out(&r).await
        })
        .await
    }

    /// 刪除（aooi070.4gl:1029-1109）：已確認不可刪除（SA9-2）；刪除後重算月平均（TQC-720007）
    pub async fn delete_daily(
        &self,
        actor: &Actor,
        currency: &str,
        date: NaiveDate,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, DAILY, "delete", async {
            let grant = self.authorize(actor, DAILY, "delete").await?;
            let r = self.load_daily(currency, date).await?;
            grant.record(&r.record.created_by, &r.record.created_dept)?;
            r.record.stamp.check(version)?;
            r.record.confirmation.check_deletable()?;
            match self.rates.delete_daily_rate(&r.currency, r.date).await {
                Ok(true) => {}
                Ok(false) => return Err(DomainError::ExchangeRateNotFound.into()),
                Err(e) => return Err(infra(e)),
            }
            self.sync_month(actor, &r.currency, RatePeriod::of(r.date))
                .await
        })
        .await
    }

    /// 複製 reproduce（aooi070.4gl:1111-1211）：新幣別＋新日期；複製後重算月平均（DR5）
    pub async fn copy_daily(
        &self,
        actor: &Actor,
        currency: &str,
        date: NaiveDate,
        new_currency: &str,
        new_date: NaiveDate,
    ) -> Result<DailyRateDto, AppError> {
        crate::change_log::run(actor, DAILY, "reproduce", async {
            self.authorize(actor, DAILY, "reproduce").await?;
            let source = self.load_daily(currency, date).await?;
            let to = self.active_currency(new_currency).await?;
            let base = self.is_base(&to.code).await?;
            let r = source.copy_as(to.code, new_date, base, self.record(actor))?;
            self.insert_daily(&r).await?;
            self.sync_month(actor, &r.currency, RatePeriod::of(r.date))
                .await?;
            self.daily_out(&r).await
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm_daily(
        &self,
        actor: &Actor,
        currency: &str,
        date: NaiveDate,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<DailyRateDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, DAILY, action, async {
            self.authorize(actor, DAILY, action).await?;
            let mut r = self.load_daily(currency, date).await?;
            r.record.stamp.check(version)?;
            r.record
                .set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save_daily(&r).await?;
            self.daily_out(&r).await
        })
        .await
    }

    /// 統一匯率 unify_rate（aooi070.4gl:1447-1471 cal_rate，客製 10605060；DR7）：該日所有有效每日匯率的
    /// 銀行買入、賣出改為中價，海關旬、海關賣出改為海關買入；已確認的略過；做完重算月平均
    pub async fn unify(&self, actor: &Actor, date: NaiveDate) -> Result<RateBatchResult, AppError> {
        crate::change_log::run(actor, DAILY, "unify_rate", async {
            self.authorize(actor, DAILY, "unify_rate").await?;
            let day = self
                .rates
                .list_daily_rates(None, Some(date), Some(date))
                .await
                .map_err(infra)?;
            let mut result = RateBatchResult::default();
            for mut r in day.into_iter().filter(|r| r.record.active) {
                let code = r.currency.as_str().to_owned();
                if r.record.confirmation.confirmed {
                    result.skipped.push(code);
                    continue;
                }
                let base = self.is_base(&r.currency).await?;
                r.set_rates(r.rates.unified(), base)?;
                r.record.touch(&actor.user_id, self.today());
                self.save_daily(&r).await?;
                self.sync_month(actor, &r.currency, RatePeriod::of(date))
                    .await?;
                result.processed.push(code);
            }
            Ok(result)
        })
        .await
    }

    // ----- 每日匯率複製（aoop010） -----

    /// 每日匯率複製 run（aoop010.4gl:237-654；CP1–CP6）：每個有效幣別取補到日以前最近一筆有效每日匯率，
    /// 複製到隔天至補到日的每一天（已有資料的日子略過）；複製後重算月平均（CP4）
    pub async fn fill(
        &self,
        actor: &Actor,
        until: NaiveDate,
    ) -> Result<Vec<DailyRateFillLine>, AppError> {
        crate::change_log::run(actor, COPY, "run", async {
            self.authorize(actor, COPY, "run").await?;
            if until > self.today() {
                return Err(DomainError::ExchangeRateCopyDateInvalid.into());
            }
            let mut lines = Vec::new();
            let currencies = self.currencies.list_currencies().await.map_err(infra)?;
            for cur in currencies.iter().filter(|c| c.active) {
                let list = self
                    .rates
                    .list_daily_rates(Some(&cur.code), None, Some(until))
                    .await
                    .map_err(infra)?;
                let Some(source) = list.iter().rev().find(|r| r.record.active) else {
                    continue;
                };
                let base = self.is_base(&cur.code).await?;
                let mut months = Vec::new();
                let mut day = source.date;
                while day < until {
                    day = day.checked_add_days(Days::new(1)).expect("日期");
                    if list.iter().any(|r| r.date == day) {
                        continue;
                    }
                    let r = source.copy_as(cur.code.clone(), day, base, self.record(actor))?;
                    self.insert_daily(&r).await?;
                    lines.push(DailyRateFillLine {
                        currency_code: cur.code.as_str().to_owned(),
                        rate_date: day,
                        rates: rates_dto(&r.rates),
                    });
                    let p = RatePeriod::of(day);
                    if !months.contains(&p) {
                        months.push(p);
                    }
                }
                for p in months {
                    self.sync_month(actor, &cur.code, p).await?;
                }
            }
            Ok(lines)
        })
        .await
    }

    /// 每日匯率複製結果的列印、匯出 Excel（aoop010 報表，CP1）
    pub async fn report_fill(
        &self,
        actor: &Actor,
        lines: Vec<DailyRateFillLine>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, COPY, format.action()).await?;
        self.reports.check(lines.len() as u64)?;
        let lines = lines
            .iter()
            .map(|l| {
                let mut line = vec![l.currency_code.clone(), l.rate_date.to_string()];
                line.extend(rate_cells(&rates_of(&l.rates)));
                line
            })
            .collect();
        self.reports
            .build(actor, None, &FILL_REPORT, lines, &[], format, lang)
            .await
    }

    // ----- 匯率自動匯入（aooi070 auto_import；exchange_rate_import.feature IM1–IM9） -----

    /// 匯入台銀牌告匯率（aooi070.4gl:1475-1748）：今天的銀行買入、賣出、中價都存（買入＋賣出）/ 2（IM3）、保留海關匯率；
    /// 今天已有每日匯率時須同意覆寫（IM5）；`csv` 有值時用上傳的檔案（IM1 備援），否則由來源下載
    pub async fn import_bank(
        &self,
        actor: &Actor,
        kind: BankRateType,
        overwrite: bool,
        csv: Option<&str>,
    ) -> Result<RateImportResult, AppError> {
        crate::change_log::run(actor, DAILY, "auto_import", async {
            self.authorize(actor, DAILY, "auto_import").await?;
            let quotes = match csv {
                Some(text) => feed::parse_bot_csv(text, kind)?,
                None => self.feed.bank_quotes(kind).await?,
            };
            let today = self.today();
            let existing = self
                .rates
                .list_daily_rates(None, Some(today), Some(today))
                .await
                .map_err(infra)?;
            if !overwrite && !existing.is_empty() {
                return Err(DomainError::ExchangeRateImportOverwriteRequired.into());
            }
            self.apply_quotes(actor, today, &quotes, Side::Bank).await
        })
        .await
    }

    /// 匯入關務署每旬報關匯率（aooi070.4gl:1752-2190）：海關旬、買入、賣出都存海關買入（IM4）、保留銀行匯率；
    /// 當旬檔不是今天的旬或少於 5 筆時用歷史檔；沒有報價但今天有每日匯率的幣別，取本旬已有的第一筆海關匯率
    pub async fn import_customs(&self, actor: &Actor) -> Result<RateImportResult, AppError> {
        crate::change_log::run(actor, DAILY, "auto_import", async {
            self.authorize(actor, DAILY, "auto_import").await?;
            let today = self.today();
            let current = self.feed.customs_current().await;
            let covered =
                matches!(&current, Ok((s, e, q)) if *s <= today && today <= *e && q.len() >= 5);
            let history = if covered {
                Vec::new()
            } else {
                match self.feed.customs_history().await {
                    Ok(h) => h,
                    // 歷史檔也拿不到時，只有當旬檔可用才繼續
                    Err(e) if current.is_err() => return Err(e.into()),
                    Err(_) => Vec::new(),
                }
            };
            let quotes = feed::choose_customs(today, current.ok(), || history);
            let mut result = self
                .apply_quotes(actor, today, &quotes, Side::Customs)
                .await?;
            self.fill_missing_customs(actor, today, &quotes, &mut result)
                .await?;
            Ok(result)
        })
        .await
    }

    /// 每日匯率排程（IM8）：台銀即期匯入 → 海關匯入 → 每日匯率複製（補到今天）→ 月底重評價（昨天，MR8）。
    /// 某一步失敗仍繼續；有失敗時寄信給管理員、全部結果記在稽核紀錄
    pub async fn run_daily_job(&self, actor: &Actor) -> Vec<RateJobStep> {
        let today = self.today();
        let yesterday = today.pred_opt().unwrap_or(today);
        let summary = |r: &RateImportResult| {
            format!(
                "匯入 {}；略過 {}；警告 {}",
                r.imported.join(","),
                r.skipped.join(","),
                r.warnings.join(",")
            )
        };
        let mut steps = Vec::new();
        let mut push = |step: &str, r: Result<String, AppError>| {
            steps.push(match r {
                Ok(message) => RateJobStep {
                    step: step.to_owned(),
                    ok: true,
                    message,
                },
                Err(e) => RateJobStep {
                    step: step.to_owned(),
                    ok: false,
                    message: e.to_string(),
                },
            });
        };
        push(
            "台銀匯入",
            self.import_bank(actor, BankRateType::Spot, true, None)
                .await
                .map(|r| summary(&r)),
        );
        push(
            "海關匯入",
            self.import_customs(actor).await.map(|r| summary(&r)),
        );
        push(
            "每日匯率複製",
            self.fill(actor, today)
                .await
                .map(|l| format!("複製 {} 筆", l.len())),
        );
        push(
            "月底重評價",
            self.revaluate(actor, yesterday).await.map(|r| {
                format!(
                    "處理 {}；略過 {}",
                    r.processed.join(","),
                    r.skipped.join(",")
                )
            }),
        );
        let failed = steps.iter().any(|s| !s.ok);
        let detail = steps
            .iter()
            .map(|s| {
                format!(
                    "{}：{}（{}）",
                    s.step,
                    if s.ok { "成功" } else { "失敗" },
                    s.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let _ = self
            .audit
            .record(&AuditEntry {
                program: DAILY.to_owned(),
                user_id: actor.user_id.clone(),
                key: today.to_string(),
                action: "rate_job".to_owned(),
                detail: Some(detail.clone()),
            })
            .await;
        if failed && let Some((mailer, to)) = &self.job_mail {
            let mail = Mail::render(
                to,
                &ttrs_domain::lang::Lang::default_lang(),
                MailTemplate::RateJobFailed,
                &[
                    ("date", today.to_string().as_str()),
                    ("detail", detail.as_str()),
                ],
            );
            if let Err(e) = mailer.send(&mail).await {
                tracing::warn!(error = %e, "每日匯率排程失敗通知寄送失敗");
            }
        }
        steps
    }

    /// 把報價寫入今天的每日匯率：停用或不存在的幣別略過；報價為 0、已確認的列在警告（IM5、IM7）；
    /// 補本國幣別全部為 1 的一筆（i070_auto_ins_ntd）；重算月平均
    async fn apply_quotes(
        &self,
        actor: &Actor,
        today: NaiveDate,
        quotes: &[Quote],
        side: Side,
    ) -> Result<RateImportResult, AppError> {
        let currencies = self.currencies.list_currencies().await.map_err(infra)?;
        let mut result = RateImportResult::default();
        for q in quotes {
            // 匯入代號對照（IM6）：先找匯入代號相同的幣別，沒有再找同代號
            let cur = currencies
                .iter()
                .find(|c| c.attrs.import_code.as_deref() == Some(q.code.as_str()))
                .or_else(|| currencies.iter().find(|c| c.code.as_str() == q.code));
            let Some(cur) = cur.filter(|c| c.active) else {
                result
                    .skipped
                    .push(cur.map_or(q.code.clone(), |c| c.code.as_str().to_owned()));
                continue;
            };
            let code = cur.code.as_str().to_owned();
            let zero = match side {
                Side::Bank => q.buy.is_zero() || q.sell.is_zero(),
                Side::Customs => q.buy.is_zero(),
            };
            if zero {
                result.warnings.push(code);
                continue;
            }
            let value = match side {
                Side::Bank => (q.buy + q.sell) / Decimal::TWO,
                Side::Customs => q.buy,
            };
            if self
                .upsert_side(actor, &cur.code, today, side, value)
                .await?
            {
                result.imported.push(code);
            } else {
                result.warnings.push(code);
            }
        }
        self.ensure_base_row(actor, today).await?;
        Ok(result)
    }

    /// 寫入今天一個幣別的銀行或海關匯率；已確認的不覆寫（回 false）
    async fn upsert_side(
        &self,
        actor: &Actor,
        code: &CurrencyCode,
        date: NaiveDate,
        side: Side,
        value: Decimal,
    ) -> Result<bool, AppError> {
        let base = self.is_base(code).await?;
        let set = |r: &mut Rates| match side {
            Side::Bank => {
                r.bank_buy = value;
                r.bank_sell = value;
                r.bank_mid = value;
            }
            Side::Customs => {
                r.customs_period = value;
                r.customs_buy = value;
                r.customs_sell = value;
            }
        };
        match self.rates.get_daily_rate(code, date).await.map_err(infra)? {
            Some(r) if r.record.confirmation.confirmed => return Ok(false),
            Some(mut r) => {
                let mut rates = r.rates;
                set(&mut rates);
                r.set_rates(rates, base)?;
                r.record.active = true;
                r.record.touch(&actor.user_id, self.today());
                self.save_daily(&r).await?;
            }
            None => {
                let mut rates = Rates::default();
                set(&mut rates);
                let r = DailyRate::new(code.clone(), date, rates, base, self.record(actor))?;
                self.insert_daily(&r).await?;
            }
        }
        self.sync_month(actor, code, RatePeriod::of(date)).await?;
        Ok(true)
    }

    /// 本國幣別補一筆全部為 1 的每日匯率（aooi070.4gl:2313-2360；IM7）
    async fn ensure_base_row(&self, actor: &Actor, date: NaiveDate) -> Result<(), AppError> {
        let Some(base) = self.params.base_currency().await.map_err(infra)? else {
            return Ok(());
        };
        let Ok(code) = CurrencyCode::parse(&base) else {
            return Ok(());
        };
        let active = self
            .currencies
            .get_currency(&code)
            .await
            .map_err(infra)?
            .is_some_and(|c| c.active);
        if !active {
            return Ok(());
        }
        match self
            .rates
            .get_daily_rate(&code, date)
            .await
            .map_err(infra)?
        {
            Some(r) if r.record.confirmation.confirmed || r.rates == Rates::unit() => Ok(()),
            Some(mut r) => {
                r.set_rates(Rates::unit(), true)?;
                r.record.touch(&actor.user_id, self.today());
                self.save_daily(&r).await
            }
            None => {
                let r = DailyRate::new(code, date, Rates::unit(), true, self.record(actor))?;
                self.insert_daily(&r).await?;
                self.sync_month(actor, &r.currency, RatePeriod::of(date))
                    .await
            }
        }
    }

    /// 當旬沒有海關報價、今天卻有每日匯率的幣別：取本旬已有的第一筆海關匯率（i070_date_set，aoo1006；IM4）
    async fn fill_missing_customs(
        &self,
        actor: &Actor,
        today: NaiveDate,
        quotes: &[Quote],
        result: &mut RateImportResult,
    ) -> Result<(), AppError> {
        let (_, start, _) = feed::ten_day(today);
        let base = self.params.base_currency().await.map_err(infra)?;
        let currencies = self.currencies.list_currencies().await.map_err(infra)?;
        let quoted = |code: &str| {
            quotes.iter().any(|q| {
                q.code == code
                    || currencies.iter().any(|c| {
                        c.code.as_str() == code
                            && c.attrs.import_code.as_deref() == Some(q.code.as_str())
                    })
            })
        };
        let day = self
            .rates
            .list_daily_rates(None, Some(today), Some(today))
            .await
            .map_err(infra)?;
        for r in day
            .iter()
            .filter(|r| r.record.active && r.rates.customs_buy.is_zero())
        {
            let code = r.currency.as_str();
            if base.as_deref() == Some(code) || quoted(code) {
                continue;
            }
            let earlier = self
                .rates
                .list_daily_rates(Some(&r.currency), Some(start), today.pred_opt())
                .await
                .map_err(infra)?;
            match earlier
                .iter()
                .find(|x| x.record.active && !x.rates.customs_buy.is_zero())
            {
                Some(first) => {
                    if self
                        .upsert_side(
                            actor,
                            &r.currency,
                            today,
                            Side::Customs,
                            first.rates.customs_buy,
                        )
                        .await?
                    {
                        result.imported.push(code.to_owned());
                    }
                }
                None => result.warnings.push(code.to_owned()),
            }
        }
        Ok(())
    }

    // ----- 匯率取用（s_exrate；ER5） -----

    /// 匯率取用：兩幣別各取「日期 <= 指定日」最近一筆有效每日匯率，本國幣別為 1，原幣 / 目的幣。
    /// 供其他模組的 use case 呼叫（不另做權限檢查，由呼叫端的作業檢查）；找不到回 EXCHANGE_RATE_NOT_FOUND
    pub async fn rate(
        &self,
        from: &str,
        to: &str,
        date: NaiveDate,
        kind: RateKind,
    ) -> Result<Decimal, AppError> {
        if from == to {
            return Ok(Decimal::ONE);
        }
        let a = self.latest_rate(from, date, kind).await?;
        let b = self.latest_rate(to, date, kind).await?;
        Ok(convert(a, b)?)
    }

    /// 取一個幣別的匯率（s_curr.4gl:46-66、s_curr3.4gl:56-120）：本國幣別為 1；依整體管理系統參數的匯率採用（GP6）——
    /// 每月取該年月、沒有時取以前最近的年月的有效每月匯率；每日取當天或以前最近一天的有效每日匯率（DR8）
    async fn latest_rate(
        &self,
        currency: &str,
        date: NaiveDate,
        kind: RateKind,
    ) -> Result<Decimal, AppError> {
        let code = CurrencyCode::parse(currency).map_err(|_| DomainError::CurrencyNotFound)?;
        if self.is_base(&code).await? {
            return Ok(Decimal::ONE);
        }
        let found = match self.rate_basis().await? {
            RateBasis::Monthly => {
                let period = RatePeriod::of(date);
                self.rates
                    .list_monthly_rates()
                    .await
                    .map_err(infra)?
                    .into_iter()
                    .filter(|r| r.currency == code && r.period <= period && r.record.active)
                    .max_by_key(|r| r.period)
                    .map(|r| r.rates.get(kind))
            }
            RateBasis::Daily => self
                .rates
                .list_daily_rates(Some(&code), None, Some(date))
                .await
                .map_err(infra)?
                .iter()
                .rev()
                .find(|r| r.record.active)
                .map(|r| r.rates.get(kind)),
        };
        found.ok_or_else(|| DomainError::ExchangeRateNotFound.into())
    }

    /// 匯率採用（aza19）；沒有參數資料時為每日（aoos010.4gl:278）
    async fn rate_basis(&self) -> Result<RateBasis, AppError> {
        Ok(self
            .params
            .general_parameters()
            .await
            .map_err(infra)?
            .map_or(RateBasis::Daily, |p| p.rate_basis))
    }

    /// 匯率維護畫面的設定（GP7；aooi060.4gl:177 aoo-066、aooi070.4gl:169 aoo-059）：本國幣別、匯率採用，
    /// 以及本作業的匯率不會被單據取用時的提示
    pub async fn settings(
        &self,
        actor: &Actor,
        program: &str,
    ) -> Result<RateSettingsDto, AppError> {
        let (program, unused_when) = match program.trim() {
            MONTHLY => (MONTHLY, RateBasis::Daily),
            DAILY => (DAILY, RateBasis::Monthly),
            _ => return Err(DomainError::ProgramNotFound.into()),
        };
        self.authorize(actor, program, "query").await?;
        let basis = self.rate_basis().await?;
        let base = self
            .params
            .base_currency()
            .await
            .map_err(infra)?
            .unwrap_or_default();
        let hint = (basis == unused_when).then(|| {
            if program == MONTHLY {
                "monthly-unused".to_owned()
            } else {
                "daily-unused".to_owned()
            }
        });
        Ok(RateSettingsDto {
            base_currency: base,
            rate_basis: basis.code().to_owned(),
            hint,
        })
    }

    // ----- 共用 -----

    /// 月平均同步（aooi070.4gl:741-819 i070_mth；DR5）：該月有效每日匯率的平均寫入每月匯率；
    /// 沒有有效每日匯率時保留原值（DR5 (a)）；已確認或停用的每月匯率不覆寫（DR5 (b)）；不存在就新增
    async fn sync_month(
        &self,
        actor: &Actor,
        currency: &CurrencyCode,
        period: RatePeriod,
    ) -> Result<(), AppError> {
        let (first, last) = period.range();
        let list = self
            .rates
            .list_daily_rates(Some(currency), Some(first), Some(last))
            .await
            .map_err(infra)?;
        let Some(avg) = Rates::average(list.iter().filter(|r| r.record.active).map(|r| &r.rates))
        else {
            return Ok(());
        };
        let base = self.is_base(currency).await?;
        match self
            .rates
            .get_monthly_rate(currency, period)
            .await
            .map_err(infra)?
        {
            Some(mut m) => {
                if m.overwritable() {
                    let before = m.rates;
                    m.apply_daily_average(&avg, base);
                    if m.rates != before {
                        m.record.touch(&actor.user_id, self.today());
                        self.save_monthly(&m).await?;
                    }
                }
                Ok(())
            }
            None => {
                let mut m = MonthlyRate::new(
                    currency.clone(),
                    period,
                    Rates::default(),
                    Decimal::ZERO,
                    base,
                    self.record(actor),
                )?;
                m.apply_daily_average(&avg, base);
                self.insert_monthly(&m).await
            }
        }
    }

    async fn insert_monthly(&self, r: &MonthlyRate) -> Result<(), AppError> {
        match self.rates.insert_monthly_rate(r).await {
            Ok(()) => Ok(()),
            Err(RepoError::Conflict) => Err(DomainError::ExchangeRateAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save_monthly(&self, r: &MonthlyRate) -> Result<(), AppError> {
        self.rates.update_monthly_rate(r).await.map_err(infra)
    }

    async fn insert_daily(&self, r: &DailyRate) -> Result<(), AppError> {
        match self.rates.insert_daily_rate(r).await {
            Ok(()) => Ok(()),
            Err(RepoError::Conflict) => Err(DomainError::ExchangeRateAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save_daily(&self, r: &DailyRate) -> Result<(), AppError> {
        self.rates.update_daily_rate(r).await.map_err(infra)
    }

    async fn monthly_out(&self, r: &MonthlyRate) -> Result<MonthlyRateDto, AppError> {
        // 重讀以取得資料庫給的版本與時間
        let r = self
            .rates
            .get_monthly_rate(&r.currency, r.period)
            .await
            .map_err(infra)?
            .ok_or(DomainError::ExchangeRateNotFound)?;
        Ok(monthly_dto(&r, &self.currency_names().await?))
    }

    async fn daily_out(&self, r: &DailyRate) -> Result<DailyRateDto, AppError> {
        let r = self
            .rates
            .get_daily_rate(&r.currency, r.date)
            .await
            .map_err(infra)?
            .ok_or(DomainError::ExchangeRateNotFound)?;
        Ok(daily_dto(&r, &self.currency_names().await?))
    }

    async fn load_monthly(&self, currency: &str, period: &str) -> Result<MonthlyRate, AppError> {
        let code = CurrencyCode::parse(currency).map_err(|_| DomainError::ExchangeRateNotFound)?;
        let period = RatePeriod::parse(period).map_err(|_| DomainError::ExchangeRateNotFound)?;
        self.rates
            .get_monthly_rate(&code, period)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::ExchangeRateNotFound.into())
    }

    async fn load_daily(&self, currency: &str, date: NaiveDate) -> Result<DailyRate, AppError> {
        let code = CurrencyCode::parse(currency).map_err(|_| DomainError::ExchangeRateNotFound)?;
        self.rates
            .get_daily_rate(&code, date)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::ExchangeRateNotFound.into())
    }

    /// 幣別須存在（CURRENCY_NOT_FOUND）且有效（CURRENCY_INACTIVE）（aooi060.4gl:611-622、aooi070.4gl:473-488；MR5）
    async fn active_currency(&self, code: &str) -> Result<Currency, AppError> {
        let code = CurrencyCode::parse(code).map_err(|_| DomainError::CurrencyNotFound)?;
        let cur = self
            .currencies
            .get_currency(&code)
            .await
            .map_err(infra)?
            .ok_or(DomainError::CurrencyNotFound)?;
        if !cur.active {
            return Err(DomainError::CurrencyInactive.into());
        }
        Ok(cur)
    }

    /// 是否為本國幣別（aza17；ER2）
    async fn is_base(&self, code: &CurrencyCode) -> Result<bool, AppError> {
        Ok(self.params.base_currency().await.map_err(infra)?.as_deref() == Some(code.as_str()))
    }

    async fn currency_names(&self) -> Result<BTreeMap<String, String>, AppError> {
        Ok(self
            .currencies
            .list_currencies()
            .await
            .map_err(infra)?
            .into_iter()
            .filter_map(|c| c.attrs.name.map(|n| (c.code.as_str().to_owned(), n)))
            .collect())
    }

    fn record(&self, actor: &Actor) -> RateRecord {
        RateRecord::new(&actor.user_id, &actor.dept, self.today())
    }

    fn all_rows(&self) -> query::Paging {
        query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        }
    }

    fn today(&self) -> NaiveDate {
        self.today
            .unwrap_or_else(|| chrono::Local::now().date_naive())
    }

    async fn authorize(
        &self,
        actor: &Actor,
        program: &'static str,
        action: &'static str,
    ) -> Result<Grant, AppError> {
        self.authz.authorize(actor, program, action).await
    }
}

fn rates_of(d: &RatesDto) -> Rates {
    Rates {
        bank_buy: d.bank_buy,
        bank_sell: d.bank_sell,
        bank_mid: d.bank_mid,
        customs_period: d.customs_period,
        customs_buy: d.customs_buy,
        customs_sell: d.customs_sell,
    }
}

fn rates_dto(r: &Rates) -> RatesDto {
    RatesDto {
        bank_buy: r.bank_buy,
        bank_sell: r.bank_sell,
        bank_mid: r.bank_mid,
        customs_period: r.customs_period,
        customs_buy: r.customs_buy,
        customs_sell: r.customs_sell,
    }
}

fn rate_cells(r: &Rates) -> Vec<String> {
    [
        r.bank_buy,
        r.bank_sell,
        r.bank_mid,
        r.customs_period,
        r.customs_buy,
        r.customs_sell,
    ]
    .iter()
    .map(|v| v.normalize().to_string())
    .collect()
}

fn active_text(lang: &ttrs_domain::lang::Lang, active: bool) -> String {
    report::text(
        lang,
        if active {
            "report-active"
        } else {
            "report-inactive"
        },
    )
}

fn monthly_dto(r: &MonthlyRate, names: &BTreeMap<String, String>) -> MonthlyRateDto {
    let x = &r.record;
    MonthlyRateDto {
        currency_code: r.currency.as_str().to_owned(),
        currency_name: names.get(r.currency.as_str()).cloned(),
        period: r.period.to_string(),
        rates: rates_dto(&r.rates),
        revaluation: r.revaluation,
        active: x.active,
        confirmed: x.confirmation.confirmed,
        confirmed_by: x.confirmation.by.clone(),
        confirmed_at: x.confirmation.at,
        created_by: x.created_by.clone(),
        created_dept: x.created_dept.clone(),
        created_on: x.created_on,
        updated_by: x.updated_by.clone(),
        updated_on: x.updated_on,
        version: x.stamp.version,
        created_at: x.stamp.created_at,
        updated_at: x.stamp.updated_at,
    }
}

fn daily_dto(r: &DailyRate, names: &BTreeMap<String, String>) -> DailyRateDto {
    let x = &r.record;
    DailyRateDto {
        currency_code: r.currency.as_str().to_owned(),
        currency_name: names.get(r.currency.as_str()).cloned(),
        rate_date: r.date,
        rates: rates_dto(&r.rates),
        active: x.active,
        confirmed: x.confirmation.confirmed,
        confirmed_by: x.confirmation.by.clone(),
        confirmed_at: x.confirmation.at,
        created_by: x.created_by.clone(),
        created_dept: x.created_dept.clone(),
        created_on: x.created_on,
        updated_by: x.updated_by.clone(),
        updated_on: x.updated_on,
        version: x.stamp.version,
        created_at: x.stamp.created_at,
        updated_at: x.stamp.updated_at,
    }
}

fn infra(e: RepoError) -> AppError {
    match e {
        RepoError::Stale => DomainError::RecordChanged.into(),
        e => AppError::Infra(e.to_string()),
    }
}

/// 匯入的是銀行或海關匯率
#[derive(Clone, Copy)]
enum Side {
    Bank,
    Customs,
}
