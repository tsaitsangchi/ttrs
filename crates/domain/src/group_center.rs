//! 集團中心（TipTop: geu_file 集團中心資料檔、gev_file 資料控管維護檔中 gev03＝Y 的列；aooi600 集團中心資料維護）
//! 規格：specs/features/aoo/group_center.feature（GC1–GC11）

use chrono::NaiveDate;

use crate::{error::DomainError, record::Stamp};

/// 集團中心代碼最多 10 字（geu01 varchar2(10)，GC3）
pub const CODE_MAX: usize = 10;
/// 名稱最多 80 字（geu02 varchar2(80)）
pub const NAME_MAX: usize = 80;
/// 類別 1 資料中心（只有資料中心可設定資料類別與拋轉設定，aoo-046、aoo-030）
pub const DATA_CENTER: CenterCategory = CenterCategory(1);

/// 集團中心代碼：去頭尾空白、區分大小寫、1–10 字（GC3）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupCenterCode(String);

impl GroupCenterCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::GroupCenterCodeEmpty);
        }
        if v.chars().count() > CODE_MAX {
            return Err(DomainError::GroupCenterCodeTooLong { max: CODE_MAX });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 集團中心類別 geu00：1 資料中心、2 預測中心、3 訂單中心、4 採購中心、5 信用控管中心、6 帳務中心、7 資金中心（GC4）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CenterCategory(u8);

impl CenterCategory {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        match raw.trim().parse::<u8>() {
            Ok(v @ 1..=7) if raw.trim().len() == 1 => Ok(Self(v)),
            _ => Err(DomainError::GroupCenterCategoryInvalid),
        }
    }

    pub fn code(self) -> String {
        self.0.to_string()
    }
}

/// 資料類別 gev01：0 基本資料、1 料件、2 BOM、3 ECN、4 客戶、5 供應商、6 會計科目、7 價格、8 匯率（GC5）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DataType(u8);

impl DataType {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        match raw.trim().parse::<u8>() {
            Ok(v @ 0..=8) if raw.trim().len() == 1 => Ok(Self(v)),
            _ => Err(DomainError::DataCenterTypeInvalid),
        }
    }

    pub fn code(self) -> String {
        self.0.to_string()
    }
}

