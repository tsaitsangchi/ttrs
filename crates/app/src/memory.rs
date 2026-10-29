//! 記憶體版 repository，行為需與 PostgreSQL 版一致（含唯一鍵衝突）。

use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use ttrs_domain::{
    item::{Item, ItemNo},
    plant::PlantId,
    program::{ModuleCode, Program, ProgramNo},
    qbe::QbeFilter,
};

use crate::{
    error::RepoError,
    ports::{AuditEntry, ItemRepository, MasterDataRepository, ProgramName, ProgramRepository},
};

#[derive(Default)]
struct State {
    plants: HashSet<String>,
    units: HashSet<String>,
    items: BTreeMap<(PlantId, ItemNo), Item>,
    transactions: HashSet<(PlantId, ItemNo)>,
    // Program Maintenance
    modules: HashSet<String>,
    permission_classes: HashSet<String>,
    /// (選單代號, 程式代號)（zm_file）
    menu_items: HashSet<(String, String)>,
    /// p_query 查詢定義代號（zai_file）
    query_definitions: HashSet<String>,
    programs: BTreeMap<ProgramNo, Program>,
    /// (程式代號, 語系) → (名稱, 報表抬頭)（gaz_file）
    names: BTreeMap<(String, String), (String, Option<String>)>,
    /// (權限類別, 程式代號)（zy_file）
    permissions: HashSet<(String, String)>,
    audit: Vec<AuditEntry>,
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
        for m in ["AIM", "AGL", "AZZ"] {
            s.add_module(m);
        }
        s.add_permission_class("ADMIN");
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

    /// 模組（gao_file）
    pub fn add_module(&self, module: &str) {
        self.lock().modules.insert(module.to_uppercase());
    }

    /// 權限類別（zw_file）
    pub fn add_permission_class(&self, class: &str) {
        self.lock().permission_classes.insert(class.to_string());
    }

    pub fn add_menu_item(&self, menu: &str, program: &str) {
        self.lock()
            .menu_items
            .insert((menu.to_string(), program.to_string()));
    }

    pub fn add_query_definition(&self, query: &str) {
        self.lock().query_definitions.insert(query.to_string());
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

#[async_trait]
impl ProgramRepository for InMemoryStore {
    async fn get(&self, no: &ProgramNo) -> Result<Option<Program>, RepoError> {
        Ok(self.lock().programs.get(no).cloned())
    }

    async fn list(&self) -> Result<Vec<Program>, RepoError> {
        Ok(self.lock().programs.values().cloned().collect())
    }

    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Program>, u64), RepoError> {
        let s = self.lock();
        // BTreeMap 依 ProgramNo（String）排序＝位元組順序，與 PostgreSQL 的 COLLATE "C" 一致
        let hits: Vec<Program> = s
            .programs
            .values()
            .filter(|p| {
                let names: Vec<ProgramName> = s
                    .names
                    .iter()
                    .filter(|((n, _), _)| n == p.program_no.as_str())
                    .map(|((_, lang), (name, report_title))| ProgramName {
                        lang: lang.clone(),
                        name: name.clone(),
                        report_title: report_title.clone(),
                    })
                    .collect();
                crate::program_query::matches_all(p, &names, filters)
            })
            .cloned()
            .collect();
        let total = hits.len() as u64;
        let page = hits
            .into_iter()
            .skip(usize::try_from(offset).unwrap_or(usize::MAX))
            .take(usize::try_from(limit).unwrap_or(usize::MAX))
            .collect();
        Ok((page, total))
    }

    async fn insert(
        &self,
        program: &Program,
        names: &[ProgramName],
        grant_to: Option<&str>,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.programs.contains_key(&program.program_no) {
            return Err(RepoError::Conflict);
        }
        let no = program.program_no.as_str().to_owned();
        s.programs
            .insert(program.program_no.clone(), program.clone());
        for n in names {
            s.names.insert(
                (no.clone(), n.lang.clone()),
                (n.name.clone(), n.report_title.clone()),
            );
        }
        if let Some(class) = grant_to {
            s.permissions.insert((class.to_owned(), no));
        }
        Ok(())
    }

    async fn update(&self, program: &Program) -> Result<(), RepoError> {
        self.lock()
            .programs
            .insert(program.program_no.clone(), program.clone());
        Ok(())
    }

    async fn change_no(&self, old: &ProgramNo, new: &ProgramNo) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.programs.contains_key(new) {
            return Err(RepoError::Conflict);
        }
        let Some(mut p) = s.programs.remove(old) else {
            return Ok(());
        };
        p.program_no = new.clone();
        s.programs.insert(new.clone(), p);
        let (old, new) = (old.as_str(), new.as_str());
        s.names = std::mem::take(&mut s.names)
            .into_iter()
            .map(|((no, lang), name)| ((rename(no, old, new), lang), name))
            .collect();
        s.permissions = std::mem::take(&mut s.permissions)
            .into_iter()
            .map(|(class, no)| (class, rename(no, old, new)))
            .collect();
        s.menu_items = std::mem::take(&mut s.menu_items)
            .into_iter()
            .map(|(menu, no)| (menu, rename(no, old, new)))
            .collect();
        Ok(())
    }

