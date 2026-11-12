//! 單位換算 use case（TipTop: aooi102 / smc_file、aooi103 / smd_file、換算率 s_umfchk）
//! 規格：specs/features/aoo/unit_conversion.feature（UC1–UC12）
//!
//! 一般換算共用（UNIT_CONVERSION_PROGRAM），料件換算在工廠層（ITEM_PROGRAM）；一組單位只存一筆，不分方向（UC3）。

use std::sync::Arc;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use ttrs_domain::{
    DomainError,
    item::{ActiveCode, ItemNo},
    plant::PlantId,
    unit::UnitCode,
    unit_conversion::{self, Conversion, ItemUnitConversion, UnitConversion},
};
use ttrs_shared::{
    ChangeDto, ItemUnitConversionDto, Page, QbeCondition, QueryRequest, UnitConversionDto,
    UnitConversionInput,
};

use crate::{
    Actor, AppError, Authorizer,
    change_log::HistoryReader,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{
        ItemRepository, ItemUnitConversionRepository, UnitConversionRepository, UnitRepository,
    },
    query,
    report::{ReportData, ReportFormat},
};

/// 一般換算（aooi102）
pub const UNIT_CONVERSION_PROGRAM: &str = "unit-conversion-maintenance";
/// 料件換算（aooi103）
pub const ITEM_PROGRAM: &str = "item-unit-conversion-maintenance";

const REPORT: ListSpec = ListSpec {
    program: UNIT_CONVERSION_PROGRAM,
    title: "conversion-report-title",
    columns: &[
        ("from_unit", "conversion-col-from-unit"),
        ("from_quantity", "conversion-col-from-quantity"),
        ("to_unit", "conversion-col-to-unit"),
        ("to_quantity", "conversion-col-to-quantity"),
        ("note", "conversion-col-note"),
        ("active", "conversion-col-active"),
    ],
};

const ITEM_REPORT: ListSpec = ListSpec {
    program: ITEM_PROGRAM,
    title: "item-conversion-report-title",
    columns: &[
        ("item_no", "conversion-col-item"),
        ("from_unit", "conversion-col-from-unit"),
        ("from_quantity", "conversion-col-from-quantity"),
        ("to_unit", "conversion-col-to-unit"),
        ("to_quantity", "conversion-col-to-quantity"),
        ("note", "conversion-col-note"),
        ("active", "conversion-col-active"),
    ],
};

/// 兩邊單位須存在且有效（UC5；mfg2605、aoo-012）
async fn check_units(
    units: &dyn UnitRepository,
    from: &UnitCode,
    to: &UnitCode,
) -> Result<(), AppError> {
    for u in [from, to] {
        let unit = units
            .get_unit(u)
            .await
            .map_err(infra)?
            .ok_or(DomainError::UnitNotFound)?;
        if !unit.active {
            return Err(DomainError::UnitInactive.into());
        }
    }
    Ok(())
}

fn conversion_of(input: &UnitConversionInput) -> Result<Conversion, DomainError> {
    Conversion::new(
        UnitCode::parse(&input.from_unit)?,
        UnitCode::parse(&input.to_unit)?,
        input.from_quantity,
        input.to_quantity,
        input.note.clone(),
    )
}

fn yes_no(v: bool) -> String {
    if v { "Y" } else { "N" }.to_owned()
}

// ----- 一般換算（aooi102） -----

#[derive(Clone)]
pub struct UnitConversionService {
    authz: Arc<dyn Authorizer>,
    convs: Arc<dyn UnitConversionRepository>,
    units: Arc<dyn UnitRepository>,
    reports: ListReporter,
}

