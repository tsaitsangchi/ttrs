//! Department Maintenance 部門維護 use case（TipTop: aooi030 / gem_file）
//! 規格：specs/features/aoo/department_maintenance.feature（DQ1–DQ10）
//!
//! 工廠層：每個操作都指定營運中心，資料在該營運中心的工廠 schema（docs/adr/0007-multi-plant.md）。
//! TipTop 沒有設定資料所有者，不檢查資料權限（DQ4）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    department::{Accounting, Department, DepartmentAttrs, DeptCode},
    plant::PlantId,
};
use ttrs_shared::{DepartmentAccountingInput, DepartmentDto, DepartmentInput, Page, QueryRequest};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    ports::{
        AuditEntry, AuditLog, DepartmentRepository, MasterDataRepository, PlantRepository,
        ProgramRepository,
    },
    query,
    report::{self, ReportData, ReportFormat},
};
use ttrs_domain::{lang::Lang, program::ProgramNo, report::MAX_SYNC_ROWS};
use ttrs_shared::QbeCondition;

/// 權限檢查用的程式代號
const PROGRAM: &str = "department-maintenance";

#[derive(Clone)]
pub struct DepartmentService {
    authz: Arc<dyn Authorizer>,
    depts: Arc<dyn DepartmentRepository>,
    master: Arc<dyn MasterDataRepository>,
    /// 報表：營運中心名稱（表頭）、程式資料 zz05（列印條件）、稽核紀錄（department_report.feature）
    plants: Arc<dyn PlantRepository>,
    programs: Arc<dyn ProgramRepository>,
    audit: Arc<dyn AuditLog>,
    /// 同步產生報表的筆數上限（ADR 0011 RP6、DR10）
    report_limit: u64,
}

