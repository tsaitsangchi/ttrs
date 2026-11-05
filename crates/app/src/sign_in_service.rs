//! Sign In 登入與切換營運中心 use case（TipTop: weblogin、aoos901）
//! 規格：specs/features/azz/sign_in.feature（SQ1–SQ10）；登入方式：docs/adr/0009
//!
//! 以使用者代號＋Web 密碼登入（ADR 0009，specs/features/azz/web_password.feature）。
//! 每個已註冊的使用者都能登入、變更密碼與切換營運中心，不檢查程式執行權限（SQ9）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    password::{Credential, PasswordAge, check_new_password, must_change, password_age},
    plant::{Plant, PlantId},
    sign_in::{check_account, check_plant, check_switch, switchable_plants},
    user::{User, UserId},
};
use ttrs_shared::{PlantOptionDto, SessionDto};

use crate::{
    AppError,
    error::RepoError,
    password_hash,
    ports::{AccountPolicyRepository, CredentialRepository, PlantRepository, UserRepository},
};

#[derive(Clone)]
pub struct SignInService {
    users: Arc<dyn UserRepository>,
    plants: Arc<dyn PlantRepository>,
    credentials: Arc<dyn CredentialRepository>,
    policy: Arc<dyn AccountPolicyRepository>,
}

impl SignInService {
    pub fn new(
        users: Arc<dyn UserRepository>,
        plants: Arc<dyn PlantRepository>,
        credentials: Arc<dyn CredentialRepository>,
        policy: Arc<dyn AccountPolicyRepository>,
    ) -> Self {
        Self {
            users,
            plants,
            credentials,
            policy,
        }
    }

    /// 登入（weblogin，ADR 0009）：帳號（存在、有效、未過期、未鎖定）→ 密碼 → 須改密碼 → 營運中心（SQ4–SQ6）
    pub async fn sign_in(&self, user_id: &str, password: &str) -> Result<SessionDto, AppError> {
        // 帳號不存在與密碼錯誤同一訊息（W2，lib-064）
        let Ok(id) = UserId::parse(user_id) else {
            return Err(DomainError::SignInFailed.into());
        };
        let user = self
            .users
            .get(&id)
            .await
            .map_err(infra)?
            .ok_or(DomainError::SignInFailed)?;
        check_account(&user, today())?;
        let policy = self.policy.account_policy().await.map_err(infra)?;
        let mut cred = self.credential(&id).await?;
        if cred.locked(&policy) {
            return Err(DomainError::UserLocked.into());
        }
        if !password_matches(&id, &cred, password) {
            cred.record_failure(&policy);
            self.save(&id, &cred).await?;
            return Err(DomainError::SignInFailed.into());
        }
        cred.record_success(&policy);
        self.save(&id, &cred).await?;
        if must_change(&policy, id.as_str(), &cred, password, today()) {
            return Err(DomainError::PasswordChangeRequired.into());
        }
        let mut session = self.enter(&user, &user.attrs.default_plant).await?;
        match password_age(&policy, &cred, today()) {
            PasswordAge::ExpiringInDays(d) => session.password_expires_in_days = Some(d),
            PasswordAge::ExpiringInUses(n) => session.password_uses_left = Some(n),
            PasswordAge::Valid | PasswordAge::Expired => {}
        }
        Ok(session)
    }

    /// 變更密碼（webpasswd）：以舊密碼驗證身分，登入前（須改密碼時）與登入後都可使用
    pub async fn change_password(
        &self,
        user_id: &str,
        old: &str,
        new: &str,
        confirm: &str,
    ) -> Result<(), AppError> {
        let Ok(id) = UserId::parse(user_id) else {
            return Err(DomainError::SignInFailed.into());
        };
        let user = self
            .users
            .get(&id)
            .await
            .map_err(infra)?
            .ok_or(DomainError::SignInFailed)?;
        check_account(&user, today())?;
        let policy = self.policy.account_policy().await.map_err(infra)?;
        let mut cred = self.credential(&id).await?;
        if cred.locked(&policy) {
            return Err(DomainError::UserLocked.into());
        }
        let old_correct = password_matches(&id, &cred, old);
        if !old_correct {
            // 不需登入即可呼叫：舊密碼錯誤同樣計入試誤次數，避免以此猜密碼
            cred.record_failure(&policy);
            self.save(&id, &cred).await?;
        }
        check_new_password(&policy, id.as_str(), old_correct, old, new, confirm)?;
        let changed = Credential::changed(password_hash::hash(new)?, today());
        self.save(&id, &changed).await
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

    /// 連線是否仍有效：重設密碼（忘記密碼、ttrs-admin）後舊連線失效（ADR 0010 R6）
    pub async fn check_session(&self, user_id: &str, version: u32) -> Result<(), AppError> {
        let id = UserId::parse(user_id).map_err(|_| DomainError::NotSignedIn)?;
        let current = self.credentials.session_version(&id).await.map_err(infra)?;
        if current != version {
            return Err(DomainError::NotSignedIn.into());
        }
        Ok(())
    }

    /// 登入中的連線定期重新檢查（SQ3）：停用、過期、鎖定或預設營運中心不可用即失效
    pub async fn revalidate(&self, user_id: &str) -> Result<SessionDto, AppError> {
        let user = self.load(user_id).await?;
        check_account(&user, today())?;
        let policy = self.policy.account_policy().await.map_err(infra)?;
        if self.credential(&user.user_id).await?.locked(&policy) {
            return Err(DomainError::UserLocked.into());
        }
        self.enter(&user, &user.attrs.default_plant).await
    }

    async fn credential(&self, id: &UserId) -> Result<Credential, AppError> {
        Ok(self
            .credentials
            .credential(id)
            .await
            .map_err(infra)?
            .unwrap_or_default())
    }

    async fn save(&self, id: &UserId, cred: &Credential) -> Result<(), AppError> {
        self.credentials
            .save_credential(id, cred)
            .await
            .map_err(infra)
    }

    async fn load(&self, user_id: &str) -> Result<User, AppError> {
        let id = UserId::parse(user_id)?;
        self.users
            .get(&id)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::UserNotRegistered.into())
    }

    /// 檢查並進入營運中心，回傳連線資訊
    async fn enter(&self, user: &User, target: &PlantId) -> Result<SessionDto, AppError> {
        let allowed = self.users.plants(&user.user_id).await.map_err(infra)?;
        let all = self.plants.list().await.map_err(infra)?;
        let plant = all.iter().find(|p| &p.id == target);
        check_plant(&allowed, target, plant)?;
        let session_version = self
            .credentials
            .session_version(&user.user_id)
            .await
            .map_err(infra)?;
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
            password_expires_in_days: None,
            password_uses_left: None,
            plants: switchable_plants(&allowed, &all)
                .into_iter()
                .map(|p| PlantOptionDto {
                    plant_id: p.id.as_str().to_owned(),
                    name: plant_name(p),
                })
                .collect(),
            session_version,
        })
    }
}

/// 尚未設定密碼時初始密碼＝使用者代號（p_zx.4gl:561，W1）
pub(crate) fn password_matches(id: &UserId, cred: &Credential, password: &str) -> bool {
    match &cred.password_hash {
        Some(h) => password_hash::verify(h, password),
        None => password == id.as_str(),
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
