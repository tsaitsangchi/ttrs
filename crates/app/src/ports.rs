//! Repository 介面。實作在 ttrs-infra（PostgreSQL）與 memory 模組（測試）。

use async_trait::async_trait;
use ttrs_domain::{
    item::{Item, ItemNo},
    plant::PlantId,
    program::{ModuleCode, Program, ProgramNo},
    qbe::QbeFilter,
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

/// 程式名稱（TipTop: gaz_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramName {
    pub lang: String,
    pub name: String,
    /// 報表抬頭（gaz06）
    pub report_title: Option<String>,
}

/// 異動紀錄（TipTop: azo_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub program: String,
    pub user_id: String,
    pub key: String,
    pub action: String,
}

/// 程式維護（TipTop: zz_file、gaz_file、zy_file、zm_file、azo_file）。
/// 需要連動多張表的方法（新增、改代號、刪除）必須在同一交易內完成。
#[async_trait]
pub trait ProgramRepository: Send + Sync {
    async fn get(&self, no: &ProgramNo) -> Result<Option<Program>, RepoError>;
    async fn list(&self) -> Result<Vec<Program>, RepoError>;
    /// QBE 查詢：符合全部篩選者依程式代號（位元組順序）排序，回傳指定範圍與總筆數
    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Program>, u64), RepoError>;
    /// 新增程式與名稱；`grant_to` 有值時同時授權給該權限類別。代號重複回傳 `Conflict`
    async fn insert(
        &self,
        program: &Program,
        names: &[ProgramName],
        grant_to: Option<&str>,
    ) -> Result<(), RepoError>;
    async fn update(&self, program: &Program) -> Result<(), RepoError>;
    /// 修改代號，連動名稱、權限、選單。新代號重複回傳 `Conflict`
    async fn change_no(&self, old: &ProgramNo, new: &ProgramNo) -> Result<(), RepoError>;
    /// 刪除程式並連帶刪除名稱、權限、選單，同時寫入異動紀錄
    async fn delete(&self, no: &ProgramNo, audit: &AuditEntry) -> Result<(), RepoError>;
    async fn names(&self, no: &ProgramNo) -> Result<Vec<ProgramName>, RepoError>;
    /// 新增或更新某語系的名稱與報表抬頭
    async fn set_name(&self, no: &ProgramNo, name: &ProgramName) -> Result<(), RepoError>;
    /// 授權（已存在則不變，同 p_zz_gen 的 upsert）
    async fn grant(&self, permission_class: &str, no: &ProgramNo) -> Result<(), RepoError>;
    async fn permission_classes(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError>;
    async fn menus_containing(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError>;
    async fn audit_entries(&self, key: &str) -> Result<Vec<AuditEntry>, RepoError>;
    async fn module_exists(&self, module: &ModuleCode) -> Result<bool, RepoError>;
    async fn permission_class_exists(&self, class: &str) -> Result<bool, RepoError>;
    async fn query_definition_exists(&self, query: &str) -> Result<bool, RepoError>;
}
