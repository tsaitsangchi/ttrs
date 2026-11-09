//! 料件基本資料（TipTop: ima_file 料件資料主檔；aimi100 料件基本資料建立，以客製 topcust/cim 為準）
//! 規格：specs/features/aim/item_maintenance.feature（IM1–IM31、情境 IM-1～IM-24）
//!
//! 第一階段 71 欄（IM1）。值域與長度在此檢查；引用其他主檔（分群、單位、產品分類、分類碼、員工、部門）在 use case。

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::{error::DomainError, item_group::ItemGroupAttrs, record::Stamp};

/// 料件編號長度上限（ima01 varchar2(40)）
pub const ITEM_NO_MAX_LEN: usize = 40;
/// 料件編號不可含的字元（aimi100.4gl:6174-6184，aim-122）
pub const FORBIDDEN_CHARS: [char; 10] = ['*', ':', '|', '?', '!', '%', '&', '^', '<', '>'];
/// 來源碼 ima08 的值域（mfg1001）
pub const SOURCE_CODES: [&str; 13] = [
    "C", "T", "D", "A", "M", "P", "X", "K", "U", "V", "R", "Z", "S",
];
/// 補貨策略碼 ima37（IM23：只允許 0–5）
pub const REPLENISH_CODES: [&str; 6] = ["0", "1", "2", "3", "4", "5"];
/// 補貨策略 0（再訂購點）、5（期間採購）可用的來源碼（:6482-6488，mfg3201）
const REORDER_SOURCES: [&str; 5] = ["M", "S", "P", "V", "Z"];
/// 新料件處理狀況初始值（ima93）
pub const NEW_PART_STATUS: &str = "NNNNNNNN";
/// 新增料件的低階碼（ima16）
pub const NEW_LOW_LEVEL_CODE: i32 = 99;

/// 料件編號：去頭尾空白、不轉大寫（IM21）、最多 40 字、不可含 `* : | ? ! % & ^ < >`
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemNo(String);

impl ItemNo {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::ItemNoEmpty);
        }
        if v.chars().count() > ITEM_NO_MAX_LEN {
            return Err(DomainError::ItemNoTooLong {
                max: ITEM_NO_MAX_LEN,
            });
        }
        if let Some(c) = v.chars().find(|c| FORBIDDEN_CHARS.contains(c)) {
            return Err(DomainError::ItemNoInvalidChar(c));
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// MISC 開頭的料件：來源碼一律 Z（:6196-6199）
    pub fn is_misc(&self) -> bool {
        self.0.starts_with("MISC")
    }

    /// MISC 之後還有字（如 MISC01）：須先有 MISC 料件（aim-806）
    pub fn needs_misc_master(&self) -> bool {
        self.is_misc() && self.0.chars().count() > 4
    }
}

/// imaacti 資料有效碼（IM27）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveCode {
    /// P 處理中
    Processing,
    /// Y 已確認
    Confirmed,
    /// N 無效
    Invalid,
}

impl ActiveCode {
    pub fn code(self) -> &'static str {
        match self {
            Self::Processing => "P",
            Self::Confirmed => "Y",
            Self::Invalid => "N",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "P" => Self::Processing,
            "Y" => Self::Confirmed,
            "N" => Self::Invalid,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Processing => "處理中",
            Self::Confirmed => "已確認",
            Self::Invalid => "無效",
        }
    }
}

/// ima1010 狀態碼
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCode {
    /// 0 開立
    Open,
    /// 1 確認
    Confirmed,
}

impl StatusCode {
    pub fn code(self) -> &'static str {
        match self {
            Self::Open => "0",
            Self::Confirmed => "1",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "0" => Self::Open,
            "1" => Self::Confirmed,
            _ => return None,
        })
    }
}

