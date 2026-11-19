//! 資料中心拋轉設定（TipTop: gew_file 資料拋轉原則、gez_file 拋轉預設資料；aooi602 資料中心拋轉設定作業）
//! 規格：specs/features/aoo/carry_setting.feature（CS1–CS12）

use chrono::NaiveDate;

use crate::{
    data_control::FieldRule,
    error::DomainError,
    group_center::{DataType, GroupCenterCode},
    record::Stamp,
};

/// 通知人員最多 1500 字（gew08 varchar2(1500)）
pub const NOTIFY_MAX: usize = 1500;
/// 預設值最多 255 字（gez05 varchar2(255)）
pub const DEFAULT_MAX: usize = 255;

/// 資料類別 0 基本資料：不可設定拋轉條件、預設值，「子公司可刪除」不可修改（CS6(d)、CS7(d)）
fn is_basic(data_type: DataType) -> bool {
    data_type.code() == "0"
}

/// 拋轉條件可用的欄位（CS6(c)）：目前只開放料件
pub fn condition_fields(data_type: DataType) -> &'static [&'static str] {
    match data_type.code().as_str() {
        "1" => &[
            "item_no",
            "group_code",
            "material_category",
            "source_code",
            "product_class",
        ],
        _ => &[],
    }
}

/// 預設值可用的欄位（CS7(a)(b)）：料件不開放料件編號
pub fn default_fields(data_type: DataType) -> &'static [&'static str] {
    match data_type.code().as_str() {
        "1" => &["group_code", "material_category"],
        _ => &[],
    }
}

/// 拋轉方式 gew03：1 自動拋轉、2 詢問後拋轉、3 手動拋轉（CS3(d)：未給時 3）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CarryMode(u8);

impl CarryMode {
    pub const MANUAL: Self = Self(3);

    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        match raw.trim() {
            "" => Ok(Self::MANUAL),
            "1" => Ok(Self(1)),
            "2" => Ok(Self(2)),
            "3" => Ok(Self(3)),
            _ => Err(DomainError::CarryModeInvalid),
        }
    }

    pub fn code(self) -> String {
        self.0.to_string()
    }
}

/// 預設值一列（gez04 欄位、gez05 值）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarryDefault {
    pub field: String,
    pub value: String,
}

/// 拋轉營運中心（gew_file 單身）與它的拋轉條件、預設值
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarryTarget {
    pub plant: String,
    /// gew06 子公司可刪除資料否
    pub deletable: bool,
    /// gew07 更新已存在資料否
    pub update_existing: bool,
    /// gew08 拋轉通知人員（電子郵件，以 ; 分隔）
    pub notify: Option<String>,
    /// 拋轉條件（gew09），依欄位排序
    pub conditions: Vec<FieldRule>,
    /// 預設值（gez_file），依欄位排序
    pub defaults: Vec<CarryDefault>,
}

impl CarryTarget {
    /// 新增預設：可刪除 Y、更新 Y、沒有條件（全部拋轉）、沒有通知人員（aooi602.4gl:720-733，CS4(d)）
    pub fn new(plant: &str) -> Self {
        Self {
            plant: plant.to_owned(),
            deletable: true,
            update_existing: true,
            notify: None,
            conditions: Vec::new(),
            defaults: Vec::new(),
        }
    }

    /// 是否符合全部拋轉條件（不同欄位之間「而且」；沒有條件時全部拋轉，CS6(b)）
    pub fn accepts(&self, value_of: impl Fn(&str) -> String) -> bool {
        self.conditions
            .iter()
            .all(|c| c.allows(&value_of(&c.field)))
    }
}

