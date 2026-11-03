//! 使用者維護（user_maintenance.feature）的 PostgreSQL 實作；表結構見 migrations/0010_user_account.sql

use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::UserRepository};
use ttrs_domain::{
    authorization::DataScope,
    lang::Lang,
    plant::PlantId,
    qbe::QbeFilter,
    user::{User, UserAttrs, UserId},
};

use crate::{
    PgStore,
    pg::db_err,
    qbe_sql::{Column, push_filters, push_scope, query_page},
};

/// 查詢鍵值（UserDto 欄位名稱，ttrs_app::query::user）→ 資料庫欄位
fn user_column(field: &str) -> Option<Column> {
    Some(match field {
        "user_id" => Column::Scalar("u.user_id", true),
        "name" => Column::Scalar("u.name", true),
        "dept_code" => Column::Scalar("u.dept_code", true),
        "permission_class" => Column::Scalar("u.class_code", true),
        "menu" => Column::Scalar("u.menu_program", true),
        "multi_plant" => Column::Scalar("u.multi_plant", false),
        "default_plant" => Column::Scalar("u.default_plant", true),
        "email" => Column::Scalar("u.email", true),
        "idp_username" => Column::Scalar("u.idp_username", true),
        "valid_until" => Column::Scalar("u.valid_until", false),
        "lang" => Column::Scalar("u.lang", true),
        "active" => Column::Scalar("u.active", false),
        "created_by" => Column::Scalar("u.created_by", true),
        "created_dept" => Column::Scalar("u.created_dept", true),
        "updated_by" => Column::Scalar("u.updated_by", true),
        "updated_on" => Column::Scalar("u.updated_on", false),
        _ => return None,
    })
}

#[derive(FromRow)]
struct UserRow {
    user_id: String,
    name: Option<String>,
    dept_code: Option<String>,
    class_code: String,
    menu_program: String,
    multi_plant: bool,
    default_plant: String,
    email: Option<String>,
    idp_username: Option<String>,
    idp_subject: Option<String>,
    lang: String,
    valid_until: Option<NaiveDate>,
    active: bool,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
}

/// SELECT 字面值（concat! 於編譯期組字串，仍為 SQL 字面值）
macro_rules! select_user {
    ($rest:literal) => {
        concat!(
            "SELECT user_id, name, dept_code, class_code, menu_program, multi_plant, default_plant, ",
            "email, idp_username, idp_subject, lang, valid_until, active, created_by, created_dept, ",
            "created_on, updated_by, updated_on FROM user_account ",
            $rest
        )
    };
}

fn bad(what: &str, v: &str) -> RepoError {
    RepoError::Other(format!("資料庫內容不合法：{what}={v}"))
}

impl TryFrom<UserRow> for User {
    type Error = RepoError;

    fn try_from(r: UserRow) -> Result<Self, RepoError> {
        Ok(User {
            user_id: UserId::parse(&r.user_id).map_err(|_| bad("user_id", &r.user_id))?,
            attrs: UserAttrs {
                name: r.name,
                dept_code: r.dept_code,
                permission_class: r.class_code,
                menu: r.menu_program,
                multi_plant: r.multi_plant,
                default_plant: PlantId::parse(&r.default_plant)
                    .map_err(|_| bad("default_plant", &r.default_plant))?,
                email: r.email,
                idp_username: r.idp_username,
                lang: Lang::parse(&r.lang).map_err(|_| bad("lang", &r.lang))?,
                valid_until: r.valid_until,
            },
            idp_subject: r.idp_subject,
            active: r.active,
            created_by: r.created_by,
            created_dept: r.created_dept,
            created_on: r.created_on,
            updated_by: r.updated_by,
            updated_on: r.updated_on,
        })
    }
}

async fn replace_plants(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &UserId,
    plants: &[PlantId],
) -> Result<(), RepoError> {
    sqlx::query("DELETE FROM user_plant WHERE user_id = $1")
        .bind(user.as_str())
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
    for p in plants {
        sqlx::query("INSERT INTO user_plant (user_id, plant_id) VALUES ($1, $2)")
            .bind(user.as_str())
            .bind(p.as_str())
            .execute(&mut **tx)
            .await
            .map_err(db_err)?;
    }
    Ok(())
}

