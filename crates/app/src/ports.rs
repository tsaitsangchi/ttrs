//! Repository 介面。實作在 ttrs-infra（PostgreSQL）與 memory 模組（測試）。

use async_trait::async_trait;
use ttrs_domain::{
    item::{Item, ItemNo},
    plant::PlantId,
};

use crate::error::RepoError;

#[async_trait]
pub trait ItemRepository: Send + Sync {
    async fn get(&self, plant: &PlantId, no: &ItemNo) -> Result<Option<Item>, RepoError>;
    async fn list(&self, plant: &PlantId) -> Result<Vec<Item>, RepoError>;
    async fn insert(&self, item: &Item) -> Result<(), RepoError>;
    async fn update(&self, item: &Item) -> Result<(), RepoError>;
    async fn delete(&self, plant: &PlantId, no: &ItemNo) -> Result<(), RepoError>;
    /// 是否已有庫存異動（TipTop: tlf_file）
    async fn has_transactions(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, RepoError>;
}

/// 共用基本資料查詢（營運中心、單位…）
#[async_trait]
pub trait MasterDataRepository: Send + Sync {
    async fn plant_active(&self, plant: &PlantId) -> Result<bool, RepoError>;
    async fn unit_exists(&self, unit: &str) -> Result<bool, RepoError>;
}
