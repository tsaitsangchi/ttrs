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
    ports::{DepartmentRepository, MasterDataRepository},
    query,
};

/// 權限檢查用的程式代號
const PROGRAM: &str = "department-maintenance";

#[derive(Clone)]
pub struct DepartmentService {
    authz: Arc<dyn Authorizer>,
    depts: Arc<dyn DepartmentRepository>,
    master: Arc<dyn MasterDataRepository>,
}

impl DepartmentService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        depts: Arc<dyn DepartmentRepository>,
        master: Arc<dyn MasterDataRepository>,
    ) -> Self {
        Self {
            authz,
            depts,
            master,
        }
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
