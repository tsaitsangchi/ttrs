//! Currency Maintenance 幣別維護 use case（TipTop: aooi050 / azi_file）
//! 規格：specs/features/aoo/currency_maintenance.feature（CY1–CY8）
//!
//! 共用 schema（CY2）。TipTop 為多筆單身維護：查詢 query、維護 detail（新增列 add_row、刪除列 delete_row）、
//! 列印 output、匯出 Excel exporttoexcel；依 ADR 0013 補上複製、確認、異動記錄。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    currency::{Currency, CurrencyAttrs, CurrencyCode, RateMethod},
};
use ttrs_shared::{CurrencyDto, CurrencyInput, Page, QbeCondition, QueryRequest};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::CurrencyRepository,
    query,
    report::{self, ReportData, ReportFormat},
};

/// 清單報表（ADR 0013 SA3；CY 標準功能鈕）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "currency-report-title",
    columns: &[
        ("currency_code", "currency-col-code"),
        ("name", "currency-col-name"),
        ("price_decimals", "currency-col-price-decimals"),
        ("amount_decimals", "currency-col-amount-decimals"),
        ("total_decimals", "currency-col-total-decimals"),
        ("rate_decimals", "currency-col-rate-decimals"),
        ("rate_method", "currency-col-rate-method"),
        ("active", "currency-col-active"),
    ],
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "currency-maintenance";

#[derive(Clone)]
pub struct CurrencyService {
    authz: Arc<dyn Authorizer>,
    currencies: Arc<dyn CurrencyRepository>,
    reports: ListReporter,
}

