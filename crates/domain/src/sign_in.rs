//! 登入與切換營運中心（TipTop: weblogin、aoos901、s_chkdbs）
//! 規格：specs/features/azz/sign_in.feature（SQ1–SQ10）
//!
//! 身分由公司 SSO 驗證（ADR 0006 §1）；這裡只做 TipTop 登入後的帳號與營運中心檢查。
//! 密碼、試誤次數與帳號稽核原則（zx10、zx16、zx17、zx19、zx20、gbt_file）不移植（SQ10）。

use chrono::NaiveDate;

use crate::{
    DomainError,
    plant::{Plant, PlantId},
    user::User,
};

/// 帳號檢查（weblogin.4gl:426-476 user_validate）：資料有效（zxacti，lib-064）；
/// 有效日期早於今天即過期（zx18，azz1092），等於今天仍可登入（SQ3）
pub fn check_account(user: &User, today: NaiveDate) -> Result<(), DomainError> {
    if !user.active {
        return Err(DomainError::UserInactive);
    }
    if user.attrs.valid_until.is_some_and(|d| d < today) {
        return Err(DomainError::UserExpired);
    }
    Ok(())
}

/// 使用者可否使用該營運中心（s_chkdbs：zxy03＝營運中心或 ALL，sub-118）
pub fn plant_allowed(allowed: &[PlantId], plant: &PlantId) -> bool {
    allowed.iter().any(|p| p.is_all() || p == plant)
}

/// 進入營運中心前的檢查：須存在（azp_file）、可使用（s_chkdbs）、有效（SQ6）
pub fn check_plant(
    allowed: &[PlantId],
    target: &PlantId,
    plant: Option<&Plant>,
) -> Result<(), DomainError> {
    let plant = plant.ok_or(DomainError::PlantNotFound)?;
    if !plant_allowed(allowed, target) {
        return Err(DomainError::PlantNotAllowed(target.as_str().to_owned()));
    }
    if !plant.active {
        return Err(DomainError::PlantInactive(target.as_str().to_owned()));
    }
    Ok(())
}

/// 切換營運中心：非多營運中心使用者不能選（aoos901.4gl:74、:106，SQ7）
pub fn check_switch(user: &User) -> Result<(), DomainError> {
    if !user.attrs.multi_plant {
        return Err(DomainError::UserSinglePlant);
    }
    Ok(())
}

/// 可切換的營運中心（q_zxy）：可使用且有效者，依代號排序；ALL 為全部有效的營運中心
pub fn switchable_plants<'a>(allowed: &[PlantId], plants: &'a [Plant]) -> Vec<&'a Plant> {
    let mut out: Vec<&Plant> = plants
        .iter()
        .filter(|p| p.active && plant_allowed(allowed, &p.id))
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> PlantId {
        PlantId::parse(s).unwrap()
    }

    #[test]
    fn all_allows_any_plant() {
        assert!(plant_allowed(&[PlantId::all()], &id("JP01")));
        assert!(plant_allowed(&[id("TW01")], &id("TW01")));
        assert!(!plant_allowed(&[id("TW01")], &id("JP01")));
    }
}