/// 可輸入的欄位（ima 欄位代號見各欄；shared::ItemInput 對應）
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemAttrs {
    /// ima02 品名（IM22：必填）
    pub name: Option<String>,
    /// ima021 規格
    pub spec: Option<String>,
    /// ima06 分群碼
    pub group_code: Option<String>,
    /// ima08 來源碼（預設 P）
    pub source_code: Option<String>,
    /// ima13 規格組件料件編號（來源碼 T 才有）
    pub configurable_item_no: Option<String>,
    /// ima05 目前使用版本
    pub current_version: Option<String>,
    /// ima03 目錄參考號碼
    pub catalog_ref: Option<String>,
    /// ima915 供應商管制 0–3（預設 0）
    pub supplier_control: Option<String>,
    /// ima131 產品分類（客製必填，aim-372）
    pub product_class: Option<String>,
    /// ima04 工程圖號
    pub drawing_no: Option<String>,
    /// ta_ima08 申請部門（客製）
    pub apply_dept: Option<String>,
    /// ima105 軟體物件
    pub software: bool,
    /// ima14 工程料件
    pub engineering: bool,
    /// ima107 插件位置
    pub insert_position: bool,
    /// ima147 插件位置與 QPA 勾稽
    pub insert_qpa_check: bool,
    /// ima109 材料類別（分類碼類別 8）
    pub material_category: Option<String>,
    /// ima903 可否做聯產品入庫（來源碼 M、T 才可）
    pub coproduct: bool,
    /// ima24 檢驗否
    pub inspection: bool,
    /// ima911 重覆性生產
    pub repetitive: bool,
    /// ta_ima02 PCN（客製）
    pub pcn: Option<String>,
    /// ima07 ABC 碼（預設 A）
    pub abc_code: Option<String>,
    /// ima70 消耗料件
    pub consumable: bool,
    /// ima37 補貨策略碼
    pub replenish_policy: Option<String>,
    /// ima51 經濟訂購量（> 0，預設 1）
    pub economic_order_qty: Option<Decimal>,
    /// ima52 平均訂購量（> 0，預設 1）
    pub average_order_qty: Option<Decimal>,
    /// ima27 安全庫存量
    pub safety_stock: Option<Decimal>,
    /// ima28 安全庫存期間
    pub safety_period: Option<Decimal>,
    /// ima271 最高儲存數量
    pub max_stock_qty: Option<Decimal>,
    /// ima71 儲存有效天數
    pub shelf_life_days: Option<i32>,
    /// ima25 庫存單位（必填）
    pub stock_unit: Option<String>,
    /// ima35 主要倉庫（倉庫未移植，不檢查）
    pub warehouse: Option<String>,
    /// ima36 主要儲位
    pub location: Option<String>,
    /// ima23 倉管員
    pub stock_keeper: Option<String>,
    /// ima907 第二單位（IM14：保留，不檢查換算）
    pub second_unit: Option<String>,
    /// ima908 計價單位
    pub pricing_unit: Option<String>,
    /// ima09–ima12 其他分群碼一～四（分類碼類別 D／E／F／G）
    pub other_groups: [Option<String>; 4],
    /// ima39 會計科目（未移植，不檢查）
    pub account: Option<String>,
    /// ima391 會計科目二
    pub account2: Option<String>,
    /// ima15 保稅與否
    pub bonded: bool,
    /// ima1001 中文名稱（全稱）
    pub full_name: Option<String>,
    /// ima1002 英文名稱
    pub english_name: Option<String>,
    /// ima1015 預計停產日
    pub planned_discontinue_on: Option<NaiveDate>,
    /// ima1014 產品週期狀態 1–6
    pub life_cycle: Option<String>,
    /// ima1016 箱條碼
    pub carton_barcode: Option<String>,
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
        Some(x) if x < T::default() => Err(DomainError::ItemQuantityInvalid),
        _ => Ok(()),
    }
}

/// 經濟訂購量、平均訂購量：空白為 1，須 > 0（:6505-6530，mfg1322）
fn order_qty(v: Option<Decimal>) -> Result<Option<Decimal>, DomainError> {
    match v {
        None => Ok(Some(Decimal::ONE)),
        Some(x) if x <= Decimal::ZERO => Err(DomainError::ItemOrderQtyInvalid),
        some => Ok(some),
    }
}

