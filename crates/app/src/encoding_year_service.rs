//! Encoding Year Maintenance 編碼年對照維護 use case（TipTop: aooi404 / gff_file）
//! 規格：specs/features/aoo/encoding_year.feature（EY1–EY8）
//!
//! 共用 schema（EY2）。主鍵為年＋長度（TQC-950117）；長度由編碼編號的字數帶出。TipTop 為單身式作業
//! （query、detail），依 ADR 0013 補上複製、確認／取消確認、列印、匯出 Excel、異動記錄（EY8）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{DomainError, encoding_year::EncodingYear};
use ttrs_shared::{EncodingYearDto, EncodingYearInput, Page, QbeCondition, QueryRequest};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::EncodingYearRepository,
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "encoding-year-maintenance";

/// 清單報表（EY8）：年、長度、編碼編號、資料有效
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "encoding-year-report-title",
    columns: &[
        ("year", "encoding-year-col-year"),
        ("length", "encoding-year-col-length"),
        ("code_value", "encoding-year-col-code"),
        ("active", "encoding-year-col-active"),
    ],
};

#[derive(Clone)]
pub struct EncodingYearService {
    authz: Arc<dyn Authorizer>,
    years: Arc<dyn EncodingYearRepository>,
    reports: ListReporter,
}

impl EncodingYearService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        years: Arc<dyn EncodingYearRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            years,
            reports,
        }
    }

    /// 全部對照，依年、長度排序（aooi404.4gl:382-427 ORDER BY gff01）
    async fn sorted(&self) -> Result<Vec<EncodingYear>, AppError> {
        let mut all = self.years.list_encoding_years().await.map_err(infra)?;
        all.sort_by_key(|y| (y.year, y.length));
        Ok(all)
    }

    /// QBE 查詢（:338-381 CONSTRUCT gff01、gff02、gff03；ADR 0013 SA1）
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<EncodingYearDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::encoding_year::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.sorted().await?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::encoding_year::value, p);
        Ok(Page {
            items: rows.iter().map(to_dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印 output、匯出 Excel exporttoexcel（ADR 0013 SA3；aooi404 原本只有匯出 Excel）
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::encoding_year::filters(&conditions)?;
        let all = self.sorted().await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::encoding_year::value, p);
        self.reports.check(total)?;
        let t = |id: &str| report::text(lang, id);
        let lines = rows
            .iter()
            .map(|y| {
                vec![
                    y.year.to_string(),
                    y.length.to_string(),
                    y.code_value.clone(),
                    t(if y.active {
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

    pub async fn get(
        &self,
        actor: &Actor,
        year: i32,
        length: i16,
    ) -> Result<EncodingYearDto, AppError> {
        self.authorize(actor, "query").await?;
        Ok(to_dto(&self.load(year, length).await?))
    }

    /// 新增（:195-234）：長度由編碼編號帶出；同一年同一長度不可重複（主鍵）
    pub async fn create(
        &self,
        actor: &Actor,
        input: EncodingYearInput,
    ) -> Result<EncodingYearDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let y = EncodingYear::new(
                input.year,
                &input.code_value,
                &actor.user_id,
                &actor.dept,
                today(),
            )?;
            self.insert(&y).await
        })
        .await
    }

    /// 修改編碼編號：字數不可改變（EY4）
    pub async fn update(
        &self,
        actor: &Actor,
        year: i32,
        length: i16,
        input: EncodingYearInput,
    ) -> Result<EncodingYearDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut y = self.load(year, length).await?;
            y.stamp.check(input.version)?;
            y.update(&input.code_value, &actor.user_id, today())?;
            self.save(&y).await
        })
        .await
    }

    /// 停用／啟用（EY5；TTRS 新增的資料有效）
    pub async fn set_active(
        &self,
        actor: &Actor,
        year: i32,
        length: i16,
        active: bool,
        version: Option<i32>,
    ) -> Result<EncodingYearDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut y = self.load(year, length).await?;
            y.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            y.set_active(active, &actor.user_id, today());
            self.save(&y).await
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        year: i32,
        length: i16,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<EncodingYearDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let mut y = self.load(year, length).await?;
            y.stamp.check(version)?;
            y.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&y).await
        })
        .await
    }

    /// 複製到新的年（ADR 0013 SA5；EY8）
    pub async fn copy(
        &self,
        actor: &Actor,
        year: i32,
        length: i16,
        new_year: Option<i32>,
    ) -> Result<EncodingYearDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load(year, length).await?;
            let y = source.copy_to(new_year, &actor.user_id, &actor.dept, today())?;
            self.insert(&y).await
        })
        .await
    }

    /// 刪除（:235-260）：已確認不可刪除（SA9-2）
    pub async fn delete(
        &self,
        actor: &Actor,
        year: i32,
        length: i16,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let y = self.load(year, length).await?;
            y.stamp.check(version)?;
            y.confirmation.check_deletable()?;
            match self.years.delete_encoding_year(year, length).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::EncodingYearNotFound.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    async fn insert(&self, y: &EncodingYear) -> Result<EncodingYearDto, AppError> {
        match self.years.insert_encoding_year(y).await {
            Ok(()) => Ok(to_dto(&self.load(y.year, y.length).await?)),
            Err(RepoError::Conflict) => Err(DomainError::EncodingYearAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, y: &EncodingYear) -> Result<EncodingYearDto, AppError> {
        self.years.update_encoding_year(y).await.map_err(infra)?;
        Ok(to_dto(&self.load(y.year, y.length).await?))
    }

    async fn load(&self, year: i32, length: i16) -> Result<EncodingYear, AppError> {
        self.years
            .get_encoding_year(year, length)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::EncodingYearNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn to_dto(y: &EncodingYear) -> EncodingYearDto {
    EncodingYearDto {
        year: y.year,
        length: y.length,
        code_value: y.code_value.clone(),
        active: y.active,
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
