//! 庫存參數（TipTop: asms230 製造管理系統參數設定－庫存料件 / sma_file）
//! 規格：specs/features/aim/inventory_parameters.feature（IP1–IP10）
//!
//! 每個營運中心一份（IP2）。sma_file 由多支 asms 參數作業共用，本模組只放 asms230 的 10 個參數；
//! sma12 一律為多倉儲，不移植（IP10）。

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::{DomainError, record::Stamp};

/// 週轉率小數位數（sma68、sma69 number(9,4)）
pub const TURNOVER_SCALE: u32 = 4;

/// 料件存放位置管制（sma42；IP4）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageControl {
    /// 1 使用預設料件／倉庫：只能入預設倉庫，儲位自選
    Warehouse,
    /// 2 使用預設料件／倉庫／儲位：只能入預設倉庫＋儲位
    WarehouseLocation,
    /// 3 輸入時自行決定：不管制
    Free,
}

impl StorageControl {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        match raw.trim() {
            "1" => Ok(Self::Warehouse),
            "2" => Ok(Self::WarehouseLocation),
            "3" => Ok(Self::Free),
            _ => Err(DomainError::InventoryStorageControlInvalid),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Warehouse => "1",
            Self::WarehouseLocation => "2",
            Self::Free => "3",
        }
    }
}

/// 庫存帳務基礎日期（sma892 第 1 碼：1 單據日期、2 產生日期；IP6(a)）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostingDateBasis {
    Document,
    Generated,
}

impl PostingDateBasis {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        match raw.trim() {
            "1" => Ok(Self::Document),
            "2" => Ok(Self::Generated),
            _ => Err(DomainError::InventoryPostingDateBasisInvalid),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Document => "1",
            Self::Generated => "2",
        }
    }
}

/// 週轉率（sma68 快速、sma69 平速）：0–100、最多 4 位小數，快速 ≥ 平速（TQC-790020；IP5）
pub fn check_turnover(fast: Decimal, average: Decimal) -> Result<(), DomainError> {
    let ok = |v: Decimal| {
        v >= Decimal::ZERO && v <= Decimal::ONE_HUNDRED && v.normalize().scale() <= TURNOVER_SCALE
    };
    if !ok(fast) || !ok(average) || fast < average {
        return Err(DomainError::InventoryTurnoverInvalid);
    }
    Ok(())
}

/// 庫存參數的設定值（asms230 畫面上的 10 個參數）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventorySettings {
    /// sma42
    pub storage_control: StorageControl,
    /// sma68
    pub fast_turnover: Decimal,
    /// sma69
    pub average_turnover: Decimal,
    /// sma892 第 1 碼
    pub posting_date_basis: PostingDateBasis,
    /// sma892 第 2 碼：雜收時可新增儲位（意義待確認，IP6(b)）
    pub misc_receipt_new_location: bool,
    /// sma892 第 3 碼：開窗查詢庫存明細（意義待確認，IP6(b)）
    pub stock_query_window: bool,
    /// sma88 兩階段營運中心調撥，撥出單號與撥入單號相同
    pub plant_transfer_same_no: bool,
    /// sma884 兩階段倉庫調撥，撥出單號與撥入單號相同
    pub warehouse_transfer_same_no: bool,
    /// sma847 第 1 碼：雜發單要輸入部門
    pub misc_issue_dept: bool,
    /// sma847 第 2 碼：雜收單要輸入部門
    pub misc_receipt_dept: bool,
    /// sma39 可不經倉儲維護新增倉儲
    pub add_storage_freely: bool,
}

impl InventorySettings {
    /// 檢查週轉率（IP5）；其他欄位的值域由型別保證
    pub fn validate(&self) -> Result<(), DomainError> {
        check_turnover(self.fast_turnover, self.average_turnover)
    }
}

impl Default for InventorySettings {
    /// 預設值（IP4–IP8）：不管制存放位置、週轉率 0、依單據日期、其餘為否
    fn default() -> Self {
        Self {
            storage_control: StorageControl::Free,
            fast_turnover: Decimal::ZERO,
            average_turnover: Decimal::ZERO,
            posting_date_basis: PostingDateBasis::Document,
            misc_receipt_new_location: false,
            stock_query_window: false,
            plant_transfer_same_no: false,
            warehouse_transfer_same_no: false,
            misc_issue_dept: false,
            misc_receipt_dept: false,
            add_storage_freely: false,
        }
    }
}

/// 一個營運中心的庫存參數（manufacturing_parameter 的一筆）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryParameters {
    pub settings: InventorySettings,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl InventoryParameters {
    /// 沒有資料時以預設值建立（IP2）
    pub fn defaults(user: &str, dept: &str, today: NaiveDate) -> Self {
        Self {
            settings: InventorySettings::default(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn update(
        &mut self,
        settings: InventorySettings,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        settings.validate()?;
        self.settings = settings;
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(v: &str) -> Decimal {
        v.parse().unwrap()
    }

    #[test]
    fn turnover_rules() {
        assert!(check_turnover(d("80"), d("40.5")).is_ok());
        assert!(check_turnover(d("100"), d("100")).is_ok());
        for (f, a) in [("101", "0"), ("50", "-1"), ("30", "40"), ("1.23456", "0")] {
            assert_eq!(
                check_turnover(d(f), d(a)).unwrap_err(),
                DomainError::InventoryTurnoverInvalid
            );
        }
    }

    #[test]
    fn storage_control_codes() {
        assert_eq!(
            StorageControl::parse("2").unwrap(),
            StorageControl::WarehouseLocation
        );
        for bad in ["0", "4", ""] {
            assert_eq!(
                StorageControl::parse(bad).unwrap_err(),
                DomainError::InventoryStorageControlInvalid
            );
        }
    }
}
