//! Warehouse Transfer 庫存直接調撥（TipTop aimt324 / imm_file、imn_file → warehouse_transfer、warehouse_transfer_line；
//! specs/features/aim/warehouse_transfer.feature）
//!
//! 一階段倉庫調撥：同一營運中心內，把料件從撥出倉儲批直接搬到撥入倉儲批。流程同雜項單：開立 → 確認 → 過帳。
//! 需要查其他資料（料件、倉庫、庫存、單別）的檢查在 use case。

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::{
    DomainError, department::DeptCode, inventory_balance::BalanceKey,
    inventory_transaction::TxnStatus, record::Stamp,
};

pub const MEMO_MAX: usize = 255;
/// 單據性質（aim 4 調撥；s_check_no "4"）
pub const DOC_KIND: &str = "4";
/// TTRS 程式代號（WT1）
pub const PROGRAM: &str = "warehouse-transfer";

/// 單頭可修改的欄位
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferHeader {
    /// imm02 調撥日（編號、期間檢查、過帳都用它，WT2、WT3(a)）
    pub transfer_date: NaiveDate,
    /// imm14 部門（不必填，WT3(c)）
    pub dept: Option<DeptCode>,
    /// imm16 申請人（員工代號）
    pub applicant: Option<String>,
    /// imm09 備註（不必填，WT3(d)）
    pub memo: Option<String>,
}

impl TransferHeader {
    pub fn validated(mut self) -> Result<Self, DomainError> {
        self.memo = self
            .memo
            .map(|m| m.trim().to_owned())
            .filter(|m| !m.is_empty());
        if self
            .memo
            .as_ref()
            .is_some_and(|m| m.chars().count() > MEMO_MAX)
        {
            return Err(DomainError::InventoryTransactionMemoTooLong);
        }
        self.applicant = self
            .applicant
            .map(|a| a.trim().to_owned())
            .filter(|a| !a.is_empty());
        Ok(self)
    }
}

/// 單身（imn_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferLine {
    /// imn02 項次
    pub line_no: i32,
    /// imn03 料號
    pub item: String,
    /// imn04、imn05、imn06 撥出倉庫、儲位、批號
    pub from_warehouse: String,
    pub from_location: String,
    pub from_lot: String,
    /// imn15、imn16、imn17 撥入倉庫、儲位、批號
    pub to_warehouse: String,
    pub to_location: String,
    pub to_lot: String,
    /// imn09 庫存單位（撥入單位 imn20 相同，WT6）
    pub unit: String,
    /// imn10 調撥數量（轉換後數量 imn22 相同）
    pub qty: Decimal,
    /// imn28 調撥理由
    pub reason: Option<String>,
}

impl TransferLine {
    pub fn from_key(&self) -> BalanceKey {
        BalanceKey::new(
            &self.item,
            &self.from_warehouse,
            &self.from_location,
            &self.from_lot,
        )
    }

    pub fn to_key(&self) -> BalanceKey {
        BalanceKey::new(
            &self.item,
            &self.to_warehouse,
            &self.to_location,
            &self.to_lot,
        )
    }

    /// 撥出與撥入的倉庫＋儲位＋批號不可完全相同（mfg9090、mfg6103；WT5(c)）
    pub fn check_storages(&self) -> Result<(), DomainError> {
        if self.from_key() == self.to_key() {
            return Err(DomainError::WarehouseTransferSameStorage);
        }
        Ok(())
    }
}

/// 單身整體：項次大於 0 且不重複（-239）
pub fn check_lines(lines: &[TransferLine]) -> Result<(), DomainError> {
    let mut seen = std::collections::BTreeSet::new();
    for l in lines {
        if l.line_no <= 0 {
            return Err(DomainError::InventoryTransactionLineNoInvalid);
        }
        if !seen.insert(l.line_no) {
            return Err(DomainError::InventoryTransactionLineDuplicate(l.line_no));
        }
        l.check_storages()?;
    }
    Ok(())
}

