//! 營運中心維護（plant_maintenance.feature）的 PostgreSQL 實作；
//! 表結構見 migrations/0001_init.sql、0012_plant.sql、0016_plant_schema.sql（一廠一個 schema，ADR 0007）

use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::PlantRepository};
use ttrs_domain::{
    plant::{Plant, PlantAttrs, PlantId, PlantSchema},
    qbe::QbeFilter,
};

use crate::{
    PgStore,
    pg::db_err,
    plant_schema::prepare,
    qbe_sql::{Column, push_filters, query_page},
};

/// 查詢鍵值（PlantDto 欄位名稱，ttrs_app::query::plant）→ 資料庫欄位
fn plant_column(field: &str) -> Option<Column> {
    Some(match field {
        "plant_id" => Column::Scalar("p.plant_id", true),
        "name" => Column::Scalar("p.name", true),
        "time_zone" => Column::Scalar("p.time_zone", true),
        "address" => Column::Scalar("p.address", true),
        "phone" => Column::Scalar("p.phone", true),
        "fax" => Column::Scalar("p.fax", true),
        "active" => Column::Scalar("p.active", false),
        "schema_name" => Column::Scalar("p.schema_name", true),
        _ => return None,
    })
}

#[derive(FromRow)]
struct PlantRow {
    plant_id: String,
    schema_name: String,
    name: String,
    time_zone: String,
    address: Option<String>,
    phone: Option<String>,
    fax: Option<String>,
    active: bool,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
}

/// SELECT 字面值（concat! 於編譯期組字串，仍為 SQL 字面值）
macro_rules! select_plant {
    ($rest:literal) => {
        concat!(
            "SELECT plant_id, schema_name, name, time_zone, address, phone, fax, active, ",
            "created_by, created_dept, created_on, updated_by, updated_on FROM plant ",
            $rest
        )
    };
}

impl TryFrom<PlantRow> for Plant {
    type Error = RepoError;

    fn try_from(r: PlantRow) -> Result<Self, RepoError> {
        Ok(Plant {
            id: PlantId::parse(&r.plant_id).map_err(|_| {
                RepoError::Other(format!("資料庫內容不合法：plant_id={}", r.plant_id))
            })?,
            schema: PlantSchema::parse(&r.schema_name).map_err(|_| {
                RepoError::Other(format!("資料庫內容不合法：schema_name={}", r.schema_name))
            })?,
            attrs: PlantAttrs {
                name: r.name,
                time_zone: r.time_zone,
                address: r.address,
                phone: r.phone,
                fax: r.fax,
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

#[async_trait]
impl PlantRepository for PgStore {
    async fn get(&self, id: &PlantId) -> Result<Option<Plant>, RepoError> {
        sqlx::query_as::<_, PlantRow>(select_plant!("WHERE plant_id = $1"))
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .map(Plant::try_from)
            .transpose()
    }

    async fn list(&self) -> Result<Vec<Plant>, RepoError> {
        sqlx::query_as::<_, PlantRow>(select_plant!("ORDER BY plant_id COLLATE \"C\""))
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(Plant::try_from)
            .collect()
    }

    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Plant>, u64), RepoError> {
        let (rows, total) = query_page::<PlantRow>(
            &self.pool,
            "SELECT COUNT(*) FROM plant p WHERE TRUE",
            select_plant!("p WHERE TRUE"),
            |qb| push_filters(qb, filters, plant_column),
            r#" ORDER BY p.plant_id COLLATE "C""#,
            offset,
            limit,
        )
        .await?;
        let plants = rows
            .into_iter()
            .map(Plant::try_from)
            .collect::<Result<_, _>>()?;
        Ok((plants, total))
    }

    async fn schema_names(&self) -> Result<Vec<PlantSchema>, RepoError> {
        let names: Vec<String> = sqlx::query_scalar("SELECT schema_name FROM plant")
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        names
            .iter()
            .map(|n| {
                PlantSchema::parse(n)
                    .map_err(|_| RepoError::Other(format!("資料庫內容不合法：schema_name={n}")))
            })
            .collect()
    }

    async fn insert(&self, p: &Plant) -> Result<(), RepoError> {
        let a = &p.attrs;
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query(
            "INSERT INTO plant (plant_id, schema_name, name, time_zone, address, phone, fax, active,
                 created_by, created_dept, created_on, updated_by, updated_on)
             VALUES ($1, $13, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(p.id.as_str())
        .bind(&a.name)
        .bind(&a.time_zone)
        .bind(&a.address)
        .bind(&a.phone)
        .bind(&a.fax)
        .bind(p.active)
        .bind(&p.created_by)
        .bind(&p.created_dept)
        .bind(p.created_on)
        .bind(&p.updated_by)
        .bind(p.updated_on)
        .bind(p.schema.as_str())
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        // 同一個交易建立工廠 schema、角色並套用工廠 migration；失敗則全部不建（P14）
        prepare(&mut tx, p.schema.as_str())
            .await
            .map_err(|e| RepoError::Other(format!("建立工廠 schema 失敗：{e}")))?;
        tx.commit().await.map_err(db_err)
    }

    async fn update(&self, p: &Plant) -> Result<(), RepoError> {
        let a = &p.attrs;
        sqlx::query(
            "UPDATE plant SET name = $2, time_zone = $3, address = $4, phone = $5, fax = $6,
                 active = $7, updated_by = $8, updated_on = $9
             WHERE plant_id = $1",
        )
        .bind(p.id.as_str())
        .bind(&a.name)
        .bind(&a.time_zone)
        .bind(&a.address)
        .bind(&a.phone)
        .bind(&a.fax)
        .bind(p.active)
        .bind(&p.updated_by)
        .bind(p.updated_on)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    /// 使用者預設／可使用營運中心（外鍵 user_account、user_plant）或工廠 schema 內有資料（P10）
    async fn in_use(&self, id: &PlantId) -> Result<bool, RepoError> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM user_account WHERE default_plant = p.plant_id)
                 OR EXISTS (SELECT 1 FROM user_plant WHERE plant_id = p.plant_id)
                 OR ttrs_plant_schema_has_data(p.schema_name)
               FROM plant p WHERE p.plant_id = $1",
        )
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)
        .map(|v: Option<bool>| v.unwrap_or(false))
    }

    async fn delete(&self, id: &PlantId) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let schema: Option<String> =
            sqlx::query_scalar("SELECT schema_name FROM plant WHERE plant_id = $1 FOR UPDATE")
                .bind(id.as_str())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_err)?;
        let Some(schema) = schema else {
            return Ok(());
        };
        // 工廠 schema 內有任何資料時不可刪除（P10）
        let has_data: bool = sqlx::query_scalar("SELECT ttrs_plant_schema_has_data($1)")
            .bind(&schema)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_err)?;
        if has_data {
            return Err(RepoError::InUse);
        }
        // 仍被使用者參照時外鍵擋下，由 db_err 轉成 InUse（P10）
        sqlx::query("DELETE FROM plant WHERE plant_id = $1")
            .bind(id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("SELECT ttrs_drop_plant_schema($1)")
            .bind(&schema)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }
}
