//! API 契約：Axum 後端與 Dioxus 前端共用。
//! 這裡只放純資料型別，不放業務邏輯，也不可依賴 tokio/sqlx 等非 wasm 套件。

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// 料件狀態（對應 TipTop 料件確認碼，見 docs/legacy-map/aim.yaml）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatusDto {
    Unconfirmed,
    Confirmed,
    Voided,
}

impl ItemStatusDto {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unconfirmed => "未確認",
            Self::Confirmed => "已確認",
            Self::Voided => "作廢",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemDto {
    pub plant_id: String,
    pub item_no: String,
    pub name: String,
    pub spec: Option<String>,
    pub stock_unit: String,
    pub status: ItemStatusDto,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateItemRequest {
    pub plant_id: String,
    pub item_no: String,
    pub name: String,
    pub spec: Option<String>,
    pub stock_unit: String,
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
    /// 未給時由程式代號推導（a 開頭 7 碼取第 4 碼）
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
    /// 各語系的名稱與報表抬頭，一次儲存（ADR 0005，畫面的 MultilingualField）；
    /// 有給時取代 `lang`、`name`、`report_title`。修改時名稱空白代表清除該語系
    pub names: Vec<ProgramNameDto>,
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
    pub created_by: String,
    pub created_dept: String,
    pub updated_by: Option<String>,
    /// 最後建立/修改日期（zzdate）
    pub updated_on: NaiveDate,
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
            // 只沿用預設語系的 name/report_title；各語系一次儲存時由呼叫端填 names
            names: Vec::new(),
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
    pub const PROGRAM_NO: usize = 20;
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