impl UnitConversionService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        convs: Arc<dyn UnitConversionRepository>,
        units: Arc<dyn UnitRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            convs,
            units,
            reports,
        }
    }

    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<UnitConversionDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::unit_conversion::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.convs.list_unit_conversions().await.map_err(infra)?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::unit_conversion::value, p);
        Ok(Page {
            items: rows.iter().map(dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 一組換算（不分方向）
    pub async fn get(
        &self,
        actor: &Actor,
        from: &str,
        to: &str,
    ) -> Result<UnitConversionDto, AppError> {
        self.authorize(actor, "query").await?;
        Ok(dto(&self.load(from, to).await?))
    }

    /// 新增（aooi102.4gl:300-437）：單位須存在且有效、同一組不可重複（不分方向，UC3）
    pub async fn create(
        &self,
        actor: &Actor,
        input: UnitConversionInput,
    ) -> Result<UnitConversionDto, AppError> {
        crate::change_log::run(actor, UNIT_CONVERSION_PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let c = conversion_of(&input)?;
            self.insert(actor, c).await
        })
        .await
    }

    /// 修改數量與說明（單位不可變更）
    pub async fn update(
        &self,
        actor: &Actor,
        from: &str,
        to: &str,
        input: UnitConversionInput,
    ) -> Result<UnitConversionDto, AppError> {
        crate::change_log::run(actor, UNIT_CONVERSION_PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut cur = self.load(from, to).await?;
            cur.stamp.check(input.version)?;
            cur.conversion = Conversion::new(
                cur.conversion.from_unit.clone(),
                cur.conversion.to_unit.clone(),
                input.from_quantity,
                input.to_quantity,
                input.note.clone(),
            )?;
            cur.touch(&actor.user_id, today());
            self.save(&cur).await
        })
        .await
    }

    /// 刪除（UC6：不沿用「單位被料件使用就不可刪」）；已確認不可刪（SA9）
    pub async fn delete(
        &self,
        actor: &Actor,
        from: &str,
        to: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, UNIT_CONVERSION_PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let cur = self.load(from, to).await?;
            cur.stamp.check(version)?;
            cur.confirmation.check_deletable()?;
            let c = &cur.conversion;
            self.convs
                .delete_unit_conversion(&c.from_unit, &c.to_unit)
                .await
                .map_err(infra)?;
            Ok(())
        })
        .await
    }

    /// 停用／啟用（smcacti）
    pub async fn set_active(
        &self,
        actor: &Actor,
        from: &str,
        to: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<UnitConversionDto, AppError> {
        crate::change_log::run(actor, UNIT_CONVERSION_PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut cur = self.load(from, to).await?;
            cur.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            cur.active = active;
            cur.touch(&actor.user_id, today());
            self.save(&cur).await
        })
        .await
    }

    /// 確認／取消確認（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        from: &str,
        to: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<UnitConversionDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, UNIT_CONVERSION_PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let mut cur = self.load(from, to).await?;
            cur.stamp.check(version)?;
            crate::change_log::set_operation(action);
            if confirm {
                cur.confirmation
                    .confirm(&actor.user_id, chrono::Utc::now())?;
            } else {
                cur.confirmation.unconfirm()?;
            }
            cur.touch(&actor.user_id, today());
            self.save(&cur).await
        })
        .await
    }

    /// 複製為另一組單位（ADR 0013 SA5）：數量與說明帶入，未確認
    pub async fn copy(
        &self,
        actor: &Actor,
        from: &str,
        to: &str,
        new_from: &str,
        new_to: &str,
    ) -> Result<UnitConversionDto, AppError> {
        crate::change_log::run(actor, UNIT_CONVERSION_PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let src = self.load(from, to).await?;
            let c = Conversion::new(
                UnitCode::parse(new_from)?,
                UnitCode::parse(new_to)?,
                src.conversion.from_quantity,
                src.conversion.to_quantity,
                src.conversion.note.clone(),
            )?;
            self.insert(actor, c).await
        })
        .await
    }

    pub async fn report(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::unit_conversion::filters(&conditions)?;
        let all = self.convs.list_unit_conversions().await.map_err(infra)?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::unit_conversion::value, p);
        self.reports.check(total)?;
        let lines = rows
            .iter()
            .map(|c| {
                let v = &c.conversion;
                vec![
                    v.from_unit.as_str().to_owned(),
                    v.from_quantity.normalize().to_string(),
                    v.to_unit.as_str().to_owned(),
                    v.to_quantity.normalize().to_string(),
                    v.note.clone().unwrap_or_default(),
                    yes_no(c.active),
                ]
            })
            .collect();
        self.reports
            .build(actor, None, &REPORT, lines, &conditions, format, lang)
            .await
    }

    async fn insert(&self, actor: &Actor, c: Conversion) -> Result<UnitConversionDto, AppError> {
        check_units(self.units.as_ref(), &c.from_unit, &c.to_unit).await?;
        let all = self.convs.list_unit_conversions().await.map_err(infra)?;
        if all
            .iter()
            .any(|x| x.conversion.same_pair(&c.from_unit, &c.to_unit))
        {
            return Err(DomainError::UnitConversionAlreadyExists.into());
        }
        let row = UnitConversion::new(c, &actor.user_id, &actor.dept, today());
        self.convs
            .insert_unit_conversion(&row)
            .await
            .map_err(|e| match e {
                RepoError::Conflict => DomainError::UnitConversionAlreadyExists.into(),
                e => infra(e),
            })?;
        let c = &row.conversion;
        Ok(dto(&self
            .load(c.from_unit.as_str(), c.to_unit.as_str())
            .await?))
    }

    async fn save(&self, c: &UnitConversion) -> Result<UnitConversionDto, AppError> {
        self.convs.update_unit_conversion(c).await.map_err(infra)?;
        let v = &c.conversion;
        Ok(dto(&self
            .load(v.from_unit.as_str(), v.to_unit.as_str())
            .await?))
    }

    async fn load(&self, from: &str, to: &str) -> Result<UnitConversion, AppError> {
        let (from, to) = (UnitCode::parse(from)?, UnitCode::parse(to)?);
        self.convs
            .list_unit_conversions()
            .await
            .map_err(infra)?
            .into_iter()
            .find(|c| c.conversion.same_pair(&from, &to))
            .ok_or_else(|| DomainError::UnitConversionNotFound.into())
    }

    async fn authorize(
        &self,
        actor: &Actor,
        action: &'static str,
    ) -> Result<crate::authorization::Grant, AppError> {
        self.authz
            .authorize(actor, UNIT_CONVERSION_PROGRAM, action)
            .await
    }
}

