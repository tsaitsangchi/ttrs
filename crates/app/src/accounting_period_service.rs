//! Accounting Periods 會計期間維護 use case（TipTop: aoos020 / azm_file；aoos030、aooq010 由即時計算取代）
//! 規格：specs/features/aoo/accounting_periods.feature（PD1–PD12）
//!
//! 共用 schema（PD2）。功能（ADR 0013、PD9）：新增、修改、刪除、查詢、複製（另一個年度）、確認／取消確認、列印、匯出 Excel、異動記錄；
//! 失效為例外。日期所屬期間與年度＋期別的起訖日由 domain [`Calendar`] 計算（s_yp、s_gactpd、s_azm，PD3）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    accounting_period::{self, AccountingDay, AccountingYear, Calendar, Period},
};
use ttrs_shared::{
    AccountingDayDto, AccountingPeriodLine, AccountingYearDto, AccountingYearInput, Page,
    QbeCondition, QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{AccountingPeriodRepository, SystemParameterRepository},
    query,
    report::{ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "accounting-period-maintenance";

/// 列印、匯出 Excel：一列一期（PD10）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "period-report-title",
    columns: &[
        ("year", "period-col-year"),
        ("period_count", "period-col-count"),
        ("period", "period-col-period"),
        ("start_date", "period-col-start"),
        ("end_date", "period-col-end"),
        ("quarter", "period-col-quarter"),
    ],
};

#[derive(Clone)]
pub struct AccountingPeriodService {
    authz: Arc<dyn Authorizer>,
    years: Arc<dyn AccountingPeriodRepository>,
    params: Arc<dyn SystemParameterRepository>,
    reports: ListReporter,
}

impl AccountingPeriodService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        years: Arc<dyn AccountingPeriodRepository>,
        params: Arc<dyn SystemParameterRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            years,
            params,
            reports,
        }
    }

    /// QBE 查詢（aoos020.4gl:102-161），依年度排序
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<AccountingYearDto>, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        let filters = query::accounting_year::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.all().await?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::accounting_year::value, p);
        Ok(Page {
            items: rows.iter().map(to_dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    pub async fn get(&self, actor: &Actor, year: i32) -> Result<AccountingYearDto, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        Ok(to_dto(&self.load(year).await?))
    }

    /// 新增時的預設各期（aoos020.4gl:345-355、:684-818；PD5）：期數取整體管理系統參數（PD6）
    pub async fn defaults(
        &self,
        actor: &Actor,
        _year: i32,
        first_start: NaiveDate,
    ) -> Result<Vec<AccountingPeriodLine>, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        let count = self.period_count().await?;
        Ok(accounting_period::default_periods(first_start, count)
            .iter()
            .map(line)
            .collect())
    }

    /// 新增（aoos020.4gl:236-290、aoos021.4gl:277-343）：年度不可重複、不可與其他年度重疊（PD7）
    pub async fn create(
        &self,
        actor: &Actor,
        input: AccountingYearInput,
    ) -> Result<AccountingYearDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authz.authorize(actor, PROGRAM, "insert").await?;
            let count = self.period_count().await?;
            let y = AccountingYear::new(
                input.year,
                count,
                periods_of(&input.periods),
                &actor.user_id,
                &actor.dept,
                today(),
            )?;
            self.insert(y).await
        })
        .await
    }

    /// 修改各期（PD8；TipTop 沒有）：已確認不可修改、比對版本
    pub async fn update(
        &self,
        actor: &Actor,
        year: i32,
        input: AccountingYearInput,
    ) -> Result<AccountingYearDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            self.authz.authorize(actor, PROGRAM, "modify").await?;
            let mut y = self.load(year).await?;
            y.stamp.check(input.version)?;
            y.update(periods_of(&input.periods), &actor.user_id, today())?;
            self.check_overlap(&y).await?;
            self.years.update_accounting_year(&y).await.map_err(infra)?;
            Ok(to_dto(&self.load(year).await?))
        })
        .await
    }

    /// 刪除（aoos021.4gl:1271-1310）：已確認不可刪除（SA9）
    pub async fn delete(
        &self,
        actor: &Actor,
        year: i32,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            self.authz.authorize(actor, PROGRAM, "delete").await?;
            let y = self.load(year).await?;
            y.stamp.check(version)?;
            y.confirmation.check_deletable()?;
            if !self
                .years
                .delete_accounting_year(year)
                .await
                .map_err(infra)?
            {
                return Err(DomainError::AccountingYearNotFound.into());
            }
            Ok(())
        })
        .await
    }

    /// 複製為另一個年度（PD9）：各期日期依年度差平移，未確認
    pub async fn copy(
        &self,
        actor: &Actor,
        from: i32,
        to: i32,
    ) -> Result<AccountingYearDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authz.authorize(actor, PROGRAM, "reproduce").await?;
            let src = self.load(from).await?;
            let y = src.copy_as(to, &actor.user_id, &actor.dept, today())?;
            self.insert(y).await
        })
        .await
    }

    /// 確認／取消確認（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        year: i32,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<AccountingYearDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authz.authorize(actor, PROGRAM, action).await?;
            let mut y = self.load(year).await?;
            y.stamp.check(version)?;
            crate::change_log::set_operation(action);
            y.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.years.update_accounting_year(&y).await.map_err(infra)?;
            Ok(to_dto(&self.load(year).await?))
        })
        .await
    }

    /// 日期所屬的年度、季、期、週（s_yp、s_gactpd；PD3、PD4）
    pub async fn locate(
        &self,
        actor: &Actor,
        date: NaiveDate,
    ) -> Result<AccountingDayDto, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        let all = self.all().await?;
        Ok(day_dto(Calendar::new(&all).locate(date)?))
    }

    /// 年度＋期別的起訖日（s_azm、s_azn01；PD11）
    pub async fn period_dates(
        &self,
        actor: &Actor,
        year: i32,
        period: i16,
    ) -> Result<AccountingPeriodLine, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        let all = self.all().await?;
        Ok(line(&Calendar::new(&all).period(year, period)?))
    }

    /// 週期期間查詢（aooq010，PD3）：日期區間內每一天的年度、季、期、週
    pub async fn days(
        &self,
        actor: &Actor,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<AccountingDayDto>, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        let all = self.all().await?;
        Ok(Calendar::new(&all)
            .days(from, to)?
            .into_iter()
            .map(day_dto)
            .collect())
    }

    /// 列印、匯出 Excel（PD10）：一列一期
    pub async fn report(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authz
            .authorize(actor, PROGRAM, format.action())
            .await?;
        let filters = query::accounting_year::filters(&conditions)?;
        let all = self.all().await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (years, _) = query::page_in_memory(&all, &filters, query::accounting_year::value, p);
        let rows: Vec<Vec<String>> = years
            .iter()
            .flat_map(|y| {
                y.periods.iter().map(|p| {
                    vec![
                        y.year.to_string(),
                        y.period_count.to_string(),
                        p.no.to_string(),
                        p.start.to_string(),
                        p.end.to_string(),
                        p.quarter.to_string(),
                    ]
                })
            })
            .collect();
        self.reports
            .check(u64::try_from(rows.len()).unwrap_or(u64::MAX))?;
        self.reports
            .build(actor, None, &REPORT, rows, &conditions, format, lang)
            .await
    }

    async fn insert(&self, y: AccountingYear) -> Result<AccountingYearDto, AppError> {
        if self
            .years
            .get_accounting_year(y.year)
            .await
            .map_err(infra)?
            .is_some()
        {
            return Err(DomainError::AccountingYearAlreadyExists.into());
        }
        self.check_overlap(&y).await?;
        self.years
            .insert_accounting_year(&y)
            .await
            .map_err(|e| match e {
                RepoError::Conflict => DomainError::AccountingYearAlreadyExists.into(),
                e => infra(e),
            })?;
        Ok(to_dto(&self.load(y.year).await?))
    }

    /// 不可與其他年度的期間重疊（PD7）
    async fn check_overlap(&self, y: &AccountingYear) -> Result<(), AppError> {
        let all = self.all().await?;
        if all
            .iter()
            .any(|o| o.year != y.year && accounting_period::overlaps(o, y))
        {
            return Err(DomainError::AccountingYearOverlap.into());
        }
        Ok(())
    }

    async fn period_count(&self) -> Result<i16, AppError> {
        Ok(self
            .params
            .general_parameters()
            .await
            .map_err(infra)?
            .map_or(12, |p| p.accounting_periods.get()))
    }

    async fn all(&self) -> Result<Vec<AccountingYear>, AppError> {
        self.years.list_accounting_years().await.map_err(infra)
    }

    async fn load(&self, year: i32) -> Result<AccountingYear, AppError> {
        self.years
            .get_accounting_year(year)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::AccountingYearNotFound.into())
    }
}

