//! 記憶體版 repository，行為需與 PostgreSQL 版一致（含唯一鍵衝突）。

use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use ttrs_domain::{
    dictionary::{ColumnText, TableDictionary, TableText},
    item::{Item, ItemNo},
    lang::Lang,
    legacy_catalog::LegacyTable,
    permission::ProgramPermission,
    permission_class::{ClassCode, ClassIdleControl, PermissionClass, PermissionClassAttrs},
    plant::{DEFAULT_TIME_ZONE, Plant, PlantAttrs, PlantId},
    program::{ModuleCode, Program, ProgramNo},
    qbe::QbeFilter,
    user::{User, UserId},
    user_permission::{SpecialTarget, UserPermission},
};

use crate::{
    error::RepoError,
    ports::{
        AuditEntry, DictionaryRepository, ItemRepository, LegacyCatalogRepository,
        LegacyTableSummary, MasterDataRepository, MigrationSource, PermissionClassRepository,
        PermissionListItem, PlantRepository, ProgramName, ProgramPermissionRepository,
        ProgramRepository, SchemaCatalog, TableSchema, UserPermissionItem,
        UserPermissionRepository, UserRepository,
    },
    query,
};

#[derive(Default)]
struct State {
    /// 使用者（zx_file）與可使用營運中心（zxy_file）
    users: BTreeMap<UserId, User>,
    user_plants: BTreeMap<UserId, Vec<PlantId>>,
    /// (使用者, 項次) → 使用者特殊權限（zxw_file）
    user_permissions: BTreeMap<(UserId, i32), UserPermission>,
    /// TipTop 對照目錄（舊表代碼 → 對照）
    legacy: BTreeMap<String, LegacyTable>,
    /// 營運中心（azp_file）
    plants: BTreeMap<PlantId, Plant>,
    units: HashSet<String>,
    items: BTreeMap<(PlantId, ItemNo), Item>,
    transactions: HashSet<(PlantId, ItemNo)>,
    // Program Maintenance
    modules: HashSet<String>,
    /// 權限類別（zw_file）
    permission_classes: BTreeMap<ClassCode, PermissionClass>,
    /// (選單代號, 程式代號)（zm_file）
    menu_items: HashSet<(String, String)>,
    /// p_query 查詢定義代號（zai_file）
    query_definitions: HashSet<String>,
    programs: BTreeMap<ProgramNo, Program>,
    /// (程式代號, 語系) → (名稱, 報表抬頭)（gaz_file）
    names: BTreeMap<(String, Lang), (String, Option<String>)>,
    /// (權限類別, 程式代號)（zy_file）
    /// (權限類別, 程式代號) → 程式執行權限（zy_file）
    permissions: BTreeMap<(String, String), ProgramPermission>,
    audit: Vec<AuditEntry>,
    // 資料字典：資料庫結構與 migration 由測試提供（記憶體版無法讀取真正的資料庫）
    schema: BTreeMap<String, TableSchema>,
    migrations: Vec<MigrationSource>,
    dictionaries: BTreeMap<String, TableDictionary>,
    /// (資料表, 語系) → 表說明
    table_texts: BTreeMap<(String, String), TableText>,
    /// (資料表, 欄位, 語系) → 欄位說明
    column_texts: BTreeMap<(String, String, String), ColumnText>,
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

    /// 測試用：使用者可使用的營運中心數
    pub fn user_plant_count(&self, id: &UserId) -> usize {
        self.lock().user_plants.get(id).map_or(0, Vec::len)
    }

