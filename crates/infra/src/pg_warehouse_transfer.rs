//! 庫存直接調撥（imm_file、imn_file → warehouse_transfer、warehouse_transfer_line；warehouse_transfer.feature）。工廠層
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, Postgres, Transaction};
use ttrs_app::{error::RepoError, ports::WarehouseTransferRepository};
use ttrs_domain::{
    department::DeptCode,
    inventory_balance::BalanceKey,
    inventory_transaction::TxnStatus,
    plant::PlantId,
    record::Stamp,
    warehouse_transfer::{TransferHeader, TransferLine, WarehouseTransfer},
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct Head {
    doc_no: String,
    transfer_date: NaiveDate,
    dept_code: Option<String>,
    applicant: Option<String>,
    memo: Option<String>,
    status: String,
    posted: bool,
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
struct Line {
    doc_no: String,
    line_no: i32,
    item_no: String,
    from_warehouse: String,
    from_location: String,
    from_lot: String,
    to_warehouse: String,
    to_location: String,
    to_lot: String,
    unit: String,
    qty: Decimal,
    reason: Option<String>,
}

macro_rules! select_head {
    ($rest:literal) => {
        concat!(
            "SELECT doc_no, transfer_date, dept_code, applicant, memo, status, posted, created_by, created_dept, created_on, ",
            "updated_by, updated_on, version, created_at, updated_at FROM warehouse_transfer ",
            $rest
        )
    };
}

fn bad(e: ttrs_domain::DomainError) -> RepoError {
    RepoError::Other(format!("調撥單資料不正確：{e}"))
}

fn to_domain(h: Head, lines: &[Line]) -> Result<WarehouseTransfer, RepoError> {
    let mine = lines
        .iter()
        .filter(|l| l.doc_no == h.doc_no)
        .map(|l| TransferLine {
            line_no: l.line_no,
            item: l.item_no.clone(),
            from_warehouse: l.from_warehouse.clone(),
            from_location: l.from_location.clone(),
            from_lot: l.from_lot.clone(),
            to_warehouse: l.to_warehouse.clone(),
            to_location: l.to_location.clone(),
            to_lot: l.to_lot.clone(),
            unit: l.unit.clone(),
            qty: l.qty.normalize(),
            reason: l.reason.clone(),
        })
        .collect();
    Ok(WarehouseTransfer {
        no: h.doc_no,
        header: TransferHeader {
            transfer_date: h.transfer_date,
            dept: h
                .dept_code
                .as_deref()
                .map(DeptCode::parse)
                .transpose()
                .map_err(bad)?,
            applicant: h.applicant,
            memo: h.memo,
        },
        lines: mine,
        status: TxnStatus::from_code(&h.status)
            .ok_or_else(|| RepoError::Other(format!("調撥單狀態不正確：{}", h.status)))?,
        posted: h.posted,
        created_by: h.created_by,
        created_dept: h.created_dept,
        created_on: h.created_on,
        updated_by: h.updated_by,
        updated_on: h.updated_on,
        stamp: Stamp {
            version: h.version,
            created_at: Some(h.created_at),
            updated_at: h.updated_at,
        },
    })
}

async fn load_lines(
    tx: &mut Transaction<'static, Postgres>,
    no: Option<&str>,
) -> Result<Vec<Line>, RepoError> {
    sqlx::query_as(
        "SELECT doc_no, line_no, item_no, from_warehouse, from_location, from_lot, to_warehouse, to_location, to_lot,
                unit, qty, reason
           FROM warehouse_transfer_line WHERE $1::text IS NULL OR doc_no = $1
          ORDER BY doc_no COLLATE \"C\", line_no",
    )
    .bind(no)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_err)
}

/// 單身同步：刪除已不在的項次、新增新的、只更新內容有變的（觸發器只記有變動的列）
async fn sync_lines(
    tx: &mut Transaction<'static, Postgres>,
    t: &WarehouseTransfer,
) -> Result<(), RepoError> {
    let nos: Vec<i32> = t.lines.iter().map(|l| l.line_no).collect();
    sqlx::query(
        "DELETE FROM warehouse_transfer_line WHERE doc_no = $1 AND NOT (line_no = ANY($2))",
    )
    .bind(&t.no)
    .bind(&nos)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    for l in &t.lines {
        sqlx::query(
            "INSERT INTO warehouse_transfer_line AS x (doc_no, line_no, item_no, from_warehouse, from_location, from_lot,
                    to_warehouse, to_location, to_lot, unit, qty, reason)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
             ON CONFLICT (doc_no, line_no) DO UPDATE SET item_no = EXCLUDED.item_no,
                    from_warehouse = EXCLUDED.from_warehouse, from_location = EXCLUDED.from_location,
                    from_lot = EXCLUDED.from_lot, to_warehouse = EXCLUDED.to_warehouse,
                    to_location = EXCLUDED.to_location, to_lot = EXCLUDED.to_lot, unit = EXCLUDED.unit,
                    qty = EXCLUDED.qty, reason = EXCLUDED.reason
              WHERE (x.item_no, x.from_warehouse, x.from_location, x.from_lot, x.to_warehouse, x.to_location, x.to_lot,
                     x.unit, x.qty, x.reason)
                    IS DISTINCT FROM (EXCLUDED.item_no, EXCLUDED.from_warehouse, EXCLUDED.from_location,
                     EXCLUDED.from_lot, EXCLUDED.to_warehouse, EXCLUDED.to_location, EXCLUDED.to_lot, EXCLUDED.unit,
                     EXCLUDED.qty, EXCLUDED.reason)",
        )
        .bind(&t.no)
        .bind(l.line_no)
        .bind(&l.item)
        .bind(&l.from_warehouse)
        .bind(&l.from_location)
        .bind(&l.from_lot)
        .bind(&l.to_warehouse)
        .bind(&l.to_location)
        .bind(&l.to_lot)
        .bind(&l.unit)
        .bind(l.qty)
        .bind(&l.reason)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
    }
    Ok(())
}

