//! 料件分群（TipTop: imz_file 料件分類基本資料檔；aimi110 料件分群碼基本資料維護作業－基本／庫存資料）
//! 規格：specs/features/aim/item_group.feature（IG1–IG12）
//!
//! 第一階段 41 欄（IG3）：主畫面、工程／計劃／庫存、會計／狀態／分群。引用其他主檔的檢查（單位、分類碼、員工）在 use case。

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::{error::DomainError, record::Stamp};

/// 來源碼 imz08 的值域（aimi110.4gl:869-887，mfg1001）
pub const SOURCE_CODES: [&str; 13] = [
    "C", "T", "D", "A", "M", "P", "X", "K", "U", "V", "R", "Z", "S",
];
/// 補貨策略碼 imz37 的值域（:1114-1130，mfg1003）
pub const REPLENISH_CODES: [&str; 6] = ["0", "1", "2", "3", "4", "5"];

/// 料件分群碼：去頭尾空白、區分大小寫、1–10 字
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemGroupCode(String);

impl ItemGroupCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() || v.chars().count() > 10 {
            return Err(DomainError::ItemGroupCodeInvalid { max: 10 });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 第一階段欄位（imz 欄位代號見各欄說明）；代碼欄位存文字，引用檢查在 use case
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemGroupAttrs {
    /// imz02 說明
    pub description: Option<String>,
    /// imz08 來源碼
    pub source_code: Option<String>,
    /// imz25 庫存單位
    pub stock_unit: Option<String>,
    /// imz03 目錄參考號碼
    pub catalog_ref: Option<String>,
    /// imz72 是否需做供應商的管制（預設 0）
    pub supplier_control: String,
    /// imz04 工程圖號
    pub drawing_no: Option<String>,
    /// imz105 軟體物件
    pub software: bool,
    /// imz14 工程料件
    pub engineering: bool,
    /// imz147 插件位置與 QPA 勾稽
    pub insert_qpa_check: bool,
    /// imz107 插件位置
    pub insert_position: bool,
    /// imz109 材料類別（分類碼類別 8）
    pub material_category: Option<String>,
    /// imz903 聯產品入庫
    pub coproduct: bool,
    /// imz24 檢驗否
    pub inspection: bool,
    /// imz911 重覆性生產
    pub repetitive: bool,
    /// imz07 ABC 碼
    pub abc_code: Option<String>,
    /// imz70 消耗料件
    pub consumable: bool,
    /// imz37 補貨策略碼
    pub replenish_policy: Option<String>,
    /// imz38 再補貨點
    pub reorder_point: Option<Decimal>,
    /// imz99 再補貨量
    pub reorder_qty: Option<Decimal>,
    /// imz27 安全庫存量
    pub safety_stock: Option<Decimal>,
    /// imz28 安全庫存期間
    pub safety_period: Option<Decimal>,
    /// imz71 儲存有效天數
    pub shelf_life_days: Option<i32>,
    /// imz909 MRP 匯總時距（天）
    pub mrp_bucket_days: Option<i32>,
    /// imz35 主要倉庫別（倉庫未移植，不檢查，IG4）
    pub warehouse: Option<String>,
    /// imz36 主要儲位別
    pub location: Option<String>,
    /// imz23 倉管員（本營運中心的員工）
    pub stock_keeper: Option<String>,
    /// imz17 銷售統計單位
    pub sales_stat_unit: Option<String>,
    /// imz907 第二單位（IG6：單一單位，保留不檢查換算）
    pub second_unit: Option<String>,
    /// imz908 計價單位
    pub pricing_unit: Option<String>,
    /// imz09–imz12 其他分群碼一～四（分類碼類別 D／E／F／G）
    pub other_groups: [Option<String>; 4],
    /// imz39 料件所屬會計科目（會計科目未移植，不檢查）
    pub account: Option<String>,
    /// imz391 會計科目二
    pub account2: Option<String>,
    /// imz15 保稅與否
    pub bonded: bool,
    /// imz19 保稅料件進出口分類統計用類
    pub bonded_class: Option<String>,
    /// imz21 保稅料件稅則編號
    pub tariff_no: Option<String>,
    /// imz106 保稅料件型態
    pub bonded_type: Option<String>,
    /// imz131 產品分類（料件帶入預設值用，IM24）
    pub product_class: Option<String>,
    /// imz51 經濟訂購量（IM24）
    pub economic_order_qty: Option<Decimal>,
    /// imz52 平均訂購量（IM24）
    pub average_order_qty: Option<Decimal>,
}

fn clean(v: &mut Option<String>, field: &'static str, max: usize) -> Result<(), DomainError> {
    *v = v
        .take()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    if v.as_ref().is_some_and(|s| s.chars().count() > max) {
        return Err(DomainError::FieldTooLong { field, max });
    }
    Ok(())
}

fn non_negative<T: PartialOrd + Default>(v: Option<T>) -> Result<(), DomainError> {
    match v {
        Some(x) if x < T::default() => Err(DomainError::ItemGroupQuantityInvalid),
        _ => Ok(()),
    }
}

impl ItemGroupAttrs {
    /// 長度、值域與不可小於 0 的檢查（IG5）；回傳整理過空白的欄位
    pub fn validated(mut self) -> Result<Self, DomainError> {
        clean(&mut self.description, "說明", 80)?;
        clean(&mut self.source_code, "來源碼", 1)?;
        clean(&mut self.stock_unit, "庫存單位", 4)?;
        clean(&mut self.catalog_ref, "目錄參考號碼", 20)?;
        clean(&mut self.drawing_no, "工程圖號", 20)?;
        clean(&mut self.material_category, "材料類別", 10)?;
        clean(&mut self.abc_code, "ABC 碼", 1)?;
        clean(&mut self.replenish_policy, "補貨策略碼", 1)?;
        clean(&mut self.warehouse, "主要倉庫別", 10)?;
        clean(&mut self.location, "主要儲位別", 10)?;
        clean(&mut self.stock_keeper, "倉管員", 10)?;
        clean(&mut self.sales_stat_unit, "銷售統計單位", 4)?;
        clean(&mut self.second_unit, "第二單位", 4)?;
        clean(&mut self.pricing_unit, "計價單位", 4)?;
        for g in &mut self.other_groups {
            clean(g, "其他分群碼", 10)?;
        }
        clean(&mut self.account, "會計科目", 24)?;
        clean(&mut self.account2, "會計科目二", 24)?;
        clean(&mut self.bonded_class, "保稅進出口分類", 4)?;
        clean(&mut self.tariff_no, "保稅稅則編號", 4)?;
        clean(&mut self.bonded_type, "保稅料件型態", 1)?;
        clean(&mut self.product_class, "產品分類", 10)?;
        let sc = self.supplier_control.trim().to_owned();
        self.supplier_control = if sc.is_empty() { "0".to_owned() } else { sc };
        if self.supplier_control.chars().count() > 1 {
            return Err(DomainError::FieldTooLong {
                field: "供應商管制",
                max: 1,
            });
        }
        if let Some(s) = &self.source_code
            && !SOURCE_CODES.contains(&s.as_str())
        {
            return Err(DomainError::ItemGroupSourceInvalid(s.clone()));
        }
        if let Some(a) = &self.abc_code
            && !["A", "B", "C"].contains(&a.as_str())
        {
            return Err(DomainError::ItemGroupAbcInvalid(a.clone()));
        }
        if let Some(r) = &self.replenish_policy
            && !REPLENISH_CODES.contains(&r.as_str())
        {
            return Err(DomainError::ItemGroupReplenishInvalid(r.clone()));
        }
        non_negative(self.reorder_point)?;
        non_negative(self.reorder_qty)?;
        non_negative(self.safety_stock)?;
        non_negative(self.safety_period)?;
        non_negative(self.shelf_life_days)?;
        non_negative(self.mrp_bucket_days)?;
        non_negative(self.economic_order_qty)?;
        non_negative(self.average_order_qty)?;
        Ok(self)
    }

