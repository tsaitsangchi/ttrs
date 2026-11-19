//! Item Storage Maintenance 料件預設倉儲維護（TipTop aimi202 / imf_file；specs/features/aim/item_storage.feature）
//! 單頭是料件（唯讀），單身是預設倉庫、儲位、最高存量。寫入一律經 change_log::run（ADR 0012）。
//! 標準功能例外（IS6）：單頭新增、刪除由料件維護；失效、確認不適用。

use std::sync::Arc;

use ttrs_domain::{
    DomainError,
    item::{ActiveCode, Item, ItemNo},
    item_storage::{self, ItemStorage},
    plant::PlantId,
    warehouse::{LocationCode, Warehouse, WarehouseCode},
};
use ttrs_shared::{
    ChangeDto, ItemStorageBatchRequest, ItemStorageBatchResult, ItemStorageDto, ItemStorageInput,
    ItemStorageItemDto, Page, QbeCondition, QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    change_log::HistoryReader,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{ItemRepository, ItemStorageRepository, WarehouseRepository},
    query::{self, item_storage::View},
    report::{ReportData, ReportFormat},
    warehouse_service::check_storage,
};

pub const PROGRAM: &str = "item-storage-maintenance";

const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "item-storage-report-title",
    columns: &[
        ("item_no", "item-storage-col-item"),
        ("name", "item-storage-col-name"),
        ("spec", "item-storage-col-spec"),
        ("warehouse_code", "item-storage-col-warehouse"),
        ("warehouse_name", "item-storage-col-warehouse-name"),
        ("location_code", "item-storage-col-location"),
        ("max_qty", "item-storage-col-max-qty"),
    ],
};

#[derive(Clone)]
pub struct ItemStorageService {
    authz: Arc<dyn Authorizer>,
    storages: Arc<dyn ItemStorageRepository>,
    items: Arc<dyn ItemRepository>,
    warehouses: Arc<dyn WarehouseRepository>,
    history: HistoryReader,
    reports: ListReporter,
}

