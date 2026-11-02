//! 營運中心（TipTop: aooi901 / azp_file；TipTop 一個營運中心一個資料庫 schema）
//! 新系統採單一 schema + plant_id 欄位隔離，見 docs/adr/0001-architecture.md。
//! 規格：specs/features/aoo/plant_maintenance.feature（P1–P12）
//!
//! 資料庫連線欄位（azp03 schema、azp04 DB link、azp20 MSSQL IP）與上線人數控管（azp09、azp10）不移植（P1、P8）。

use chrono::NaiveDate;

use crate::{DomainError, lang::normalize_text, program::check_length};

/// 欄位上限（TipTop azp_file.sch；與 ttrs_shared::plant_max_len 一致）
pub mod max_len {
    pub const ID: usize = 10; // azp01
    pub const NAME: usize = 80; // azp02
    pub const ADDRESS: usize = 255; // azp051
    pub const PHONE: usize = 40; // azp06
    pub const FAX: usize = 40; // azp07
}

/// 未指定時區時的預設值（P6）
pub const DEFAULT_TIME_ZONE: &str = "Asia/Taipei";

/// 營運中心代號（azp01）：必填、最多 10 碼、轉大寫（P3）；建立後不可修改（P4）
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlantId(String);

impl PlantId {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::PlantIdEmpty);
        }
        if v.chars().count() > max_len::ID {
            return Err(DomainError::PlantIdTooLong { max: max_len::ID });
        }
        Ok(Self(v.to_uppercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 時區（azp052）：IANA 名稱，空白為 Asia/Taipei（P6）；回傳 IANA 標準寫法
pub fn parse_time_zone(raw: Option<&str>) -> Result<String, DomainError> {
    let v = raw.map(str::trim).unwrap_or_default();
    if v.is_empty() {
        return Ok(DEFAULT_TIME_ZONE.to_owned());
    }
    v.parse::<chrono_tz::Tz>()
        .map(|tz| tz.name().to_owned())
        .map_err(|_| DomainError::PlantTimeZoneInvalid(v.to_owned()))
}

/// 可由畫面輸入的欄位
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlantAttrs {
    pub name: String,
    pub time_zone: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub fax: Option<String>,
}

impl PlantAttrs {
    /// 名稱必填（P5）、時區檢核（P6）、地址電話傳真原樣保存（P7）
    pub fn new(
        name: Option<String>,
        time_zone: Option<&str>,
        address: Option<String>,
        phone: Option<String>,
        fax: Option<String>,
    ) -> Result<Self, DomainError> {
        let name = normalize_text(name).ok_or(DomainError::PlantNameEmpty)?;
        check_length("營運中心名稱", Some(&name), max_len::NAME)?;
        let attrs = Self {
            name,
            time_zone: parse_time_zone(time_zone)?,
            address: normalize_text(address),
            phone: normalize_text(phone),
            fax: normalize_text(fax),
        };
        check_length("地址", attrs.address.as_deref(), max_len::ADDRESS)?;
        check_length("電話", attrs.phone.as_deref(), max_len::PHONE)?;
        check_length("傳真", attrs.fax.as_deref(), max_len::FAX)?;
        Ok(attrs)
    }
}

/// 營運中心（azp_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plant {
    pub id: PlantId,
    pub attrs: PlantAttrs,
    /// 資料有效（TipTop azp_file 無此欄，P9）
    pub active: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
}

impl Plant {
    /// 新增：資料有效；資料所有者／部門＝目前使用者（P9、P12）
    pub fn new(
        id: PlantId,
        attrs: PlantAttrs,
        actor: &str,
        actor_dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            id,
            attrs,
            active: true,
            created_by: actor.to_owned(),
            created_dept: actor_dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
        }
    }

    pub fn update(&mut self, attrs: PlantAttrs, actor: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(actor, today);
    }

    /// 停用／啟用：一律允許，既有資料不受影響（P9）
    pub fn set_active(&mut self, active: bool, actor: &str, today: NaiveDate) {
        self.active = active;
        self.touch(actor, today);
    }

    /// 複製：新代號，其餘欄位沿用來源（aooi901.4gl:994-1046，P11）
    pub fn copy_as(&self, id: PlantId, actor: &str, actor_dept: &str, today: NaiveDate) -> Self {
        Self::new(id, self.attrs.clone(), actor, actor_dept, today)
    }

    fn touch(&mut self, actor: &str, today: NaiveDate) {
        self.updated_by = Some(actor.to_owned());
        self.updated_on = Some(today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plant_id_rules() {
        assert_eq!(PlantId::parse(" ").unwrap_err().code(), "PLANT_ID_EMPTY");
        assert_eq!(
            PlantId::parse("PLANT012345").unwrap_err().code(),
            "PLANT_ID_TOO_LONG"
        );
        assert_eq!(PlantId::parse(" cn02 ").unwrap().as_str(), "CN02");
    }

    #[test]
    fn time_zone_rules() {
        assert_eq!(parse_time_zone(None).unwrap(), "Asia/Taipei");
        assert_eq!(parse_time_zone(Some(" ")).unwrap(), "Asia/Taipei");
        assert_eq!(
            parse_time_zone(Some("Asia/Shanghai")).unwrap(),
            "Asia/Shanghai"
        );
        assert_eq!(
            parse_time_zone(Some("Taipei")).unwrap_err().code(),
            "PLANT_TIME_ZONE_INVALID"
        );
    }

    #[test]
    fn name_required() {
        let err = PlantAttrs::new(Some(" ".into()), None, None, None, None).unwrap_err();
        assert_eq!(err.code(), "PLANT_NAME_EMPTY");
    }
}
