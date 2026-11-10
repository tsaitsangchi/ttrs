//! 基本代碼（region ← gea_file、country ← geb_file、area ← geo_file、shipping_method ← ged_file、
//! report_title ← azh_file；migrations/0041_basic_codes.sql）。共用 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::BasicCodeRepository};
use ttrs_domain::{
    basic_code::{BasicCode, BasicCodeAttrs, BasicCodeKind, BasicCodeName, BasicCodeValue},
    confirmation::Confirmation,
    lang::Lang,
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct CodeRow {
    code: String,
    name: Option<String>,
    parent: Option<String>,
    active: bool,
    confirmed: bool,
    confirmed_by: Option<String>,
    confirmed_at: Option<DateTime<Utc>>,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

/// 各表的 SQL（字面值，sqlx 不接受動態字串）
struct Sql {
    list: &'static str,
    get: &'static str,
    insert: &'static str,
    update: &'static str,
    delete: &'static str,
}

/// 表名、鍵值欄位、隸屬欄位（沒有時省略）
macro_rules! sql {
    ($table:literal, $key:literal) => {
        sql!(@ $table, $key, "NULL::varchar",
            concat!("INSERT INTO ", $table, " (", $key, ", name, active, created_by, created_dept, created_on, ",
                "confirmed, confirmed_by, confirmed_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"),
            concat!("UPDATE ", $table, " SET name = $2, active = $3, updated_by = $4, updated_on = $5, ",
                "confirmed = $7, confirmed_by = $8, confirmed_at = $9 WHERE ", $key, " = $1 AND version = $6"))
    };
    ($table:literal, $key:literal, $parent:literal) => {
        sql!(@ $table, $key, $parent,
            concat!("INSERT INTO ", $table, " (", $key, ", name, active, created_by, created_dept, created_on, ",
                "confirmed, confirmed_by, confirmed_at, ", $parent, ") VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"),
            concat!("UPDATE ", $table, " SET name = $2, active = $3, updated_by = $4, updated_on = $5, ",
                "confirmed = $7, confirmed_by = $8, confirmed_at = $9, ", $parent, " = $10 WHERE ", $key,
                " = $1 AND version = $6"))
    };
    (@ $table:literal, $key:literal, $parent:literal, $insert:expr, $update:expr) => {
        Sql {
            list: concat!(
                "SELECT ", $key, " AS code, name, ", $parent, " AS parent, active, confirmed, confirmed_by, confirmed_at, ",
                "created_by, created_dept, created_on, updated_by, updated_on, version, created_at, updated_at FROM ",
                $table, " ORDER BY ", $key, " COLLATE \"C\""
            ),
            get: concat!(
                "SELECT ", $key, " AS code, name, ", $parent, " AS parent, active, confirmed, confirmed_by, confirmed_at, ",
                "created_by, created_dept, created_on, updated_by, updated_on, version, created_at, updated_at FROM ",
                $table, " WHERE ", $key, " = $1"
            ),
            insert: $insert,
            update: $update,
            delete: concat!("DELETE FROM ", $table, " WHERE ", $key, " = $1"),
        }
    };
}

const REGION: Sql = sql!("region", "region_code");
const COUNTRY: Sql = sql!("country", "country_code", "region_code");
const AREA: Sql = sql!("area", "area_code", "country_code");
const SHIPPING: Sql = sql!("shipping_method", "method_code");
const TITLE: Sql = sql!("report_title", "title_code");

fn sql(kind: BasicCodeKind) -> &'static Sql {
    match kind {
        BasicCodeKind::Region => &REGION,
        BasicCodeKind::Country => &COUNTRY,
        BasicCodeKind::Area => &AREA,
        BasicCodeKind::ShippingMethod => &SHIPPING,
        BasicCodeKind::ReportTitle => &TITLE,
    }
}

fn to_code(
    kind: BasicCodeKind,
    r: CodeRow,
    names: Vec<BasicCodeName>,
) -> Result<BasicCode, RepoError> {
    Ok(BasicCode {
        kind,
        code: BasicCodeValue::parse(kind, &r.code)
            .map_err(|_| RepoError::Other(format!("{kind}資料不正確：{}", r.code)))?,
        attrs: BasicCodeAttrs {
            name: r.name,
            parent: r.parent,
            names,
        },
        active: r.active,
        confirmation: Confirmation {
            confirmed: r.confirmed,
            by: r.confirmed_by,
            at: r.confirmed_at,
        },
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

impl PgStore {
    /// 報表抬頭的其他語系名稱（report_title_text）：代號 → 名稱
    async fn title_texts(
        &self,
        code: Option<&str>,
    ) -> Result<Vec<(String, BasicCodeName)>, RepoError> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT title_code, lang, name FROM report_title_text
              WHERE $1::varchar IS NULL OR title_code = $1 ORDER BY title_code, lang",
        )
        .bind(code)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.into_iter()
            .map(|(c, lang, name)| {
                let lang = Lang::parse(&lang).map_err(|e| RepoError::Other(e.to_string()))?;
                Ok((c, BasicCodeName { lang, name }))
            })
            .collect()
    }

    async fn write_parts(
        tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
        c: &BasicCode,
        replace_texts: bool,
    ) -> Result<(), RepoError> {
        if c.kind.multilingual() {
            if replace_texts {
                sqlx::query("DELETE FROM report_title_text WHERE title_code = $1")
                    .bind(c.code.as_str())
                    .execute(&mut **tx)
                    .await
                    .map_err(db_err)?;
            }
            for t in &c.attrs.names {
                sqlx::query(
                    "INSERT INTO report_title_text (title_code, lang, name) VALUES ($1, $2, $3)",
                )
                .bind(c.code.as_str())
                .bind(t.lang.as_str())
                .bind(&t.name)
                .execute(&mut **tx)
                .await
                .map_err(db_err)?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl BasicCodeRepository for PgStore {
    async fn list_basic_codes(&self, kind: BasicCodeKind) -> Result<Vec<BasicCode>, RepoError> {
        let rows = sqlx::query_as::<_, CodeRow>(sql(kind).list)
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        let mut texts = if kind.multilingual() {
            self.title_texts(None).await?
        } else {
            Vec::new()
        };
        rows.into_iter()
            .map(|r| {
                let mut names = Vec::new();
                texts.retain(|(c, t)| {
                    if *c == r.code {
                        names.push(t.clone());
                        false
                    } else {
                        true
                    }
                });
                to_code(kind, r, names)
            })
            .collect()
    }

    async fn get_basic_code(
        &self,
        kind: BasicCodeKind,
        code: &BasicCodeValue,
    ) -> Result<Option<BasicCode>, RepoError> {
        let Some(r) = sqlx::query_as::<_, CodeRow>(sql(kind).get)
            .bind(code.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
        else {
            return Ok(None);
        };
        let names = if kind.multilingual() {
            self.title_texts(Some(code.as_str()))
                .await?
                .into_iter()
                .map(|(_, t)| t)
                .collect()
        } else {
            Vec::new()
        };
        to_code(kind, r, names).map(Some)
    }

    async fn insert_basic_code(&self, c: &BasicCode) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let q = sqlx::query(sql(c.kind).insert)
            .bind(c.code.as_str())
            .bind(&c.attrs.name)
            .bind(c.active)
            .bind(&c.created_by)
            .bind(&c.created_dept)
            .bind(c.created_on)
            .bind(c.confirmation.confirmed)
            .bind(&c.confirmation.by)
            .bind(c.confirmation.at);
        let q = if c.kind.parent().is_some() {
            q.bind(&c.attrs.parent)
        } else {
            q
        };
        q.execute(&mut *tx).await.map_err(db_err)?;
        Self::write_parts(&mut tx, c, false).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_basic_code(&self, c: &BasicCode) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let q = sqlx::query(sql(c.kind).update)
            .bind(c.code.as_str())
            .bind(&c.attrs.name)
            .bind(c.active)
            .bind(&c.updated_by)
            .bind(c.updated_on)
            .bind(c.stamp.version)
            .bind(c.confirmation.confirmed)
            .bind(&c.confirmation.by)
            .bind(c.confirmation.at);
        let q = if c.kind.parent().is_some() {
            q.bind(&c.attrs.parent)
        } else {
            q
        };
        let n = q.execute(&mut *tx).await.map_err(db_err)?.rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        Self::write_parts(&mut tx, c, true).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete_basic_code(
        &self,
        kind: BasicCodeKind,
        code: &BasicCodeValue,
    ) -> Result<bool, RepoError> {
        // 被下層代碼引用時外鍵擋下，由 db_err 轉成 InUse（RC4）
        let n = sqlx::query(sql(kind).delete)
            .bind(code.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?
            .rows_affected();
        Ok(n > 0)
    }
}
