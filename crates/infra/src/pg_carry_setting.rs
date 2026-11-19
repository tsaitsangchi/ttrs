//! 資料中心拋轉設定（carry_setting、carry_target、carry_condition ← gew_file；carry_default ← gez_file；carry_setting.feature）。共用 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, Postgres, Transaction};
use ttrs_app::{error::RepoError, ports::CarrySettingRepository};
use ttrs_domain::{
    carry_setting::{CarryDefault, CarryMode, CarrySetting, CarryTarget},
    data_control::{Condition, FieldRule},
    group_center::{DataType, GroupCenterCode},
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct HeadRow {
    center_code: String,
    data_type: String,
    mode: String,
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
struct TargetRow {
    center_code: String,
    data_type: String,
    plant_id: String,
    deletable: bool,
    update_existing: bool,
    notify: Option<String>,
}

#[derive(FromRow)]
struct ConditionRow {
    center_code: String,
    data_type: String,
    plant_id: String,
    field_code: String,
    condition: i16,
    field_values: String,
}

#[derive(FromRow)]
struct DefaultRow {
    center_code: String,
    data_type: String,
    plant_id: String,
    field_code: String,
    field_value: String,
}

struct Children {
    targets: Vec<TargetRow>,
    conditions: Vec<ConditionRow>,
    defaults: Vec<DefaultRow>,
}

macro_rules! select_head {
    ($rest:literal) => {
        concat!(
            "SELECT center_code, data_type, mode, created_by, created_dept, created_on, updated_by, updated_on, ",
            "version, created_at, updated_at FROM carry_setting ",
            $rest
        )
    };
}

fn to_domain(h: HeadRow, ch: &Children) -> Result<CarrySetting, RepoError> {
    let bad = || {
        RepoError::Other(format!(
            "拋轉設定資料不正確：{} {}",
            h.center_code, h.data_type
        ))
    };
    let mine = |c: &str, t: &str| c == h.center_code && t == h.data_type;
    let mut targets = Vec::new();
    for t in ch
        .targets
        .iter()
        .filter(|t| mine(&t.center_code, &t.data_type))
    {
        let mut conditions = Vec::new();
        for c in ch
            .conditions
            .iter()
            .filter(|c| mine(&c.center_code, &c.data_type) && c.plant_id == t.plant_id)
        {
            conditions.push(FieldRule {
                field: c.field_code.clone(),
                condition: Condition::parse(&c.condition.to_string()).map_err(|_| bad())?,
                values: c.field_values.clone(),
            });
        }
        conditions.sort_by(|a, b| a.field.cmp(&b.field));
        let mut defaults: Vec<CarryDefault> = ch
            .defaults
            .iter()
            .filter(|d| mine(&d.center_code, &d.data_type) && d.plant_id == t.plant_id)
            .map(|d| CarryDefault {
                field: d.field_code.clone(),
                value: d.field_value.clone(),
            })
            .collect();
        defaults.sort_by(|a, b| a.field.cmp(&b.field));
        targets.push(CarryTarget {
            plant: t.plant_id.clone(),
            deletable: t.deletable,
            update_existing: t.update_existing,
            notify: t.notify.clone(),
            conditions,
            defaults,
        });
    }
    targets.sort_by(|a, b| a.plant.cmp(&b.plant));
    Ok(CarrySetting {
        center: GroupCenterCode::parse(&h.center_code).map_err(|_| bad())?,
        data_type: DataType::parse(&h.data_type).map_err(|_| bad())?,
        mode: CarryMode::parse(&h.mode).map_err(|_| bad())?,
        targets,
        created_by: h.created_by.clone(),
        created_dept: h.created_dept.clone(),
        created_on: h.created_on,
        updated_by: h.updated_by.clone(),
        updated_on: h.updated_on,
        stamp: Stamp {
            version: h.version,
            created_at: Some(h.created_at),
            updated_at: h.updated_at,
        },
    })
}

fn condition_no(c: Condition) -> i16 {
    c.code().parse().unwrap_or(1)
}

impl PgStore {
    async fn carry_children(&self, key: Option<(&str, String)>) -> Result<Children, RepoError> {
        let (c, t) = key.map_or((None, None), |(c, t)| (Some(c.to_owned()), Some(t)));
        let targets: Vec<TargetRow> = sqlx::query_as(
            "SELECT center_code, data_type, plant_id, deletable, update_existing, notify FROM carry_target
              WHERE ($1::text IS NULL OR center_code = $1) AND ($2::text IS NULL OR data_type = $2)",
        )
        .bind(&c)
        .bind(&t)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        let conditions: Vec<ConditionRow> = sqlx::query_as(
            "SELECT center_code, data_type, plant_id, field_code, condition, field_values FROM carry_condition
              WHERE ($1::text IS NULL OR center_code = $1) AND ($2::text IS NULL OR data_type = $2)",
        )
        .bind(&c)
        .bind(&t)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        let defaults: Vec<DefaultRow> = sqlx::query_as(
            "SELECT center_code, data_type, plant_id, field_code, field_value FROM carry_default
              WHERE ($1::text IS NULL OR center_code = $1) AND ($2::text IS NULL OR data_type = $2)",
        )
        .bind(&c)
        .bind(&t)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(Children {
            targets,
            conditions,
            defaults,
        })
    }
}

/// 單身寫入：拋轉營運中心、條件、預設值只寫有變動的列（與記憶體後端相同）；刪除時先子後母
async fn sync_children(
    tx: &mut Transaction<'static, Postgres>,
    s: &CarrySetting,
) -> Result<(), RepoError> {
    let (center, t) = (s.center.as_str(), s.data_type.code());
    let plants: Vec<String> = s.targets.iter().map(|x| x.plant.clone()).collect();
    for sql in [
        "DELETE FROM carry_condition WHERE center_code = $1 AND data_type = $2 AND NOT (plant_id = ANY($3))",
        "DELETE FROM carry_default WHERE center_code = $1 AND data_type = $2 AND NOT (plant_id = ANY($3))",
        "DELETE FROM carry_target WHERE center_code = $1 AND data_type = $2 AND NOT (plant_id = ANY($3))",
    ] {
        sqlx::query(sql)
            .bind(center)
            .bind(&t)
            .bind(&plants)
            .execute(&mut **tx)
            .await
            .map_err(db_err)?;
    }
    for g in &s.targets {
        sqlx::query(
            "INSERT INTO carry_target (center_code, data_type, plant_id, deletable, update_existing, notify)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (center_code, data_type, plant_id) DO UPDATE
                SET deletable = EXCLUDED.deletable, update_existing = EXCLUDED.update_existing,
                    notify = EXCLUDED.notify
              WHERE (carry_target.deletable, carry_target.update_existing, carry_target.notify)
                    IS DISTINCT FROM (EXCLUDED.deletable, EXCLUDED.update_existing, EXCLUDED.notify)",
        )
        .bind(center)
        .bind(&t)
        .bind(&g.plant)
        .bind(g.deletable)
        .bind(g.update_existing)
        .bind(&g.notify)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
        let fields: Vec<String> = g.conditions.iter().map(|c| c.field.clone()).collect();
        sqlx::query(
            "DELETE FROM carry_condition
              WHERE center_code = $1 AND data_type = $2 AND plant_id = $3 AND NOT (field_code = ANY($4))",
        )
        .bind(center)
        .bind(&t)
        .bind(&g.plant)
        .bind(&fields)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
        for c in &g.conditions {
            sqlx::query(
                "INSERT INTO carry_condition (center_code, data_type, plant_id, field_code, condition, field_values)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (center_code, data_type, plant_id, field_code) DO UPDATE
                    SET condition = EXCLUDED.condition, field_values = EXCLUDED.field_values
                  WHERE (carry_condition.condition, carry_condition.field_values)
                        IS DISTINCT FROM (EXCLUDED.condition, EXCLUDED.field_values)",
            )
            .bind(center)
            .bind(&t)
            .bind(&g.plant)
            .bind(&c.field)
            .bind(condition_no(c.condition))
            .bind(&c.values)
            .execute(&mut **tx)
            .await
            .map_err(db_err)?;
        }
        let fields: Vec<String> = g.defaults.iter().map(|d| d.field.clone()).collect();
        sqlx::query(
            "DELETE FROM carry_default
              WHERE center_code = $1 AND data_type = $2 AND plant_id = $3 AND NOT (field_code = ANY($4))",
        )
        .bind(center)
        .bind(&t)
        .bind(&g.plant)
        .bind(&fields)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
        for d in &g.defaults {
            sqlx::query(
                "INSERT INTO carry_default (center_code, data_type, plant_id, field_code, field_value)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (center_code, data_type, plant_id, field_code) DO UPDATE
                    SET field_value = EXCLUDED.field_value
                  WHERE carry_default.field_value IS DISTINCT FROM EXCLUDED.field_value",
            )
            .bind(center)
            .bind(&t)
            .bind(&g.plant)
            .bind(&d.field)
            .bind(&d.value)
            .execute(&mut **tx)
            .await
            .map_err(db_err)?;
        }
    }
    Ok(())
}

#[async_trait]
impl CarrySettingRepository for PgStore {
    async fn list_carry_settings(&self) -> Result<Vec<CarrySetting>, RepoError> {
        let heads: Vec<HeadRow> = sqlx::query_as(select_head!(
            "ORDER BY center_code COLLATE \"C\", data_type"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        let ch = self.carry_children(None).await?;
        heads.into_iter().map(|h| to_domain(h, &ch)).collect()
    }

    async fn get_carry_setting(
        &self,
        center: &GroupCenterCode,
        data_type: DataType,
    ) -> Result<Option<CarrySetting>, RepoError> {
        let head: Option<HeadRow> =
            sqlx::query_as(select_head!("WHERE center_code = $1 AND data_type = $2"))
                .bind(center.as_str())
                .bind(data_type.code())
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        let Some(head) = head else { return Ok(None) };
        let ch = self
            .carry_children(Some((center.as_str(), data_type.code())))
            .await?;
        to_domain(head, &ch).map(Some)
    }

    async fn insert_carry_setting(&self, s: &CarrySetting) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query(
            "INSERT INTO carry_setting (center_code, data_type, mode, created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(s.center.as_str())
        .bind(s.data_type.code())
        .bind(s.mode.code())
        .bind(&s.created_by)
        .bind(&s.created_dept)
        .bind(s.created_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sync_children(&mut tx, s).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_carry_setting(&self, s: &CarrySetting) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        // 版本先鎖定比對（樂觀鎖，ADR 0012 §4），不符時不寫單身
        let current: Option<i32> = sqlx::query_scalar(
            "SELECT version FROM carry_setting WHERE center_code = $1 AND data_type = $2 FOR UPDATE",
        )
        .bind(s.center.as_str())
        .bind(s.data_type.code())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        if current != Some(s.stamp.version) {
            return Err(RepoError::Stale);
        }
        // 先寫單身、再更新單頭（異動記錄順序與記憶體後端一致）
        sync_children(&mut tx, s).await?;
        sqlx::query(
            "UPDATE carry_setting SET mode = $3, updated_by = $4, updated_on = $5
              WHERE center_code = $1 AND data_type = $2",
        )
        .bind(s.center.as_str())
        .bind(s.data_type.code())
        .bind(s.mode.code())
        .bind(&s.updated_by)
        .bind(s.updated_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete_carry_setting(
        &self,
        center: &GroupCenterCode,
        data_type: DataType,
    ) -> Result<bool, RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        // 先子後母（異動記錄順序與記憶體後端一致）
        for sql in [
            "DELETE FROM carry_condition WHERE center_code = $1 AND data_type = $2",
            "DELETE FROM carry_default WHERE center_code = $1 AND data_type = $2",
            "DELETE FROM carry_target WHERE center_code = $1 AND data_type = $2",
        ] {
            sqlx::query(sql)
                .bind(center.as_str())
                .bind(data_type.code())
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        let n = sqlx::query("DELETE FROM carry_setting WHERE center_code = $1 AND data_type = $2")
            .bind(center.as_str())
            .bind(data_type.code())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}
