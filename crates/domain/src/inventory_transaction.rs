//! Inventory Transaction 雜項發料／收料／報廢（TipTop aimt301／aimt302／aimt303 → saimt370 / ina_file、inb_file；
//! specs/features/aim/misc_transaction.feature）
//!
//! 三支作業（misc-issue、misc-receipt、misc-scrap）各自有 service、API、畫面與資料表（Key User 2026-11-20）；
//! 這裡只放三者共同的單據規則與狀態。各作業特有的檢查（庫存、有效日期、部門必填）寫在各自的 service。

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::{DomainError, department::DeptCode, record::Stamp};

pub const MEMO_MAX: usize = 255;
/// 儲位（inb06 varchar2(10)）、批號（inb07 varchar2(24)）長度
pub const LOCATION_MAX: usize = 10;
pub const LOT_MAX: usize = 24;

/// 單據類別（ina00）：本份只做一般倉版 1／3／5（WIP 版 2／4／6 延到 asf，MT3）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TxnKind {
    /// 1 庫存雜項發料（aimt301）
    Issue,
    /// 3 庫存雜項收料（aimt302）
    Receipt,
    /// 5 庫存雜項報廢（aimt303）
    Scrap,
}

impl TxnKind {
    pub const ALL: [TxnKind; 3] = [Self::Issue, Self::Receipt, Self::Scrap];

    /// ina00
    pub fn code(self) -> &'static str {
        match self {
            Self::Issue => "1",
            Self::Receipt => "3",
            Self::Scrap => "5",
        }
    }

    pub fn from_code(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.code() == raw)
    }

    /// 網址與 API 用的名稱
    pub fn slug(self) -> &'static str {
        match self {
            Self::Issue => "issue",
            Self::Receipt => "receipt",
            Self::Scrap => "scrap",
        }
    }

    pub fn from_slug(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.slug() == raw)
    }

    /// TTRS 程式代號（MT1）
    pub fn program(self) -> &'static str {
        match self {
            Self::Issue => "misc-issue",
            Self::Receipt => "misc-receipt",
            Self::Scrap => "misc-scrap",
        }
    }

    /// 單別的單據性質（aim 1 雜發、2 雜收、3 報廢；t370_chk_ina01 g_chr）
    pub fn doc_kind(self) -> &'static str {
        match self {
            Self::Issue => "1",
            Self::Receipt => "2",
            Self::Scrap => "3",
        }
    }

    /// 出庫：須已有庫存、不可超過庫存（ina00 1／2／5／6；mfg6069、mfg1303）
    pub fn takes_stock(self) -> bool {
        matches!(self, Self::Issue | Self::Scrap)
    }

    /// 檢查有效日期（ina00 不為 5／6；aim-400）
    pub fn checks_expiry(self) -> bool {
        !matches!(self, Self::Scrap)
    }
}

/// 狀態（inaconf：N 開立、Y 已確認、X 作廢）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxnStatus {
    Open,
    Confirmed,
    Voided,
}

impl TxnStatus {
    pub fn code(self) -> &'static str {
        match self {
            Self::Open => "N",
            Self::Confirmed => "Y",
            Self::Voided => "X",
        }
    }

    pub fn from_code(raw: &str) -> Option<Self> {
        match raw {
            "N" => Some(Self::Open),
            "Y" => Some(Self::Confirmed),
            "X" => Some(Self::Voided),
            _ => None,
        }
    }
}

/// 單頭可修改的欄位
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxnHeader {
    /// ina03 輸入日
    pub input_date: NaiveDate,
    /// ina02 扣帳日
    pub posting_date: NaiveDate,
    /// ina04 部門
    pub dept: Option<DeptCode>,
    /// ina11 申請人（員工代號）
    pub applicant: Option<String>,
    /// ina07 備註
    pub memo: Option<String>,
}

