//! Group Center Maintenance 集團中心維護 use case（TipTop: aooi600 / geu_file 集團中心、gev_file 資料中心的資料類別）
//! 規格：specs/features/aoo/group_center.feature（GC1–GC11）
//!
//! 共用 schema（GC2，ADR 0007 §8）。功能代號：query、insert、modify、delete、invalid、reproduce、confirm、notconfirm、
//! output、exporttoexcel、history，以及 TipTop 的 data_type_setting（資料類別設定）、carry_setting（拋轉設定，GC7）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    group_center::{
        CenterCategory, DataCenterType, DataType, GroupCenter, GroupCenterCode, parse_name,
    },
    plant::PlantId,
};
use ttrs_shared::{
    DataCenterTypeDto, DataCenterTypeInput, GroupCenterDto, GroupCenterInput, Page, QbeCondition,
    QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{
        CarrySettingRepository, DataControlRepository, GroupCenterRepository, PlantRepository,
    },
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "group-center-maintenance";

/// 清單報表（GC10）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "group-center-report-title",
    columns: &[
        ("category", "group-center-col-category"),
        ("center_code", "group-center-col-code"),
        ("name", "group-center-col-name"),
        ("active", "group-center-col-active"),
    ],
};

#[derive(Clone)]
pub struct GroupCenterService {
    authz: Arc<dyn Authorizer>,
    centers: Arc<dyn GroupCenterRepository>,
    plants: Arc<dyn PlantRepository>,
    /// 受控營運中心（data_control.feature DC2：同一資料類別＋營運中心不可又是資料中心）
    controls: Arc<dyn DataControlRepository>,
    /// 拋轉設定（carry_setting.feature CS8(b)：刪除集團中心或最後一個來源營運中心時連同刪除）
    carries: Arc<dyn CarrySettingRepository>,
    reports: ListReporter,
}

