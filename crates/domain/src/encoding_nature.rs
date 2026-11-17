//! 編碼性質（TipTop: geh_file 編碼性質檔；aooi401 編碼性質說明）
//! 規格：specs/features/aoo/encoding_nature.feature（EN1–EN8）

use chrono::NaiveDate;

use crate::{error::DomainError, record::Stamp};

/// 性質代碼最多 2 字（geh01 varchar2(2)，EN3）
pub const CODE_MAX: usize = 2;
/// 說明最多 80 字（geh02）
pub const NAME_MAX: usize = 80;

/// 性質代碼：去頭尾空白、區分大小寫、1–2 字（EN3）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NatureCode(String);

impl NatureCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::EncodingNatureCodeEmpty);
        }
        if v.chars().count() > CODE_MAX {
            return Err(DomainError::EncodingNatureCodeTooLong { max: CODE_MAX });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 類別 geh04（aooi401.4gl:267-270、aooi401.4fd）：號碼用在哪一種資料。
/// TipTop 另存的檔案名稱 geh03、索引值 geh05 是由類別決定的固定值，TTRS 不存（EN5）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NatureCategory {
    /// 1 料件編號（ima_file.ima01）
    Item,
    /// 2 客戶編號（occ_file.occ01）
    Customer,
    /// 3 供應商編號（pmc_file.pmc01）
    Supplier,
    /// 4 固定資產編號（faj_file.faj02）
    FixedAsset,
    /// 5 批號（rvbs_file.rvbs04）
    Lot,
    /// 6 序號（rvbs_file.rvbs03）
    Serial,
}

impl NatureCategory {
    /// 只能是 1–6（:267-270，EN4）
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        match raw.trim() {
            "1" => Ok(Self::Item),
            "2" => Ok(Self::Customer),
            "3" => Ok(Self::Supplier),
            "4" => Ok(Self::FixedAsset),
            "5" => Ok(Self::Lot),
            "6" => Ok(Self::Serial),
            _ => Err(DomainError::EncodingNatureCategoryInvalid),
        }
    }

    /// TipTop geh04 的值
    pub fn code(self) -> &'static str {
        match self {
            Self::Item => "1",
            Self::Customer => "2",
            Self::Supplier => "3",
            Self::FixedAsset => "4",
            Self::Lot => "5",
            Self::Serial => "6",
        }
    }
}

/// 可修改的欄位：說明、類別
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NatureAttrs {
    pub name: Option<String>,
    pub category: NatureCategory,
}

impl NatureAttrs {
    /// 說明最多 80 字、可空白；類別必填且只能 1–6（EN3、EN4）
    pub fn new(name: Option<String>, category: &str) -> Result<Self, DomainError> {
        let name = name.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        if name.as_ref().is_some_and(|n| n.chars().count() > NAME_MAX) {
            return Err(DomainError::FieldTooLong {
                field: "說明",
                max: NAME_MAX,
            });
        }
        Ok(Self {
            name,
            category: NatureCategory::parse(category)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodingNature {
    pub code: NatureCode,
    pub attrs: NatureAttrs,
    /// gehacti 資料有效碼
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；geh_file 無此欄）
    pub confirmation: crate::confirmation::Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl EncodingNature {
    /// 新增：資料有效預設 Y（aooi401.4gl:200-215）
    pub fn new(
        code: NatureCode,
        attrs: NatureAttrs,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            code,
            attrs,
            active: true,
            confirmation: crate::confirmation::Confirmation::default(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn update(&mut self, attrs: NatureAttrs, user: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(user, today);
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製（ADR 0013 SA5；aooi401 原本沒有）：新性質代碼，說明與類別帶入；資料所有者為目前使用者、未確認
    pub fn copy_as(&self, code: NatureCode, user: &str, dept: &str, today: NaiveDate) -> Self {
        Self::new(code, self.attrs.clone(), user, dept, today)
    }

    /// 確認／取消確認（ADR 0013 SA9）
    pub fn set_confirmed(
        &mut self,
        confirm: bool,
        user: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), DomainError> {
        if confirm {
            self.confirmation.confirm(user, now)?;
        } else {
            self.confirmation.unconfirm()?;
        }
        self.touch(user, now.date_naive());
        Ok(())
    }

    fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_rules() {
        assert_eq!(NatureCode::parse(" A1 ").unwrap().as_str(), "A1");
        assert_eq!(
            NatureCode::parse(""),
            Err(DomainError::EncodingNatureCodeEmpty)
        );
        assert_eq!(
            NatureCode::parse("ABC"),
            Err(DomainError::EncodingNatureCodeTooLong { max: 2 })
        );
    }

    #[test]
    fn category_rules() {
        for c in ["1", "2", "3", "4", "5", "6"] {
            assert_eq!(NatureCategory::parse(c).unwrap().code(), c);
        }
        for c in ["", "0", "7", "A"] {
            assert_eq!(
                NatureCategory::parse(c),
                Err(DomainError::EncodingNatureCategoryInvalid)
            );
        }
        assert!(
            NatureAttrs::new(Some("  ".into()), "1")
                .unwrap()
                .name
                .is_none()
        );
    }
}
