//! 資料控管原則（data_control_plant ← gev_file gev03＝N、data_control_rule ← gey_file；data_control.feature）。共用 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::DataControlRepository};
use ttrs_domain::{
    data_control::{Condition, ControlledPlant, FieldRule},
    group_center::DataType,
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct PlantRow {
    data_type: String,
    plant_id: String,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct RuleRow {
    data_type: String,
    plant_id: String,
    field_code: String,
    condition: i16,
    field_values: String,
}

macro_rules! select_plant {
    ($rest:literal) => {
        concat!(
            "SELECT data_type, plant_id, created_by, created_dept, created_on, updated_by, updated_on, version, ",
            "created_at, updated_at FROM data_control_plant ",
            $rest
        )
    };
}

fn to_domain(p: PlantRow, rules: &[RuleRow]) -> Result<ControlledPlant, RepoError> {
    let bad = || {
        RepoError::Other(format!(
            "資料控管資料不正確：{} {}",
            p.data_type, p.plant_id
        ))
    };
    let mut out = Vec::new();
    for r in rules
        .iter()
        .filter(|r| r.data_type == p.data_type && r.plant_id == p.plant_id)
    {
        out.push(FieldRule {
            field: r.field_code.clone(),
            condition: Condition::parse(&r.condition.to_string()).map_err(|_| bad())?,
            values: r.field_values.clone(),
        });
    }
    out.sort_by(|a, b| a.field.cmp(&b.field));
    Ok(ControlledPlant {
        data_type: DataType::parse(&p.data_type).map_err(|_| bad())?,
        plant: p.plant_id.clone(),
        rules: out,
        created_by: p.created_by.clone(),
        created_dept: p.created_dept.clone(),
        created_on: p.created_on,
        updated_by: p.updated_by.clone(),
        updated_on: p.updated_on,
        stamp: Stamp {
            version: p.version,
            created_at: Some(p.created_at),
            updated_at: p.updated_at,
        },
    })
}

fn condition_no(c: Condition) -> i16 {
    c.code().parse().unwrap_or(1)
}

#[async_trait]
impl DataControlRepository for PgStore {
    async fn list_controlled_plants(&self) -> Result<Vec<ControlledPlant>, RepoError> {
        let plants: Vec<PlantRow> = sqlx::query_as(select_plant!("ORDER BY data_type, plant_id"))
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        let rules: Vec<RuleRow> = sqlx::query_as(
            "SELECT data_type, plant_id, field_code, condition, field_values FROM data_control_rule",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        plants.into_iter().map(|p| to_domain(p, &rules)).collect()
    }

    async fn get_controlled_plant(
        &self,
        data_type: DataType,
        plant: &str,
    ) -> Result<Option<ControlledPlant>, RepoError> {
        let row: Option<PlantRow> =
            sqlx::query_as(select_plant!("WHERE data_type = $1 AND plant_id = $2"))
                .bind(data_type.code())
                .bind(plant)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        let Some(row) = row else { return Ok(None) };
        let rules: Vec<RuleRow> = sqlx::query_as(
            "SELECT data_type, plant_id, field_code, condition, field_values FROM data_control_rule
              WHERE data_type = $1 AND plant_id = $2",
        )
        .bind(data_type.code())
        .bind(plant)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        to_domain(row, &rules).map(Some)
    }

    async fn insert_controlled_plant(&self, p: &ControlledPlant) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        // 主鍵重複或已是資料中心（觸發器 ttrs_data_control_exclusive）時 db_err 轉成 Conflict（DC2）
        sqlx::query(
            "INSERT INTO data_control_plant (data_type, plant_id, created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(p.data_type.code())
        .bind(&p.plant)
        .bind(&p.created_by)
        .bind(&p.created_dept)
        .bind(p.created_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        for r in &p.rules {
            sqlx::query(
                "INSERT INTO data_control_rule (data_type, plant_id, field_code, condition, field_values)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(p.data_type.code())
            .bind(&p.plant)
            .bind(&r.field)
            .bind(condition_no(r.condition))
            .bind(&r.values)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn update_controlled_plant(&self, p: &ControlledPlant) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        // 版本先鎖定比對（樂觀鎖，ADR 0012 §4），不符時不寫欄位條件
        let current: Option<i32> = sqlx::query_scalar(
            "SELECT version FROM data_control_plant WHERE data_type = $1 AND plant_id = $2 FOR UPDATE",
        )
        .bind(p.data_type.code())
        .bind(&p.plant)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        if current != Some(p.stamp.version) {
            return Err(RepoError::Stale);
        }
        // 欄位條件只寫入有變動的列（與記憶體後端相同）；先寫條件、再更新營運中心列（異動記錄順序一致）
        let fields: Vec<String> = p.rules.iter().map(|r| r.field.clone()).collect();
        sqlx::query(
            "DELETE FROM data_control_rule
              WHERE data_type = $1 AND plant_id = $2 AND NOT (field_code = ANY($3))",
        )
        .bind(p.data_type.code())
        .bind(&p.plant)
        .bind(&fields)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        for r in &p.rules {
            sqlx::query(
                "INSERT INTO data_control_rule (data_type, plant_id, field_code, condition, field_values)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (data_type, plant_id, field_code) DO UPDATE
                    SET condition = EXCLUDED.condition, field_values = EXCLUDED.field_values
                  WHERE (data_control_rule.condition, data_control_rule.field_values)
                        IS DISTINCT FROM (EXCLUDED.condition, EXCLUDED.field_values)",
            )
            .bind(p.data_type.code())
            .bind(&p.plant)
            .bind(&r.field)
            .bind(condition_no(r.condition))
            .bind(&r.values)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        sqlx::query(
            "UPDATE data_control_plant SET updated_by = $3, updated_on = $4
              WHERE data_type = $1 AND plant_id = $2",
        )
        .bind(p.data_type.code())
        .bind(&p.plant)
        .bind(&p.updated_by)
        .bind(p.updated_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete_controlled_plant(
        &self,
        data_type: DataType,
        plant: &str,
    ) -> Result<bool, RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query("DELETE FROM data_control_rule WHERE data_type = $1 AND plant_id = $2")
            .bind(data_type.code())
            .bind(plant)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        let n =
            sqlx::query("DELETE FROM data_control_plant WHERE data_type = $1 AND plant_id = $2")
                .bind(data_type.code())
                .bind(plant)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?
                .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}
