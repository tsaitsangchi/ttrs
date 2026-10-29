//! API 契約：Axum 後端與 Dioxus 前端共用。
//! 這裡只放純資料型別，不放業務邏輯，也不可依賴 tokio/sqlx 等非 wasm 套件。

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
// p_zz 程式登錄（見 docs/legacy-map/azz.yaml）
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

/// 新增/修改程式的輸入（未給的欄位套用 p_zz 預設值，JSON 可省略）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgramInput {
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
    pub print_setting: Option<String>,
    pub print_default: Option<String>,
    pub quick_run_data: Option<String>,
    pub help_enabled: bool,
    pub log_errors: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgramDto {
    pub program_no: String,
    pub module_code: String,
    pub program_type: Option<String>,
    pub name: Option<String>,
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
    pub print_setting: Option<String>,
    pub print_default: Option<String>,
    pub quick_run_data: Option<String>,
    pub help_enabled: bool,
    pub log_errors: bool,
    pub created_by: String,
    pub created_dept: String,
    pub updated_by: Option<String>,
}

impl ProgramDto {
    /// 轉回輸入格式，供修改時沿用其餘欄位
    pub fn to_input(&self) -> ProgramInput {
        ProgramInput {
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
            print_setting: self.print_setting.clone(),
            print_default: self.print_default.clone(),
            quick_run_data: self.quick_run_data.clone(),
            help_enabled: self.help_enabled,
            log_errors: self.log_errors,
        }
    }
}

/// 程式名稱（gaz_file，多語系）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgramNameDto {
    pub lang: String,
    pub name: String,
}

/// 異動紀錄（azo_file）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntryDto {
    /// 執行的作業，如 "p_zz"
    pub program: String,
    pub user_id: String,
    pub key: String,
    /// "delete" 等
    pub action: String,
}

/// 改代號、複製時指定新代號
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NewProgramNoRequest {
    pub new_no: String,
}

/// 設定群組權限
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GrantRequest {
    pub permission_class: String,
}