    /// 引用的單位（刪除單位時檢查用）
    pub fn units(&self) -> impl Iterator<Item = &str> {
        [
            &self.stock_unit,
            &self.sales_stat_unit,
            &self.second_unit,
            &self.pricing_unit,
        ]
        .into_iter()
        .filter_map(|u| u.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemGroup {
    pub code: ItemGroupCode,
    pub attrs: ItemGroupAttrs,
    /// imzacti 資料有效碼
    pub active: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl ItemGroup {
    pub fn new(
        code: ItemGroupCode,
        attrs: ItemGroupAttrs,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            code,
            attrs,
            active: true,
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn update(&mut self, attrs: ItemGroupAttrs, user: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(user, today);
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製（IG9）：新代碼、全部欄位，所有者為目前使用者
    pub fn copy_as(&self, code: ItemGroupCode, user: &str, dept: &str, today: NaiveDate) -> Self {
        Self::new(code, self.attrs.clone(), user, dept, today)
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
    fn value_lists() {
        let ok = ItemGroupAttrs {
            source_code: Some("P".into()),
            ..Default::default()
        };
        assert!(ok.validated().is_ok());
        let bad = ItemGroupAttrs {
            source_code: Some("Q".into()),
            ..Default::default()
        };
        assert_eq!(
            bad.validated(),
            Err(DomainError::ItemGroupSourceInvalid("Q".into()))
        );
        let neg = ItemGroupAttrs {
            safety_stock: Some(Decimal::NEGATIVE_ONE),
            ..Default::default()
        };
        assert_eq!(neg.validated(), Err(DomainError::ItemGroupQuantityInvalid));
    }
}
