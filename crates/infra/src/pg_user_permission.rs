//! 使用者特殊權限維護（user_permission_maintenance.feature）的 PostgreSQL 實作；
//! 表結構見 migrations/0014_user_permission.sql

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{
    error::RepoError,
    ports::{ProgramName, UserPermissionItem, UserPermissionRepository},
};
use ttrs_domain::record::Stamp;
use ttrs_domain::{
    lang::Lang,
    permission::{DetailRows, OtherDeptData, OtherUserData, PermissionAttrs},
    permission_class::ClassCode,
    program::ProgramNo,
    user::UserId,
    user_permission::{SpecialTarget, UserPermission},
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct Row {
    user_id: String,
    line_no: i32,
    kind: String,
    class_code: Option<String>,
    program_no: Option<String>,
    actions: Option<Vec<String>>,
    other_user_data: Option<String>,
    other_dept_data: Option<String>,
    detail_rows: Option<String>,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    class_name: Option<String>,
    default_actions: Option<String>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

fn bad(what: &str, v: &str) -> RepoError {
    RepoError::Other(format!("資料庫內容不合法：{what}={v}"))
}

fn code<T>(what: &str, v: Option<String>, parse: fn(&str) -> Option<T>) -> Result<T, RepoError> {
    let v = v.unwrap_or_default();
    parse(&v).ok_or_else(|| bad(what, &v))
}

impl TryFrom<&mut Row> for UserPermission {
    type Error = RepoError;

    fn try_from(r: &mut Row) -> Result<Self, RepoError> {
        let target = match r.kind.as_str() {
            "1" => {
                let c = r.class_code.take().unwrap_or_default();
                SpecialTarget::Class(ClassCode::parse(&c).map_err(|_| bad("class_code", &c))?)
            }
            "2" => {
                let p = r.program_no.take().unwrap_or_default();
                SpecialTarget::Program {
                    program: ProgramNo::parse(&p).map_err(|_| bad("program_no", &p))?,
                    attrs: PermissionAttrs {
                        actions: r.actions.take().unwrap_or_default(),
                        other_user: code(
                            "other_user_data",
                            r.other_user_data.take(),
                            OtherUserData::from_code,
                        )?,
                        other_dept: code(
                            "other_dept_data",
                            r.other_dept_data.take(),
                            OtherDeptData::from_code,
                        )?,
                        detail: code("detail_rows", r.detail_rows.take(), DetailRows::from_code)?,
                    },
                }
            }
            other => return Err(bad("kind", other)),
        };
        Ok(UserPermission {
            user_id: UserId::parse(&r.user_id).map_err(|_| bad("user_id", &r.user_id))?,
            line_no: r.line_no,
            target,
            created_by: std::mem::take(&mut r.created_by),
            created_dept: std::mem::take(&mut r.created_dept),
            created_on: r.created_on,
            updated_by: r.updated_by.take(),
            updated_on: r.updated_on,
            stamp: Stamp {
                version: r.version,
                created_at: Some(r.created_at),
                updated_at: r.updated_at,
            },
        })
    }
}

/// 程式列的 (允許執行功能, 他人資料權限, 他部門資料權限, 單身權限)
type ProgramValues<'a> = (
    Option<&'a Vec<String>>,
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
);

/// 程式列的欄位值；權限類別列全為 NULL
fn program_values(p: &UserPermission) -> ProgramValues<'_> {
    match &p.target {
        SpecialTarget::Class(_) => (None, None, None, None),
        SpecialTarget::Program { attrs, .. } => (
            Some(&attrs.actions),
            Some(attrs.other_user.code()),
            Some(attrs.other_dept.code()),
            Some(attrs.detail.code()),
        ),
    }
}

#[async_trait]
impl UserPermissionRepository for PgStore {
    async fn list(&self, user: &UserId) -> Result<Vec<UserPermissionItem>, RepoError> {
        let rows = sqlx::query_as::<_, Row>(
            "SELECT up.user_id, up.line_no, up.kind, up.class_code, up.program_no, up.actions,
                    up.other_user_data, up.other_dept_data, up.detail_rows, up.created_by,
                    up.created_dept, up.created_on, up.updated_by, up.updated_on, up.version, up.created_at, up.updated_at,
                    c.name AS class_name, p.default_actions
             FROM user_permission up
             LEFT JOIN permission_class c ON c.class_code = up.class_code
             LEFT JOIN program p ON p.program_no = up.program_no
             WHERE up.user_id = $1 ORDER BY up.line_no",
        )
        .bind(user.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        let name_rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT n.program_no, n.lang, n.name, n.report_title FROM program_name n
             JOIN user_permission up ON up.program_no = n.program_no
             WHERE up.user_id = $1",
        )
        .bind(user.as_str())
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
            .map(|mut r| {
                let program_names = r
                    .program_no
                    .as_ref()
                    .and_then(|no| names.remove(no))
                    .unwrap_or_default();
                let class_name = r.class_name.take();
                let default_actions = r.default_actions.take();
                Ok(UserPermissionItem {
                    permission: UserPermission::try_from(&mut r)?,
                    class_name,
                    default_actions,
                    names: program_names,
                })
            })
            .collect()
    }

    async fn insert(&self, p: &UserPermission) -> Result<(), RepoError> {
        let (actions, user_data, dept_data, detail) = program_values(p);
        sqlx::query(
            "INSERT INTO user_permission (user_id, line_no, kind, class_code, program_no, actions,
                 other_user_data, other_dept_data, detail_rows, created_by, created_dept, created_on,
                 updated_by, updated_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
        )
        .bind(p.user_id.as_str())
        .bind(p.line_no)
        .bind(p.target.kind_code())
        .bind(p.target.class().map(ClassCode::as_str))
        .bind(p.target.program().map(ProgramNo::as_str))
        .bind(actions)
        .bind(user_data)
        .bind(dept_data)
        .bind(detail)
        .bind(&p.created_by)
        .bind(&p.created_dept)
        .bind(p.created_on)
        .bind(&p.updated_by)
        .bind(p.updated_on)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update(&self, p: &UserPermission) -> Result<(), RepoError> {
        let (actions, user_data, dept_data, detail) = program_values(p);
        let n = sqlx::query(
            "UPDATE user_permission SET kind = $3, class_code = $4, program_no = $5, actions = $6,
                 other_user_data = $7, other_dept_data = $8, detail_rows = $9, updated_by = $10,
                 updated_on = $11
             WHERE user_id = $1 AND line_no = $2 AND version = $12",
        )
        .bind(p.user_id.as_str())
        .bind(p.line_no)
        .bind(p.target.kind_code())
        .bind(p.target.class().map(ClassCode::as_str))
        .bind(p.target.program().map(ProgramNo::as_str))
        .bind(actions)
        .bind(user_data)
        .bind(dept_data)
        .bind(detail)
        .bind(&p.updated_by)
        .bind(p.updated_on)
        .bind(p.stamp.version)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        Ok(())
    }

    async fn delete(&self, user: &UserId, line_no: i32) -> Result<bool, RepoError> {
        let r = sqlx::query("DELETE FROM user_permission WHERE user_id = $1 AND line_no = $2")
            .bind(user.as_str())
            .bind(line_no)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(r.rows_affected() > 0)
    }

    async fn delete_all(&self, user: &UserId) -> Result<u64, RepoError> {
        let r = sqlx::query("DELETE FROM user_permission WHERE user_id = $1")
            .bind(user.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(r.rows_affected())
    }
}
