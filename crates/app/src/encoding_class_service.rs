//! Encoding Class Maintenance 編碼類別維護 use case（TipTop: aooi402 / gei_file 單頭、gel_file 單身）
//! 規格：specs/features/aoo/encoding_class.feature（EC1–EC11）
//!
//! 共用 schema（EC2）。功能代號沿用 TipTop：insert、query、modify、delete、invalid、reproduce、detail（單身）、
//! output、exporttoexcel，加上 history、confirm、notconfirm（EC11，ADR 0013）。
//! 各段的檢查要用到編碼性質（aooi401）、獨立段（aooi403）、編碼年對照（aooi404），在這裡查好交給 domain。
//! 「編碼試算」依類別即時組下一個號碼（取代 aooi400 的編碼規則與 s_auno；encoding_rule.feature ER1–ER7）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    accounting_period::Calendar,
    encoding_class::{
        ClassAttrs, ClassCode, ComposeContext, EncodingClass, SegmentContext, SegmentInput,
        SegmentType, compose,
    },
    encoding_nature::{NatureCategory, NatureCode},
    plant::PlantId,
};
use ttrs_shared::{
    EncodingClassDto, EncodingClassInput, EncodingClassSegmentDto, EncodingClassSegmentInput,
    EncodingPreviewDto, Page, QbeCondition, QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{
        AccountingPeriodRepository, EncodingClassRepository, EncodingNatureRepository,
        EncodingSegmentRepository, EncodingYearRepository, ItemRepository,
    },
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "encoding-class-maintenance";

/// 清單報表（EC11）：每段一列
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "encoding-class-report-title",
    columns: &[
        ("class_code", "encoding-class-col-code"),
        ("name", "encoding-class-col-name"),
        ("nature_code", "encoding-class-col-nature"),
        ("active", "encoding-class-col-active"),
        ("seq_no", "encoding-class-col-seq"),
        ("description", "encoding-class-col-description"),
        ("length", "encoding-class-col-length"),
        ("segment_type", "encoding-class-col-type"),
        ("value", "encoding-class-col-value"),
        ("show_name", "encoding-class-col-show-name"),
    ],
};

/// 提示代碼：這種性質還查不到已存在的號碼，流水號從 1 起算（encoding_rule.feature ER3）
pub const TARGET_NOT_SUPPORTED: &str = "ENCODING_RULE_TARGET_NOT_SUPPORTED";

/// 編碼類別用到的資料：類別本身與檢查各段要查的編碼性質、獨立段、編碼年對照；
/// 試算時另查料件（已存在的料號）與會計期間（季別）
#[derive(Clone)]
pub struct EncodingClassRepos {
    pub classes: Arc<dyn EncodingClassRepository>,
    pub natures: Arc<dyn EncodingNatureRepository>,
    pub segments: Arc<dyn EncodingSegmentRepository>,
    pub years: Arc<dyn EncodingYearRepository>,
    pub items: Arc<dyn ItemRepository>,
    pub periods: Arc<dyn AccountingPeriodRepository>,
}

#[derive(Clone)]
pub struct EncodingClassService {
    authz: Arc<dyn Authorizer>,
    repos: EncodingClassRepos,
    reports: ListReporter,
    /// 測試用：指定今天
    today: Option<NaiveDate>,
}

