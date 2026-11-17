//! Encoding Segment Maintenance 編碼獨立段維護 use case（TipTop: aooi403 / gej_file 單頭、gek_file 單身）
//! 規格：specs/features/aoo/encoding_segment.feature（IS1–IS8）
//!
//! 共用 schema（IS2）。功能代號沿用 TipTop：insert、query、modify、delete、invalid、reproduce、detail（單身）、
//! output、exporttoexcel，加上 history、confirm、notconfirm（IS8，ADR 0013）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    encoding_segment::{EncodingSegment, SegmentAttrs, SegmentCode},
};
use ttrs_shared::{
    EncodingSegmentDto, EncodingSegmentInput, EncodingSegmentValueDto, EncodingSegmentValueInput,
    Page, QbeCondition, QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::EncodingSegmentRepository,
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "encoding-segment-maintenance";

/// 清單報表（IS8；沿用舊報表 i403_out 的欄位）：每個編碼編號一列
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "encoding-segment-report-title",
    columns: &[
        ("segment_code", "encoding-segment-col-code"),
        ("name", "encoding-segment-col-name"),
        ("length", "encoding-segment-col-length"),
        ("active", "encoding-segment-col-active"),
        ("line", "encoding-segment-col-line"),
        ("value", "encoding-segment-col-value"),
        ("value_name", "encoding-segment-col-value-name"),
    ],
};

#[derive(Clone)]
pub struct EncodingSegmentService {
    authz: Arc<dyn Authorizer>,
    segments: Arc<dyn EncodingSegmentRepository>,
    reports: ListReporter,
}