impl TxnHeader {
    pub fn validated(mut self) -> Result<Self, DomainError> {
        self.memo = self
            .memo
            .map(|m| m.trim().to_owned())
            .filter(|m| !m.is_empty());
        // 備註必填（客製 aim-372，MT14 保留）
        let Some(memo) = &self.memo else {
            return Err(DomainError::InventoryTransactionMemoRequired);
        };
        if memo.chars().count() > MEMO_MAX {
            return Err(DomainError::InventoryTransactionMemoTooLong);
        }
        self.applicant = self
            .applicant
            .map(|a| a.trim().to_owned())
            .filter(|a| !a.is_empty());
        Ok(self)
    }
}

/// 單身（inb_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxnLine {
    /// inb03 項次
    pub line_no: i32,
    /// inb04 料號
    pub item: String,
    /// inb05 倉庫
    pub warehouse: String,
    /// inb06 儲位（空白為空字串）
    pub location: String,
    /// inb07 批號（空白為空字串）
    pub lot: String,
    /// inb08 單位（一律為庫存單位，MT7(d)）
    pub unit: String,
    /// inb09 異動數量
    pub qty: Decimal,
    /// inb15 理由碼
    pub reason: Option<String>,
}

/// 數量：大於 0、小數最多 3 位（aim-120、aim-989）
pub fn check_qty(qty: Decimal) -> Result<Decimal, DomainError> {
    if qty <= Decimal::ZERO || qty.normalize().scale() > 3 {
        return Err(DomainError::InventoryTransactionQtyInvalid);
    }
    Ok(qty.normalize())
}

/// 儲位、批號長度
pub fn check_storage_text(location: &str, lot: &str) -> Result<(), DomainError> {
    if location.chars().count() > LOCATION_MAX {
        return Err(DomainError::InventoryTransactionStorageTooLong(
            LOCATION_MAX,
        ));
    }
    if lot.chars().count() > LOT_MAX {
        return Err(DomainError::InventoryTransactionStorageTooLong(LOT_MAX));
    }
    Ok(())
}

/// 單身整體：項次大於 0 且不重複（-239）、理由碼須相同（客製 cim-001，MT14）
pub fn check_lines(lines: &[TxnLine]) -> Result<(), DomainError> {
    let mut seen = std::collections::BTreeSet::new();
    for l in lines {
        if l.line_no <= 0 {
            return Err(DomainError::InventoryTransactionLineNoInvalid);
        }
        if !seen.insert(l.line_no) {
            return Err(DomainError::InventoryTransactionLineDuplicate(l.line_no));
        }
    }
    let mut reasons = lines.iter().filter_map(|l| l.reason.as_deref());
    if let Some(first) = reasons.next()
        && reasons.any(|r| r != first)
    {
        return Err(DomainError::InventoryTransactionReasonMixed);
    }
    Ok(())
}

