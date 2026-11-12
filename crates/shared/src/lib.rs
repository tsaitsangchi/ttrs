//! API 契約：Axum 後端與 Dioxus 前端共用。
//! 這裡只放純資料型別，不放業務邏輯，也不可依賴 tokio/sqlx 等非 wasm 套件。

pub use chrono::NaiveDate;
pub use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Item Maintenance 料件基本資料維護（TipTop aimi100 / ima_file；specs/features/aim/item_maintenance.feature）
// ---------------------------------------------------------------------------

/// 料件可輸入的欄位（第一階段，IM1）；新增、修改共用，也是「取得分群預設值」的結果（IM25）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemInput {
    /// ima01 料件編號
    pub item_no: String,
    /// ima02 品名（zh-TW；其他語系見 texts）
    pub name: Option<String>,
    /// ima021 規格
    pub spec: Option<String>,
    /// ima06 分群碼
    pub group_code: Option<String>,
    /// ima08 來源碼
    pub source_code: Option<String>,
    /// ima13 規格組件料件編號
    pub configurable_item_no: Option<String>,
    /// ima05 目前使用版本
    pub current_version: Option<String>,
    /// ima03 目錄參考號碼
    pub catalog_ref: Option<String>,
    /// ima915 供應商管制 0–3
    pub supplier_control: Option<String>,
    /// ima131 產品分類
    pub product_class: Option<String>,
    /// ima04 工程圖號
    pub drawing_no: Option<String>,
    /// ta_ima08 申請部門（客製）
    pub apply_dept: Option<String>,
    pub software: bool,
    pub engineering: bool,
    pub insert_position: bool,
    pub insert_qpa_check: bool,
    /// ima109 材料類別（分類碼類別 8）
    pub material_category: Option<String>,
    pub coproduct: bool,
    pub inspection: bool,
    pub repetitive: bool,
    /// ta_ima02 PCN（客製）
    pub pcn: Option<String>,
    pub abc_code: Option<String>,
    pub consumable: bool,
    pub replenish_policy: Option<String>,
    pub economic_order_qty: Option<rust_decimal::Decimal>,
    pub average_order_qty: Option<rust_decimal::Decimal>,
    pub safety_stock: Option<rust_decimal::Decimal>,
    pub safety_period: Option<rust_decimal::Decimal>,
    pub max_stock_qty: Option<rust_decimal::Decimal>,
    pub shelf_life_days: Option<i32>,
    pub stock_unit: Option<String>,
    pub warehouse: Option<String>,
    pub location: Option<String>,
    pub stock_keeper: Option<String>,
    pub second_unit: Option<String>,
    pub pricing_unit: Option<String>,
    pub other_group1: Option<String>,
    pub other_group2: Option<String>,
    pub other_group3: Option<String>,
    pub other_group4: Option<String>,
    pub account: Option<String>,
    pub account2: Option<String>,
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
    /// 修改時帶回的版本號（ADR 0012 §4）；ItemDto 攤平時不輸出（版本號在外層）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<i32>,
}

/// 料件的多語言品名、規格（item_text；ADR 0005、IM7）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemTextDto {
    pub lang: String,
    pub name: Option<String>,
    pub spec: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemDto {
    /// 可輸入的欄位；name、spec 依查詢語系（指定語系 → zh-TW）
    #[serde(flatten)]
    pub fields: ItemInput,
    /// imaacti 資料有效碼：P 處理中、Y 已確認、N 無效（IM27）
    pub active_code: String,
    /// ima1010 狀態碼：0 開立、1 確認
    pub status_code: String,
    /// ta_ima01 訂單無效否（客製）
    pub order_invalid: bool,
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
    /// ima93 新料件處理狀況（8 碼 Y/N）
    pub new_part_status: String,
    /// ima916 資料來源
    pub data_source: String,
    /// ima901 料件建檔日期
    pub item_created_on: NaiveDate,
    pub first_shipped_on: Option<NaiveDate>,
    pub first_received_on: Option<NaiveDate>,
    pub slow_moving_on: Option<NaiveDate>,
    pub last_period_purchase_on: Option<NaiveDate>,
    pub last_received_on: Option<NaiveDate>,
    pub last_issued_on: Option<NaiveDate>,
    pub last_txn_on: Option<NaiveDate>,
    pub last_count_on: Option<NaiveDate>,
    /// 帶出的名稱
    pub group_description: Option<String>,
    pub product_class_name: Option<String>,
    pub stock_keeper_name: Option<String>,
    pub apply_dept_name: Option<String>,
    /// 各語系的品名、規格（zh-TW 為 name、spec 本身）
    #[serde(default)]
    pub texts: Vec<ItemTextDto>,
    /// 資料有效（資料有效碼不是 N）
    pub active: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl ItemDto {
    pub fn to_input(&self) -> ItemInput {
        ItemInput {
            version: Some(self.version),
            ..self.fields.clone()
        }
    }
}

/// 複製料件（reproduce）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CopyItemRequest {
    pub new_item_no: String,
    pub name: Option<String>,
}

/// 畫面 maxlength（與 ttrs_domain::item 一致）
pub mod item_max_len {
    pub const ITEM_NO: usize = 40;
    pub const NAME: usize = 120;
    pub const SPEC: usize = 120;
    pub const GROUP: usize = 10;
    pub const VERSION: usize = 10;
    pub const CATALOG_REF: usize = 20;
    pub const DRAWING_NO: usize = 255;
    pub const CODE10: usize = 10;
    pub const PCN: usize = 20;
    pub const UNIT: usize = 4;
    pub const ACCOUNT: usize = 24;
    pub const FULL_NAME: usize = 100;
    pub const ENGLISH_NAME: usize = 80;
    pub const BARCODE: usize = 40;
}

/// 所有 API 錯誤的統一格式；`code` 是穩定的機器可讀代碼，`message` 給使用者看。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Program Maintenance 程式維護（見 docs/legacy-map/azz.yaml）
// ---------------------------------------------------------------------------

/// 報表類型（zz14）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportTypeDto {
    Voucher,
    Detail,
    Statistics,
    Tracking,
    #[default]
    None,
}

impl ReportTypeDto {
    pub const ALL: [Self; 5] = [
        Self::Voucher,
        Self::Detail,
        Self::Statistics,
        Self::Tracking,
        Self::None,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Voucher => "憑證",
            Self::Detail => "明細表",
            Self::Statistics => "統計表",
            Self::Tracking => "追蹤表",
            Self::None => "無",
        }
    }
}

/// 閒置控制（zz28）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdleControlDto {
    Off,
    ByProgram,
    #[default]
    BySystem,
}

impl IdleControlDto {
    pub const ALL: [Self; 3] = [Self::Off, Self::ByProgram, Self::BySystem];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "不控制",
            Self::ByProgram => "由程式",
            Self::BySystem => "依系統",
        }
    }
}

/// 逾時處理（zz29）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdleActionDto {
    WarnOnly,
    WarnThenClose,
    Close,
}

impl IdleActionDto {
    pub const ALL: [Self; 3] = [Self::WarnOnly, Self::WarnThenClose, Self::Close];

    pub fn label(self) -> &'static str {
        match self {
            Self::WarnOnly => "僅警告",
            Self::WarnThenClose => "警告後關閉",
            Self::Close => "直接關閉",
        }
    }
}

/// 報表副檔名（zz06）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportExtModeDto {
    Fixed,
    Dynamic,
}

impl ReportExtModeDto {
    pub const ALL: [Self; 2] = [Self::Fixed, Self::Dynamic];

    pub fn label(self) -> &'static str {
        match self {
            Self::Fixed => "固定",
            Self::Dynamic => "動態",
        }
    }
}

