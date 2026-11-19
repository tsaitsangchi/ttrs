//! 基本資料拋轉（aoop603；一個營運中心一個交易）與拋轉紀錄（carry_log ← gex_file；basic_data_carry.feature）
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use ttrs_app::{
    error::RepoError,
    ports::{BasicCarryBatch, BasicCarryRepository, CarryLogRepository, CarryWrite},
};
use ttrs_domain::{
    basic_carry::{CarryAction, CarryLogEntry},
    group_center::{DataType, GroupCenterCode},
    plant::PlantId,
};

use crate::{
    PgStore,
    pg::db_err,
    pg_classification::{code_insert, code_update},
    pg_department::{dept_insert, dept_update, employee_insert, employee_update},
    pg_item_group::{item_group_insert, item_group_update},
    pg_product_class::{product_class_insert, product_class_update},
};

#[derive(FromRow)]
struct LogRow {
    center_code: String,
    data_type: String,
    plant_id: String,
    program_no: String,
    record_key: String,
    action: String,
    carried_at: DateTime<Utc>,
    carried_by: String,
}

#[async_trait]
impl BasicCarryRepository for PgStore {
    /// 同一個交易寫入這個營運中心的全部資料；任一筆失敗整批回復（BC6(d)）
    async fn apply_basic_carry(
        &self,
        target: &PlantId,
        batch: &BasicCarryBatch,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(target).await?;
        for w in &batch.departments {
            match w {
                CarryWrite::Insert(d) => dept_insert(&mut tx, d).await?,
                CarryWrite::Update(d) => dept_update(&mut tx, d).await?,
            }
        }
        for w in &batch.employees {
            match w {
                CarryWrite::Insert(e) => employee_insert(&mut tx, e).await?,
                CarryWrite::Update(e) => employee_update(&mut tx, e).await?,
            }
        }
        for w in &batch.codes {
            match w {
                CarryWrite::Insert(c) => code_insert(&mut tx, c).await?,
                CarryWrite::Update(c) => code_update(&mut tx, c).await?,
            }
        }
        for w in &batch.product_classes {
            match w {
                CarryWrite::Insert(c) => product_class_insert(&mut tx, c).await?,
                CarryWrite::Update(c) => product_class_update(&mut tx, c).await?,
            }
        }
        for w in &batch.item_groups {
            match w {
                CarryWrite::Insert(g) => item_group_insert(&mut tx, g).await?,
                CarryWrite::Update(g) => item_group_update(&mut tx, g).await?,
            }
        }
        tx.commit().await.map_err(db_err)
    }
}

#[async_trait]
impl CarryLogRepository for PgStore {
    async fn append_carry_logs(&self, entries: &[CarryLogEntry]) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        for e in entries {
            sqlx::query(
                "INSERT INTO carry_log (center_code, data_type, plant_id, program_no, record_key, action,
                     carried_at, carried_by)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(e.center.as_str())
            .bind(e.data_type.code())
            .bind(&e.plant)
            .bind(&e.program)
            .bind(&e.record_key)
            .bind(e.action.code())
            .bind(e.carried_at)
            .bind(&e.carried_by)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn list_carry_logs(&self) -> Result<Vec<CarryLogEntry>, RepoError> {
        let rows: Vec<LogRow> = sqlx::query_as(
            "SELECT center_code, data_type, plant_id, program_no, record_key, action, carried_at, carried_by
               FROM carry_log ORDER BY carried_at DESC, log_id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.into_iter()
            .map(|r| {
                let bad = || RepoError::Other(format!("拋轉紀錄資料不正確：{}", r.record_key));
                Ok(CarryLogEntry {
                    center: GroupCenterCode::parse(&r.center_code).map_err(|_| bad())?,
                    data_type: DataType::parse(&r.data_type).map_err(|_| bad())?,
                    plant: r.plant_id.clone(),
                    program: r.program_no.clone(),
                    record_key: r.record_key.clone(),
                    action: CarryAction::parse(&r.action).ok_or_else(bad)?,
                    carried_at: r.carried_at,
                    carried_by: r.carried_by.clone(),
                })
            })
            .collect()
    }

    async fn carry_log_exists(
        &self,
        center: &GroupCenterCode,
        data_type: Option<DataType>,
    ) -> Result<bool, RepoError> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM carry_log
                             WHERE center_code = $1 AND ($2::text IS NULL OR data_type = $2))",
        )
        .bind(center.as_str())
        .bind(data_type.map(|t| t.code()))
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)
    }
}
