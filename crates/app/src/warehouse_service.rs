//! Warehouse Maintenance 倉庫儲位維護 use case（TipTop aimi201 / imd_file 倉庫、ime_file 儲位；aimi200 併入）
//! 規格：specs/features/aim/warehouse.feature（WH1–WH13）
//!
//! 工廠層（WH2）。功能代號沿用 TipTop：insert、query、modify、delete、invalid、reproduce、detail（儲位）、output、exporttoexcel，
//! 加上 confirm、notconfirm、history（WH8，ADR 0013）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    department::{ControlType, DeptCode},
    plant::PlantId,
    warehouse::{
        AccountMethod, Location, LocationCode, StorageClass, StorageFlags, Warehouse,
        WarehouseAttrs, WarehouseCode,
    },
};
use ttrs_shared::{
    ChangeDto, Page, QbeCondition, QueryRequest, WarehouseDto, WarehouseInput,
    WarehouseLocationDto, WarehouseLocationInput,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    change_log::HistoryReader,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{DepartmentRepository, MasterDataRepository, WarehouseRepository},
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "warehouse-maintenance";

/// 清單報表（WH8）：每個儲位一列，沒有儲位的倉庫印一列
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "warehouse-report-title",
    columns: &[
        ("warehouse_code", "warehouse-col-code"),
        ("name", "warehouse-col-name"),
        ("storage_class", "warehouse-col-class"),
        ("usable", "warehouse-col-usable"),
        ("mrp_usable", "warehouse-col-mrp"),
        ("bonded", "warehouse-col-bonded"),
        ("location_code", "warehouse-col-location"),
        ("location_name", "warehouse-col-location-name"),
    ],
};

#[derive(Clone)]
pub struct WarehouseService {
    authz: Arc<dyn Authorizer>,
    warehouses: Arc<dyn WarehouseRepository>,
    depts: Arc<dyn DepartmentRepository>,
    master: Arc<dyn MasterDataRepository>,
    history: HistoryReader,
    reports: ListReporter,
}

