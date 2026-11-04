//! 組裝各作業的 use case：同一份資料存取實作（PostgreSQL 或記憶體）＋權限檢查（ADR 0006 §4）

use std::sync::Arc;

use crate::{
    Authorizer, DataDictionaryService, DepartmentService, EmployeeService, ItemService,
    LegacyCatalogService, PermissionAuthorizer, PermissionClassService, PlantService,
    ProgramPermissionService, ProgramService, SignInService, UserPermissionService, UserService,
    ports::{
        AccountPolicyRepository, CredentialRepository, DepartmentRepository, DictionaryRepository,
        EmployeeRepository, ItemRepository, LegacyCatalogRepository, MasterDataRepository,
        PermissionClassRepository, PlantRepository, ProgramPermissionRepository, ProgramRepository,
        SchemaCatalog, UserPermissionRepository, UserRepository,
    },
};

/// 實作全部 repository 的資料存取（ttrs_infra::PgStore、memory::InMemoryStore）
pub trait Store:
    ItemRepository
    + MasterDataRepository
    + ProgramRepository
    + SchemaCatalog
    + DictionaryRepository
    + LegacyCatalogRepository
    + UserRepository
    + PermissionClassRepository
    + PlantRepository
    + ProgramPermissionRepository
    + UserPermissionRepository
    + DepartmentRepository
    + EmployeeRepository
    + CredentialRepository
    + AccountPolicyRepository
    + 'static
{
}

impl<T> Store for T where
    T: ItemRepository
        + MasterDataRepository
        + ProgramRepository
        + SchemaCatalog
        + DictionaryRepository
        + LegacyCatalogRepository
        + UserRepository
        + PermissionClassRepository
        + PlantRepository
        + ProgramPermissionRepository
        + UserPermissionRepository
        + DepartmentRepository
        + EmployeeRepository
        + CredentialRepository
        + AccountPolicyRepository
        + 'static
{
}

#[derive(Clone)]
pub struct Services {
    pub items: ItemService,
    pub programs: ProgramService,
    pub dictionary: DataDictionaryService,
    pub legacy: LegacyCatalogService,
    pub users: UserService,
    pub classes: PermissionClassService,
    pub plants: PlantService,
    pub permissions: ProgramPermissionService,
    pub special: UserPermissionService,
    pub departments: DepartmentService,
    pub employees: EmployeeService,
    pub sign_in: SignInService,
    pub authorizer: Arc<dyn Authorizer>,
}

impl Services {
    /// 依權限資料檢查（zy_file、zxw_file）
    pub fn new<S: Store>(store: Arc<S>) -> Self {
        let authorizer = Arc::new(PermissionAuthorizer::new(store.clone(), store.clone()));
        Self::with_authorizer(store, authorizer)
    }

    pub fn with_authorizer<S: Store>(store: Arc<S>, authorizer: Arc<dyn Authorizer>) -> Self {
        let a = authorizer.clone();
        Self {
            items: ItemService::new(a.clone(), store.clone(), store.clone()),
            programs: ProgramService::new(a.clone(), store.clone()),
            dictionary: DataDictionaryService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
            ),
            legacy: LegacyCatalogService::new(a.clone(), store.clone()),
            users: UserService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
            ),
            classes: PermissionClassService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
            ),
            plants: PlantService::new(a.clone(), store.clone()),
            permissions: ProgramPermissionService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
            ),
            departments: DepartmentService::new(a.clone(), store.clone(), store.clone()),
            sign_in: SignInService::new(store.clone(), store.clone(), store.clone(), store.clone()),
            employees: EmployeeService::new(a.clone(), store.clone(), store.clone(), store.clone()),
            special: UserPermissionService::new(
                a,
                store.clone(),
                store.clone(),
                store.clone(),
                store,
            ),
            authorizer,
        }
    }
}