/// 新增/修改程式的輸入（未給的欄位套用預設值，JSON 可省略）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgramInput {
    /// `name`、`report_title` 所屬語系；未給為 zh-TW
    pub lang: Option<String>,
    pub program_no: String,
    pub module_code: String,
    /// 必填；模組為 MENU 時為 M（ADR 0008 P3）
    pub program_type: Option<String>,
    /// 預設語系名稱（gaz03）
    pub name: Option<String>,
    pub report_type: Option<ReportTypeDto>,
    pub idle_control: Option<IdleControlDto>,
    pub idle_seconds: Option<i32>,
    pub idle_action: Option<IdleActionDto>,
    pub pass_book_no: bool,
    pub deferred_print_count: Option<i32>,
    pub current_print_count: Option<i32>,
    pub report_width: Option<i32>,
    // Genero 執行環境欄位：原樣保存（Q1）
    pub run_command: Option<String>,
    pub window_style: Option<String>,
    pub report_ext_mode: Option<ReportExtModeDto>,
    pub program_group: Option<String>,
    pub fixed_print_method: Option<String>,
    pub fixed_print_condition: Option<String>,
    pub sql_explain: bool,
    pub log_errors: bool,
    // 其他設定：原樣保存
    pub default_actions: Option<String>,
    pub print_conditions: bool,
    pub allow_key_update: bool,
    pub remark: Option<String>,
    /// 預設語系的報表抬頭（gaz06）
    pub report_title: Option<String>,
    /// 設計日期（zz09），未給時為今天
    pub designed_on: Option<NaiveDate>,
    /// TipTop 程式代號（zz01，ADR 0008 P2）：選填，只用於轉檔與對照
    pub tiptop_program_no: Option<String>,
    /// 各語系的名稱與報表抬頭，一次儲存（ADR 0005，畫面的 MultilingualField）；
    /// 有給時取代 `lang`、`name`、`report_title`。修改時名稱空白代表清除該語系
    pub names: Vec<ProgramNameDto>,
    /// 修改時帶回的版本號（ADR 0012 §4）；API 要求必填，沒帶時回 400（CH4）
    pub version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgramDto {
    pub program_no: String,
    pub module_code: String,
    pub program_type: Option<String>,
    /// 依查詢語系顯示的名稱；該語系沒有時退回繁中（L2）
    pub name: Option<String>,
    /// 所有語系的名稱與報表抬頭（gaz_file）
    pub names: Vec<ProgramNameDto>,
    pub report_type: ReportTypeDto,
    pub idle_control: IdleControlDto,
    pub idle_seconds: Option<i32>,
    pub idle_action: Option<IdleActionDto>,
    pub pass_book_no: bool,
    pub deferred_print_count: i32,
    pub current_print_count: i32,
    pub report_width: Option<i32>,
    pub run_command: Option<String>,
    pub window_style: Option<String>,
    pub report_ext_mode: Option<ReportExtModeDto>,
    pub program_group: Option<String>,
    pub fixed_print_method: Option<String>,
    pub fixed_print_condition: Option<String>,
    pub sql_explain: bool,
    pub log_errors: bool,
    pub default_actions: Option<String>,
    pub print_conditions: bool,
    pub allow_key_update: bool,
    pub remark: Option<String>,
    pub report_title: Option<String>,
    pub designed_on: NaiveDate,
    pub tiptop_program_no: Option<String>,
    pub created_by: String,
    pub created_dept: String,
    pub updated_by: Option<String>,
    /// 最後建立/修改日期（zzdate）
    pub updated_on: NaiveDate,
    /// 版本號（樂觀鎖，ADR 0012 §4）：修改、刪除、啟用／停用時帶回
    #[serde(default)]
    pub version: i32,
    /// 建立、最後修改時間（到秒，ADR 0012 §1）
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl ProgramDto {
    /// 轉回輸入格式，供修改時沿用其餘欄位
    pub fn to_input(&self) -> ProgramInput {
        ProgramInput {
            lang: None,
            program_no: self.program_no.clone(),
            module_code: self.module_code.clone(),
            program_type: self.program_type.clone(),
            name: self.name.clone(),
            report_type: Some(self.report_type),
            idle_control: Some(self.idle_control),
            idle_seconds: self.idle_seconds,
            idle_action: self.idle_action,
            pass_book_no: self.pass_book_no,
            deferred_print_count: Some(self.deferred_print_count),
            current_print_count: Some(self.current_print_count),
            report_width: self.report_width,
            run_command: self.run_command.clone(),
            window_style: self.window_style.clone(),
            report_ext_mode: self.report_ext_mode,
            program_group: self.program_group.clone(),
            fixed_print_method: self.fixed_print_method.clone(),
            fixed_print_condition: self.fixed_print_condition.clone(),
            sql_explain: self.sql_explain,
            log_errors: self.log_errors,
            default_actions: self.default_actions.clone(),
            print_conditions: self.print_conditions,
            allow_key_update: self.allow_key_update,
            remark: self.remark.clone(),
            report_title: self.report_title.clone(),
            designed_on: Some(self.designed_on),
            tiptop_program_no: self.tiptop_program_no.clone(),
            // 只沿用預設語系的 name/report_title；各語系一次儲存時由呼叫端填 names
            names: Vec::new(),
            version: Some(self.version),
        }
    }
}

/// 程式名稱（gaz_file，多語系）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgramNameDto {
    pub lang: String,
    pub name: String,
    pub report_title: Option<String>,
}

/// 異動紀錄（azo_file）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntryDto {
    /// 執行的作業，如 "program-maintenance"
    pub program: String,
    pub user_id: String,
    pub key: String,
    /// "delete" 等
    pub action: String,
    /// 補充說明（報表的條件、格式、筆數）
    #[serde(default)]
    pub detail: Option<String>,
}

/// 複製時指定新代號
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NewProgramNoRequest {
    pub new_no: String,
}

/// 設定群組權限
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GrantRequest {
    pub permission_class: String,
}

// ---------------------------------------------------------------------------
// QBE 查詢（TipTop CONSTRUCT）：各維護作業共用
// ---------------------------------------------------------------------------

/// 單一欄位的查詢條件；`expr` 為使用者輸入的 QBE 字串，如 `aim*`、`1:5`、`a|b`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QbeCondition {
    /// 欄位鍵值，與該作業 DTO 的欄位名稱相同（如 `program_no`）
    pub field: String,
    pub expr: String,
}

/// 查詢請求：條件之間為 AND；未給頁碼時取第 1 頁、每頁 `DEFAULT_PAGE_SIZE` 筆
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QueryRequest {
    pub conditions: Vec<QbeCondition>,
    /// 從 1 開始
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    /// 結果中程式名稱等多語言欄位的顯示語系；未給為 zh-TW
    pub lang: Option<String>,
}

impl QueryRequest {
    pub const DEFAULT_PAGE_SIZE: u32 = 50;
}

/// 分頁結果；`total` 為符合條件的總筆數（TipTop 的 cnt）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

/// 程式維護欄位長度上限（字元），供畫面 maxlength 使用。
/// 來源與檢核在 `ttrs_domain::program::max_len`；兩者由 ttrs-app 的單元測試保證一致。
pub mod program_max_len {
    pub const PROGRAM_NO: usize = 40;
    pub const TIPTOP_PROGRAM_NO: usize = 20;
    pub const MODULE_CODE: usize = 10;
    pub const DEFAULT_ACTIONS: usize = 3000;
    pub const RUN_COMMAND: usize = 60;
    pub const PROGRAM_GROUP: usize = 10;
    pub const FIXED_PRINT_CONDITION: usize = 80;
    pub const FIXED_PRINT_METHOD: usize = 1;
    pub const WINDOW_STYLE: usize = 20;
    pub const REMARK: usize = 255;
    pub const NAME: usize = 500;
    pub const REPORT_TITLE: usize = 255;
}

// ---------------------------------------------------------------------------
// Data Dictionary Maintenance 資料字典維護（TipTop p_zta；見 docs/legacy-map/azz.yaml）
// ---------------------------------------------------------------------------

/// 資料表類型（TipTop gat07，D7）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableTypeDto {
    Parameter,
    Master,
    Transaction,
    Statistics,
    Base,
}

impl TableTypeDto {
    pub const ALL: [Self; 5] = [
        Self::Parameter,
        Self::Master,
        Self::Transaction,
        Self::Statistics,
        Self::Base,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Parameter => "參數",
            Self::Master => "主檔",
            Self::Transaction => "交易",
            Self::Statistics => "統計",
            Self::Base => "基本",
        }
    }
}

