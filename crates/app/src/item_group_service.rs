//! Item Group Maintenance 料件分群維護 use case（TipTop: aimi110 / imz_file）
//! 規格：specs/features/aim/item_group.feature（IG1–IG12）
//!
//! 工廠層（IG2）。單筆維護：查詢 query、新增 insert、修改 modify、無效 invalid（停用／啟用）、刪除 delete、
//! 複製 reproduce、異動記錄 history（aimi110 的功能代號）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    classification::{Code, CodeKind},
    employee::EmployeeId,
    item_group::{ItemGroup, ItemGroupAttrs, ItemGroupCode},
    plant::PlantId,
    unit::UnitCode,
};
use ttrs_shared::{ChangeDto, ItemGroupDto, ItemGroupInput};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    change_log::HistoryReader,
    error::RepoError,
    ports::{
        ClassificationCodeRepository, EmployeeRepository, ItemGroupRepository,
        MasterDataRepository, UnitRepository,
    },
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "item-group-maintenance";

/// 其他分群碼一～四對應的分類碼類別（IG4）
const OTHER_GROUP_KINDS: [&str; 4] = ["D", "E", "F", "G"];

#[derive(Clone)]
pub struct ItemGroupService {
    authz: Arc<dyn Authorizer>,
    groups: Arc<dyn ItemGroupRepository>,
    units: Arc<dyn UnitRepository>,
    codes: Arc<dyn ClassificationCodeRepository>,
    employees: Arc<dyn EmployeeRepository>,
    master: Arc<dyn MasterDataRepository>,
    history: HistoryReader,
}