impl ItemStorageService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        storages: Arc<dyn ItemStorageRepository>,
        items: Arc<dyn ItemRepository>,
        warehouses: Arc<dyn WarehouseRepository>,
        history: HistoryReader,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            storages,
            items,
            warehouses,
            history,
            reports,
        }
    }

    /// 查詢（aimi202.4gl:123-256）：只列有效料件、依料號排序。沒有條件時只列已有預設倉儲的料件；
    /// 有條件時列出符合的有效料件（可以找到尚未設定的料件來新增，IS1）
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<ItemStorageItemDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let p = query::paging(&req);
        let rows = self.matching(&plant, &req.conditions).await?;
        let total = rows.len() as u64;
        let warehouses = self
            .warehouses
            .list_warehouses(&plant)
            .await
            .map_err(infra)?;
        let items = rows
            .iter()
            .skip(usize::try_from(p.offset).unwrap_or(usize::MAX))
            .take(p.page_size as usize)
            .map(|v| to_dto(v, &warehouses))
            .collect();
        Ok(Page {
            items,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    pub async fn get(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
    ) -> Result<ItemStorageItemDto, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let item = self.item(&plant, item).await?;
        self.view_dto(&plant, item).await
    }

    /// 單身新增（detail；aimi202.4gl:505-600）：料件有效（mfg1000）、倉庫有效（mfg1100）、儲位屬於該倉庫（mfg1101）、
    /// 同料件同倉庫儲位不可重複（-239）
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
        input: ItemStorageInput,
    ) -> Result<ItemStorageItemDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            let grant = self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let item = self.active_item(&plant, item).await?;
            grant.record(&item.created_by, &item.created_dept)?;
            let row = self.storage_of(&plant, &item.no, &input).await?;
            self.storages
                .insert_item_storage(&plant, &row)
                .await
                .map_err(conflict)?;
            self.view_dto(&plant, item).await
        })
        .await
    }

    /// 單身修改（detail；aimi202.4gl:633-665）：可改倉庫、儲位與最高存量（IS3(e)）
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
        place: (&str, &str),
        input: ItemStorageInput,
    ) -> Result<ItemStorageItemDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            let grant = self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let item = self.active_item(&plant, item).await?;
            grant.record(&item.created_by, &item.created_dept)?;
            let cur = self.load(&plant, &item.no, place).await?;
            cur.stamp.check(input.version)?;
            let mut row = if cur.warehouse.as_str() == input.warehouse_code.trim()
                && cur.location_code() == input.location_code.trim()
            {
                // 倉庫儲位沒變：只檢查最高存量（停用的倉庫仍可修改既有資料，IS10）
                ItemStorage::new(
                    item.no.clone(),
                    cur.warehouse.clone(),
                    cur.location.clone(),
                    input.max_qty,
                )?
            } else {
                self.storage_of(&plant, &item.no, &input).await?
            };
            row.stamp = cur.stamp.clone();
            self.storages
                .update_item_storage(&plant, (&cur.warehouse, cur.location.as_ref()), &row)
                .await
                .map_err(conflict)?;
            self.view_dto(&plant, item).await
        })
        .await
    }

    /// 單身刪除（detail；aimi202.4gl:600-625）
    pub async fn delete(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
        place: (&str, &str),
        version: Option<i32>,
    ) -> Result<ItemStorageItemDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            let grant = self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let item = self.item(&plant, item).await?;
            grant.record(&item.created_by, &item.created_dept)?;
            let cur = self.load(&plant, &item.no, place).await?;
            cur.stamp.check(version)?;
            self.storages
                .delete_item_storage(&plant, &cur)
                .await
                .map_err(infra)?;
            self.view_dto(&plant, item).await
        })
        .await
    }

    /// 複製（reproduce；aimi202.4gl:983-1060）：目的料件須存在且有效（mfg1201）、尚無預設倉儲（mfg1202）
    pub async fn copy(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
        target: &str,
    ) -> Result<ItemStorageItemDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            let grant = self.authorize(actor, "reproduce").await?;
            let plant = PlantId::parse(plant)?;
            let source = self.item(&plant, item).await?;
            let target = self.active_item(&plant, target).await?;
            grant.record(&target.created_by, &target.created_dept)?;
            let existing = self
                .storages
                .list_item_storages(&plant, Some(&target.no))
                .await
                .map_err(infra)?;
            if !existing.is_empty() {
                return Err(DomainError::ItemStorageCopyTargetNotEmpty.into());
            }
            let rows = self
                .storages
                .list_item_storages(&plant, Some(&source.no))
                .await
                .map_err(infra)?;
            for r in rows {
                let n = ItemStorage::new(target.no.clone(), r.warehouse, r.location, r.max_qty)?;
                self.storages
                    .insert_item_storage(&plant, &n)
                    .await
                    .map_err(conflict)?;
            }
            self.view_dto(&plant, target).await
        })
        .await
    }

    /// 整批產生（batch_generate；aimi202.4gl:1255-1330，IS7）：依料件條件替有效料件加上同一組倉庫儲位，
    /// 已有該組合的略過；最高存量不限
    pub async fn batch_generate(
        &self,
        actor: &Actor,
        plant: &str,
        req: ItemStorageBatchRequest,
    ) -> Result<ItemStorageBatchResult, AppError> {
        crate::change_log::run(actor, PROGRAM, "batch_generate", async {
            let grant = self.authorize(actor, "batch_generate").await?;
            let plant = PlantId::parse(plant)?;
            let filters = query::item_storage::batch_filters(&req.conditions)?;
            let (warehouse, location) = self
                .place(&plant, &req.warehouse_code, &req.location_code)
                .await?;
            let mut items: Vec<Item> = self
                .items
                .list_items(&plant)
                .await
                .map_err(infra)?
                .into_iter()
                .filter(|i| i.active_code != ActiveCode::Invalid)
                .filter(|i| {
                    query::matches_all(&filters, |f| query::item_storage::batch_value(i, f))
                })
                .collect();
            items.sort_by(|a, b| a.no.cmp(&b.no));
            let existing = self
                .storages
                .list_item_storages(&plant, None)
                .await
                .map_err(infra)?;
            let mut result = ItemStorageBatchResult::default();
            for i in items {
                if existing
                    .iter()
                    .any(|s| s.item == i.no && s.same_place(&warehouse, location.as_ref()))
                {
                    result.skipped += 1;
                    continue;
                }
                // 資料權限：沒有權限的料件略過
                if grant.record(&i.created_by, &i.created_dept).is_err() {
                    result.skipped += 1;
                    continue;
                }
                let n = ItemStorage::new(i.no.clone(), warehouse.clone(), location.clone(), None)?;
                self.storages
                    .insert_item_storage(&plant, &n)
                    .await
                    .map_err(conflict)?;
                result.inserted += 1;
            }
            Ok(result)
        })
        .await
    }

    /// 列印 output、匯出 Excel exporttoexcel：依目前查詢條件，每筆單身一列（IS6）
    pub async fn report_in(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let plant = PlantId::parse(plant)?;
        let rows = self.matching(&plant, &conditions).await?;
        let warehouses = self
            .warehouses
            .list_warehouses(&plant)
            .await
            .map_err(infra)?;
        let mut lines = Vec::new();
        for v in &rows {
            let d = to_dto(v, &warehouses);
            for s in &d.storages {
                lines.push(vec![
                    d.item_no.clone(),
                    d.name.clone().unwrap_or_default(),
                    d.spec.clone().unwrap_or_default(),
                    s.warehouse_code.clone(),
                    s.warehouse_name.clone().unwrap_or_default(),
                    s.location_code.clone(),
                    s.max_qty
                        .map(|q| q.normalize().to_string())
                        .unwrap_or_default(),
                ]);
            }
        }
        self.reports.check(lines.len() as u64)?;
        self.reports
            .build(
                actor,
                Some(&plant),
                &REPORT,
                lines,
                &conditions,
                format,
                lang,
            )
            .await
    }

    /// 異動記錄：該料件的預設倉儲（ADR 0012 §8）
    pub async fn history(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        self.authorize(actor, "history").await?;
        let plant = PlantId::parse(plant)?;
        let item = ItemNo::parse(item)?;
        self.history
            .read(
                Some(&plant),
                &["item_storage"],
                &[("item_no", item.as_str())],
                lang,
            )
            .await
    }

    /// 符合條件的有效料件與其預設倉儲；沒有條件時只列有預設倉儲的料件
    async fn matching(
        &self,
        plant: &PlantId,
        conditions: &[QbeCondition],
    ) -> Result<Vec<View>, AppError> {
        let filters = query::item_storage::filters(conditions)?;
        let mut all = self
            .storages
            .list_item_storages(plant, None)
            .await
            .map_err(infra)?;
        let mut items: Vec<Item> = self
            .items
            .list_items(plant)
            .await
            .map_err(infra)?
            .into_iter()
            .filter(|i| i.active_code != ActiveCode::Invalid)
            .collect();
        items.sort_by(|a, b| a.no.cmp(&b.no));
        let mut views = Vec::new();
        for item in items {
            let (mine, rest): (Vec<_>, Vec<_>) = all.into_iter().partition(|s| s.item == item.no);
            all = rest;
            if conditions.is_empty() && mine.is_empty() {
                continue;
            }
            let v = View {
                item,
                storages: sorted(mine),
            };
            if query::matches_all(&filters, |f| query::item_storage::value(&v, f)) {
                views.push(v);
            }
        }
        Ok(views)
    }

    async fn view_dto(&self, plant: &PlantId, item: Item) -> Result<ItemStorageItemDto, AppError> {
        let storages = self
            .storages
            .list_item_storages(plant, Some(&item.no))
            .await
            .map_err(infra)?;
        let warehouses = self
            .warehouses
            .list_warehouses(plant)
            .await
            .map_err(infra)?;
        Ok(to_dto(
            &View {
                item,
                storages: sorted(storages),
            },
            &warehouses,
        ))
    }

    /// 單身輸入：倉庫有效、儲位屬於該倉庫（IS3(a)(b)）、最高存量（IS3(d)）
    async fn storage_of(
        &self,
        plant: &PlantId,
        item: &ItemNo,
        input: &ItemStorageInput,
    ) -> Result<ItemStorage, AppError> {
        let (warehouse, location) = self
            .place(plant, &input.warehouse_code, &input.location_code)
            .await?;
        Ok(ItemStorage::new(
            item.clone(),
            warehouse,
            location,
            input.max_qty,
        )?)
    }

    async fn place(
        &self,
        plant: &PlantId,
        warehouse: &str,
        location: &str,
    ) -> Result<(WarehouseCode, Option<LocationCode>), AppError> {
        let warehouse = WarehouseCode::parse(warehouse)?;
        let location = item_storage::parse_location(location)?;
        check_storage(
            self.warehouses.as_ref(),
            plant,
            Some(warehouse.as_str()),
            location.as_ref().map(|l| l.as_str()),
        )
        .await?;
        Ok((warehouse, location))
    }

    async fn load(
        &self,
        plant: &PlantId,
        item: &ItemNo,
        (warehouse, location): (&str, &str),
    ) -> Result<ItemStorage, AppError> {
        self.storages
            .list_item_storages(plant, Some(item))
            .await
            .map_err(infra)?
            .into_iter()
            .find(|s| {
                s.warehouse.as_str() == warehouse.trim() && s.location_code() == location.trim()
            })
            .ok_or_else(|| DomainError::ItemStorageNotFound.into())
    }

    async fn item(&self, plant: &PlantId, item: &str) -> Result<Item, AppError> {
        let no = ItemNo::parse(item).map_err(|_| DomainError::ItemNotFound)?;
        self.items
            .get_item(plant, &no)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::ItemNotFound.into())
    }

    /// 料件須存在且有效（aimi202.4gl:449-456 mfg1000；IS4）
    async fn active_item(&self, plant: &PlantId, item: &str) -> Result<Item, AppError> {
        let it = self.item(plant, item).await?;
        if it.active_code == ActiveCode::Invalid {
            return Err(DomainError::ItemInactive.into());
        }
        Ok(it)
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn sorted(mut v: Vec<ItemStorage>) -> Vec<ItemStorage> {
    v.sort_by(|a, b| {
        (a.warehouse.as_str(), a.location_code()).cmp(&(b.warehouse.as_str(), b.location_code()))
    });
    v
}

fn to_dto(v: &View, warehouses: &[Warehouse]) -> ItemStorageItemDto {
    let a = &v.item.attrs;
    ItemStorageItemDto {
        item_no: v.item.no.as_str().to_owned(),
        name: a.name.clone(),
        spec: a.spec.clone(),
        current_version: a.current_version.clone(),
        source_code: a.source_code.clone(),
        stock_unit: a.stock_unit.clone(),
        active: v.item.active_code != ActiveCode::Invalid,
        storages: v
            .storages
            .iter()
            .map(|s| {
                let w = warehouses.iter().find(|w| w.code == s.warehouse);
                ItemStorageDto {
                    warehouse_code: s.warehouse.as_str().to_owned(),
                    warehouse_name: w.and_then(|w| w.attrs.name.clone()),
                    location_code: s.location_code().to_owned(),
                    location_name: s.location.as_ref().and_then(|l| {
                        w.and_then(|w| w.locations.iter().find(|x| &x.code == l))
                            .and_then(|x| x.name.clone())
                    }),
                    max_qty: s.max_qty.map(|q| q.normalize()),
                    version: s.stamp.version,
                    created_at: s.stamp.created_at,
                    updated_at: s.stamp.updated_at,
                }
            })
            .collect(),
    }
}

fn conflict(e: RepoError) -> AppError {
    match e {
        RepoError::Conflict => DomainError::ItemStorageAlreadyExists.into(),
        e => infra(e),
    }
}

fn infra(e: RepoError) -> AppError {
    match e {
        RepoError::Stale => DomainError::RecordChanged.into(),
        e => AppError::Infra(e.to_string()),
    }
}
