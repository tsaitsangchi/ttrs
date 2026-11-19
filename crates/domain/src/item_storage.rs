//! Item Storage 料件預設倉儲（TipTop aimi202 / imf_file；specs/features/aim/item_storage.feature）
//! 每個料件可指定多組預設存放的倉庫＋儲位與最高存量。儲位空白表示整個倉庫（IS3(b)）。

use rust_decimal::Decimal;

use crate::{
    error::DomainError,
    item::ItemNo,
    record::Stamp,
    warehouse::{LocationCode, WarehouseCode},
};

/// 最高存量小數位數（imf04 number(15,3)）
pub const MAX_QTY_SCALE: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStorage {
    /// imf01 料件編號
    pub item: ItemNo,
    /// imf02 倉庫
    pub warehouse: WarehouseCode,
    /// imf03 儲位；None 為整個倉庫（TipTop 存 ' '）
    pub location: Option<LocationCode>,
    /// imf04 最高存量；None 為不限制（TipTop 999999999，IS3(d)）
    pub max_qty: Option<Decimal>,
    pub stamp: Stamp,
}

impl ItemStorage {
    /// 最高存量不可小於 0、最多 3 位小數（IS3(d)）
    pub fn new(
        item: ItemNo,
        warehouse: WarehouseCode,
        location: Option<LocationCode>,
        max_qty: Option<Decimal>,
    ) -> Result<Self, DomainError> {
        if let Some(q) = max_qty
            && (q < Decimal::ZERO || q.normalize().scale() > MAX_QTY_SCALE)
        {
            return Err(DomainError::ItemStorageMaxQtyInvalid);
        }
        Ok(Self {
            item,
            warehouse,
            location,
            max_qty,
            stamp: Stamp::default(),
        })
    }

    /// 儲位代號；整個倉庫時為空字串（資料庫主鍵存 ''）
    pub fn location_code(&self) -> &str {
        self.location.as_ref().map_or("", |l| l.as_str())
    }

    /// 同一組倉庫＋儲位（IS3(c)）
    pub fn same_place(&self, warehouse: &WarehouseCode, location: Option<&LocationCode>) -> bool {
        &self.warehouse == warehouse && self.location.as_ref() == location
    }
}

/// 畫面或 API 傳來的儲位：空白為整個倉庫
pub fn parse_location(raw: &str) -> Result<Option<LocationCode>, DomainError> {
    if raw.trim().is_empty() {
        Ok(None)
    } else {
        LocationCode::parse(raw).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wh() -> WarehouseCode {
        WarehouseCode::parse("W01").unwrap()
    }

    #[test]
    fn negative_or_too_precise_max_qty_is_rejected() {
        let item = ItemNo::parse("A001").unwrap();
        for q in ["-1", "1.2345"] {
            let r = ItemStorage::new(item.clone(), wh(), None, Some(q.parse().unwrap()));
            assert_eq!(r.unwrap_err(), DomainError::ItemStorageMaxQtyInvalid);
        }
        assert!(ItemStorage::new(item.clone(), wh(), None, Some("1.500".parse().unwrap())).is_ok());
        assert!(ItemStorage::new(item, wh(), None, None).is_ok());
    }

    #[test]
    fn blank_location_is_whole_warehouse() {
        assert_eq!(parse_location("  ").unwrap(), None);
        assert_eq!(parse_location("A-01").unwrap().unwrap().as_str(), "A-01");
    }
}
