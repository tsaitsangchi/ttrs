//! Carry Setting Maintenance 資料中心拋轉設定 use case（TipTop: aooi602 / gew_file 拋轉原則、gez_file 拋轉預設資料）
//! 規格：specs/features/aoo/carry_setting.feature（CS1–CS12）
//!
//! 共用 schema、集團層（CS2）。功能代號沿用 TipTop：insert、modify、delete、query、detail（拋轉營運中心與拋轉條件）、
//! set_default_value（預設值）、exporttoexcel，加上 output、reproduce、history（CS9）。失效、確認為例外。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    carry_setting::{CarryMode, CarrySetting},
    group_center::{DataType, GroupCenter, GroupCenterCode},
    plant::PlantId,
};
use ttrs_shared::{
    CarryCopyInput, CarryDefaultDto, CarryDefaultInput, CarrySettingDto, CarrySettingInput,
    CarryTargetDto, CarryTargetInput, DataControlRuleDto, DataControlRuleInput, Page, QbeCondition,
    QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{CarryLogRepository, CarrySettingRepository, GroupCenterRepository, PlantRepository},
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "carry-setting-maintenance";

/// 清單報表（CS9）：每個拋轉營運中心一列
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "carry-setting-report-title",
    columns: &[
        ("center_code", "carry-setting-col-center"),
        ("data_type", "carry-setting-col-data-type"),
        ("mode", "carry-setting-col-mode"),
        ("target_plant", "carry-setting-col-plant"),
        ("deletable", "carry-setting-col-deletable"),
        ("update_existing", "carry-setting-col-update-existing"),
        ("conditions", "carry-setting-col-conditions"),
        ("defaults", "carry-setting-col-defaults"),
        ("notify", "carry-setting-col-notify"),
    ],
};

#[derive(Clone)]
pub struct CarrySettingService {
    authz: Arc<dyn Authorizer>,
    carries: Arc<dyn CarrySettingRepository>,
    centers: Arc<dyn GroupCenterRepository>,
    plants: Arc<dyn PlantRepository>,
    /// 拋轉紀錄（已有紀錄時不可刪除，CS8(c)、basic_data_carry.feature BC11）
    logs: Arc<dyn CarryLogRepository>,
    reports: ListReporter,
}

