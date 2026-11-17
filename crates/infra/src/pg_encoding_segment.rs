//! 編碼獨立段（encoding_segment ← gej_file、encoding_segment_code ← gek_file；encoding_segment.feature）。共用 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::EncodingSegmentRepository};
use ttrs_domain::{
    encoding_segment::{EncodingSegment, SegmentAttrs, SegmentCode, SegmentValue},
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct HeadRow {
    segment_code: String,
    name: Option<String>,
    length: i16,
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
struct ValueRow {
    segment_code: String,
    line_no: i32,
    code_value: String,
    name: Option<String>,
}

macro_rules! select_head {
    ($rest:literal) => {
        concat!(
            "SELECT segment_code, name, length, active, confirmed, confirmed_by, confirmed_at, created_by, created_dept, ",
            "created_on, updated_by, updated_on, version, created_at, updated_at FROM encoding_segment ",
            $rest
        )
    };
}

fn to_domain(h: HeadRow, values: &[ValueRow]) -> Result<EncodingSegment, RepoError> {
    let bad = || RepoError::Other(format!("獨立段資料不正確：{}", h.segment_code));
    Ok(EncodingSegment {
        code: SegmentCode::parse(&h.segment_code).map_err(|_| bad())?,
        attrs: SegmentAttrs {
            name: h.name.clone(),
            length: h.length,
        },
        values: values
            .iter()
            .filter(|v| v.segment_code == h.segment_code)
            .map(|v| SegmentValue {
                line: v.line_no,
                value: v.code_value.clone(),
                name: v.name.clone(),
            })
            .collect(),
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
impl EncodingSegmentRepository for PgStore {
    async fn list_encoding_segments(&self) -> Result<Vec<EncodingSegment>, RepoError> {
        let heads: Vec<HeadRow> =
            sqlx::query_as(select_head!("ORDER BY segment_code COLLATE \"C\""))
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;
        let values: Vec<ValueRow> = sqlx::query_as(
            "SELECT segment_code, line_no, code_value, name FROM encoding_segment_code
              ORDER BY segment_code, line_no",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        heads.into_iter().map(|h| to_domain(h, &values)).collect()
    }

    async fn get_encoding_segment(
        &self,
        code: &SegmentCode,
    ) -> Result<Option<EncodingSegment>, RepoError> {
        let head: Option<HeadRow> = sqlx::query_as(select_head!("WHERE segment_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        let Some(head) = head else { return Ok(None) };
        let values: Vec<ValueRow> = sqlx::query_as(
            "SELECT segment_code, line_no, code_value, name FROM encoding_segment_code
              WHERE segment_code = $1 ORDER BY line_no",
        )
        .bind(code.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        to_domain(head, &values).map(Some)
    }

    async fn insert_encoding_segment(&self, s: &EncodingSegment) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query(
            "INSERT INTO encoding_segment (segment_code, name, length, active, confirmed, confirmed_by, confirmed_at,
                 created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(s.code.as_str())
        .bind(&s.attrs.name)
        .bind(s.attrs.length)
        .bind(s.active)
        .bind(s.confirmation.confirmed)
        .bind(&s.confirmation.by)
        .bind(s.confirmation.at)
        .bind(&s.created_by)
        .bind(&s.created_dept)
        .bind(s.created_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        for v in &s.values {
            sqlx::query(
                "INSERT INTO encoding_segment_code (segment_code, line_no, code_value, name)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(s.code.as_str())
            .bind(v.line)
            .bind(&v.value)
            .bind(&v.name)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn update_encoding_segment(&self, s: &EncodingSegment) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let n = sqlx::query(
            "UPDATE encoding_segment SET name = $2, length = $3, active = $4, confirmed = $5, confirmed_by = $6,
                    confirmed_at = $7, updated_by = $8, updated_on = $9
              WHERE segment_code = $1 AND version = $10",
        )
        .bind(s.code.as_str())
        .bind(&s.attrs.name)
        .bind(s.attrs.length)
        .bind(s.active)
        .bind(s.confirmation.confirmed)
        .bind(&s.confirmation.by)
        .bind(s.confirmation.at)
        .bind(&s.updated_by)
        .bind(s.updated_on)
        .bind(s.stamp.version)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        // 單身只寫入有變動的列（異動記錄只記變動的列，與記憶體後端相同）
        let lines: Vec<i32> = s.values.iter().map(|v| v.line).collect();
        sqlx::query(
            "DELETE FROM encoding_segment_code WHERE segment_code = $1 AND NOT (line_no = ANY($2))",
        )
        .bind(s.code.as_str())
        .bind(&lines)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        for v in &s.values {
            sqlx::query(
                "INSERT INTO encoding_segment_code (segment_code, line_no, code_value, name)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (segment_code, line_no) DO UPDATE
                    SET code_value = EXCLUDED.code_value, name = EXCLUDED.name
                  WHERE (encoding_segment_code.code_value, encoding_segment_code.name)
                        IS DISTINCT FROM (EXCLUDED.code_value, EXCLUDED.name)",
            )
            .bind(s.code.as_str())
            .bind(v.line)
            .bind(&v.value)
            .bind(&v.name)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn encoding_segment_in_use(&self, code: &SegmentCode) -> Result<bool, RepoError> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM encoding_class_segment WHERE independent_segment = $1)",
        )
        .bind(code.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)
    }

    async fn delete_encoding_segment(&self, code: &SegmentCode) -> Result<bool, RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query("DELETE FROM encoding_segment_code WHERE segment_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        // 被編碼類別引用時由外鍵擋下、db_err 轉成 InUse（IS6；aooi402 移植時加外鍵）
        let n = sqlx::query("DELETE FROM encoding_segment WHERE segment_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}
