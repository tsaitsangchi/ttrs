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