fn dto(c: &UnitConversion) -> UnitConversionDto {
    let v = &c.conversion;
    UnitConversionDto {
        from_unit: v.from_unit.as_str().to_owned(),
        to_unit: v.to_unit.as_str().to_owned(),
        from_quantity: v.from_quantity,
        to_quantity: v.to_quantity,
        note: v.note.clone(),
        active: c.active,
        confirmed: c.confirmation.confirmed,
        confirmed_by: c.confirmation.by.clone(),
        confirmed_at: c.confirmation.at,
        created_by: c.created_by.clone(),
        created_dept: c.created_dept.clone(),
        created_on: c.created_on,
        updated_by: c.updated_by.clone(),
        updated_on: c.updated_on,
        version: c.stamp.version,
        created_at: c.stamp.created_at,
        updated_at: c.stamp.updated_at,
    }
}

// ----- 料件換算（aooi103）與換算率（s_umfchk） -----

#[derive(Clone)]
pub struct ItemUnitConversionService {
    authz: Arc<dyn Authorizer>,
    convs: Arc<dyn ItemUnitConversionRepository>,
    general: Arc<dyn UnitConversionRepository>,
    units: Arc<dyn UnitRepository>,
    items: Arc<dyn ItemRepository>,
    history: HistoryReader,
    reports: ListReporter,
}