impl DepartmentService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        depts: Arc<dyn DepartmentRepository>,
        master: Arc<dyn MasterDataRepository>,
        plants: Arc<dyn PlantRepository>,
        programs: Arc<dyn ProgramRepository>,
        audit: Arc<dyn AuditLog>,
    ) -> Self {
        Self {
            authz,
            depts,
            master,
            plants,
            programs,
            audit,
            report_limit: MAX_SYNC_ROWS,
        }
    }

    /// 測試用：調整同步產生報表的筆數上限
    pub fn with_report_limit(self, limit: u64) -> Self {
        Self {
            report_limit: limit,
            ..self
        }
    }

    /// 列印、匯出 Excel（aooi030 output／exporttoexcel，i030_out :1081-1153）：依目前的查詢條件（沒有條件時全部，DR2）、
    /// 依部門編號排序；超過上限 → REPORT_TOO_LARGE（DR10）；寫入稽核紀錄（DR11）。報表文字用繁中
    pub async fn report(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
    ) -> Result<ReportData, AppError> {
        self.report_in(actor, plant, conditions, format, &Lang::default_lang())
            .await
    }

    /// 同上，報表文字依指定語系（ADR 0011 RP12）
    pub async fn report_in(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let plant = PlantId::parse(plant)?;
        let filters = query::department::filters(&conditions)?;
        let (rows, total) = self
            .depts
            .query_departments(&plant, &filters, 0, self.report_limit + 1)
            .await
            .map_err(infra)?;
        ttrs_domain::report::check_size(total, self.report_limit)?;
        let t = |id: &str| report::text(lang, id);
        let yes_no = |b: bool| t(if b { "report-yes" } else { "report-no" });
        let mut lines = Vec::with_capacity(rows.len());
        for d in &rows {
            let dto = self.to_dto(&plant, d).await?;
            lines.push(vec![
                dto.dept_code.clone(),
                dto.name.clone(),
                dto.full_name.clone().unwrap_or_default(),
                yes_no(dto.accounting_dept),
                dto.expense_type
                    .as_deref()
                    .map(|e| t(&format!("dept-expense-{}", e.to_lowercase())))
                    .unwrap_or_default(),
                t(&format!("dept-control-{}", dto.control_type)),
                dto.cost_center
                    .as_ref()
                    .map(|c| format!("{c} {}", dto.cost_center_name.clone().unwrap_or_default()))
                    .unwrap_or_default(),
                t(if dto.active {
                    "report-active"
                } else {
                    "report-inactive"
                }),
            ]);
        }
        // zz05 列印選擇條件（aooi030.4gl:1146-1149，DR7）
        let print_conditions = match ProgramNo::parse(PROGRAM) {
            Ok(no) => self
                .programs
                .get(&no)
                .await
                .map_err(infra)?
                .is_some_and(|p| p.other.print_conditions),
            Err(_) => false,
        };
        let condition = if print_conditions {
            report::describe_conditions(&conditions, |f| {
                let id = match f {
                    "dept_code" => "dept-col-code",
                    "name" => "dept-col-name",
                    "full_name" => "dept-col-full-name",
                    "accounting_dept" => "dept-col-accounting",
                    "expense_type" => "dept-col-expense",
                    "control_type" => "dept-col-control",
                    "cost_center" => "dept-col-cost-center",
                    "active" => "dept-col-active",
                    _ => return None,
                };
                Some(t(id))
            })
        } else {
            None
        };
        let plant_name = self
            .plants
            .get(&plant)
            .await
            .map_err(infra)?
            .map(|p| p.attrs.name)
            .unwrap_or_default();
        let data = ReportData {
            program: PROGRAM.to_owned(),
            title: t("dept-report-title"),
            plant: plant.as_str().to_owned(),
            plant_name,
            printed_by: actor.user_id.clone(),
            printed_at: chrono::Local::now().format("%Y/%m/%d %H:%M").to_string(),
            columns: [
                "dept-col-code",
                "dept-col-name",
                "dept-col-full-name",
                "dept-col-accounting",
                "dept-col-expense",
                "dept-col-control",
                "dept-col-cost-center",
                "dept-col-active",
            ]
            .iter()
            .map(|id| t(id))
            .collect(),
            rows: lines,
            condition,
            labels: report::labels(lang),
            lang: lang.clone(),
        };
        let all =
            report::describe_conditions(&conditions, |f| Some(f.to_owned())).unwrap_or_default();
        self.audit
            .record(&AuditEntry {
                program: PROGRAM.to_owned(),
                user_id: actor.user_id.clone(),
                key: plant.as_str().to_owned(),
                action: format.action().to_owned(),
                detail: Some(format!(
                    "條件={all}；格式={}；筆數={}",
                    format.code(),
                    data.rows.len()
                )),
            })
            .await
            .map_err(infra)?;
        Ok(data)
    }

    /// QBE 查詢（DQ8）；依部門編號排序並分頁
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<DepartmentDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let filters = query::department::filters(&req.conditions)?;
        let p = query::paging(&req);
        let (rows, total) = self
            .depts
            .query_departments(&plant, &filters, p.offset, u64::from(p.page_size))
            .await
            .map_err(infra)?;
        let mut items = Vec::with_capacity(rows.len());
        for d in &rows {
            items.push(self.to_dto(&plant, d).await?);
        }
        Ok(Page {
            items,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    pub async fn get(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
    ) -> Result<DepartmentDto, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let d = self.load(&plant, code).await?;
        self.to_dto(&plant, &d).await
    }

    /// 新增（aooi030.4gl:294-357）：TipTop 在單身新增列，需 detail 與單身新增權限
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        input: DepartmentInput,
    ) -> Result<DepartmentDto, AppError> {
        self.authorize(actor, "detail").await?.add_row()?;
        let plant = PlantId::parse(plant)?;
        if !self.master.plant_active(&plant).await.map_err(infra)? {
            return Err(DomainError::PlantNotFound.into());
        }
        let code = DeptCode::parse(&input.dept_code)?;
        let attrs = DepartmentAttrs::new(input.name, input.full_name)?;
        let dept = Department::new(code, attrs, &actor.user_id, &actor.dept, today());
        match self.depts.insert(&plant, &dept).await {
            Ok(()) => self.to_dto(&plant, &dept).await,
            Err(RepoError::Conflict) => Err(DomainError::DepartmentAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    /// 修改基本欄位：部門編號建立後不可修改（DQ2）
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        input: DepartmentInput,
    ) -> Result<DepartmentDto, AppError> {
        self.authorize(actor, "detail").await?;
        let plant = PlantId::parse(plant)?;
        let mut dept = self.load(&plant, code).await?;
        if !input.dept_code.trim().is_empty() && DeptCode::parse(&input.dept_code)? != dept.code {
            return Err(DomainError::DepartmentCodeImmutable.into());
        }
        dept.update(
            DepartmentAttrs::new(input.name, input.full_name)?,
            &actor.user_id,
            today(),
        );
        self.save(&plant, &dept).await
    }

    /// 會計資料維護（aooi030 account，:1260-1277）：對應成本中心須為有效的部門（:402-420）
    pub async fn set_accounting(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        input: DepartmentAccountingInput,
    ) -> Result<DepartmentDto, AppError> {
        self.authorize(actor, "account").await?;
        let plant = PlantId::parse(plant)?;
        let mut dept = self.load(&plant, code).await?;
        let accounting = Accounting::new(
            &dept.code,
            input.accounting_dept,
            input.expense_type.as_deref(),
            &input.control_type,
            input.cost_center.as_deref(),
        )?;
        if let Some(cc) = &accounting.cost_center
            && cc != &dept.code
        {
            let ok = self
                .depts
                .get(&plant, cc)
                .await
                .map_err(infra)?
                .is_some_and(|d| d.active);
            if !ok {
                return Err(
                    DomainError::DepartmentCostCenterInvalid(cc.as_str().to_owned()).into(),
                );
            }
        }
        dept.set_accounting(accounting, &actor.user_id, today());
        self.save(&plant, &dept).await
    }

    /// 停用（資料有效 → 無效）：一律允許（DQ5）
    pub async fn deactivate(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
    ) -> Result<DepartmentDto, AppError> {
        self.set_active(actor, plant, code, false).await
    }

    pub async fn activate(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
    ) -> Result<DepartmentDto, AppError> {
        self.set_active(actor, plant, code, true).await
    }

    /// 刪除：還有員工時拒絕（aoo-105，DQ5）；TipTop 在單身刪除列，需 detail 與單身刪除權限
    pub async fn delete(&self, actor: &Actor, plant: &str, code: &str) -> Result<(), AppError> {
        self.authorize(actor, "detail").await?.delete_row()?;
        let plant = PlantId::parse(plant)?;
        let code = DeptCode::parse(code).map_err(|_| DomainError::DepartmentNotFound)?;
        match self.depts.delete(&plant, &code).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(DomainError::DepartmentNotFound.into()),
            Err(RepoError::InUse) => Err(DomainError::DepartmentInUse.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn set_active(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        active: bool,
    ) -> Result<DepartmentDto, AppError> {
        self.authorize(actor, "detail").await?;
        let plant = PlantId::parse(plant)?;
        let mut dept = self.load(&plant, code).await?;
        dept.set_active(active, &actor.user_id, today());
        self.save(&plant, &dept).await
    }

    async fn save(&self, plant: &PlantId, dept: &Department) -> Result<DepartmentDto, AppError> {
        self.depts.update(plant, dept).await.map_err(infra)?;
        self.to_dto(plant, dept).await
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }

    async fn load(&self, plant: &PlantId, code: &str) -> Result<Department, AppError> {
        let code = DeptCode::parse(code).map_err(|_| DomainError::DepartmentNotFound)?;
        self.depts
            .get(plant, &code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::DepartmentNotFound.into())
    }

    /// DTO；對應成本中心帶出部門簡稱（i030_get_gem02a，:1243-1258）
    async fn to_dto(&self, plant: &PlantId, d: &Department) -> Result<DepartmentDto, AppError> {
        let a = &d.accounting;
        let cost_center_name = match &a.cost_center {
            None => None,
            Some(cc) if cc == &d.code => Some(d.attrs.name.clone()),
            Some(cc) => self
                .depts
                .get(plant, cc)
                .await
                .map_err(infra)?
                .map(|c| c.attrs.name),
        };
        Ok(DepartmentDto {
            dept_code: d.code.as_str().to_owned(),
            name: d.attrs.name.clone(),
            full_name: d.attrs.full_name.clone(),
            accounting_dept: a.accounting_dept,
            expense_type: a.expense_type.map(|e| e.code().to_owned()),
            control_type: a.control_type.code().to_owned(),
            cost_center: a.cost_center.as_ref().map(|c| c.as_str().to_owned()),
            cost_center_name,
            active: d.active,
            created_by: d.created_by.clone(),
            created_dept: d.created_dept.clone(),
            created_on: d.created_on,
            updated_by: d.updated_by.clone(),
            updated_on: d.updated_on,
        })
    }
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_max_len_matches_domain() {
        use ttrs_domain::department::max_len as d;
        use ttrs_shared::department_max_len as s;
        assert_eq!(
            (d::CODE, d::NAME, d::FULL_NAME),
            (s::CODE, s::NAME, s::FULL_NAME)
        );
    }
}