    async fn delete(&self, no: &ProgramNo, audit: &AuditEntry) -> Result<(), RepoError> {
        let mut s = self.lock();
        s.programs.remove(no);
        let no = no.as_str();
        s.names.retain(|(n, _), _| n != no);
        s.permissions.retain(|(_, n)| n != no);
        s.menu_items.retain(|(_, n)| n != no);
        s.audit.push(audit.clone());
        Ok(())
    }

    async fn names(&self, no: &ProgramNo) -> Result<Vec<ProgramName>, RepoError> {
        Ok(self
            .lock()
            .names
            .iter()
            .filter(|((n, _), _)| n == no.as_str())
            .map(|((_, lang), (name, report_title))| ProgramName {
                lang: lang.clone(),
                name: name.clone(),
                report_title: report_title.clone(),
            })
            .collect())
    }

    async fn set_name(&self, no: &ProgramNo, name: &ProgramName) -> Result<(), RepoError> {
        self.lock().names.insert(
            (no.as_str().to_owned(), name.lang.clone()),
            (name.name.clone(), name.report_title.clone()),
        );
        Ok(())
    }

    async fn grant(&self, permission_class: &str, no: &ProgramNo) -> Result<(), RepoError> {
        self.lock()
            .permissions
            .insert((permission_class.to_owned(), no.as_str().to_owned()));
        Ok(())
    }

    async fn permission_classes(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError> {
        let mut v: Vec<String> = self
            .lock()
            .permissions
            .iter()
            .filter(|(_, n)| n == no.as_str())
            .map(|(c, _)| c.clone())
            .collect();
        v.sort();
        Ok(v)
    }

    async fn menus_containing(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError> {
        let mut v: Vec<String> = self
            .lock()
            .menu_items
            .iter()
            .filter(|(_, n)| n == no.as_str())
            .map(|(m, _)| m.clone())
            .collect();
        v.sort();
        Ok(v)
    }

    async fn audit_entries(&self, key: &str) -> Result<Vec<AuditEntry>, RepoError> {
        Ok(self
            .lock()
            .audit
            .iter()
            .filter(|a| a.key == key)
            .cloned()
            .collect())
    }

    async fn module_exists(&self, module: &ModuleCode) -> Result<bool, RepoError> {
        Ok(self.lock().modules.contains(module.as_str()))
    }

    async fn permission_class_exists(&self, class: &str) -> Result<bool, RepoError> {
        Ok(self.lock().permission_classes.contains(class))
    }

    async fn query_definition_exists(&self, query: &str) -> Result<bool, RepoError> {
        Ok(self.lock().query_definitions.contains(query))
    }
}

fn rename(no: String, old: &str, new: &str) -> String {
    if no == old { new.to_owned() } else { no }
}