fn default_to(v: &mut Option<String>, d: &str) {
    if v.is_none() {
        *v = Some(d.to_owned());
    }
}

impl ItemAttrs {
    /// 長度、值域、預設值與來源碼連帶的欄位（IM-1、IM-6、IM-9～IM-12）；`misc` 為 MISC 料件（來源碼一律 Z）
    pub fn validated(mut self, misc: bool) -> Result<Self, DomainError> {
        clean(&mut self.name, "品名", 120)?;
        clean(&mut self.spec, "規格", 120)?;
        clean(&mut self.group_code, "分群碼", 10)?;
        clean(&mut self.source_code, "來源碼", 1)?;
        clean(&mut self.configurable_item_no, "規格組件料件編號", 40)?;
        clean(&mut self.current_version, "目前使用版本", 10)?;
        clean(&mut self.catalog_ref, "目錄參考號碼", 20)?;
        clean(&mut self.supplier_control, "供應商管制", 1)?;
        clean(&mut self.product_class, "產品分類", 10)?;
        clean(&mut self.drawing_no, "工程圖號", 255)?;
        clean(&mut self.apply_dept, "申請部門", 10)?;
        clean(&mut self.material_category, "材料類別", 10)?;
        clean(&mut self.pcn, "PCN", 20)?;
        clean(&mut self.abc_code, "ABC 碼", 1)?;
        clean(&mut self.replenish_policy, "補貨策略碼", 1)?;
        clean(&mut self.stock_unit, "庫存單位", 4)?;
        clean(&mut self.warehouse, "主要倉庫", 10)?;
        clean(&mut self.location, "主要儲位", 10)?;
        clean(&mut self.stock_keeper, "倉管員", 10)?;
        clean(&mut self.second_unit, "第二單位", 4)?;
        clean(&mut self.pricing_unit, "計價單位", 4)?;
        for g in &mut self.other_groups {
            clean(g, "其他分群碼", 10)?;
        }
        clean(&mut self.account, "會計科目", 24)?;
        clean(&mut self.account2, "會計科目二", 24)?;
        clean(&mut self.full_name, "中文名稱", 100)?;
        clean(&mut self.english_name, "英文名稱", 80)?;
        clean(&mut self.life_cycle, "產品週期狀態", 1)?;
        clean(&mut self.carton_barcode, "箱條碼", 40)?;

        if self.name.is_none() {
            return Err(DomainError::ItemNameEmpty);
        }
        if self.stock_unit.is_none() {
            return Err(DomainError::ItemStockUnitRequired);
        }
        if self.product_class.is_none() {
            return Err(DomainError::ItemProductClassRequired);
        }
        default_to(&mut self.source_code, "P");
        if misc {
            self.source_code = Some("Z".to_owned());
        }
        default_to(&mut self.abc_code, "A");
        default_to(&mut self.supplier_control, "0");
        let source = self.source_code.clone().unwrap_or_default();
        if !SOURCE_CODES.contains(&source.as_str()) {
            return Err(DomainError::ItemSourceInvalid(source));
        }
        if let Some(a) = &self.abc_code
            && !["A", "B", "C"].contains(&a.as_str())
        {
            return Err(DomainError::ItemAbcInvalid(a.clone()));
        }
        if let Some(r) = &self.replenish_policy {
            if !REPLENISH_CODES.contains(&r.as_str()) {
                return Err(DomainError::ItemReplenishInvalid(r.clone()));
            }
            if (r == "0" || r == "5") && !REORDER_SOURCES.contains(&source.as_str()) {
                return Err(DomainError::ItemReplenishSourceMismatch);
            }
        }
        if let Some(s) = &self.supplier_control
            && !["0", "1", "2", "3"].contains(&s.as_str())
        {
            return Err(DomainError::ItemSupplierControlInvalid(s.clone()));
        }
        if let Some(l) = &self.life_cycle
            && !["1", "2", "3", "4", "5", "6"].contains(&l.as_str())
        {
            return Err(DomainError::ItemLifeCycleInvalid(l.clone()));
        }
        self.economic_order_qty = order_qty(self.economic_order_qty)?;
        self.average_order_qty = order_qty(self.average_order_qty)?;
        non_negative(self.safety_stock)?;
        non_negative(self.safety_period)?;
        non_negative(self.max_stock_qty)?;
        non_negative(self.shelf_life_days)?;
        // 來源碼連帶的欄位（:6342）
        if source != "T" {
            self.configurable_item_no = None;
        }
        if source != "M" && source != "T" {
            self.coproduct = false;
        }
        Ok(self)
    }

