//! Plant Maintenance 營運中心維護 use case（TipTop: aooi901）
//! 規格：specs/features/aoo/plant_maintenance.feature（P1–P12）
//!
//! TipTop 新增營運中心需先由 DBA 建 schema；新系統只新增一筆 plant 資料，不建任何表格（ADR 0001 §4）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    plant::{Plant, PlantAttrs, PlantId},
};
use ttrs_shared::{Page, PlantDto, PlantInput, QueryRequest};

use crate::{
    Actor, AppError, Authorizer, authorization::Grant, error::RepoError, ports::PlantRepository,
    query,
};

/// 權限檢查用的程式代號；TipTop 沒有設定資料所有者，不檢查資料權限（AQ5）
const PROGRAM: &str = "aooi901";

#[derive(Clone)]
pub struct PlantService {
    authz: Arc<dyn Authorizer>,
    plants: Arc<dyn PlantRepository>,
}

impl PlantService {
    pub fn new(authz: Arc<dyn Authorizer>, plants: Arc<dyn PlantRepository>) -> Self {
        Self { authz, plants }
    }

    /// 新增（aooi901.4gl:296-388）
    pub async fn create(&self, actor: &Actor, input: PlantInput) -> Result<PlantDto, AppError> {
        self.authorize(actor, "insert").await?;
        let id = PlantId::parse(&input.plant_id)?;
        let plant = Plant::new(id, attrs(&input)?, &actor.user_id, &actor.dept, today());
        self.insert(plant).await
    }

    /// 修改（aooi901.4gl:753-911）：代號不可變（P4）
    pub async fn update(
        &self,
        actor: &Actor,
        id: &str,
        input: PlantInput,
    ) -> Result<PlantDto, AppError> {
        self.authorize(actor, "modify").await?;
        let mut plant = self.load(id).await?;
        if !input.plant_id.trim().is_empty() && PlantId::parse(&input.plant_id)? != plant.id {
            return Err(DomainError::PlantIdImmutable.into());
        }
        plant.update(attrs(&input)?, &actor.user_id, today());
        self.plants.update(&plant).await.map_err(infra)?;
        Ok(to_dto(&plant))
    }

    pub async fn get(&self, actor: &Actor, id: &str) -> Result<PlantDto, AppError> {
        self.authorize(actor, "query").await?;
        Ok(to_dto(&self.load(id).await?))
    }

    /// QBE 查詢（i901_curs，aooi901.4gl:119-156）；欄位見 query::plant（PQ1–PQ3）
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<PlantDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::plant::filters(&req.conditions)?;
        let p = query::paging(&req);
        let (rows, total) = self
            .plants
            .query(&filters, p.offset, u64::from(p.page_size))
            .await
            .map_err(infra)?;
        let items = rows.iter().map(to_dto).collect();
        Ok(Page {
            items,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 全部清單：下拉選單、欄位檢查用的參考資料，只要求登入（AQ4）
    pub async fn list(&self) -> Result<Vec<PlantDto>, AppError> {
        let plants = self.plants.list().await.map_err(infra)?;
        Ok(plants.iter().map(to_dto).collect())
    }

    /// 刪除：仍有使用者或業務資料時拒絕，不做連帶刪除（aooi901.4gl:944-953 aoo-088，P10）
    pub async fn delete(&self, actor: &Actor, id: &str) -> Result<(), AppError> {
        self.authorize(actor, "delete").await?;
        let plant = self.load(id).await?;
        match self.plants.delete(&plant.id).await {
            Ok(()) => Ok(()),
            Err(RepoError::InUse) => Err(DomainError::PlantInUse.into()),
            Err(e) => Err(infra(e)),
        }
    }

    /// 停用（P9）：一律允許，既有資料不受影響；之後不可再指定給使用者或新的業務資料。
    /// azp_file 沒有有效碼，停用／啟用用 modify（AQ3）
    pub async fn deactivate(&self, actor: &Actor, id: &str) -> Result<PlantDto, AppError> {
        self.set_active(actor, id, false).await
    }

    pub async fn activate(&self, actor: &Actor, id: &str) -> Result<PlantDto, AppError> {
        self.set_active(actor, id, true).await
    }

    /// 複製（aooi901.4gl:994-1046，P11）
    pub async fn copy(&self, actor: &Actor, id: &str, new_id: &str) -> Result<PlantDto, AppError> {
        self.authorize(actor, "reproduce").await?;
        let source = self.load(id).await?;
        let new_id = PlantId::parse(new_id)?;
        self.insert(source.copy_as(new_id, &actor.user_id, &actor.dept, today()))
            .await
    }

    async fn insert(&self, plant: Plant) -> Result<PlantDto, AppError> {
        match self.plants.insert(&plant).await {
            Ok(()) => Ok(to_dto(&plant)),
            Err(RepoError::Conflict) => Err(DomainError::PlantAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn set_active(
        &self,
        actor: &Actor,
        id: &str,
        active: bool,
    ) -> Result<PlantDto, AppError> {
        self.authorize(actor, "modify").await?;
        let mut plant = self.load(id).await?;
        plant.set_active(active, &actor.user_id, today());
        self.plants.update(&plant).await.map_err(infra)?;
        Ok(to_dto(&plant))
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }

    async fn load(&self, id: &str) -> Result<Plant, AppError> {
        let id = PlantId::parse(id).map_err(|_| DomainError::PlantNotFound)?;
        self.plants
            .get(&id)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::PlantNotFound.into())
    }
}

fn attrs(i: &PlantInput) -> Result<PlantAttrs, AppError> {
    Ok(PlantAttrs::new(
        i.name.clone(),
        i.time_zone.as_deref(),
        i.address.clone(),
        i.phone.clone(),
        i.fax.clone(),
    )?)
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

fn to_dto(p: &Plant) -> PlantDto {
    let a = &p.attrs;
    PlantDto {
        plant_id: p.id.as_str().to_owned(),
        name: a.name.clone(),
        time_zone: a.time_zone.clone(),
        address: a.address.clone(),
        phone: a.phone.clone(),
        fax: a.fax.clone(),
        active: p.active,
        created_by: p.created_by.clone(),
        created_dept: p.created_dept.clone(),
        created_on: p.created_on,
        updated_by: p.updated_by.clone(),
        updated_on: p.updated_on,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_max_len_matches_domain() {
        use ttrs_domain::plant::max_len as d;
        use ttrs_shared::plant_max_len as s;
        assert_eq!(d::ID, s::ID);
        assert_eq!(d::NAME, s::NAME);
        assert_eq!(d::ADDRESS, s::ADDRESS);
        assert_eq!(d::PHONE, s::PHONE);
        assert_eq!(d::FAX, s::FAX);
    }
}