/// 資料中心負責的一個「資料類別＋營運中心」（gev01、gev02；gev03＝Y、gev04＝本集團中心）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DataCenterType {
    pub data_type: DataType,
    /// 營運中心代碼（azp01）；是否存在由 use case 檢查
    pub plant: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupCenter {
    pub code: GroupCenterCode,
    pub name: Option<String>,
    pub category: CenterCategory,
    /// 資料類別設定，依資料類別、營運中心排序（aooi600.4gl:700）
    pub data_types: Vec<DataCenterType>,
    /// geuacti 資料有效碼
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；geu_file 無此欄）
    pub confirmation: crate::confirmation::Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

/// 名稱：去頭尾空白、空白視為未輸入、最多 80 字
pub fn parse_name(raw: Option<String>) -> Result<Option<String>, DomainError> {
    let name = raw.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    if name.as_ref().is_some_and(|n| n.chars().count() > NAME_MAX) {
        return Err(DomainError::FieldTooLong {
            field: "名稱",
            max: NAME_MAX,
        });
    }
    Ok(name)
}

impl GroupCenter {
    /// 新增：資料有效預設 Y（aooi600.4gl:204-227）
    pub fn new(
        code: GroupCenterCode,
        name: Option<String>,
        category: CenterCategory,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            code,
            name,
            category,
            data_types: Vec::new(),
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

    /// 有效的資料中心才能設定資料類別與拋轉設定（:98-118 aoo-046、:681-689）
    pub fn check_data_center(&self) -> Result<(), DomainError> {
        if self.category == DATA_CENTER && self.active {
            Ok(())
        } else {
            Err(DomainError::GroupCenterNotDataCenter)
        }
    }

    /// 修改名稱與類別（:314-337）：已有資料類別設定時不可改成資料中心以外（GC4）
    pub fn update(
        &mut self,
        name: Option<String>,
        category: CenterCategory,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        if category != DATA_CENTER && !self.data_types.is_empty() {
            return Err(DomainError::GroupCenterHasDataTypes);
        }
        self.name = name;
        self.category = category;
        self.touch(user, today);
        Ok(())
    }

    /// 新增一個資料類別（:778-824）：同一集團中心內不可重複；其他資料中心是否已有由儲存層的主鍵檢查（GC5）
    pub fn add_data_type(&mut self, row: DataCenterType) -> Result<(), DomainError> {
        self.check_data_center()?;
        if self.data_types.contains(&row) {
            return Err(DomainError::DataCenterTypeAlreadyExists);
        }
        self.data_types.push(row);
        self.data_types.sort();
        Ok(())
    }

    /// 刪除一個資料類別（:826-875）
    pub fn remove_data_type(&mut self, row: &DataCenterType) -> Result<(), DomainError> {
        self.check_data_center()?;
        let before = self.data_types.len();
        self.data_types.retain(|t| t != row);
        if self.data_types.len() == before {
            return Err(DomainError::DataCenterTypeNotFound);
        }
        Ok(())
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製只複製集團中心本身，不複製資料類別設定（GC10）：資料所有者為目前使用者、資料有效、未確認
    pub fn copy_as(&self, code: GroupCenterCode, user: &str, dept: &str, today: NaiveDate) -> Self {
        Self::new(code, self.name.clone(), self.category, user, dept, today)
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

    /// 修改者與日期（geumodu、geudate）
    fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 18).unwrap()
    }

    fn center(category: &str) -> GroupCenter {
        GroupCenter::new(
            GroupCenterCode::parse("DC").unwrap(),
            None,
            CenterCategory::parse(category).unwrap(),
            "u",
            "d",
            today(),
        )
    }

    fn row(t: &str, plant: &str) -> DataCenterType {
        DataCenterType {
            data_type: DataType::parse(t).unwrap(),
            plant: plant.to_owned(),
        }
    }

    #[test]
    fn categories() {
        assert!(CenterCategory::parse("7").is_ok());
        for v in ["0", "8", "", "11", "A"] {
            assert_eq!(
                CenterCategory::parse(v),
                Err(DomainError::GroupCenterCategoryInvalid)
            );
        }
        assert!(DataType::parse("0").is_ok());
        assert!(DataType::parse("8").is_ok());
        assert_eq!(
            DataType::parse("9"),
            Err(DomainError::DataCenterTypeInvalid)
        );
    }

    #[test]
    fn data_types() {
        let mut c = center("1");
        assert!(c.add_data_type(row("4", "TW01")).is_ok());
        assert!(c.add_data_type(row("1", "TW02")).is_ok());
        assert!(c.add_data_type(row("1", "TW01")).is_ok());
        assert_eq!(
            c.add_data_type(row("1", "TW01")),
            Err(DomainError::DataCenterTypeAlreadyExists)
        );
        let order: Vec<_> = c
            .data_types
            .iter()
            .map(|t| format!("{} {}", t.data_type.code(), t.plant))
            .collect();
        assert_eq!(order, ["1 TW01", "1 TW02", "4 TW01"]);
        assert_eq!(
            c.update(None, CenterCategory::parse("4").unwrap(), "u", today()),
            Err(DomainError::GroupCenterHasDataTypes)
        );
        assert_eq!(
            c.remove_data_type(&row("2", "TW01")),
            Err(DomainError::DataCenterTypeNotFound)
        );
    }

    #[test]
    fn only_active_data_center() {
        let mut c = center("4");
        assert_eq!(
            c.add_data_type(row("1", "TW01")),
            Err(DomainError::GroupCenterNotDataCenter)
        );
        let mut c = center("1");
        c.set_active(false, "u", today());
        assert_eq!(
            c.add_data_type(row("1", "TW01")),
            Err(DomainError::GroupCenterNotDataCenter)
        );
    }
}