fn periods_of(lines: &[AccountingPeriodLine]) -> Vec<Period> {
    lines
        .iter()
        .map(|l| Period {
            no: l.period,
            start: l.start_date,
            end: l.end_date,
            quarter: l.quarter,
        })
        .collect()
}

fn line(p: &Period) -> AccountingPeriodLine {
    AccountingPeriodLine {
        period: p.no,
        start_date: p.start,
        end_date: p.end,
        quarter: p.quarter,
    }
}

fn day_dto(d: AccountingDay) -> AccountingDayDto {
    AccountingDayDto {
        date: d.date,
        year: d.year,
        quarter: d.quarter,
        period: d.period,
        week: d.week,
    }
}

fn to_dto(y: &AccountingYear) -> AccountingYearDto {
    AccountingYearDto {
        year: y.year,
        period_count: y.period_count,
        periods: y.periods.iter().map(line).collect(),
        confirmed: y.confirmation.confirmed,
        confirmed_by: y.confirmation.by.clone(),
        confirmed_at: y.confirmation.at,
        created_by: y.created_by.clone(),
        created_dept: y.created_dept.clone(),
        created_on: y.created_on,
        updated_by: y.updated_by.clone(),
        updated_on: y.updated_on,
        version: y.stamp.version,
        created_at: y.stamp.created_at,
        updated_at: y.stamp.updated_at,
    }
}

fn infra(e: RepoError) -> AppError {
    match e {
        RepoError::Stale => DomainError::RecordChanged.into(),
        e => AppError::Infra(e.to_string()),
    }
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}
