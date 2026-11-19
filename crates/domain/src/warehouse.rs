//! 倉庫與儲位（TipTop: imd_file 倉庫別資料主檔、ime_file 倉庫／存放位置資料主檔；aimi201 倉儲別資料維護、aimi200 倉庫別資料維護）
//! 規格：specs/features/aim/warehouse.feature（WH1–WH13）

use chrono::NaiveDate;

use crate::{department::DeptCode, error::DomainError, record::Stamp};

/// 倉庫別、儲位最多 10 字（imd01、ime02 varchar2(10)）
pub const CODE_MAX: usize = 10;
/// 名稱最多 80 字（imd02、ime03）
pub const NAME_MAX: usize = 80;
/// 地址最多 255 字（imd03）
pub const ADDRESS_MAX: usize = 255;
/// 負責人、電話最多 40 字（imd06、imd07）
pub const CONTACT_MAX: usize = 40;
/// 會計科目最多 24 字（imd08、imd081、ime09、ime091）
pub const ACCOUNT_MAX: usize = 24;

fn code_of(raw: &str, empty: DomainError, too_long: DomainError) -> Result<String, DomainError> {
    let v = raw.trim();
    if v.is_empty() {
        return Err(empty);
    }
    if v.chars().count() > CODE_MAX {
        return Err(too_long);
    }
    Ok(v.to_owned())
}

/// 倉庫別：去頭尾空白、區分大小寫、1–10 字（WH3(a)）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WarehouseCode(String);

impl WarehouseCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        code_of(
            raw,
            DomainError::WarehouseCodeEmpty,
            DomainError::WarehouseCodeTooLong { max: CODE_MAX },
        )
        .map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 儲位：去頭尾空白、區分大小寫、1–10 字；不建空白儲位（WH4(a)(b)）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocationCode(String);

impl LocationCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        code_of(
            raw,
            DomainError::WarehouseLocationCodeEmpty,
            DomainError::WarehouseLocationCodeTooLong { max: CODE_MAX },
        )
        .map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 倉儲類別 imd10／ime04：S 一般性倉庫、W 在製品倉庫
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageClass {
    Store,
    Wip,
}

impl StorageClass {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        match raw.trim().to_uppercase().as_str() {
            "S" => Ok(Self::Store),
            "W" => Ok(Self::Wip),
            _ => Err(DomainError::WarehouseClassInvalid),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Store => "S",
            Self::Wip => "W",
        }
    }
}

/// 倉庫與儲位共有、由倉庫帶到儲位的設定（imd10–imd15 ↔ ime04–ime07、ime10、ime11）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageFlags {
    pub class: StorageClass,
    /// 可用倉儲
    pub usable: bool,
    /// MRP 可用倉儲
    pub mrp_usable: bool,
    /// 保稅
    pub bonded: bool,
    /// 生產發料優先順序
    pub issue_priority: i32,
    /// 銷售出貨優先順序
    pub ship_priority: i32,
}

impl StorageFlags {
    /// 優先順序不可小於 0（aim1007、aim-223）；不可用倉儲必定 MRP 不可用（.sch 說明，WH3(b)）
    pub fn new(
        class: StorageClass,
        usable: bool,
        mrp_usable: bool,
        bonded: bool,
        issue_priority: i32,
        ship_priority: i32,
    ) -> Result<Self, DomainError> {
        if issue_priority < 0 || ship_priority < 0 {
            return Err(DomainError::WarehousePriorityInvalid);
        }
        if !usable && mrp_usable {
            return Err(DomainError::WarehouseMrpRequiresUsable);
        }
        Ok(Self {
            class,
            usable,
            mrp_usable,
            bonded,
            issue_priority,
            ship_priority,
        })
    }
}

fn text(
    raw: Option<String>,
    max: usize,
    field: &'static str,
) -> Result<Option<String>, DomainError> {
    let v = raw.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    if v.as_ref().is_some_and(|s| s.chars().count() > max) {
        return Err(DomainError::FieldTooLong { field, max });
    }
    Ok(v)
}

/// 倉庫可修改的欄位（WH3）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarehouseAttrs {
    pub name: Option<String>,
    /// imd03 地址
    pub address: Option<String>,
    /// imd06 負責人
    pub contact: Option<String>,
    /// imd07 電話
    pub phone: Option<String>,
    /// imd08、imd081 存貨科目一、二（會計科目未移植，不檢查）
    pub account: Option<String>,
    pub account2: Option<String>,
    /// imd09 成本庫別（用途待確認，照存）
    pub cost_group: Option<String>,
    /// imd16 成本中心（成本／利潤中心部門，由 use case 檢查）
    pub cost_center: Option<DeptCode>,
    pub flags: StorageFlags,
}

