//! 記憶體版 repository，行為需與 PostgreSQL 版一致（含唯一鍵衝突）。

use chrono::{DateTime, NaiveDate, Utc};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use ttrs_domain::{
    authorization::DataScope,
    department::{Department, DeptCode},
    dictionary::{ColumnText, TableDictionary, TableText},
    employee::{Employee, EmployeeId},
    item::{Item, ItemNo},
    lang::Lang,
    legacy_catalog::LegacyTable,
    permission::{PermissionAttrs, ProgramPermission, parse_actions},
    permission_class::{ClassCode, ClassIdleControl, PermissionClass, PermissionClassAttrs},
    plant::{DEFAULT_TIME_ZONE, Plant, PlantAttrs, PlantId, PlantSchema},
    program::{ModuleCode, OtherSettings, Program, ProgramAttrs, ProgramNo},
    qbe::QbeFilter,
    user::{User, UserId},
    user_permission::{SpecialTarget, UserPermission},
};

use crate::{
    error::RepoError,
    ports::{
        AccountPolicy, AccountPolicyRepository, AuditEntry, Credential, CredentialRepository,
        DepartmentRepository, DictionaryRepository, EmployeeRepository, ItemRepository,
        LegacyCatalogRepository, LegacyTableSummary, MasterDataRepository, MigrationSource,
        PermissionClassRepository, PermissionListItem, PlantRepository, ProgramName,
        ProgramPermissionRepository, ProgramRepository, SchemaCatalog, TableSchema,
        UserPermissionItem, UserPermissionRepository, UserRepository,
    },
    query,
};
use crate::{
    mail::{Mail, Mailer},
    ports::{AuditLog, PasswordResetTokenRepository, ResetToken},
    ports::{ClassificationCode, ClassificationCodeRepository, Code, CodeKind},
};