impl GroupCenterService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        centers: Arc<dyn GroupCenterRepository>,
        plants: Arc<dyn PlantRepository>,
        controls: Arc<dyn DataControlRepository>,
        carries: Arc<dyn CarrySettingRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            centers,
            plants,
            controls,
            carries,
            reports,
        }
    }

    /// 全部集團中心，依類別、代碼排序（GC10；TipTop 清單 ORDER BY geu00、報表 ORDER BY geu01）
    async fn sorted(&self) -> Result<Vec<GroupCenter>, AppError> {
        let mut all = self.centers.list_group_centers().await.map_err(infra)?;
        all.sort_by(|a, b| (a.category, a.code.as_str()).cmp(&(b.category, b.code.as_str())));
        Ok(all)
    }

    /// QBE 查詢（aooi600.4gl:143-170）
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<GroupCenterDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::group_center::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.sorted().await?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::group_center::value, p);
        Ok(Page {
            items: rows.iter().map(to_dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印 output、匯出 Excel exporttoexcel（GC10）
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::group_center::filters(&conditions)?;
        let all = self.sorted().await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::group_center::value, p);
        self.reports.check(total)?;
        let t = |id: &str| report::text(lang, id);
        let lines = rows
            .iter()
            .map(|c| {
                vec![
                    t(&format!("group-center-category-{}", c.category.code())),
                    c.code.as_str().to_owned(),
                    c.name.clone().unwrap_or_default(),
                    t(if c.active {
                        "report-active"
                    } else {
                        "report-inactive"
                    }),
                ]
            })
            .collect();
        self.reports
            .build(actor, None, &REPORT, lines, &conditions, format, lang)
            .await
    }

    pub async fn get(&self, actor: &Actor, code: &str) -> Result<GroupCenterDto, AppError> {
        self.authorize(actor, "query").await?;
        Ok(to_dto(&self.load(code).await?))
    }

    /// 新增（:204-250）：資料有效 Y；代碼不可重複（-239）
    pub async fn create(
        &self,
        actor: &Actor,
        input: GroupCenterInput,
    ) -> Result<GroupCenterDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let code = GroupCenterCode::parse(&input.center_code)?;
            let category = CenterCategory::parse(input.category.as_deref().unwrap_or_default())?;
            let center = GroupCenter::new(
                code,
                parse_name(input.name)?,
                category,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            self.insert(&center).await
        })
        .await
    }

    /// 修改名稱與類別（:314-337）：代碼不可修改（GC3）；已有資料類別設定時不可改成類別 1 以外（GC4）
    pub async fn update(
        &self,
        actor: &Actor,
        code: &str,
        input: GroupCenterInput,
    ) -> Result<GroupCenterDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            self.authorize(actor, "modify").await?;
            let mut center = self.load(code).await?;
            center.stamp.check(input.version)?;
            let category = CenterCategory::parse(input.category.as_deref().unwrap_or_default())?;
            center.update(parse_name(input.name)?, category, &actor.user_id, today())?;
            self.save(&center).await
        })
        .await
    }

    /// 資料類別設定新增一列（data_type_setting；:778-824）：限有效的資料中心（aoo-046）；
    /// 營運中心須存在；資料類別＋營運中心全集團不可重複（-239）
    pub async fn add_data_type(
        &self,
        actor: &Actor,
        code: &str,
        input: DataCenterTypeInput,
    ) -> Result<GroupCenterDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "data_type_setting", async {
            self.authorize(actor, "data_type_setting").await?;
            let mut center = self.load(code).await?;
            center.stamp.check(input.version)?;
            center.check_data_center()?;
            let row = self.data_type_row(&input.data_type, &input.plant).await?;
            center.add_data_type(row.clone())?;
            // 已是受控營運中心時不可設為資料中心（DC2）
            if self
                .controls
                .get_controlled_plant(row.data_type, &row.plant)
                .await
                .map_err(infra)?
                .is_some()
            {
                return Err(DomainError::DataControlPlantAlreadyExists.into());
            }
            match self.centers.add_data_center_type(&center.code, &row).await {
                Ok(()) => Ok(to_dto(&self.load(code).await?)),
                Err(RepoError::Conflict) => Err(DomainError::DataCenterTypeAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 資料類別設定刪除一列（data_type_setting；:826-875）
    pub async fn remove_data_type(
        &self,
        actor: &Actor,
        code: &str,
        data_type: &str,
        plant: &str,
        version: Option<i32>,
    ) -> Result<GroupCenterDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "data_type_setting", async {
            self.authorize(actor, "data_type_setting").await?;
            let mut center = self.load(code).await?;
            center.stamp.check(version)?;
            let row = DataCenterType {
                data_type: DataType::parse(data_type)?,
                plant: PlantId::parse(plant)
                    .map_err(|_| DomainError::DataCenterTypeNotFound)?
                    .as_str()
                    .to_owned(),
            };
            center.remove_data_type(&row)?;
            if !self
                .centers
                .remove_data_center_type(&center.code, &row)
                .await
                .map_err(infra)?
            {
                return Err(DomainError::DataCenterTypeNotFound.into());
            }
            // 該資料類別已沒有來源營運中心時，連同拋轉設定一起刪除（carry_setting.feature CS8(b)）
            if !center
                .data_types
                .iter()
                .any(|t| t.data_type == row.data_type)
            {
                self.carries
                    .delete_carry_setting(&center.code, row.data_type)
                    .await
                    .map_err(infra)?;
            }
            Ok(to_dto(&self.load(code).await?))
        })
        .await
    }

    /// 停用／啟用（invalid）
    pub async fn set_active(
        &self,
        actor: &Actor,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<GroupCenterDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "invalid", async {
            self.authorize(actor, "invalid").await?;
            let mut center = self.load(code).await?;
            center.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            center.set_active(active, &actor.user_id, today());
            self.save(&center).await
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        code: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<GroupCenterDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let mut center = self.load(code).await?;
            center.stamp.check(version)?;
            center.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&center).await
        })
        .await
    }

    /// 複製（reproduce；ADR 0013 SA5）：只複製集團中心本身，不複製資料類別設定（GC10）
    pub async fn copy(
        &self,
        actor: &Actor,
        code: &str,
        new_code: &str,
    ) -> Result<GroupCenterDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load(code).await?;
            let center = source.copy_as(
                GroupCenterCode::parse(new_code)?,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            self.insert(&center).await
        })
        .await
    }

    /// 刪除連同資料類別設定（:252-299，GC8）：已確認不可刪除（SA9-2）；
    /// 有拋轉紀錄時回 GROUP_CENTER_IN_USE（aoop603 移植時接上；受控營運中心依資料類別、不依資料中心，DC8）
    pub async fn delete(
        &self,
        actor: &Actor,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            self.authorize(actor, "delete").await?;
            let center = self.load(code).await?;
            center.stamp.check(version)?;
            center.confirmation.check_deletable()?;
            // 連同拋轉設定一起刪除（CS8(b)；PostgreSQL 另有外鍵 ON DELETE CASCADE）
            for c in self.carries.list_carry_settings().await.map_err(infra)? {
                if c.center == center.code {
                    match self
                        .carries
                        .delete_carry_setting(&c.center, c.data_type)
                        .await
                    {
                        Ok(_) => {}
                        Err(RepoError::InUse) => return Err(DomainError::GroupCenterInUse.into()),
                        Err(e) => return Err(infra(e)),
                    }
                }
            }
            match self.centers.delete_group_center(&center.code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::GroupCenterNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::GroupCenterInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 資料類別 0–8 與營運中心（須存在於營運中心維護，:805-811）
    async fn data_type_row(
        &self,
        data_type: &str,
        plant: &str,
    ) -> Result<DataCenterType, AppError> {
        let data_type = DataType::parse(data_type)?;
        let id = PlantId::parse(plant).map_err(|_| DomainError::PlantNotFound)?;
        if self.plants.get(&id).await.map_err(infra)?.is_none() {
            return Err(DomainError::PlantNotFound.into());
        }
        Ok(DataCenterType {
            data_type,
            plant: id.as_str().to_owned(),
        })
    }

    async fn insert(&self, center: &GroupCenter) -> Result<GroupCenterDto, AppError> {
        match self.centers.insert_group_center(center).await {
            Ok(()) => Ok(to_dto(&self.load(center.code.as_str()).await?)),
            Err(RepoError::Conflict) => Err(DomainError::GroupCenterAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, center: &GroupCenter) -> Result<GroupCenterDto, AppError> {
        self.centers
            .update_group_center(center)
            .await
            .map_err(infra)?;
        Ok(to_dto(&self.load(center.code.as_str()).await?))
    }

    async fn load(&self, code: &str) -> Result<GroupCenter, AppError> {
        let code = GroupCenterCode::parse(code).map_err(|_| DomainError::GroupCenterNotFound)?;
        self.centers
            .get_group_center(&code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::GroupCenterNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn to_dto(c: &GroupCenter) -> GroupCenterDto {
    GroupCenterDto {
        center_code: c.code.as_str().to_owned(),
        name: c.name.clone(),
        category: c.category.code(),
        data_types: c
            .data_types
            .iter()
            .map(|t| DataCenterTypeDto {
                data_type: t.data_type.code(),
                plant: t.plant.clone(),
            })
            .collect(),
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