/// 調撥單（imm_file＋imn_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarehouseTransfer {
    /// imm01 調撥單號
    pub no: String,
    pub header: TransferHeader,
    pub lines: Vec<TransferLine>,
    /// immconf 確認碼（N／Y／X）
    pub status: TxnStatus,
    /// imm03 過帳否
    pub posted: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl WarehouseTransfer {
    pub fn new(
        no: String,
        header: TransferHeader,
        lines: Vec<TransferLine>,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        check_lines(&lines)?;
        Ok(Self {
            no,
            header: header.validated()?,
            lines,
            status: TxnStatus::Open,
            posted: false,
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        })
    }

    pub fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }

    /// 修改、單身、刪除：已過帳、已確認（9023）、已作廢（9024）不可
    pub fn check_editable(&self) -> Result<(), DomainError> {
        if self.posted {
            return Err(DomainError::InventoryTransactionPosted);
        }
        match self.status {
            TxnStatus::Open => Ok(()),
            TxnStatus::Confirmed => Err(DomainError::InventoryTransactionConfirmed),
            TxnStatus::Voided => Err(DomainError::InventoryTransactionVoided),
        }
    }

    pub fn update(
        &mut self,
        header: TransferHeader,
        lines: Vec<TransferLine>,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_editable()?;
        check_lines(&lines)?;
        self.header = header.validated()?;
        self.lines = lines;
        self.touch(user, today);
        Ok(())
    }

    /// 確認（t324sub_y_chk；WT8）：已確認 9023、已作廢 9024 不可；須有單身 mfg-009
    pub fn confirm(&mut self, user: &str, today: NaiveDate) -> Result<(), DomainError> {
        match self.status {
            TxnStatus::Confirmed => return Err(DomainError::RecordAlreadyConfirmed),
            TxnStatus::Voided => return Err(DomainError::InventoryTransactionVoided),
            TxnStatus::Open => {}
        }
        if self.lines.is_empty() {
            return Err(DomainError::InventoryTransactionNoLines);
        }
        self.status = TxnStatus::Confirmed;
        self.touch(user, today);
        Ok(())
    }

    /// 取消確認（t324_z）：未確認 9025、已作廢 9024、已過帳 afa-101 不可
    pub fn unconfirm(&mut self, user: &str, today: NaiveDate) -> Result<(), DomainError> {
        match self.status {
            TxnStatus::Confirmed => {}
            TxnStatus::Voided => return Err(DomainError::InventoryTransactionVoided),
            TxnStatus::Open => return Err(DomainError::RecordNotConfirmed),
        }
        if self.posted {
            return Err(DomainError::InventoryTransactionPosted);
        }
        self.status = TxnStatus::Open;
        self.touch(user, today);
        Ok(())
    }

    /// 作廢／取消作廢（t324_x；WT11）：已確認、已過帳不可
    pub fn set_void(
        &mut self,
        void: bool,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        if self.posted {
            return Err(DomainError::InventoryTransactionPosted);
        }
        match (self.status, void) {
            (TxnStatus::Confirmed, _) => return Err(DomainError::InventoryTransactionConfirmed),
            (TxnStatus::Open, true) => self.status = TxnStatus::Voided,
            (TxnStatus::Voided, false) => self.status = TxnStatus::Open,
            (TxnStatus::Open, false) => return Err(DomainError::RecordNotConfirmed),
            (TxnStatus::Voided, true) => return Err(DomainError::InventoryTransactionVoided),
        }
        self.touch(user, today);
        Ok(())
    }

    /// 過帳（t324_s；WT9）：須已確認（aba-100）、未過帳（asf-812）、未作廢（9024）、有單身（mfg-009）。過帳日＝調撥日
    pub fn post(&mut self, user: &str, today: NaiveDate) -> Result<(), DomainError> {
        match self.status {
            TxnStatus::Voided => return Err(DomainError::InventoryTransactionVoided),
            TxnStatus::Open => return Err(DomainError::RecordNotConfirmed),
            TxnStatus::Confirmed => {}
        }
        if self.posted {
            return Err(DomainError::InventoryTransactionPosted);
        }
        if self.lines.is_empty() {
            return Err(DomainError::InventoryTransactionNoLines);
        }
        self.posted = true;
        self.touch(user, today);
        Ok(())
    }

    /// 過帳還原（aimp378；WT10）：須已過帳（aim-206）、未作廢
    pub fn unpost(&mut self, user: &str, today: NaiveDate) -> Result<(), DomainError> {
        if self.status == TxnStatus::Voided {
            return Err(DomainError::InventoryTransactionVoided);
        }
        if !self.posted {
            return Err(DomainError::InventoryTransactionNotPosted);
        }
        self.posted = false;
        self.touch(user, today);
        Ok(())
    }

    /// 複製（WT12，同 MT11）：新單號、調撥日；狀態回到開立、未過帳；資料所有者改為目前使用者
    pub fn copy_as(
        &self,
        no: String,
        transfer_date: NaiveDate,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            no,
            header: TransferHeader {
                transfer_date,
                ..self.header.clone()
            },
            status: TxnStatus::Open,
            posted: false,
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
            ..self.clone()
        }
    }

    pub fn next_line_no(lines: &[TransferLine]) -> i32 {
        lines.iter().map(|l| l.line_no).max().unwrap_or(0) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(no: i32, to_wh: &str) -> TransferLine {
        TransferLine {
            line_no: no,
            item: "A001".into(),
            from_warehouse: "W01".into(),
            from_location: String::new(),
            from_lot: String::new(),
            to_warehouse: to_wh.into(),
            to_location: String::new(),
            to_lot: String::new(),
            unit: "PCS".into(),
            qty: Decimal::ONE,
            reason: None,
        }
    }

    #[test]
    fn same_storage() {
        assert_eq!(
            check_lines(&[line(1, "W01")]).unwrap_err().code(),
            "WAREHOUSE_TRANSFER_SAME_STORAGE"
        );
        assert!(check_lines(&[line(1, "W02")]).is_ok());
    }

    #[test]
    fn flow() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
        let h = TransferHeader {
            transfer_date: d,
            dept: None,
            applicant: None,
            memo: None,
        };
        let mut t =
            WarehouseTransfer::new("TR1-26110001".into(), h, vec![line(1, "W02")], "u", "D", d)
                .unwrap();
        assert_eq!(t.post("u", d).unwrap_err().code(), "RECORD_NOT_CONFIRMED");
        t.confirm("u", d).unwrap();
        t.post("u", d).unwrap();
        assert_eq!(
            t.unconfirm("u", d).unwrap_err().code(),
            "INVENTORY_TRANSACTION_POSTED"
        );
        t.unpost("u", d).unwrap();
        t.unconfirm("u", d).unwrap();
    }
}
