//! Data Control Maintenance 資料控管原則維護 use case（TipTop: aooi601 / gev_file 受控營運中心、gey_file 欄位條件）
//! 規格：specs/features/aoo/data_control.feature（DC1–DC11）
//!
//! 共用 schema、集團層（DC2）。一個資料類別一筆：資料中心的營運中心來自集團中心維護（唯讀），受控營運中心與欄位條件在這裡維護。
//! 功能代號沿用 TipTop：query、detail（受控營運中心）、condition_detail（欄位條件）、exporttoexcel，
//! 加上 output、reproduce（複製欄位條件）、history（DC9）。失效、確認為例外（DC9）。

use std::{collections::BTreeMap, sync::Arc};

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    data_control::{ControlledPlant, FieldRule},
    group_center::{DATA_CENTER, DataType, GroupCenter},
    plant::PlantId,
};
use ttrs_shared::{
    DataControlCopyInput, DataControlDto, DataControlPlantDto, DataControlPlantInput,
    DataControlRuleDto, DataControlRuleInput, Page, QbeCondition, QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{DataControlRepository, GroupCenterRepository, PlantRepository},
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "data-control-maintenance";

/// 清單報表（DC9）：每個欄位條件一列，沒有條件的營運中心印一列
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "data-control-report-title",
    columns: &[
        ("data_type", "data-control-col-data-type"),
        ("plant", "data-control-col-plant"),
        ("is_data_center", "data-control-col-is-data-center"),
        ("center_code", "data-control-col-center-code"),
        ("field", "data-control-col-field"),
        ("condition", "data-control-col-condition"),
        ("values", "data-control-col-values"),
    ],
};

#[derive(Clone)]
pub struct DataControlService {
    authz: Arc<dyn Authorizer>,
    controls: Arc<dyn DataControlRepository>,
    centers: Arc<dyn GroupCenterRepository>,
    plants: Arc<dyn PlantRepository>,
    reports: ListReporter,
}

