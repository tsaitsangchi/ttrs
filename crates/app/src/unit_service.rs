//! Unit Maintenance 單位維護 use case（TipTop: aooi101 / gfe_file）
//! 規格：specs/features/aoo/unit_maintenance.feature（UN1–UN7）
//!
//! 共用 schema（UN2）。TipTop 為多筆單身維護：查詢 query、維護 detail（新增列 add_row、刪除列 delete_row）、異動記錄 history。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    unit::{Unit, UnitAttrs, UnitCode},
};
use ttrs_shared::{Page, QbeCondition, QueryRequest, UnitDto, UnitInput};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::UnitRepository,
    query,
    report::{self, ReportData, ReportFormat},
};

/// 清單報表（ADR 0013 SA3；unit_maintenance_standard_actions.feature）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "unit-report-title",
    columns: &[
        ("unit_code", "unit-col-code"),
        ("name", "unit-col-name"),
        ("decimals", "unit-col-decimals"),
        ("active", "unit-col-active"),
    ],
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "unit-maintenance";

#[derive(Clone)]
pub struct UnitService {
    authz: Arc<dyn Authorizer>,
    units: Arc<dyn UnitRepository>,
    reports: ListReporter,
}

impl UnitService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        units: Arc<dyn UnitRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            units,
            reports,
        }
    }

    /// QBE 查詢（ADR 0013 SA1；aooi101 CONSTRUCT）：依單位代碼排序並分頁
    pub async fn query(&self, actor: &Actor, req: QueryRequest) -> Result<Page<UnitDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::unit::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.units.list_units().await.map_err(infra)?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::unit::value, p);
        Ok(Page {
            items: rows.iter().map(to_dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印 output、匯出 Excel exporttoexcel（ADR 0013 SA3；aooi101 原本沒有）
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::unit::filters(&conditions)?;
        let all = self.units.list_units().await.map_err(infra)?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::unit::value, p);
        self.reports.check(total)?;
        let t = |id: &str| report::text(lang, id);
        let lines = rows
            .iter()
            .map(|u| {
                vec![
                    u.code.as_str().to_owned(),
                    u.attrs.name.clone().unwrap_or_default(),
                    u.attrs.decimals.to_string(),
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

    /// 複製（ADR 0013 SA5；aooi101 原本沒有）
    pub async fn copy(
        &self,
        actor: &Actor,
        code: &str,
        new_code: &str,
    ) -> Result<UnitDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load(code).await?;
            let unit = source.copy_as(
                UnitCode::parse(new_code)?,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            match self.units.insert_unit(&unit).await {
                Ok(()) => Ok(to_dto(&self.load(unit.code.as_str()).await?)),
                Err(RepoError::Conflict) => Err(DomainError::UnitAlreadyExists.into()),
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
    ) -> Result<UnitDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let mut unit = self.load(code).await?;
            unit.stamp.check(version)?;
            unit.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&unit).await
        })
        .await
    }

    /// 全部單位，依代碼排序
    pub async fn list(&self, actor: &Actor) -> Result<Vec<UnitDto>, AppError> {
        self.authorize(actor, "query").await?;
        Ok(self
            .units
            .list_units()
            .await
            .map_err(infra)?
            .iter()
            .map(to_dto)
            .collect())
    }

    pub async fn get(&self, actor: &Actor, code: &str) -> Result<UnitDto, AppError> {
        self.authorize(actor, "query").await?;
        Ok(to_dto(&self.load(code).await?))
    }

    /// 新增（aooi101.4gl:195-260）：小數位數預設 0、資料有效 Y；代碼不可重複（-239）
    pub async fn create(&self, actor: &Actor, input: UnitInput) -> Result<UnitDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let code = UnitCode::parse(&input.unit_code)?;
            let attrs = UnitAttrs::new(input.name, input.decimals)?;
            let unit = Unit::new(code, attrs, &actor.user_id, &actor.dept, today());
            match self.units.insert_unit(&unit).await {
                Ok(()) => Ok(to_dto(&self.load(unit.code.as_str()).await?)),
                Err(RepoError::Conflict) => Err(DomainError::UnitAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 修改說明、小數位數；單位建立後不可修改（:694-711）
    pub async fn update(
        &self,
        actor: &Actor,
        code: &str,
        input: UnitInput,
    ) -> Result<UnitDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut unit = self.load(code).await?;
            unit.stamp.check(input.version)?;
            unit.update(
                UnitAttrs::new(input.name, input.decimals)?,
                &actor.user_id,
                today(),
            );
            self.save(&unit).await
        })
        .await
    }

    /// 停用／啟用（gfeacti，:268-275）
    pub async fn set_active(
        &self,
        actor: &Actor,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<UnitDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut unit = self.load(code).await?;
            unit.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            unit.set_active(active, &actor.user_id, today());
            self.save(&unit).await
        })
        .await
    }

    /// 刪除：已被料件等資料引用時拒絕，請改為停用（UN5；TipTop 不檢查）
    pub async fn delete(
        &self,
        actor: &Actor,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let unit = self.load(code).await?;
            unit.stamp.check(version)?;
            // 已確認不可刪除，須先取消確認（ADR 0013 SA9-2）
            unit.confirmation.check_deletable()?;
            match self.units.delete_unit(&unit.code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::UnitNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::UnitInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    async fn save(&self, unit: &Unit) -> Result<UnitDto, AppError> {
        self.units.update_unit(unit).await.map_err(infra)?;
        Ok(to_dto(&self.load(unit.code.as_str()).await?))
    }

    async fn load(&self, code: &str) -> Result<Unit, AppError> {
        let code = UnitCode::parse(code).map_err(|_| DomainError::UnitNotFound)?;
        self.units
            .get_unit(&code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::UnitNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn to_dto(u: &Unit) -> UnitDto {
    UnitDto {
        unit_code: u.code.as_str().to_owned(),
        name: u.attrs.name.clone(),
        decimals: u.attrs.decimals,
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
