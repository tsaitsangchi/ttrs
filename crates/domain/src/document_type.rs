//! Document Type 單別（TipTop asmi300 / smy_file、smz_file、smu_file、smv_file；specs/features/aim/document_type.feature）
//!
//! 單別是單號的前幾碼，決定單據的系統別、單據性質與編號、成本、倉儲限制。本批只開放系統別 aim（DT3）。
//! 使用單別時的檢查（s_mfgslip、s_tlf1）在 [`DocumentType::check_use`]、[`DocumentType::check_storage`]。

use chrono::NaiveDate;

use crate::{DomainError, confirmation::Confirmation, department::DeptCode, record::Stamp};

pub const NAME_MAX: usize = 80;
pub const MEMO_MAX: usize = 255;
/// 倉庫、儲位設限值的長度（smz021、smz031 varchar2(10)）
pub const STORAGE_MAX: usize = 10;

/// 單別代號（smyslip）：長度依整體參數 aza41；不可有空白或「-」；區分大小寫（DT5）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocTypeCode(String);

impl DocTypeCode {
    pub fn parse(raw: &str, len: u8) -> Result<Self, DomainError> {
        if raw.chars().any(|c| c.is_whitespace() || c == '-') {
            return Err(DomainError::DocumentTypeCodeInvalid);
        }
        if raw.chars().count() != usize::from(len) {
            return Err(DomainError::DocumentTypeCodeLength { len });
        }
        Ok(Self(raw.to_owned()))
    }

    /// 已存在的單別（查詢、讀取時不檢查長度）
    pub fn existing(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() || v.chars().count() > 5 {
            return Err(DomainError::DocumentTypeNotFound);
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 系統別（smysys）；本批只開放 aim（DT3）
pub const SYSTEMS: &[&str] = &["aim"];

/// aim 的單據性質（smykind；DT4）
pub const AIM_KINDS: &[&str] = &[
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F", "H", "Z",
];

fn kinds_of(system: &str) -> Option<&'static [&'static str]> {
    match system {
        "aim" => Some(AIM_KINDS),
        _ => None,
    }
}

/// 系統別與單據性質（mfg0070、mfg0071）
pub fn check_kind(system: &str, kind: &str) -> Result<(), DomainError> {
    let kinds = kinds_of(system).ok_or(DomainError::DocumentTypeSystemInvalid)?;
    if !kinds.contains(&kind) {
        return Err(DomainError::DocumentTypeKindInvalid);
    }
    Ok(())
}

/// 新增時依 aim 單據性質帶的成會單據類別（DT7）
pub fn default_cost_class(system: &str, kind: &str) -> Option<&'static str> {
    match (system, kind) {
        ("aim", "1") => Some("3"),
        ("aim", "2") => Some("1"),
        ("aim", "3" | "5" | "D") => Some("5"),
        ("aim", "4") => Some("4"),
        _ => None,
    }
}

/// 倉庫設限方式（smyware；DT8）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageRestriction {
    /// 0 不限
    Free,
    /// 1 只能用列出的倉庫
    AllowWarehouse,
    /// 2 只能用列出的倉庫＋儲位
    AllowLocation,
    /// 3 不能用列出的倉庫
    DenyWarehouse,
    /// 4 不能用列出的倉庫＋儲位
    DenyLocation,
}

impl StorageRestriction {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        Ok(match raw.trim() {
            "0" => Self::Free,
            "1" => Self::AllowWarehouse,
            "2" => Self::AllowLocation,
            "3" => Self::DenyWarehouse,
            "4" => Self::DenyLocation,
            _ => return Err(DomainError::DocumentTypeRestrictionInvalid),
        })
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Free => "0",
            Self::AllowWarehouse => "1",
            Self::AllowLocation => "2",
            Self::DenyWarehouse => "3",
            Self::DenyLocation => "4",
        }
    }
}

/// 單頭的設定（asmi300.4fd「一般」「成本」頁籤）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocTypeAttrs {
    /// smydesc
    pub name: String,
    /// smysys
    pub system: String,
    /// smykind
    pub kind: String,
    /// smyauno 自動編號
    pub auto_number: bool,
    /// smydmy5 編號方式 1–4
    pub numbering: String,
    /// smyprint 立即列印
    pub print_now: bool,
    /// smydmy4 立即確認
    pub confirm_now: bool,
    /// smydmy1 計算成本
    pub cost_enabled: bool,
    /// smydmy2 成會單據類別 1–5、X
    pub cost_class: String,
    /// smy56 影響呆滯日期
    pub affects_idle_date: bool,
    /// smy53 內部交易單據
    pub internal_trade: bool,
    /// smy59 預算控管
    pub budget_control: bool,
    /// smymemo1
    pub memo: Option<String>,
}

