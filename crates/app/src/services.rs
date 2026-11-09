//! 組裝各作業的 use case：同一份資料存取實作（PostgreSQL 或記憶體）＋權限檢查（ADR 0006 §4）

use std::sync::Arc;

use crate::change_log::HistoryReader;
use crate::{
    Authorizer, ClassificationCodeService, DataDictionaryService, DepartmentService,
    EmergencyService, EmployeeService, ItemService, LegacyCatalogService, PasswordResetService,
    PermissionAuthorizer, PermissionClassService, PlantService, ProgramPermissionService,
    ProgramService, SignInService, UserPermissionService, UserService,
    ports::{
        AccountPolicyRepository, AuditLog, ChangeLogRepository, ClassificationCodeRepository,
        CredentialRepository, DepartmentRepository, DictionaryRepository, EmployeeRepository,
        ItemRepository, LegacyCatalogRepository, MasterDataRepository, MenuRepository,
        PasswordResetTokenRepository, PermissionClassRepository, PlantRepository,
        ProgramPermissionRepository, ProgramRepository, SchemaCatalog, UserPermissionRepository,
        UserRepository,
    },
};
use crate::{ChangeHistoryService, MenuService};

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
    + PasswordResetTokenRepository
    + AuditLog
    + ClassificationCodeRepository
    + ChangeLogRepository
    + MenuRepository
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
        + PasswordResetTokenRepository
        + AuditLog
        + ClassificationCodeRepository
        + ChangeLogRepository
        + MenuRepository
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
    /// 分類碼（aooi300）
    pub classification: ClassificationCodeService,
    pub sign_in: SignInService,
    /// 忘記密碼（ADR 0010 §A）；寄信設定以 [`PasswordResetService::with_mail`] 接上
    pub password_reset: PasswordResetService,
    /// 伺服器端緊急重設（ttrs-admin，ADR 0010 §B）；不提供給 HTTP API
    pub emergency: EmergencyService,
    /// 共用層作業的異動記錄（ADR 0012 §8）
    pub history: ChangeHistoryService,
    /// 選單維護與側欄（p_zm）
    pub menus: MenuService,
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
        // 判斷使用者維護管理員一律依權限資料（R14），與請求的權限檢查分開
        let admins: Arc<dyn Authorizer> =
            Arc::new(PermissionAuthorizer::new(store.clone(), store.clone()));
        let history = HistoryReader::new(store.clone(), store.clone());
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
                admins,
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
            departments: DepartmentService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
            ),
            sign_in: SignInService::new(store.clone(), store.clone(), store.clone(), store.clone()),
            password_reset: PasswordResetService::new(
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
            ),
            emergency: EmergencyService::new(store.clone(), store.clone(), store.clone()),
            employees: EmployeeService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
            ),
            classification: ClassificationCodeService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
            ),
            history: ChangeHistoryService::new(a.clone(), history),
            menus: MenuService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
            ),
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
