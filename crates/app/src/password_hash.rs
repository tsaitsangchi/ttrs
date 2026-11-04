//! 密碼雜湊（ADR 0009 決策 2）：Argon2id、隨機 salt，存 PHC 字串；不存明文、不可還原

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};

use crate::AppError;

pub fn hash(password: &str) -> Result<String, AppError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| AppError::Infra(format!("亂數：{e}")))?;
    let salt = SaltString::encode_b64(&bytes).map_err(|e| AppError::Infra(e.to_string()))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Infra(format!("密碼雜湊：{e}")))
}

/// 密碼是否與雜湊相符；雜湊格式不正確視為不符
pub fn verify(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| {
        Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify() {
        let h = hash("Secret123").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(verify(&h, "Secret123"));
        assert!(!verify(&h, "secret123"));
        assert_ne!(h, hash("Secret123").unwrap(), "salt 每次不同");
    }
}
