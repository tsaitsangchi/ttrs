//! 單位換算（TipTop: smc_file 單位換算資料檔／aooi102、smd_file 料件單位換算資料檔／aooi103；換算率 s_umfchk）
//! 規格：specs/features/aoo/unit_conversion.feature（UC1–UC12）
//!
//! 一組單位只存一筆，A→B 與 B→A 視為同一組，取換算率時兩個方向都找（UC3）。換算率＝目的數量÷來源數量（UC7）。

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::{DomainError, confirmation::Confirmation, item::ItemNo, record::Stamp, unit::UnitCode};

/// 數量小數位數（smc03、smc04 number(15,3)）
pub const QUANTITY_SCALE: u32 = 3;
/// 換算率保留的小數位數（s_umfchk l_factor num26_10）
pub const FACTOR_SCALE: u32 = 10;
/// 說明最多 80 字（smc05）
pub const NOTE_MAX: usize = 80;

/// 一組換算：來源單位、目的單位與兩邊的兌換數量
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    pub from_unit: UnitCode,
    pub to_unit: UnitCode,
    pub from_quantity: Decimal,
    pub to_quantity: Decimal,
    /// smc05／smd05 說明
    pub note: Option<String>,
}

impl Conversion {
    /// 檢查（UC4、UC5）：數量須大於 0、最多 3 位小數；來源與目的單位不可相同
    pub fn new(
        from_unit: UnitCode,
        to_unit: UnitCode,
        from_quantity: Decimal,
        to_quantity: Decimal,
        note: Option<String>,
    ) -> Result<Self, DomainError> {
        if from_unit == to_unit {
            return Err(DomainError::UnitConversionSameUnit);
        }
        for q in [from_quantity, to_quantity] {
            if q <= Decimal::ZERO || q.round_dp(QUANTITY_SCALE) != q {
                return Err(DomainError::UnitConversionQuantityInvalid);
            }
        }
        let note = note.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
        if note.as_ref().is_some_and(|n| n.chars().count() > NOTE_MAX) {
            return Err(DomainError::FieldTooLong {
                field: "說明",
                max: NOTE_MAX,
            });
        }
        Ok(Self {
            from_unit,
            to_unit,
            from_quantity,
            to_quantity,
            note,
        })
    }

    /// 是否為同一組單位（不分方向，UC3）
    pub fn same_pair(&self, a: &UnitCode, b: &UnitCode) -> bool {
        (&self.from_unit == a && &self.to_unit == b) || (&self.from_unit == b && &self.to_unit == a)
    }

    /// 由 `from` 換到 `to` 的換算率（s_umfchk：目的數量÷來源數量；反方向時倒過來）；不是這組單位回 None
    pub fn factor(&self, from: &UnitCode, to: &UnitCode) -> Option<Decimal> {
        let (s, t) = if &self.from_unit == from && &self.to_unit == to {
            (self.from_quantity, self.to_quantity)
        } else if &self.from_unit == to && &self.to_unit == from {
            (self.to_quantity, self.from_quantity)
        } else {
            return None;
        };
        Some((t / s).round_dp(FACTOR_SCALE).normalize())
    }
}

/// 一般換算（smc_file 一列；共用，UC2）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitConversion {
    pub conversion: Conversion,
    /// smcacti 資料有效碼
    pub active: bool,
    /// 確認狀態（ADR 0013 SA9；smc_file 無此欄）
    pub confirmation: Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl UnitConversion {
    /// 新增：資料有效（aooi102.4gl:311）
    pub fn new(conversion: Conversion, user: &str, dept: &str, today: NaiveDate) -> Self {
        Self {
            conversion,
            active: true,
            confirmation: Confirmation::default(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

/// 料件換算（smd_file 一列；工廠層，UC2）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemUnitConversion {
    /// smd01 料件編號
    pub item: ItemNo,
    pub conversion: Conversion,
    /// smdacti 資料有效碼
    pub active: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl ItemUnitConversion {
    pub fn new(
        item: ItemNo,
        conversion: Conversion,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            item,
            conversion,
            active: true,
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

/// 料號以 MISC 開頭的料件不換算（s_umfchk.4gl：p_item[1,4]='MISC'，IM20）
pub fn is_misc(item: &str) -> bool {
    item.starts_with("MISC")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> UnitCode {
        UnitCode::parse(s).unwrap()
    }

    fn d(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn factor_both_directions() {
        let c = Conversion::new(u("KG"), u("G"), d("1"), d("1000"), None).unwrap();
        assert_eq!(c.factor(&u("KG"), &u("G")), Some(d("1000")));
        assert_eq!(c.factor(&u("G"), &u("KG")), Some(d("0.001")));
        assert_eq!(c.factor(&u("KG"), &u("BOX")), None);
        let c = Conversion::new(u("BOX"), u("PCS"), d("3"), d("1"), None).unwrap();
        assert_eq!(c.factor(&u("BOX"), &u("PCS")), Some(d("0.3333333333")));
    }

    #[test]
    fn checks() {
        let e = |a: &str, b: &str, x: &str, y: &str| {
            Conversion::new(u(a), u(b), d(x), d(y), None)
                .unwrap_err()
                .code()
        };
        assert_eq!(e("KG", "KG", "1", "1"), "UNIT_CONVERSION_SAME_UNIT");
        assert_eq!(e("KG", "G", "0", "1"), "UNIT_CONVERSION_QUANTITY_INVALID");
        assert_eq!(
            e("KG", "G", "1", "0.0001"),
            "UNIT_CONVERSION_QUANTITY_INVALID"
        );
        assert!(is_misc("MISC-01"));
    }
}
