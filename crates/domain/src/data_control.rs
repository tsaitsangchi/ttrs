//! 資料控管原則（TipTop: gev_file 中 gev03＝N 的列＝受控營運中心、gey_file 欄位條件；aooi601 資料控管原則維護作業、
//! s_dc_funs.s_field_chk 欄位值檢查）
//! 規格：specs/features/aoo/data_control.feature（DC1–DC11）

use chrono::NaiveDate;

use crate::{error::DomainError, group_center::DataType, record::Stamp};

/// 值最多 255 字（gey05 varchar2(255)）
pub const VALUES_MAX: usize = 255;
/// 值不可含的字元（s_value_chk，aoo-031；TipTop 少檢查了 =，TTRS 一併檢查，DC5(c)）
const INVALID_CHARS: [char; 8] = ['\'', '"', ')', '+', '>', '<', '=', '*'];

/// 資料類別 0 基本資料不可維護（aooi601.4gl:507-510 aoo-048，DC3）
pub fn check_maintainable(data_type: DataType) -> Result<(), DomainError> {
    if data_type.code() == "0" {
        Err(DomainError::DataControlTypeNotMaintainable)
    } else {
        Ok(())
    }
}

/// 各資料類別可設定的欄位（s_set_combo_all，DC5(b)）：目前只開放已移植的料件欄位，
/// 其他資料類別在該模組移植時加入
pub fn fields_of(data_type: DataType) -> &'static [&'static str] {
    match data_type.code().as_str() {
        // ima01 料件編號、ima06 分群碼、ima109 材料類別
        "1" => &["item_no", "group_code", "material_category"],
        _ => &[],
    }
}

/// 允許新增的條件 gey04（s_get_option，DC6）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Condition {
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    BeginsWith,
    NotBeginsWith,
    EndsWith,
    NotEndsWith,
    Contains,
    NotContains,
}

impl Condition {
    pub const ALL: [Self; 11] = [
        Self::NotEqual,
        Self::Greater,
        Self::GreaterOrEqual,
        Self::Less,
        Self::LessOrEqual,
        Self::BeginsWith,
        Self::NotBeginsWith,
        Self::EndsWith,
        Self::NotEndsWith,
        Self::Contains,
        Self::NotContains,
    ];

    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let n: usize = raw
            .trim()
            .parse()
            .map_err(|_| DomainError::DataControlConditionInvalid)?;
        n.checked_sub(1)
            .and_then(|i| Self::ALL.get(i).copied())
            .ok_or(DomainError::DataControlConditionInvalid)
    }

    pub fn code(self) -> String {
        let i = Self::ALL.iter().position(|c| *c == self).unwrap_or(0);
        (i + 1).to_string()
    }

    /// 比較條件 2–5 只能有一個值（DC5(d)）
    fn single_value(self) -> bool {
        matches!(
            self,
            Self::Greater | Self::GreaterOrEqual | Self::Less | Self::LessOrEqual
        )
    }

    /// 肯定條件（開頭是、結尾是、包含）符合任一個值即可；否定條件（不等於、開頭不是、結尾不是、不包含）須全部符合
    fn any_of(self) -> bool {
        matches!(self, Self::BeginsWith | Self::EndsWith | Self::Contains)
    }

    fn test(self, value: &str, v: &str) -> bool {
        match self {
            Self::NotEqual => value != v,
            Self::Greater => value > v,
            Self::GreaterOrEqual => value >= v,
            Self::Less => value < v,
            Self::LessOrEqual => value <= v,
            Self::BeginsWith => value.starts_with(v),
            Self::NotBeginsWith => !value.starts_with(v),
            Self::EndsWith => value.ends_with(v),
            Self::NotEndsWith => !value.ends_with(v),
            Self::Contains => value.contains(v),
            Self::NotContains => !value.contains(v),
        }
    }
}

/// 欄位條件一列（gey03 欄位、gey04 條件、gey05 值）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRule {
    pub field: String,
    pub condition: Condition,
    /// 原樣保存的值（以 ; 分隔）
    pub values: String,
}

impl FieldRule {
    /// 檢查欄位、條件、值（DC5）
    pub fn new(
        data_type: DataType,
        field: &str,
        condition: &str,
        values: &str,
    ) -> Result<Self, DomainError> {
        Self::within(
            fields_of(data_type),
            DomainError::DataControlFieldInvalid,
            field,
            condition,
            values,
        )
    }

    /// 欄位須在 `allowed` 之中（拋轉條件另有欄位清單，carry_setting.feature CS6）；條件與值的檢查同 DC5(c)(d)
    pub fn within(
        allowed: &[&str],
        invalid_field: DomainError,
        field: &str,
        condition: &str,
        values: &str,
    ) -> Result<Self, DomainError> {
        let field = field.trim();
        if !allowed.contains(&field) {
            return Err(invalid_field);
        }
        let condition = Condition::parse(condition)?;
        let values = values.trim();
        let parts: Vec<&str> = split(values);
        if parts.is_empty()
            || values.chars().count() > VALUES_MAX
            || values.chars().any(|c| INVALID_CHARS.contains(&c))
            || (condition.single_value() && parts.len() != 1)
        {
            return Err(DomainError::DataControlValueInvalid);
        }
        Ok(Self {
            field: field.to_owned(),
            condition,
            values: values.to_owned(),
        })
    }

