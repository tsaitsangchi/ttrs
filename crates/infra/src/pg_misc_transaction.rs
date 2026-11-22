//! 雜項發料／收料／報廢（ina_file、inb_file → misc_issue、misc_receipt、misc_scrap 與各自的 _line；
//! misc_issue／misc_receipt／misc_scrap.feature）與庫存明細（img_file → inventory_balance）。工廠層。
//!
//! 三支作業各自一組資料表（MT2）。sqlx 只接受字面 SQL，所以以 [`sql!`] 為每個單據類別展開各自的字面字串。
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, Postgres, Transaction};
use ttrs_app::{
    error::RepoError,
    ports::{
        InventoryBalanceRepository, InventoryPostingRepository, InventoryTransactionRepository,
        PostedDoc, PostingWrite,
    },
};
use ttrs_domain::{
    department::DeptCode,
    inventory_balance::{BalanceKey, InventoryBalance, InventoryTxnRecord},
    inventory_transaction::{InventoryTransaction, TxnHeader, TxnKind, TxnLine, TxnStatus},
    plant::PlantId,
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

/// 依單據類別展開字面 SQL：`H` 換成單頭表、`L` 換成單身表，其餘為字面字串
macro_rules! sql {
    (@p $t:literal, H) => { $t };
    (@p $t:literal, L) => { concat!($t, "_line") };
    (@p $t:literal, $s:literal) => { $s };
    ($kind:expr, $($part:tt),+ $(,)?) => {
        match $kind {
            TxnKind::Issue => concat!($(sql!(@p "misc_issue", $part)),+),
            TxnKind::Receipt => concat!($(sql!(@p "misc_receipt", $part)),+),
            TxnKind::Scrap => concat!($(sql!(@p "misc_scrap", $part)),+),
        }
    };
}

#[derive(FromRow)]
struct Head {
    doc_no: String,
    input_date: NaiveDate,
    posting_date: NaiveDate,
    dept_code: Option<String>,
    applicant: Option<String>,
    memo: String,
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
    warehouse: String,
    location: String,
    lot: String,
    unit: String,
    qty: Decimal,
    reason: Option<String>,
}

fn bad(e: ttrs_domain::DomainError) -> RepoError {
    RepoError::Other(format!("雜項單資料不正確：{e}"))
}

fn to_domain(kind: TxnKind, h: Head, lines: &[Line]) -> Result<InventoryTransaction, RepoError> {
    let mine = lines
        .iter()
        .filter(|l| l.doc_no == h.doc_no)
        .map(|l| TxnLine {
            line_no: l.line_no,
            item: l.item_no.clone(),
            warehouse: l.warehouse.clone(),
            location: l.location.clone(),
            lot: l.lot.clone(),
            unit: l.unit.clone(),
            qty: l.qty.normalize(),
            reason: l.reason.clone(),
        })
        .collect();
    Ok(InventoryTransaction {
        no: h.doc_no,
        kind,
        header: TxnHeader {
            input_date: h.input_date,
            posting_date: h.posting_date,
            dept: h
                .dept_code
                .as_deref()
                .map(DeptCode::parse)
                .transpose()
                .map_err(bad)?,
            applicant: h.applicant,
            memo: Some(h.memo),
        },
        lines: mine,
        status: TxnStatus::from_code(&h.status)
            .ok_or_else(|| RepoError::Other(format!("雜項單狀態不正確：{}", h.status)))?,
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
    kind: TxnKind,
    no: Option<&str>,
) -> Result<Vec<Line>, RepoError> {
    sqlx::query_as(sql!(
        kind,
        "SELECT doc_no, line_no, item_no, warehouse, location, lot, unit, qty, reason FROM ",
        L,
        " WHERE $1::text IS NULL OR doc_no = $1 ORDER BY doc_no COLLATE \"C\", line_no"
    ))
    .bind(no)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_err)
}

/// 單身同步：刪除已不在的項次、新增新的項次、只更新內容有變的項次（觸發器只記有變動的列）
async fn sync_lines(
    tx: &mut Transaction<'static, Postgres>,
    t: &InventoryTransaction,
) -> Result<(), RepoError> {
    let nos: Vec<i32> = t.lines.iter().map(|l| l.line_no).collect();
    sqlx::query(sql!(
        t.kind,
        "DELETE FROM ",
        L,
        " WHERE doc_no = $1 AND NOT (line_no = ANY($2))"
    ))
    .bind(&t.no)
    .bind(&nos)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    for l in &t.lines {
        sqlx::query(sql!(
            t.kind,
            "INSERT INTO ",
            L,
            " AS x (doc_no, line_no, item_no, warehouse, location, lot, unit, qty, reason)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (doc_no, line_no) DO UPDATE SET item_no = EXCLUDED.item_no, warehouse = EXCLUDED.warehouse,
                    location = EXCLUDED.location, lot = EXCLUDED.lot, unit = EXCLUDED.unit, qty = EXCLUDED.qty,
                    reason = EXCLUDED.reason
              WHERE (x.item_no, x.warehouse, x.location, x.lot, x.unit, x.qty, x.reason)
                    IS DISTINCT FROM (EXCLUDED.item_no, EXCLUDED.warehouse, EXCLUDED.location, EXCLUDED.lot,
                                      EXCLUDED.unit, EXCLUDED.qty, EXCLUDED.reason)"
        ))
        .bind(&t.no)
        .bind(l.line_no)
        .bind(&l.item)
        .bind(&l.warehouse)
        .bind(&l.location)
        .bind(&l.lot)
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
impl InventoryTransactionRepository for PgStore {
    async fn list_inventory_transactions(
        &self,
        plant: &PlantId,
        kind: TxnKind,
    ) -> Result<Vec<InventoryTransaction>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let heads: Vec<Head> = sqlx::query_as(sql!(
            kind,
            "SELECT doc_no, input_date, posting_date, dept_code, applicant, memo, status, posted, created_by, created_dept,
                    created_on, updated_by, updated_on, version, created_at, updated_at FROM ",
            H,
            " ORDER BY doc_no COLLATE \"C\""
        ))
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?;
        let lines = load_lines(&mut tx, kind, None).await?;
        tx.commit().await.map_err(db_err)?;
        heads
            .into_iter()
            .map(|h| to_domain(kind, h, &lines))
            .collect()
    }

    async fn get_inventory_transaction(
        &self,
        plant: &PlantId,
        kind: TxnKind,
        no: &str,
    ) -> Result<Option<InventoryTransaction>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let head: Option<Head> = sqlx::query_as(sql!(
            kind,
            "SELECT doc_no, input_date, posting_date, dept_code, applicant, memo, status, posted, created_by, created_dept,
                    created_on, updated_by, updated_on, version, created_at, updated_at FROM ",
            H,
            " WHERE doc_no = $1"
        ))
        .bind(no)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        let lines = load_lines(&mut tx, kind, Some(no)).await?;
        tx.commit().await.map_err(db_err)?;
        head.map(|h| to_domain(kind, h, &lines)).transpose()
    }

    async fn insert_inventory_transaction(
        &self,
        plant: &PlantId,
        t: &InventoryTransaction,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let h = &t.header;
        sqlx::query(sql!(
            t.kind,
            "INSERT INTO ",
            H,
            " (doc_no, input_date, posting_date, dept_code, applicant, memo, status, posted, created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
        ))
        .bind(&t.no)
        .bind(h.input_date)
        .bind(h.posting_date)
        .bind(h.dept.as_ref().map(|d| d.as_str()))
        .bind(&h.applicant)
        .bind(h.memo.as_deref().unwrap_or_default())
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

    async fn update_inventory_transaction(
        &self,
        plant: &PlantId,
        t: &InventoryTransaction,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let h = &t.header;
        let n = sqlx::query(sql!(
            t.kind,
            "UPDATE ",
            H,
            " SET input_date = $2, posting_date = $3, dept_code = $4, applicant = $5, memo = $6, status = $7, posted = $8,
                  updated_by = $9, updated_on = $10
              WHERE doc_no = $1 AND version = $11"
        ))
        .bind(&t.no)
        .bind(h.input_date)
        .bind(h.posting_date)
        .bind(h.dept.as_ref().map(|d| d.as_str()))
        .bind(&h.applicant)
        .bind(h.memo.as_deref().unwrap_or_default())
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

    async fn delete_inventory_transaction(
        &self,
        plant: &PlantId,
        kind: TxnKind,
        no: &str,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        sqlx::query(sql!(kind, "DELETE FROM ", L, " WHERE doc_no = $1"))
            .bind(no)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query(sql!(kind, "DELETE FROM ", H, " WHERE doc_no = $1"))
            .bind(no)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn max_doc_no(
        &self,
        plant: &PlantId,
        kind: TxnKind,
        prefix: &str,
    ) -> Result<Option<String>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let max: Option<String> = sqlx::query_scalar(sql!(
            kind,
            "SELECT MAX(doc_no COLLATE \"C\") FROM ",
            H,
            " WHERE left(doc_no, char_length($1)) = $1"
        ))
        .bind(prefix)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(max)
    }

    async fn pending_issue_qty(
        &self,
        plant: &PlantId,
        key: &BalanceKey,
        exclude_no: &str,
    ) -> Result<Decimal, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Decimal::ZERO);
        };
        let sum: Option<Decimal> = sqlx::query_scalar(
            "SELECT SUM(l.qty) FROM misc_issue h JOIN misc_issue_line l ON l.doc_no = h.doc_no
              WHERE NOT h.posted AND h.status <> 'X' AND h.doc_no <> $5
                AND l.item_no = $1 AND l.warehouse = $2 AND l.location = $3 AND l.lot = $4",
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

    async fn any_inventory_transaction(&self) -> Result<bool, RepoError> {
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
            let any: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM misc_issue) OR EXISTS (SELECT 1 FROM misc_receipt)
                     OR EXISTS (SELECT 1 FROM misc_scrap)",
            )
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

#[derive(FromRow)]
struct BalanceRow {
    item_no: String,
    warehouse: String,
    location: String,
    lot: String,
    unit: String,
    qty: Decimal,
    expiry: Option<NaiveDate>,
    last_received_on: Option<NaiveDate>,
    last_issued_on: Option<NaiveDate>,
    last_txn_on: Option<NaiveDate>,
    usable: bool,
    mrp_usable: bool,
    bonded: bool,
    idle_on: Option<NaiveDate>,
    ref_no: Option<String>,
    ref_line: Option<i32>,
}

impl From<BalanceRow> for InventoryBalance {
    fn from(r: BalanceRow) -> Self {
        InventoryBalance {
            key: BalanceKey::new(&r.item_no, &r.warehouse, &r.location, &r.lot),
            unit: r.unit,
            qty: r.qty.normalize(),
            expiry: r.expiry,
            last_received_on: r.last_received_on,
            last_issued_on: r.last_issued_on,
            last_txn_on: r.last_txn_on,
            usable: r.usable,
            mrp_usable: r.mrp_usable,
            bonded: r.bonded,
            idle_on: r.idle_on,
            ref_no: r.ref_no,
            ref_line: r.ref_line,
        }
    }
}

macro_rules! select_balance {
    ($rest:literal) => {
        concat!(
            "SELECT item_no, warehouse, location, lot, unit, qty, expiry, last_received_on, last_issued_on, last_txn_on, ",
            "usable, mrp_usable, bonded, idle_on, ref_no, ref_line FROM inventory_balance ",
            $rest
        )
    };
}

/// 新增或取代一筆庫存明細（交易內）
async fn upsert_balance(
    tx: &mut Transaction<'static, Postgres>,
    b: &InventoryBalance,
) -> Result<(), RepoError> {
    sqlx::query(
        "INSERT INTO inventory_balance (item_no, warehouse, location, lot, unit, qty, expiry, last_received_on,
                last_issued_on, last_txn_on, usable, mrp_usable, bonded, idle_on, ref_no, ref_line)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
         ON CONFLICT (item_no, warehouse, location, lot)
         DO UPDATE SET unit = EXCLUDED.unit, qty = EXCLUDED.qty, expiry = EXCLUDED.expiry,
                last_received_on = EXCLUDED.last_received_on, last_issued_on = EXCLUDED.last_issued_on,
                last_txn_on = EXCLUDED.last_txn_on, usable = EXCLUDED.usable, mrp_usable = EXCLUDED.mrp_usable,
                bonded = EXCLUDED.bonded, idle_on = EXCLUDED.idle_on, ref_no = EXCLUDED.ref_no, ref_line = EXCLUDED.ref_line",
    )
    .bind(&b.key.item)
    .bind(&b.key.warehouse)
    .bind(&b.key.location)
    .bind(&b.key.lot)
    .bind(&b.unit)
    .bind(b.qty)
    .bind(b.expiry)
    .bind(b.last_received_on)
    .bind(b.last_issued_on)
    .bind(b.last_txn_on)
    .bind(b.usable)
    .bind(b.mrp_usable)
    .bind(b.bonded)
    .bind(b.idle_on)
    .bind(&b.ref_no)
    .bind(b.ref_line)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    Ok(())
}

#[async_trait]
impl InventoryBalanceRepository for PgStore {
    async fn get_inventory_balance(
        &self,
        plant: &PlantId,
        key: &BalanceKey,
    ) -> Result<Option<InventoryBalance>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let row: Option<BalanceRow> = sqlx::query_as(select_balance!(
            "WHERE item_no = $1 AND warehouse = $2 AND location = $3 AND lot = $4"
        ))
        .bind(&key.item)
        .bind(&key.warehouse)
        .bind(&key.location)
        .bind(&key.lot)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(row.map(Into::into))
    }

    async fn put_inventory_balance(
        &self,
        plant: &PlantId,
        b: &InventoryBalance,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        upsert_balance(&mut tx, b).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete_inventory_balance(
        &self,
        plant: &PlantId,
        key: &BalanceKey,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        sqlx::query(
            "DELETE FROM inventory_balance WHERE item_no = $1 AND warehouse = $2 AND location = $3 AND lot = $4",
        )
        .bind(&key.item)
        .bind(&key.warehouse)
        .bind(&key.location)
        .bind(&key.lot)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn list_all_inventory_balances(
        &self,
        plant: &PlantId,
    ) -> Result<Vec<InventoryBalance>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<BalanceRow> = sqlx::query_as(select_balance!(
            "ORDER BY item_no COLLATE \"C\", warehouse COLLATE \"C\", location COLLATE \"C\", lot COLLATE \"C\""
        ))
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_inventory_balances(
        &self,
        plant: &PlantId,
        item: &str,
        warehouse: &str,
        location: &str,
    ) -> Result<Vec<InventoryBalance>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<BalanceRow> = sqlx::query_as(select_balance!(
            "WHERE item_no = $1 AND warehouse = $2 AND location = $3 ORDER BY lot COLLATE \"C\""
        ))
        .bind(item)
        .bind(warehouse)
        .bind(location)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[derive(FromRow)]
struct TxnRow {
    item_no: String,
    warehouse: String,
    location: String,
    lot: String,
    doc_no: String,
    doc_line: i32,
    doc_type: String,
    direction: i16,
    source: Option<i16>,
    dest: Option<i16>,
    txn_date: NaiveDate,
    qty: Decimal,
    unit_code: String,
    balance_after: Option<Decimal>,
    program: String,
    reason: Option<String>,
    memo: Option<String>,
    dept_code: Option<String>,
    cost_group: Option<String>,
    created_by: String,
    created_at: DateTime<Utc>,
}

fn txn_record(r: TxnRow) -> InventoryTxnRecord {
    InventoryTxnRecord {
        key: BalanceKey::new(&r.item_no, &r.warehouse, &r.location, &r.lot),
        doc_no: r.doc_no,
        doc_line: r.doc_line,
        doc_type: r.doc_type,
        direction: r.direction,
        source: r.source.unwrap_or_default(),
        dest: r.dest.unwrap_or_default(),
        txn_date: r.txn_date,
        qty: r.qty.normalize(),
        unit: r.unit_code,
        balance_after: r.balance_after.unwrap_or_default().normalize(),
        program: r.program,
        reason: r.reason,
        memo: r.memo,
        dept: r.dept_code,
        cost_group: r.cost_group,
        created_by: r.created_by,
        created_at: Some(r.created_at),
    }
}

#[async_trait]
impl InventoryPostingRepository for PgStore {
    async fn apply_posting(&self, plant: &PlantId, w: &PostingWrite) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let n = match &w.doc {
            PostedDoc::Misc(t) => sqlx::query(sql!(
                t.kind,
                "UPDATE ",
                H,
                " SET posting_date = $2, posted = $3, updated_by = $4, updated_on = $5 WHERE doc_no = $1 AND version = $6"
            ))
            .bind(&t.no)
            .bind(t.header.posting_date)
            .bind(t.posted)
            .bind(&t.updated_by)
            .bind(t.updated_on)
            .bind(t.stamp.version),
            PostedDoc::Transfer(t) => sqlx::query(
                "UPDATE warehouse_transfer SET transfer_date = $2, posted = $3, updated_by = $4, updated_on = $5
                  WHERE doc_no = $1 AND version = $6",
            )
            .bind(&t.no)
            .bind(t.header.transfer_date)
            .bind(t.posted)
            .bind(&t.updated_by)
            .bind(t.updated_on)
            .bind(t.stamp.version),
        }
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        if n == 0 {
            return Err(RepoError::Stale);
        }
        for (expected, b) in &w.balances {
            // 讀取後庫存量被他人改過：整筆重來（樂觀鎖）
            let cur: Option<Decimal> = sqlx::query_scalar(
                "SELECT qty FROM inventory_balance
                  WHERE item_no = $1 AND warehouse = $2 AND location = $3 AND lot = $4 FOR UPDATE",
            )
            .bind(&b.key.item)
            .bind(&b.key.warehouse)
            .bind(&b.key.location)
            .bind(&b.key.lot)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
            if cur.map(|q| q.normalize()) != expected.map(|q| q.normalize()) {
                return Err(RepoError::Stale);
            }
            upsert_balance(&mut tx, b).await?;
        }
        if w.remove_records {
            sqlx::query("DELETE FROM inventory_txn WHERE program = $1 AND doc_no = $2")
                .bind(w.doc.program())
                .bind(w.doc.no())
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        for r in &w.add_records {
            let dup: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM inventory_txn
                                 WHERE program = $1 AND doc_no = $2 AND doc_line = $3 AND direction = $4)",
            )
            .bind(&r.program)
            .bind(&r.doc_no)
            .bind(r.doc_line)
            .bind(r.direction)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_err)?;
            if dup {
                return Err(RepoError::Conflict);
            }
            sqlx::query(
                "INSERT INTO inventory_txn (item_no, txn_date, doc_no, doc_line, qty, unit_code, program, doc_type, warehouse,
                        location, lot, direction, source, dest, balance_after, reason, memo, dept_code, cost_group, created_by,
                        created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                         COALESCE($21, now()))",
            )
            .bind(&r.key.item)
            .bind(r.txn_date)
            .bind(&r.doc_no)
            .bind(r.doc_line)
            .bind(r.qty)
            .bind(&r.unit)
            .bind(&r.program)
            .bind(&r.doc_type)
            .bind(&r.key.warehouse)
            .bind(&r.key.location)
            .bind(&r.key.lot)
            .bind(r.direction)
            .bind(r.source)
            .bind(r.dest)
            .bind(r.balance_after)
            .bind(&r.reason)
            .bind(&r.memo)
            .bind(&r.dept)
            .bind(&r.cost_group)
            .bind(&r.created_by)
            .bind(r.created_at)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        for d in &w.items {
            sqlx::query(
                "UPDATE item SET last_received_on = COALESCE($2, last_received_on),
                        last_issued_on = COALESCE($3, last_issued_on), last_txn_on = COALESCE($4, last_txn_on)
                  WHERE item_no = $1",
            )
            .bind(&d.item)
            .bind(d.last_received_on)
            .bind(d.last_issued_on)
            .bind(d.last_txn_on)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn inventory_txn_records_since(
        &self,
        plant: &PlantId,
        key: &BalanceKey,
        from: NaiveDate,
    ) -> Result<Vec<InventoryTxnRecord>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<TxnRow> = sqlx::query_as(
            "SELECT item_no, warehouse, location, lot, doc_no, doc_line, doc_type, direction, source, dest, txn_date, qty,
                    unit_code, balance_after, program, reason, memo, dept_code, cost_group, created_by, created_at
               FROM inventory_txn
              WHERE item_no = $1 AND warehouse = $2 AND location = $3 AND lot = $4 AND txn_date >= $5 AND direction <> 0
              ORDER BY txn_date, created_at, txn_id",
        )
        .bind(&key.item)
        .bind(&key.warehouse)
        .bind(&key.location)
        .bind(&key.lot)
        .bind(from)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(rows.into_iter().map(txn_record).collect())
    }

    async fn inventory_txn_records(
        &self,
        plant: &PlantId,
        program: &str,
        doc_no: &str,
    ) -> Result<Vec<InventoryTxnRecord>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<TxnRow> = sqlx::query_as(
            "SELECT item_no, warehouse, location, lot, doc_no, doc_line, doc_type, direction, source, dest, txn_date, qty,
                    unit_code, balance_after, program, reason, memo, dept_code, cost_group, created_by, created_at
               FROM inventory_txn WHERE program = $1 AND doc_no = $2 ORDER BY doc_line, direction",
        )
        .bind(program)
        .bind(doc_no)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(rows.into_iter().map(txn_record).collect())
    }
}
