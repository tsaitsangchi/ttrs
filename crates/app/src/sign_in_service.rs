//! Sign In 登入與切換營運中心 use case（TipTop: weblogin、aoos901）
//! 規格：specs/features/azz/sign_in.feature（SQ1–SQ10）；登入方式：docs/adr/0006 §1、§2
//!
//! 每個已註冊的使用者都能登入與切換營運中心，不檢查程式執行權限（SQ9）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    plant::{Plant, PlantId},
    sign_in::{check_account, check_plant, check_switch, switchable_plants},
    user::{User, UserId},
};
use ttrs_shared::{PlantOptionDto, SessionDto};

use crate::{
    AppError,
    error::RepoError,
    ports::{PlantRepository, UserRepository},
};

/// 公司 IdP 已驗證的身分（ADR 0006 §1）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInIdentity {
    /// 公司帳號識別碼（OIDC `sub`）
    pub subject: String,
    /// 公司帳號（UPN／preferred_username）
    pub username: String,
}

#[derive(Clone)]
pub struct SignInService {
    users: Arc<dyn UserRepository>,
    plants: Arc<dyn PlantRepository>,
}

impl SignInService {
    pub fn new(users: Arc<dyn UserRepository>, plants: Arc<dyn PlantRepository>) -> Self {
        Self { users, plants }
    }

    /// 登入（weblogin）：對應使用者（SQ2）→ 帳號檢查（SQ3）→ 進入預設營運中心（SQ4–SQ6）
    pub async fn sign_in(&self, identity: SignInIdentity) -> Result<SessionDto, AppError> {
        let user = self.find_user(&identity).await?;
        check_account(&user, today())?;
        self.enter(&user, &user.attrs.default_plant).await
    }

    /// 切換營運中心（aoos901）：多營運中心使用者才能切換（SQ7），切換後寫回預設營運中心（SQ8）
    pub async fn switch_plant(&self, user_id: &str, plant: &str) -> Result<SessionDto, AppError> {
        let user = self.load(user_id).await?;
        check_account(&user, today())?;
        check_switch(&user)?;
        let plant = PlantId::parse(plant)?;
        let session = self.enter(&user, &plant).await?;
        self.users
            .set_default_plant(&user.user_id, &plant)
            .await
            .map_err(infra)?;
        Ok(session)
    }

    /// 登入中的連線定期重新檢查（SQ3，ADR 0006 §2）：停用、過期或預設營運中心不可用即失效
    pub async fn revalidate(&self, user_id: &str) -> Result<SessionDto, AppError> {
        let user = self.load(user_id).await?;
        check_account(&user, today())?;
        self.enter(&user, &user.attrs.default_plant).await
    }

    async fn find_user(&self, identity: &SignInIdentity) -> Result<User, AppError> {
        if let Some(id) = self
            .users
            .find_by_idp_subject(&identity.subject)
            .await
            .map_err(infra)?
        {
            return self.get(&id).await;
        }
        let id = self
            .users
            .find_by_idp_username(&identity.username)
            .await
            .map_err(infra)?
            .ok_or(DomainError::UserNotRegistered)?;
        let user = self.get(&id).await?;
        // 公司帳號已綁定別的識別碼：不是同一個人，不可登入
        if user.idp_subject.is_some() {
            return Err(DomainError::UserNotRegistered.into());
        }
        self.users
            .bind_idp_subject(&id, &identity.subject)
            .await
            .map_err(infra)?;
        Ok(user)
    }

    /// 連線的使用者已不在使用者資料中，視同未註冊
    async fn load(&self, user_id: &str) -> Result<User, AppError> {
        let id = UserId::parse(user_id)?;
        self.users
            .get(&id)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::UserNotRegistered.into())
    }

    async fn get(&self, id: &UserId) -> Result<User, AppError> {
        self.users
            .get(id)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::UserNotFound.into())
    }

    /// 檢查並進入營運中心，回傳連線資訊
    async fn enter(&self, user: &User, target: &PlantId) -> Result<SessionDto, AppError> {
        let allowed = self.users.plants(&user.user_id).await.map_err(infra)?;
        let all = self.plants.list().await.map_err(infra)?;
        let plant = all.iter().find(|p| &p.id == target);
        check_plant(&allowed, target, plant)?;
        let a = &user.attrs;
        Ok(SessionDto {
            user_id: user.user_id.as_str().to_owned(),
            name: a.name.clone(),
            dept_code: a.dept_code.clone(),
            permission_class: a.permission_class.clone(),
            lang: a.lang.as_str().to_owned(),
            multi_plant: a.multi_plant,
            plant: target.as_str().to_owned(),
            plant_name: plant.map(plant_name).unwrap_or_default(),
            plants: switchable_plants(&allowed, &all)
                .into_iter()
                .map(|p| PlantOptionDto {
                    plant_id: p.id.as_str().to_owned(),
                    name: plant_name(p),
                })
                .collect(),
        })
    }
}

fn plant_name(p: &Plant) -> String {
    p.attrs.name.clone()
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}

/// 系統日期（TipTop: TODAY）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}