/// 通知人員：去空白後以 ; 分隔的電子郵件，每個都要像 名稱@網域.後綴（CS5）
pub fn parse_notify(raw: Option<String>) -> Result<Option<String>, DomainError> {
    let Some(raw) = raw.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if raw.chars().count() > NOTIFY_MAX {
        return Err(DomainError::CarryNotifyInvalid);
    }
    let parts: Vec<&str> = raw
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let ok = |m: &str| {
        let Some((user, domain)) = m.split_once('@') else {
            return false;
        };
        !user.is_empty()
            && !m.chars().any(char::is_whitespace)
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
            && !domain.contains('@')
    };
    if parts.is_empty() || !parts.iter().all(|m| ok(m)) {
        return Err(DomainError::CarryNotifyInvalid);
    }
    Ok(Some(parts.join(";")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarrySetting {
    /// gew01 資料中心
    pub center: GroupCenterCode,
    /// gew02 資料類別
    pub data_type: DataType,
    /// gew03 拋轉方式（TipTop 存在每一列單身上，TTRS 移到單頭，CS12）
    pub mode: CarryMode,
    /// 拋轉營運中心，依營運中心排序（:922-926 ORDER BY gew04）
    pub targets: Vec<CarryTarget>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl CarrySetting {
    pub fn new(
        center: GroupCenterCode,
        data_type: DataType,
        mode: CarryMode,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            center,
            data_type,
            mode,
            targets: Vec::new(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn set_mode(&mut self, mode: CarryMode, user: &str, today: NaiveDate) {
        self.mode = mode;
        self.touch(user, today);
    }

    fn target_mut(&mut self, plant: &str) -> Result<&mut CarryTarget, DomainError> {
        self.targets
            .iter_mut()
            .find(|t| t.plant == plant)
            .ok_or(DomainError::CarryTargetNotFound)
    }

    /// 新增拋轉營運中心：同一設定內不可重複（-239，CS4(a)）；是否為來源營運中心、是否存在由 use case 檢查
    pub fn add_target(
        &mut self,
        plant: &str,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        if self.targets.iter().any(|t| t.plant == plant) {
            return Err(DomainError::CarryTargetAlreadyExists);
        }
        self.targets.push(CarryTarget::new(plant));
        self.targets.sort_by(|a, b| a.plant.cmp(&b.plant));
        self.touch(user, today);
        Ok(())
    }

    /// 修改子公司可刪除、更新已存在資料、通知人員（:819-852）；基本資料的「子公司可刪除」不可改（CS6(d)）
    pub fn update_target(
        &mut self,
        plant: &str,
        deletable: bool,
        update_existing: bool,
        notify: Option<String>,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        let basic = is_basic(self.data_type);
        let notify = parse_notify(notify)?;
        let t = self.target_mut(plant)?;
        if basic && deletable != t.deletable {
            return Err(DomainError::CarryBasicDataNotAllowed);
        }
        t.deletable = deletable;
        t.update_existing = update_existing;
        t.notify = notify;
        self.touch(user, today);
        Ok(())
    }

    pub fn remove_target(
        &mut self,
        plant: &str,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        let before = self.targets.len();
        self.targets.retain(|t| t.plant != plant);
        if self.targets.len() == before {
            return Err(DomainError::CarryTargetNotFound);
        }
        self.touch(user, today);
        Ok(())
    }

    /// 拋轉條件（CS6）：新增或修改（`replace`＝true 時欄位須已存在）
    pub fn put_condition(
        &mut self,
        plant: &str,
        field: &str,
        (condition, values): (&str, &str),
        replace: bool,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        if is_basic(self.data_type) {
            return Err(DomainError::CarryBasicDataNotAllowed);
        }
        let rule = FieldRule::within(
            condition_fields(self.data_type),
            DomainError::CarryFieldInvalid,
            field,
            condition,
            values,
        )?;
        let t = self.target_mut(plant)?;
        let existing = t.conditions.iter().position(|c| c.field == rule.field);
        match (existing, replace) {
            (Some(_), false) => return Err(DomainError::CarryConditionAlreadyExists),
            (None, true) => return Err(DomainError::CarryConditionNotFound),
            (Some(i), true) => t.conditions[i] = rule,
            (None, false) => t.conditions.push(rule),
        }
        t.conditions.sort_by(|a, b| a.field.cmp(&b.field));
        self.touch(user, today);
        Ok(())
    }

    pub fn remove_condition(
        &mut self,
        plant: &str,
        field: &str,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        let t = self.target_mut(plant)?;
        let before = t.conditions.len();
        t.conditions.retain(|c| c.field != field.trim());
        if t.conditions.len() == before {
            return Err(DomainError::CarryConditionNotFound);
        }
        self.touch(user, today);
        Ok(())
    }

    /// 預設值（CS7）：新增或修改（`replace`＝true 時欄位須已存在）
    pub fn put_default(
        &mut self,
        plant: &str,
        field: &str,
        value: &str,
        replace: bool,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        if is_basic(self.data_type) {
            return Err(DomainError::CarryBasicDataNotAllowed);
        }
        let field = field.trim();
        if !default_fields(self.data_type).contains(&field) {
            return Err(DomainError::CarryDefaultFieldInvalid);
        }
        let value = value.trim();
        if value.is_empty() || value.chars().count() > DEFAULT_MAX {
            return Err(DomainError::CarryDefaultValueInvalid);
        }
        let t = self.target_mut(plant)?;
        let existing = t.defaults.iter().position(|d| d.field == field);
        let row = CarryDefault {
            field: field.to_owned(),
            value: value.to_owned(),
        };
        match (existing, replace) {
            (Some(_), false) => return Err(DomainError::CarryDefaultAlreadyExists),
            (None, true) => return Err(DomainError::CarryDefaultNotFound),
            (Some(i), true) => t.defaults[i] = row,
            (None, false) => t.defaults.push(row),
        }
        t.defaults.sort_by(|a, b| a.field.cmp(&b.field));
        self.touch(user, today);
        Ok(())
    }

    pub fn remove_default(
        &mut self,
        plant: &str,
        field: &str,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        let t = self.target_mut(plant)?;
        let before = t.defaults.len();
        t.defaults.retain(|d| d.field != field.trim());
        if t.defaults.len() == before {
            return Err(DomainError::CarryDefaultNotFound);
        }
        self.touch(user, today);
        Ok(())
    }

    /// 複製到另一個資料中心＋資料類別（CS9）：條件與預設值依新資料類別重新檢查；所有者為目前使用者
    pub fn copy_as(
        &self,
        center: GroupCenterCode,
        data_type: DataType,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        let mut s = Self::new(center, data_type, self.mode, user, dept, today);
        for t in &self.targets {
            s.add_target(&t.plant, user, today)?;
            if !is_basic(data_type) {
                s.update_target(
                    &t.plant,
                    t.deletable,
                    t.update_existing,
                    t.notify.clone(),
                    user,
                    today,
                )?;
            }
            for c in &t.conditions {
                s.put_condition(
                    &t.plant,
                    &c.field,
                    (&c.condition.code(), &c.values),
                    false,
                    user,
                    today,
                )?;
            }
            for d in &t.defaults {
                s.put_default(&t.plant, &d.field, &d.value, false, user, today)?;
            }
        }
        s.updated_by = None;
        s.updated_on = None;
        Ok(s)
    }

    fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 19).unwrap()
    }

    fn setting(t: &str) -> CarrySetting {
        CarrySetting::new(
            GroupCenterCode::parse("DC").unwrap(),
            DataType::parse(t).unwrap(),
            CarryMode::parse("1").unwrap(),
            "u",
            "d",
            today(),
        )
    }

    #[test]
    fn modes() {
        assert_eq!(CarryMode::parse("").unwrap(), CarryMode::MANUAL);
        assert_eq!(CarryMode::parse("4"), Err(DomainError::CarryModeInvalid));
        assert_eq!(CarryMode::parse("N"), Err(DomainError::CarryModeInvalid));
    }

    #[test]
    fn notify() {
        assert_eq!(
            parse_notify(Some(" a@example.com ; b@example.com ".into())).unwrap(),
            Some("a@example.com;b@example.com".into())
        );
        assert_eq!(parse_notify(Some(" ".into())).unwrap(), None);
        for bad in ["不是郵件", "a@b", "@b.com", "a b@c.com", "a@.com"] {
            assert_eq!(
                parse_notify(Some(bad.into())),
                Err(DomainError::CarryNotifyInvalid),
                "{bad}"
            );
        }
    }

    #[test]
    fn targets_conditions_defaults() {
        let mut s = setting("1");
        s.add_target("TW02", "u", today()).unwrap();
        assert_eq!(
            s.add_target("TW02", "u", today()),
            Err(DomainError::CarryTargetAlreadyExists)
        );
        s.put_condition("TW02", "group_code", ("6", "FG"), false, "u", today())
            .unwrap();
        assert_eq!(
            s.put_condition("TW02", "group_code", ("1", "RM"), false, "u", today()),
            Err(DomainError::CarryConditionAlreadyExists)
        );
        assert_eq!(
            s.put_condition("TW02", "spec", ("6", "A"), false, "u", today()),
            Err(DomainError::CarryFieldInvalid)
        );
        assert_eq!(
            s.put_default("TW02", "item_no", "X", false, "u", today()),
            Err(DomainError::CarryDefaultFieldInvalid)
        );
        let t = &s.targets[0];
        assert!(t.accepts(|_| "FG01".into()));
        assert!(!t.accepts(|_| "RM01".into()));
    }

    #[test]
    fn basic_data() {
        let mut s = setting("0");
        s.add_target("TW02", "u", today()).unwrap();
        assert_eq!(
            s.put_condition("TW02", "item_no", ("6", "A"), false, "u", today()),
            Err(DomainError::CarryBasicDataNotAllowed)
        );
        assert_eq!(
            s.update_target("TW02", false, true, None, "u", today()),
            Err(DomainError::CarryBasicDataNotAllowed)
        );
        assert!(
            s.update_target("TW02", true, false, None, "u", today())
                .is_ok()
        );
    }
}
