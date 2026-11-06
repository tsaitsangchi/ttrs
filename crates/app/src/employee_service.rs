//! Employee Maintenance 員工維護 use case（TipTop: aooi040 / gen_file）
//! 規格：specs/features/aoo/employee_maintenance.feature（EQ1–EQ9）
//!
//! 工廠層：每個操作都指定營運中心，資料在該營運中心的工廠 schema（docs/adr/0007-multi-plant.md）。
//! TipTop 沒有設定資料所有者，不檢查資料權限（EQ5）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    department::DeptCode,
    employee::{Employee, EmployeeAttrs, EmployeeId},
    plant::PlantId,
};
use ttrs_shared::{ChangeDto, EmployeeDto, EmployeeInput, Page, QueryRequest};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    change_log::HistoryReader,
    error::RepoError,
    ports::{DepartmentRepository, EmployeeRepository, MasterDataRepository},
    query,
};

/// 權限檢查用的程式代號
const PROGRAM: &str = "employee-maintenance";

#[derive(Clone)]
pub struct EmployeeService {
    authz: Arc<dyn Authorizer>,
    employees: Arc<dyn EmployeeRepository>,
    depts: Arc<dyn DepartmentRepository>,
    master: Arc<dyn MasterDataRepository>,
    history: HistoryReader,
}

