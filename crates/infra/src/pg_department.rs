//! 部門維護（aooi030）、員工維護（aooi040）的 PostgreSQL 實作。
//! 表在營運中心的工廠 schema（migrations/plant/0002_department_employee.sql），每個方法在工廠交易內執行。

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{
    error::RepoError,
    ports::{DepartmentRepository, EmployeeRepository},
};
use ttrs_domain::record::Stamp;
use ttrs_domain::{
    department::{Accounting, ControlType, Department, DepartmentAttrs, DeptCode, ExpenseType},
    employee::{Employee, EmployeeAttrs, EmployeeId},
    plant::PlantId,
    qbe::QbeFilter,
};

use crate::{
    PgStore,
    pg::db_err,
    qbe_sql::{Column, push_filters, query_page_in},
};

fn bad(what: &str) -> RepoError {
    RepoError::Other(format!("資料庫內容不合法：{what}"))
}

// ----- 部門 -----

/// 查詢鍵值（DepartmentDto 欄位名稱，ttrs_app::query::department）→ 資料庫欄位
fn dept_column(field: &str) -> Option<Column> {
    Some(match field {
        "dept_code" => Column::Scalar("d.dept_code", true),
        "name" => Column::Scalar("d.name", true),
        "full_name" => Column::Scalar("d.full_name", true),
        "accounting_dept" => Column::Scalar("d.accounting_dept", false),
        "expense_type" => Column::Scalar("d.expense_type", true),
        "control_type" => Column::Scalar("d.control_type", true),
        "cost_center" => Column::Scalar("d.cost_center", true),
        "active" => Column::Scalar("d.active", false),
        _ => return None,
    })
}

#[derive(FromRow)]
struct DeptRow {
    dept_code: String,
    name: String,
    full_name: Option<String>,
    accounting_dept: bool,
    expense_type: Option<String>,
    control_type: String,
    cost_center: Option<String>,
    active: bool,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

macro_rules! select_dept {
    ($rest:literal) => {
        concat!(
            "SELECT dept_code, name, full_name, accounting_dept, expense_type, control_type, cost_center, ",
            "active, created_by, created_dept, created_on, updated_by, updated_on, version, created_at, updated_at ",
            "FROM department ",
            $rest
        )
    };
}

impl TryFrom<DeptRow> for Department {
    type Error = RepoError;

    fn try_from(r: DeptRow) -> Result<Self, RepoError> {
        Ok(Department {
            code: DeptCode::parse(&r.dept_code).map_err(|_| bad(&r.dept_code))?,
            attrs: DepartmentAttrs {
                name: r.name,
                full_name: r.full_name,
            },
            accounting: Accounting {
                accounting_dept: r.accounting_dept,
                expense_type: match r.expense_type.as_deref() {
                    None => None,
                    Some(e) => Some(ExpenseType::from_code(e).ok_or_else(|| bad(e))?),
                },
                control_type: ControlType::from_code(&r.control_type)
                    .ok_or_else(|| bad(&r.control_type))?,
                cost_center: match r.cost_center.as_deref() {
                    None => None,
                    Some(c) => Some(DeptCode::parse(c).map_err(|_| bad(c))?),
                },
            },
            active: r.active,
            created_by: r.created_by,
            created_dept: r.created_dept,
            created_on: r.created_on,
            updated_by: r.updated_by,
            updated_on: r.updated_on,
            stamp: Stamp {
                version: r.version,
                created_at: Some(r.created_at),
                updated_at: r.updated_at,
            },
        })
    }
}

#[async_trait]
impl DepartmentRepository for PgStore {
    async fn get(&self, plant: &PlantId, code: &DeptCode) -> Result<Option<Department>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        sqlx::query_as::<_, DeptRow>(select_dept!("WHERE dept_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?
            .map(Department::try_from)
            .transpose()
    }

    async fn query_departments(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Department>, u64), RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok((Vec::new(), 0));
        };
        let (rows, total) = query_page_in::<DeptRow>(
            &mut tx,
            "SELECT COUNT(*) FROM department d WHERE TRUE",
            select_dept!("d WHERE TRUE"),
            |qb| push_filters(qb, filters, dept_column),
            r#" ORDER BY d.dept_code COLLATE "C""#,
            offset,
            limit,
        )
        .await?;
        let depts = rows
            .into_iter()
            .map(Department::try_from)
            .collect::<Result<_, _>>()?;
        Ok((depts, total))
    }

    async fn insert(&self, plant: &PlantId, d: &Department) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &d.accounting;
        sqlx::query(
            "INSERT INTO department (dept_code, name, full_name, accounting_dept, expense_type, control_type,
                 cost_center, active, created_by, created_dept, created_on, updated_by, updated_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        )
        .bind(d.code.as_str())
        .bind(&d.attrs.name)
        .bind(&d.attrs.full_name)
        .bind(a.accounting_dept)
        .bind(a.expense_type.map(|e| e.code()))
        .bind(a.control_type.code())
        .bind(a.cost_center.as_ref().map(|c| c.as_str()))
        .bind(d.active)
        .bind(&d.created_by)
        .bind(&d.created_dept)
        .bind(d.created_on)
        .bind(&d.updated_by)
        .bind(d.updated_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn update(&self, plant: &PlantId, d: &Department) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &d.accounting;
        let n = sqlx::query(
            "UPDATE department SET name = $2, full_name = $3, accounting_dept = $4, expense_type = $5,
                 control_type = $6, cost_center = $7, active = $8, updated_by = $9, updated_on = $10
             WHERE dept_code = $1 AND version = $11",
        )
        .bind(d.code.as_str())
        .bind(&d.attrs.name)
        .bind(&d.attrs.full_name)
        .bind(a.accounting_dept)
        .bind(a.expense_type.map(|e| e.code()))
        .bind(a.control_type.code())
        .bind(a.cost_center.as_ref().map(|c| c.as_str()))
        .bind(d.active)
        .bind(&d.updated_by)
        .bind(d.updated_on)
        .bind(d.stamp.version)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        tx.commit().await.map_err(db_err)
    }