impl WarehouseService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        warehouses: Arc<dyn WarehouseRepository>,
        depts: Arc<dyn DepartmentRepository>,
        master: Arc<dyn MasterDataRepository>,
        history: HistoryReader,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            warehouses,
            depts,
            master,
            history,
            reports,
        }
    }

    async fn sorted(&self, plant: &PlantId) -> Result<Vec<Warehouse>, AppError> {
        let mut all = self
            .warehouses
            .list_warehouses(plant)
            .await
            .map_err(infra)?;
        all.sort_by(|a, b| a.code.cmp(&b.code));
        Ok(all)
    }

    /// QBE 查詢（aimi201.4gl:189-349）：倉庫與儲位欄位都可當條件，儲位符合任一列即列出；依倉庫別排序
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<WarehouseDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let filters = query::warehouse::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.sorted(&plant).await?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::warehouse::value, p);
        let mut items = Vec::with_capacity(rows.len());
        for w in &rows {
            items.push(self.to_dto(&plant, w).await?);
        }
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
        code: &str,
    ) -> Result<WarehouseDto, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let w = self.load(&plant, code).await?;
        self.to_dto(&plant, &w).await
    }

    /// 列印 output、匯出 Excel exporttoexcel（WH8）
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
        let filters = query::warehouse::filters(&conditions)?;
        let all = self.sorted(&plant).await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };
        let (rows, _) = query::page_in_memory(&all, &filters, query::warehouse::value, p);
        let t = |id: &str| report::text(lang, id);
        let yn = |v: bool| t(if v { "report-yes" } else { "report-no" });
        let mut lines = Vec::new();
        for w in &rows {
            let f = &w.attrs.flags;
            let head = vec![
                w.code.as_str().to_owned(),
                w.attrs.name.clone().unwrap_or_default(),
                t(&format!("warehouse-class-{}", f.class.code())),
                yn(f.usable),
                yn(f.mrp_usable),
                yn(f.bonded),
            ];
            if w.locations.is_empty() {
                let mut l = head.clone();
                l.extend([String::new(), String::new()]);
                lines.push(l);
            }
            for loc in &w.locations {
                let mut l = head.clone();
                l.extend([
                    loc.code.as_str().to_owned(),
                    loc.name.clone().unwrap_or_default(),
                ]);
                lines.push(l);
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

    /// 新增（insert；aimi201.4gl:427-560）：資料有效、優先順序 0；倉庫別不可重複（-239）
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        input: WarehouseInput,
    ) -> Result<WarehouseDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let plant = PlantId::parse(plant)?;
            if !self.master.plant_active(&plant).await.map_err(infra)? {
                return Err(DomainError::PlantNotFound.into());
            }
            let code = WarehouseCode::parse(&input.warehouse_code)?;
            let attrs = self.attrs(&plant, input, None).await?;
            let w = Warehouse::new(code, attrs, &actor.user_id, &actor.dept, today());
            self.insert(&plant, &w).await
        })
        .await
    }

    /// 修改（modify；:834-1066）：停用的不可修改（mfg1000）；設定同步到儲位（WH5）
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        input: WarehouseInput,
    ) -> Result<WarehouseDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            self.authorize(actor, "modify").await?;
            let plant = PlantId::parse(plant)?;
            let mut w = self.load(&plant, code).await?;
            w.stamp.check(input.version)?;
            w.check_active()?;
            let attrs = self.attrs(&plant, input, Some(&w.attrs.flags)).await?;
            w.update(attrs, &actor.user_id, today())?;
            self.save(&plant, &w).await
        })
        .await
    }

    /// 新增儲位（detail；:1680-1830）：未給的設定從倉庫帶入
    pub async fn add_location(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        input: WarehouseLocationInput,
    ) -> Result<WarehouseDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let plant = PlantId::parse(plant)?;
            let mut w = self.load(&plant, code).await?;
            w.stamp.check(input.version)?;
            w.check_active()?;
            let loc = location_of(&input, &w.default_flags())?;
            w.add_location(loc, &actor.user_id, today())?;
            self.save(&plant, &w).await
        })
        .await
    }

    /// 修改儲位（detail）：儲位代號不可改（WH4(e)）
    pub async fn update_location(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        input: WarehouseLocationInput,
    ) -> Result<WarehouseDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let mut w = self.load(&plant, code).await?;
            w.stamp.check(input.version)?;
            let current = w
                .locations
                .iter()
                .find(|l| l.code.as_str() == input.location_code.trim())
                .map(|l| l.flags.clone())
                .ok_or(DomainError::WarehouseLocationNotFound)?;
            let loc = location_of(&input, &current)?;
            w.update_location(loc, &actor.user_id, today())?;
            self.save(&plant, &w).await
        })
        .await
    }

    /// 刪除儲位（detail）：被料件、料件分群使用時不可刪除（WH7(b)）
    pub async fn remove_location(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        location: &str,
        version: Option<i32>,
    ) -> Result<WarehouseDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let plant = PlantId::parse(plant)?;
            let mut w = self.load(&plant, code).await?;
            w.stamp.check(version)?;
            let loc = LocationCode::parse(location)
                .map_err(|_| DomainError::WarehouseLocationNotFound)?;
            if self
                .warehouses
                .warehouse_in_use(&plant, &w.code, Some(&loc))
                .await
                .map_err(infra)?
            {
                return Err(DomainError::WarehouseLocationInUse.into());
            }
            w.remove_location(&loc, &actor.user_id, today())?;
            self.save(&plant, &w).await
        })
        .await
    }

    /// 停用／啟用（invalid；:1193-1310）
    pub async fn set_active(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<WarehouseDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "invalid", async {
            self.authorize(actor, "invalid").await?;
            let plant = PlantId::parse(plant)?;
            let mut w = self.load(&plant, code).await?;
            w.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            w.set_active(active, &actor.user_id, today());
            self.save(&plant, &w).await
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<WarehouseDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let plant = PlantId::parse(plant)?;
            let mut w = self.load(&plant, code).await?;
            w.stamp.check(version)?;
            w.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&plant, &w).await
        })
        .await
    }

    /// 複製倉庫與儲位（reproduce；:2554-2643）
    pub async fn copy(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        new_code: &str,
    ) -> Result<WarehouseDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let plant = PlantId::parse(plant)?;
            let source = self.load(&plant, code).await?;
            let w = source.copy_as(
                WarehouseCode::parse(new_code)?,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            self.insert(&plant, &w).await
        })
        .await
    }

    /// 刪除連同儲位（delete；:1311-1439）：停用、已確認不可刪；被料件、料件分群使用時不可刪（WH7）
    pub async fn delete(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            self.authorize(actor, "delete").await?;
            let plant = PlantId::parse(plant)?;
            let w = self.load(&plant, code).await?;
            w.stamp.check(version)?;
            w.check_deletable()?;
            match self.warehouses.delete_warehouse(&plant, &w.code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::WarehouseNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::WarehouseInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 異動記錄（ADR 0012 §8，功能 history）：倉庫與儲位一併列出
    pub async fn history(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        self.authorize(actor, "history").await?;
        let plant = PlantId::parse(plant)?;
        let code = WarehouseCode::parse(code).map_err(|_| DomainError::WarehouseNotFound)?;
        self.history
            .read(
                Some(&plant),
                &["warehouse", "warehouse_location"],
                &[("warehouse_code", code.as_str())],
                lang,
            )
            .await
    }

    /// 倉庫欄位：新增時旗標未給為預設（可用、MRP 可用、非保稅、0）；修改時未給沿用原值；成本中心檢查（WH3(f)）
    async fn attrs(
        &self,
        plant: &PlantId,
        i: WarehouseInput,
        current: Option<&StorageFlags>,
    ) -> Result<WarehouseAttrs, AppError> {
        let class = StorageClass::parse(&i.storage_class)?;
        let flags = StorageFlags::new(
            class,
            i.usable.or(current.map(|f| f.usable)).unwrap_or(true),
            i.mrp_usable
                .or(current.map(|f| f.mrp_usable))
                .unwrap_or(true),
            i.bonded.or(current.map(|f| f.bonded)).unwrap_or(false),
            i.issue_priority
                .or(current.map(|f| f.issue_priority))
                .unwrap_or(0),
            i.ship_priority
                .or(current.map(|f| f.ship_priority))
                .unwrap_or(0),
        )?;
        let attrs = WarehouseAttrs::new(
            i.name,
            i.address,
            i.contact,
            i.phone,
            i.account,
            i.account2,
            i.cost_group,
            i.cost_center,
            flags,
        )?;
        if let Some(cc) = &attrs.cost_center {
            let ok = self
                .depts
                .get(plant, cc)
                .await
                .map_err(infra)?
                .is_some_and(|d| {
                    d.active
                        && matches!(
                            d.accounting.control_type,
                            ControlType::CostCenter | ControlType::ProfitCenter
                        )
                });
            if !ok {
                return Err(DomainError::WarehouseCostCenterInvalid.into());
            }
        }
        Ok(attrs)
    }

    async fn insert(&self, plant: &PlantId, w: &Warehouse) -> Result<WarehouseDto, AppError> {
        match self.warehouses.insert_warehouse(plant, w).await {
            Ok(()) => {
                let saved = self.load(plant, w.code.as_str()).await?;
                self.to_dto(plant, &saved).await
            }
            Err(RepoError::Conflict) => Err(DomainError::WarehouseAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, plant: &PlantId, w: &Warehouse) -> Result<WarehouseDto, AppError> {
        self.warehouses
            .update_warehouse(plant, w)
            .await
            .map_err(infra)?;
        let saved = self.load(plant, w.code.as_str()).await?;
        self.to_dto(plant, &saved).await
    }

    async fn load(&self, plant: &PlantId, code: &str) -> Result<Warehouse, AppError> {
        let code = WarehouseCode::parse(code).map_err(|_| DomainError::WarehouseNotFound)?;
        self.warehouses
            .get_warehouse(plant, &code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::WarehouseNotFound.into())
    }

    async fn to_dto(&self, plant: &PlantId, w: &Warehouse) -> Result<WarehouseDto, AppError> {
        let a = &w.attrs;
        let f = &a.flags;
        let cost_center_name = match &a.cost_center {
            Some(cc) => self.dept_name(plant, cc).await?,
            None => None,
        };
        Ok(WarehouseDto {
            warehouse_code: w.code.as_str().to_owned(),
            name: a.name.clone(),
            address: a.address.clone(),
            contact: a.contact.clone(),
            phone: a.phone.clone(),
            account: a.account.clone(),
            account2: a.account2.clone(),
            cost_group: a.cost_group.clone(),
            cost_center: a.cost_center.as_ref().map(|d| d.as_str().to_owned()),
            cost_center_name,
            storage_class: f.class.code().to_owned(),
            usable: f.usable,
            mrp_usable: f.mrp_usable,
            bonded: f.bonded,
            issue_priority: f.issue_priority,
            ship_priority: f.ship_priority,
            locations: w
                .locations
                .iter()
                .map(|l| WarehouseLocationDto {
                    location_code: l.code.as_str().to_owned(),
                    name: l.name.clone(),
                    storage_class: l.flags.class.code().to_owned(),
                    usable: l.flags.usable,
                    mrp_usable: l.flags.mrp_usable,
                    bonded: l.flags.bonded,
                    issue_priority: l.flags.issue_priority,
                    ship_priority: l.flags.ship_priority,
                    account_method: l.account_method.code().to_owned(),
                    account: l.account.clone(),
                    account2: l.account2.clone(),
                })
                .collect(),
            active: w.active,
            confirmed: w.confirmation.confirmed,
            confirmed_by: w.confirmation.by.clone(),
            confirmed_at: w.confirmation.at,
            created_by: w.created_by.clone(),
            created_dept: w.created_dept.clone(),
            created_on: w.created_on,
            updated_by: w.updated_by.clone(),
            updated_on: w.updated_on,
            version: w.stamp.version,
            created_at: w.stamp.created_at,
            updated_at: w.stamp.updated_at,
        })
    }

    async fn dept_name(
        &self,
        plant: &PlantId,
        code: &DeptCode,
    ) -> Result<Option<String>, AppError> {
        Ok(self
            .depts
            .get(plant, code)
            .await
            .map_err(infra)?
            .map(|d| d.attrs.name))
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

/// 儲位輸入：未給的設定沿用 `base`（新增時為倉庫的設定，修改時為儲位原值）
fn location_of(i: &WarehouseLocationInput, base: &StorageFlags) -> Result<Location, AppError> {
    let class = match &i.storage_class {
        Some(c) => StorageClass::parse(c)?,
        None => base.class,
    };
    let usable = i.usable.unwrap_or(base.usable);
    // 倉庫帶入的 MRP 可用在不可用時一律為否（WH4(c)）
    let mrp = i.mrp_usable.unwrap_or(base.mrp_usable && usable);
    let flags = StorageFlags::new(
        class,
        usable,
        mrp,
        i.bonded.unwrap_or(base.bonded),
        i.issue_priority.unwrap_or(base.issue_priority),
        i.ship_priority.unwrap_or(base.ship_priority),
    )?;
    Ok(Location::new(
        LocationCode::parse(&i.location_code)?,
        i.name.clone(),
        flags,
        AccountMethod::parse(i.account_method.as_deref().unwrap_or_default())?,
        i.account.clone(),
        i.account2.clone(),
    )?)
}

/// 料件、料件分群的主要倉庫與儲位（WH9）：倉庫須有效（mfg1100）、儲位須為該倉庫的儲位（mfg1101）；只填儲位時擋下
pub async fn check_storage(
    warehouses: &dyn WarehouseRepository,
    plant: &PlantId,
    warehouse: Option<&str>,
    location: Option<&str>,
) -> Result<(), AppError> {
    let warehouse = warehouse.map(str::trim).filter(|s| !s.is_empty());
    let location = location.map(str::trim).filter(|s| !s.is_empty());
    let Some(code) = warehouse else {
        return match location {
            Some(_) => Err(DomainError::WarehouseNotFound.into()),
            None => Ok(()),
        };
    };
    let w = match WarehouseCode::parse(code) {
        Ok(c) => warehouses.get_warehouse(plant, &c).await.map_err(infra)?,
        Err(_) => None,
    };
    let Some(w) = w.filter(|w| w.active) else {
        return Err(DomainError::WarehouseNotFound.into());
    };
    if let Some(loc) = location
        && !w.locations.iter().any(|l| l.code.as_str() == loc)
    {
        return Err(DomainError::WarehouseLocationNotFound.into());
    }
    Ok(())
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