    /// 營運中心（azp_file）：名稱同代號、其餘為資料庫預設值（同 migrations/0012 的欄位預設）
    pub fn add_plant(&self, plant: &str) {
        let id = PlantId::parse(plant).expect("營運中心代號");
        let p = Plant {
            attrs: PlantAttrs {
                name: id.as_str().to_owned(),
                time_zone: DEFAULT_TIME_ZONE.to_owned(),
                address: None,
                phone: None,
                fax: None,
            },
            active: true,
            created_by: String::new(),
            created_dept: String::new(),
            created_on: chrono::Local::now().date_naive(),
            updated_by: None,
            updated_on: None,
            id: id.clone(),
        };
        self.lock().plants.insert(id, p);
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

    /// 權限類別（zw_file）：說明同代號、其餘為資料庫預設值（同 migrations/0011 的欄位預設）
    pub fn add_permission_class(&self, class: &str) {
        let code = ClassCode::parse(class).expect("權限類別代號");
        let pc = PermissionClass {
            attrs: PermissionClassAttrs {
                name: class.to_string(),
                default_menu: None,
                idle_control: ClassIdleControl::Off,
                idle_action: None,
                idle_seconds: None,
            },
            active: true,
            created_by: String::new(),
            created_dept: String::new(),
            created_on: chrono::Local::now().date_naive(),
            updated_by: None,
            updated_on: None,
            code: code.clone(),
        };
        self.lock().permission_classes.insert(code, pc);
    }

    /// 測試用：使用者的特殊權限筆數
    pub fn user_permission_count(&self, user: &str) -> usize {
        self.lock()
            .user_permissions
            .keys()
            .filter(|(u, _)| u.as_str() == user)
            .count()
    }

    /// 測試用：權限類別的程式權限筆數
    pub fn class_permission_count(&self, class: &str) -> usize {
        self.lock()
            .permissions
            .keys()
            .filter(|(c, _)| c == class)
            .count()
    }

    pub fn add_menu_item(&self, menu: &str, program: &str) {
        self.lock()
            .menu_items
            .insert((menu.to_string(), program.to_string()));
    }

    pub fn add_query_definition(&self, query: &str) {
        self.lock().query_definitions.insert(query.to_string());
    }

    /// 測試用：登錄一張資料表的實際結構（PostgreSQL 版由系統目錄讀取）
    pub fn add_table_schema(&self, table: TableSchema) {
        self.lock().schema.insert(table.name.clone(), table);
    }

    /// 測試用：登錄 migration 內容（PostgreSQL 版取自 sqlx 內嵌的 migrations）
    pub fn add_migration(&self, migration: MigrationSource) {
        self.lock().migrations.push(migration);
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
        Ok(self.lock().plants.get(plant).is_some_and(|p| p.active))
    }

    async fn unit_exists(&self, unit: &str) -> Result<bool, RepoError> {
        Ok(self.lock().units.contains(&unit.to_uppercase()))
    }
}

/// 依序結果取指定範圍，並回傳總筆數
fn page_of<T>(hits: Vec<T>, offset: u64, limit: u64) -> (Vec<T>, u64) {
    let total = hits.len() as u64;
    let page = hits
        .into_iter()
        .skip(usize::try_from(offset).unwrap_or(usize::MAX))
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .collect();
    (page, total)
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
        let hits = s
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
        Ok(page_of(hits, offset, limit))
    }

