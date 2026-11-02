//! 程式執行權限維護（program_permission_maintenance.feature）的 PostgreSQL 實作；
//! 表結構見 migrations/0003_program_registry.sql、0013_program_permission.sql

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::{FromRow, Postgres, postgres::PgArguments, query::Query};
use ttrs_app::{
    error::RepoError,
    ports::{PermissionListItem, ProgramName, ProgramPermissionRepository},
};
use ttrs_domain::{
    lang::Lang,
    permission::{DetailRows, OtherDeptData, OtherUserData, PermissionAttrs, ProgramPermission},
    permission_class::{ClassCode, PermissionClass},
    program::ProgramNo,
    qbe::QbeFilter,
};

use crate::{PgStore, pg::db_err, qbe_sql::Column};

/// 程式執行權限單身的查詢鍵值（ttrs_app::query::program_permission）→ 資料庫欄位（別名 pp）
pub(crate) fn permission_column(field: &str) -> Option<Column> {
    Some(match field {
        "program_no" => Column::Scalar("pp.program_no", true),
        "actions" => Column::TextArray("pp.actions"),
        "other_user_data" => Column::Scalar("pp.other_user_data", true),
        "other_dept_data" => Column::Scalar("pp.other_dept_data", true),
        "detail_rows" => Column::Scalar("pp.detail_rows", true),
        _ => return None,
    })
}

#[derive(FromRow)]
struct PermissionRow {
    class_code: String,
    program_no: String,
    actions: Vec<String>,
    other_user_data: String,
    other_dept_data: String,
    detail_rows: String,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
}

#[derive(FromRow)]
struct ListRow {
    #[sqlx(flatten)]
    permission: PermissionRow,
    default_actions: Option<String>,
}

/// SELECT 字面值（concat! 於編譯期組字串，仍為 SQL 字面值）
macro_rules! select_permission {
    ($rest:literal) => {
        concat!(
            "SELECT pp.class_code, pp.program_no, pp.actions, pp.other_user_data, pp.other_dept_data, ",
            "pp.detail_rows, pp.created_by, pp.created_dept, pp.created_on, pp.updated_by, pp.updated_on ",
            $rest
        )
    };
}

fn bad(what: &str, v: &str) -> RepoError {
    RepoError::Other(format!("資料庫內容不合法：{what}={v}"))
}

impl TryFrom<PermissionRow> for ProgramPermission {
    type Error = RepoError;

    fn try_from(r: PermissionRow) -> Result<Self, RepoError> {
        Ok(ProgramPermission {
            class: ClassCode::parse(&r.class_code).map_err(|_| bad("class_code", &r.class_code))?,
            program: ProgramNo::parse(&r.program_no)
                .map_err(|_| bad("program_no", &r.program_no))?,
            attrs: PermissionAttrs {
                actions: r.actions,
                other_user: OtherUserData::from_code(&r.other_user_data)
                    .ok_or_else(|| bad("other_user_data", &r.other_user_data))?,
                other_dept: OtherDeptData::from_code(&r.other_dept_data)
                    .ok_or_else(|| bad("other_dept_data", &r.other_dept_data))?,
                detail: DetailRows::from_code(&r.detail_rows)
                    .ok_or_else(|| bad("detail_rows", &r.detail_rows))?,
            },
            created_by: r.created_by,
            created_dept: r.created_dept,
            created_on: r.created_on,
            updated_by: r.updated_by,
            updated_on: r.updated_on,
        })
    }
}

/// 新增權限的 INSERT（p_zz 授權與 p_zy 共用）；`sql` 為下列兩個字面值之一
pub(crate) const INSERT_PERMISSION: &str =
    "INSERT INTO program_permission (class_code, program_no, actions,
         other_user_data, other_dept_data, detail_rows, created_by, created_dept, created_on,
         updated_by, updated_on)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)";
/// 已存在則不變（p_zz 授權）
pub(crate) const INSERT_PERMISSION_IF_ABSENT: &str =
    "INSERT INTO program_permission (class_code, program_no, actions,
         other_user_data, other_dept_data, detail_rows, created_by, created_dept, created_on,
         updated_by, updated_on)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
     ON CONFLICT DO NOTHING";