impl WarehouseAttrs {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: Option<String>,
        address: Option<String>,
        contact: Option<String>,
        phone: Option<String>,
        account: Option<String>,
        account2: Option<String>,
        cost_group: Option<String>,
        cost_center: Option<String>,
        flags: StorageFlags,
    ) -> Result<Self, DomainError> {
        let cost_group = text(cost_group, 1, "成本庫別")?;
        Ok(Self {
            name: text(name, NAME_MAX, "名稱")?,
            address: text(address, ADDRESS_MAX, "地址")?,
            contact: text(contact, CONTACT_MAX, "負責人")?,
            phone: text(phone, CONTACT_MAX, "電話")?,
            account: text(account, ACCOUNT_MAX, "存貨科目")?,
            account2: text(account2, ACCOUNT_MAX, "存貨科目二")?,
            cost_group,
            cost_center: text(cost_center, CODE_MAX, "成本中心")?
                .map(|c| DeptCode::parse(&c))
                .transpose()?,
            flags,
        })
    }
}

/// 會計科目自動顯示方式 ime08：0 自行輸入、1 用料件科目、2 用儲位科目（WH4(d)）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountMethod {
    Manual,
    Item,
    Location,
}

impl AccountMethod {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        match raw.trim() {
            "" | "0" => Ok(Self::Manual),
            "1" => Ok(Self::Item),
            "2" => Ok(Self::Location),
            _ => Err(DomainError::WarehouseAccountMethodInvalid),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Manual => "0",
            Self::Item => "1",
            Self::Location => "2",
        }
    }
}

/// 儲位一列（ime_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub code: LocationCode,
    pub name: Option<String>,
    pub flags: StorageFlags,
    pub account_method: AccountMethod,
    /// ime09、ime091 儲位科目一、二（方式 2 才有）
    pub account: Option<String>,
    pub account2: Option<String>,
}