#[derive(Default)]
struct State {
    /// 使用者（zx_file）與可使用營運中心（zxy_file）
    users: BTreeMap<UserId, User>,
    user_plants: BTreeMap<UserId, Vec<PlantId>>,
    /// Web 密碼（ADR 0009）；使用者刪除時一併刪除
    credentials: BTreeMap<UserId, Credential>,
    /// 連線版本（user_credential.session_version，ADR 0010 R6）
    session_versions: BTreeMap<UserId, u32>,
    /// 權杖雜湊 → 重設權杖（password_reset_token）
    reset_tokens: BTreeMap<String, ResetToken>,
    account_policy: AccountPolicy,
    /// (使用者, 項次) → 使用者特殊權限（zxw_file）
    user_permissions: BTreeMap<(UserId, i32), UserPermission>,
    /// TipTop 對照目錄（舊表代碼 → 對照）
    legacy: BTreeMap<String, LegacyTable>,
    /// 營運中心（azp_file）
    plants: BTreeMap<PlantId, Plant>,
    /// 已建立的工廠 schema（PostgreSQL 版為 schema＋資料庫角色＋工廠 migration，ADR 0007）
    plant_schemas: HashSet<String>,
    /// 部門（gem_file）、員工（gen_file）：工廠層，以營運中心區分
    departments: BTreeMap<(PlantId, DeptCode), Department>,
    /// (營運中心, 碼類別, 代號) → 分類碼（azf_file，工廠層）
    classification_codes: BTreeMap<(PlantId, String, String), ClassificationCode>,
    employees: BTreeMap<(PlantId, EmployeeId), Employee>,
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
        for m in ["AIM", "AGL", "AZZ", "AOO"] {
            s.add_module(m);
        }
        s.add_permission_class("ADMIN");
        s.add_ported_programs(Some("ADMIN"));
        s
    }

    /// 登錄 TTRS 已移植的作業（AQ11，同 migrations/0015）；`grant_to` 有值時給該權限類別全部功能、資料權限 0／0／0、單身 0。
    /// 已存在的程式不變
    pub fn add_ported_programs(&self, grant_to: Option<&str>) {
        let today = chrono::Local::now().date_naive();
        let mut s = self.lock();
        for (no, tiptop, module, actions) in crate::authorization::PORTED_PROGRAMS {
            let no = ProgramNo::parse(no).expect("程式代號");
            let attrs = ProgramAttrs {
                program_type: Some("I".to_owned()),
                tiptop_program_no: Some((*tiptop).to_owned()),
                other: OtherSettings {
                    default_actions: Some((*actions).to_owned()),
                    ..OtherSettings::default()
                },
                ..ProgramAttrs::default()
            };
            let module = ModuleCode::parse(module).expect("模組");
            let program =
                Program::new(no.clone(), module, attrs, "tiptop", "", today).expect("程式");
            s.programs.entry(no.clone()).or_insert(program);
            if let Some(class) = grant_to {
                let class = ClassCode::parse(class).expect("權限類別代號");
                let attrs = PermissionAttrs::defaults(&parse_actions([*actions]));
                let p = ProgramPermission::new(class, no, attrs, "tiptop", "", today);
                s.permissions.entry(perm_key(&p)).or_insert(p);
            }
        }
    }

    /// 測試用：使用者可使用的營運中心數
    pub fn user_plant_count(&self, id: &UserId) -> usize {
        self.lock().user_plants.get(id).map_or(0, Vec::len)
    }

    /// 營運中心（azp_file）：名稱同代號、資料庫代碼為下一個未使用的號碼，其餘為資料庫預設值
    pub fn add_plant(&self, plant: &str) {
        let id = PlantId::parse(plant).expect("營運中心代號");
        let mut s = self.lock();
        let used: Vec<PlantSchema> = s.plants.values().map(|p| p.schema.clone()).collect();
        let schema = PlantSchema::next_free(&used).expect("資料庫代碼");
        s.plant_schemas.insert(schema.as_str().to_owned());
        let p = Plant {
            schema,
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
        s.plants.insert(id, p);
    }

    /// 測試用：工廠 schema 是否已建立
    pub fn plant_schema_exists(&self, schema: &str) -> bool {
        self.lock().plant_schemas.contains(schema)
    }

    /// 可使用營運中心加入 ALL（zxy03＝ALL，sign_in.feature SQ5）
    pub fn set_user_all_plants(&self, user: &str) {
        let id = UserId::parse(user).expect("使用者代號");
        let mut s = self.lock();
        let plants = s.user_plants.entry(id).or_default();
        if !plants.iter().any(PlantId::is_all) {
            plants.push(PlantId::all());
        }
    }

    /// 直接設定有效日期（不經使用者維護，資料有效不變）
    pub fn set_user_valid_until(&self, user: &str, date: Option<NaiveDate>) {
        let id = UserId::parse(user).expect("使用者代號");
        if let Some(u) = self.lock().users.get_mut(&id) {
            u.attrs.valid_until = date;
        }
    }

    /// 帳號稽核原則（gbt_file）
    pub fn set_account_policy(&self, policy: AccountPolicy) {
        self.lock().account_policy = policy;
    }

    /// 使用者目前的密碼狀態（BDD 驗證用）
    pub fn credential_of(&self, user: &str) -> Option<Credential> {
        let id = UserId::parse(user).expect("使用者代號");
        self.lock().credentials.get(&id).cloned()
    }

    /// 測試用：已存的重設權杖雜湊
    pub fn reset_token_hashes(&self) -> Vec<String> {
        self.lock().reset_tokens.keys().cloned().collect()
    }

    /// 測試用：重設權杖的建立與到期時間往前推（模擬時間經過）
    pub fn age_reset_tokens(&self, minutes: i64) {
        let d = chrono::Duration::minutes(minutes);
        for t in self.lock().reset_tokens.values_mut() {
            t.created_at -= d;
            t.expires_at -= d;
        }
    }

    /// 直接設定密碼更新日（模擬先前變更的密碼）
    pub fn set_password_changed_on(&self, user: &str, date: NaiveDate) {
        let id = UserId::parse(user).expect("使用者代號");
        if let Some(c) = self.lock().credentials.get_mut(&id) {
            c.changed_on = Some(date);
        }
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

    async fn find_by_tiptop_no(&self, tiptop_no: &str) -> Result<Option<ProgramNo>, RepoError> {
        Ok(self
            .lock()
            .programs
            .values()
            .find(|p| p.tiptop_program_no.as_deref() == Some(tiptop_no))
            .map(|p| p.program_no.clone()))
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
        scope: &DataScope,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<User>, u64), RepoError> {
        let hits = self
            .lock()
            .users
            .values()
            .filter(|x| scope.includes(&x.created_by, &x.created_dept))
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
        s.credentials.remove(id);
        s.session_versions.remove(id);
        s.reset_tokens.retain(|_, t| t.user_id != id.as_str());
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
            .find(|u| {
                u.attrs
                    .idp_username
                    .as_deref()
                    .is_some_and(|a| a.eq_ignore_ascii_case(username))
            })
            .map(|u| u.user_id.clone()))
    }

    async fn find_by_idp_subject(&self, subject: &str) -> Result<Option<UserId>, RepoError> {
        Ok(self
            .lock()
            .users
            .values()
            .find(|u| u.idp_subject.as_deref() == Some(subject))
            .map(|u| u.user_id.clone()))
    }

    async fn bind_idp_subject(&self, id: &UserId, subject: &str) -> Result<(), RepoError> {
        if let Some(u) = self.lock().users.get_mut(id) {
            u.idp_subject = Some(subject.to_owned());
        }
        Ok(())
    }

    async fn set_default_plant(&self, id: &UserId, plant: &PlantId) -> Result<(), RepoError> {
        if let Some(u) = self.lock().users.get_mut(id) {
            u.attrs.default_plant = plant.clone();
        }
        Ok(())
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

    async fn schema_names(&self) -> Result<Vec<PlantSchema>, RepoError> {
        Ok(self
            .lock()
            .plants
            .values()
            .map(|p| p.schema.clone())
            .collect())
    }

    async fn insert(&self, plant: &Plant) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.plants.contains_key(&plant.id) || s.plants.values().any(|p| p.schema == plant.schema) {
            return Err(RepoError::Conflict);
        }
        s.plant_schemas.insert(plant.schema.as_str().to_owned());
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
        if self.in_use(id).await? {
            return Err(RepoError::InUse);
        }
        let mut s = self.lock();
        if let Some(p) = s.plants.remove(id) {
            s.plant_schemas.remove(p.schema.as_str());
        }
        Ok(())
    }

    /// 與 PostgreSQL 一致：使用者預設／可使用營運中心（外鍵）、工廠 schema 內有資料（料件、部門、員工）
    async fn in_use(&self, id: &PlantId) -> Result<bool, RepoError> {
        let s = self.lock();
        Ok(s.users.values().any(|u| &u.attrs.default_plant == id)
            || s.user_plants.values().any(|ps| ps.contains(id))
            || s.items.keys().any(|(p, _)| p == id)
            || s.departments.keys().any(|(p, _)| p == id)
            || s.employees.keys().any(|(p, _)| p == id))
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

#[async_trait]
impl ClassificationCodeRepository for InMemoryStore {
    async fn list_codes(
        &self,
        plant: &PlantId,
        kind: CodeKind,
    ) -> Result<Vec<ClassificationCode>, RepoError> {
        // BTreeMap 依代號排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(self
            .lock()
            .classification_codes
            .iter()
            .filter(|((p, k, _), _)| p == plant && k == kind.code())
            .map(|(_, c)| c.clone())
            .collect())
    }

    async fn get_code(
        &self,
        plant: &PlantId,
        kind: CodeKind,
        code: &Code,
    ) -> Result<Option<ClassificationCode>, RepoError> {
        Ok(self
            .lock()
            .classification_codes
            .get(&code_key(plant, kind, code))
            .cloned())
    }

    async fn insert_code(&self, plant: &PlantId, c: &ClassificationCode) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.plants.contains_key(plant) {
            return Err(RepoError::Other(format!(
                "營運中心不存在：{}",
                plant.as_str()
            )));
        }
        let key = code_key(plant, c.kind, &c.code);
        if s.classification_codes.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        s.classification_codes.insert(key, c.clone());
        Ok(())
    }

    async fn update_code(&self, plant: &PlantId, c: &ClassificationCode) -> Result<(), RepoError> {
        if let Some(old) = self
            .lock()
            .classification_codes
            .get_mut(&code_key(plant, c.kind, &c.code))
        {
            *old = c.clone();
        }
        Ok(())
    }

    async fn delete_code(
        &self,
        plant: &PlantId,
        kind: CodeKind,
        code: &Code,
    ) -> Result<bool, RepoError> {
        Ok(self
            .lock()
            .classification_codes
            .remove(&code_key(plant, kind, code))
            .is_some())
    }
}