    /// 由料件分群帶入的預設值（i100_set_rel_ima06 :3842-3930；IM25）。品名、規格等料件自己的欄位不帶
    pub fn from_group(group_code: &str, g: &ItemGroupAttrs) -> Self {
        Self {
            group_code: Some(group_code.to_owned()),
            source_code: g.source_code.clone(),
            catalog_ref: g.catalog_ref.clone(),
            supplier_control: Some(g.supplier_control.clone()),
            product_class: g.product_class.clone(),
            drawing_no: g.drawing_no.clone(),
            software: g.software,
            engineering: g.engineering,
            insert_position: g.insert_position,
            insert_qpa_check: g.insert_qpa_check,
            material_category: g.material_category.clone(),
            coproduct: g.coproduct,
            inspection: g.inspection,
            repetitive: g.repetitive,
            abc_code: g.abc_code.clone(),
            consumable: g.consumable,
            replenish_policy: g.replenish_policy.clone(),
            economic_order_qty: g.economic_order_qty,
            average_order_qty: g.average_order_qty,
            safety_stock: g.safety_stock,
            safety_period: g.safety_period,
            shelf_life_days: g.shelf_life_days,
            stock_unit: g.stock_unit.clone(),
            warehouse: g.warehouse.clone(),
            location: g.location.clone(),
            stock_keeper: g.stock_keeper.clone(),
            second_unit: g.second_unit.clone(),
            pricing_unit: g.pricing_unit.clone(),
            other_groups: g.other_groups.clone(),
            account: g.account.clone(),
            account2: g.account2.clone(),
            bonded: g.bonded,
            ..Self::default()
        }
    }

    /// 引用的單位（刪除單位時檢查用）
    pub fn units(&self) -> impl Iterator<Item = &str> {
        [&self.stock_unit, &self.second_unit, &self.pricing_unit]
            .into_iter()
            .filter_map(|u| u.as_deref())
    }
}

/// 系統維護、唯讀的欄位（IM29）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSystem {
    /// ima140 Phase Out 否
    pub phase_out: bool,
    /// ima906 單位使用方式（固定 1，IM14）
    pub unit_usage: String,
    /// ima16 低階碼
    pub low_level_code: i32,
    /// ima146 BOM 異動否
    pub bom_changed: Option<String>,
    /// ima905 當月是否入聯產品
    pub coproduct_this_month: bool,
    /// ima93 新料件處理狀況
    pub new_part_status: String,
    /// ima916 資料來源
    pub data_source: String,
    /// ima901 料件建檔日期
    pub item_created_on: NaiveDate,
    /// ima1012 首次出貨日
    pub first_shipped_on: Option<NaiveDate>,
    /// ima1013 首次入庫日
    pub first_received_on: Option<NaiveDate>,
    /// ima902 呆滯日期
    pub slow_moving_on: Option<NaiveDate>,
    /// ima881 期間採購最近採購日期
    pub last_period_purchase_on: Option<NaiveDate>,
    /// ima73 最近入庫日
    pub last_received_on: Option<NaiveDate>,
    /// ima74 最近出庫日
    pub last_issued_on: Option<NaiveDate>,
    /// ima29 最近異動日期
    pub last_txn_on: Option<NaiveDate>,
    /// ima30 最近盤點日期
    pub last_count_on: Option<NaiveDate>,
}