impl EncodingSegmentService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        segments: Arc<dyn EncodingSegmentRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            segments,
            reports,
        }
    }

    /// 全部獨立段，依編號排序（:199、:205 ORDER BY gej01）
    async fn sorted(&self) -> Result<Vec<EncodingSegment>, AppError> {
        let mut all = self
            .segments
            .list_encoding_segments()
            .await
            .map_err(infra)?;
        all.sort_by(|a, b| a.code.as_str().cmp(b.code.as_str()));
        Ok(all)
    }

    /// QBE 查詢（:108-226）：單頭與單身欄位都可當條件，單身符合任一列即列出
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<EncodingSegmentDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::encoding_segment::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.sorted().await?;
        let (rows, total) =
            query::page_in_memory(&all, &filters, query::encoding_segment::value, p);
        Ok(Page {
            items: rows.iter().map(to_dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印 output、匯出 Excel exporttoexcel（IS8）：每個編碼編號一列，沒有單身的獨立段印一列
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::encoding_segment::filters(&conditions)?;
        let all = self.sorted().await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };
        let (rows, _) = query::page_in_memory(&all, &filters, query::encoding_segment::value, p);
        let t = |id: &str| report::text(lang, id);
        let mut lines: Vec<Vec<String>> = Vec::new();
        for s in &rows {
            let head = vec![
                s.code.as_str().to_owned(),
                s.attrs.name.clone().unwrap_or_default(),
                s.attrs.length.to_string(),
                t(if s.active {
                    "report-active"
                } else {
                    "report-inactive"
                }),
            ];
            if s.values.is_empty() {
                let mut l = head.clone();
                l.extend([String::new(), String::new(), String::new()]);
                lines.push(l);
            }
            for v in &s.values {
                let mut l = head.clone();
                l.extend([
                    v.line.to_string(),
                    v.value.clone(),
                    v.name.clone().unwrap_or_default(),
                ]);
                lines.push(l);
            }
        }
        self.reports.check(lines.len() as u64)?;
        self.reports
            .build(actor, None, &REPORT, lines, &conditions, format, lang)
            .await
    }

    pub async fn get(&self, actor: &Actor, code: &str) -> Result<EncodingSegmentDto, AppError> {
        self.authorize(actor, "query").await?;
        Ok(to_dto(&self.load(code).await?))
    }

    /// 新增單頭（:292-335、:401-463）：資料有效 Y；編號不可重複（-239）
    pub async fn create(
        &self,
        actor: &Actor,
        input: EncodingSegmentInput,
    ) -> Result<EncodingSegmentDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let code = SegmentCode::parse(&input.segment_code)?;
            let attrs = SegmentAttrs::new(input.name, input.length)?;
            let segment = EncodingSegment::new(code, attrs, &actor.user_id, &actor.dept, today());
            self.insert(&segment).await
        })
        .await
    }

    /// 修改單頭（:336-400）：停用的不可修改（9027）；已有單身時不可改長度（IS5(b)）；編號不可修改
    pub async fn update(
        &self,
        actor: &Actor,
        code: &str,
        input: EncodingSegmentInput,
    ) -> Result<EncodingSegmentDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            self.authorize(actor, "modify").await?;
            let mut segment = self.load(code).await?;
            segment.stamp.check(input.version)?;
            let attrs = SegmentAttrs::new(input.name, input.length)?;
            // 被編碼類別使用時不可改長度（EC9(b)），以免和類別的段長度不符
            if attrs.length != segment.attrs.length
                && self
                    .segments
                    .encoding_segment_in_use(&segment.code)
                    .await
                    .map_err(infra)?
            {
                return Err(DomainError::EncodingSegmentInUse.into());
            }
            segment.update(attrs, &actor.user_id, today())?;
            self.save(&segment).await
        })
        .await
    }

    /// 新增單身一列（detail；:780-830）
    pub async fn add_value(
        &self,
        actor: &Actor,
        code: &str,
        input: EncodingSegmentValueInput,
    ) -> Result<EncodingSegmentDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let mut segment = self.load(code).await?;
            segment.stamp.check(input.version)?;
            segment.add_value(
                input.line,
                &input.value,
                input.name,
                &actor.user_id,
                today(),
            )?;
            self.save(&segment).await
        })
        .await
    }

    /// 修改單身一列的編碼編號與名稱（detail）
    pub async fn update_value(
        &self,
        actor: &Actor,
        code: &str,
        line: i32,
        input: EncodingSegmentValueInput,
    ) -> Result<EncodingSegmentDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut segment = self.load(code).await?;
            segment.stamp.check(input.version)?;
            segment.update_value(line, &input.value, input.name, &actor.user_id, today())?;
            self.save(&segment).await
        })
        .await
    }

    /// 刪除單身一列（detail；:832-860）
    pub async fn remove_value(
        &self,
        actor: &Actor,
        code: &str,
        line: i32,
        version: Option<i32>,
    ) -> Result<EncodingSegmentDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let mut segment = self.load(code).await?;
            segment.stamp.check(version)?;
            segment.remove_value(line, &actor.user_id, today())?;
            self.save(&segment).await
        })
        .await
    }

    /// 停用／啟用（invalid；:647-689）
    pub async fn set_active(
        &self,
        actor: &Actor,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<EncodingSegmentDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "invalid", async {
            self.authorize(actor, "invalid").await?;
            let mut segment = self.load(code).await?;
            segment.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            segment.set_active(active, &actor.user_id, today());
            self.save(&segment).await
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
    ) -> Result<EncodingSegmentDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let mut segment = self.load(code).await?;
            segment.stamp.check(version)?;
            segment.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&segment).await
        })
        .await
    }

    /// 複製單頭與單身（reproduce；:1192-1290）
    pub async fn copy(
        &self,
        actor: &Actor,
        code: &str,
        new_code: &str,
    ) -> Result<EncodingSegmentDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load(code).await?;
            let segment = source.copy_as(
                SegmentCode::parse(new_code)?,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            self.insert(&segment).await
        })
        .await
    }

    /// 刪除連同單身（:599-646）：已確認不可刪除（SA9-2）；已被編碼類別引用時拒絕（IS6，aooi402 移植時接上）
    pub async fn delete(
        &self,
        actor: &Actor,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            self.authorize(actor, "delete").await?;
            let segment = self.load(code).await?;
            segment.stamp.check(version)?;
            segment.confirmation.check_deletable()?;
            match self.segments.delete_encoding_segment(&segment.code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::EncodingSegmentNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::EncodingSegmentInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    async fn insert(&self, segment: &EncodingSegment) -> Result<EncodingSegmentDto, AppError> {
        match self.segments.insert_encoding_segment(segment).await {
            Ok(()) => Ok(to_dto(&self.load(segment.code.as_str()).await?)),
            Err(RepoError::Conflict) => Err(DomainError::EncodingSegmentAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, segment: &EncodingSegment) -> Result<EncodingSegmentDto, AppError> {
        self.segments
            .update_encoding_segment(segment)
            .await
            .map_err(infra)?;
        Ok(to_dto(&self.load(segment.code.as_str()).await?))
    }

    async fn load(&self, code: &str) -> Result<EncodingSegment, AppError> {
        let code = SegmentCode::parse(code).map_err(|_| DomainError::EncodingSegmentNotFound)?;
        self.segments
            .get_encoding_segment(&code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::EncodingSegmentNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn to_dto(s: &EncodingSegment) -> EncodingSegmentDto {
    EncodingSegmentDto {
        segment_code: s.code.as_str().to_owned(),
        name: s.attrs.name.clone(),
        length: s.attrs.length,
        values: s
            .values
            .iter()
            .map(|v| EncodingSegmentValueDto {
                line: v.line,
                value: v.value.clone(),
                name: v.name.clone(),
            })
            .collect(),
        active: s.active,
        confirmed: s.confirmation.confirmed,
        confirmed_by: s.confirmation.by.clone(),
        confirmed_at: s.confirmation.at,
        created_by: s.created_by.clone(),
        created_dept: s.created_dept.clone(),
        created_on: s.created_on,
        updated_by: s.updated_by.clone(),
        updated_on: s.updated_on,
        version: s.stamp.version,
        created_at: s.stamp.created_at,
        updated_at: s.stamp.updated_at,
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
