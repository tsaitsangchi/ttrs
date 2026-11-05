//! Forgot Password 忘記密碼 use case（TTRS 新增，TipTop 無對應程式；ADR 0010 §A）
//! 規格：specs/features/azz/password_reset.feature
//!
//! 登入前的流程，同 sign-in、change-password 不經 Authorizer（R10）。
//! 申請時不透露帳號是否存在（R2）：除了尚未設定寄信伺服器，一律回應成功。

use std::sync::Arc;

use chrono::Utc;
use sha2::{Digest, Sha256};
use ttrs_domain::{
    DomainError,
    password::{Credential, check_reset_password},
    password_reset::{REQUEST_WINDOW, ResetToken, may_request},
    sign_in::check_account,
    user::{User, UserId},
};

use crate::{
    AppError,
    error::RepoError,
    mail::{Mail, MailSettings, MailTemplate},
    password_hash,
    ports::{
        AccountPolicyRepository, AuditEntry, AuditLog, CredentialRepository,
        PasswordResetTokenRepository, UserRepository,
    },
    sign_in_service::password_matches,
};

/// 程式代號（稽核紀錄）
pub const PROGRAM: &str = "forgot-password";

#[derive(Clone)]
pub struct PasswordResetService {
    users: Arc<dyn UserRepository>,
    credentials: Arc<dyn CredentialRepository>,
    policy: Arc<dyn AccountPolicyRepository>,
    tokens: Arc<dyn PasswordResetTokenRepository>,
    audit: Arc<dyn AuditLog>,
    /// 寄信設定；None 時不開放（R9）
    mail: Option<MailSettings>,
}

impl PasswordResetService {
    pub fn new(
        users: Arc<dyn UserRepository>,
        credentials: Arc<dyn CredentialRepository>,
        policy: Arc<dyn AccountPolicyRepository>,
        tokens: Arc<dyn PasswordResetTokenRepository>,
        audit: Arc<dyn AuditLog>,
    ) -> Self {
        Self {
            users,
            credentials,
            policy,
            tokens,
            audit,
            mail: None,
        }
    }

    pub fn with_mail(self, mail: MailSettings) -> Self {
        Self {
            mail: Some(mail),
            ..self
        }
    }

    pub fn without_mail(self) -> Self {
        Self { mail: None, ..self }
    }

    /// 是否開放忘記密碼（登入畫面據此顯示入口）
    pub fn available(&self) -> bool {
        self.mail.is_some()
    }

    fn settings(&self) -> Result<&MailSettings, AppError> {
        self.mail
            .as_ref()
            .ok_or_else(|| DomainError::PasswordResetUnavailable.into())
    }

    /// 申請重設（R1：以使用者代號）：帳號存在、有效、未過期、有 Email，且未超過申請次數時寄出連結
    pub async fn request(&self, user_id: &str) -> Result<(), AppError> {
        let settings = self.settings()?;
        let Some((user, email)) = self.recipient(user_id).await? else {
            return Ok(());
        };
        let now = Utc::now();
        let recent = self
            .tokens
            .reset_requests_since(&user.user_id, now - REQUEST_WINDOW)
            .await
            .map_err(infra)?;
        if !may_request(recent) {
            tracing::info!(user = %user.user_id.as_str(), "password reset: too many requests");
            return Ok(());
        }
        let raw = random_token()?;
        let token = ResetToken::issue(user.user_id.as_str(), now);
        self.tokens
            .issue_reset_token(&token_hash(&raw), &token)
            .await
            .map_err(infra)?;
        let link = format!(
            "{}/reset-password?token={raw}",
            settings.public_url.trim_end_matches('/')
        );
        self.send(&user, &email, MailTemplate::PasswordReset, Some(&link))
            .await?;
        self.record(&user.user_id, "request").await
    }