#[async_trait]
impl UserRepository for PgStore {
    async fn get(&self, id: &UserId) -> Result<Option<User>, RepoError> {
        sqlx::query_as::<_, UserRow>(select_user!("WHERE user_id = $1"))
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .map(User::try_from)
            .transpose()
    }

    async fn list(&self) -> Result<Vec<User>, RepoError> {
        sqlx::query_as::<_, UserRow>(select_user!("ORDER BY user_id COLLATE \"C\""))
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(User::try_from)
            .collect()
    }

    async fn query(
        &self,
        filters: &[QbeFilter],
        scope: &DataScope,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<User>, u64), RepoError> {
        let (rows, total) = query_page::<UserRow>(
            &self.pool,
            "SELECT COUNT(*) FROM user_account u WHERE TRUE",
            select_user!("u WHERE TRUE"),
            |qb| {
                push_scope(qb, scope, "u.created_by", "u.created_dept");
                push_filters(qb, filters, user_column)
            },
            r#" ORDER BY u.user_id COLLATE "C""#,
            offset,
            limit,
        )
        .await?;
        let users = rows
            .into_iter()
            .map(User::try_from)
            .collect::<Result<_, _>>()?;
        Ok((users, total))
    }

    async fn insert(&self, user: &User, plants: &[PlantId]) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let a = &user.attrs;
        sqlx::query(
            "INSERT INTO user_account (user_id, name, dept_code, class_code, menu_program, multi_plant,
                 default_plant, email, idp_username, idp_subject, lang, valid_until, active,
                 created_by, created_dept, created_on, updated_by, updated_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
        )
        .bind(user.user_id.as_str())
        .bind(&a.name)
        .bind(&a.dept_code)
        .bind(&a.permission_class)
        .bind(&a.menu)
        .bind(a.multi_plant)
        .bind(a.default_plant.as_str())
        .bind(&a.email)
        .bind(&a.idp_username)
        .bind(&user.idp_subject)
        .bind(a.lang.as_str())
        .bind(a.valid_until)
        .bind(user.active)
        .bind(&user.created_by)
        .bind(&user.created_dept)
        .bind(user.created_on)
        .bind(&user.updated_by)
        .bind(user.updated_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        replace_plants(&mut tx, &user.user_id, plants).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn update(&self, user: &User, plants: &[PlantId]) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let a = &user.attrs;
        sqlx::query(
            "UPDATE user_account SET name = $2, dept_code = $3, class_code = $4, menu_program = $5,
                 multi_plant = $6, default_plant = $7, email = $8, idp_username = $9, idp_subject = $10,
                 lang = $11, valid_until = $12, active = $13, updated_by = $14, updated_on = $15
             WHERE user_id = $1",
        )
        .bind(user.user_id.as_str())
        .bind(&a.name)
        .bind(&a.dept_code)
        .bind(&a.permission_class)
        .bind(&a.menu)
        .bind(a.multi_plant)
        .bind(a.default_plant.as_str())
        .bind(&a.email)
        .bind(&a.idp_username)
        .bind(&user.idp_subject)
        .bind(a.lang.as_str())
        .bind(a.valid_until)
        .bind(user.active)
        .bind(&user.updated_by)
        .bind(user.updated_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        replace_plants(&mut tx, &user.user_id, plants).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete(&self, id: &UserId) -> Result<(), RepoError> {
        // user_plant 以 ON DELETE CASCADE 一併刪除（U9）
        sqlx::query("DELETE FROM user_account WHERE user_id = $1")
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn plants(&self, id: &UserId) -> Result<Vec<PlantId>, RepoError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT plant_id FROM user_plant WHERE user_id = $1 ORDER BY plant_id COLLATE \"C\"",
        )
        .bind(id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.into_iter()
            .map(|(p,)| PlantId::parse(&p).map_err(|_| bad("plant_id", &p)))
            .collect()
    }

    async fn find_by_idp_username(&self, username: &str) -> Result<Option<UserId>, RepoError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT user_id FROM user_account WHERE lower(idp_username) = lower($1)",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        row.map(|(id,)| UserId::parse(&id).map_err(|_| bad("user_id", &id)))
            .transpose()
    }
}