/// 支援的語系（gat02/gaq02；繁中、英文、簡中）
pub const DICTIONARY_LANGS: [&str; 3] = ["zh-TW", "en", "zh-CN"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableDictionaryInput {
    pub table_name: String,
    pub module_code: String,
    pub table_type: Option<TableTypeDto>,
    pub upstream: Option<String>,
    pub downstream: Option<String>,
    /// 修改時帶回的版本號（ADR 0012 §4）；API 要求必填，沒帶時回 400（CH4）
    pub version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableDictionaryDto {
    pub table_name: String,
    pub module_code: String,
    pub table_type: Option<TableTypeDto>,
    pub upstream: Option<String>,
    pub downstream: Option<String>,
    pub created_by: String,
    pub created_on: NaiveDate,
    /// 顯示語系（`?lang=`）的表名稱；無則退回繁中（ADR 0005）
    #[serde(default)]
    pub name: Option<String>,
    /// 顯示語系的目的；無則退回繁中
    #[serde(default)]
    pub purpose: Option<String>,
    /// 所有語系的說明
    #[serde(default)]
    pub texts: Vec<TableTextDto>,
    /// 版本號（樂觀鎖，ADR 0012 §4）：修改、刪除、啟用／停用時帶回
    #[serde(default)]
    pub version: i32,
    /// 建立、最後修改時間（到秒，ADR 0012 §1）
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

/// 表名稱、目的、備註（某一語系；gat03/gat04/gat05）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableTextDto {
    pub lang: String,
    pub name: Option<String>,
    pub purpose: Option<String>,
    pub remark: Option<String>,
}

/// 欄位名稱、說明、額外說明（某一語系；gaq03/gaq04/gaq05）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnTextDto {
    pub column_name: String,
    pub lang: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub extra: Option<String>,
}

/// 資料庫實際欄位結構（唯讀）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnInfoDto {
    pub name: String,
    /// 正規化型別，如 varchar(20)、integer、numeric(18,4)、timestamptz
    pub data_type: String,
    pub not_null: bool,
    pub primary_key: bool,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintKindDto {
    PrimaryKey,
    ForeignKey,
    Unique,
    Check,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstraintDto {
    pub name: String,
    pub kind: ConstraintKindDto,
    pub columns: Vec<String>,
    pub ref_table: Option<String>,
    pub ref_columns: Vec<String>,
    /// 外鍵參照的主鍵/唯一限制名稱（rzad07 Ref. Constraint Name）
    #[serde(default)]
    pub ref_constraint: Option<String>,
    pub check_clause: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexDto {
    pub name: String,
    pub unique: bool,
    pub columns: Vec<String>,
}

/// 變更紀錄：內容涉及該表的 migration（D5）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationDto {
    pub version: i64,
    pub description: String,
}

// ----- TipTop 對照目錄（legacy_catalog.feature） -----

/// 一張 TipTop 資料表的對照（匯入與查看共用）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyTableDto {
    /// TipTop 表代碼，如 zx_file
    pub legacy_name: String,
    /// 英文表名，如 user_account
    pub name: String,
    /// .sch 表說明
    pub title: String,
    pub purpose: Option<String>,
    pub module: String,
    pub columns: Vec<LegacyColumnDto>,
}

/// 一個 TipTop 欄位的對照
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyColumnDto {
    pub legacy_name: String,
    pub name: String,
    pub data_type: String,
    pub primary_key: bool,
    pub description: String,
    pub notes: Option<String>,
    /// .sch 標示 No Use
    pub unused: bool,
    /// 命名待確認
    pub verify: bool,
}

/// 對照目錄清單的一列
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LegacyTableSummaryDto {
    pub legacy_name: String,
    pub name: String,
    pub title: String,
    pub module: String,
    pub column_count: u32,
}

/// 匯入結果
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LegacyImportResultDto {
    pub tables: u32,
    pub columns: u32,
}

// ----- User Maintenance 使用者維護（TipTop p_zx；user_maintenance.feature） -----

/// 使用者欄位上限（依 TipTop zx_file.sch；與 domain user::max_len 一致）
pub mod user_max_len {
    pub const USER_ID: usize = 10;
    pub const NAME: usize = 80;
    pub const DEPT: usize = 10;
    pub const EMAIL: usize = 80;
    pub const IDP_USERNAME: usize = 255;
}

/// 新增/修改使用者的輸入（未給的欄位套用預設值）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserInput {
    /// zx01 使用者代號；修改時不可變更（U1）
    pub user_id: String,
    /// zx02 使用者名稱
    pub name: Option<String>,
    /// zx03 部門代號：須為預設營運中心存在且有效的部門（user_maintenance_department.feature）
    pub dept_code: Option<String>,
    /// zx04 權限類別
    pub permission_class: String,
    /// zx05 使用者選單（模組 MENU 的程式）
    pub menu: String,
    /// zx07 多營運中心使用者
    pub multi_plant: bool,
    /// zx08 預設營運中心
    pub default_plant: String,
    /// zx09 電子郵件
    pub email: Option<String>,
    /// 公司帳號（UPN，ADR 0006）
    pub idp_username: Option<String>,
    /// zx06 偏好語言；未給為 zh-TW
    pub lang: Option<String>,
    /// zx18 失效日期
    pub valid_until: Option<NaiveDate>,
    /// zx10 Web 密碼：只寫不讀，空白表示不變更；設定後下次登入須改密碼（ADR 0009 W7）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_password: Option<String>,
    /// zxy 可使用營運中心（勾選清單，須含預設營運中心）；未給時沿用目前清單，
    /// 改預設營運中心時自動加入（user_maintenance_plants.feature PZ1）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_plants: Option<Vec<String>>,
    /// zxy03＝ALL 可使用全部營運中心；未給時不變（PZ3）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_plants: Option<bool>,
    /// 修改時帶回的版本號（ADR 0012 §4）；API 要求必填，沒帶時回 400（CH4）
    pub version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserDto {
    pub user_id: String,
    pub name: Option<String>,
    pub dept_code: Option<String>,
    pub permission_class: String,
    pub menu: String,
    pub multi_plant: bool,
    pub default_plant: String,
    pub email: Option<String>,
    pub idp_username: Option<String>,
    /// 首次以公司帳號登入時綁定（ADR 0006 §2）
    pub idp_bound: bool,
    pub lang: String,
    pub valid_until: Option<NaiveDate>,
    /// zxacti 資料有效
    pub active: bool,
    /// 確認狀態（ADR 0013 SA9）：已確認否、確認人、確認時間
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    /// zxy 可使用的營運中心（不含 ALL）
    pub allowed_plants: Vec<String>,
    /// zxy03＝ALL 可使用全部營運中心（PZ3）
    #[serde(default)]
    pub all_plants: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本號（樂觀鎖，ADR 0012 §4）：修改、刪除、啟用／停用時帶回
    #[serde(default)]
    pub version: i32,
    /// 建立、最後修改時間（到秒，ADR 0012 §1）
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl UserDto {
    /// 轉回輸入格式，供修改時沿用其餘欄位
    pub fn to_input(&self) -> UserInput {
        UserInput {
            user_id: self.user_id.clone(),
            name: self.name.clone(),
            dept_code: self.dept_code.clone(),
            permission_class: self.permission_class.clone(),
            menu: self.menu.clone(),
            multi_plant: self.multi_plant,
            default_plant: self.default_plant.clone(),
            email: self.email.clone(),
            idp_username: self.idp_username.clone(),
            lang: Some(self.lang.clone()),
            valid_until: self.valid_until,
            web_password: None,
            allowed_plants: None,
            all_plants: None,
            version: Some(self.version),
        }
    }
}

/// 複製使用者的新代號
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NewUserIdRequest {
    pub new_user_id: String,
}

// ---------------------------------------------------------------------------
// Permission Class Maintenance 權限類別維護（TipTop p_zw / zw_file，見 docs/legacy-map/azz.yaml）
// ---------------------------------------------------------------------------

/// 欄位上限（與 ttrs_domain::permission_class::max_len 一致）
pub mod permission_class_max_len {
    pub const CODE: usize = 10;
    pub const NAME: usize = 80;
}

/// 閒置控制（zw04）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassIdleControlDto {
    #[default]
    Off,
    ByClass,
    BySystem,
}

impl ClassIdleControlDto {
    pub const ALL: [Self; 3] = [Self::Off, Self::ByClass, Self::BySystem];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "不控制",
            Self::ByClass => "依權限類別",
            Self::BySystem => "依系統",
        }
    }
}