/// 修改單身時要做的事
enum Edit<'a> {
    AddTarget(&'a str),
    UpdateTarget(&'a CarryTargetInput),
    RemoveTarget(&'a str),
    PutCondition(&'a str, &'a DataControlRuleInput, bool),
    RemoveCondition(&'a str, &'a str),
    PutDefault(&'a str, &'a CarryDefaultInput, bool),
    RemoveDefault(&'a str, &'a str),
}

impl CarrySettingService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        carries: Arc<dyn CarrySettingRepository>,
        centers: Arc<dyn GroupCenterRepository>,
        plants: Arc<dyn PlantRepository>,
        logs: Arc<dyn CarryLogRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            carries,
            centers,
            plants,
            logs,
            reports,
        }
    }

    /// 全部拋轉設定，依資料中心、資料類別排序（:186-188）
    async fn all(&self) -> Result<Vec<CarrySettingDto>, AppError> {
        let centers = self.centers.list_group_centers().await.map_err(infra)?;
        let mut all = self.carries.list_carry_settings().await.map_err(infra)?;
        all.sort_by(|a, b| (a.center.as_str(), a.data_type).cmp(&(b.center.as_str(), b.data_type)));
        Ok(all
            .iter()
            .map(|c| {
                let name = centers
                    .iter()
                    .find(|g| g.code == c.center)
                    .and_then(|g| g.name.clone());
                to_dto(c, name)
            })
            .collect())
    }

    /// QBE 查詢（:118-202）：單身欄位符合任一列即列出
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<CarrySettingDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::carry_setting::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.all().await?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::carry_setting::value, p);
        Ok(Page {
            items: rows,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    pub async fn get(
        &self,
        actor: &Actor,
        center: &str,
        data_type: &str,
    ) -> Result<CarrySettingDto, AppError> {
        self.authorize(actor, "query").await?;
        let c = self.load(center, data_type).await?;
        self.dto(&c).await
    }

    /// 列印 output、匯出 Excel exporttoexcel（CS9）：每個拋轉營運中心一列
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::carry_setting::filters(&conditions)?;
        let all = self.all().await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };
        let (rows, _) = query::page_in_memory(&all, &filters, query::carry_setting::value, p);
        let t = |id: &str| report::text(lang, id);
        let yes_no = |v: bool| {
            t(if v {
                "data-control-yes"
            } else {
                "data-control-no"
            })
        };
        let mut lines: Vec<Vec<String>> = Vec::new();
        for c in &rows {
            let head = vec![
                c.center_code.clone(),
                t(&format!("carry-setting-data-type-{}", c.data_type)),
                t(&format!("carry-setting-mode-{}", c.mode)),
            ];
            if c.targets.is_empty() {
                let mut l = head.clone();
                l.extend(std::iter::repeat_n(String::new(), 6));
                lines.push(l);
            }
            for g in &c.targets {
                let conditions = g
                    .conditions
                    .iter()
                    .map(|r| {
                        format!(
                            "{} {} {}",
                            t(&format!("data-control-field-{}", r.field)),
                            t(&format!("data-control-condition-{}", r.condition)),
                            r.values
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("；");
                let defaults = g
                    .defaults
                    .iter()
                    .map(|d| {
                        format!(
                            "{}＝{}",
                            t(&format!("data-control-field-{}", d.field)),
                            d.value
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("；");
                let mut l = head.clone();
                l.extend([
                    g.plant.clone(),
                    yes_no(g.deletable),
                    yes_no(g.update_existing),
                    conditions,
                    defaults,
                    g.notify.clone().unwrap_or_default(),
                ]);
                lines.push(l);
            }
        }
        self.reports.check(lines.len() as u64)?;
        self.reports
            .build(actor, None, &REPORT, lines, &conditions, format, lang)
            .await
    }

    /// 新增（insert；:415-581）：資料中心須有效且為類別 1、須負責此資料類別（aoo-094）；拋轉方式 1–3（空白為 3）
    pub async fn create(
        &self,
        actor: &Actor,
        input: CarrySettingInput,
    ) -> Result<CarrySettingDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let (center, data_type) = self
                .check_head(&input.center_code, &input.data_type)
                .await?;
            let mode = CarryMode::parse(&input.mode)?;
            let c = CarrySetting::new(
                center.code,
                data_type,
                mode,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            self.insert(&c).await
        })
        .await
    }

    /// 修改拋轉方式（modify；:451-497）：資料中心、資料類別不可改
    pub async fn update_mode(
        &self,
        actor: &Actor,
        center: &str,
        data_type: &str,
        input: CarrySettingInput,
    ) -> Result<CarrySettingDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            self.authorize(actor, "modify").await?;
            let mut c = self.load(center, data_type).await?;
            c.stamp.check(input.version)?;
            let mode = CarryMode::parse(&input.mode)?;
            c.set_mode(mode, &actor.user_id, today());
            self.save(&c).await
        })
        .await
    }

    /// 新增拋轉營運中心（detail；:720-783）
    pub async fn add_target(
        &self,
        actor: &Actor,
        center: &str,
        data_type: &str,
        input: CarryTargetInput,
    ) -> Result<CarrySettingDto, AppError> {
        let plant = input.plant.clone();
        self.edit(
            actor,
            "detail",
            center,
            data_type,
            input.version,
            Edit::AddTarget(&plant),
        )
        .await
    }

    /// 修改拋轉營運中心的子公司可刪除、更新已存在資料、通知人員（detail；:819-852）
    pub async fn update_target(
        &self,
        actor: &Actor,
        center: &str,
        data_type: &str,
        input: CarryTargetInput,
    ) -> Result<CarrySettingDto, AppError> {
        self.edit(
            actor,
            "detail",
            center,
            data_type,
            input.version,
            Edit::UpdateTarget(&input),
        )
        .await
    }

    /// 刪除拋轉營運中心連同條件與預設值（detail；:785-817）
    pub async fn remove_target(
        &self,
        actor: &Actor,
        center: &str,
        data_type: &str,
        plant: &str,
        version: Option<i32>,
    ) -> Result<CarrySettingDto, AppError> {
        self.edit(
            actor,
            "detail",
            center,
            data_type,
            version,
            Edit::RemoveTarget(plant),
        )
        .await
    }

    /// 新增（replace＝false）或修改拋轉條件（detail；:1135-1285）
    pub async fn put_condition(
        &self,
        actor: &Actor,
        center: &str,
        data_type: &str,
        plant: &str,
        input: DataControlRuleInput,
        replace: bool,
    ) -> Result<CarrySettingDto, AppError> {
        let version = input.version;
        self.edit(
            actor,
            "detail",
            center,
            data_type,
            version,
            Edit::PutCondition(plant, &input, replace),
        )
        .await
    }

    pub async fn remove_condition(
        &self,
        actor: &Actor,
        center: &str,
        data_type: &str,
        plant: &str,
        field: &str,
        version: Option<i32>,
    ) -> Result<CarrySettingDto, AppError> {
        self.edit(
            actor,
            "detail",
            center,
            data_type,
            version,
            Edit::RemoveCondition(plant, field),
        )
        .await
    }

    /// 新增（replace＝false）或修改預設值（set_default_value；:1287-1539）
    pub async fn put_default(
        &self,
        actor: &Actor,
        center: &str,
        data_type: &str,
        plant: &str,
        input: CarryDefaultInput,
        replace: bool,
    ) -> Result<CarrySettingDto, AppError> {
        let version = input.version;
        self.edit(
            actor,
            "set_default_value",
            center,
            data_type,
            version,
            Edit::PutDefault(plant, &input, replace),
        )
        .await
    }

    pub async fn remove_default(
        &self,
        actor: &Actor,
        center: &str,
        data_type: &str,
        plant: &str,
        field: &str,
        version: Option<i32>,
    ) -> Result<CarrySettingDto, AppError> {
        self.edit(
            actor,
            "set_default_value",
            center,
            data_type,
            version,
            Edit::RemoveDefault(plant, field),
        )
        .await
    }

    /// 複製到另一個資料中心＋資料類別（reproduce；CS9）
    pub async fn copy(
        &self,
        actor: &Actor,
        center: &str,
        data_type: &str,
        input: CarryCopyInput,
    ) -> Result<CarrySettingDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load(center, data_type).await?;
            let (to_center, to_type) = self
                .check_head(&input.center_code, &input.data_type)
                .await?;
            for t in &source.targets {
                check_not_source(&to_center, to_type, &t.plant)?;
            }
            let c = source.copy_as(
                to_center.code,
                to_type,
                &actor.user_id,
                &actor.dept,
                today(),
            )?;
            self.insert(&c).await
        })
        .await
    }

    /// 刪除連同全部單身（delete；:602-648）；已有拋轉紀錄時不可刪除（CS8(c)，aoop603 移植時接上）
    pub async fn delete(
        &self,
        actor: &Actor,
        center: &str,
        data_type: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            self.authorize(actor, "delete").await?;
            let c = self.load(center, data_type).await?;
            c.stamp.check(version)?;
            if self
                .logs
                .carry_log_exists(&c.center, Some(c.data_type))
                .await
                .map_err(infra)?
            {
                return Err(DomainError::CarrySettingInUse.into());
            }
            match self
                .carries
                .delete_carry_setting(&c.center, c.data_type)
                .await
            {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::CarrySettingNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::CarrySettingInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    async fn edit(
        &self,
        actor: &Actor,
        action: &'static str,
        center: &str,
        data_type: &str,
        version: Option<i32>,
        edit: Edit<'_>,
    ) -> Result<CarrySettingDto, AppError> {
        crate::change_log::run(actor, PROGRAM, action, async {
            let grant = self.authorize(actor, action).await?;
            match edit {
                Edit::AddTarget(_)
                | Edit::PutCondition(_, _, false)
                | Edit::PutDefault(_, _, false) => grant.add_row()?,
                Edit::RemoveTarget(_) | Edit::RemoveCondition(..) | Edit::RemoveDefault(..) => {
                    grant.delete_row()?
                }
                _ => {}
            }
            let mut c = self.load(center, data_type).await?;
            c.stamp.check(version)?;
            let (u, d) = (actor.user_id.as_str(), today());
            match edit {
                Edit::AddTarget(plant) => {
                    let plant = self.plant(plant).await?;
                    let g = self.center(c.center.as_str()).await?;
                    check_not_source(&g, c.data_type, &plant)?;
                    c.add_target(&plant, u, d)?;
                }
                Edit::UpdateTarget(i) => c.update_target(
                    &plant_key(&i.plant)?,
                    i.deletable,
                    i.update_existing,
                    i.notify.clone(),
                    u,
                    d,
                )?,
                Edit::RemoveTarget(plant) => c.remove_target(&plant_key(plant)?, u, d)?,
                Edit::PutCondition(plant, i, replace) => c.put_condition(
                    &plant_key(plant)?,
                    &i.field,
                    (&i.condition, &i.values),
                    replace,
                    u,
                    d,
                )?,
                Edit::RemoveCondition(plant, field) => {
                    c.remove_condition(&plant_key(plant)?, field, u, d)?
                }
                Edit::PutDefault(plant, i, replace) => {
                    c.put_default(&plant_key(plant)?, &i.field, &i.value, replace, u, d)?
                }
                Edit::RemoveDefault(plant, field) => {
                    c.remove_default(&plant_key(plant)?, field, u, d)?
                }
            }
            self.save(&c).await
        })
        .await
    }

    /// 單頭檢查（CS3(a)(b)）
    async fn check_head(
        &self,
        center: &str,
        data_type: &str,
    ) -> Result<(GroupCenter, DataType), AppError> {
        let g = self.center(center).await?;
        let data_type = DataType::parse(data_type)?;
        g.check_data_center()?;
        if !g.data_types.iter().any(|t| t.data_type == data_type) {
            return Err(DomainError::CarrySettingNoDataType.into());
        }
        Ok((g, data_type))
    }

    async fn center(&self, code: &str) -> Result<GroupCenter, AppError> {
        let code = GroupCenterCode::parse(code).map_err(|_| DomainError::GroupCenterNotFound)?;
        self.centers
            .get_group_center(&code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::GroupCenterNotFound.into())
    }

    /// 營運中心須存在（aap-025）
    async fn plant(&self, raw: &str) -> Result<String, AppError> {
        let id = PlantId::parse(raw).map_err(|_| DomainError::PlantNotFound)?;
        if self.plants.get(&id).await.map_err(infra)?.is_none() {
            return Err(DomainError::PlantNotFound.into());
        }
        Ok(id.as_str().to_owned())
    }

    async fn load(&self, center: &str, data_type: &str) -> Result<CarrySetting, AppError> {
        let center =
            GroupCenterCode::parse(center).map_err(|_| DomainError::CarrySettingNotFound)?;
        let data_type = DataType::parse(data_type)?;
        self.carries
            .get_carry_setting(&center, data_type)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::CarrySettingNotFound.into())
    }

    async fn insert(&self, c: &CarrySetting) -> Result<CarrySettingDto, AppError> {
        match self.carries.insert_carry_setting(c).await {
            Ok(()) => {}
            Err(RepoError::Conflict) => return Err(DomainError::CarrySettingAlreadyExists.into()),
            Err(e) => return Err(infra(e)),
        }
        let saved = self.load(c.center.as_str(), &c.data_type.code()).await?;
        self.dto(&saved).await
    }

    async fn save(&self, c: &CarrySetting) -> Result<CarrySettingDto, AppError> {
        self.carries.update_carry_setting(c).await.map_err(infra)?;
        let saved = self.load(c.center.as_str(), &c.data_type.code()).await?;
        self.dto(&saved).await
    }

    async fn dto(&self, c: &CarrySetting) -> Result<CarrySettingDto, AppError> {
        let name = self
            .centers
            .get_group_center(&c.center)
            .await
            .map_err(infra)?
            .and_then(|g| g.name);
        Ok(to_dto(c, name))
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

/// 拋轉營運中心不可是這個資料中心該資料類別的來源營運中心（CS4(b)）
fn check_not_source(
    center: &GroupCenter,
    data_type: DataType,
    plant: &str,
) -> Result<(), AppError> {
    if center
        .data_types
        .iter()
        .any(|t| t.data_type == data_type && t.plant == plant)
    {
        return Err(DomainError::CarryTargetIsSource.into());
    }
    Ok(())
}

fn plant_key(raw: &str) -> Result<String, AppError> {
    Ok(PlantId::parse(raw)
        .map_err(|_| DomainError::CarryTargetNotFound)?
        .as_str()
        .to_owned())
}

fn to_dto(c: &CarrySetting, center_name: Option<String>) -> CarrySettingDto {
    CarrySettingDto {
        center_code: c.center.as_str().to_owned(),
        center_name,
        data_type: c.data_type.code(),
        mode: c.mode.code(),
        targets: c
            .targets
            .iter()
            .map(|t| CarryTargetDto {
                plant: t.plant.clone(),
                deletable: t.deletable,
                update_existing: t.update_existing,
                notify: t.notify.clone(),
                conditions: t
                    .conditions
                    .iter()
                    .map(|r| DataControlRuleDto {
                        field: r.field.clone(),
                        condition: r.condition.code(),
                        values: r.values.clone(),
                    })
                    .collect(),
                defaults: t
                    .defaults
                    .iter()
                    .map(|d| CarryDefaultDto {
                        field: d.field.clone(),
                        value: d.value.clone(),
                    })
                    .collect(),
            })
            .collect(),
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
