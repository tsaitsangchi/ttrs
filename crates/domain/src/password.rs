//! Web 密碼與帳號稽核原則（TipTop: zx10、zx16、zx17、zx19、zx20 / gbt_file；weblogin、webpasswd、p_zx）
//! 規格：specs/features/azz/web_password.feature；決策：docs/adr/0009-password-sign-in.md（W1–W12）
//!
//! 密碼只存雜湊（雜湊在 app 層計算）；這裡只放規則。

use chrono::{Duration, NaiveDate};

use crate::DomainError;

/// 密碼最長字數（W6：TipTop 20 字放寬為 64 字）
pub const MAX_LEN: usize = 64;

/// 密碼要求變更依據（gbt01）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpiryBasis {
    /// 不要求定期變更
    Never,
    /// 1：依天數（gbt02 更新週期）
    Days,
    /// 2：依登入次數（gbt03 可用次數）
    Uses,
}

/// 密碼複雜度（gbt06）：N 不限、Y 不可含帳號、M 另須含數字、S 另須含符號
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Complexity {
    None,
    NoUserId,
    Digit,
    Symbol,
}

impl Complexity {
    pub fn code(self) -> &'static str {
        match self {
            Self::None => "N",
            Self::NoUserId => "Y",
            Self::Digit => "M",
            Self::Symbol => "S",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "N" => Self::None,
            "Y" => Self::NoUserId,
            "M" => Self::Digit,
            "S" => Self::Symbol,
            _ => return None,
        })
    }
}

/// 系統帳號稽核原則（gbt_file，系統一筆）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountPolicy {
    pub expiry_basis: ExpiryBasis,
    /// gbt02 密碼更新週期（天）
    pub cycle_days: u32,
    /// gbt03 密碼可用次數
    pub use_limit: u32,
    /// gbt04 提前通知天數（依次數時為剩餘次數）
    pub notify: u32,
    /// gbt05 密碼最小長度；0 不限
    pub min_length: u32,
    pub complexity: Complexity,
    /// gbt08 更換密碼可否與原密碼相同
    pub allow_same_as_old: bool,
    /// gbt10 允許最大試誤次數；0 不鎖定
    pub max_failures: u32,
}

impl Default for AccountPolicy {
    /// 起始值（W5）：依天數 90 天、提前 7 天通知、最少 8 字、不可含帳號且須含數字、不可與舊密碼相同、試誤 5 次鎖定
    fn default() -> Self {
        Self {
            expiry_basis: ExpiryBasis::Days,
            cycle_days: 90,
            use_limit: 0,
            notify: 7,
            min_length: 8,
            complexity: Complexity::Digit,
            allow_same_as_old: false,
            max_failures: 5,
        }
    }
}

/// 使用者的密碼狀態（zx10 雜湊、zx16、zx17、zx19、zx20）
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Credential {
    /// zx10：Argon2id 雜湊；None 為尚未設定，初始密碼＝使用者代號（p_zx.4gl:561，W1）
    pub password_hash: Option<String>,
    /// zx16 密碼更新日
    pub changed_on: Option<NaiveDate>,
    /// zx17 此密碼已登入次數（依次數到期時才計）
    pub use_count: u32,
    /// zx19 下次登入須改密碼
    pub change_required: bool,
    /// zx20 已試誤次數
    pub failed_count: u32,
}

impl Credential {
    /// 管理員設定密碼（p_zx.4gl:640-650）：一律須於下次登入改密碼（W7）
    pub fn reset(hash: String, today: NaiveDate) -> Self {
        Self {
            password_hash: Some(hash),
            changed_on: Some(today),
            use_count: 0,
            change_required: true,
            failed_count: 0,
        }
    }

    /// 使用者變更密碼成功（webpasswd.4gl:140-144）
    pub fn changed(hash: String, today: NaiveDate) -> Self {
        Self {
            change_required: false,
            ..Self::reset(hash, today)
        }
    }

    /// 已達試誤上限（weblogin.4gl:450-462，azz1090）
    pub fn locked(&self, policy: &AccountPolicy) -> bool {
        policy.max_failures > 0 && self.failed_count >= policy.max_failures
    }

    /// 密碼錯誤：有試誤上限時才計次（weblogin.4gl:525-528）
    pub fn record_failure(&mut self, policy: &AccountPolicy) {
        if policy.max_failures > 0 {
            self.failed_count += 1;
        }
    }

    /// 密碼正確：試誤次數歸零；依次數到期時登入次數＋1（weblogin.4gl:535-548、:611-615）
    pub fn record_success(&mut self, policy: &AccountPolicy) {
        self.failed_count = 0;
        if policy.expiry_basis == ExpiryBasis::Uses && policy.use_limit > 0 {
            self.use_count += 1;
        }
    }
}

/// 密碼期限（weblogin.4gl:551-608）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordAge {
    Valid,
    /// 即將到期：剩餘天數（依天數）或剩餘次數（依次數）
    ExpiringInDays(i64),
    ExpiringInUses(u32),
    Expired,
}

pub fn password_age(policy: &AccountPolicy, cred: &Credential, today: NaiveDate) -> PasswordAge {
    match policy.expiry_basis {
        ExpiryBasis::Days if policy.cycle_days > 0 => {
            let Some(changed) = cred.changed_on else {
                return PasswordAge::Expired;
            };
            let expires = changed + Duration::days(i64::from(policy.cycle_days));
            if expires < today {
                PasswordAge::Expired
            } else if expires - Duration::days(i64::from(policy.notify)) < today {
                PasswordAge::ExpiringInDays((expires - today).num_days())
            } else {
                PasswordAge::Valid
            }
        }
        ExpiryBasis::Uses if policy.use_limit > 0 => {
            if policy.use_limit < cred.use_count {
                PasswordAge::Expired
            } else if policy.use_limit - cred.use_count <= policy.notify {
                PasswordAge::ExpiringInUses(policy.use_limit - cred.use_count)
            } else {
                PasswordAge::Valid
            }
        }
        _ => PasswordAge::Valid,
    }
}

