//! 單別（document_type ← smy_file，與 smz、smu、smv 三個單身；document_type.feature）。工廠層
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, Postgres, Transaction};
use ttrs_app::{error::RepoError, ports::DocumentTypeRepository};
use ttrs_domain::{
    confirmation::Confirmation,
    department::DeptCode,
    document_type::{DocTypeAttrs, DocTypeCode, DocumentType, StorageRestriction, StorageRule},
    plant::PlantId,
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct Row {
    doc_type_code: String,
    name: String,
    system: String,
    kind: String,
    auto_number: bool,
    numbering: String,
    print_now: bool,
    confirm_now: bool,
    cost_enabled: bool,
    cost_class: String,
    affects_idle_date: bool,
    internal_trade: bool,
    budget_control: bool,
    memo: Option<String>,
    restriction: String,
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

macro_rules! select_head {
    ($rest:literal) => {
        concat!(
            "SELECT doc_type_code, name, system, kind, auto_number, numbering, print_now, confirm_now, cost_enabled, ",
            "cost_class, affects_idle_date, internal_trade, budget_control, memo, restriction, active, confirmed, ",
            "confirmed_by, confirmed_at, created_by, created_dept, created_on, updated_by, updated_on, version, ",
            "created_at, updated_at FROM document_type ",
            $rest
        )
    };
}

fn bad(e: ttrs_domain::DomainError) -> RepoError {
    RepoError::Other(format!("單別資料不正確：{e}"))
}

/// 三個單身：(單別, 倉庫, 儲位)、(單別, 使用者)、(單別, 部門)
type Children = (
    Vec<(String, String, String)>,
    Vec<(String, String)>,
    Vec<(String, String)>,
);

async fn load_children(
    tx: &mut Transaction<'static, Postgres>,
    code: Option<&str>,
) -> Result<Children, RepoError> {
    let storages: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT doc_type_code, warehouse, location FROM document_type_storage
          WHERE $1::text IS NULL OR doc_type_code = $1
          ORDER BY warehouse COLLATE \"C\", location COLLATE \"C\"",
    )
    .bind(code)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_err)?;
    let users: Vec<(String, String)> = sqlx::query_as(
        "SELECT doc_type_code, user_id FROM document_type_user
          WHERE $1::text IS NULL OR doc_type_code = $1 ORDER BY user_id COLLATE \"C\"",
    )
    .bind(code)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_err)?;
    let depts: Vec<(String, String)> = sqlx::query_as(
        "SELECT doc_type_code, dept_code FROM document_type_dept
          WHERE $1::text IS NULL OR doc_type_code = $1 ORDER BY dept_code COLLATE \"C\"",
    )
    .bind(code)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_err)?;
    Ok((storages, users, depts))
}