    async fn insert(
        &self,
        program: &Program,
        names: &[ProgramName],
        grant_to: Option<&ProgramPermission>,
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
        if let Some(p) = grant_to {
            s.permissions.insert(perm_key(p), p.clone());
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
        let renamed = ProgramNo::parse(new).expect("程式代號");
        s.permissions = std::mem::take(&mut s.permissions)
            .into_iter()
            .map(|((class, no), mut p)| {
                if no == old {
                    p.program = renamed.clone();
                }
                ((class, rename(no, old, new)), p)
            })
            .collect();
        s.menu_items = std::mem::take(&mut s.menu_items)
            .into_iter()
            .map(|(menu, no)| (menu, rename(no, old, new)))
            .collect();
        // 特殊權限的程式列（同 PostgreSQL 的 ON UPDATE CASCADE）
        for up in s.user_permissions.values_mut() {
            if let SpecialTarget::Program { program, .. } = &mut up.target
                && program.as_str() == old
            {
                *program = renamed.clone();
            }
        }
        Ok(())
    }

    async fn delete(&self, no: &ProgramNo, audit: &AuditEntry) -> Result<(), RepoError> {
        let mut s = self.lock();
        s.programs.remove(no);
        let no = no.as_str();
        s.names.retain(|(n, _), _| n != no);
        s.permissions.retain(|(_, n), _| n != no);
        s.menu_items.retain(|(_, n)| n != no);
        s.user_permissions
            .retain(|_, up| up.target.program().is_none_or(|p| p.as_str() != no));
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

    async fn grant(&self, permission: &ProgramPermission) -> Result<(), RepoError> {
        self.lock()
            .permissions
            .entry(perm_key(permission))
            .or_insert_with(|| permission.clone());
        Ok(())
    }

    async fn permission_classes(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError> {
        let mut v: Vec<String> = self
            .lock()
            .permissions
            .keys()
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

    async fn menu_children(&self, menu: &str) -> Result<Vec<Program>, RepoError> {
        let s = self.lock();
        // programs 為 BTreeMap，依程式代號排序
        Ok(s.programs
            .values()
            .filter(|p| {
                s.menu_items
                    .contains(&(menu.to_owned(), p.program_no.as_str().to_owned()))
            })
            .cloned()
            .collect())
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
        Ok(ClassCode::parse(class).is_ok_and(|c| self.lock().permission_classes.contains_key(&c)))
    }

    async fn query_definition_exists(&self, query: &str) -> Result<bool, RepoError> {
        Ok(self.lock().query_definitions.contains(query))
    }
}

fn perm_key(p: &ProgramPermission) -> (String, String) {
    (p.class.as_str().to_owned(), p.program.as_str().to_owned())
}

fn rename(no: String, old: &str, new: &str) -> String {
    if no == old { new.to_owned() } else { no }
}

#[async_trait]
impl SchemaCatalog for InMemoryStore {
    async fn table_names(&self) -> Result<Vec<String>, RepoError> {
        Ok(self.lock().schema.keys().cloned().collect())
    }

    async fn table(&self, name: &str) -> Result<Option<TableSchema>, RepoError> {
        Ok(self.lock().schema.get(name).cloned())
    }

    async fn migrations(&self) -> Result<Vec<MigrationSource>, RepoError> {
        let mut v = self.lock().migrations.clone();
        v.sort_by_key(|m| m.version);
        Ok(v)
    }
}

#[async_trait]
impl DictionaryRepository for InMemoryStore {
    async fn get(&self, table: &str) -> Result<Option<TableDictionary>, RepoError> {
        Ok(self.lock().dictionaries.get(table).cloned())
    }

    async fn list(&self) -> Result<Vec<TableDictionary>, RepoError> {
        Ok(self.lock().dictionaries.values().cloned().collect())
    }

    async fn insert(&self, dict: &TableDictionary) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.dictionaries.contains_key(&dict.table_name) {
            return Err(RepoError::Conflict);
        }
        s.dictionaries.insert(dict.table_name.clone(), dict.clone());
        Ok(())
    }

    async fn update(&self, dict: &TableDictionary) -> Result<(), RepoError> {
        self.lock()
            .dictionaries
            .insert(dict.table_name.clone(), dict.clone());
        Ok(())
    }

    async fn delete(&self, table: &str) -> Result<(), RepoError> {
        let mut s = self.lock();
        s.dictionaries.remove(table);
        s.table_texts.retain(|(t, _), _| t != table);
        s.column_texts.retain(|(t, _, _), _| t != table);
        Ok(())
    }

    async fn set_table_text(&self, table: &str, text: &TableText) -> Result<(), RepoError> {
        self.lock().table_texts.insert(
            (table.to_owned(), text.lang.as_str().to_owned()),
            text.clone(),
        );
        Ok(())
    }

    async fn table_texts(&self, table: &str) -> Result<Vec<TableText>, RepoError> {
        Ok(self
            .lock()
            .table_texts
            .iter()
            .filter(|((t, _), _)| t == table)
            .map(|(_, v)| v.clone())
            .collect())
    }

    async fn set_column_text(&self, table: &str, text: &ColumnText) -> Result<(), RepoError> {
        self.lock().column_texts.insert(
            (
                table.to_owned(),
                text.column_name.clone(),
                text.lang.as_str().to_owned(),
            ),
            text.clone(),
        );
        Ok(())
    }

    async fn column_texts(&self, table: &str) -> Result<Vec<ColumnText>, RepoError> {
        Ok(self
            .lock()
            .column_texts
            .iter()
            .filter(|((t, _, _), _)| t == table)
            .map(|(_, v)| v.clone())
            .collect())
    }

    async fn module_exists(&self, module: &ModuleCode) -> Result<bool, RepoError> {
        Ok(self.lock().modules.contains(module.as_str()))
    }
}

#[async_trait]
impl LegacyCatalogRepository for InMemoryStore {
    async fn replace_all(&self, tables: &[LegacyTable]) -> Result<(), RepoError> {
        self.lock().legacy = tables
            .iter()
            .map(|t| (t.legacy_name.clone(), t.clone()))
            .collect();
        Ok(())
    }

    async fn list_tables(&self) -> Result<Vec<LegacyTableSummary>, RepoError> {
        // BTreeMap 依舊表代碼排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(self
            .lock()
            .legacy
            .values()
            .map(|t| LegacyTableSummary {
                legacy_name: t.legacy_name.clone(),
                name: t.name.clone(),
                title: t.title.clone(),
                module: t.module.clone(),
                column_count: u32::try_from(t.columns.len()).unwrap_or(u32::MAX),
            })
            .collect())
    }

    async fn get(&self, legacy_name: &str) -> Result<Option<LegacyTable>, RepoError> {
        Ok(self.lock().legacy.get(legacy_name).cloned())
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<LegacyTable>, RepoError> {
        Ok(self
            .lock()
            .legacy
            .values()
            .find(|t| t.name == name)
            .cloned())
    }
}

#[async_trait]
impl UserRepository for InMemoryStore {
    async fn get(&self, id: &UserId) -> Result<Option<User>, RepoError> {
        Ok(self.lock().users.get(id).cloned())
    }

    async fn list(&self) -> Result<Vec<User>, RepoError> {
        // BTreeMap 依使用者代號排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(self.lock().users.values().cloned().collect())
    }

    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<User>, u64), RepoError> {
        let hits = self
            .lock()
            .users
            .values()
            .filter(|x| query::matches_all(filters, |f| query::user::value(x, f)))
            .cloned()
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert(&self, user: &User, plants: &[PlantId]) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.users.contains_key(&user.user_id) {
            return Err(RepoError::Conflict);
        }
        s.users.insert(user.user_id.clone(), user.clone());
        s.user_plants.insert(user.user_id.clone(), plants.to_vec());
        Ok(())
    }

    async fn update(&self, user: &User, plants: &[PlantId]) -> Result<(), RepoError> {
        let mut s = self.lock();
        s.users.insert(user.user_id.clone(), user.clone());
        s.user_plants.insert(user.user_id.clone(), plants.to_vec());
        Ok(())
    }

    async fn delete(&self, id: &UserId) -> Result<(), RepoError> {
        let mut s = self.lock();
        s.users.remove(id);
        s.user_plants.remove(id);
        s.user_permissions.retain(|(u, _), _| u != id);
        Ok(())
    }

    async fn plants(&self, id: &UserId) -> Result<Vec<PlantId>, RepoError> {
        // 與 PostgreSQL 一致：依營運中心代號排序
        let mut plants = self.lock().user_plants.get(id).cloned().unwrap_or_default();
        plants.sort();
        Ok(plants)
    }

    async fn find_by_idp_username(&self, username: &str) -> Result<Option<UserId>, RepoError> {
        Ok(self
            .lock()
            .users
            .values()
            .find(|u| u.attrs.idp_username.as_deref() == Some(username))
            .map(|u| u.user_id.clone()))
    }
}

#[async_trait]
impl PermissionClassRepository for InMemoryStore {
    async fn get(&self, code: &ClassCode) -> Result<Option<PermissionClass>, RepoError> {
        Ok(self.lock().permission_classes.get(code).cloned())
    }

    async fn list(&self) -> Result<Vec<PermissionClass>, RepoError> {
        // BTreeMap 依代號排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(self.lock().permission_classes.values().cloned().collect())
    }

    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<PermissionClass>, u64), RepoError> {
        let hits = self
            .lock()
            .permission_classes
            .values()
            .filter(|x| query::matches_all(filters, |f| query::permission_class::value(x, f)))
            .cloned()
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert(&self, class: &PermissionClass) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.permission_classes.contains_key(&class.code) {
            return Err(RepoError::Conflict);
        }
        s.permission_classes
            .insert(class.code.clone(), class.clone());
        Ok(())
    }

    async fn update(&self, class: &PermissionClass) -> Result<(), RepoError> {
        if let Some(c) = self.lock().permission_classes.get_mut(&class.code) {
            *c = class.clone();
        }
        Ok(())
    }

    async fn delete(&self, code: &ClassCode) -> Result<(), RepoError> {
        self.lock().permission_classes.remove(code);
        Ok(())
    }

    async fn user_count(&self, code: &ClassCode) -> Result<u64, RepoError> {
        let s = self.lock();
        let direct = s
            .users
            .values()
            .filter(|u| u.attrs.permission_class == code.as_str())
            .count();
        let special: HashSet<&UserId> = s
            .user_permissions
            .values()
            .filter(|up| up.target.class() == Some(code))
            .map(|up| &up.user_id)
            .collect();
        Ok((direct + special.len()) as u64)
    }

    async fn permission_count(&self, code: &ClassCode) -> Result<u64, RepoError> {
        Ok(self.class_permission_count(code.as_str()) as u64)
    }
}

#[async_trait]
impl PlantRepository for InMemoryStore {
    async fn get(&self, id: &PlantId) -> Result<Option<Plant>, RepoError> {
        Ok(self.lock().plants.get(id).cloned())
    }