/// 新增／修改權限類別
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PermissionClassInput {
    pub class_code: String,
    pub name: Option<String>,
    pub default_menu: Option<String>,
    /// 未指定為「不控制」
    pub idle_control: Option<ClassIdleControlDto>,
    pub idle_action: Option<IdleActionDto>,
    pub idle_seconds: Option<i32>,
    /// 修改時帶回的版本號（ADR 0012 §4）；API 要求必填，沒帶時回 400（CH4）
    pub version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionClassDto {
    pub class_code: String,
    pub name: String,
    pub default_menu: Option<String>,
    pub idle_control: ClassIdleControlDto,
    pub idle_action: Option<IdleActionDto>,
    pub idle_seconds: Option<i32>,
    pub active: bool,
    /// 確認狀態（ADR 0013 SA9）：已確認否、確認人、確認時間
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本號（樂觀鎖，ADR 0012 §4）：修改、刪除、啟用／停用時帶回
    #[serde(default)]
    pub version: i32,
    /// 建立、最後修改時間（到秒，ADR 0012 §1）
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl PermissionClassDto {
    pub fn to_input(&self) -> PermissionClassInput {
        PermissionClassInput {
            class_code: self.class_code.clone(),
            name: Some(self.name.clone()),
            default_menu: self.default_menu.clone(),
            idle_control: Some(self.idle_control),
            idle_action: self.idle_action,
            idle_seconds: self.idle_seconds,
            version: Some(self.version),
        }
    }
}

/// 批次作業的處理筆數（如「更新所有使用者目錄編號」）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatedCountDto {
    pub updated: u64,
}

// ---------------------------------------------------------------------------
// Plant Maintenance 營運中心維護（TipTop aooi901 / azp_file，見 docs/legacy-map/aoo.yaml）
// ---------------------------------------------------------------------------

/// 欄位上限（與 ttrs_domain::plant::max_len 一致）
pub mod plant_max_len {
    pub const ID: usize = 10;
    pub const NAME: usize = 80;
    pub const ADDRESS: usize = 255;
    pub const PHONE: usize = 40;
    pub const FAX: usize = 40;
}

/// 新增／修改營運中心
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlantInput {
    pub plant_id: String,
    pub name: Option<String>,
    /// IANA 時區名稱；未指定為 Asia/Taipei
    pub time_zone: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub fax: Option<String>,
    /// 資料庫代碼（azp03，工廠 schema 名稱，P13）：新增時空白為下一個未使用的號碼；建立後不可修改
    pub schema_name: Option<String>,
    /// 修改時帶回的版本號（ADR 0012 §4）；API 要求必填，沒帶時回 400（CH4）
    pub version: Option<i32>,
}

/// 登入後的連線資訊（sign_in.feature SQ9）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDto {
    /// zx01 使用者代號
    pub user_id: String,
    pub name: Option<String>,
    pub dept_code: Option<String>,
    pub permission_class: String,
    /// zx06 偏好語言（畫面語系預設）
    pub lang: String,
    /// zx07 多營運中心使用者（可切換營運中心）
    pub multi_plant: bool,
    /// 目前營運中心
    pub plant: String,
    pub plant_name: String,
    /// 密碼即將到期的剩餘天數／次數（weblogin.4gl:566-608 提前通知）
    pub password_expires_in_days: Option<i64>,
    pub password_uses_left: Option<u32>,
    /// 可切換的營運中心（q_zxy），依代號排序
    pub plants: Vec<PlantOptionDto>,
    /// 連線版本：重設密碼後舊連線即失效（ADR 0010 R6）
    #[serde(default)]
    pub session_version: u32,
}

/// 登入（ADR 0009）；營運中心未指定時自動選取（sign_in.feature SQ12、SQ14）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignInInput {
    pub user_id: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plant_id: Option<String>,
}

/// 登入畫面輸入使用者代號後的營運中心選項（sign_in.feature SL1–SL8）
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SignInPlantsDto {
    /// 有效、可使用的營運中心，依代號排序；帳號不存在或不能登入時空白（SQ11）
    pub plants: Vec<PlantOptionDto>,
    /// 自動選取的營運中心（SQ12）
    pub selected: Option<String>,
    /// 非多營運中心使用者：不能改（SQ13）
    pub fixed: bool,
}

/// 變更密碼（webpasswd）：以舊密碼驗證
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangePasswordInput {
    pub user_id: String,
    pub old_password: String,
    pub new_password: String,
    pub confirm_password: String,
}

/// 忘記密碼是否開放（ADR 0010 R9：未設定寄信伺服器時不開放）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PasswordResetStatusDto {
    pub available: bool,
}

/// 申請重設密碼（R1：以使用者代號）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PasswordResetRequestInput {
    pub user_id: String,
}

