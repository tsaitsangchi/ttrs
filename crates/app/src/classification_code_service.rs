//! Classification Code Maintenance 分類碼維護 use case（TipTop: aooi300 / azf_file）
//! 規格：specs/features/aoo/classification_code.feature（CC1–CC9）
//!
//! 工廠層：每個操作都指定營運中心（ADR 0007，CC8）。TipTop 為多筆單身維護（aooi300.4gl:362-600），
//! 權限沿用單身：查詢 query、維護 detail（新增列 add_row、刪除列 delete_row）。TipTop 沒有設定資料所有者，不檢查資料權限。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    classification::{ClassificationCode, Code, CodeAttrs, CodeKind},
    plant::PlantId,
};
use ttrs_shared::{ClassificationCodeDto, ClassificationCodeInput};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    ports::{ClassificationCodeRepository, MasterDataRepository},
};

/// 權限檢查用的程式代號（CC1：一支作業，以類別切換）
pub const PROGRAM: &str = "classification-code-maintenance";

#[derive(Clone)]
pub struct ClassificationCodeService {
    authz: Arc<dyn Authorizer>,
    codes: Arc<dyn ClassificationCodeRepository>,
    master: Arc<dyn MasterDataRepository>,
}

impl ClassificationCodeService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        codes: Arc<dyn ClassificationCodeRepository>,
        master: Arc<dyn MasterDataRepository>,
    ) -> Self {
        Self {
            authz,
            codes,
            master,
        }
    }

    /// 該類別的全部分類碼（TipTop 單身），依代號排序
    pub async fn list(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
    ) -> Result<Vec<ClassificationCodeDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let kind = CodeKind::parse(kind)?;
        let rows = self.codes.list_codes(&plant, kind).await.map_err(infra)?;
        Ok(rows.iter().map(to_dto).collect())
    }

    /// 新增（aooi300.4gl:362-424）：資料有效預設 Y；同一類別內代號不可重複
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        input: ClassificationCodeInput,
    ) -> Result<ClassificationCodeDto, AppError> {
        self.authorize(actor, "detail").await?.add_row()?;
        let plant = PlantId::parse(plant)?;
        if !self.master.plant_active(&plant).await.map_err(infra)? {
            return Err(DomainError::PlantNotFound.into());
        }
        let kind = CodeKind::parse(kind)?;
        let code = Code::parse(&input.code)?;
        let attrs = attrs(kind, input)?;
        let c = ClassificationCode::new(kind, code, attrs, &actor.user_id, &actor.dept, today());
        match self.codes.insert_code(&plant, &c).await {
            Ok(()) => Ok(to_dto(&c)),
            Err(RepoError::Conflict) => Err(DomainError::CodeAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    /// 修改說明等欄位；代號建立後不可修改（以網址的代號為準）
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        code: &str,
        input: ClassificationCodeInput,
    ) -> Result<ClassificationCodeDto, AppError> {
        self.authorize(actor, "detail").await?;
        let plant = PlantId::parse(plant)?;
        let mut c = self.load(&plant, kind, code).await?;
        c.update(attrs(c.kind, input)?, &actor.user_id, today());
        self.save(&plant, &c).await
    }

    /// 資料有效（azfacti，aooi300.4gl:523-529）
    pub async fn set_active(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        code: &str,
        active: bool,
    ) -> Result<ClassificationCodeDto, AppError> {
        self.authorize(actor, "detail").await?;
        let plant = PlantId::parse(plant)?;
        let mut c = self.load(&plant, kind, code).await?;
        c.set_active(active, &actor.user_id, today());
        self.save(&plant, &c).await
    }

    /// 刪除（aooi300.4gl:531-548）：只刪該類別的代號（CC4）。
    /// 已被料件引用者不可刪除（CC5）——料件的分群欄位移植後由 repository 回 InUse
    pub async fn delete(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        code: &str,
    ) -> Result<(), AppError> {
        self.authorize(actor, "detail").await?.delete_row()?;
        let plant = PlantId::parse(plant)?;
        let kind = CodeKind::parse(kind)?;
        let code = Code::parse(code).map_err(|_| DomainError::CodeNotFound)?;
        match self.codes.delete_code(&plant, kind, &code).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(DomainError::CodeNotFound.into()),
            Err(RepoError::InUse) => Err(DomainError::CodeInUse.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn load(
        &self,
        plant: &PlantId,
        kind: &str,
        code: &str,
    ) -> Result<ClassificationCode, AppError> {
        let kind = CodeKind::parse(kind)?;
        let code = Code::parse(code).map_err(|_| DomainError::CodeNotFound)?;
        self.codes
            .get_code(plant, kind, &code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::CodeNotFound.into())
    }

    async fn save(
        &self,
        plant: &PlantId,
        c: &ClassificationCode,
    ) -> Result<ClassificationCodeDto, AppError> {
        self.codes.update_code(plant, c).await.map_err(infra)?;
        Ok(to_dto(c))
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn attrs(kind: CodeKind, i: ClassificationCodeInput) -> Result<CodeAttrs, AppError> {
    Ok(CodeAttrs::new(
        kind,
        i.description,
        i.variance_account,
        i.variance_account2,
        i.cost_nature,
    )?)
}

fn to_dto(c: &ClassificationCode) -> ClassificationCodeDto {
    ClassificationCodeDto {
        kind: c.kind.code().to_owned(),
        code: c.code.as_str().to_owned(),
        description: c.attrs.description.clone(),
        variance_account: c.attrs.variance_account.clone(),
        variance_account2: c.attrs.variance_account2.clone(),
        cost_nature: c.attrs.cost_nature,
        active: c.active,
        created_by: c.created_by.clone(),
        created_dept: c.created_dept.clone(),
        created_on: c.created_on,
        updated_by: c.updated_by.clone(),
        updated_on: c.updated_on,
    }
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_max_len_matches_domain() {
        use ttrs_domain::classification::max_len as d;
        use ttrs_shared::classification_max_len as s;
        assert_eq!(
            (d::CODE, d::DESCRIPTION, d::ACCOUNT),
            (s::CODE, s::DESCRIPTION, s::ACCOUNT)
        );
    }
}