impl Location {
    /// 科目方式 2 才可輸入科目且科目一必填（mfg9043）；其他方式清空科目（:1760-1764）
    pub fn new(
        code: LocationCode,
        name: Option<String>,
        flags: StorageFlags,
        account_method: AccountMethod,
        account: Option<String>,
        account2: Option<String>,
    ) -> Result<Self, DomainError> {
        let name = text(name, NAME_MAX, "儲位名稱")?;
        let (account, account2) = if account_method == AccountMethod::Location {
            let a = text(account, ACCOUNT_MAX, "儲位科目")?
                .ok_or(DomainError::WarehouseLocationAccountRequired)?;
            (Some(a), text(account2, ACCOUNT_MAX, "儲位科目二")?)
        } else {
            (None, None)
        };
        Ok(Self {
            code,
            name,
            flags,
            account_method,
            account,
            account2,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warehouse {
    pub code: WarehouseCode,
    pub attrs: WarehouseAttrs,
    /// 儲位，依儲位代號排序
    pub locations: Vec<Location>,
    /// imdacti 資料有效碼
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；imd_file 無此欄）
    pub confirmation: crate::confirmation::Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl Warehouse {
    pub fn new(
        code: WarehouseCode,
        attrs: WarehouseAttrs,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            code,
            attrs,
            locations: Vec::new(),
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

    /// 停用的倉庫不可修改、不可維護儲位（mfg1000，WH6）
    pub fn check_active(&self) -> Result<(), DomainError> {
        if self.active {
            Ok(())
        } else {
            Err(DomainError::WarehouseInactive)
        }
    }

    /// 修改倉庫；倉儲類別、可用、MRP、保稅、優先順序改變時同步改全部儲位（s_upd_ime_img，WH5）
    pub fn update(
        &mut self,
        attrs: WarehouseAttrs,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        if attrs.flags != self.attrs.flags {
            let (old, new) = (&self.attrs.flags, &attrs.flags);
            for l in &mut self.locations {
                if old.class != new.class {
                    l.flags.class = new.class;
                }
                if old.usable != new.usable {
                    l.flags.usable = new.usable;
                }
                if old.mrp_usable != new.mrp_usable {
                    l.flags.mrp_usable = new.mrp_usable;
                }
                if old.bonded != new.bonded {
                    l.flags.bonded = new.bonded;
                }
                if old.issue_priority != new.issue_priority {
                    l.flags.issue_priority = new.issue_priority;
                }
                if old.ship_priority != new.ship_priority {
                    l.flags.ship_priority = new.ship_priority;
                }
                // 倉庫改成不可用時，儲位的 MRP 可用也跟著改（WH3(b)）
                if !l.flags.usable {
                    l.flags.mrp_usable = false;
                }
            }
        }
        self.attrs = attrs;
        self.touch(user, today);
        Ok(())
    }

    /// 新增儲位；同一倉庫內不可重複（-239）
    pub fn add_location(
        &mut self,
        location: Location,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        if self.locations.iter().any(|l| l.code == location.code) {
            return Err(DomainError::WarehouseLocationAlreadyExists);
        }
        self.locations.push(location);
        self.locations.sort_by(|a, b| a.code.cmp(&b.code));
        self.touch(user, today);
        Ok(())
    }

    /// 新增儲位時從倉庫帶入的設定（:1694-1705，WH4(c)）
    pub fn default_flags(&self) -> StorageFlags {
        self.attrs.flags.clone()
    }

    pub fn update_location(
        &mut self,
        location: Location,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        let row = self
            .locations
            .iter_mut()
            .find(|l| l.code == location.code)
            .ok_or(DomainError::WarehouseLocationNotFound)?;
        *row = location;
        self.touch(user, today);
        Ok(())
    }

    pub fn remove_location(
        &mut self,
        code: &LocationCode,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        let before = self.locations.len();
        self.locations.retain(|l| &l.code != code);
        if self.locations.len() == before {
            return Err(DomainError::WarehouseLocationNotFound);
        }
        self.touch(user, today);
        Ok(())
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製倉庫與儲位（:2554-2643）：所有者為目前使用者、資料有效、未確認
    pub fn copy_as(&self, code: WarehouseCode, user: &str, dept: &str, today: NaiveDate) -> Self {
        let mut w = Self::new(code, self.attrs.clone(), user, dept, today);
        w.locations = self.locations.clone();
        w
    }

    /// 刪除前：停用的不可刪除（aim-009）、已確認的不可刪除（SA9-2）
    pub fn check_deletable(&self) -> Result<(), DomainError> {
        if !self.active {
            return Err(DomainError::WarehouseInactive);
        }
        self.confirmation.check_deletable()
    }

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

    /// 修改單身時一併更新倉庫的修改者與日期（WH10）
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

    fn flags() -> StorageFlags {
        StorageFlags::new(StorageClass::Store, true, true, false, 0, 0).unwrap()
    }

    fn wh() -> Warehouse {
        let attrs =
            WarehouseAttrs::new(None, None, None, None, None, None, None, None, flags()).unwrap();
        Warehouse::new(
            WarehouseCode::parse("W01").unwrap(),
            attrs,
            "u",
            "d",
            today(),
        )
    }

    #[test]
    fn flag_rules() {
        assert_eq!(
            StorageFlags::new(StorageClass::Store, false, true, false, 0, 0),
            Err(DomainError::WarehouseMrpRequiresUsable)
        );
        assert_eq!(
            StorageFlags::new(StorageClass::Store, true, true, false, -1, 0),
            Err(DomainError::WarehousePriorityInvalid)
        );
        assert_eq!(
            StorageClass::parse("x"),
            Err(DomainError::WarehouseClassInvalid)
        );
    }

    #[test]
    fn propagate_to_locations() {
        let mut w = wh();
        let loc = Location::new(
            LocationCode::parse("A-01").unwrap(),
            None,
            w.default_flags(),
            AccountMethod::Manual,
            None,
            None,
        )
        .unwrap();
        w.add_location(loc.clone(), "u", today()).unwrap();
        assert_eq!(
            w.add_location(loc, "u", today()),
            Err(DomainError::WarehouseLocationAlreadyExists)
        );
        let mut attrs = w.attrs.clone();
        attrs.flags = StorageFlags::new(StorageClass::Wip, true, true, true, 0, 0).unwrap();
        w.update(attrs, "u", today()).unwrap();
        assert_eq!(w.locations[0].flags.class, StorageClass::Wip);
        assert!(w.locations[0].flags.bonded);
    }

    #[test]
    fn location_accounts() {
        let code = || LocationCode::parse("A").unwrap();
        assert_eq!(
            Location::new(code(), None, flags(), AccountMethod::Location, None, None),
            Err(DomainError::WarehouseLocationAccountRequired)
        );
        let l = Location::new(
            code(),
            None,
            flags(),
            AccountMethod::Manual,
            Some("1".into()),
            None,
        )
        .unwrap();
        assert_eq!(l.account, None);
    }
}
