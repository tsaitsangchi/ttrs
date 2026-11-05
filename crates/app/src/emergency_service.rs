//! 伺服器端緊急重設（ttrs-admin；TTRS 新增，TipTop 緊急時由 DBA 直接改 zx_file；ADR 0010 §B）
//! 規格：specs/features/azz/emergency_password_reset.feature
//!
//! 只給伺服器上的指令程式使用，沒有 HTTP API（R11）；執行者是作業系統使用者，不經 Authorizer。
//! 臨時密碼只回傳給呼叫端顯示一次，不寫入稽核紀錄或程式紀錄（R12）。

use std::sync::Arc;

use ttrs_domain::{
    DomainError,
    password::Credential,
    sign_in::check_account,
    user::{User, UserId},
};

use crate::{
    AppError,
    error::RepoError,
    password_hash,
    ports::{AuditEntry, AuditLog, CredentialRepository, UserRepository},
};

/// 稽核紀錄的程式代號
pub const PROGRAM: &str = "ttrs-admin";

#[derive(Clone)]
pub struct EmergencyService {
    users: Arc<dyn UserRepository>,
    credentials: Arc<dyn CredentialRepository>,
    audit: Arc<dyn AuditLog>,
}

impl EmergencyService {
    pub fn new(
        users: Arc<dyn UserRepository>,
        credentials: Arc<dyn CredentialRepository>,
        audit: Arc<dyn AuditLog>,
    ) -> Self {
        Self {
            users,
            credentials,
            audit,
        }
    }

    /// 重設密碼：隨機臨時密碼、須改密碼（p_zx.4gl:648）、試誤次數歸零、結束已登入的連線；
    /// 停用或過期的帳號須 `reactivate` 才同時恢復有效（R13）。回傳臨時密碼
    pub async fn reset_password(
        &self,
        user_id: &str,
        operator: &str,
        reactivate: bool,
    ) -> Result<String, AppError> {
        let mut user = self.load(user_id).await?;
        if reactivate && check_account(&user, today()).is_err() {
            user.activate(&actor(operator), today());
            let plants = self.users.plants(&user.user_id).await.map_err(infra)?;
            self.users.update(&user, &plants).await.map_err(infra)?;
        }
        check_account(&user, today())?;
        let temp = temp_password(user.user_id.as_str())?;
        let cred = Credential::reset(password_hash::hash(&temp)?, today());
        self.credentials
            .save_credential(&user.user_id, &cred)
            .await
            .map_err(infra)?;
        self.credentials
            .end_sessions(&user.user_id)
            .await
            .map_err(infra)?;
        self.record(&user, operator, "reset-password").await?;
        Ok(temp)
    }

    /// 只解除鎖定（同 p_zx user_unlock，p_zx.4gl:367-400）
    pub async fn unlock(&self, user_id: &str, operator: &str) -> Result<(), AppError> {
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
        self.record(&user, operator, "unlock").await
    }

    async fn load(&self, user_id: &str) -> Result<User, AppError> {
        let id = UserId::parse(user_id)?;
        self.users
            .get(&id)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::UserNotFound.into())
    }

    async fn record(&self, user: &User, operator: &str, action: &str) -> Result<(), AppError> {
        self.audit
            .record(&AuditEntry {
                program: PROGRAM.to_owned(),
                user_id: actor(operator),
                key: user.user_id.as_str().to_owned(),
                action: action.to_owned(),
            })
            .await
            .map_err(infra)
    }
}

/// 稽核紀錄的執行者：「os:作業系統使用者」，與 TTRS 使用者代號區分
fn actor(operator: &str) -> String {
    format!("os:{}", operator.trim())
}

/// 臨時密碼：14 個英數字＋數字＋符號，符合任何複雜度設定；不含使用者代號
fn temp_password(user_id: &str) -> Result<String, AppError> {
    // 去掉容易看錯的 0/O、1/l/I
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    loop {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|e| AppError::Infra(format!("亂數：{e}")))?;
        let mut s: String = bytes[..14]
            .iter()
            .map(|b| CHARS[usize::from(*b) % CHARS.len()] as char)
            .collect();
        s.push(char::from(b'2' + bytes[14] % 8));
        s.push(['#', '%', '+', '-', '@', '!'][usize::from(bytes[15]) % 6]);
        if !s.to_lowercase().contains(&user_id.to_lowercase()) {
            return Ok(s);
        }
    }
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
    use ttrs_domain::password::{AccountPolicy, Complexity, check_strength};

    #[test]
    fn temp_password_meets_strictest_policy() {
        let policy = AccountPolicy {
            complexity: Complexity::Symbol,
            min_length: 16,
            ..AccountPolicy::default()
        };
        for _ in 0..50 {
            let p = temp_password("ab").unwrap();
            assert_eq!(p.chars().count(), 16);
            assert!(check_strength(&policy, "ab", &p).is_ok(), "{p}");
        }
    }
}