/// 密碼強度（webpasswd.4gl:154-199 ver_passwd）
pub fn check_strength(
    policy: &AccountPolicy,
    user_id: &str,
    password: &str,
) -> Result<(), DomainError> {
    let len = password.chars().count();
    if len > MAX_LEN {
        return Err(DomainError::PasswordTooLong { max: MAX_LEN });
    }
    if policy.min_length > 0 && len < policy.min_length as usize {
        return Err(DomainError::PasswordTooShort {
            min: policy.min_length as usize,
        });
    }
    if policy.complexity >= Complexity::NoUserId
        && password
            .to_lowercase()
            .contains(&user_id.trim().to_lowercase())
    {
        return Err(DomainError::PasswordContainsUserId);
    }
    if policy.complexity >= Complexity::Digit && !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(DomainError::PasswordNeedsDigit);
    }
    if policy.complexity >= Complexity::Symbol
        && !password.chars().any(|c| !c.is_ascii_alphanumeric())
    {
        return Err(DomainError::PasswordNeedsSymbol);
    }
    Ok(())
}

/// 變更密碼的檢查（webpasswd.4gl:78-118）；舊密碼是否正確由呼叫端驗證雜湊後傳入
pub fn check_new_password(
    policy: &AccountPolicy,
    user_id: &str,
    old_correct: bool,
    old: &str,
    new: &str,
    confirm: &str,
) -> Result<(), DomainError> {
    if !old_correct {
        return Err(DomainError::OldPasswordIncorrect);
    }
    if new != confirm {
        return Err(DomainError::PasswordConfirmMismatch);
    }
    check_strength(policy, user_id, new)?;
    if !policy.allow_same_as_old && new == old {
        return Err(DomainError::PasswordSameAsOld);
    }
    Ok(())
}

/// 以重設連結設定新密碼（ADR 0010 決策 A5）：規則同變更密碼，但不需舊密碼；
/// 是否與現行密碼相同由呼叫端比對雜湊後傳入
pub fn check_reset_password(
    policy: &AccountPolicy,
    user_id: &str,
    same_as_current: bool,
    new: &str,
    confirm: &str,
) -> Result<(), DomainError> {
    if new != confirm {
        return Err(DomainError::PasswordConfirmMismatch);
    }
    check_strength(policy, user_id, new)?;
    if !policy.allow_same_as_old && same_as_current {
        return Err(DomainError::PasswordSameAsOld);
    }
    Ok(())
}

/// 登入後是否須先改密碼（weblogin.4gl:329-388）：尚未設定密碼、管理員要求、不符現行規則或已過期（W4）
pub fn must_change(
    policy: &AccountPolicy,
    user_id: &str,
    cred: &Credential,
    password: &str,
    today: NaiveDate,
) -> bool {
    cred.password_hash.is_none()
        || cred.change_required
        || check_strength(policy, user_id, password).is_err()
        || password_age(policy, cred, today) == PasswordAge::Expired
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn strength_follows_complexity() {
        let p = AccountPolicy::default();
        assert!(check_strength(&p, "u001", "Secret123").is_ok());
        assert_eq!(
            check_strength(&p, "u001", "Ab1"),
            Err(DomainError::PasswordTooShort { min: 8 })
        );
        assert_eq!(
            check_strength(&p, "u001", "U001pass9"),
            Err(DomainError::PasswordContainsUserId)
        );
        assert_eq!(
            check_strength(&p, "u001", "NoDigitsHere"),
            Err(DomainError::PasswordNeedsDigit)
        );
        let s = AccountPolicy {
            complexity: Complexity::Symbol,
            ..p
        };
        assert_eq!(
            check_strength(&s, "u001", "Secret123"),
            Err(DomainError::PasswordNeedsSymbol)
        );
        assert!(check_strength(&s, "u001", "Secret12#").is_ok());
    }

    #[test]
    fn expiry_by_days() {
        let p = AccountPolicy::default();
        let today = date("2026-11-04");
        let cred = |d| Credential {
            changed_on: Some(today - Duration::days(d)),
            ..Credential::default()
        };
        assert_eq!(password_age(&p, &cred(91), today), PasswordAge::Expired);
        assert_eq!(
            password_age(&p, &cred(90), today),
            PasswordAge::ExpiringInDays(0)
        );
        assert_eq!(
            password_age(&p, &cred(85), today),
            PasswordAge::ExpiringInDays(5)
        );
        assert_eq!(password_age(&p, &cred(10), today), PasswordAge::Valid);
    }

    #[test]
    fn expiry_by_uses() {
        let p = AccountPolicy {
            expiry_basis: ExpiryBasis::Uses,
            use_limit: 10,
            notify: 2,
            ..AccountPolicy::default()
        };
        let today = date("2026-11-04");
        let cred = |n| Credential {
            use_count: n,
            ..Credential::default()
        };
        assert_eq!(password_age(&p, &cred(11), today), PasswordAge::Expired);
        assert_eq!(
            password_age(&p, &cred(8), today),
            PasswordAge::ExpiringInUses(2)
        );
        assert_eq!(password_age(&p, &cred(3), today), PasswordAge::Valid);
    }

    #[test]
    fn lock_after_max_failures() {
        let p = AccountPolicy::default();
        let mut c = Credential::default();
        for _ in 0..5 {
            assert!(!c.locked(&p));
            c.record_failure(&p);
        }
        assert!(c.locked(&p));
        c.record_success(&p);
        assert!(!c.locked(&p));
    }
}
