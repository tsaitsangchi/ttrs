//! 料件基本資料
//! TipTop 對應：aimi100（料件基本資料維護作業）／ ima_file
//! 規格：specs/features/aim/item_master.feature

use crate::{DomainError, plant::PlantId};

const ITEM_NO_MAX_LEN: usize = 40;

/// 料件編號：去除前後空白、轉大寫，只允許英數與 `-_.`。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemNo(String);

impl ItemNo {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim().to_uppercase();
        if v.is_empty() {
            return Err(DomainError::ItemNoEmpty);
        }
        if v.chars().count() > ITEM_NO_MAX_LEN {
            return Err(DomainError::ItemNoTooLong {
                max: ITEM_NO_MAX_LEN,
            });
        }
        if let Some(c) = v
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        {
            return Err(DomainError::ItemNoInvalidChar(c));
        }
        Ok(Self(v))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStatus {
    Unconfirmed,
    Confirmed,
    Voided,
}

impl ItemStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unconfirmed => "未確認",
            Self::Confirmed => "已確認",
            Self::Voided => "作廢",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub plant_id: PlantId,
    pub item_no: ItemNo,
    pub name: String,
    pub spec: Option<String>,
    pub stock_unit: String,
    pub status: ItemStatus,
}

impl Item {
    /// 新增料件：一律從「未確認」開始。
    pub fn new(
        plant_id: PlantId,
        item_no: ItemNo,
        name: &str,
        spec: Option<&str>,
        stock_unit: &str,
    ) -> Result<Self, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::ItemNameEmpty);
        }
        Ok(Self {
            plant_id,
            item_no,
            name: name.to_owned(),
            spec: spec
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            stock_unit: stock_unit.trim().to_uppercase(),
            status: ItemStatus::Unconfirmed,
        })
    }

    /// 確認：未確認 → 已確認
    pub fn confirm(&mut self) -> Result<(), DomainError> {
        self.transition(ItemStatus::Unconfirmed, ItemStatus::Confirmed, "確認")
    }

    /// 取消確認：已確認 → 未確認（是否已有異動由應用層檢查）
    pub fn unconfirm(&mut self) -> Result<(), DomainError> {
        self.transition(ItemStatus::Confirmed, ItemStatus::Unconfirmed, "取消確認")
    }

    /// 作廢：僅未確認可作廢
    pub fn void(&mut self) -> Result<(), DomainError> {
        self.transition(ItemStatus::Unconfirmed, ItemStatus::Voided, "作廢")
    }

    fn transition(
        &mut self,
        from: ItemStatus,
        to: ItemStatus,
        action: &'static str,
    ) -> Result<(), DomainError> {
        if self.status != from {
            return Err(DomainError::InvalidItemTransition {
                from: self.status.label(),
                action,
            });
        }
        self.status = to;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> Item {
        Item::new(
            PlantId::parse("tw01").unwrap(),
            ItemNo::parse(" a001 ").unwrap(),
            "螺絲 M3",
            None,
            "pcs",
        )
        .unwrap()
    }

    #[test]
    fn item_no_is_normalized() {
        assert_eq!(ItemNo::parse(" ab-01 ").unwrap().as_str(), "AB-01");
    }

    #[test]
    fn item_no_rejects_invalid_char() {
        assert_eq!(
            ItemNo::parse("A 01"),
            Err(DomainError::ItemNoInvalidChar(' '))
        );
    }

    #[test]
    fn new_item_starts_unconfirmed() {
        let i = item();
        assert_eq!(i.status, ItemStatus::Unconfirmed);
        assert_eq!(i.stock_unit, "PCS");
    }

    #[test]
    fn cannot_confirm_twice() {
        let mut i = item();
        i.confirm().unwrap();
        assert_eq!(i.confirm().unwrap_err().code(), "ITEM_INVALID_TRANSITION");
    }

    #[test]
    fn confirmed_item_cannot_be_voided() {
        let mut i = item();
        i.confirm().unwrap();
        assert!(i.void().is_err());
    }
}