impl ItemSystem {
    /// 新增料件的預設值（i100_default :2817-3224）
    pub fn new(data_source: &str, today: NaiveDate) -> Self {
        Self {
            phase_out: false,
            unit_usage: "1".to_owned(),
            low_level_code: NEW_LOW_LEVEL_CODE,
            bom_changed: None,
            coproduct_this_month: false,
            new_part_status: NEW_PART_STATUS.to_owned(),
            data_source: data_source.to_owned(),
            item_created_on: today,
            first_shipped_on: None,
            first_received_on: None,
            slow_moving_on: None,
            last_period_purchase_on: None,
            last_received_on: None,
            last_issued_on: None,
            last_txn_on: None,
            last_count_on: Some(today),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub no: ItemNo,
    pub attrs: ItemAttrs,
    pub system: ItemSystem,
    /// imaacti 資料有效碼
    pub active_code: ActiveCode,
    /// ima1010 狀態碼
    pub status_code: StatusCode,
    /// ta_ima01 訂單無效否（客製，新增預設是，IM18）
    pub order_invalid: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl Item {
    /// 新增：處理中／開立（IM-1）；`plant` 記在資料來源（IM13）
    pub fn new(
        no: ItemNo,
        attrs: ItemAttrs,
        plant: &str,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            no,
            attrs,
            system: ItemSystem::new(plant, today),
            active_code: ActiveCode::Processing,
            status_code: StatusCode::Open,
            order_invalid: true,
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    /// 資料有效（資料有效碼不是 N；TTRS 標準欄位 active，IM27）
    pub fn active(&self) -> bool {
        self.active_code != ActiveCode::Invalid
    }

    fn not_invalid(&self) -> Result<(), DomainError> {
        if self.active() {
            Ok(())
        } else {
            Err(DomainError::ItemInactive)
        }
    }

    fn transition_error(&self, action: &'static str) -> DomainError {
        DomainError::InvalidItemTransition {
            from: self.active_code.label(),
            action,
        }
    }

    /// 修改：無效不可修改（mfg1000）；已確認可以修改
    pub fn update(
        &mut self,
        attrs: ItemAttrs,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.not_invalid()?;
        self.attrs = attrs;
        self.touch(user, today);
        Ok(())
    }

    /// 確認（i100sub_y_chk：已確認 9023、無效 aim-153）
    pub fn confirm(&mut self, user: &str, today: NaiveDate) -> Result<(), DomainError> {
        self.not_invalid()?;
        if self.active_code == ActiveCode::Confirmed {
            return Err(self.transition_error("確認"));
        }
        self.active_code = ActiveCode::Confirmed;
        self.status_code = StatusCode::Confirmed;
        self.touch(user, today);
        Ok(())
    }

    /// 取消確認（i100_notconfirm :4771，atm-365）：回到處理中／開立
    pub fn unconfirm(&mut self, user: &str, today: NaiveDate) -> Result<(), DomainError> {
        self.not_invalid()?;
        if self.status_code != StatusCode::Confirmed {
            return Err(self.transition_error("取消確認"));
        }
        self.active_code = ActiveCode::Processing;
        self.status_code = StatusCode::Open;
        self.touch(user, today);
        Ok(())
    }

    /// 作廢切換（i100_x :4195-4210）：狀態碼不變，開立 P↔N、確認 Y↔N
    pub fn toggle_void(&mut self, user: &str, today: NaiveDate) {
        self.active_code = match (self.active_code, self.status_code) {
            (ActiveCode::Invalid, StatusCode::Open) => ActiveCode::Processing,
            (ActiveCode::Invalid, StatusCode::Confirmed) => ActiveCode::Confirmed,
            _ => ActiveCode::Invalid,
        };
        self.touch(user, today);
    }

    /// 刪除只限處理中（i100_r :4274；已確認 9023、無效 aim-153）
    pub fn check_deletable(&self) -> Result<(), DomainError> {
        self.not_invalid()?;
        if self.active_code != ActiveCode::Processing {
            return Err(self.transition_error("刪除"));
        }
        Ok(())
    }

    /// 訂單無效切換（客製 i100_order_invalid :8003-8050）：任何狀態都可
    pub fn toggle_order_invalid(&mut self, user: &str, today: NaiveDate) {
        self.order_invalid = !self.order_invalid;
        self.touch(user, today);
    }

    /// 複製（i100_copy_default :6676-6790）：新料號、可改品名；版本清空、平均訂購量 1、系統欄位重設、
    /// 處理中／開立、所有者為目前使用者；其餘欄位照原料件
    pub fn copy_as(
        &self,
        no: ItemNo,
        name: Option<String>,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        let mut attrs = self.attrs.clone();
        if let Some(n) = name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty()) {
            attrs.name = Some(n);
        }
        attrs.current_version = None;
        attrs.average_order_qty = Some(Decimal::ONE);
        if no.is_misc() {
            attrs.source_code = Some("Z".to_owned());
        }
        let mut system = ItemSystem::new(&self.system.data_source, today);
        system.last_count_on = None;
        Self {
            order_invalid: self.order_invalid,
            system,
            ..Self::new(no, attrs, "", user, dept, today)
        }
    }

    fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

/// 其他語系的品名、規格（item_text；ADR 0005、IM7）；zh-TW 存在料件本身
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemText {
    pub lang: crate::lang::Lang,
    pub name: Option<String>,
    pub spec: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs() -> ItemAttrs {
        ItemAttrs {
            name: Some("螺絲".into()),
            stock_unit: Some("PCS".into()),
            product_class: Some("P01".into()),
            ..Default::default()
        }
    }

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 9).unwrap()
    }

    #[test]
    fn item_no_keeps_case_and_rejects_forbidden_chars() {
        assert_eq!(ItemNo::parse(" a 01 ").unwrap().as_str(), "a 01");
        assert_eq!(
            ItemNo::parse("A*1"),
            Err(DomainError::ItemNoInvalidChar('*'))
        );
        assert!(ItemNo::parse("MISC01").unwrap().needs_misc_master());
        assert!(!ItemNo::parse("MISC").unwrap().needs_misc_master());
    }

    #[test]
    fn defaults_and_source_links() {
        let a = ItemAttrs {
            configurable_item_no: Some("X".into()),
            coproduct: true,
            ..attrs()
        }
        .validated(false)
        .unwrap();
        assert_eq!(a.source_code.as_deref(), Some("P"));
        assert_eq!(a.abc_code.as_deref(), Some("A"));
        assert_eq!(a.economic_order_qty, Some(Decimal::ONE));
        assert_eq!(a.configurable_item_no, None);
        assert!(!a.coproduct);
        assert_eq!(
            attrs().validated(true).unwrap().source_code.as_deref(),
            Some("Z")
        );
    }

    #[test]
    fn replenish_rules() {
        let bad = ItemAttrs {
            source_code: Some("X".into()),
            replenish_policy: Some("0".into()),
            ..attrs()
        };
        assert_eq!(
            bad.validated(false),
            Err(DomainError::ItemReplenishSourceMismatch)
        );
        let six = ItemAttrs {
            replenish_policy: Some("6".into()),
            ..attrs()
        };
        assert_eq!(
            six.validated(false),
            Err(DomainError::ItemReplenishInvalid("6".into()))
        );
    }

    #[test]
    fn state_changes() {
        let no = ItemNo::parse("A001").unwrap();
        let mut i = Item::new(
            no,
            attrs().validated(false).unwrap(),
            "TW01",
            "u",
            "D",
            day(),
        );
        assert!(i.check_deletable().is_ok());
        i.confirm("u", day()).unwrap();
        assert_eq!(
            i.confirm("u", day()).unwrap_err().code(),
            "ITEM_INVALID_TRANSITION"
        );
        i.toggle_void("u", day());
        assert_eq!(i.active_code, ActiveCode::Invalid);
        assert_eq!(i.check_deletable().unwrap_err().code(), "ITEM_INACTIVE");
        i.toggle_void("u", day());
        assert_eq!(i.active_code, ActiveCode::Confirmed);
        i.unconfirm("u", day()).unwrap();
        assert_eq!(
            (i.active_code, i.status_code),
            (ActiveCode::Processing, StatusCode::Open)
        );
    }
}