impl DocTypeAttrs {
    /// 檢查與正規化（DT6、DT7）：盤點單一律自動編號、依流水號（asmi300.4gl:2855-2862）
    pub fn validated(mut self) -> Result<Self, DomainError> {
        self.name = self.name.trim().to_owned();
        if self.name.is_empty() {
            return Err(DomainError::DocumentTypeNameEmpty);
        }
        if self.name.chars().count() > NAME_MAX {
            return Err(DomainError::DocumentTypeNameTooLong { max: NAME_MAX });
        }
        self.memo = self
            .memo
            .map(|m| m.trim().to_owned())
            .filter(|m| !m.is_empty());
        if self
            .memo
            .as_ref()
            .is_some_and(|m| m.chars().count() > MEMO_MAX)
        {
            return Err(DomainError::DocumentTypeMemoTooLong { max: MEMO_MAX });
        }
        check_kind(&self.system, &self.kind)?;
        if self.system == "aim" && self.kind == "5" {
            self.auto_number = true;
            self.numbering = "1".into();
        }
        if !["1", "2", "3", "4"].contains(&self.numbering.as_str()) {
            return Err(DomainError::DocumentTypeNumberingInvalid);
        }
        if !["1", "2", "3", "4", "5", "X"].contains(&self.cost_class.as_str())
            || (self.cost_enabled && self.cost_class == "X")
        {
            return Err(DomainError::DocumentTypeCostClassInvalid);
        }
        Ok(self)
    }
}

/// 倉庫設限的一筆（smz_file）：倉庫、儲位可用 * ? 萬用字元；儲位空白表示「儲位空白」
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageRule {
    pub warehouse: String,
    pub location: Option<String>,
}

impl StorageRule {
    pub fn new(warehouse: &str, location: &str) -> Result<Self, DomainError> {
        let w = warehouse.trim();
        let l = location.trim();
        if w.is_empty() || w.chars().count() > STORAGE_MAX || l.chars().count() > STORAGE_MAX {
            return Err(DomainError::DocumentTypeStorageInvalid);
        }
        Ok(Self {
            warehouse: w.to_owned(),
            location: Some(l.to_owned()).filter(|s| !s.is_empty()),
        })
    }

    fn hit(&self, warehouse: &str, location: Option<&str>, with_location: bool) -> bool {
        wildcard(&self.warehouse, warehouse)
            && (!with_location
                || wildcard(
                    self.location.as_deref().unwrap_or(""),
                    location.unwrap_or(""),
                ))
    }
}