fn code_key(plant: &PlantId, kind: CodeKind, code: &Code) -> (PlantId, String, String) {
    (
        plant.clone(),
        kind.code().to_owned(),
        code.as_str().to_owned(),
    )
}

#[async_trait]
impl DepartmentRepository for InMemoryStore {
    async fn get(&self, plant: &PlantId, code: &DeptCode) -> Result<Option<Department>, RepoError> {
        Ok(self
            .lock()
            .departments
            .get(&(plant.clone(), code.clone()))
            .cloned())
    }

    async fn query_departments(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Department>, u64), RepoError> {
        let hits = self
            .lock()
            .departments
            .iter()
            .filter(|((p, _), d)| {
                p == plant && query::matches_all(filters, |f| query::department::value(d, f))
            })
            .map(|(_, d)| d.clone())
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert(&self, plant: &PlantId, dept: &Department) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.plants.contains_key(plant) {
            return Err(RepoError::Other(format!(
                "營運中心不存在：{}",
                plant.as_str()
            )));
        }
        let key = (plant.clone(), dept.code.clone());
        if s.departments.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        s.departments.insert(key, dept.clone());
        Ok(())
    }

    async fn update(&self, plant: &PlantId, dept: &Department) -> Result<(), RepoError> {
        if let Some(d) = self
            .lock()
            .departments
            .get_mut(&(plant.clone(), dept.code.clone()))
        {
            *d = dept.clone();
        }
        Ok(())
    }