fn to_domain(r: Row, children: &Children) -> Result<DocumentType, RepoError> {
    let code = r.doc_type_code;
    let storages = children
        .0
        .iter()
        .filter(|(c, _, _)| *c == code)
        .map(|(_, w, l)| StorageRule::new(w, l))
        .collect::<Result<Vec<_>, _>>()
        .map_err(bad)?;
    let users = children
        .1
        .iter()
        .filter(|(c, _)| *c == code)
        .map(|(_, u)| u.clone())
        .collect();
    let depts = children
        .2
        .iter()
        .filter(|(c, _)| *c == code)
        .map(|(_, d)| DeptCode::parse(d))
        .collect::<Result<Vec<_>, _>>()
        .map_err(bad)?;
    Ok(DocumentType {
        code: DocTypeCode::existing(&code).map_err(bad)?,
        attrs: DocTypeAttrs {
            name: r.name,
            system: r.system,
            kind: r.kind,
            auto_number: r.auto_number,
            numbering: r.numbering,
            print_now: r.print_now,
            confirm_now: r.confirm_now,
            cost_enabled: r.cost_enabled,
            cost_class: r.cost_class,
            affects_idle_date: r.affects_idle_date,
            internal_trade: r.internal_trade,
            budget_control: r.budget_control,
            memo: r.memo,
        },
        restriction: StorageRestriction::parse(&r.restriction).map_err(bad)?,
        storages,
        users,
        depts,
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

/// 單身同步：只刪除已不在清單的列、只新增新的列（觸發器只記有變動的列）
async fn sync_children(
    tx: &mut Transaction<'static, Postgres>,
    d: &DocumentType,
) -> Result<(), RepoError> {
    let code = d.code.as_str();
    let ws: Vec<String> = d.storages.iter().map(|r| r.warehouse.clone()).collect();
    let ls: Vec<String> = d
        .storages
        .iter()
        .map(|r| r.location.clone().unwrap_or_default())
        .collect();
    sqlx::query(
        "DELETE FROM document_type_storage WHERE doc_type_code = $1
            AND (warehouse, location) NOT IN (SELECT * FROM unnest($2::text[], $3::text[]))",
    )
    .bind(code)
    .bind(&ws)
    .bind(&ls)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    sqlx::query(
        "INSERT INTO document_type_storage (doc_type_code, warehouse, location)
         SELECT $1, w, l FROM unnest($2::text[], $3::text[]) AS t(w, l) ON CONFLICT DO NOTHING",
    )
    .bind(code)
    .bind(&ws)
    .bind(&ls)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    sqlx::query(
        "DELETE FROM document_type_user WHERE doc_type_code = $1 AND NOT (user_id = ANY($2))",
    )
    .bind(code)
    .bind(&d.users)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    sqlx::query(
        "INSERT INTO document_type_user (doc_type_code, user_id)
         SELECT $1, u FROM unnest($2::text[]) AS t(u) ON CONFLICT DO NOTHING",
    )
    .bind(code)
    .bind(&d.users)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    let depts: Vec<String> = d.depts.iter().map(|x| x.as_str().to_owned()).collect();
    sqlx::query(
        "DELETE FROM document_type_dept WHERE doc_type_code = $1 AND NOT (dept_code = ANY($2))",
    )
    .bind(code)
    .bind(&depts)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    sqlx::query(
        "INSERT INTO document_type_dept (doc_type_code, dept_code)
         SELECT $1, x FROM unnest($2::text[]) AS t(x) ON CONFLICT DO NOTHING",
    )
    .bind(code)
    .bind(&depts)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    Ok(())
}

#[async_trait]
impl DocumentTypeRepository for PgStore {
    async fn list_document_types(&self, plant: &PlantId) -> Result<Vec<DocumentType>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<Row> = sqlx::query_as(select_head!("ORDER BY doc_type_code COLLATE \"C\""))
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?;
        let children = load_children(&mut tx, None).await?;
        tx.commit().await.map_err(db_err)?;
        rows.into_iter().map(|r| to_domain(r, &children)).collect()
    }

    async fn get_document_type(
        &self,
        plant: &PlantId,
        code: &DocTypeCode,
    ) -> Result<Option<DocumentType>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let row: Option<Row> = sqlx::query_as(select_head!("WHERE doc_type_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
        let children = load_children(&mut tx, Some(code.as_str())).await?;
        tx.commit().await.map_err(db_err)?;
        row.map(|r| to_domain(r, &children)).transpose()
    }

    async fn insert_document_type(
        &self,
        plant: &PlantId,
        d: &DocumentType,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &d.attrs;
        sqlx::query(
            "INSERT INTO document_type (doc_type_code, name, system, kind, auto_number, numbering, print_now, confirm_now,
                    cost_enabled, cost_class, affects_idle_date, internal_trade, budget_control, memo, restriction, active,
                    created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)",
        )
        .bind(d.code.as_str())
        .bind(&a.name)
        .bind(&a.system)
        .bind(&a.kind)
        .bind(a.auto_number)
        .bind(&a.numbering)
        .bind(a.print_now)
        .bind(a.confirm_now)
        .bind(a.cost_enabled)
        .bind(&a.cost_class)
        .bind(a.affects_idle_date)
        .bind(a.internal_trade)
        .bind(a.budget_control)
        .bind(&a.memo)
        .bind(d.restriction.code())
        .bind(d.active)
        .bind(&d.created_by)
        .bind(&d.created_dept)
        .bind(d.created_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sync_children(&mut tx, d).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_document_type(
        &self,
        plant: &PlantId,
        d: &DocumentType,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &d.attrs;
        let n = sqlx::query(
            "UPDATE document_type SET name = $2, system = $3, kind = $4, auto_number = $5, numbering = $6,
                    print_now = $7, confirm_now = $8, cost_enabled = $9, cost_class = $10, affects_idle_date = $11,
                    internal_trade = $12, budget_control = $13, memo = $14, restriction = $15, active = $16,
                    confirmed = $17, confirmed_by = $18, confirmed_at = $19, updated_by = $20, updated_on = $21
              WHERE doc_type_code = $1 AND version = $22",
        )
        .bind(d.code.as_str())
        .bind(&a.name)
        .bind(&a.system)
        .bind(&a.kind)
        .bind(a.auto_number)
        .bind(&a.numbering)
        .bind(a.print_now)
        .bind(a.confirm_now)
        .bind(a.cost_enabled)
        .bind(&a.cost_class)
        .bind(a.affects_idle_date)
        .bind(a.internal_trade)
        .bind(a.budget_control)
        .bind(&a.memo)
        .bind(d.restriction.code())
        .bind(d.active)
        .bind(d.confirmation.confirmed)
        .bind(&d.confirmation.by)
        .bind(d.confirmation.at)
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
        sync_children(&mut tx, d).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete_document_type(
        &self,
        plant: &PlantId,
        code: &DocTypeCode,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        for sql in [
            "DELETE FROM document_type_storage WHERE doc_type_code = $1",
            "DELETE FROM document_type_user WHERE doc_type_code = $1",
            "DELETE FROM document_type_dept WHERE doc_type_code = $1",
            "DELETE FROM document_type WHERE doc_type_code = $1",
        ] {
            sqlx::query(sql)
                .bind(code.as_str())
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn any_document_type(&self) -> Result<bool, RepoError> {
        let plants: Vec<String> = sqlx::query_scalar("SELECT plant_id FROM plant")
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        for p in plants {
            let Ok(plant) = PlantId::parse(&p) else {
                continue;
            };
            let Some(mut tx) = self.plant_tx(&plant).await? else {
                continue;
            };
            let any: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM document_type)")
                .fetch_one(&mut *tx)
                .await
                .map_err(db_err)?;
            tx.commit().await.map_err(db_err)?;
            if any {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