impl ItemUnitConversionService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        convs: Arc<dyn ItemUnitConversionRepository>,
        general: Arc<dyn UnitConversionRepository>,
        units: Arc<dyn UnitRepository>,
        items: Arc<dyn ItemRepository>,
        history: HistoryReader,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            convs,
            general,
            units,
            items,
            history,
            reports,
        }
    }

    /// 料件的換算（單身）
    pub async fn list(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
    ) -> Result<Vec<ItemUnitConversionDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let item = ItemNo::parse(item)?;
        let all = self
            .convs
            .list_item_unit_conversions(&plant, Some(&item))
            .await
            .map_err(infra)?;
        Ok(all.iter().map(item_dto).collect())
    }

    /// 新增（aooi103.4gl:539-618）：料件須存在且有效（UC8）、單位須存在且有效、同料件同一組不可重複
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
        input: UnitConversionInput,
    ) -> Result<ItemUnitConversionDto, AppError> {
        crate::change_log::run(actor, ITEM_PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let item = self.active_item(&plant, item).await?;
            let c = conversion_of(&input)?;
            check_units(self.units.as_ref(), &c.from_unit, &c.to_unit).await?;
            let all = self
                .convs
                .list_item_unit_conversions(&plant, Some(&item))
                .await
                .map_err(infra)?;
            if all
                .iter()
                .any(|x| x.conversion.same_pair(&c.from_unit, &c.to_unit))
            {
                return Err(DomainError::UnitConversionAlreadyExists.into());
            }
            let row = ItemUnitConversion::new(item, c, &actor.user_id, &actor.dept, today());
            self.convs
                .insert_item_unit_conversion(&plant, &row)
                .await
                .map_err(|e| match e {
                    RepoError::Conflict => DomainError::UnitConversionAlreadyExists.into(),
                    e => infra(e),
                })?;
            Ok(item_dto(&row))
        })
        .await
    }

    /// 修改數量與說明
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
        from: &str,
        to: &str,
        input: UnitConversionInput,
    ) -> Result<ItemUnitConversionDto, AppError> {
        crate::change_log::run(actor, ITEM_PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            self.active_item(&plant, item).await?;
            let mut cur = self.load(&plant, item, from, to).await?;
            cur.stamp.check(input.version)?;
            cur.conversion = Conversion::new(
                cur.conversion.from_unit.clone(),
                cur.conversion.to_unit.clone(),
                input.from_quantity,
                input.to_quantity,
                input.note.clone(),
            )?;
            cur.touch(&actor.user_id, today());
            self.convs
                .update_item_unit_conversion(&plant, &cur)
                .await
                .map_err(infra)?;
            Ok(item_dto(&self.load(&plant, item, from, to).await?))
        })
        .await
    }

    pub async fn delete(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
        from: &str,
        to: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, ITEM_PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let cur = self.load(&plant, item, from, to).await?;
            cur.stamp.check(version)?;
            let c = &cur.conversion;
            self.convs
                .delete_item_unit_conversion(&plant, &cur.item, &c.from_unit, &c.to_unit)
                .await
                .map_err(infra)?;
            Ok(())
        })
        .await
    }

    /// 停用／啟用（smdacti）；鍵值有營運中心、料號、兩個單位
    #[allow(clippy::too_many_arguments)]
    pub async fn set_active(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
        from: &str,
        to: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<ItemUnitConversionDto, AppError> {
        crate::change_log::run(actor, ITEM_PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let mut cur = self.load(&plant, item, from, to).await?;
            cur.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            cur.active = active;
            cur.touch(&actor.user_id, today());
            self.convs
                .update_item_unit_conversion(&plant, &cur)
                .await
                .map_err(infra)?;
            Ok(item_dto(&self.load(&plant, item, from, to).await?))
        })
        .await
    }

    /// 換算率（s_umfchk，UC7）：同單位或 MISC 料件為 1；先找該料件有效的料件換算，再找有效的一般換算（都不分方向）；
    /// 換算率＝目的數量÷來源數量，找不到回 UNIT_CONVERSION_NOT_FOUND
    pub async fn factor(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
        from: &str,
        to: &str,
    ) -> Result<Decimal, AppError> {
        self.authorize(actor, "query").await?;
        let (from, to) = (UnitCode::parse(from)?, UnitCode::parse(to)?);
        if from == to || unit_conversion::is_misc(item.trim()) {
            return Ok(Decimal::ONE);
        }
        let plant = PlantId::parse(plant)?;
        if let Ok(no) = ItemNo::parse(item) {
            let own = self
                .convs
                .list_item_unit_conversions(&plant, Some(&no))
                .await
                .map_err(infra)?;
            if let Some(f) = own
                .iter()
                .filter(|c| c.active)
                .find_map(|c| c.conversion.factor(&from, &to))
            {
                return Ok(f);
            }
        }
        self.general
            .list_unit_conversions()
            .await
            .map_err(infra)?
            .iter()
            .filter(|c| c.active)
            .find_map(|c| c.conversion.factor(&from, &to))
            .ok_or_else(|| DomainError::UnitConversionNotFound.into())
    }

    /// 異動記錄：該料件的換算（ADR 0012 §8）
    pub async fn history(
        &self,
        actor: &Actor,
        plant: &str,
        item: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        self.authorize(actor, "history").await?;
        let plant = PlantId::parse(plant)?;
        self.history
            .read(
                Some(&plant),
                &["item_unit_conversion"],
                &[("item_no", item.trim())],
                lang,
            )
            .await
    }

    /// 列印、匯出 Excel：該營運中心全部料件換算（`item` 有值時只列該料件）
    pub async fn report(
        &self,
        actor: &Actor,
        plant: &str,
        item: Option<&str>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let plant_id = PlantId::parse(plant)?;
        let item = item.map(ItemNo::parse).transpose()?;
        let all = self
            .convs
            .list_item_unit_conversions(&plant_id, item.as_ref())
            .await
            .map_err(infra)?;
        self.reports
            .check(u64::try_from(all.len()).unwrap_or(u64::MAX))?;
        let lines = all
            .iter()
            .map(|c| {
                let v = &c.conversion;
                vec![
                    c.item.as_str().to_owned(),
                    v.from_unit.as_str().to_owned(),
                    v.from_quantity.normalize().to_string(),
                    v.to_unit.as_str().to_owned(),
                    v.to_quantity.normalize().to_string(),
                    v.note.clone().unwrap_or_default(),
                    yes_no(c.active),
                ]
            })
            .collect();
        self.reports
            .build(
                actor,
                Some(&plant_id),
                &ITEM_REPORT,
                lines,
                &[],
                format,
                lang,
            )
            .await
    }

    /// 料件須存在且有效（aooi103.4gl:470-482 mfg1000）
    async fn active_item(&self, plant: &PlantId, item: &str) -> Result<ItemNo, AppError> {
        let no = ItemNo::parse(item)?;
        let it = self
            .items
            .get_item(plant, &no)
            .await
            .map_err(infra)?
            .ok_or(DomainError::ItemNotFound)?;
        if it.active_code == ActiveCode::Invalid {
            return Err(DomainError::ItemInactive.into());
        }
        Ok(no)
    }

    async fn load(
        &self,
        plant: &PlantId,
        item: &str,
        from: &str,
        to: &str,
    ) -> Result<ItemUnitConversion, AppError> {
        let no = ItemNo::parse(item)?;
        let (from, to) = (UnitCode::parse(from)?, UnitCode::parse(to)?);
        self.convs
            .list_item_unit_conversions(plant, Some(&no))
            .await
            .map_err(infra)?
            .into_iter()
            .find(|c| c.conversion.same_pair(&from, &to))
            .ok_or_else(|| DomainError::UnitConversionNotFound.into())
    }

    async fn authorize(
        &self,
        actor: &Actor,
        action: &'static str,
    ) -> Result<crate::authorization::Grant, AppError> {
        self.authz.authorize(actor, ITEM_PROGRAM, action).await
    }
}

fn item_dto(c: &ItemUnitConversion) -> ItemUnitConversionDto {
    let v = &c.conversion;
    ItemUnitConversionDto {
        item_no: c.item.as_str().to_owned(),
        from_unit: v.from_unit.as_str().to_owned(),
        to_unit: v.to_unit.as_str().to_owned(),
        from_quantity: v.from_quantity,
        to_quantity: v.to_quantity,
        note: v.note.clone(),
        active: c.active,
        created_by: c.created_by.clone(),
        created_dept: c.created_dept.clone(),
        created_on: c.created_on,
        updated_by: c.updated_by.clone(),
        updated_on: c.updated_on,
        version: c.stamp.version,
        created_at: c.stamp.created_at,
        updated_at: c.stamp.updated_at,
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