/// 以重設連結設定新密碼
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PasswordResetInput {
    pub token: String,
    pub new_password: String,
    pub confirm_password: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlantOptionDto {
    pub plant_id: String,
    pub name: String,
}

/// 切換營運中心（aoos901）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchPlantInput {
    pub plant_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlantDto {
    pub plant_id: String,
    pub name: String,
    /// 資料庫代碼（工廠 schema 名稱，如 ds01）
    pub schema_name: String,
    pub time_zone: String,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub fax: Option<String>,
    pub active: bool,
    /// 確認狀態（ADR 0013 SA9）：已確認否、確認人、確認時間
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本號（樂觀鎖，ADR 0012 §4）：修改、刪除、啟用／停用時帶回
    #[serde(default)]
    pub version: i32,
    /// 建立、最後修改時間（到秒，ADR 0012 §1）
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl PlantDto {
    pub fn to_input(&self) -> PlantInput {
        PlantInput {
            plant_id: self.plant_id.clone(),
            name: Some(self.name.clone()),
            time_zone: Some(self.time_zone.clone()),
            address: self.address.clone(),
            phone: self.phone.clone(),
            fax: self.fax.clone(),
            schema_name: Some(self.schema_name.clone()),
            version: Some(self.version),
        }
    }
}

/// 複製營運中心的新代號
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewPlantIdRequest {
    pub new_plant_id: String,
}

// ---------------------------------------------------------------------------
// Program Permission Maintenance 程式執行權限維護（TipTop p_zy / zy_file，見 docs/legacy-map/azz.yaml）
// ---------------------------------------------------------------------------

/// 新增／修改程式權限；未指定的欄位：新增時用預設值（程式的全部功能、0／0／0），修改時沿用原值
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgramPermissionInput {
    pub program_no: String,
    /// 允許執行功能（TipTop action id）
    pub actions: Option<Vec<String>>,
    /// 他人資料權限 zy04（'0'–'4'）
    pub other_user_data: Option<String>,
    /// 他部門資料權限 zy05（'0'–'8'）
    pub other_dept_data: Option<String>,
    /// 單身權限 zy07（'0'–'3'）
    pub detail_rows: Option<String>,
    /// 修改時帶回的版本號（ADR 0012 §4）；API 要求必填，沒帶時回 400（CH4）
    pub version: Option<i32>,
}

/// 目前使用者在某作業的有效權限（authorization.feature AQ13）：畫面據以隱藏沒有權限的按鈕
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyPermissionDto {
    pub program: String,
    /// 有沒有本作業的權限（沒有時其餘欄位為空）
    pub allowed: bool,
    pub actions: Vec<String>,
    pub other_user_data: String,
    pub other_dept_data: String,
    pub detail_rows: String,
}

impl MyPermissionDto {
    /// 功能在允許清單中（不分大小寫）
    pub fn allows(&self, action: &str) -> bool {
        self.actions.iter().any(|a| a.eq_ignore_ascii_case(action))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgramPermissionDto {
    pub class_code: String,
    pub program_no: String,
    /// 依 ?lang= 顯示的程式名稱（退回規則同程式維護）
    pub program_name: Option<String>,
    /// 程式提供的功能（可勾選的範圍）
    pub available_actions: Vec<String>,
    pub actions: Vec<String>,
    pub other_user_data: String,
    pub other_dept_data: String,
    pub detail_rows: String,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本號（樂觀鎖，ADR 0012 §4）：修改、刪除、啟用／停用時帶回
    #[serde(default)]
    pub version: i32,
    /// 建立、最後修改時間（到秒，ADR 0012 §1）
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl ProgramPermissionDto {
    pub fn to_input(&self) -> ProgramPermissionInput {
        ProgramPermissionInput {
            program_no: self.program_no.clone(),
            actions: Some(self.actions.clone()),
            other_user_data: Some(self.other_user_data.clone()),
            other_dept_data: Some(self.other_dept_data.clone()),
            detail_rows: Some(self.detail_rows.clone()),
            version: Some(self.version),
        }
    }
}

/// 複製權限到另一個權限類別
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopyPermissionsRequest {
    pub target_class: String,
}

/// 依程式整批產生
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GeneratePermissionsRequest {
    pub program_nos: Vec<String>,
}

/// 依選單整批產生／刪除
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MenuPermissionsRequest {
    pub menu: String,
    /// 整批產生時，已有的權限是否重設為預設值（預設否，Y10）
    pub reset_existing: bool,
}

/// 整批作業結果
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionBatchResultDto {
    pub added: u64,
    pub reset: u64,
    pub removed: u64,
}

// ---------------------------------------------------------------------------
// User Special Permission Maintenance 使用者特殊權限維護（TipTop p_zxw / zxw_file，見 docs/legacy-map/azz.yaml）
// ---------------------------------------------------------------------------

/// 特殊權限的分類（zxw03：1 權限類別、2 程式編號）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialPermissionKindDto {
    #[default]
    Class,
    Program,
}

/// 新增或修改特殊權限；程式列的功能與資料權限未指定時，新增用預設值、修改沿用原值
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPermissionInput {
    pub kind: SpecialPermissionKindDto,
    /// 權限類別代號或程式代號
    pub target: String,
    /// 使用者還沒有特殊權限時，先加入使用者原本的權限類別（X1）
    pub include_own_class: bool,
    pub actions: Option<Vec<String>>,
    pub other_user_data: Option<String>,
    pub other_dept_data: Option<String>,
    pub detail_rows: Option<String>,
    /// 修改時帶回的版本號（ADR 0012 §4）；API 要求必填，沒帶時回 400（CH4）
    pub version: Option<i32>,
}

/// 使用者特殊權限查詢的一筆結果（ADR 0013 SA1）：有特殊權限的使用者
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecialPermissionUserDto {
    pub user_id: String,
    pub name: Option<String>,
    /// 特殊權限筆數
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPermissionDto {
    pub user_id: String,
    pub line_no: i32,
    pub kind: SpecialPermissionKindDto,
    pub target: String,
    /// 權限說明或依 ?lang= 顯示的程式名稱
    pub target_name: Option<String>,
    /// 程式列：程式提供的功能（可勾選的範圍）
    pub available_actions: Vec<String>,
    pub actions: Vec<String>,
    pub other_user_data: Option<String>,
    pub other_dept_data: Option<String>,
    pub detail_rows: Option<String>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本號（樂觀鎖，ADR 0012 §4）：修改、刪除、啟用／停用時帶回
    #[serde(default)]
    pub version: i32,
    /// 建立、最後修改時間（到秒，ADR 0012 §1）
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl UserPermissionDto {
    pub fn to_input(&self) -> UserPermissionInput {
        UserPermissionInput {
            kind: self.kind,
            target: self.target.clone(),
            include_own_class: false,
            actions: Some(self.actions.clone()),
            other_user_data: self.other_user_data.clone(),
            other_dept_data: self.other_dept_data.clone(),
            detail_rows: self.detail_rows.clone(),
            version: Some(self.version),
        }
    }
}

/// 複製特殊權限到另一個使用者
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopyUserPermissionsRequest {
    pub target_user: String,
}

// ---------------------------------------------------------------------------
// Department Maintenance 部門維護（aooi030 / gem_file；工廠層）
// ---------------------------------------------------------------------------

/// 部門基本欄位（新增、修改）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DepartmentInput {
    pub dept_code: String,
    pub name: Option<String>,
    pub full_name: Option<String>,
    /// 修改時帶回的版本號（ADR 0012 §4）；API 要求必填，沒帶時回 400（CH4）
    pub version: Option<i32>,
}

/// 部門會計欄位（aooi030「會計資料維護」）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DepartmentAccountingInput {
    pub accounting_dept: bool,
    /// 費用類別 M 管理、P 製造、R 研發、S 銷售
    pub expense_type: Option<String>,
    /// 管理類別 1 成本中心、2 利潤中心、3 其他
    pub control_type: String,
    /// 對應成本中心；管理類別 1、2 時未給為本部門
    pub cost_center: Option<String>,
    /// 修改時帶回的版本號（ADR 0012 §4）；API 要求必填，沒帶時回 400（CH4）
    pub version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DepartmentDto {
    pub dept_code: String,
    pub name: String,
    pub full_name: Option<String>,
    pub accounting_dept: bool,
    pub expense_type: Option<String>,
    pub control_type: String,
    pub cost_center: Option<String>,
    /// 對應成本中心的部門簡稱
    pub cost_center_name: Option<String>,
    pub active: bool,
    /// 確認狀態（ADR 0013 SA9）：已確認否、確認人、確認時間
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本號（樂觀鎖，ADR 0012 §4）：修改、刪除、啟用／停用時帶回
    #[serde(default)]
    pub version: i32,
    /// 建立、最後修改時間（到秒，ADR 0012 §1）
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl DepartmentDto {
    pub fn to_input(&self) -> DepartmentInput {
        DepartmentInput {
            dept_code: self.dept_code.clone(),
            name: Some(self.name.clone()),
            full_name: self.full_name.clone(),
            version: Some(self.version),
        }
    }

    pub fn to_accounting(&self) -> DepartmentAccountingInput {
        DepartmentAccountingInput {
            accounting_dept: self.accounting_dept,
            expense_type: self.expense_type.clone(),
            control_type: self.control_type.clone(),
            cost_center: self.cost_center.clone(),
            version: Some(self.version),
        }
    }
}

/// 畫面 maxlength（與 ttrs_domain::department::max_len 一致）
pub mod department_max_len {
    pub const CODE: usize = 10;
    pub const NAME: usize = 80;
    pub const FULL_NAME: usize = 80;
}

// ---------------------------------------------------------------------------
// Classification Code 分類碼維護（TipTop aooi300 / azf_file；classification_code.feature）
// ---------------------------------------------------------------------------

/// 分類碼
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationCodeDto {
    /// azf02 碼類別：8、D、E、F、G
    pub kind: String,
    /// azf01 碼別代號
    pub code: String,
    /// azf03 說明
    pub description: Option<String>,
    /// azf05、azf051 存貨差異科目（類別 G）
    pub variance_account: Option<String>,
    pub variance_account2: Option<String>,
    /// azf06 成本性質（類別 G）
    pub cost_nature: Option<i32>,
    pub active: bool,
    /// 確認狀態（ADR 0013 SA9）：已確認否、確認人、確認時間
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本號（樂觀鎖，ADR 0012 §4）：修改、刪除、啟用／停用時帶回
    #[serde(default)]
    pub version: i32,
    /// 建立、最後修改時間（到秒，ADR 0012 §1）
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

/// 分類碼新增、修改；類別由網址決定
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassificationCodeInput {
    pub code: String,
    pub description: Option<String>,
    pub variance_account: Option<String>,
    pub variance_account2: Option<String>,
    pub cost_nature: Option<i32>,
    /// 修改時帶回的版本號（ADR 0012 §4）；API 要求必填，沒帶時回 400（CH4）
    pub version: Option<i32>,
}

impl ClassificationCodeDto {
    pub fn to_input(&self) -> ClassificationCodeInput {
        ClassificationCodeInput {
            code: self.code.clone(),
            description: self.description.clone(),
            variance_account: self.variance_account.clone(),
            variance_account2: self.variance_account2.clone(),
            cost_nature: self.cost_nature,
            version: Some(self.version),
        }
    }
}

/// 欄位上限（與 ttrs_domain::classification::max_len 一致）
pub mod classification_max_len {
    pub const CODE: usize = 10;
    pub const DESCRIPTION: usize = 80;
    pub const ACCOUNT: usize = 24;
}

// ---------------------------------------------------------------------------
// Employee Maintenance 員工維護（aooi040 / gen_file；工廠層）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EmployeeInput {
    pub employee_id: String,
    pub name: Option<String>,
    pub dept_code: Option<String>,
    pub title: Option<String>,
    pub extension: Option<String>,
    pub email: Option<String>,
    /// 修改時帶回的版本號（ADR 0012 §4）；API 要求必填，沒帶時回 400（CH4）
    pub version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmployeeDto {
    pub employee_id: String,
    pub name: String,
    pub dept_code: Option<String>,
    /// 部門簡稱
    pub dept_name: Option<String>,
    pub title: Option<String>,
    pub extension: Option<String>,
    pub email: Option<String>,
    pub active: bool,
    /// 確認狀態（ADR 0013 SA9）：已確認否、確認人、確認時間
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本號（樂觀鎖，ADR 0012 §4）：修改、刪除、啟用／停用時帶回
    #[serde(default)]
    pub version: i32,
    /// 建立、最後修改時間（到秒，ADR 0012 §1）
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl EmployeeDto {
    pub fn to_input(&self) -> EmployeeInput {
        EmployeeInput {
            employee_id: self.employee_id.clone(),
            name: Some(self.name.clone()),
            dept_code: self.dept_code.clone(),
            title: self.title.clone(),
            extension: self.extension.clone(),
            email: self.email.clone(),
            version: Some(self.version),
        }
    }
}

/// 畫面 maxlength（與 ttrs_domain::employee::max_len 一致）
pub mod employee_max_len {
    pub const ID: usize = 10;
    pub const NAME: usize = 40;
    pub const TITLE: usize = 80;
    pub const EXTENSION: usize = 5;
    pub const EMAIL: usize = 80;
}

// ---------------------------------------------------------------------------
// 資料異動記錄（ADR 0012；change_history.feature）
// ---------------------------------------------------------------------------

/// 一次異動中的一個欄位（label 為資料字典的欄位說明，依語系；字典沒有時為欄位名稱，CH5）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeFieldDto {
    pub column: String,
    pub label: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// 一筆異動記錄
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeDto {
    pub change_id: i64,
    pub occurred_at: DateTime<Utc>,
    pub table_name: String,
    /// insert／update／delete 或業務動作（activate、deactivate…）
    pub operation: String,
    pub version: Option<i32>,
    pub user_id: String,
    pub user_dept: Option<String>,
    /// user／ai／service／db
    pub actor_kind: String,
    pub on_behalf_of: Option<String>,
    pub program: Option<String>,
    pub action: Option<String>,
    pub request_id: Option<String>,
    pub client_ip: Option<String>,
    /// 新增、刪除為全部欄位；修改為變動的欄位
    pub fields: Vec<ChangeFieldDto>,
}

// ---------------------------------------------------------------------------
// Menu Maintenance 選單維護（TipTop p_zm / zm_file；menu_maintenance.feature）
// ---------------------------------------------------------------------------

/// 目錄清單的一筆（模組 MENU 的程式）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuSummaryDto {
    pub menu_no: String,
    /// 依 ?lang= 顯示的目錄名稱（程式名稱）
    pub name: Option<String>,
    pub item_count: u32,
}

/// 目錄的一筆單身
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuItemDto {
    pub seq: i32,
    pub program_no: String,
    pub name: Option<String>,
    /// 子項為目錄（模組 MENU）
    pub is_menu: bool,
}

/// 新增單身；序號空白時接在最後（MN6）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MenuItemInput {
    pub program_no: String,
    pub seq: Option<i32>,
}

/// 複製目錄（MN7）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MenuCopyInput {
    pub new_menu_no: String,
}

/// 側欄樹狀選單的節點（cl_create_4sm：目錄為 StartMenuGroup、程式為 StartMenuCommand）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuNodeDto {
    pub program_no: String,
    /// 依語系的程式名稱；沒有名稱時為程式代號
    pub name: String,
    /// 程式節點的模組（前端依模組與程式代號組成網址 /<模組>/<程式代號>，MN12）
    pub module_code: String,
    pub children: Vec<MenuNodeDto>,
}

impl MenuNodeDto {
    pub fn is_menu(&self) -> bool {
        self.module_code == "MENU"
    }
}

// ---------------------------------------------------------------------------
// Unit Maintenance 單位維護（TipTop aooi101 / gfe_file；unit_maintenance.feature）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitDto {
    pub unit_code: String,
    pub name: Option<String>,
    pub decimals: i16,
    pub active: bool,
    /// 確認狀態（ADR 0013 SA9）：已確認否、確認人、確認時間
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnitInput {
    pub unit_code: String,
    pub name: Option<String>,
    /// 小數位數；空白為 0（UN4）
    pub decimals: Option<i16>,
    /// 修改時帶回的版本號（ADR 0012 §4）
    pub version: Option<i32>,
}

impl UnitDto {
    pub fn to_input(&self) -> UnitInput {
        UnitInput {
            unit_code: self.unit_code.clone(),
            name: self.name.clone(),
            decimals: Some(self.decimals),
            version: Some(self.version),
        }
    }
}

/// 畫面 maxlength（與 ttrs_domain::unit 一致）
pub mod unit_max_len {
    pub const CODE: usize = 4;
    pub const NAME: usize = 80;
}

// ---------------------------------------------------------------------------
// Project Maintenance 專案維護（TipTop aooi200 / gja_file；project_maintenance.feature；工廠層）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDto {
    pub project_code: String,
    pub name: Option<String>,
    pub start_on: Option<NaiveDate>,
    pub end_on: Option<NaiveDate>,
    pub active: bool,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectInput {
    pub project_code: String,
    pub name: Option<String>,
    pub start_on: Option<NaiveDate>,
    pub end_on: Option<NaiveDate>,
    /// 修改時帶回的版本號（ADR 0012 §4）
    pub version: Option<i32>,
}

impl ProjectDto {
    pub fn to_input(&self) -> ProjectInput {
        ProjectInput {
            project_code: self.project_code.clone(),
            name: self.name.clone(),
            start_on: self.start_on,
            end_on: self.end_on,
            version: Some(self.version),
        }
    }
}

/// 畫面 maxlength（與 ttrs_domain::project 一致）
pub mod project_max_len {
    pub const CODE: usize = 10;
    pub const NAME: usize = 80;
}

// ---------------------------------------------------------------------------
// Tax Type Maintenance 稅別維護（TipTop aooi150 / gec_file；tax_type_maintenance.feature）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaxTypeDto {
    pub tax_code: String,
    /// 進／銷項：1 進項、2 銷項（gec011）
    pub direction: String,
    pub name: Option<String>,
    pub account: Option<String>,
    pub account2: Option<String>,
    /// 發票聯數：0、2、3、4、X（gec05）
    pub invoice_copies: Option<String>,
    /// 課稅別：1 應稅、2 零稅率、3 免稅（gec06）
    pub tax_class: Option<String>,
    pub rate: rust_decimal::Decimal,
    pub include_tax: bool,
    pub filing_format: String,
    pub active: bool,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TaxTypeInput {
    pub tax_code: String,
    pub direction: String,
    pub name: Option<String>,
    pub account: Option<String>,
    pub account2: Option<String>,
    pub invoice_copies: Option<String>,
    pub tax_class: Option<String>,
    /// 稅率；空白為 0
    pub rate: Option<rust_decimal::Decimal>,
    /// 單價含稅否；空白為否
    pub include_tax: Option<bool>,
    /// 媒體申報格式；空白為 XX 不申報（TX6）
    pub filing_format: Option<String>,
    /// 修改時帶回的版本號（ADR 0012 §4）
    pub version: Option<i32>,
}

/// 稅別複製的目標（ADR 0013 SA5）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaxTypeCopyRequest {
    pub new_code: String,
    pub new_direction: String,
}

impl TaxTypeDto {
    /// 畫面選取用的鍵值：「稅別碼 進銷項」
    pub fn key(&self) -> String {
        format!("{} {}", self.tax_code, self.direction)
    }

    pub fn to_input(&self) -> TaxTypeInput {
        TaxTypeInput {
            tax_code: self.tax_code.clone(),
            direction: self.direction.clone(),
            name: self.name.clone(),
            account: self.account.clone(),
            account2: self.account2.clone(),
            invoice_copies: self.invoice_copies.clone(),
            tax_class: self.tax_class.clone(),
            rate: Some(self.rate),
            include_tax: Some(self.include_tax),
            filing_format: Some(self.filing_format.clone()),
            version: Some(self.version),
        }
    }
}

/// 畫面 maxlength（與 ttrs_domain::tax_type 一致）
pub mod tax_type_max_len {
    pub const CODE: usize = 4;
    pub const NAME: usize = 80;
    pub const ACCOUNT: usize = 24;
}

// ---------------------------------------------------------------------------
// Currency Maintenance 幣別維護（TipTop aooi050 / azi_file；currency_maintenance.feature）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrencyDto {
    pub currency_code: String,
    pub name: Option<String>,
    pub price_decimals: i16,
    pub amount_decimals: i16,
    pub total_decimals: i16,
    pub rate_decimals: i16,
    /// 計算方式：1 乘、2 除（azi10）
    pub rate_method: String,
    /// 匯率自動匯入的來源幣別代號（IM6）
    #[serde(default)]
    pub import_code: Option<String>,
    pub active: bool,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CurrencyInput {
    pub currency_code: String,
    pub name: Option<String>,
    /// 小數位數；空白用預設值（單價 4、金額 2、小計總計 2、匯率 6，CY3）
    pub price_decimals: Option<i16>,
    pub amount_decimals: Option<i16>,
    pub total_decimals: Option<i16>,
    pub rate_decimals: Option<i16>,
    /// 計算方式；空白為 1 乘（CY4）
    pub rate_method: Option<String>,
    /// 匯入代號（IM6）
    pub import_code: Option<String>,
    /// 修改時帶回的版本號（ADR 0012 §4）
    pub version: Option<i32>,
}

impl CurrencyDto {
    pub fn to_input(&self) -> CurrencyInput {
        CurrencyInput {
            currency_code: self.currency_code.clone(),
            name: self.name.clone(),
            price_decimals: Some(self.price_decimals),
            amount_decimals: Some(self.amount_decimals),
            total_decimals: Some(self.total_decimals),
            rate_decimals: Some(self.rate_decimals),
            rate_method: Some(self.rate_method.clone()),
            import_code: self.import_code.clone(),
            version: Some(self.version),
        }
    }
}

/// 畫面 maxlength（與 ttrs_domain::currency 一致）
pub mod currency_max_len {
    pub const CODE: usize = 4;
    pub const NAME: usize = 80;
}

// ---------------------------------------------------------------------------
// 基本代碼：區域別 aooi100、國別 aooi090、地區 aooi110、交運方式 aooi160、報表抬頭 aooi210
// （region_country_area_maintenance.feature、shipping_method_maintenance.feature、report_title_maintenance.feature）
// ---------------------------------------------------------------------------

/// 基本代碼的種類（與 ttrs_domain::basic_code::BasicCodeKind 一致）；API 路徑 /api/basic-codes/{slug}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BasicCodeKindDto {
    Region,
    Country,
    Area,
    ShippingMethod,
    ReportTitle,
}

impl BasicCodeKindDto {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Region => "regions",
            Self::Country => "countries",
            Self::Area => "areas",
            Self::ShippingMethod => "shipping-methods",
            Self::ReportTitle => "report-titles",
        }
    }

    pub fn program(self) -> &'static str {
        match self {
            Self::Region => "region-maintenance",
            Self::Country => "country-maintenance",
            Self::Area => "area-maintenance",
            Self::ShippingMethod => "shipping-method-maintenance",
            Self::ReportTitle => "report-title-maintenance",
        }
    }

    /// 隸屬的上層種類（國別 → 區域、地區 → 國別）
    pub fn parent(self) -> Option<Self> {
        match self {
            Self::Country => Some(Self::Region),
            Self::Area => Some(Self::Country),
            _ => None,
        }
    }

    pub fn multilingual(self) -> bool {
        self == Self::ReportTitle
    }
}

