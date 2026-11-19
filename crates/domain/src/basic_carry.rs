//! 基本資料拋轉（TipTop: aoop603 基本資料拋轉作業、s_basic_data_carry）與拋轉紀錄（gex_file；aooq604）
//! 規格：specs/features/aoo/basic_data_carry.feature（BC1–BC12）

use chrono::{DateTime, Utc};

use crate::{
    error::DomainError,
    group_center::{DataType, GroupCenterCode},
};

/// 可拋轉的基本資料（BC3），依拋轉順序排列（BC4：後面的會參照前面的資料）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BasicItem {
    Department,
    Employee,
    MaterialCategory,
    OtherGroup1,
    OtherGroup2,
    OtherGroup3,
    OtherGroup4,
    ProductClass,
    ItemGroup,
}

impl BasicItem {
    pub const ALL: [Self; 9] = [
        Self::Department,
        Self::Employee,
        Self::MaterialCategory,
        Self::OtherGroup1,
        Self::OtherGroup2,
        Self::OtherGroup3,
        Self::OtherGroup4,
        Self::ProductClass,
        Self::ItemGroup,
    ];

    /// API 與翻譯用的代號
    pub fn code(self) -> &'static str {
        match self {
            Self::Department => "department",
            Self::Employee => "employee",
            Self::MaterialCategory => "material_category",
            Self::OtherGroup1 => "other_group_1",
            Self::OtherGroup2 => "other_group_2",
            Self::OtherGroup3 => "other_group_3",
            Self::OtherGroup4 => "other_group_4",
            Self::ProductClass => "product_class",
            Self::ItemGroup => "item_group",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        Self::ALL
            .into_iter()
            .find(|i| i.code() == raw.trim())
            .ok_or_else(|| DomainError::BasicCarryItemInvalid(raw.trim().to_owned()))
    }

    /// TipTop 作業代號（拋轉紀錄 gex04，aoop603.4gl:332-418）
    pub fn program(self) -> &'static str {
        match self {
            Self::Department => "aooi030",
            Self::Employee => "aooi040",
            Self::MaterialCategory => "aooi305",
            Self::OtherGroup1 => "aooi309",
            Self::OtherGroup2 => "aooi310",
            Self::OtherGroup3 => "aooi311",
            Self::OtherGroup4 => "aooi312",
            Self::ProductClass => "axmi110",
            Self::ItemGroup => "aimi110",
        }
    }

    /// 分類碼類別（azf02）；不是分類碼時為 None
    pub fn code_kind(self) -> Option<&'static str> {
        match self {
            Self::MaterialCategory => Some("8"),
            Self::OtherGroup1 => Some("D"),
            Self::OtherGroup2 => Some("E"),
            Self::OtherGroup3 => Some("F"),
            Self::OtherGroup4 => Some("G"),
            _ => None,
        }
    }
}

/// 拋轉動作：新增或覆蓋（略過的不記錄，BC7）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarryAction {
    Insert,
    Update,
}

impl CarryAction {
    pub fn code(self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Update => "update",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "insert" => Some(Self::Insert),
            "update" => Some(Self::Update),
            _ => None,
        }
    }
}

/// 拋轉紀錄一筆（gex_file；BC7）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarryLogEntry {
    /// gex01 資料中心
    pub center: GroupCenterCode,
    /// gex02 資料類別
    pub data_type: DataType,
    /// gex03 拋轉營運中心
    pub plant: String,
    /// gex04 程式代號（TipTop 作業代號）
    pub program: String,
    /// gex05 拋轉主鍵值
    pub record_key: String,
    pub action: CarryAction,
    /// gex07＋gex08 拋轉日期與時間
    pub carried_at: DateTime<Utc>,
    /// gex09 拋轉人員
    pub carried_by: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items() {
        assert_eq!(BasicItem::parse("employee"), Ok(BasicItem::Employee));
        assert!(BasicItem::parse("currency").is_err());
        let mut v = vec![
            BasicItem::ItemGroup,
            BasicItem::Department,
            BasicItem::Employee,
        ];
        v.sort();
        assert_eq!(
            v,
            [
                BasicItem::Department,
                BasicItem::Employee,
                BasicItem::ItemGroup
            ]
        );
        assert_eq!(BasicItem::OtherGroup3.code_kind(), Some("F"));
        assert_eq!(BasicItem::ProductClass.program(), "axmi110");
    }
}