impl CurrencyService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        currencies: Arc<dyn CurrencyRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            currencies,
            reports,
        }
    }

    /// QBE 查詢（ADR 0013 SA1；aooi050.4gl:395 CONSTRUCT）：依幣別代碼排序並分頁
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<CurrencyDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::currency::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.currencies.list_currencies().await.map_err(infra)?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::currency::value, p);
        Ok(Page {
            items: rows.iter().map(to_dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印 output、匯出 Excel exporttoexcel（ADR 0013 SA3；aooi050 原本沒有）
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::currency::filters(&conditions)?;
        let all = self.currencies.list_currencies().await.map_err(infra)?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::currency::value, p);
        self.reports.check(total)?;
        let t = |id: &str| report::text(lang, id);
        let lines = rows
            .iter()
            .map(|u| {
                let a = &u.attrs;
                vec![
                    u.code.as_str().to_owned(),
                    a.name.clone().unwrap_or_default(),
                    a.price_decimals.to_string(),
                    a.amount_decimals.to_string(),
                    a.total_decimals.to_string(),
                    a.rate_decimals.to_string(),
                    t(match a.rate_method {
                        RateMethod::Multiply => "currency-rate-multiply",
                        RateMethod::Divide => "currency-rate-divide",
                    }),
                    t(if u.active {
                        "report-active"
                    } else {
                        "report-inactive"
                    }),
                ]
            })
            .collect();
        self.reports
            .build(actor, None, &REPORT, lines, &conditions, format, lang)
            .await
    }

    /// 測試用：調整同步產生報表的筆數上限
    pub fn with_report_limit(mut self, limit: u64) -> Self {
        self.reports.limit = limit;
        self
    }

    /// 複製（ADR 0013 SA5；aooi050 原本沒有）
    pub async fn copy(
        &self,
        actor: &Actor,
        code: &str,
        new_code: &str,
    ) -> Result<CurrencyDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load(code).await?;
            let cur = source.copy_as(
                CurrencyCode::parse(new_code)?,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            match self.currencies.insert_currency(&cur).await {
                Ok(()) => Ok(to_dto(&self.load(cur.code.as_str()).await?)),
                Err(RepoError::Conflict) => Err(DomainError::CurrencyAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        code: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<CurrencyDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let mut cur = self.load(code).await?;
            cur.stamp.check(version)?;
            cur.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&cur).await
        })
        .await
    }

    /// 全部幣別，依代碼排序
    pub async fn list(&self, actor: &Actor) -> Result<Vec<CurrencyDto>, AppError> {
        self.authorize(actor, "query").await?;
        Ok(self
            .currencies
            .list_currencies()
            .await
            .map_err(infra)?
            .iter()
            .map(to_dto)
            .collect())
    }

    pub async fn get(&self, actor: &Actor, code: &str) -> Result<CurrencyDto, AppError> {
        self.authorize(actor, "query").await?;
        Ok(to_dto(&self.load(code).await?))
    }

    /// 新增（aooi050.4gl:222-249）：資料有效 Y；小數位數空白用預設值（CY3）；代碼不可重複（-239）
    pub async fn create(
        &self,
        actor: &Actor,
        input: CurrencyInput,
    ) -> Result<CurrencyDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let code = CurrencyCode::parse(&input.currency_code)?;
            let attrs = attrs_of(input)?;
            let cur = Currency::new(code, attrs, &actor.user_id, &actor.dept, today());
            match self.currencies.insert_currency(&cur).await {
                Ok(()) => Ok(to_dto(&self.load(cur.code.as_str()).await?)),
                Err(RepoError::Conflict) => Err(DomainError::CurrencyAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 修改說明、小數位數、計算方式；幣別建立後不可修改（CY5）
    pub async fn update(
        &self,
        actor: &Actor,
        code: &str,
        input: CurrencyInput,
    ) -> Result<CurrencyDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut cur = self.load(code).await?;
            cur.stamp.check(input.version)?;
            let attrs = attrs_of(input)?;
            cur.update(attrs, &actor.user_id, today());
            self.save(&cur).await
        })
        .await
    }

    /// 停用／啟用（aziacti，:271-278）
    pub async fn set_active(
        &self,
        actor: &Actor,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<CurrencyDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut cur = self.load(code).await?;
            cur.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            cur.set_active(active, &actor.user_id, today());
            self.save(&cur).await
        })
        .await
    }

    /// 刪除（:279）：已被匯率等資料引用時拒絕，請改為停用（CY6；引用的作業移植後才有）
    pub async fn delete(
        &self,
        actor: &Actor,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let cur = self.load(code).await?;
            cur.stamp.check(version)?;
            // 已確認不可刪除，須先取消確認（ADR 0013 SA9-2）
            cur.confirmation.check_deletable()?;
            match self.currencies.delete_currency(&cur.code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::CurrencyNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::CurrencyInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    async fn save(&self, cur: &Currency) -> Result<CurrencyDto, AppError> {
        self.currencies.update_currency(cur).await.map_err(infra)?;
        Ok(to_dto(&self.load(cur.code.as_str()).await?))
    }

    async fn load(&self, code: &str) -> Result<Currency, AppError> {
        let code = CurrencyCode::parse(code).map_err(|_| DomainError::CurrencyNotFound)?;
        self.currencies
            .get_currency(&code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::CurrencyNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn attrs_of(input: CurrencyInput) -> Result<CurrencyAttrs, DomainError> {
    CurrencyAttrs::new(
        input.name,
        input.price_decimals,
        input.amount_decimals,
        input.total_decimals,
        input.rate_decimals,
        input.rate_method.as_deref(),
    )
}

fn to_dto(u: &Currency) -> CurrencyDto {
    let a = &u.attrs;
    CurrencyDto {
        currency_code: u.code.as_str().to_owned(),
        name: a.name.clone(),
        price_decimals: a.price_decimals,
        amount_decimals: a.amount_decimals,
        total_decimals: a.total_decimals,
        rate_decimals: a.rate_decimals,
        rate_method: a.rate_method.code().to_owned(),
        active: u.active,
        confirmed: u.confirmation.confirmed,
        confirmed_by: u.confirmation.by.clone(),
        confirmed_at: u.confirmation.at,
        created_by: u.created_by.clone(),
        created_dept: u.created_dept.clone(),
        created_on: u.created_on,
        updated_by: u.updated_by.clone(),
        updated_on: u.updated_on,
        version: u.stamp.version,
        created_at: u.stamp.created_at,
        updated_at: u.stamp.updated_at,
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