/// 名稱的某一語系
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasicCodeNameDto {
    pub lang: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BasicCodeDto {
    pub code: String,
    /// 依 ?lang= 的顯示名稱（退回規則：指定語系 → zh-TW → 空白）
    pub name: Option<String>,
    /// 隸屬的上層代號與其名稱（國別、地區）
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub parent_name: Option<String>,
    /// 名稱的全部語系（報表抬頭；含 zh-TW）
    #[serde(default)]
    pub names: Vec<BasicCodeNameDto>,
    pub active: bool,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BasicCodeInput {
    pub code: String,
    /// 名稱（繁中）
    pub name: Option<String>,
    pub parent: Option<String>,
    /// 名稱的其他語系（報表抬頭）；zh-TW 以 name 為準
    pub names: Vec<BasicCodeNameDto>,
    /// 修改時帶回的版本號（ADR 0012 §4）
    pub version: Option<i32>,
}

impl BasicCodeDto {
    pub fn to_input(&self) -> BasicCodeInput {
        let zh = self
            .names
            .iter()
            .find(|n| n.lang == "zh-TW")
            .map(|n| n.name.clone());
        BasicCodeInput {
            code: self.code.clone(),
            name: zh.or_else(|| self.name.clone()),
            parent: self.parent.clone(),
            names: self
                .names
                .iter()
                .filter(|n| n.lang != "zh-TW")
                .cloned()
                .collect(),
            version: Some(self.version),
        }
    }
}

/// 畫面 maxlength（與 ttrs_domain::basic_code 一致）
pub mod basic_code_max_len {
    pub const CODE: usize = 10;
    pub const NAME: usize = 80;
}

// ---------------------------------------------------------------------------
// Product Classification 產品分類維護（TipTop axmi110 / oba_file；product_classification.feature）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductClassDto {
    pub class_code: String,
    pub name: Option<String>,
    pub period_format: Option<String>,
    pub periods: Option<i32>,
    pub cost_center: Option<String>,
    /// 成本中心的部門簡稱
    pub cost_center_name: Option<String>,
    pub revenue_account: Option<String>,
    pub revenue_account2: Option<String>,
    pub active: bool,
    /// 確認狀態（ADR 0013 SA9）：已確認否、確認人、確認時間
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProductClassInput {
    pub class_code: String,
    pub name: Option<String>,
    pub period_format: Option<String>,
    pub periods: Option<i32>,
    pub cost_center: Option<String>,
    pub revenue_account: Option<String>,
    pub revenue_account2: Option<String>,
    /// 修改時帶回的版本號（ADR 0012 §4）
    pub version: Option<i32>,
}

impl ProductClassDto {
    pub fn to_input(&self) -> ProductClassInput {
        ProductClassInput {
            class_code: self.class_code.clone(),
            name: self.name.clone(),
            period_format: self.period_format.clone(),
            periods: self.periods,
            cost_center: self.cost_center.clone(),
            revenue_account: self.revenue_account.clone(),
            revenue_account2: self.revenue_account2.clone(),
            version: Some(self.version),
        }
    }
}

/// 畫面 maxlength（與 ttrs_domain::product_class::max_len 一致）
pub mod product_class_max_len {
    pub const CODE: usize = 10;
    pub const NAME: usize = 80;
    pub const PERIOD_FORMAT: usize = 1;
    pub const ACCOUNT: usize = 24;
}

// ---------------------------------------------------------------------------
// Item Group 料件分群維護（TipTop aimi110 / imz_file；item_group.feature）
// ---------------------------------------------------------------------------

/// 料件分群的欄位（第一階段 41 欄，IG3）；新增、修改共用
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemGroupInput {
    pub group_code: String,
    pub description: Option<String>,
    pub source_code: Option<String>,
    pub stock_unit: Option<String>,
    pub catalog_ref: Option<String>,
    pub supplier_control: Option<String>,
    pub drawing_no: Option<String>,
    pub software: bool,
    pub engineering: bool,
    pub insert_qpa_check: bool,
    pub insert_position: bool,
    pub material_category: Option<String>,
    pub coproduct: bool,
    pub inspection: bool,
    pub repetitive: bool,
    pub abc_code: Option<String>,
    pub consumable: bool,
    pub replenish_policy: Option<String>,
    pub reorder_point: Option<rust_decimal::Decimal>,
    pub reorder_qty: Option<rust_decimal::Decimal>,
    pub safety_stock: Option<rust_decimal::Decimal>,
    pub safety_period: Option<rust_decimal::Decimal>,
    pub shelf_life_days: Option<i32>,
    pub mrp_bucket_days: Option<i32>,
    pub warehouse: Option<String>,
    pub location: Option<String>,
    pub stock_keeper: Option<String>,
    pub sales_stat_unit: Option<String>,
    pub second_unit: Option<String>,
    pub pricing_unit: Option<String>,
    pub other_group1: Option<String>,
    pub other_group2: Option<String>,
    pub other_group3: Option<String>,
    pub other_group4: Option<String>,
    pub account: Option<String>,
    pub account2: Option<String>,
    pub bonded: bool,
    pub bonded_class: Option<String>,
    pub tariff_no: Option<String>,
    pub bonded_type: Option<String>,
    /// imz131 產品分類（新增料件時帶入，IM24）
    pub product_class: Option<String>,
    /// imz51 經濟訂購量（IM24）
    pub economic_order_qty: Option<rust_decimal::Decimal>,
    /// imz52 平均訂購量（IM24）
    pub average_order_qty: Option<rust_decimal::Decimal>,
    /// 修改時帶回的版本號（ADR 0012 §4）。ItemGroupDto 攤平此結構時不輸出（版本號在外層 `version`），
    /// 否則 JSON 會有兩個 version，前端解析失敗
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemGroupDto {
    /// 欄位（version 為目前版本）
    #[serde(flatten)]
    pub fields: ItemGroupInput,
    /// 倉管員姓名
    pub stock_keeper_name: Option<String>,
    pub active: bool,
    /// 確認狀態（ADR 0013 SA9）：已確認否、確認人、確認時間
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl ItemGroupDto {
    pub fn to_input(&self) -> ItemGroupInput {
        ItemGroupInput {
            version: Some(self.version),
            ..self.fields.clone()
        }
    }
}

#[cfg(test)]
mod item_group_dto_tests {
    use super::*;

    /// 攤平的欄位與外層都有 version 時 JSON 會重複、前端解析失敗（瀏覽器驗收 2026-11-09）
    #[test]
    fn item_group_dto_round_trips_with_one_version() {
        let dto = ItemGroupDto {
            fields: ItemGroupInput {
                group_code: "RM".into(),
                safety_stock: Some(rust_decimal::Decimal::new(1005, 1)),
                ..Default::default()
            },
            stock_keeper_name: None,
            active: true,
            confirmed: true,
            confirmed_by: Some("tiptop".into()),
            confirmed_at: None,
            created_by: "tiptop".into(),
            created_dept: "MIS".into(),
            created_on: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
            updated_by: None,
            updated_on: None,
            version: 3,
            created_at: None,
            updated_at: None,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert_eq!(json.matches("\"version\"").count(), 1, "{json}");
        let back: ItemGroupDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, 3);
        assert_eq!(back.to_input().version, Some(3));
        assert_eq!(back, dto);
    }
}

// ---------------------------------------------------------------------------
// Exchange Rates 匯率（TipTop aooi060 / azj_file、aooi070 / azk_file、aoop010；exchange_rate_*.feature）
// ---------------------------------------------------------------------------

/// 六種匯率（銀行買入、賣出、中價、海關旬、海關買入、賣出）
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RatesDto {
    pub bank_buy: rust_decimal::Decimal,
    pub bank_sell: rust_decimal::Decimal,
    pub bank_mid: rust_decimal::Decimal,
    pub customs_period: rust_decimal::Decimal,
    pub customs_buy: rust_decimal::Decimal,
    pub customs_sell: rust_decimal::Decimal,
}

/// 每月匯率（aooi060）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonthlyRateDto {
    pub currency_code: String,
    /// 幣別說明（azi02，顯示用）
    #[serde(default)]
    pub currency_name: Option<String>,
    /// 年月 YYYYMM
    pub period: String,
    pub rates: RatesDto,
    /// 月底重評價（azj07）
    pub revaluation: rust_decimal::Decimal,
    pub active: bool,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

/// 新增、修改每月匯率；海關旬（月平均）只由月底重評價寫入，輸入時忽略（MR10）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MonthlyRateInput {
    pub currency_code: String,
    pub period: String,
    pub rates: RatesDto,
    pub revaluation: rust_decimal::Decimal,
    /// 修改時帶回的版本號（ADR 0012 §4）
    pub version: Option<i32>,
}

impl MonthlyRateDto {
    pub fn key(&self) -> String {
        format!("{} {}", self.currency_code, self.period)
    }

    pub fn to_input(&self) -> MonthlyRateInput {
        MonthlyRateInput {
            currency_code: self.currency_code.clone(),
            period: self.period.clone(),
            rates: self.rates,
            revaluation: self.revaluation,
            version: Some(self.version),
        }
    }
}

/// 複製每月匯率：新幣別＋新年月（aooi060 reproduce）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MonthlyRateCopyRequest {
    pub currency_code: String,
    pub period: String,
}

/// 月底重評價（aooi060 re_rate，MR7）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevaluationRequest {
    pub date: NaiveDate,
}

/// 月底重評價、統一匯率的結果：處理的幣別與略過（已確認、已停用）的幣別
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RateBatchResult {
    pub processed: Vec<String>,
    pub skipped: Vec<String>,
}

/// 每日匯率（aooi070）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyRateDto {
    pub currency_code: String,
    #[serde(default)]
    pub currency_name: Option<String>,
    pub rate_date: NaiveDate,
    pub rates: RatesDto,
    pub active: bool,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyRateInput {
    pub currency_code: String,
    pub rate_date: NaiveDate,
    #[serde(default)]
    pub rates: RatesDto,
    #[serde(default)]
    pub version: Option<i32>,
}

impl DailyRateDto {
    pub fn key(&self) -> String {
        format!("{} {}", self.currency_code, self.rate_date)
    }

    pub fn to_input(&self) -> DailyRateInput {
        DailyRateInput {
            currency_code: self.currency_code.clone(),
            rate_date: self.rate_date,
            rates: self.rates,
            version: Some(self.version),
        }
    }
}

/// 複製每日匯率：新幣別＋新日期（aooi070 reproduce）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyRateCopyRequest {
    pub currency_code: String,
    pub rate_date: NaiveDate,
}

