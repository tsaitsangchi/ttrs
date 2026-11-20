//! Inventory Balance 庫存明細（TipTop img_file 庫存資料明細檔）與庫存異動記錄（tlf_file）
//! 規格：misc_issue.feature MT8、MT17；misc_issue_posting.feature PT4、PT6、PT14；misc_receipt_posting.feature PR1–PR3。
//!
//! 一筆＝料件＋倉庫＋儲位＋批號的庫存量。由各作業的過帳維護。儲位、批號空白存空字串（TipTop 存一個空白）。

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::DomainError;

/// 庫存明細的鍵值（img01–img04）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BalanceKey {
    pub item: String,
    pub warehouse: String,
    pub location: String,
    pub lot: String,
}

impl BalanceKey {
    pub fn new(item: &str, warehouse: &str, location: &str, lot: &str) -> Self {
        Self {
            item: item.to_owned(),
            warehouse: warehouse.to_owned(),
            location: location.trim().to_owned(),
            lot: lot.trim().to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryBalance {
    pub key: BalanceKey,
    /// img09 庫存單位
    pub unit: String,
    /// img10 庫存數量
    pub qty: Decimal,
    /// img18 有效日期
    pub expiry: Option<NaiveDate>,
    /// img15 最近收料日
    pub last_received_on: Option<NaiveDate>,
    /// img16 最近發料日
    pub last_issued_on: Option<NaiveDate>,
    /// img17 最近異動日
    pub last_txn_on: Option<NaiveDate>,
    /// img23 可用倉儲
    pub usable: bool,
    /// img24 MRP 可用倉儲
    pub mrp_usable: bool,
    /// img25 保稅
    pub bonded: bool,
    /// img37 呆滯日期
    pub idle_on: Option<NaiveDate>,
    /// img05、img06 建立它的單號、項次
    pub ref_no: Option<String>,
    pub ref_line: Option<i32>,
}

impl InventoryBalance {
    /// 只有鍵值、單位、數量的庫存明細（測試資料、尚未有其他資訊時）
    pub fn simple(key: BalanceKey, unit: &str, qty: Decimal) -> Self {
        Self {
            key,
            unit: unit.to_owned(),
            qty,
            expiry: None,
            last_received_on: None,
            last_issued_on: None,
            last_txn_on: None,
            usable: true,
            mrp_usable: true,
            bonded: false,
            idle_on: None,
            ref_no: None,
            ref_line: None,
        }
    }

    /// 出庫（s_upimg p_type -1 雜發、0 報廢；PT4）：扣完不可小於 0（asf-375）；
    /// 雜發改最近發料日，報廢只改最近異動日（PS1）
    pub fn take(&mut self, qty: Decimal, date: NaiveDate, issue: bool) -> Result<(), DomainError> {
        if self.qty < qty {
            return Err(DomainError::InventoryInsufficient);
        }
        self.qty -= qty;
        if issue {
            self.last_issued_on = Some(date);
        }
        self.last_txn_on = Some(date);
        Ok(())
    }

    /// 入庫（s_upimg p_type +1；PR1）：庫存量增加、最近收料日、最近異動日
    pub fn put(&mut self, qty: Decimal, date: NaiveDate) {
        self.qty += qty;
        self.last_received_on = Some(date);
        self.last_txn_on = Some(date);
    }

    /// 過帳還原（aimp379 → s_upimg 反向；PT8、PR3）：只改數量，日期不還原；扣回不可小於 0
    pub fn undo(&mut self, delta: Decimal) -> Result<(), DomainError> {
        let q = self.qty + delta;
        if q < Decimal::ZERO {
            return Err(DomainError::InventoryInsufficient);
        }
        self.qty = q;
        Ok(())
    }
}

/// 雜收到新的倉儲批時新建的庫存明細（s_upimg:884-1000；PR1）
#[allow(clippy::too_many_arguments)]
pub fn new_balance(
    key: BalanceKey,
    unit: &str,
    date: NaiveDate,
    shelf_life_days: Option<i32>,
    usable: bool,
    mrp_usable: bool,
    bonded: bool,
    ref_no: &str,
    ref_line: i32,
) -> InventoryBalance {
    // 儲存有效天數 0 或未設定時為無期限（g_lastdat）
    let expiry = shelf_life_days
        .filter(|d| *d > 0)
        .map(|d| date + chrono::Duration::days(i64::from(d)));
    InventoryBalance {
        key,
        unit: unit.to_owned(),
        qty: Decimal::ZERO,
        expiry,
        last_received_on: Some(date),
        last_issued_on: Some(date),
        last_txn_on: Some(date),
        usable,
        mrp_usable,
        bonded,
        idle_on: Some(date),
        ref_no: Some(ref_no.to_owned()),
        ref_line: Some(ref_line),
    }
}

/// 庫存異動記錄（tlf_file → inventory_txn；PT6）：一筆單身一筆
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryTxnRecord {
    /// tlf01、tlf902–tlf904
    pub key: BalanceKey,
    /// tlf905、tlf906 單號、項次
    pub doc_no: String,
    pub doc_line: i32,
    /// tlf61 單別
    pub doc_type: String,
    /// tlf907 入出庫碼：-1 出庫、+1 入庫
    pub direction: i16,
    /// tlf02、tlf03 來源、目的狀況（50 庫存、90 雜項、40 報廢）
    pub source: i16,
    pub dest: i16,
    /// tlf06 單據日期（扣帳日）
    pub txn_date: NaiveDate,
    /// tlf10、tlf11 異動數量與單位（換算率 tlf12、tlf60 固定 1）
    pub qty: Decimal,
    pub unit: String,
    /// tlf024／tlf034 異動後庫存量
    pub balance_after: Decimal,
    /// tlf13 程式代號（TTRS 程式代號）
    pub program: String,
    /// tlf14、tlf43 理由碼
    pub reason: Option<String>,
    /// tlf17 備註
    pub memo: Option<String>,
    /// tlf19 部門
    pub dept: Option<String>,
    /// tlf901 成本庫別（倉庫 imd09）
    pub cost_group: Option<String>,
    /// tlf09 產生人；tlf07、tlf08 產生時間
    pub created_by: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn take_and_put() {
        let mut b =
            InventoryBalance::simple(BalanceKey::new("A", "W", "", ""), "PCS", Decimal::TEN);
        assert_eq!(
            b.take(Decimal::from(11), d("2026-11-20"), true)
                .unwrap_err()
                .code(),
            "INVENTORY_INSUFFICIENT"
        );
        b.take(Decimal::from(4), d("2026-11-20"), false).unwrap();
        assert_eq!(b.qty, Decimal::from(6));
        assert_eq!(b.last_issued_on, None);
        b.put(Decimal::ONE, d("2026-11-21"));
        assert_eq!(b.qty, Decimal::from(7));
        assert!(b.undo(Decimal::from(-8)).is_err());
    }

    #[test]
    fn new_balance_expiry() {
        let b = new_balance(
            BalanceKey::new("A", "W", "", "B1"),
            "PCS",
            d("2026-11-20"),
            Some(30),
            true,
            true,
            false,
            "MR1-26110001",
            1,
        );
        assert_eq!(b.expiry, Some(d("2026-12-20")));
        assert_eq!(
            new_balance(
                b.key.clone(),
                "PCS",
                d("2026-11-20"),
                Some(0),
                true,
                true,
                false,
                "X",
                1
            )
            .expiry,
            None
        );
    }
}