impl EncodingClassService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        repos: EncodingClassRepos,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            repos,
            reports,
            today: None,
        }
    }

    /// 測試用：指定今天
    pub fn with_today(mut self, today: NaiveDate) -> Self {
        self.today = Some(today);
        self
    }

    /// 編碼試算（encoding_rule.feature ER2–ER5；功能 query）：依類別組下一個號碼與名稱，不保留號碼、不寫入資料。
    /// `selections` 依序為各獨立段挑選的編碼編號
    pub async fn preview(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        selections: &[String],
    ) -> Result<EncodingPreviewDto, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let class = self.load(code).await?;
        let today = self.today.unwrap_or_else(today);
        let nature = self
            .repos
            .natures
            .get_encoding_nature(
                &NatureCode::parse(&class.attrs.nature)
                    .map_err(|_| DomainError::EncodingClassNatureInvalid)?,
            )
            .await
            .map_err(infra)?
            .ok_or(DomainError::EncodingClassNatureInvalid)?;
        let mut independent_values = Vec::new();
        for s in self
            .repos
            .segments
            .list_encoding_segments()
            .await
            .map_err(infra)?
            .into_iter()
            .filter(|s| s.active)
        {
            for v in s.values {
                independent_values.push((s.code.as_str().to_owned(), v.value, v.name));
            }
        }
        let year_codes = self
            .repos
            .years
            .list_encoding_years()
            .await
            .map_err(infra)?
            .into_iter()
            .filter(|y| y.active)
            .map(|y| (y.year, y.length, y.code_value))
            .collect();
        let quarter = if class
            .segments
            .iter()
            .any(|s| s.kind == SegmentType::Quarter)
        {
            let years = self
                .repos
                .periods
                .list_accounting_years()
                .await
                .map_err(infra)?;
            Calendar::new(&years).locate(today).ok().map(|d| d.quarter)
        } else {
            None
        };
        // 已存在的號碼（ER3）：料件編號＝這個營運中心的料號；其他性質的模組尚未移植
        let existing = match nature.attrs.category {
            NatureCategory::Item => Some(
                self.repos
                    .items
                    .list_items(&plant)
                    .await
                    .map_err(infra)?
                    .into_iter()
                    .map(|i| i.no.as_str().to_owned())
                    .collect(),
            ),
            _ => None,
        };
        let ctx = ComposeContext {
            independent_values,
            year_codes,
            quarter,
            today,
            existing,
        };
        let r = compose(&class, selections, &ctx)?;
        Ok(EncodingPreviewDto {
            number: r.number,
            name: r.name,
            notices: if r.serial_unchecked {
                vec![TARGET_NOT_SUPPORTED.to_owned()]
            } else {
                Vec::new()
            },
        })
    }

    /// 全部類別，依代碼排序
    async fn sorted(&self) -> Result<Vec<EncodingClass>, AppError> {
        let mut all = self
            .repos
            .classes
            .list_encoding_classes()
            .await
            .map_err(infra)?;
        all.sort_by(|a, b| a.code.as_str().cmp(b.code.as_str()));
        Ok(all)
    }

    /// QBE 查詢（:127-226）：單頭與單身欄位都可當條件，單身符合任一段即列出
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<EncodingClassDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::encoding_class::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.sorted().await?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::encoding_class::value, p);
        Ok(Page {
            items: rows.iter().map(to_dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印 output、匯出 Excel exporttoexcel（EC11）：每段一列，沒有單身的類別印一列
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::encoding_class::filters(&conditions)?;
        let all = self.sorted().await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };
        let (rows, _) = query::page_in_memory(&all, &filters, query::encoding_class::value, p);
        let t = |id: &str| report::text(lang, id);
        let mut lines: Vec<Vec<String>> = Vec::new();
        for c in &rows {
            let head = vec![
                c.code.as_str().to_owned(),
                c.attrs.name.clone(),
                c.attrs.nature.clone(),
                t(if c.active {
                    "report-active"
                } else {
                    "report-inactive"
                }),
            ];
            if c.segments.is_empty() {
                let mut l = head.clone();
                l.extend(std::iter::repeat_n(String::new(), 6));
                lines.push(l);
            }
            for s in &c.segments {
                let mut l = head.clone();
                l.extend([
                    s.no.to_string(),
                    s.description.clone().unwrap_or_default(),
                    s.length.to_string(),
                    t(&format!("encoding-class-type-{}", s.kind.code())),
                    s.value.clone().unwrap_or_default(),
                    t(if s.show_name {
                        "encoding-class-yes"
                    } else {
                        "encoding-class-no"
                    }),
                ]);
                lines.push(l);
            }
        }
        self.reports.check(lines.len() as u64)?;
        self.reports
            .build(actor, None, &REPORT, lines, &conditions, format, lang)
            .await
    }

    pub async fn get(&self, actor: &Actor, code: &str) -> Result<EncodingClassDto, AppError> {
        self.authorize(actor, "query").await?;
        Ok(to_dto(&self.load(code).await?))
    }

    /// 檢查各段要用的資料；性質須存在且有效（:494-504 aoo-131）
    async fn context(&self, nature: &str) -> Result<SegmentContext, AppError> {
        let code =
            NatureCode::parse(nature).map_err(|_| DomainError::EncodingClassNatureInvalid)?;
        let nature = self
            .repos
            .natures
            .get_encoding_nature(&code)
            .await
            .map_err(infra)?
            .filter(|n| n.active)
            .ok_or(DomainError::EncodingClassNatureInvalid)?;
        let independent_segments = self
            .repos
            .segments
            .list_encoding_segments()
            .await
            .map_err(infra)?
            .into_iter()
            .filter(|s| s.active)
            .map(|s| (s.code.as_str().to_owned(), s.attrs.length))
            .collect();
        let mut year_lengths: Vec<i16> = self
            .repos
            .years
            .list_encoding_years()
            .await
            .map_err(infra)?
            .into_iter()
            .filter(|y| y.active)
            .map(|y| y.length)
            .collect();
        year_lengths.dedup();
        Ok(SegmentContext {
            lot_or_serial: matches!(
                nature.attrs.category,
                NatureCategory::Lot | NatureCategory::Serial
            ),
            independent_segments,
            year_lengths,
        })
    }

    /// 新增單頭（:351-395、:462-549）：代碼不可重複（-239）；性質須存在且有效（aoo-131）
    pub async fn create(
        &self,
        actor: &Actor,
        input: EncodingClassInput,
    ) -> Result<EncodingClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let code = ClassCode::parse(&input.class_code)?;
            let attrs = ClassAttrs::new(input.name, &input.nature_code)?;
            self.context(&attrs.nature).await?;
            let class = EncodingClass::new(code, attrs, &actor.user_id, &actor.dept, today());
            self.insert(&class).await
        })
        .await
    }

    /// 修改名稱、性質（:397-459）：停用的不可修改（9027）；改性質時既有各段須仍符合新性質
    pub async fn update(
        &self,
        actor: &Actor,
        code: &str,
        input: EncodingClassInput,
    ) -> Result<EncodingClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            self.authorize(actor, "modify").await?;
            let mut class = self.load(code).await?;
            class.stamp.check(input.version)?;
            let attrs = ClassAttrs::new(input.name, &input.nature_code)?;
            let ctx = self.context(&attrs.nature).await?;
            class.update(attrs, &ctx, &actor.user_id, today())?;
            self.save(&class).await
        })
        .await
    }

    async fn edit_segments<F>(
        &self,
        actor: &Actor,
        code: &str,
        version: Option<i32>,
        delete_row: bool,
        f: F,
    ) -> Result<EncodingClassDto, AppError>
    where
        F: FnOnce(&mut EncodingClass, &SegmentContext) -> Result<(), DomainError>,
    {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            let grant = self.authorize(actor, "detail").await?;
            if delete_row {
                grant.delete_row()?;
            }
            let mut class = self.load(code).await?;
            class.stamp.check(version)?;
            let ctx = self.context(&class.attrs.nature).await?;
            f(&mut class, &ctx)?;
            self.save(&class).await
        })
        .await
    }

    /// 新增一段（detail；:885-1076）
    pub async fn add_segment(
        &self,
        actor: &Actor,
        code: &str,
        input: EncodingClassSegmentInput,
    ) -> Result<EncodingClassDto, AppError> {
        self.authorize(actor, "detail").await?.add_row()?;
        let (no, version, seg) = segment_input(input);
        self.edit_segments(actor, code, version, false, |c, ctx| {
            c.add_segment(no, seg, ctx, &actor.user_id, today())
                .map(drop)
        })
        .await
    }

    /// 在指定段次插入一段，之後的段次全部加 1（detail；:1289-1322 addno）
    pub async fn insert_segment(
        &self,
        actor: &Actor,
        code: &str,
        at: i32,
        input: EncodingClassSegmentInput,
    ) -> Result<EncodingClassDto, AppError> {
        self.authorize(actor, "detail").await?.add_row()?;
        let (_, version, seg) = segment_input(input);
        self.edit_segments(actor, code, version, false, |c, ctx| {
            c.insert_segment(at, seg, ctx, &actor.user_id, today())
        })
        .await
    }

    /// 修改一段（detail）
    pub async fn update_segment(
        &self,
        actor: &Actor,
        code: &str,
        no: i32,
        input: EncodingClassSegmentInput,
    ) -> Result<EncodingClassDto, AppError> {
        let (_, version, seg) = segment_input(input);
        self.edit_segments(actor, code, version, false, |c, ctx| {
            c.update_segment(no, seg, ctx, &actor.user_id, today())
        })
        .await
    }

    /// 刪除一段（detail；:1078-1102）
    pub async fn remove_segment(
        &self,
        actor: &Actor,
        code: &str,
        no: i32,
        version: Option<i32>,
    ) -> Result<EncodingClassDto, AppError> {
        self.edit_segments(actor, code, version, true, |c, _| {
            c.remove_segment(no, &actor.user_id, today())
        })
        .await
    }

    /// 段次重排（detail；:1251-1287 lineno_from_1_resort）
    pub async fn renumber(
        &self,
        actor: &Actor,
        code: &str,
        version: Option<i32>,
    ) -> Result<EncodingClassDto, AppError> {
        self.edit_segments(actor, code, version, false, |c, _| {
            c.renumber(&actor.user_id, today())
        })
        .await
    }

    /// 停用／啟用（invalid；:755-795）
    pub async fn set_active(
        &self,
        actor: &Actor,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<EncodingClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "invalid", async {
            self.authorize(actor, "invalid").await?;
            let mut class = self.load(code).await?;
            class.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            class.set_active(active, &actor.user_id, today());
            self.save(&class).await
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
    ) -> Result<EncodingClassDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let mut class = self.load(code).await?;
            class.stamp.check(version)?;
            class.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&class).await
        })
        .await
    }

    /// 複製單頭與單身（reproduce；:1588-1682）
    pub async fn copy(
        &self,
        actor: &Actor,
        code: &str,
        new_code: &str,
    ) -> Result<EncodingClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load(code).await?;
            let class = source.copy_as(
                ClassCode::parse(new_code)?,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            self.insert(&class).await
        })
        .await
    }

    /// 刪除連同單身（:706-752）：已確認不可刪除（SA9-2）；已有編碼規則時拒絕（EC8，aooi400 移植時接上）
    pub async fn delete(
        &self,
        actor: &Actor,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            self.authorize(actor, "delete").await?;
            let class = self.load(code).await?;
            class.stamp.check(version)?;
            class.confirmation.check_deletable()?;
            match self.repos.classes.delete_encoding_class(&class.code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::EncodingClassNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::EncodingClassInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    async fn insert(&self, class: &EncodingClass) -> Result<EncodingClassDto, AppError> {
        match self.repos.classes.insert_encoding_class(class).await {
            Ok(()) => Ok(to_dto(&self.load(class.code.as_str()).await?)),
            Err(RepoError::Conflict) => Err(DomainError::EncodingClassAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, class: &EncodingClass) -> Result<EncodingClassDto, AppError> {
        self.repos
            .classes
            .update_encoding_class(class)
            .await
            .map_err(infra)?;
        Ok(to_dto(&self.load(class.code.as_str()).await?))
    }

    async fn load(&self, code: &str) -> Result<EncodingClass, AppError> {
        let code = ClassCode::parse(code).map_err(|_| DomainError::EncodingClassNotFound)?;
        self.repos
            .classes
            .get_encoding_class(&code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::EncodingClassNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

/// 畫面輸入轉成 domain 的一段：（段次, 單頭版本, 內容）
fn segment_input(i: EncodingClassSegmentInput) -> (Option<i32>, Option<i32>, SegmentInput) {
    (
        i.seq_no,
        i.version,
        SegmentInput {
            description: i.description,
            length: i.length,
            kind: i.segment_type,
            value: i.value,
            show_name: i.show_name,
        },
    )
}

fn to_dto(c: &EncodingClass) -> EncodingClassDto {
    EncodingClassDto {
        class_code: c.code.as_str().to_owned(),
        name: c.attrs.name.clone(),
        nature_code: c.attrs.nature.clone(),
        segment_count: c.segment_count(),
        total_length: c.total_length(),
        segments: c
            .segments
            .iter()
            .map(|s| EncodingClassSegmentDto {
                seq_no: s.no,
                description: s.description.clone(),
                length: s.length,
                segment_type: s.kind.code().to_owned(),
                value: s.value.clone(),
                show_name: s.show_name,
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