impl DataControlService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        controls: Arc<dyn DataControlRepository>,
        centers: Arc<dyn GroupCenterRepository>,
        plants: Arc<dyn PlantRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            controls,
            centers,
            plants,
            reports,
        }
    }

    /// 全部資料類別（DC3：已有資料中心或受控營運中心的 1–8），依資料類別排序；營運中心依代碼排序
    async fn all(&self) -> Result<Vec<DataControlDto>, AppError> {
        let centers = self.centers.list_group_centers().await.map_err(infra)?;
        let controlled = self
            .controls
            .list_controlled_plants()
            .await
            .map_err(infra)?;
        let mut by_type: BTreeMap<String, Vec<DataControlPlantDto>> = BTreeMap::new();
        for c in &centers {
            for t in &c.data_types {
                if t.data_type.code() == "0" {
                    continue;
                }
                by_type
                    .entry(t.data_type.code())
                    .or_default()
                    .push(center_row(c, &t.plant));
            }
        }
        for p in &controlled {
            by_type
                .entry(p.data_type.code())
                .or_default()
                .push(controlled_row(p));
        }
        Ok(by_type
            .into_iter()
            .map(|(data_type, mut plants)| {
                plants.sort_by(|a, b| a.plant.cmp(&b.plant));
                DataControlDto { data_type, plants }
            })
            .collect())
    }

    /// QBE 查詢：營運中心或欄位條件符合任一列即列出該資料類別（aooi601.4gl:207-219）
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<DataControlDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::data_control::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.all().await?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::data_control::value, p);
        Ok(Page {
            items: rows,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    pub async fn get(&self, actor: &Actor, data_type: &str) -> Result<DataControlDto, AppError> {
        self.authorize(actor, "query").await?;
        let t = DataType::parse(data_type)?;
        Ok(self
            .all()
            .await?
            .into_iter()
            .find(|d| d.data_type == t.code())
            .unwrap_or(DataControlDto {
                data_type: t.code(),
                plants: Vec::new(),
            }))
    }

    /// 列印 output、匯出 Excel exporttoexcel（DC9）
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::data_control::filters(&conditions)?;
        let all = self.all().await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };
        let (rows, _) = query::page_in_memory(&all, &filters, query::data_control::value, p);
        let t = |id: &str| report::text(lang, id);
        let mut lines: Vec<Vec<String>> = Vec::new();
        for d in &rows {
            for pl in &d.plants {
                let head = vec![
                    t(&format!("data-control-data-type-{}", d.data_type)),
                    pl.plant.clone(),
                    t(if pl.is_data_center {
                        "data-control-yes"
                    } else {
                        "data-control-no"
                    }),
                    pl.center_code.clone().unwrap_or_default(),
                ];
                if pl.rules.is_empty() {
                    let mut l = head.clone();
                    l.extend([String::new(), String::new(), String::new()]);
                    lines.push(l);
                }
                for r in &pl.rules {
                    let mut l = head.clone();
                    l.extend([
                        t(&format!("data-control-field-{}", r.field)),
                        t(&format!("data-control-condition-{}", r.condition)),
                        r.values.clone(),
                    ]);
                    lines.push(l);
                }
            }
        }
        self.reports.check(lines.len() as u64)?;
        self.reports
            .build(actor, None, &REPORT, lines, &conditions, format, lang)
            .await
    }

    /// 新增受控營運中心（detail；aooi601.4gl:559-615）：營運中心須存在（aap-025）；資料類別須有有效的資料中心（DC3）；
    /// 資料類別＋營運中心不可已是資料中心或受控營運中心（-239，DC2）
    pub async fn add_plant(
        &self,
        actor: &Actor,
        input: DataControlPlantInput,
    ) -> Result<DataControlDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let data_type = DataType::parse(&input.data_type)?;
            ttrs_domain::data_control::check_maintainable(data_type)?;
            let plant = self.plant(&input.plant).await?;
            let centers = self.centers.list_group_centers().await.map_err(infra)?;
            let has_center = centers.iter().any(|c| {
                c.category == DATA_CENTER
                    && c.active
                    && c.data_types.iter().any(|t| t.data_type == data_type)
            });
            if !has_center {
                return Err(DomainError::DataControlNoDataCenter.into());
            }
            let row =
                ControlledPlant::new(data_type, &plant, &actor.user_id, &actor.dept, today())?;
            self.insert(&row).await?;
            self.get_unchecked(data_type).await
        })
        .await
    }

    /// 刪除受控營運中心連同欄位條件（detail；:617-649）；資料中心的營運中心不能在這裡刪除（DC4）
    pub async fn remove_plant(
        &self,
        actor: &Actor,
        data_type: &str,
        plant: &str,
        version: Option<i32>,
    ) -> Result<DataControlDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let row = self.load(data_type, plant).await?;
            row.stamp.check(version)?;
            if !self
                .controls
                .delete_controlled_plant(row.data_type, &row.plant)
                .await
                .map_err(infra)?
            {
                return Err(DomainError::DataControlPlantNotFound.into());
            }
            self.get_unchecked(row.data_type).await
        })
        .await
    }

    /// 新增欄位條件（condition_detail；:810-876）
    pub async fn add_rule(
        &self,
        actor: &Actor,
        data_type: &str,
        plant: &str,
        input: DataControlRuleInput,
    ) -> Result<DataControlDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "condition_detail", async {
            self.authorize(actor, "condition_detail").await?.add_row()?;
            let mut row = self.load_for_rule(data_type, plant).await?;
            row.stamp.check(input.version)?;
            let rule =
                FieldRule::new(row.data_type, &input.field, &input.condition, &input.values)?;
            row.add_rule(rule, &actor.user_id, today())?;
            self.save(&row).await
        })
        .await
    }

    /// 修改欄位條件的條件與值（condition_detail；:902-929）
    pub async fn update_rule(
        &self,
        actor: &Actor,
        data_type: &str,
        plant: &str,
        input: DataControlRuleInput,
    ) -> Result<DataControlDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "condition_detail", async {
            self.authorize(actor, "condition_detail").await?;
            let mut row = self.load_for_rule(data_type, plant).await?;
            row.stamp.check(input.version)?;
            let rule =
                FieldRule::new(row.data_type, &input.field, &input.condition, &input.values)?;
            row.update_rule(rule, &actor.user_id, today())?;
            self.save(&row).await
        })
        .await
    }

    /// 刪除欄位條件（condition_detail；:878-900）
    pub async fn remove_rule(
        &self,
        actor: &Actor,
        data_type: &str,
        plant: &str,
        field: &str,
        version: Option<i32>,
    ) -> Result<DataControlDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "condition_detail", async {
            self.authorize(actor, "condition_detail")
                .await?
                .delete_row()?;
            let mut row = self.load_for_rule(data_type, plant).await?;
            row.stamp.check(version)?;
            row.remove_rule(field.trim(), &actor.user_id, today())?;
            self.save(&row).await
        })
        .await
    }

    /// 複製（reproduce；DC9）：把一個受控營運中心的全部欄位條件複製到同一資料類別的另一個營運中心（一併加入清單）
    pub async fn copy_rules(
        &self,
        actor: &Actor,
        data_type: &str,
        plant: &str,
        input: DataControlCopyInput,
    ) -> Result<DataControlDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load_for_rule(data_type, plant).await?;
            let to = self.plant(&input.to_plant).await?;
            let mut row =
                ControlledPlant::new(source.data_type, &to, &actor.user_id, &actor.dept, today())?;
            row.rules = source.rules.clone();
            self.insert(&row).await?;
            self.get_unchecked(source.data_type).await
        })
        .await
    }

    /// 營運中心須存在（aap-025）
    async fn plant(&self, raw: &str) -> Result<String, AppError> {
        let id = PlantId::parse(raw).map_err(|_| DomainError::PlantNotFound)?;
        if self.plants.get(&id).await.map_err(infra)?.is_none() {
            return Err(DomainError::PlantNotFound.into());
        }
        Ok(id.as_str().to_owned())
    }

    /// 欄位條件只限受控營運中心；資料中心的營運中心回 aoo-093（DC5(a)）
    async fn load_for_rule(
        &self,
        data_type: &str,
        plant: &str,
    ) -> Result<ControlledPlant, AppError> {
        let t = DataType::parse(data_type)?;
        ttrs_domain::data_control::check_maintainable(t)?;
        let id = PlantId::parse(plant).map_err(|_| DomainError::DataControlPlantNotFound)?;
        if let Some(row) = self
            .controls
            .get_controlled_plant(t, id.as_str())
            .await
            .map_err(infra)?
        {
            return Ok(row);
        }
        let is_center = self
            .centers
            .list_group_centers()
            .await
            .map_err(infra)?
            .iter()
            .any(|c| {
                c.data_types
                    .iter()
                    .any(|x| x.data_type == t && x.plant == id.as_str())
            });
        Err(if is_center {
            DomainError::DataControlRuleNotAllowed
        } else {
            DomainError::DataControlPlantNotFound
        }
        .into())
    }

    async fn load(&self, data_type: &str, plant: &str) -> Result<ControlledPlant, AppError> {
        let t = DataType::parse(data_type)?;
        let id = PlantId::parse(plant).map_err(|_| DomainError::DataControlPlantNotFound)?;
        self.controls
            .get_controlled_plant(t, id.as_str())
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::DataControlPlantNotFound.into())
    }

    async fn insert(&self, row: &ControlledPlant) -> Result<(), AppError> {
        match self.controls.insert_controlled_plant(row).await {
            Ok(()) => Ok(()),
            Err(RepoError::Conflict) => Err(DomainError::DataControlPlantAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, row: &ControlledPlant) -> Result<DataControlDto, AppError> {
        self.controls
            .update_controlled_plant(row)
            .await
            .map_err(infra)?;
        self.get_unchecked(row.data_type).await
    }

    async fn get_unchecked(&self, t: DataType) -> Result<DataControlDto, AppError> {
        Ok(self
            .all()
            .await?
            .into_iter()
            .find(|d| d.data_type == t.code())
            .unwrap_or(DataControlDto {
                data_type: t.code(),
                plants: Vec::new(),
            }))
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

/// 料件等作業的欄位值檢查（s_field_chk，DC6、DC7）：營運中心是該資料類別的受控營運中心時才檢查
pub async fn check_field(
    controls: &dyn DataControlRepository,
    data_type: &str,
    plant: &PlantId,
    field: &str,
    value: &str,
) -> Result<(), AppError> {
    let t = DataType::parse(data_type)?;
    let row = controls
        .get_controlled_plant(t, plant.as_str())
        .await
        .map_err(infra)?;
    match row {
        Some(r) => Ok(r.check_field(field, value)?),
        None => Ok(()),
    }
}

fn center_row(c: &GroupCenter, plant: &str) -> DataControlPlantDto {
    DataControlPlantDto {
        plant: plant.to_owned(),
        is_data_center: true,
        center_code: Some(c.code.as_str().to_owned()),
        center_name: c.name.clone(),
        rules: Vec::new(),
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

fn controlled_row(p: &ControlledPlant) -> DataControlPlantDto {
    DataControlPlantDto {
        plant: p.plant.clone(),
        is_data_center: false,
        center_code: None,
        center_name: None,
        rules: p
            .rules
            .iter()
            .map(|r| DataControlRuleDto {
                field: r.field.clone(),
                condition: r.condition.code(),
                values: r.values.clone(),
            })
            .collect(),
        created_by: p.created_by.clone(),
        created_dept: p.created_dept.clone(),
        created_on: p.created_on,
        updated_by: p.updated_by.clone(),
        updated_on: p.updated_on,
        version: p.stamp.version,
        created_at: p.stamp.created_at,
        updated_at: p.stamp.updated_at,
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
