//! 記憶體版 repository，行為需與 PostgreSQL 版一致（含唯一鍵衝突）。

use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use ttrs_domain::{
    item::{Item, ItemNo},
    plant::PlantId,
};

use crate::{
    error::RepoError,
    ports::{ItemRepository, MasterDataRepository},
};

#[derive(Default)]
struct State {
    plants: HashSet<String>,
    units: HashSet<String>,
    items: BTreeMap<(PlantId, ItemNo), Item>,
    transactions: HashSet<(PlantId, ItemNo)>,
}

#[derive(Clone, Default)]
pub struct InMemoryStore(Arc<Mutex<State>>);

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 預設種子資料，與 migrations/0002_seed.sql 對齊
    pub fn seeded() -> Self {
        let s = Self::new();
        s.add_plant("TW01");
        s.add_unit("PCS");
        s.add_unit("KG");
        s
    }

    pub fn add_plant(&self, plant: &str) {
        self.lock().plants.insert(plant.to_uppercase());
    }

    pub fn add_unit(&self, unit: &str) {
        self.lock().units.insert(unit.to_uppercase());
    }

    pub fn add_transaction(&self, plant: &PlantId, no: &ItemNo) {
        self.lock().transactions.insert((plant.clone(), no.clone()));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().expect("in-memory store poisoned")
    }
}

#[async_trait]
impl ItemRepository for InMemoryStore {
    async fn get(&self, plant: &PlantId, no: &ItemNo) -> Result<Option<Item>, RepoError> {
        Ok(self.lock().items.get(&(plant.clone(), no.clone())).cloned())
    }

    async fn list(&self, plant: &PlantId) -> Result<Vec<Item>, RepoError> {
        Ok(self
            .lock()
            .items
            .values()
            .filter(|i| &i.plant_id == plant)
            .cloned()
            .collect())
    }

    async fn insert(&self, item: &Item) -> Result<(), RepoError> {
        let key = (item.plant_id.clone(), item.item_no.clone());
        let mut s = self.lock();
        if s.items.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        s.items.insert(key, item.clone());
        Ok(())
    }

    async fn update(&self, item: &Item) -> Result<(), RepoError> {
        let key = (item.plant_id.clone(), item.item_no.clone());
        self.lock().items.insert(key, item.clone());
        Ok(())
    }

    async fn delete(&self, plant: &PlantId, no: &ItemNo) -> Result<(), RepoError> {
        self.lock().items.remove(&(plant.clone(), no.clone()));
        Ok(())
    }

    async fn has_transactions(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, RepoError> {
        Ok(self
            .lock()
            .transactions
            .contains(&(plant.clone(), no.clone())))
    }
}

#[async_trait]
impl MasterDataRepository for InMemoryStore {
    async fn plant_active(&self, plant: &PlantId) -> Result<bool, RepoError> {
        Ok(self.lock().plants.contains(plant.as_str()))
    }

    async fn unit_exists(&self, unit: &str) -> Result<bool, RepoError> {
        Ok(self.lock().units.contains(&unit.to_uppercase()))
    }
}