    async fn list(&self) -> Result<Vec<Plant>, RepoError> {
        // BTreeMap 依代號排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(self.lock().plants.values().cloned().collect())
    }

    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Plant>, u64), RepoError> {
        let hits = self
            .lock()
            .plants
            .values()
            .filter(|x| query::matches_all(filters, |f| query::plant::value(x, f)))
            .cloned()
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert(&self, plant: &Plant) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.plants.contains_key(&plant.id) {
            return Err(RepoError::Conflict);
        }
        s.plants.insert(plant.id.clone(), plant.clone());
        Ok(())
    }

    async fn update(&self, plant: &Plant) -> Result<(), RepoError> {
        if let Some(p) = self.lock().plants.get_mut(&plant.id) {
            *p = plant.clone();
        }
        Ok(())
    }

    async fn delete(&self, id: &PlantId) -> Result<(), RepoError> {
        let mut s = self.lock();
        // 與 PostgreSQL 的外鍵一致：使用者預設／可使用營運中心、料件（庫存異動經由料件）
        let referenced = s.users.values().any(|u| &u.attrs.default_plant == id)
            || s.user_plants.values().any(|ps| ps.contains(id))
            || s.items.keys().any(|(p, _)| p == id);
        if referenced {
            return Err(RepoError::InUse);
        }
        s.plants.remove(id);
        Ok(())
    }
}

