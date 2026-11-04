//! User Maintenance 使用者（TipTop: p_zx / zx_file，可使用營運中心 zxy_file）
//! 規格：specs/features/azz/user_maintenance.feature（U1–U9）；登入與權限：docs/adr/0006
//!
//! 密碼相關欄位（zx10、zx16、zx17、zx19、zx20）不移植，由公司 IdP 處理（ADR 0006 §3）。

use chrono::NaiveDate;

use crate::{
    DomainError,
    lang::{Lang, normalize_text},
    plant::PlantId,
    program::check_length,
};

/// 欄位上限（TipTop zx_file.sch；與 ttrs_shared::user_max_len 一致）
pub mod max_len {
    pub const USER_ID: usize = 10;
    pub const NAME: usize = 80;
    pub const DEPT: usize = 10;
    pub const EMAIL: usize = 80;
    pub const IDP_USERNAME: usize = 255;
}

/// 使用者代號（zx01）：必填、最多 10 碼；建立後不可修改（U1）
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UserId(String);

impl UserId {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::UserIdEmpty);
        }
        if v.chars().count() > max_len::USER_ID {
            return Err(DomainError::UserIdTooLong {
                max: max_len::USER_ID,
            });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 可由畫面輸入的欄位
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserAttrs {
    pub name: Option<String>,
    pub dept_code: Option<String>,
    pub permission_class: String,
    /// zx05 使用者選單（程式代號；是否為選單程式由 app 層查證）
    pub menu: String,
    pub multi_plant: bool,
    pub default_plant: PlantId,
    pub email: Option<String>,
    pub idp_username: Option<String>,
    pub lang: Lang,
    pub valid_until: Option<NaiveDate>,
}

impl UserAttrs {
    /// 正規化與長度、格式檢核
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: Option<String>,
        dept_code: Option<String>,
        permission_class: &str,
        menu: &str,
        multi_plant: bool,
        default_plant: PlantId,
        email: Option<String>,
        idp_username: Option<String>,
        lang: &str,
        valid_until: Option<NaiveDate>,
    ) -> Result<Self, DomainError> {
        let attrs = Self {
            name: normalize_text(name),
            dept_code: normalize_text(dept_code).map(|d| d.to_uppercase()),
            permission_class: permission_class.trim().to_owned(),
            menu: menu.trim().to_owned(),
            multi_plant,
            default_plant,
            email: normalize_text(email),
            // 公司帳號不分大小寫（U2：不可重複）
            idp_username: normalize_text(idp_username).map(|u| u.to_lowercase()),
            lang: Lang::parse(lang)?,
            valid_until,
        };
        check_length("使用者名稱", attrs.name.as_deref(), max_len::NAME)?;
        check_length("部門代號", attrs.dept_code.as_deref(), max_len::DEPT)?;
        check_length("電子郵件", attrs.email.as_deref(), max_len::EMAIL)?;
        check_length(
            "公司帳號",
            attrs.idp_username.as_deref(),
            max_len::IDP_USERNAME,
        )?;
        if let Some(e) = &attrs.email
            && !is_email(e)
        {
            return Err(DomainError::UserEmailInvalid(e.clone()));
        }
        Ok(attrs)
    }
}

/// 基本電子郵件格式（U8）：local@domain，domain 含「.」，不含空白
pub(crate) fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !s.chars().any(char::is_whitespace)
}

/// 使用者（zx_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub user_id: UserId,
    pub attrs: UserAttrs,
    /// 首次登入時綁定的公司帳號識別碼（ADR 0006 §2）
    pub idp_subject: Option<String>,
    /// zxacti 資料有效
    pub active: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
}

/// 依失效日期決定是否有效：失效日期已到（含今天）即無效（p_zx.4gl:977-981）
pub fn active_on(valid_until: Option<NaiveDate>, today: NaiveDate) -> bool {
    valid_until.is_none_or(|d| d > today)
}

impl User {
    /// 新增：資料所有者／部門＝目前使用者（p_zx.4gl:441-455）
    pub fn new(
        user_id: UserId,
        attrs: UserAttrs,
        actor: &str,
        actor_dept: &str,
        today: NaiveDate,
    ) -> Self {
        let active = active_on(attrs.valid_until, today);
        Self {
            user_id,
            attrs,
            idp_subject: None,
            active,
            created_by: actor.to_owned(),
            created_dept: actor_dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
        }
    }

    /// 修改：記錄修改者與日期，依失效日期重算有效（p_zx.4gl:953-954、977-981）。
    /// 公司帳號變更時解除原綁定，下次登入重新綁定。
    pub fn update(&mut self, attrs: UserAttrs, actor: &str, today: NaiveDate) {
        if attrs.idp_username != self.attrs.idp_username {
            self.idp_subject = None;
        }
        self.active = active_on(attrs.valid_until, today);
        self.attrs = attrs;
        self.touch(actor, today);
    }

    /// 停用：失效日期設為今天（p_zx.4gl:1125-1127）
    pub fn deactivate(&mut self, actor: &str, today: NaiveDate) {
        self.active = false;
        self.attrs.valid_until = Some(today);
        self.touch(actor, today);
    }