/// * 為任意多個字元、? 為一個字元（TipTop 轉成 SQL LIKE 的 % _）
pub fn wildcard(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match p.split_first() {
            None => t.is_empty(),
            Some(('*', rest)) => (0..=t.len()).any(|i| go(rest, &t[i..])),
            Some(('?', rest)) => !t.is_empty() && go(rest, &t[1..]),
            Some((c, rest)) => t.first() == Some(c) && go(rest, &t[1..]),
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    go(&p, &t)
}

/// 單別（smy_file）與三個單身
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentType {
    pub code: DocTypeCode,
    pub attrs: DocTypeAttrs,
    pub restriction: StorageRestriction,
    /// smz_file
    pub storages: Vec<StorageRule>,
    /// smu_file 設限使用者（zx01）
    pub users: Vec<String>,
    /// smv_file 設限部門
    pub depts: Vec<DeptCode>,
    /// smyacti
    pub active: bool,
    pub confirmation: Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl DocumentType {
    pub fn new(
        code: DocTypeCode,
        attrs: DocTypeAttrs,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            code,
            attrs: attrs.validated()?,
            restriction: StorageRestriction::Free,
            storages: Vec::new(),
            users: Vec::new(),
            depts: Vec::new(),
            active: true,
            confirmation: Confirmation::default(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        })
    }

    /// 停用的不可修改（9027）
    pub fn check_active(&self) -> Result<(), DomainError> {
        if self.active {
            Ok(())
        } else {
            Err(DomainError::DocumentTypeInactive)
        }
    }

    pub fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }

    pub fn update(
        &mut self,
        attrs: DocTypeAttrs,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        self.attrs = attrs.validated()?;
        self.touch(user, today);
        Ok(())
    }

    /// 倉庫設限（DT8）：設限 1–4 時至少一筆；同一倉庫＋儲位只留一筆
    pub fn set_storages(
        &mut self,
        restriction: StorageRestriction,
        rules: Vec<StorageRule>,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        if restriction != StorageRestriction::Free && rules.is_empty() {
            return Err(DomainError::DocumentTypeStorageRequired);
        }
        let mut uniq: Vec<StorageRule> = Vec::new();
        for r in rules {
            if !uniq.contains(&r) {
                uniq.push(r);
            }
        }
        self.restriction = restriction;
        self.storages = uniq;
        self.touch(user, today);
        Ok(())
    }

    /// 使用者、部門設限（DT9）；存在與否在 use case 檢查
    pub fn set_access(
        &mut self,
        mut users: Vec<String>,
        mut depts: Vec<DeptCode>,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        users.sort();
        users.dedup();
        depts.sort();
        depts.dedup();
        self.users = users;
        self.depts = depts;
        self.touch(user, today);
        Ok(())
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 刪除（DT10、DT11）：停用的不可刪除（abm-950）；已確認的不可刪除（SA9-2）
    pub fn check_deletable(&self) -> Result<(), DomainError> {
        self.check_active()?;
        self.confirmation.check_deletable()
    }

    /// 複製（DT11）：連同三個單身；資料有效、未確認、所有者為目前使用者
    pub fn copy_as(&self, code: DocTypeCode, user: &str, dept: &str, today: NaiveDate) -> Self {
        Self {
            code,
            confirmation: Confirmation::default(),
            active: true,
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
            ..self.clone()
        }
    }

    /// 單據使用單別的檢查（s_mfgslip）：性質相符（mfg0015）、有效（9028）、使用者與部門設限（aoo-104）
    pub fn check_use(
        &self,
        kind: &str,
        user: &str,
        user_dept: Option<&str>,
    ) -> Result<(), DomainError> {
        if self.attrs.kind != kind {
            return Err(DomainError::DocumentTypeKindMismatch);
        }
        if !self.active {
            return Err(DomainError::DocumentTypeInactive);
        }
        let in_dept = || user_dept.is_some_and(|d| self.depts.iter().any(|x| x.as_str() == d));
        let ok = if !self.users.is_empty() {
            self.users.iter().any(|u| u == user) || in_dept()
        } else if !self.depts.is_empty() {
            in_dept()
        } else {
            true
        };
        if ok {
            Ok(())
        } else {
            Err(DomainError::DocumentTypeNotAllowed)
        }
    }

    /// 倉儲是否可用於此單別（s_tlf1:395-535；DT8）
    pub fn check_storage(
        &self,
        warehouse: &str,
        location: Option<&str>,
    ) -> Result<(), DomainError> {
        let with_location = matches!(
            self.restriction,
            StorageRestriction::AllowLocation | StorageRestriction::DenyLocation
        );
        let hit = self
            .storages
            .iter()
            .any(|r| r.hit(warehouse, location, with_location));
        match self.restriction {
            StorageRestriction::Free => Ok(()),
            StorageRestriction::AllowWarehouse | StorageRestriction::AllowLocation if !hit => {
                Err(DomainError::DocumentTypeStorageNotAllowed)
            }
            StorageRestriction::DenyWarehouse | StorageRestriction::DenyLocation if hit => {
                Err(DomainError::DocumentTypeStorageForbidden)
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_rules() {
        assert!(DocTypeCode::parse("MI1", 3).is_ok());
        assert_eq!(
            DocTypeCode::parse("M1", 3).unwrap_err().code(),
            "DOCUMENT_TYPE_CODE_LENGTH"
        );
        for bad in ["M-1", "M 1"] {
            assert_eq!(
                DocTypeCode::parse(bad, 3).unwrap_err().code(),
                "DOCUMENT_TYPE_CODE_INVALID"
            );
        }
    }

    #[test]
    fn wildcards() {
        assert!(wildcard("W0*", "W01"));
        assert!(!wildcard("W0*", "W10"));
        assert!(wildcard("A-??", "A-01"));
        assert!(!wildcard("A-??", "A-1"));
        assert!(wildcard("", ""));
    }

    #[test]
    fn kinds() {
        assert!(check_kind("aim", "H").is_ok());
        assert_eq!(
            check_kind("apm", "1").unwrap_err().code(),
            "DOCUMENT_TYPE_SYSTEM_INVALID"
        );
        assert_eq!(
            check_kind("aim", "Y").unwrap_err().code(),
            "DOCUMENT_TYPE_KIND_INVALID"
        );
    }
}
