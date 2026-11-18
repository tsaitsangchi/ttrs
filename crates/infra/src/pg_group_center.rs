//! 集團中心（group_center ← geu_file、data_center_type ← gev_file 中 gev03＝Y 的列；group_center.feature）。共用 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::GroupCenterRepository};
use ttrs_domain::{
    group_center::{CenterCategory, DataCenterType, DataType, GroupCenter, GroupCenterCode},
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct HeadRow {
    center_code: String,
    name: Option<String>,
    category: String,
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

#[derive(FromRow)]
struct TypeRow {
    center_code: String,
    data_type: String,
    plant_id: String,
}

macro_rules! select_head {
    ($rest:literal) => {
        concat!(
            "SELECT center_code, name, category, active, confirmed, confirmed_by, confirmed_at, created_by, created_dept, ",
            "created_on, updated_by, updated_on, version, created_at, updated_at FROM group_center ",
            $rest
        )
    };
}

fn to_domain(h: HeadRow, types: &[TypeRow]) -> Result<GroupCenter, RepoError> {
    let bad = || RepoError::Other(format!("集團中心資料不正確：{}", h.center_code));
    let mut data_types = types
        .iter()
        .filter(|t| t.center_code == h.center_code)
        .map(|t| {
            Ok(DataCenterType {
                data_type: DataType::parse(&t.data_type).map_err(|_| bad())?,
                plant: t.plant_id.clone(),
            })
        })
        .collect::<Result<Vec<_>, RepoError>>()?;
    data_types.sort();
    Ok(GroupCenter {
        code: GroupCenterCode::parse(&h.center_code).map_err(|_| bad())?,
        name: h.name.clone(),
        category: CenterCategory::parse(&h.category).map_err(|_| bad())?,
        data_types,
        active: h.active,
        confirmation: ttrs_domain::confirmation::Confirmation {
            confirmed: h.confirmed,
            by: h.confirmed_by.clone(),
            at: h.confirmed_at,
        },
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

#[async_trait]
impl GroupCenterRepository for PgStore {
    async fn list_group_centers(&self) -> Result<Vec<GroupCenter>, RepoError> {
        let heads: Vec<HeadRow> =
            sqlx::query_as(select_head!("ORDER BY center_code COLLATE \"C\""))
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;
        let types: Vec<TypeRow> =
            sqlx::query_as("SELECT center_code, data_type, plant_id FROM data_center_type")
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;
        heads.into_iter().map(|h| to_domain(h, &types)).collect()
    }

    async fn get_group_center(
        &self,
        code: &GroupCenterCode,
    ) -> Result<Option<GroupCenter>, RepoError> {
        let head: Option<HeadRow> = sqlx::query_as(select_head!("WHERE center_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        let Some(head) = head else { return Ok(None) };
        let types: Vec<TypeRow> = sqlx::query_as(
            "SELECT center_code, data_type, plant_id FROM data_center_type WHERE center_code = $1",
        )
        .bind(code.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        to_domain(head, &types).map(Some)
    }

    async fn insert_group_center(&self, c: &GroupCenter) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO group_center (center_code, name, category, active, confirmed, confirmed_by, confirmed_at,
                 created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(c.code.as_str())
        .bind(&c.name)
        .bind(c.category.code())
        .bind(c.active)
        .bind(c.confirmation.confirmed)
        .bind(&c.confirmation.by)
        .bind(c.confirmation.at)
        .bind(&c.created_by)
        .bind(&c.created_dept)
        .bind(c.created_on)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_group_center(&self, c: &GroupCenter) -> Result<(), RepoError> {
        let n = sqlx::query(
            "UPDATE group_center SET name = $2, category = $3, active = $4, confirmed = $5, confirmed_by = $6,
                    confirmed_at = $7, updated_by = $8, updated_on = $9
              WHERE center_code = $1 AND version = $10",
        )
        .bind(c.code.as_str())
        .bind(&c.name)
        .bind(c.category.code())
        .bind(c.active)
        .bind(c.confirmation.confirmed)
        .bind(&c.confirmation.by)
        .bind(c.confirmation.at)
        .bind(&c.updated_by)
        .bind(c.updated_on)
        .bind(c.stamp.version)
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

    async fn add_data_center_type(
        &self,
        code: &GroupCenterCode,
        t: &DataCenterType,
    ) -> Result<(), RepoError> {
        // 主鍵（資料類別＋營運中心）重複、或已是受控營運中心（觸發器 ttrs_data_control_exclusive）時 db_err 轉成 Conflict（GC5、DC2）
        sqlx::query(
            "INSERT INTO data_center_type (data_type, plant_id, center_code) VALUES ($1, $2, $3)",
        )
        .bind(t.data_type.code())
        .bind(&t.plant)
        .bind(code.as_str())
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn remove_data_center_type(
        &self,
        code: &GroupCenterCode,
        t: &DataCenterType,
    ) -> Result<bool, RepoError> {
        let n = sqlx::query(
            "DELETE FROM data_center_type WHERE center_code = $1 AND data_type = $2 AND plant_id = $3",
        )
        .bind(code.as_str())
        .bind(t.data_type.code())
        .bind(&t.plant)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        Ok(n > 0)
    }

    async fn delete_group_center(&self, code: &GroupCenterCode) -> Result<bool, RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        // 先刪資料類別設定（與記憶體後端的異動記錄順序一致；外鍵另有 ON DELETE CASCADE）
        sqlx::query("DELETE FROM data_center_type WHERE center_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        let n = sqlx::query("DELETE FROM group_center WHERE center_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}