    /// 欄位值是否符合條件（s_field_chk，DC6）：值空白時不檢查；區分大小寫、文字逐字比較
    pub fn allows(&self, value: &str) -> bool {
        if value.is_empty() {
            return true;
        }
        let parts = split(&self.values);
        if self.condition.any_of() {
            parts.iter().any(|v| self.condition.test(value, v))
        } else {
            parts.iter().all(|v| self.condition.test(value, v))
        }
    }
}

fn split(values: &str) -> Vec<&str> {
    values
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// 受控營運中心（gev03＝N 的列）與它的欄位條件
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlledPlant {
    pub data_type: DataType,
    pub plant: String,
    /// 欄位條件，依欄位排序（aooi601.4gl b_fill2 ORDER BY gey03）
    pub rules: Vec<FieldRule>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl ControlledPlant {
    pub fn new(
        data_type: DataType,
        plant: &str,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        check_maintainable(data_type)?;
        Ok(Self {
            data_type,
            plant: plant.to_owned(),
            rules: Vec::new(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        })
    }

    /// 新增欄位條件：同一營運中心每個欄位只能一筆（-239）
    pub fn add_rule(
        &mut self,
        rule: FieldRule,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        if self.rules.iter().any(|r| r.field == rule.field) {
            return Err(DomainError::DataControlRuleAlreadyExists);
        }
        self.rules.push(rule);
        self.rules.sort_by(|a, b| a.field.cmp(&b.field));
        self.touch(user, today);
        Ok(())
    }

    /// 修改條件與值；欄位不可改（DC5(e)）
    pub fn update_rule(
        &mut self,
        rule: FieldRule,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        let r = self
            .rules
            .iter_mut()
            .find(|r| r.field == rule.field)
            .ok_or(DomainError::DataControlRuleNotFound)?;
        *r = rule;
        self.touch(user, today);
        Ok(())
    }

    pub fn remove_rule(
        &mut self,
        field: &str,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        let before = self.rules.len();
        self.rules.retain(|r| r.field != field);
        if self.rules.len() == before {
            return Err(DomainError::DataControlRuleNotFound);
        }
        self.touch(user, today);
        Ok(())
    }

    /// 欄位值檢查（s_field_chk）：沒有設定該欄位條件時不檢查；不符合回 DATA_CONTROL_FIELD_REJECTED（aoo-043）
    pub fn check_field(&self, field: &str, value: &str) -> Result<(), DomainError> {
        match self.rules.iter().find(|r| r.field == field) {
            Some(r) if !r.allows(value) => Err(DomainError::DataControlFieldRejected {
                field: field.to_owned(),
                value: value.to_owned(),
            }),
            _ => Ok(()),
        }
    }

    fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> DataType {
        DataType::parse("1").unwrap()
    }

    fn rule(c: &str, v: &str) -> FieldRule {
        FieldRule::new(item(), "item_no", c, v).unwrap()
    }

    #[test]
    fn conditions() {
        assert!(rule("6", "L;M").allows("M001"));
        assert!(!rule("6", "L;M").allows("A001"));
        assert!(rule("7", "L;M").allows("A001"));
        assert!(!rule("7", "L;M").allows("L001"));
        assert!(!rule("1", "A001").allows("A001"));
        assert!(rule("1", "A001;B001").allows("C001"));
        assert!(rule("8", "-T").allows("A01-T"));
        assert!(!rule("9", "-T").allows("A01-T"));
        assert!(rule("10", "X;Y").allows("AX01"));
        assert!(!rule("11", "X;Y").allows("AY01"));
        assert!(rule("3", "M").allows("M001"));
        assert!(!rule("4", "M").allows("M001"));
        assert!(!rule("6", "l").allows("L001"));
        assert!(rule("6", "L").allows(""));
    }

    #[test]
    fn invalid_rules() {
        let bad = |f: &str, c: &str, v: &str| FieldRule::new(item(), f, c, v).unwrap_err();
        assert_eq!(bad("occ01", "6", "C"), DomainError::DataControlFieldInvalid);
        assert_eq!(
            bad("item_no", "12", "L"),
            DomainError::DataControlConditionInvalid
        );
        assert_eq!(
            bad("item_no", "0", "L"),
            DomainError::DataControlConditionInvalid
        );
        for v in ["", " ; ", "L*", "L=1", "A'B"] {
            assert_eq!(bad("item_no", "6", v), DomainError::DataControlValueInvalid);
        }
        assert_eq!(
            bad("item_no", "2", "L;M"),
            DomainError::DataControlValueInvalid
        );
        assert_eq!(Condition::parse("11").unwrap().code(), "11");
    }

    #[test]
    fn plant_rules() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
        assert_eq!(
            ControlledPlant::new(DataType::parse("0").unwrap(), "TW02", "u", "d", today),
            Err(DomainError::DataControlTypeNotMaintainable)
        );
        let mut p = ControlledPlant::new(item(), "TW02", "u", "d", today).unwrap();
        p.add_rule(rule("6", "L"), "u", today).unwrap();
        assert_eq!(
            p.add_rule(rule("1", "X"), "u", today),
            Err(DomainError::DataControlRuleAlreadyExists)
        );
        assert!(p.check_field("item_no", "L1").is_ok());
        assert!(p.check_field("item_no", "A1").is_err());
        assert!(p.check_field("group_code", "A1").is_ok());
    }
}
