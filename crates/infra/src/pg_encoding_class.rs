//! 編碼類別（encoding_class ← gei_file、encoding_class_segment ← gel_file；encoding_class.feature）。共用 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::EncodingClassRepository};
use ttrs_domain::{
    encoding_class::{ClassAttrs, ClassCode, ClassSegment, EncodingClass, SegmentType},
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct HeadRow {
    class_code: String,
    name: String,
    nature_code: String,
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
struct SegRow {
    class_code: String,
    seq_no: i32,
    description: Option<String>,
    length: i16,
    segment_type: String,
    fixed_value: Option<String>,
    independent_segment: Option<String>,
    show_name: bool,
}

macro_rules! select_head {
    ($rest:literal) => {
        concat!(
            "SELECT class_code, name, nature_code, active, confirmed, confirmed_by, confirmed_at, created_by, created_dept, ",
            "created_on, updated_by, updated_on, version, created_at, updated_at FROM encoding_class ",
            $rest
        )
    };
}

const SELECT_SEGMENTS: &str = "SELECT class_code, seq_no, description, length, segment_type, fixed_value, \
     independent_segment, show_name FROM encoding_class_segment";

fn to_domain(h: HeadRow, segs: &[SegRow]) -> Result<EncodingClass, RepoError> {
    let bad = || RepoError::Other(format!("編碼類別資料不正確：{}", h.class_code));
    let segments = segs
        .iter()
        .filter(|s| s.class_code == h.class_code)
        .map(|s| {
            Ok(ClassSegment {
                no: s.seq_no,
                description: s.description.clone(),
                length: s.length,
                kind: SegmentType::parse(&s.segment_type).map_err(|_| bad())?,
                value: s
                    .fixed_value
                    .clone()
                    .or_else(|| s.independent_segment.clone()),
                show_name: s.show_name,
            })
        })
        .collect::<Result<Vec<_>, RepoError>>()?;
    Ok(EncodingClass {
        code: ClassCode::parse(&h.class_code).map_err(|_| bad())?,
        attrs: ClassAttrs {
            name: h.name.clone(),
            nature: h.nature_code.clone(),
        },
        segments,
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

/// 固定值與獨立段分兩欄存放（獨立段有外鍵，EC9）
fn split(s: &ClassSegment) -> (Option<&str>, Option<&str>) {
    match s.kind {
        SegmentType::Independent => (None, s.value.as_deref()),
        _ => (s.value.as_deref(), None),
    }
}

async fn upsert_segment(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    code: &str,
    s: &ClassSegment,
) -> Result<(), RepoError> {
    let (fixed, independent) = split(s);
    sqlx::query(
        "INSERT INTO encoding_class_segment (class_code, seq_no, description, length, segment_type, fixed_value,
             independent_segment, show_name)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (class_code, seq_no) DO UPDATE
            SET description = EXCLUDED.description, length = EXCLUDED.length, segment_type = EXCLUDED.segment_type,
                fixed_value = EXCLUDED.fixed_value, independent_segment = EXCLUDED.independent_segment,
                show_name = EXCLUDED.show_name
          WHERE (encoding_class_segment.description, encoding_class_segment.length, encoding_class_segment.segment_type,
                 encoding_class_segment.fixed_value, encoding_class_segment.independent_segment,
                 encoding_class_segment.show_name)
                IS DISTINCT FROM (EXCLUDED.description, EXCLUDED.length, EXCLUDED.segment_type,
                 EXCLUDED.fixed_value, EXCLUDED.independent_segment, EXCLUDED.show_name)",
    )
    .bind(code)
    .bind(s.no)
    .bind(&s.description)
    .bind(s.length)
    .bind(s.kind.code())
    .bind(fixed)
    .bind(independent)
    .bind(s.show_name)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    Ok(())
}

#[async_trait]
impl EncodingClassRepository for PgStore {
    async fn list_encoding_classes(&self) -> Result<Vec<EncodingClass>, RepoError> {
        let heads: Vec<HeadRow> = sqlx::query_as(select_head!("ORDER BY class_code COLLATE \"C\""))
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        let segs: Vec<SegRow> = sqlx::query_as(SELECT_SEGMENTS)
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        let mut segs = segs;
        segs.sort_by_key(|s| s.seq_no);
        heads.into_iter().map(|h| to_domain(h, &segs)).collect()
    }

    async fn get_encoding_class(
        &self,
        code: &ClassCode,
    ) -> Result<Option<EncodingClass>, RepoError> {
        let head: Option<HeadRow> = sqlx::query_as(select_head!("WHERE class_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        let Some(head) = head else { return Ok(None) };
        let mut segs: Vec<SegRow> = sqlx::query_as(
            "SELECT class_code, seq_no, description, length, segment_type, fixed_value, independent_segment, show_name
               FROM encoding_class_segment WHERE class_code = $1",
        )
        .bind(code.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        segs.sort_by_key(|s| s.seq_no);
        to_domain(head, &segs).map(Some)
    }

    async fn insert_encoding_class(&self, c: &EncodingClass) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query(
            "INSERT INTO encoding_class (class_code, name, nature_code, active, confirmed, confirmed_by, confirmed_at,
                 created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(c.code.as_str())
        .bind(&c.attrs.name)
        .bind(&c.attrs.nature)
        .bind(c.active)
        .bind(c.confirmation.confirmed)
        .bind(&c.confirmation.by)
        .bind(c.confirmation.at)
        .bind(&c.created_by)
        .bind(&c.created_dept)
        .bind(c.created_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        for s in &c.segments {
            upsert_segment(&mut tx, c.code.as_str(), s).await?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn update_encoding_class(&self, c: &EncodingClass) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let n = sqlx::query(
            "UPDATE encoding_class SET name = $2, nature_code = $3, active = $4, confirmed = $5, confirmed_by = $6,
                    confirmed_at = $7, updated_by = $8, updated_on = $9
              WHERE class_code = $1 AND version = $10",
        )
        .bind(c.code.as_str())
        .bind(&c.attrs.name)
        .bind(&c.attrs.nature)
        .bind(c.active)
        .bind(c.confirmation.confirmed)
        .bind(&c.confirmation.by)
        .bind(c.confirmation.at)
        .bind(&c.updated_by)
        .bind(c.updated_on)
        .bind(c.stamp.version)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        // 單身只寫入有變動的段（異動記錄只記變動的段，與記憶體後端相同）
        let nos: Vec<i32> = c.segments.iter().map(|s| s.no).collect();
        sqlx::query(
            "DELETE FROM encoding_class_segment WHERE class_code = $1 AND NOT (seq_no = ANY($2))",
        )
        .bind(c.code.as_str())
        .bind(&nos)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        for s in &c.segments {
            upsert_segment(&mut tx, c.code.as_str(), s).await?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn delete_encoding_class(&self, code: &ClassCode) -> Result<bool, RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query("DELETE FROM encoding_class_segment WHERE class_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        // 已有編碼規則（aooi400）時由外鍵擋下、db_err 轉成 InUse（EC8；aooi400 移植時加外鍵）
        let n = sqlx::query("DELETE FROM encoding_class WHERE class_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}