    async fn delete(&self, plant: &PlantId, code: &DeptCode) -> Result<bool, RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        // 還有員工（或被其他部門當成本中心）時外鍵擋下，由 db_err 轉成 InUse（DQ5）
        let n = sqlx::query("DELETE FROM department WHERE dept_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}

// ----- 員工 -----

/// 查詢鍵值（EmployeeDto 欄位名稱，ttrs_app::query::employee）→ 資料庫欄位
fn employee_column(field: &str) -> Option<Column> {
    Some(match field {
        "employee_id" => Column::Scalar("e.employee_id", true),
        "name" => Column::Scalar("e.name", true),
        "dept_code" => Column::Scalar("e.dept_code", true),
        "title" => Column::Scalar("e.title", true),
        "extension" => Column::Scalar("e.extension", true),
        "email" => Column::Scalar("e.email", true),
        "active" => Column::Scalar("e.active", false),
        _ => return None,
    })
}

#[derive(FromRow)]
struct EmployeeRow {
    employee_id: String,
    name: String,
    dept_code: Option<String>,
    title: Option<String>,
    extension: Option<String>,
    email: Option<String>,
    active: bool,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

macro_rules! select_employee {
    ($rest:literal) => {
        concat!(
            "SELECT employee_id, name, dept_code, title, extension, email, active, ",
            "created_by, created_dept, created_on, updated_by, updated_on, version, created_at, updated_at ",
            "FROM employee ",
            $rest
        )
    };
}

impl TryFrom<EmployeeRow> for Employee {
    type Error = RepoError;

    fn try_from(r: EmployeeRow) -> Result<Self, RepoError> {
        Ok(Employee {
            id: EmployeeId::parse(&r.employee_id).map_err(|_| bad(&r.employee_id))?,
            attrs: EmployeeAttrs {
                name: r.name,
                dept_code: match r.dept_code.as_deref() {
                    None => None,
                    Some(d) => Some(DeptCode::parse(d).map_err(|_| bad(d))?),
                },
                title: r.title,
                extension: r.extension,
                email: r.email,
            },
            active: r.active,
            created_by: r.created_by,
            created_dept: r.created_dept,
            created_on: r.created_on,
            updated_by: r.updated_by,
            updated_on: r.updated_on,
            stamp: Stamp {
                version: r.version,
                created_at: Some(r.created_at),
                updated_at: r.updated_at,
            },
        })
    }
}

#[async_trait]
impl EmployeeRepository for PgStore {
    async fn get(&self, plant: &PlantId, id: &EmployeeId) -> Result<Option<Employee>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        sqlx::query_as::<_, EmployeeRow>(select_employee!("WHERE employee_id = $1"))
            .bind(id.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?
            .map(Employee::try_from)
            .transpose()
    }

    async fn query_employees(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Employee>, u64), RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok((Vec::new(), 0));
        };
        let (rows, total) = query_page_in::<EmployeeRow>(
            &mut tx,
            "SELECT COUNT(*) FROM employee e WHERE TRUE",
            select_employee!("e WHERE TRUE"),
            |qb| push_filters(qb, filters, employee_column),
            r#" ORDER BY e.employee_id COLLATE "C""#,
            offset,
            limit,
        )
        .await?;
        let employees = rows
            .into_iter()
            .map(Employee::try_from)
            .collect::<Result<_, _>>()?;
        Ok((employees, total))
    }

    async fn insert(&self, plant: &PlantId, e: &Employee) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &e.attrs;
        sqlx::query(
            "INSERT INTO employee (employee_id, name, dept_code, title, extension, email, active,
                 created_by, created_dept, created_on, updated_by, updated_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(e.id.as_str())
        .bind(&a.name)
        .bind(a.dept_code.as_ref().map(|d| d.as_str()))
        .bind(&a.title)
        .bind(&a.extension)
        .bind(&a.email)
        .bind(e.active)
        .bind(&e.created_by)
        .bind(&e.created_dept)
        .bind(e.created_on)
        .bind(&e.updated_by)
        .bind(e.updated_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn update(&self, plant: &PlantId, e: &Employee) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &e.attrs;
        let n = sqlx::query(
            "UPDATE employee SET name = $2, dept_code = $3, title = $4, extension = $5, email = $6,
                 active = $7, updated_by = $8, updated_on = $9
             WHERE employee_id = $1 AND version = $10",
        )
        .bind(e.id.as_str())
        .bind(&a.name)
        .bind(a.dept_code.as_ref().map(|d| d.as_str()))
        .bind(&a.title)
        .bind(&a.extension)
        .bind(&a.email)
        .bind(e.active)
        .bind(&e.updated_by)
        .bind(e.updated_on)
        .bind(e.stamp.version)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        tx.commit().await.map_err(db_err)
    }

    async fn delete(&self, plant: &PlantId, id: &EmployeeId) -> Result<bool, RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let n = sqlx::query("DELETE FROM employee WHERE employee_id = $1")
            .bind(id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}
