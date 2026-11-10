//! User Maintenance 使用者維護 use case（TipTop: p_zx）
//! 規格：specs/features/azz/user_maintenance.feature（U1–U9）、user_maintenance_department.feature（UD1–UD6）、
//! user_maintenance_plants.feature（PZ1–PZ8，TipTop p_zxy）；登入與權限：docs/adr/0006

use crate::list_report::{ListReporter, ListSpec};
use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    department::DeptCode,
    lang::DEFAULT_LANG,
    password::{Credential, MAX_LEN},
    password_reset::check_keeps_admin,
    permission_class::ClassCode,
    plant::PlantId,
    program::ProgramNo,
    sign_in::check_account,
    user::{User, UserAttrs, UserId, chosen_plants, plants_after_default_change},
};
use ttrs_shared::{Page, QueryRequest, UserDto, UserInput};

use crate::{
    Actor, ActorKind, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    password_hash,
    ports::{
        CredentialRepository, DepartmentRepository, MasterDataRepository,
        PermissionClassRepository, PlantRepository, ProgramRepository, UserPermissionRepository,
        UserRepository,
    },
    query,
    user_permission_service::move_special_class,
};

/// 權限檢查用的程式代號
const PROGRAM: &str = "user-maintenance";

#[derive(Clone)]
pub struct UserService {
    authz: Arc<dyn Authorizer>,
    users: Arc<dyn UserRepository>,
    programs: Arc<dyn ProgramRepository>,
    master: Arc<dyn MasterDataRepository>,
    classes: Arc<dyn PermissionClassRepository>,
    /// 使用者特殊權限（zxw_file）：改權限類別時連動（X6）
    special: Arc<dyn UserPermissionRepository>,
    /// Web 密碼（zx10、zx20；ADR 0009）
    credentials: Arc<dyn CredentialRepository>,
    /// 部門（gem_file，工廠層）：部門代號須存在於預設營運中心（user_maintenance_department.feature UD1）
    depts: Arc<dyn DepartmentRepository>,
    /// 營運中心（azp_file）：勾選的可使用營運中心須存在且有效（user_maintenance_plants.feature PZ4）
    plants: Arc<dyn PlantRepository>,
    /// 依權限資料判斷誰是使用者維護管理員（不受測試時關閉權限檢查影響）：
    /// 至少保留一位（emergency_password_reset.feature，ADR 0010 R14）
    admins: Arc<dyn Authorizer>,
    /// 列印、匯出 Excel（ADR 0013 SA3）
    reports: ListReporter,
}

/// 清單報表（ADR 0013 SA3；user_maintenance_standard_actions.feature）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "user-report-title",
    columns: &[
        ("user_id", "user-col-id"),
        ("name", "user-col-name"),
        ("dept_code", "user-col-dept"),
        ("permission_class", "user-col-class"),
        ("default_plant", "user-col-default-plant"),
        ("active", "user-col-active"),
    ],
};

