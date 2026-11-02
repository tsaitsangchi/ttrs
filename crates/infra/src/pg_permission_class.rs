//! 權限類別維護（permission_class_maintenance.feature）的 PostgreSQL 實作；
//! 表結構見 migrations/0003_program_registry.sql、0011_permission_class.sql

use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::PermissionClassRepository};
use ttrs_domain::{
    permission_class::{ClassCode, ClassIdleControl, PermissionClass, PermissionClassAttrs},
    program::IdleAction,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct ClassRow {
    class_code: String,
    name: String,
    default_menu: Option<String>,
    idle_control: String,
    idle_action: Option<String>,
    idle_seconds: Option<i32>,
    active: bool,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
}

/// SELECT 字面值（concat! 於編譯期組字串，仍為 SQL 字面值）
macro_rules! select_class {
    ($rest:literal) => {
        concat!(
            "SELECT class_code, name, default_menu, idle_control, idle_action, idle_seconds, active, ",
            "created_by, created_dept, created_on, updated_by, updated_on FROM permission_class ",
            $rest
        )
    };
}

fn bad(what: &str, v: &str) -> RepoError {
    RepoError::Other(format!("資料庫內容不合法：{what}={v}"))
}

impl TryFrom<ClassRow> for PermissionClass {
    type Error = RepoError;

    fn try_from(r: ClassRow) -> Result<Self, RepoError> {
        let idle_action = match r.idle_action.as_deref() {
            None => None,
            Some(c) => Some(IdleAction::from_code(c).ok_or_else(|| bad("idle_action", c))?),
        };
        Ok(PermissionClass {
            code: ClassCode::parse(&r.class_code).map_err(|_| bad("class_code", &r.class_code))?,
            attrs: PermissionClassAttrs {
                name: r.name,
                default_menu: r.default_menu,
                idle_control: ClassIdleControl::from_code(&r.idle_control)
                    .ok_or_else(|| bad("idle_control", &r.idle_control))?,
                idle_action,
                idle_seconds: r.idle_seconds,
            },
            active: r.active,
            created_by: r.created_by,
            created_dept: r.created_dept,
            created_on: r.created_on,
            updated_by: r.updated_by,
            updated_on: r.updated_on,
        })
    }
}

async fn count(store: &PgStore, sql: &'static str, code: &ClassCode) -> Result<u64, RepoError> {
    let n: i64 = sqlx::query_scalar(sql)
        .bind(code.as_str())
        .fetch_one(&store.pool)
        .await
        .map_err(db_err)?;
    u64::try_from(n).map_err(|_| bad("count", &n.to_string()))
}

#[async_trait]
impl PermissionClassRepository for PgStore {
    async fn get(&self, code: &ClassCode) -> Result<Option<PermissionClass>, RepoError> {
        sqlx::query_as::<_, ClassRow>(select_class!("WHERE class_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .map(PermissionClass::try_from)
            .transpose()
    }

    async fn list(&self) -> Result<Vec<PermissionClass>, RepoError> {
        sqlx::query_as::<_, ClassRow>(select_class!("ORDER BY class_code COLLATE \"C\""))
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(PermissionClass::try_from)
            .collect()
    }

    async fn insert(&self, c: &PermissionClass) -> Result<(), RepoError> {
        let a = &c.attrs;
        sqlx::query(
            "INSERT INTO permission_class (class_code, name, default_menu, idle_control, idle_action,
                 idle_seconds, active, created_by, created_dept, created_on, updated_by, updated_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(c.code.as_str())
        .bind(&a.name)
        .bind(&a.default_menu)
        .bind(a.idle_control.code())
        .bind(a.idle_action.map(IdleAction::code))
        .bind(a.idle_seconds)
        .bind(c.active)
        .bind(&c.created_by)
        .bind(&c.created_dept)
        .bind(c.created_on)
        .bind(&c.updated_by)
        .bind(c.updated_on)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update(&self, c: &PermissionClass) -> Result<(), RepoError> {
        let a = &c.attrs;
        sqlx::query(
            "UPDATE permission_class SET name = $2, default_menu = $3, idle_control = $4,
                 idle_action = $5, idle_seconds = $6, active = $7, updated_by = $8, updated_on = $9
             WHERE class_code = $1",
        )
        .bind(c.code.as_str())
        .bind(&a.name)
        .bind(&a.default_menu)
        .bind(a.idle_control.code())
        .bind(a.idle_action.map(IdleAction::code))
        .bind(a.idle_seconds)
        .bind(c.active)
        .bind(&c.updated_by)
        .bind(c.updated_on)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn delete(&self, code: &ClassCode) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM permission_class WHERE class_code = $1")
            .bind(code.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn user_count(&self, code: &ClassCode) -> Result<u64, RepoError> {
        count(
            self,
            // 使用者維護的權限類別＋特殊權限的權限類別列（X7）
            "SELECT (SELECT count(*) FROM user_account WHERE class_code = $1)
                  + (SELECT count(DISTINCT user_id) FROM user_permission WHERE class_code = $1)",
            code,
        )
        .await
    }

    async fn permission_count(&self, code: &ClassCode) -> Result<u64, RepoError> {
        count(
            self,
            "SELECT count(*) FROM program_permission WHERE class_code = $1",
            code,
        )
        .await
    }
}
