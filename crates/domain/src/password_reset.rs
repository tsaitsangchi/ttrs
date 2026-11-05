//! 忘記密碼與緊急重設（TTRS 新增，TipTop 無對應程式；zx_file zx09 Email、zx10 Web 密碼）
//! 規格：specs/features/azz/password_reset.feature、emergency_password_reset.feature；決策：docs/adr/0010（R1–R15）
//!
//! 權杖本身與臨時密碼由 app 層以系統亂數產生；這裡只放期限、次數與「保留管理員」規則。

use chrono::{DateTime, Duration, Utc};

use crate::DomainError;

/// 重設連結有效期限（R3）
pub const TOKEN_TTL: Duration = Duration::minutes(30);
/// 申請次數上限：同一使用者在此期間內（R7）
pub const REQUEST_WINDOW: Duration = Duration::hours(1);
pub const MAX_REQUESTS: u64 = 3;

/// 重設權杖（password_reset_token）：只存雜湊
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetToken {
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    /// 已使用（重設成功）或因重新申請而失效的時間
    pub used_at: Option<DateTime<Utc>>,
}

impl ResetToken {
    pub fn issue(user_id: &str, now: DateTime<Utc>) -> Self {
        Self {
            user_id: user_id.to_owned(),
            created_at: now,
            expires_at: now + TOKEN_TTL,
            used_at: None,
        }
    }

    /// 一次性、未過期（R3）
    pub fn check_usable(&self, now: DateTime<Utc>) -> Result<(), DomainError> {
        if self.used_at.is_some() || now >= self.expires_at {
            return Err(DomainError::PasswordResetTokenInvalid);
        }
        Ok(())
    }
}

/// 期間內已申請次數未達上限才寄信（R7）
pub fn may_request(recent_requests: u64) -> bool {
    recent_requests < MAX_REQUESTS
}

/// 最後一位使用者維護管理員不可移除（R14）：`target_after` 為操作後目標使用者是否仍是管理員，
/// `others` 為目標以外的管理員人數；目標原本就不是管理員時不檢查
pub fn check_keeps_admin(
    target_before: bool,
    target_after: bool,
    others: usize,
) -> Result<(), DomainError> {
    if target_before && !target_after && others == 0 {
        return Err(DomainError::LastUserAdmin);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(min: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000, 0).unwrap() + Duration::minutes(min)
    }

    #[test]
    fn token_expires_after_30_minutes_and_is_single_use() {
        let mut t = ResetToken::issue("u001", at(0));
        assert!(t.check_usable(at(29)).is_ok());
        assert_eq!(
            t.check_usable(at(30)),
            Err(DomainError::PasswordResetTokenInvalid)
        );
        t.used_at = Some(at(1));
        assert!(t.check_usable(at(2)).is_err());
    }

    #[test]
    fn request_limit() {
        assert!(may_request(2));
        assert!(!may_request(3));
    }

    #[test]
    fn keeps_last_admin() {
        assert_eq!(
            check_keeps_admin(true, false, 0),
            Err(DomainError::LastUserAdmin)
        );
        assert!(check_keeps_admin(true, false, 1).is_ok());
        assert!(check_keeps_admin(false, false, 0).is_ok());
        assert!(check_keeps_admin(true, true, 0).is_ok());
    }
}