/// 統一匯率（aooi070 cal_rate，DR7）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifyRateRequest {
    pub date: NaiveDate,
}

/// 每日匯率複製（aoop010）：補到哪一天（CP2）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyRateFillRequest {
    pub until: NaiveDate,
}

/// 每日匯率複製的結果：複製出的每一筆（CP1）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyRateFillLine {
    pub currency_code: String,
    pub rate_date: NaiveDate,
    pub rates: RatesDto,
}

/// 匯率自動匯入（aooi070 auto_import；exchange_rate_import.feature）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RateImportRequest {
    /// 台銀牌告種類：cash 現金、spot 即期（預設，IM3）
    pub kind: Option<String>,
    /// 今天已有每日匯率時是否覆寫（IM5）
    pub overwrite: bool,
    /// 上傳的台銀 CSV 原文（IM1 備援）；空白時由伺服器下載
    pub csv: Option<String>,
}

/// 匯入結果：匯入的幣別、略過的幣別（停用或不存在）、警告（已確認不覆寫、沒有報價…）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RateImportResult {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
}

/// 每日匯率排程的一個步驟（IM8）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateJobStep {
    pub step: String,
    pub ok: bool,
    pub message: String,
}

/// 匯率維護畫面的設定（general_parameters.feature GP7）：本國幣別、匯率採用與提示
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateSettingsDto {
    pub base_currency: String,
    /// monthly 每月、daily 每日（aza19）
    pub rate_basis: String,
    /// 本作業的匯率不會被單據取用時的提示：monthly-unused、daily-unused；其餘為 None
    pub hint: Option<String>,
}

