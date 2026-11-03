//! 料件維護 use case（TipTop: aimi100）
//! 權限：這次只檢查功能；料件的資料所有者、所有部門（imauser、imagrup）等 aimi100 正式移植時再做（authorization.feature AQ5）

use std::sync::Arc;

use ttrs_domain::{
    DomainError,
    item::{Item, ItemNo, ItemStatus},
    plant::PlantId,
};
use ttrs_shared::{CreateItemRequest, ItemDto, ItemStatusDto};

use crate::{
    Actor, AppError, Authorizer,
    error::RepoError,
    ports::{ItemRepository, MasterDataRepository},
};

/// 權限檢查用的程式代號
const PROGRAM: &str = "aimi100";

#[derive(Clone)]
pub struct ItemService {
    authz: Arc<dyn Authorizer>,
    items: Arc<dyn ItemRepository>,
    master: Arc<dyn MasterDataRepository>,
}

impl ItemService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        items: Arc<dyn ItemRepository>,
        master: Arc<dyn MasterDataRepository>,
    ) -> Self {
        Self {
            authz,
            items,
            master,
        }
    }

    pub async fn create(&self, actor: &Actor, req: CreateItemRequest) -> Result<ItemDto, AppError> {
        self.authorize(actor, "insert").await?;
        let plant = PlantId::parse(&req.plant_id)?;
        let no = ItemNo::parse(&req.item_no)?;
        let item = Item::new(plant, no, &req.name, req.spec.as_deref(), &req.stock_unit)?;

        if !self
            .master
            .plant_active(&item.plant_id)
            .await
            .map_err(infra)?
        {
            return Err(DomainError::PlantNotFound.into());
        }
        if !self
            .master
            .unit_exists(&item.stock_unit)
            .await
            .map_err(infra)?
        {
            return Err(DomainError::UnitNotFound.into());
        }

        match self.items.insert(&item).await {
            Ok(()) => Ok(to_dto(&item)),
            Err(RepoError::Conflict) => Err(DomainError::ItemAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    pub async fn get(&self, actor: &Actor, plant: &str, no: &str) -> Result<ItemDto, AppError> {
        self.authorize(actor, "query").await?;
        let (plant, no) = parse_key(plant, no)?;
        Ok(to_dto(&self.load(&plant, &no).await?))
    }

    pub async fn list(&self, actor: &Actor, plant: &str) -> Result<Vec<ItemDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let items = self.items.list(&plant).await.map_err(infra)?;
        Ok(items.iter().map(to_dto).collect())
    }

    pub async fn confirm(&self, actor: &Actor, plant: &str, no: &str) -> Result<ItemDto, AppError> {
        self.authorize(actor, "confirm").await?;
        let (plant, no) = parse_key(plant, no)?;
        let mut item = self.load(&plant, &no).await?;
        item.confirm()?;
        self.items.update(&item).await.map_err(infra)?;
        Ok(to_dto(&item))
    }

    /// 取消確認規則：已有庫存異動者不可取消確認。
    pub async fn unconfirm(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
    ) -> Result<ItemDto, AppError> {
        self.authorize(actor, "notconfirm").await?;
        let (plant, no) = parse_key(plant, no)?;
        let mut item = self.load(&plant, &no).await?;
        if self
            .items
            .has_transactions(&plant, &no)
            .await
            .map_err(infra)?
        {
            return Err(DomainError::ItemHasTransactions.into());
        }
        item.unconfirm()?;
        self.items.update(&item).await.map_err(infra)?;
        Ok(to_dto(&item))
    }

    /// 刪除規則：已有庫存異動者不可刪；已確認者須先取消確認。
    pub async fn delete(&self, actor: &Actor, plant: &str, no: &str) -> Result<(), AppError> {
        self.authorize(actor, "delete").await?;
        let (plant, no) = parse_key(plant, no)?;
        let item = self.load(&plant, &no).await?;
        if self
            .items
            .has_transactions(&plant, &no)
            .await
            .map_err(infra)?
        {
            return Err(DomainError::ItemHasTransactions.into());
        }
        if item.status == ItemStatus::Confirmed {
            return Err(DomainError::InvalidItemTransition {
                from: item.status.label(),
                action: "刪除",
            }
            .into());
        }
        self.items.delete(&plant, &no).await.map_err(infra)
    }

    async fn load(&self, plant: &PlantId, no: &ItemNo) -> Result<Item, AppError> {
        self.items
            .get(plant, no)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::ItemNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<(), AppError> {
        self.authz
            .authorize(actor, PROGRAM, action)
            .await
            .map(|_| ())
    }
}

fn parse_key(plant: &str, no: &str) -> Result<(PlantId, ItemNo), DomainError> {
    Ok((PlantId::parse(plant)?, ItemNo::parse(no)?))
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}

fn to_dto(i: &Item) -> ItemDto {
    ItemDto {
        plant_id: i.plant_id.as_str().to_owned(),
        item_no: i.item_no.as_str().to_owned(),
        name: i.name.clone(),
        spec: i.spec.clone(),
        stock_unit: i.stock_unit.clone(),
        status: match i.status {
            ItemStatus::Unconfirmed => ItemStatusDto::Unconfirmed,
            ItemStatus::Confirmed => ItemStatusDto::Confirmed,
            ItemStatus::Voided => ItemStatusDto::Voided,
        },
    }
}