impl EmployeeService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        employees: Arc<dyn EmployeeRepository>,
        depts: Arc<dyn DepartmentRepository>,
        master: Arc<dyn MasterDataRepository>,
        history: HistoryReader,
    ) -> Self {
        Self {
            authz,
            employees,
            depts,
            master,
            history,
        }
    }

    /// QBE 查詢（EQ8）；依員工編號排序並分頁
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<EmployeeDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let filters = query::employee::filters(&req.conditions)?;
        let p = query::paging(&req);
        let (rows, total) = self
            .employees
            .query_employees(&plant, &filters, p.offset, u64::from(p.page_size))
            .await
            .map_err(infra)?;
        let mut items = Vec::with_capacity(rows.len());
        for e in &rows {
            items.push(self.to_dto(&plant, e).await?);
        }
        Ok(Page {
            items,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    pub async fn get(&self, actor: &Actor, plant: &str, id: &str) -> Result<EmployeeDto, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let e = self.load(&plant, id).await?;
        self.to_dto(&plant, &e).await
    }

    /// 新增（aooi040.4gl:246-325）：TipTop 在單身新增列，需 detail 與單身新增權限
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        input: EmployeeInput,
    ) -> Result<EmployeeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let plant = PlantId::parse(plant)?;
            if !self.master.plant_active(&plant).await.map_err(infra)? {
                return Err(DomainError::PlantNotFound.into());
            }
            let id = EmployeeId::parse(&input.employee_id)?;
            let attrs = self.attrs(&plant, &input).await?;
            let employee = Employee::new(id, attrs, &actor.user_id, &actor.dept, today());
            match self.employees.insert(&plant, &employee).await {
                Ok(()) => {
                    let saved = self.load(&plant, employee.id.as_str()).await?;
                    self.to_dto(&plant, &saved).await
                }
                Err(RepoError::Conflict) => Err(DomainError::EmployeeAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 修改：員工編號建立後不可修改（EQ2）
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        id: &str,
        input: EmployeeInput,
    ) -> Result<EmployeeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let mut employee = self.load(&plant, id).await?;
            employee.stamp.check(input.version)?;
            if !input.employee_id.trim().is_empty()
                && EmployeeId::parse(&input.employee_id)? != employee.id
            {
                return Err(DomainError::EmployeeIdImmutable.into());
            }
            let attrs = self.attrs(&plant, &input).await?;
            employee.update(attrs, &actor.user_id, today());
            self.save(&plant, &employee).await
        })
        .await
    }

    pub async fn deactivate(
        &self,
        actor: &Actor,
        plant: &str,
        id: &str,
        version: Option<i32>,
    ) -> Result<EmployeeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.set_active(actor, plant, id, false, version).await
        })
        .await
    }

    pub async fn activate(
        &self,
        actor: &Actor,
        plant: &str,
        id: &str,
        version: Option<i32>,
    ) -> Result<EmployeeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.set_active(actor, plant, id, true, version).await
        })
        .await
    }

    /// 刪除：不檢查引用（aooi040.4gl:380-405，EQ6）；需 detail 與單身刪除權限
    pub async fn delete(
        &self,
        actor: &Actor,
        plant: &str,
        id: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let plant = PlantId::parse(plant)?;
            let employee = self.load(&plant, id).await?;
            employee.stamp.check(version)?;
            let id = employee.id;
            if self.employees.delete(&plant, &id).await.map_err(infra)? {
                Ok(())
            } else {
                Err(DomainError::EmployeeNotFound.into())
            }
        })
        .await
    }

    async fn set_active(
        &self,
        actor: &Actor,
        plant: &str,
        id: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<EmployeeDto, AppError> {
        self.authorize(actor, "detail").await?;
        let plant = PlantId::parse(plant)?;
        let mut employee = self.load(&plant, id).await?;
        employee.stamp.check(version)?;
        crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
        employee.set_active(active, &actor.user_id, today());
        self.save(&plant, &employee).await
    }

    /// 檢核輸入：部門須為本營運中心存在（aoo-005）且有效（9028）的部門（aooi040.4gl:608-622）
    async fn attrs(&self, plant: &PlantId, i: &EmployeeInput) -> Result<EmployeeAttrs, AppError> {
        let attrs = EmployeeAttrs::new(
            i.name.clone(),
            i.dept_code.as_deref(),
            i.title.clone(),
            i.extension.clone(),
            i.email.clone(),
        )?;
        if let Some(code) = &attrs.dept_code {
            match self.depts.get(plant, code).await.map_err(infra)? {
                None => return Err(DomainError::DepartmentNotFound.into()),
                Some(d) if !d.active => return Err(DomainError::DepartmentInactive.into()),
                Some(_) => {}
            }
        }
        Ok(attrs)
    }

    /// 存檔：版本不符（他人已修改）→ RECORD_CHANGED；回傳重新讀取的資料
    async fn save(&self, plant: &PlantId, e: &Employee) -> Result<EmployeeDto, AppError> {
        match self.employees.update(plant, e).await {
            Ok(()) => {}
            Err(RepoError::Stale) => return Err(DomainError::RecordChanged.into()),
            Err(err) => return Err(infra(err)),
        }
        let saved = self.load(plant, e.id.as_str()).await?;
        self.to_dto(plant, &saved).await
    }

    /// 異動記錄（ADR 0012 §8，功能 history）
    pub async fn history(
        &self,
        actor: &Actor,
        plant: &str,
        id: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        self.authorize(actor, "history").await?;
        let plant = PlantId::parse(plant)?;
        let id = EmployeeId::parse(id).map_err(|_| DomainError::EmployeeNotFound)?;
        self.history
            .read(
                Some(&plant),
                "employee",
                &[("employee_id", id.as_str())],
                lang,
            )
            .await
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }

    async fn load(&self, plant: &PlantId, id: &str) -> Result<Employee, AppError> {
        let id = EmployeeId::parse(id).map_err(|_| DomainError::EmployeeNotFound)?;
        self.employees
            .get(plant, &id)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::EmployeeNotFound.into())
    }

    async fn dept_name(
        &self,
        plant: &PlantId,
        code: &DeptCode,
    ) -> Result<Option<String>, AppError> {
        Ok(self
            .depts
            .get(plant, code)
            .await
            .map_err(infra)?
            .map(|d| d.attrs.name))
    }

    async fn to_dto(&self, plant: &PlantId, e: &Employee) -> Result<EmployeeDto, AppError> {
        let a = &e.attrs;
        let dept_name = match &a.dept_code {
            Some(code) => self.dept_name(plant, code).await?,
            None => None,
        };
        Ok(EmployeeDto {
            employee_id: e.id.as_str().to_owned(),
            name: a.name.clone(),
            dept_code: a.dept_code.as_ref().map(|d| d.as_str().to_owned()),
            dept_name,
            title: a.title.clone(),
            extension: a.extension.clone(),
            email: a.email.clone(),
            active: e.active,
            created_by: e.created_by.clone(),
            created_dept: e.created_dept.clone(),
            created_on: e.created_on,
            updated_by: e.updated_by.clone(),
            updated_on: e.updated_on,
            version: e.stamp.version,
            created_at: e.stamp.created_at,
            updated_at: e.stamp.updated_at,
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
        use ttrs_domain::employee::max_len as d;
        use ttrs_shared::employee_max_len as s;
        assert_eq!(
            (d::ID, d::NAME, d::TITLE, d::EXTENSION, d::EMAIL),
            (s::ID, s::NAME, s::TITLE, s::EXTENSION, s::EMAIL)
        );
    }
}