#[async_trait]
impl ProgramPermissionRepository for InMemoryStore {
    async fn get(
        &self,
        class: &ClassCode,
        program: &ProgramNo,
    ) -> Result<Option<ProgramPermission>, RepoError> {
        let key = (class.as_str().to_owned(), program.as_str().to_owned());
        Ok(self.lock().permissions.get(&key).cloned())
    }

    async fn query_classes(
        &self,
        header: &[QbeFilter],
        detail: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<PermissionClass>, u64), RepoError> {
        let s = self.lock();
        let hits = s
            .permission_classes
            .values()
            .filter(|c| {
                query::matches_all(header, |f| query::permission_class::value(c, f))
                    && (detail.is_empty()
                        || s.permissions.values().any(|p| {
                            p.class == c.code
                                && query::matches_all(detail, |f| {
                                    query::program_permission::value(p, f)
                                })
                        }))
            })
            .cloned()
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn list(&self, class: &ClassCode) -> Result<Vec<PermissionListItem>, RepoError> {
        let s = self.lock();
        // BTreeMap 依 (權限類別, 程式代號) 排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(s.permissions
            .iter()
            .filter(|((c, _), _)| c == class.as_str())
            .map(|((_, no), p)| PermissionListItem {
                permission: p.clone(),
                default_actions: s
                    .programs
                    .get(&p.program)
                    .and_then(|prog| prog.other.default_actions.clone()),
                names: s
                    .names
                    .iter()
                    .filter(|((n, _), _)| n == no)
                    .map(|((_, lang), (name, report_title))| ProgramName {
                        lang: lang.clone(),
                        name: name.clone(),
                        report_title: report_title.clone(),
                    })
                    .collect(),
            })
            .collect())
    }