    /// 啟用：清空失效日期（p_zx.4gl:1128-1130）
    pub fn activate(&mut self, actor: &str, today: NaiveDate) {
        self.active = true;
        self.attrs.valid_until = None;
        self.touch(actor, today);
    }

    /// 複製：新代號、名稱與公司帳號清空、資料所有者為目前使用者（p_zx.4gl:1343-1349）
    pub fn copy_as(&self, new_id: UserId, actor: &str, actor_dept: &str, today: NaiveDate) -> Self {
        let attrs = UserAttrs {
            name: None,
            idp_username: None,
            ..self.attrs.clone()
        };
        Self::new(new_id, attrs, actor, actor_dept, today)
    }

    /// 改為權限類別的預設選單（p_zw「更新所有使用者目錄編號」，W9 記錄修改者）
    pub fn set_menu(&mut self, menu: &str, actor: &str, today: NaiveDate) {
        self.attrs.menu = menu.to_owned();
        self.touch(actor, today);
    }

    fn touch(&mut self, actor: &str, today: NaiveDate) {
        self.updated_by = Some(actor.to_owned());
        self.updated_on = Some(today);
    }
}

/// 變更預設營運中心後的可使用營運中心（p_zx.4gl:1024-1062）：
/// 多營運中心使用者加入清單；非多營運中心使用者以新的預設營運中心取代
pub fn plants_after_default_change(
    current: &[PlantId],
    default_plant: &PlantId,
    multi_plant: bool,
) -> Vec<PlantId> {
    if !multi_plant {
        return vec![default_plant.clone()];
    }
    let mut plants = current.to_vec();
    if !plants.contains(default_plant) {
        plants.push(default_plant.clone());
    }
    plants
}

/// 勾選的可使用營運中心（TipTop p_zxy；user_maintenance_plants.feature PZ1、PZ3、PZ7）：
/// 非多營運中心只能有預設營運中心、不可勾全部營運中心（azz-136）；須含預設營運中心（azz-075）；
/// 重複的合併；全部營運中心以 `PlantId::all()` 一併記在清單中
pub fn chosen_plants(
    default_plant: &PlantId,
    multi_plant: bool,
    chosen: &[PlantId],
    all: bool,
) -> Result<Vec<PlantId>, DomainError> {
    let mut plants: Vec<PlantId> = Vec::with_capacity(chosen.len() + 1);
    for p in chosen.iter().filter(|p| !p.is_all()) {
        if !plants.contains(p) {
            plants.push(p.clone());
        }
    }
    if !multi_plant && (all || plants.iter().any(|p| p != default_plant)) {
        return Err(DomainError::UserSinglePlant);
    }
    if !plants.contains(default_plant) {
        return Err(DomainError::UserDefaultPlantRequired);
    }
    if all {
        plants.push(PlantId::all());
    }
    Ok(plants)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn user_id_rules() {
        assert_eq!(UserId::parse(" ").unwrap_err().code(), "USER_ID_EMPTY");
        assert_eq!(
            UserId::parse("u0123456789").unwrap_err().code(),
            "USER_ID_TOO_LONG"
        );
        assert_eq!(UserId::parse(" u001 ").unwrap().as_str(), "u001");
    }

    #[test]
    fn active_by_valid_until() {
        let today = date("2026-10-30");
        assert!(active_on(None, today));
        assert!(active_on(Some(date("2026-10-31")), today));
        assert!(!active_on(Some(today), today));
        assert!(!active_on(Some(date("2026-10-29")), today));
    }

    #[test]
    fn email_format() {
        assert!(is_email("u001@example.com"));
        assert!(!is_email("not-an-email"));
        assert!(!is_email("a@b"));
        assert!(!is_email("a b@example.com"));
    }

    #[test]
    fn default_plant_change() {
        let tw = PlantId::parse("TW01").unwrap();
        let cn = PlantId::parse("CN01").unwrap();
        assert_eq!(
            plants_after_default_change(std::slice::from_ref(&tw), &cn, false),
            vec![cn.clone()]
        );
        assert_eq!(
            plants_after_default_change(std::slice::from_ref(&tw), &cn, true),
            vec![tw, cn]
        );
    }

    #[test]
    fn chosen_plant_rules() {
        let tw = PlantId::parse("TW01").unwrap();
        let cn = PlantId::parse("CN01").unwrap();
        assert_eq!(
            chosen_plants(&tw, true, &[tw.clone(), cn.clone(), cn.clone()], false).unwrap(),
            vec![tw.clone(), cn.clone()]
        );
        assert_eq!(
            chosen_plants(&tw, true, std::slice::from_ref(&cn), false)
                .unwrap_err()
                .code(),
            "USER_DEFAULT_PLANT_REQUIRED"
        );
        assert_eq!(
            chosen_plants(&tw, false, &[tw.clone(), cn.clone()], false)
                .unwrap_err()
                .code(),
            "USER_SINGLE_PLANT"
        );
        assert_eq!(
            chosen_plants(&tw, false, std::slice::from_ref(&tw), true)
                .unwrap_err()
                .code(),
            "USER_SINGLE_PLANT"
        );
        assert!(
            chosen_plants(&tw, true, std::slice::from_ref(&tw), true)
                .unwrap()
                .iter()
                .any(PlantId::is_all)
        );
    }
}