impl ItemGroupService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        groups: Arc<dyn ItemGroupRepository>,
        units: Arc<dyn UnitRepository>,
        codes: Arc<dyn ClassificationCodeRepository>,
        employees: Arc<dyn EmployeeRepository>,
        master: Arc<dyn MasterDataRepository>,
        history: HistoryReader,
    ) -> Self {
        Self {
            authz,
            groups,
            units,
            codes,
            employees,
            master,
            history,
        }
    }

    pub async fn list(&self, actor: &Actor, plant: &str) -> Result<Vec<ItemGroupDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let mut out = Vec::new();
        for g in self.groups.list_item_groups(&plant).await.map_err(infra)? {
            out.push(self.to_dto(&plant, &g).await?);
        }
        Ok(out)
    }

    pub async fn get(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
    ) -> Result<ItemGroupDto, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let g = self.load(&plant, code).await?;
        self.to_dto(&plant, &g).await
    }

    /// 新增（aimi110.4gl:843-866）：分群碼不可重複（-239）
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        input: ItemGroupInput,
    ) -> Result<ItemGroupDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let plant = PlantId::parse(plant)?;
            if !self.master.plant_active(&plant).await.map_err(infra)? {
                return Err(DomainError::PlantNotFound.into());
            }
            let code = ItemGroupCode::parse(&input.group_code)?;
            let attrs = self.attrs(&plant, input).await?;
            let g = ItemGroup::new(code, attrs, &actor.user_id, &actor.dept, today());
            self.insert(&plant, &g).await
        })
        .await
    }

    /// 修改：分群碼建立後不可修改
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        input: ItemGroupInput,
    ) -> Result<ItemGroupDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            let grant = self.authorize(actor, "modify").await?;
            let plant = PlantId::parse(plant)?;
            let mut g = self.load(&plant, code).await?;
            grant.record(&g.created_by, &g.created_dept)?;
            g.stamp.check(input.version)?;
            let attrs = self.attrs(&plant, input).await?;
            g.update(attrs, &actor.user_id, today());
            self.save(&plant, &g).await
        })
        .await
    }

    /// 無效／有效（aimi110_x，IG8）
    pub async fn set_active(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<ItemGroupDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "invalid", async {
            let grant = self.authorize(actor, "invalid").await?;
            let plant = PlantId::parse(plant)?;
            let mut g = self.load(&plant, code).await?;
            grant.record(&g.created_by, &g.created_dept)?;
            g.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            g.set_active(active, &actor.user_id, today());
            self.save(&plant, &g).await
        })
        .await
    }

    /// 複製（aimi110_copy，IG9）
    pub async fn copy(
        &self,
        actor: &Actor,
        plant: &str,
        from: &str,
        to: &str,
    ) -> Result<ItemGroupDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let plant = PlantId::parse(plant)?;
            let g = self.load(&plant, from).await?;
            let to = ItemGroupCode::parse(to)?;
            let copy = g.copy_as(to, &actor.user_id, &actor.dept, today());
            self.insert(&plant, &copy).await
        })
        .await
    }

    /// 刪除（aimi110_r，:2600-2635）：已有料件引用時拒絕（mfg9045，IG7）
    pub async fn delete(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            let grant = self.authorize(actor, "delete").await?;
            let plant = PlantId::parse(plant)?;
            let g = self.load(&plant, code).await?;
            grant.record(&g.created_by, &g.created_dept)?;
            g.stamp.check(version)?;
            match self.groups.delete_item_group(&plant, &g.code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::ItemGroupNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::ItemGroupInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 異動記錄（ADR 0012 §8，功能 history）
    pub async fn history(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        self.authorize(actor, "history").await?;
        let plant = PlantId::parse(plant)?;
        let code = ItemGroupCode::parse(code).map_err(|_| DomainError::ItemGroupNotFound)?;
        self.history
            .read(
                Some(&plant),
                &["item_group"],
                &[("group_code", code.as_str())],
                lang,
            )
            .await
    }

    /// 欄位檢查（IG4、IG5）：值域與長度在 domain；引用的單位、分類碼、員工在此檢查
    async fn attrs(&self, plant: &PlantId, i: ItemGroupInput) -> Result<ItemGroupAttrs, AppError> {
        let attrs = ItemGroupAttrs {
            description: i.description,
            source_code: i.source_code,
            stock_unit: i.stock_unit,
            catalog_ref: i.catalog_ref,
            supplier_control: i.supplier_control.unwrap_or_default(),
            drawing_no: i.drawing_no,
            software: i.software,
            engineering: i.engineering,
            insert_qpa_check: i.insert_qpa_check,
            insert_position: i.insert_position,
            material_category: i.material_category,
            coproduct: i.coproduct,
            inspection: i.inspection,
            repetitive: i.repetitive,
            abc_code: i.abc_code,
            consumable: i.consumable,
            replenish_policy: i.replenish_policy,
            reorder_point: i.reorder_point,
            reorder_qty: i.reorder_qty,
            safety_stock: i.safety_stock,
            safety_period: i.safety_period,
            shelf_life_days: i.shelf_life_days,
            mrp_bucket_days: i.mrp_bucket_days,
            warehouse: i.warehouse,
            location: i.location,
            stock_keeper: i.stock_keeper,
            sales_stat_unit: i.sales_stat_unit,
            second_unit: i.second_unit,
            pricing_unit: i.pricing_unit,
            other_groups: [
                i.other_group1,
                i.other_group2,
                i.other_group3,
                i.other_group4,
            ],
            account: i.account,
            account2: i.account2,
            bonded: i.bonded,
            bonded_class: i.bonded_class,
            tariff_no: i.tariff_no,
            bonded_type: i.bonded_type,
        }
        .validated()?;
        // 單位：須為有效的單位（:1505-1556）
        for u in attrs.units() {
            let ok = match UnitCode::parse(u) {
                Ok(code) => self
                    .units
                    .get_unit(&code)
                    .await
                    .map_err(infra)?
                    .is_some_and(|x| x.active),
                Err(_) => false,
            };
            if !ok {
                return Err(DomainError::UnitNotFound.into());
            }
        }
        // 其他分群碼一～四（:1031-1100，mfg1306）、材料類別（類別 8）
        for (kind, code) in OTHER_GROUP_KINDS
            .iter()
            .zip(attrs.other_groups.iter())
            .filter_map(|(k, c)| c.as_deref().map(|c| (*k, c)))
            .chain(attrs.material_category.as_deref().map(|c| ("8", c)))
        {
            self.check_code(plant, kind, code).await?;
        }
        // 倉管員（:1489-1503，mfg3096）
        if let Some(e) = &attrs.stock_keeper {
            let ok = match EmployeeId::parse(e) {
                Ok(id) => self
                    .employees
                    .get(plant, &id)
                    .await
                    .map_err(infra)?
                    .is_some_and(|x| x.active),
                Err(_) => false,
            };
            if !ok {
                return Err(DomainError::ItemGroupStockKeeperInvalid.into());
            }
        }
        Ok(attrs)
    }

    async fn check_code(&self, plant: &PlantId, kind: &str, code: &str) -> Result<(), AppError> {
        let kind = CodeKind::parse(kind)?;
        let ok = match Code::parse(code) {
            Ok(c) => self
                .codes
                .get_code(plant, kind, &c)
                .await
                .map_err(infra)?
                .is_some_and(|x| x.active),
            Err(_) => false,
        };
        if ok {
            Ok(())
        } else {
            Err(DomainError::CodeNotFound.into())
        }
    }

    async fn insert(&self, plant: &PlantId, g: &ItemGroup) -> Result<ItemGroupDto, AppError> {
        match self.groups.insert_item_group(plant, g).await {
            Ok(()) => {
                let saved = self.load(plant, g.code.as_str()).await?;
                self.to_dto(plant, &saved).await
            }
            Err(RepoError::Conflict) => Err(DomainError::ItemGroupAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, plant: &PlantId, g: &ItemGroup) -> Result<ItemGroupDto, AppError> {
        self.groups
            .update_item_group(plant, g)
            .await
            .map_err(infra)?;
        let saved = self.load(plant, g.code.as_str()).await?;
        self.to_dto(plant, &saved).await
    }

    async fn load(&self, plant: &PlantId, code: &str) -> Result<ItemGroup, AppError> {
        let code = ItemGroupCode::parse(code).map_err(|_| DomainError::ItemGroupNotFound)?;
        self.groups
            .get_item_group(plant, &code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::ItemGroupNotFound.into())
    }

    async fn to_dto(&self, plant: &PlantId, g: &ItemGroup) -> Result<ItemGroupDto, AppError> {
        let a = &g.attrs;
        let stock_keeper_name = match a.stock_keeper.as_deref().map(EmployeeId::parse) {
            Some(Ok(id)) => self
                .employees
                .get(plant, &id)
                .await
                .map_err(infra)?
                .map(|e| e.attrs.name),
            _ => None,
        };
        let [g1, g2, g3, g4] = a.other_groups.clone();
        Ok(ItemGroupDto {
            fields: ItemGroupInput {
                group_code: g.code.as_str().to_owned(),
                description: a.description.clone(),
                source_code: a.source_code.clone(),
                stock_unit: a.stock_unit.clone(),
                catalog_ref: a.catalog_ref.clone(),
                supplier_control: Some(a.supplier_control.clone()),
                drawing_no: a.drawing_no.clone(),
                software: a.software,
                engineering: a.engineering,
                insert_qpa_check: a.insert_qpa_check,
                insert_position: a.insert_position,
                material_category: a.material_category.clone(),
                coproduct: a.coproduct,
                inspection: a.inspection,
                repetitive: a.repetitive,
                abc_code: a.abc_code.clone(),
                consumable: a.consumable,
                replenish_policy: a.replenish_policy.clone(),
                reorder_point: a.reorder_point,
                reorder_qty: a.reorder_qty,
                safety_stock: a.safety_stock,
                safety_period: a.safety_period,
                shelf_life_days: a.shelf_life_days,
                mrp_bucket_days: a.mrp_bucket_days,
                warehouse: a.warehouse.clone(),
                location: a.location.clone(),
                stock_keeper: a.stock_keeper.clone(),
                sales_stat_unit: a.sales_stat_unit.clone(),
                second_unit: a.second_unit.clone(),
                pricing_unit: a.pricing_unit.clone(),
                other_group1: g1,
                other_group2: g2,
                other_group3: g3,
                other_group4: g4,
                account: a.account.clone(),
                account2: a.account2.clone(),
                bonded: a.bonded,
                bonded_class: a.bonded_class.clone(),
                tariff_no: a.tariff_no.clone(),
                bonded_type: a.bonded_type.clone(),
                version: Some(g.stamp.version),
            },
            stock_keeper_name,
            active: g.active,
            created_by: g.created_by.clone(),
            created_dept: g.created_dept.clone(),
            created_on: g.created_on,
            updated_by: g.updated_by.clone(),
            updated_on: g.updated_on,
            version: g.stamp.version,
            created_at: g.stamp.created_at,
            updated_at: g.stamp.updated_at,
        })
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn infra(e: RepoError) -> AppError {
    match e {
        RepoError::Stale => DomainError::RecordChanged.into(),
        e => AppError::Infra(e.to_string()),
    }
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}