    async fn delete(&self, plant: &PlantId, code: &DeptCode) -> Result<bool, RepoError> {
        let mut s = self.lock();
        // 與 PostgreSQL 的外鍵一致：還有員工時不可刪除
        if s.employees
            .iter()
            .any(|((p, _), e)| p == plant && e.attrs.dept_code.as_ref() == Some(code))
        {
            return Err(RepoError::InUse);
        }
        Ok(s.departments
            .remove(&(plant.clone(), code.clone()))
            .is_some())
    }
}

#[async_trait]
impl EmployeeRepository for InMemoryStore {
    async fn get(&self, plant: &PlantId, id: &EmployeeId) -> Result<Option<Employee>, RepoError> {
        Ok(self
            .lock()
            .employees
            .get(&(plant.clone(), id.clone()))
            .cloned())
    }

    async fn query_employees(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Employee>, u64), RepoError> {
        let hits = self
            .lock()
            .employees
            .iter()
            .filter(|((p, _), e)| {
                p == plant && query::matches_all(filters, |f| query::employee::value(e, f))
            })
            .map(|(_, e)| e.clone())
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert(&self, plant: &PlantId, employee: &Employee) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.plants.contains_key(plant) {
            return Err(RepoError::Other(format!(
                "營運中心不存在：{}",
                plant.as_str()
            )));
        }
        let key = (plant.clone(), employee.id.clone());
        if s.employees.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        s.employees.insert(key, employee.clone());
        Ok(())
    }