#[async_trait]
impl WarehouseTransferRepository for PgStore {
    async fn list_warehouse_transfers(
        &self,
        plant: &PlantId,
    ) -> Result<Vec<WarehouseTransfer>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let heads: Vec<Head> = sqlx::query_as(select_head!("ORDER BY doc_no COLLATE \"C\""))
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?;
        let lines = load_lines(&mut tx, None).await?;
        tx.commit().await.map_err(db_err)?;
        heads.into_iter().map(|h| to_domain(h, &lines)).collect()
    }

    async fn get_warehouse_transfer(
        &self,
        plant: &PlantId,
        no: &str,
    ) -> Result<Option<WarehouseTransfer>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let head: Option<Head> = sqlx::query_as(select_head!("WHERE doc_no = $1"))
            .bind(no)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
        let lines = load_lines(&mut tx, Some(no)).await?;
        tx.commit().await.map_err(db_err)?;
        head.map(|h| to_domain(h, &lines)).transpose()
    }

    async fn insert_warehouse_transfer(
        &self,
        plant: &PlantId,
        t: &WarehouseTransfer,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let h = &t.header;
        sqlx::query(
            "INSERT INTO warehouse_transfer (doc_no, transfer_date, dept_code, applicant, memo, status, posted, created_by,
                    created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(&t.no)
        .bind(h.transfer_date)
        .bind(h.dept.as_ref().map(|d| d.as_str()))
        .bind(&h.applicant)
        .bind(&h.memo)
        .bind(t.status.code())
        .bind(t.posted)
        .bind(&t.created_by)
        .bind(&t.created_dept)
        .bind(t.created_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sync_lines(&mut tx, t).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_warehouse_transfer(
        &self,
        plant: &PlantId,
        t: &WarehouseTransfer,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let h = &t.header;
        let n = sqlx::query(
            "UPDATE warehouse_transfer SET transfer_date = $2, dept_code = $3, applicant = $4, memo = $5, status = $6,
                    posted = $7, updated_by = $8, updated_on = $9
              WHERE doc_no = $1 AND version = $10",
        )
        .bind(&t.no)
        .bind(h.transfer_date)
        .bind(h.dept.as_ref().map(|d| d.as_str()))
        .bind(&h.applicant)
        .bind(&h.memo)
        .bind(t.status.code())
        .bind(t.posted)
        .bind(&t.updated_by)
        .bind(t.updated_on)
        .bind(t.stamp.version)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        sync_lines(&mut tx, t).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete_warehouse_transfer(&self, plant: &PlantId, no: &str) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        for sql in [
            "DELETE FROM warehouse_transfer_line WHERE doc_no = $1",
            "DELETE FROM warehouse_transfer WHERE doc_no = $1",
        ] {
            sqlx::query(sql)
                .bind(no)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn max_transfer_no(
        &self,
        plant: &PlantId,
        prefix: &str,
    ) -> Result<Option<String>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let max: Option<String> = sqlx::query_scalar(
            "SELECT MAX(doc_no COLLATE \"C\") FROM warehouse_transfer WHERE left(doc_no, char_length($1)) = $1",
        )
        .bind(prefix)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(max)
    }

    async fn pending_transfer_qty(
        &self,
        plant: &PlantId,
        key: &BalanceKey,
        exclude_no: &str,
    ) -> Result<Decimal, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Decimal::ZERO);
        };
        let sum: Option<Decimal> = sqlx::query_scalar(
            "SELECT SUM(l.qty) FROM warehouse_transfer h JOIN warehouse_transfer_line l ON l.doc_no = h.doc_no
              WHERE NOT h.posted AND h.status <> 'X' AND h.doc_no <> $5
                AND l.item_no = $1 AND l.from_warehouse = $2 AND l.from_location = $3 AND l.from_lot = $4",
        )
        .bind(&key.item)
        .bind(&key.warehouse)
        .bind(&key.location)
        .bind(&key.lot)
        .bind(exclude_no)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(sum.unwrap_or_default())
    }
}