    async fn insert(&self, permission: &ProgramPermission) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = perm_key(permission);
        if s.permissions.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        s.permissions.insert(key, permission.clone());
        Ok(())
    }

    async fn update(&self, permission: &ProgramPermission) -> Result<(), RepoError> {
        if let Some(p) = self.lock().permissions.get_mut(&perm_key(permission)) {
            *p = permission.clone();
        }
        Ok(())
    }

    async fn delete(&self, class: &ClassCode, program: &ProgramNo) -> Result<bool, RepoError> {
        let key = (class.as_str().to_owned(), program.as_str().to_owned());
        Ok(self.lock().permissions.remove(&key).is_some())
    }

    async fn delete_all(&self, class: &ClassCode) -> Result<u64, RepoError> {
        let mut s = self.lock();
        let before = s.permissions.len();
        s.permissions.retain(|(c, _), _| c != class.as_str());
        Ok((before - s.permissions.len()) as u64)
    }
}

#[async_trait]
impl UserPermissionRepository for InMemoryStore {
    async fn list(&self, user: &UserId) -> Result<Vec<UserPermissionItem>, RepoError> {
        let s = self.lock();
        // BTreeMap 依 (使用者, 項次) 排序
        Ok(s.user_permissions
            .iter()
            .filter(|((u, _), _)| u == user)
            .map(|(_, up)| {
                let (class_name, default_actions, names) = match &up.target {
                    SpecialTarget::Class(c) => (
                        s.permission_classes.get(c).map(|pc| pc.attrs.name.clone()),
                        None,
                        Vec::new(),
                    ),
                    SpecialTarget::Program { program, .. } => (
                        None,
                        s.programs
                            .get(program)
                            .and_then(|p| p.other.default_actions.clone()),
                        s.names
                            .iter()
                            .filter(|((n, _), _)| n == program.as_str())
                            .map(|((_, lang), (name, report_title))| ProgramName {
                                lang: lang.clone(),
                                name: name.clone(),
                                report_title: report_title.clone(),
                            })
                            .collect(),
                    ),
                };
                UserPermissionItem {
                    permission: up.clone(),
                    class_name,
                    default_actions,
                    names,
                }
            })
            .collect())
    }

    async fn insert(&self, permission: &UserPermission) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (permission.user_id.clone(), permission.line_no);
        let same_target = s.user_permissions.values().any(|up| {
            up.user_id == permission.user_id
                && ((up.target.class().is_some() && up.target.class() == permission.target.class())
                    || (up.target.program().is_some()
                        && up.target.program() == permission.target.program()))
        });
        if s.user_permissions.contains_key(&key) || same_target {
            return Err(RepoError::Conflict);
        }
        s.user_permissions.insert(key, permission.clone());
        Ok(())
    }

    async fn update(&self, permission: &UserPermission) -> Result<(), RepoError> {
        let key = (permission.user_id.clone(), permission.line_no);
        if let Some(up) = self.lock().user_permissions.get_mut(&key) {
            *up = permission.clone();
        }
        Ok(())
    }

    async fn delete(&self, user: &UserId, line_no: i32) -> Result<bool, RepoError> {
        Ok(self
            .lock()
            .user_permissions
            .remove(&(user.clone(), line_no))
            .is_some())
    }

    async fn delete_all(&self, user: &UserId) -> Result<u64, RepoError> {
        let mut s = self.lock();
        let before = s.user_permissions.len();
        s.user_permissions.retain(|(u, _), _| u != user);
        Ok((before - s.user_permissions.len()) as u64)
    }
}