    /// 以連結設定新密碼（決策 A5、A6）：成功時須改密碼取消、試誤次數歸零（R5）、結束已登入的連線（R6）、
    /// 寄出密碼已變更通知（R8）。新密碼不合格時權杖不消耗
    pub async fn reset(&self, token: &str, new: &str, confirm: &str) -> Result<(), AppError> {
        self.settings()?;
        let now = Utc::now();
        let found = self
            .tokens
            .reset_token(&token_hash(token.trim()))
            .await
            .map_err(infra)?
            .ok_or(DomainError::PasswordResetTokenInvalid)?;
        found.check_usable(now)?;
        let id =
            UserId::parse(&found.user_id).map_err(|_| DomainError::PasswordResetTokenInvalid)?;
        let user = self
            .users
            .get(&id)
            .await
            .map_err(infra)?
            .ok_or(DomainError::PasswordResetTokenInvalid)?;
        // 申請後才停用或過期的帳號，連結一併失效
        check_account(&user, today()).map_err(|_| DomainError::PasswordResetTokenInvalid)?;
        let policy = self.policy.account_policy().await.map_err(infra)?;
        let current = self
            .credentials
            .credential(&id)
            .await
            .map_err(infra)?
            .unwrap_or_default();
        let same = password_matches(&id, &current, new);
        check_reset_password(&policy, id.as_str(), same, new, confirm)?;
        let changed = Credential::changed(password_hash::hash(new)?, today());
        self.credentials
            .save_credential(&id, &changed)
            .await
            .map_err(infra)?;
        self.tokens
            .revoke_reset_tokens(&id, now)
            .await
            .map_err(infra)?;
        self.credentials.end_sessions(&id).await.map_err(infra)?;
        self.record(&id, "reset").await?;
        if let Some(email) = user.attrs.email.clone().filter(|e| !e.trim().is_empty()) {
            self.send(&user, &email, MailTemplate::PasswordChanged, None)
                .await?;
        }
        Ok(())
    }

    /// 可寄信的收件者：帳號存在、有效、未過期且有 Email（同 p_passnoty zx09 空白不寄）
    async fn recipient(&self, user_id: &str) -> Result<Option<(User, String)>, AppError> {
        let Ok(id) = UserId::parse(user_id) else {
            return Ok(None);
        };
        let Some(user) = self.users.get(&id).await.map_err(infra)? else {
            return Ok(None);
        };
        if check_account(&user, today()).is_err() {
            return Ok(None);
        }
        let email = user.attrs.email.clone().filter(|e| !e.trim().is_empty());
        Ok(email.map(|e| (user, e)))
    }

    async fn send(
        &self,
        user: &User,
        email: &str,
        template: MailTemplate,
        link: Option<&str>,
    ) -> Result<(), AppError> {
        let settings = self.settings()?;
        let name = user
            .attrs
            .name
            .clone()
            .unwrap_or_else(|| user.user_id.as_str().to_owned());
        let mut args = vec![("name", name.as_str()), ("user", user.user_id.as_str())];
        if let Some(link) = link {
            args.push(("link", link));
        }
        let mail = Mail::render(email, &user.attrs.lang, template, &args);
        settings.mailer.send(&mail).await.map_err(infra)
    }

    async fn record(&self, id: &UserId, action: &str) -> Result<(), AppError> {
        self.audit
            .record(&AuditEntry {
                program: PROGRAM.to_owned(),
                user_id: id.as_str().to_owned(),
                key: id.as_str().to_owned(),
                action: action.to_owned(),
            })
            .await
            .map_err(infra)
    }
}

/// 256 bits 系統亂數，十六進位
fn random_token() -> Result<String, AppError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| AppError::Infra(format!("亂數：{e}")))?;
    Ok(hex(&bytes))
}

/// 資料庫只存權杖的 SHA-256（決策 A4）
fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_hash_is_not_the_token() {
        let t = random_token().unwrap();
        assert_eq!(t.len(), 64);
        let h = token_hash(&t);
        assert_eq!(h.len(), 64);
        assert_ne!(h, t);
        assert_eq!(h, token_hash(&t));
    }
}