impl UserService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        authz: Arc<dyn Authorizer>,
        users: Arc<dyn UserRepository>,
        programs: Arc<dyn ProgramRepository>,
        master: Arc<dyn MasterDataRepository>,
        classes: Arc<dyn PermissionClassRepository>,
        special: Arc<dyn UserPermissionRepository>,
        credentials: Arc<dyn CredentialRepository>,
        depts: Arc<dyn DepartmentRepository>,
        plants: Arc<dyn PlantRepository>,
        admins: Arc<dyn Authorizer>,
        reports: ListReporter,
    ) -> Self {
        Self {
            reports,
            authz,
            users,
            programs,
            master,
            classes,
            special,
            credentials,
            depts,
            plants,
            admins,
        }
    }

    /// 新增（p_zx_a）：預設營運中心自動成為可使用營運中心（p_zx.4gl:490-501）
    pub async fn create(&self, actor: &Actor, input: UserInput) -> Result<UserDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let id = UserId::parse(&input.user_id)?;
            let attrs = self.attrs(&input, None).await?;
            self.check_dept(&attrs).await?;
            let password = web_password(&input)?;
            let chosen = match &input.allowed_plants {
                Some(list) => self.parse_new_plants(list, &[]).await?,
                None => vec![attrs.default_plant.clone()],
            };
            let all = input.all_plants.unwrap_or(false);
            let plants = chosen_plants(&attrs.default_plant, attrs.multi_plant, &chosen, all)?;
            let user = User::new(id, attrs, &actor.user_id, &actor.dept, today());
            match self.users.insert(&user, &plants).await {
                Ok(()) => {}
                Err(RepoError::Conflict) => return Err(DomainError::UserAlreadyExists.into()),
                Err(e) => return Err(infra(e)),
            }
            // 未輸入密碼時初始密碼＝使用者代號，須於下次登入改密碼（p_zx.4gl:447、:561，W1）
            if let Some(p) = password {
                self.set_password(&user.user_id, p).await?;
            }
            Ok(to_dto(&user, &plants))
        })
        .await
    }

    /// 解除鎖定（p_zx user_unlock，p_zx.4gl:367-400）：試誤次數歸零
    pub async fn unlock(&self, actor: &Actor, user_id: &str) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "user_unlock", async {
            self.authorize(actor, "user_unlock").await?;
            let user = self.load(user_id).await?;
            if let Some(mut cred) = self
                .credentials
                .credential(&user.user_id)
                .await
                .map_err(infra)?
            {
                cred.failed_count = 0;
                self.credentials
                    .save_credential(&user.user_id, &cred)
                    .await
                    .map_err(infra)?;
            }
            Ok(())
        })
        .await
    }

    /// 管理員設定 Web 密碼：一律須於下次登入改密碼（p_zx.4gl:640-650，W7）
    async fn set_password(&self, id: &UserId, password: &str) -> Result<(), AppError> {
        let cred = Credential::reset(password_hash::hash(password)?, today());
        self.credentials
            .save_credential(id, &cred)
            .await
            .map_err(infra)
    }

    /// 修改（p_zx_u）：代號不可變（U1）；依預設營運中心與是否多營運中心同步可使用營運中心
    pub async fn update(
        &self,
        actor: &Actor,
        user_id: &str,
        input: UserInput,
    ) -> Result<UserDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            let grant = self.authorize(actor, "modify").await?;
            let mut user = self.load(user_id).await?;
            user.stamp.version = input.version.unwrap_or(user.stamp.version);
            grant.record(&user.created_by, &user.created_dept)?;
            if !input.user_id.trim().is_empty() && input.user_id.trim() != user.user_id.as_str() {
                return Err(DomainError::UserIdImmutable.into());
            }
            let attrs = self.attrs(&input, Some(&user.user_id)).await?;
            // 部門或預設營運中心有變動才重新檢查部門，舊資料的其他欄位照常可改（UD4）
            if attrs.dept_code != user.attrs.dept_code
                || attrs.default_plant != user.attrs.default_plant
            {
                self.check_dept(&attrs).await?;
            }
            let password = web_password(&input)?;
            let stored = self.users.plants(&user.user_id).await.map_err(infra)?;
            let all = input
                .all_plants
                .unwrap_or_else(|| stored.iter().any(PlantId::is_all));
            let current: Vec<PlantId> = stored.into_iter().filter(|p| !p.is_all()).collect();
            let chosen = match &input.allowed_plants {
                // 勾選清單取代原有清單（PZ1）
                Some(list) => self.parse_new_plants(list, &current).await?,
                None => {
                    // 非多營運中心使用者只能有一個營運中心（p_zx.4gl:621-628）
                    if !attrs.multi_plant && user.attrs.multi_plant && current.len() > 1 {
                        return Err(DomainError::UserSinglePlant.into());
                    }
                    // 預設營運中心不在清單時加入（p_zx.4gl:1027-1060）
                    let keep =
                        attrs.default_plant == user.attrs.default_plant && !current.is_empty();
                    plants_after_default_change(
                        &current,
                        &attrs.default_plant,
                        keep || attrs.multi_plant,
                    )
                }
            };
            let plants = chosen_plants(&attrs.default_plant, attrs.multi_plant, &chosen, all)?;
            let old_class = user.attrs.permission_class.clone();
            let new_class = attrs.permission_class.clone();
            let before = user.clone();
            user.update(attrs, &actor.user_id, today());
            self.check_keeps_admin(&before, Some(&user)).await?;
            self.users.update(&user, &plants).await.map_err(infra)?;
            user.stamp.version += 1;
            if let Some(p) = password {
                self.set_password(&user.user_id, p).await?;
            }
            if old_class != new_class {
                move_special_class(
                    self.special.as_ref(),
                    actor,
                    &user.user_id,
                    &old_class,
                    &new_class,
                )
                .await?;
            }
            Ok(to_dto(&user, &plants))
        })
        .await
    }

    /// 查看單筆：被資料權限過濾掉的使用者也不能直接查看（AQ6）
    pub async fn get(&self, actor: &Actor, user_id: &str) -> Result<UserDto, AppError> {
        let grant = self.authorize(actor, "query").await?;
        let user = self.load(user_id).await?;
        grant.record(&user.created_by, &user.created_dept)?;
        let plants = self.users.plants(&user.user_id).await.map_err(infra)?;
        Ok(to_dto(&user, &plants))
    }

    /// QBE 查詢（p_zx_curs，p_zx.4gl:115-207）；欄位見 query::user（UQ1–UQ3）；
    /// 依資料權限只查得到部分使用者（p_zx.4gl:194-204，UQ4）
    pub async fn query(&self, actor: &Actor, req: QueryRequest) -> Result<Page<UserDto>, AppError> {
        let scope = self.authorize(actor, "query").await?.scope();
        let filters = query::user::filters(&req.conditions)?;
        let p = query::paging(&req);
        let (rows, total) = self
            .users
            .query(&filters, &scope, p.offset, u64::from(p.page_size))
            .await
            .map_err(infra)?;
        let mut items = Vec::with_capacity(rows.len());
        for u in &rows {
            let plants = self.users.plants(&u.user_id).await.map_err(infra)?;
            items.push(to_dto(u, &plants));
        }
        Ok(Page {
            items,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 全部清單：開窗、欄位檢查用的參考資料，只要求登入（AQ4）
    pub async fn list(&self) -> Result<Vec<UserDto>, AppError> {
        let users = self.users.list().await.map_err(infra)?;
        let mut out = Vec::with_capacity(users.len());
        for u in &users {
            let plants = self.users.plants(&u.user_id).await.map_err(infra)?;
            out.push(to_dto(u, &plants));
        }
        Ok(out)
    }

    /// 刪除（p_zx_r）：連同可使用營運中心（U9：維持 TipTop 直接刪除）
    pub async fn delete(
        &self,
        actor: &Actor,
        user_id: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            let grant = self.authorize(actor, "delete").await?;
            let user = self.load(user_id).await?;
            user.stamp.check(version)?;
            grant.record(&user.created_by, &user.created_dept)?;
            // 已確認不可刪除，須先取消確認（ADR 0013 SA9-2）
            user.confirmation.check_deletable()?;
            self.check_keeps_admin(&user, None).await?;
            self.users.delete(&user.user_id).await.map_err(infra)
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        user_id: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<UserDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            let grant = self.authorize(actor, action).await?;
            let mut user = self.load(user_id).await?;
            grant.record(&user.created_by, &user.created_dept)?;
            user.stamp.check(version)?;
            user.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            let plants = self.users.plants(&user.user_id).await.map_err(infra)?;
            self.users.update(&user, &plants).await.map_err(infra)?;
            let saved = self.load(user_id).await?;
            Ok(to_dto(&saved, &plants))
        })
        .await
    }

    /// 列印 output、匯出 Excel exporttoexcel（p_zx.4gl:267；ADR 0013 SA3）：依查詢條件與資料權限、使用者代號排序
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<ttrs_shared::QbeCondition>,
        format: crate::report::ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<crate::report::ReportData, AppError> {
        let scope = self.authorize(actor, format.action()).await?.scope();
        let filters = query::user::filters(&conditions)?;
        let (rows, total) = self
            .users
            .query(&filters, &scope, 0, self.reports.fetch_limit())
            .await
            .map_err(infra)?;
        self.reports.check(total)?;
        let t = |id: &str| crate::report::text(lang, id);
        let lines = rows
            .iter()
            .map(|u| {
                let a = &u.attrs;
                vec![
                    u.user_id.as_str().to_owned(),
                    a.name.clone().unwrap_or_default(),
                    a.dept_code.clone().unwrap_or_default(),
                    a.permission_class.clone(),
                    a.default_plant.as_str().to_owned(),
                    t(if u.active {
                        "report-active"
                    } else {
                        "report-inactive"
                    }),
                ]
            })
            .collect();
        self.reports
            .build(actor, None, &REPORT, lines, &conditions, format, lang)
            .await
    }

    /// 測試用：調整同步產生報表的筆數上限
    pub fn with_report_limit(mut self, limit: u64) -> Self {
        self.reports.limit = limit;
        self
    }

    /// 複製（p_zx_copy）：可使用營運中心只帶預設營運中心（p_zx.4gl:1360-1379）
    pub async fn copy(
        &self,
        actor: &Actor,
        user_id: &str,
        new_id: &str,
    ) -> Result<UserDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load(user_id).await?;
            let new_id = UserId::parse(new_id)?;
            let user = source.copy_as(new_id, &actor.user_id, &actor.dept, today());
            self.check_dept(&user.attrs).await?;
            // 連同可使用營運中心（含全部營運中心）一起複製（PZ6；TipTop p_zx 只帶預設營運中心）
            let plants = self.users.plants(&source.user_id).await.map_err(infra)?;
            match self.users.insert(&user, &plants).await {
                Ok(()) => Ok(to_dto(&user, &plants)),
                Err(RepoError::Conflict) => Err(DomainError::UserAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 停用（p_zx_x，有效 → 無效）；功能 invalid，不檢查資料所有者（自訂動作對應未移植，AQ7）
    pub async fn deactivate(
        &self,
        actor: &Actor,
        user_id: &str,
        version: Option<i32>,
    ) -> Result<UserDto, AppError> {
        self.set_active(actor, user_id, false, version).await
    }

    /// 啟用（p_zx_x，無效 → 有效）
    pub async fn activate(
        &self,
        actor: &Actor,
        user_id: &str,
        version: Option<i32>,
    ) -> Result<UserDto, AppError> {
        self.set_active(actor, user_id, true, version).await
    }

    async fn set_active(
        &self,
        actor: &Actor,
        user_id: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<UserDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "invalid", async {
            self.authorize(actor, "invalid").await?;
            let mut user = self.load(user_id).await?;
            user.stamp.version = version.unwrap_or(user.stamp.version);
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            let before = user.clone();
            if active {
                user.activate(&actor.user_id, today());
            } else {
                user.deactivate(&actor.user_id, today());
            }
            self.check_keeps_admin(&before, Some(&user)).await?;
            let plants = self.users.plants(&user.user_id).await.map_err(infra)?;
            self.users.update(&user, &plants).await.map_err(infra)?;
            user.stamp.version += 1;
            Ok(to_dto(&user, &plants))
        })
        .await
    }

    /// 有效、未過期，且有效權限含使用者維護的 modify（R14）
    async fn is_admin(&self, user: &User) -> Result<bool, AppError> {
        if check_account(user, today()).is_err() {
            return Ok(false);
        }
        let as_actor = Actor {
            user_id: user.user_id.as_str().to_owned(),
            dept: user.attrs.dept_code.clone().unwrap_or_default(),
            permission_class: user.attrs.permission_class.clone(),
            kind: ActorKind::Person,
        };
        Ok(self
            .admins
            .effective(&as_actor, PROGRAM)
            .await?
            .is_some_and(|p| p.allows("modify")))
    }

    /// 停用、刪除或改權限後，系統內不可沒有使用者維護管理員（R14）；`after` 為 None 表示刪除
    async fn check_keeps_admin(&self, before: &User, after: Option<&User>) -> Result<(), AppError> {
        if !self.is_admin(before).await? {
            return Ok(());
        }
        let after_admin = match after {
            Some(u) => self.is_admin(u).await?,
            None => false,
        };
        if after_admin {
            return Ok(());
        }
        let mut others = 0;
        for u in self.users.list().await.map_err(infra)? {
            if u.user_id != before.user_id && self.is_admin(&u).await? {
                others += 1;
                break;
            }
        }
        check_keeps_admin(true, after_admin, others).map_err(Into::into)
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }

    /// 部門代號須為預設營運中心存在且有效的部門（p_zx.4gl:566-580 只警告，Key User 改為擋下，UD1–UD3）
    async fn check_dept(&self, attrs: &UserAttrs) -> Result<(), AppError> {
        let Some(raw) = &attrs.dept_code else {
            return Ok(());
        };
        let dept = match DeptCode::parse(raw) {
            Ok(code) => self
                .depts
                .get(&attrs.default_plant, &code)
                .await
                .map_err(infra)?,
            Err(_) => None,
        };
        match dept {
            None => Err(DomainError::DepartmentNotFound.into()),
            Some(d) if !d.active => Err(DomainError::DepartmentInactive.into()),
            Some(_) => Ok(()),
        }
    }

    /// 勾選的營運中心：新加入的須存在（p_zxy.4gl:643-651）且有效（PZ4）；原本就在清單中的停用後仍保留
    async fn parse_new_plants(
        &self,
        list: &[String],
        current: &[PlantId],
    ) -> Result<Vec<PlantId>, AppError> {
        let mut out = Vec::with_capacity(list.len());
        for raw in list {
            let id = PlantId::parse(raw).map_err(|_| DomainError::PlantNotFound)?;
            if !current.contains(&id) {
                match self.plants.get(&id).await.map_err(infra)? {
                    None => return Err(DomainError::PlantNotFound.into()),
                    Some(p) if !p.active => {
                        return Err(DomainError::PlantInactive(id.as_str().to_owned()).into());
                    }
                    Some(_) => {}
                }
            }
            out.push(id);
        }
        Ok(out)
    }

    async fn load(&self, user_id: &str) -> Result<User, AppError> {
        let id = UserId::parse(user_id)?;
        self.users
            .get(&id)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::UserNotFound.into())
    }

    /// 檢核輸入並組出欄位：權限類別存在且有效（p_zx.4gl:582-602）、選單空白時帶入權限類別的預設選單（:584-588）、
    /// 選單為 MENU 程式（:611-619）、預設營運中心存在（:630-638）、公司帳號未綁定其他使用者（U2）
    async fn attrs(&self, i: &UserInput, me: Option<&UserId>) -> Result<UserAttrs, AppError> {
        let plant = PlantId::parse(&i.default_plant)?;
        let mut attrs = UserAttrs::new(
            i.name.clone(),
            i.dept_code.clone(),
            &i.permission_class,
            &i.menu,
            i.multi_plant,
            plant,
            i.email.clone(),
            i.idp_username.clone(),
            i.lang.as_deref().unwrap_or(DEFAULT_LANG),
            i.valid_until,
        )?;
        let class = match ClassCode::parse(&attrs.permission_class) {
            Ok(code) => self.classes.get(&code).await.map_err(infra)?,
            Err(_) => None,
        }
        .ok_or(DomainError::PermissionClassNotFound)?;
        if !class.active {
            return Err(DomainError::PermissionClassInactive.into());
        }
        if attrs.menu.is_empty()
            && let Some(menu) = class.attrs.default_menu
        {
            attrs.menu = menu;
        }
        let menu_ok = match ProgramNo::parse(&attrs.menu) {
            Ok(no) => self
                .programs
                .get(&no)
                .await
                .map_err(infra)?
                .is_some_and(|p| p.module.is_menu()),
            Err(_) => false,
        };
        if !menu_ok {
            return Err(DomainError::UserMenuInvalid(attrs.menu.clone()).into());
        }
        if !self
            .master
            .plant_active(&attrs.default_plant)
            .await
            .map_err(infra)?
        {
            return Err(DomainError::PlantNotFound.into());
        }
        if let Some(account) = &attrs.idp_username {
            let owner = self
                .users
                .find_by_idp_username(account)
                .await
                .map_err(infra)?;
            if owner.is_some_and(|o| Some(&o) != me) {
                return Err(DomainError::IdpAccountInUse(account.clone()).into());
            }
        }
        Ok(attrs)
    }
}