/// 雜項單（ina_file＋inb_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryTransaction {
    /// ina01 單號
    pub no: String,
    pub kind: TxnKind,
    pub header: TxnHeader,
    pub lines: Vec<TxnLine>,
    pub status: TxnStatus,
    /// inapost 過帳（第二份規格；本份固定未過帳）
    pub posted: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl InventoryTransaction {
    pub fn new(
        no: String,
        kind: TxnKind,
        header: TxnHeader,
        lines: Vec<TxnLine>,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        check_lines(&lines)?;
        Ok(Self {
            no,
            kind,
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

    /// 修改、單身、刪除：已確認（9023）、已作廢（9024）不可（MT9）
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
        header: TxnHeader,
        lines: Vec<TxnLine>,
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

    /// 確認（t370_y_chk）：已確認、已作廢不可；須有單身（mfg-009）
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

    /// 取消確認（t370_z）：未確認不可；已過帳不可（afa-101）
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

    /// 作廢／取消作廢（t370_x）：已過帳（asf-812）、已確認（9023）不可
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

    /// 複製（t370_copy，MT11）：新單號、輸入日、扣帳日；狀態回到開立、未過帳；資料所有者改為目前使用者
    pub fn copy_as(
        &self,
        no: String,
        input_date: NaiveDate,
        posting_date: NaiveDate,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            no,
            header: TxnHeader {
                input_date,
                posting_date,
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

    /// 過帳（t370_s_chk、t370_s_upd；PT2）：須已確認（aba-100）、有單身（mfg-008）、未過帳（asf-812）、未作廢（9024）；
    /// 扣帳日可在過帳時改。庫存檢查與更新在 use case
    pub fn post(
        &mut self,
        posting_date: NaiveDate,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
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
        self.header.posting_date = posting_date;
        self.posted = true;
        self.touch(user, today);
        Ok(())
    }

    /// 過帳還原（aimp379；PT8）：須已過帳（aim-206）、未作廢（9024）；回到已確認、未過帳
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

    /// 下一個項次（t370_bef_inb03：最大項次＋1）
    pub fn next_line_no(lines: &[TxnLine]) -> i32 {
        lines.iter().map(|l| l.line_no).max().unwrap_or(0) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn line(no: i32, reason: Option<&str>) -> TxnLine {
        TxnLine {
            line_no: no,
            item: "A001".into(),
            warehouse: "W01".into(),
            location: String::new(),
            lot: String::new(),
            unit: "PCS".into(),
            qty: Decimal::ONE,
            reason: reason.map(str::to_owned),
        }
    }

    fn txn(lines: Vec<TxnLine>) -> InventoryTransaction {
        InventoryTransaction::new(
            "MI1-26110001".into(),
            TxnKind::Issue,
            TxnHeader {
                input_date: d("2026-11-20"),
                posting_date: d("2026-11-20"),
                dept: None,
                applicant: None,
                memo: Some("領用".into()),
            },
            lines,
            "tiptop",
            "MIS",
            d("2026-11-20"),
        )
        .unwrap()
    }

    #[test]
    fn qty_rules() {
        assert!(check_qty(Decimal::new(1234, 3)).is_ok());
        for bad in [Decimal::ZERO, Decimal::new(-1, 0), Decimal::new(12345, 4)] {
            assert_eq!(
                check_qty(bad).unwrap_err().code(),
                "INVENTORY_TRANSACTION_QTY_INVALID"
            );
        }
    }

    #[test]
    fn lines_rules() {
        assert_eq!(
            check_lines(&[line(1, None), line(1, None)])
                .unwrap_err()
                .code(),
            "INVENTORY_TRANSACTION_LINE_DUPLICATE"
        );
        assert_eq!(
            check_lines(&[line(1, Some("R1")), line(2, Some("R2"))])
                .unwrap_err()
                .code(),
            "INVENTORY_TRANSACTION_REASON_MIXED"
        );
        assert!(check_lines(&[line(1, Some("R1")), line(2, None)]).is_ok());
    }

    #[test]
    fn status_flow() {
        let mut t = txn(vec![]);
        assert_eq!(
            t.confirm("u", d("2026-11-20")).unwrap_err().code(),
            "INVENTORY_TRANSACTION_NO_LINES"
        );
        let mut t2 = txn(vec![line(1, None)]);
        t2.confirm("u", d("2026-11-20")).unwrap();
        assert_eq!(
            t2.check_editable().unwrap_err().code(),
            "INVENTORY_TRANSACTION_CONFIRMED"
        );
        assert_eq!(
            t2.set_void(true, "u", d("2026-11-20")).unwrap_err().code(),
            "INVENTORY_TRANSACTION_CONFIRMED"
        );
        t2.unconfirm("u", d("2026-11-20")).unwrap();
        t2.set_void(true, "u", d("2026-11-20")).unwrap();
        assert_eq!(
            t2.confirm("u", d("2026-11-20")).unwrap_err().code(),
            "INVENTORY_TRANSACTION_VOIDED"
        );
        t.set_void(true, "u", d("2026-11-20")).unwrap();
        t.set_void(false, "u", d("2026-11-20")).unwrap();
        assert_eq!(t.status, TxnStatus::Open);
    }
}