    async fn update(&self, plant: &PlantId, employee: &Employee) -> Result<(), RepoError> {
        if let Some(e) = self
            .lock()
            .employees
            .get_mut(&(plant.clone(), employee.id.clone()))
        {
            *e = employee.clone();
        }
        Ok(())
    }

    async fn delete(&self, plant: &PlantId, id: &EmployeeId) -> Result<bool, RepoError> {
        Ok(self
            .lock()
            .employees
            .remove(&(plant.clone(), id.clone()))
            .is_some())
    }
}

#[async_trait]
impl CredentialRepository for InMemoryStore {
    async fn credential(&self, user: &UserId) -> Result<Option<Credential>, RepoError> {
        Ok(self.lock().credentials.get(user).cloned())
    }

    async fn save_credential(
        &self,
        user: &UserId,
        credential: &Credential,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.users.contains_key(user) {
            return Err(RepoError::Other(format!("使用者不存在：{}", user.as_str())));
        }
        s.credentials.insert(user.clone(), credential.clone());
        Ok(())
    }

    async fn session_version(&self, user: &UserId) -> Result<u32, RepoError> {
        Ok(self.lock().session_versions.get(user).copied().unwrap_or(0))
    }

    async fn end_sessions(&self, user: &UserId) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.users.contains_key(user) {
            return Err(RepoError::Other(format!("使用者不存在：{}", user.as_str())));
        }
        *s.session_versions.entry(user.clone()).or_insert(0) += 1;
        Ok(())
    }
}

#[async_trait]
impl PasswordResetTokenRepository for InMemoryStore {
    async fn issue_reset_token(
        &self,
        token_hash: &str,
        token: &ResetToken,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        for t in s.reset_tokens.values_mut() {
            if t.user_id == token.user_id && t.used_at.is_none() {
                t.used_at = Some(token.created_at);
            }
        }
        if s.reset_tokens.contains_key(token_hash) {
            return Err(RepoError::Conflict);
        }
        s.reset_tokens.insert(token_hash.to_owned(), token.clone());
        Ok(())
    }

    async fn reset_token(&self, token_hash: &str) -> Result<Option<ResetToken>, RepoError> {
        Ok(self.lock().reset_tokens.get(token_hash).cloned())
    }

    async fn reset_requests_since(
        &self,
        user: &UserId,
        since: DateTime<Utc>,
    ) -> Result<u64, RepoError> {
        let n = self
            .lock()
            .reset_tokens
            .values()
            .filter(|t| t.user_id == user.as_str() && t.created_at > since)
            .count();
        Ok(n as u64)
    }

    async fn revoke_reset_tokens(&self, user: &UserId, at: DateTime<Utc>) -> Result<(), RepoError> {
        for t in self.lock().reset_tokens.values_mut() {
            if t.user_id == user.as_str() && t.used_at.is_none() {
                t.used_at = Some(at);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl AuditLog for InMemoryStore {
    async fn record(&self, entry: &AuditEntry) -> Result<(), RepoError> {
        self.lock().audit.push(entry.clone());
        Ok(())
    }
}

/// 記憶體版寄信：信件留在寄件匣，供測試與沒有 SMTP 的本機開發檢視
#[derive(Clone, Default)]
pub struct Outbox(Arc<Mutex<Vec<Mail>>>);

impl Outbox {
    pub fn sent(&self) -> Vec<Mail> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[async_trait]
impl Mailer for Outbox {
    async fn send(&self, mail: &Mail) -> Result<(), RepoError> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(mail.clone());
        Ok(())
    }
}

#[async_trait]
impl AccountPolicyRepository for InMemoryStore {
    async fn account_policy(&self) -> Result<AccountPolicy, RepoError> {
        Ok(self.lock().account_policy.clone())
    }
}