fn infra(e: RepoError) -> AppError {
    match e {
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        RepoError::Stale => ttrs_domain::DomainError::RecordChanged.into(),
        e => AppError::Infra(e.to_string()),
    }
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// 輸入的 Web 密碼（只寫不讀）；空白表示不變更。長度上限同變更密碼（W6）
fn web_password(input: &UserInput) -> Result<Option<&str>, AppError> {
    match input.web_password.as_deref().filter(|p| !p.is_empty()) {
        Some(p) if p.chars().count() > MAX_LEN => {
            Err(DomainError::PasswordTooLong { max: MAX_LEN }.into())
        }
        other => Ok(other),
    }
}

fn to_dto(u: &User, plants: &[PlantId]) -> UserDto {
    let a = &u.attrs;
    UserDto {
        user_id: u.user_id.as_str().to_owned(),
        name: a.name.clone(),
        dept_code: a.dept_code.clone(),
        permission_class: a.permission_class.clone(),
        menu: a.menu.clone(),
        multi_plant: a.multi_plant,
        default_plant: a.default_plant.as_str().to_owned(),
        email: a.email.clone(),
        idp_username: a.idp_username.clone(),
        idp_bound: u.idp_subject.is_some(),
        lang: a.lang.as_str().to_owned(),
        valid_until: a.valid_until,
        active: u.active,
        confirmed: u.confirmation.confirmed,
        confirmed_by: u.confirmation.by.clone(),
        confirmed_at: u.confirmation.at,
        allowed_plants: plants
            .iter()
            .filter(|p| !p.is_all())
            .map(|p| p.as_str().to_owned())
            .collect(),
        all_plants: plants.iter().any(PlantId::is_all),
        created_by: u.created_by.clone(),
        created_dept: u.created_dept.clone(),
        created_on: u.created_on,
        updated_by: u.updated_by.clone(),
        updated_on: u.updated_on,
        version: u.stamp.version,
        created_at: u.stamp.created_at,
        updated_at: u.stamp.updated_at,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_max_len_matches_domain() {
        use ttrs_domain::user::max_len as d;
        use ttrs_shared::user_max_len as s;
        assert_eq!(d::USER_ID, s::USER_ID);
        assert_eq!(d::NAME, s::NAME);
        assert_eq!(d::DEPT, s::DEPT);
        assert_eq!(d::EMAIL, s::EMAIL);
        assert_eq!(d::IDP_USERNAME, s::IDP_USERNAME);
    }
}