// ---------------------------------------------------------------------------
// 整體管理系統參數（TipTop aoos010 / aza_file；general_parameters.feature）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneralParametersDto {
    /// 會計期間 12／13（aza02）
    pub accounting_periods: i16,
    /// 本國幣別（aza17）與說明
    pub base_currency: String,
    #[serde(default)]
    pub base_currency_name: Option<String>,
    /// 匯率採用：monthly 每月、daily 每日（aza19）
    pub rate_basis: String,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneralParametersInput {
    pub accounting_periods: i16,
    pub base_currency: String,
    pub rate_basis: String,
    /// 修改時帶回的版本號（ADR 0012 §4）
    pub version: Option<i32>,
}

// ---------------------------------------------------------------------------
// 會計期間（TipTop aoos020 / azm_file；accounting_periods.feature）
// ---------------------------------------------------------------------------

/// 一期：期別、起始日、截止日、季別（azm0n1、azm0n2、azm0n3）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountingPeriodLine {
    pub period: i16,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub quarter: i16,
}

/// 一個會計年度（azm_file 一列）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountingYearDto {
    /// azm01 年度
    pub year: i32,
    /// azm02 期數 12／13
    pub period_count: i16,
    pub periods: Vec<AccountingPeriodLine>,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_by: Option<String>,
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

/// 新增、修改會計年度：各期（用預設值時先取 /api/accounting-years/defaults）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountingYearInput {
    pub year: i32,
    pub periods: Vec<AccountingPeriodLine>,
    /// 修改時帶回的版本號（ADR 0012 §4）
    pub version: Option<i32>,
}

/// 日期所屬的會計期間（azn_file 一列；s_gactpd）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountingDayDto {
    pub date: NaiveDate,
    pub year: i32,
    pub quarter: i16,
    pub period: i16,
    pub week: i16,
}
