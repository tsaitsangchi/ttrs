//! User Maintenance 使用者維護 use case（TipTop: p_zx）
//! 規格：specs/features/azz/user_maintenance.feature（U1–U9）；登入與權限：docs/adr/0006

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    lang::DEFAULT_LANG,
    permission_class::ClassCode,
    plant::PlantId,
    program::ProgramNo,
    user::{User, UserAttrs, UserId, plants_after_default_change},
};
use ttrs_shared::{UserDto, UserInput};

use crate::{
    Actor, AppError,
    error::RepoError,
    ports::{MasterDataRepository, PermissionClassRepository, ProgramRepository, UserRepository},
};

#[derive(Clone)]
pub struct UserService {
    users: Arc<dyn UserRepository>,
    programs: Arc<dyn ProgramRepository>,
    master: Arc<dyn MasterDataRepository>,
    classes: Arc<dyn PermissionClassRepository>,
}

impl UserService {
    pub fn new(
        users: Arc<dyn UserRepository>,
        programs: Arc<dyn ProgramRepository>,
        master: Arc<dyn MasterDataRepository>,
        classes: Arc<dyn PermissionClassRepository>,
    ) -> Self {
        Self {
            users,
            programs,
            master,
            classes,
        }
    }

    /// 新增（p_zx_a）：預設營運中心自動成為可使用營運中心（p_zx.4gl:490-501）
    pub async fn create(&self, actor: &Actor, input: UserInput) -> Result<UserDto, AppError> {
        let id = UserId::parse(&input.user_id)?;
        let attrs = self.attrs(&input, None).await?;
        let user = User::new(id, attrs, &actor.user_id, &actor.dept, today());
        let plants = vec![user.attrs.default_plant.clone()];
        match self.users.insert(&user, &plants).await {
            Ok(()) => Ok(to_dto(&user, &plants)),
            Err(RepoError::Conflict) => Err(DomainError::UserAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    /// 修改（p_zx_u）：代號不可變（U1）；依預設營運中心與是否多營運中心同步可使用營運中心
    pub async fn update(
        &self,
        actor: &Actor,
        user_id: &str,
        input: UserInput,
    ) -> Result<UserDto, AppError> {
        let mut user = self.load(user_id).await?;
        if !input.user_id.trim().is_empty() && input.user_id.trim() != user.user_id.as_str() {
            return Err(DomainError::UserIdImmutable.into());
        }
        let attrs = self.attrs(&input, Some(&user.user_id)).await?;
        let current = self.users.plants(&user.user_id).await.map_err(infra)?;
        // 非多營運中心使用者只能有一個營運中心（p_zx.4gl:621-628）
        if !attrs.multi_plant && user.attrs.multi_plant && current.len() > 1 {
            return Err(DomainError::UserSinglePlant.into());
        }
        let plants = if attrs.default_plant == user.attrs.default_plant && !current.is_empty() {
            current
        } else {
            plants_after_default_change(&current, &attrs.default_plant, attrs.multi_plant)
        };
        user.update(attrs, &actor.user_id, today());
        self.users.update(&user, &plants).await.map_err(infra)?;
        Ok(to_dto(&user, &plants))
    }

    pub async fn get(&self, user_id: &str) -> Result<UserDto, AppError> {
        let user = self.load(user_id).await?;
        let plants = self.users.plants(&user.user_id).await.map_err(infra)?;
        Ok(to_dto(&user, &plants))
    }

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
    pub async fn delete(&self, user_id: &str) -> Result<(), AppError> {
        let user = self.load(user_id).await?;
        self.users.delete(&user.user_id).await.map_err(infra)
    }

    /// 複製（p_zx_copy）：可使用營運中心只帶預設營運中心（p_zx.4gl:1360-1379）
    pub async fn copy(
        &self,
        actor: &Actor,
        user_id: &str,
        new_id: &str,
    ) -> Result<UserDto, AppError> {
        let source = self.load(user_id).await?;
        let new_id = UserId::parse(new_id)?;
        let user = source.copy_as(new_id, &actor.user_id, &actor.dept, today());
        let plants = vec![user.attrs.default_plant.clone()];
        match self.users.insert(&user, &plants).await {
            Ok(()) => Ok(to_dto(&user, &plants)),
            Err(RepoError::Conflict) => Err(DomainError::UserAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    /// 停用（p_zx_x，有效 → 無效）
    pub async fn deactivate(&self, actor: &Actor, user_id: &str) -> Result<UserDto, AppError> {
        self.set_active(actor, user_id, false).await
    }

    /// 啟用（p_zx_x，無效 → 有效）
    pub async fn activate(&self, actor: &Actor, user_id: &str) -> Result<UserDto, AppError> {
        self.set_active(actor, user_id, true).await
    }

    async fn set_active(
        &self,
        actor: &Actor,
        user_id: &str,
        active: bool,
    ) -> Result<UserDto, AppError> {
        let mut user = self.load(user_id).await?;
        if active {
            user.activate(&actor.user_id, today());
        } else {
            user.deactivate(&actor.user_id, today());
        }
        let plants = self.users.plants(&user.user_id).await.map_err(infra)?;
        self.users.update(&user, &plants).await.map_err(infra)?;
        Ok(to_dto(&user, &plants))
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
    AppError::Infra(e.to_string())
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
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
        allowed_plants: plants.iter().map(|p| p.as_str().to_owned()).collect(),
        created_by: u.created_by.clone(),
        created_dept: u.created_dept.clone(),
        created_on: u.created_on,
        updated_by: u.updated_by.clone(),
        updated_on: u.updated_on,
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