pub(crate) fn insert_permission<'q>(
    sql: &'static str,
    p: &'q ProgramPermission,
) -> Query<'q, Postgres, PgArguments> {
    let a = &p.attrs;
    sqlx::query(sql)
        .bind(p.class.as_str())
        .bind(p.program.as_str())
        .bind(&a.actions)
        .bind(a.other_user.code())
        .bind(a.other_dept.code())
        .bind(a.detail.code())
        .bind(&p.created_by)
        .bind(&p.created_dept)
        .bind(p.created_on)
        .bind(&p.updated_by)
        .bind(p.updated_on)
}

#[async_trait]
impl ProgramPermissionRepository for PgStore {
    async fn query_classes(
        &self,
        header: &[QbeFilter],
        detail: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<PermissionClass>, u64), RepoError> {
        self.query_classes_by(header, detail, offset, limit).await
    }

    async fn get(
        &self,
        class: &ClassCode,
        program: &ProgramNo,
    ) -> Result<Option<ProgramPermission>, RepoError> {
        sqlx::query_as::<_, PermissionRow>(select_permission!(
            "FROM program_permission pp WHERE pp.class_code = $1 AND pp.program_no = $2"
        ))
        .bind(class.as_str())
        .bind(program.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .map(ProgramPermission::try_from)
        .transpose()
    }

    async fn list(&self, class: &ClassCode) -> Result<Vec<PermissionListItem>, RepoError> {
        let rows = sqlx::query_as::<_, ListRow>(select_permission!(
            ", p.default_actions FROM program_permission pp
             JOIN program p ON p.program_no = pp.program_no
             WHERE pp.class_code = $1 ORDER BY pp.program_no COLLATE \"C\""
        ))
        .bind(class.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        let name_rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT n.program_no, n.lang, n.name, n.report_title FROM program_name n
             JOIN program_permission pp ON pp.program_no = n.program_no
             WHERE pp.class_code = $1",
        )
        .bind(class.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        let mut names: HashMap<String, Vec<ProgramName>> = HashMap::new();
        for (no, lang, name, report_title) in name_rows {
            let lang = Lang::parse(&lang).map_err(|_| bad("lang", &lang))?;
            names.entry(no).or_default().push(ProgramName {
                lang,
                name,
                report_title,
            });
        }
        rows.into_iter()
            .map(|r| {
                let names = names.remove(&r.permission.program_no).unwrap_or_default();
                Ok(PermissionListItem {
                    permission: ProgramPermission::try_from(r.permission)?,
                    default_actions: r.default_actions,
                    names,
                })
            })
            .collect()
    }

    async fn insert(&self, p: &ProgramPermission) -> Result<(), RepoError> {
        insert_permission(INSERT_PERMISSION, p)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn update(&self, p: &ProgramPermission) -> Result<(), RepoError> {
        let a = &p.attrs;
        sqlx::query(
            "UPDATE program_permission SET actions = $3, other_user_data = $4, other_dept_data = $5,
                 detail_rows = $6, updated_by = $7, updated_on = $8
             WHERE class_code = $1 AND program_no = $2",
        )
        .bind(p.class.as_str())
        .bind(p.program.as_str())
        .bind(&a.actions)
        .bind(a.other_user.code())
        .bind(a.other_dept.code())
        .bind(a.detail.code())
        .bind(&p.updated_by)
        .bind(p.updated_on)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn delete(&self, class: &ClassCode, program: &ProgramNo) -> Result<bool, RepoError> {
        let r =
            sqlx::query("DELETE FROM program_permission WHERE class_code = $1 AND program_no = $2")
                .bind(class.as_str())
                .bind(program.as_str())
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
        Ok(r.rows_affected() > 0)
    }

    async fn delete_all(&self, class: &ClassCode) -> Result<u64, RepoError> {
        let r = sqlx::query("DELETE FROM program_permission WHERE class_code = $1")
            .bind(class.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(r.rows_affected())
    }
}
