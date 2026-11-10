//! Classification Code Maintenance 分類碼維護 use case（TipTop: aooi300 / azf_file）
//! 規格：specs/features/aoo/classification_code.feature（CC1–CC9）
//!
//! 工廠層：每個操作都指定營運中心（ADR 0007，CC8）。TipTop 為多筆單身維護（aooi300.4gl:362-600），
//! 權限沿用單身：查詢 query、維護 detail（新增列 add_row、刪除列 delete_row）。TipTop 沒有設定資料所有者，不檢查資料權限。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    classification::{ClassificationCode, Code, CodeAttrs, CodeKind},
    plant::PlantId,
};
use ttrs_shared::{
    ChangeDto, ClassificationCodeDto, ClassificationCodeInput, Page, QbeCondition, QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    change_log::HistoryReader,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{ClassificationCodeRepository, MasterDataRepository},
    query,
    report::{self, ReportData, ReportFormat},
};

/// 清單報表（ADR 0013 SA3；classification_code_standard_actions.feature）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "code-report-title",
    columns: &[
        ("code", "code-col-code"),
        ("description", "code-col-description"),
        ("active", "code-col-active"),
    ],
};

/// 權限檢查用的程式代號（CC1：一支作業，以類別切換）
pub const PROGRAM: &str = "classification-code-maintenance";

#[derive(Clone)]
pub struct ClassificationCodeService {
    authz: Arc<dyn Authorizer>,
    codes: Arc<dyn ClassificationCodeRepository>,
    master: Arc<dyn MasterDataRepository>,
    history: HistoryReader,
    /// 列印、匯出 Excel（ADR 0013 SA3）
    reports: ListReporter,
}

impl ClassificationCodeService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        codes: Arc<dyn ClassificationCodeRepository>,
        master: Arc<dyn MasterDataRepository>,
        history: HistoryReader,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            codes,
            master,
            history,
            reports,
        }
    }

    /// QBE 查詢（ADR 0013 SA1）：同一類別內，依代號排序並分頁
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        req: QueryRequest,
    ) -> Result<Page<ClassificationCodeDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let kind = CodeKind::parse(kind)?;
        let filters = query::classification::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.codes.list_codes(&plant, kind).await.map_err(infra)?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::classification::value, p);
        Ok(Page {
            items: rows.iter().map(to_dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印 output、匯出 Excel exporttoexcel（ADR 0013 SA3；aooi300 原本沒有）：同一類別內
    #[allow(clippy::too_many_arguments)]
    pub async fn report_in(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let plant = PlantId::parse(plant)?;
        let kind = CodeKind::parse(kind)?;
        let filters = query::classification::filters(&conditions)?;
        let all = self.codes.list_codes(&plant, kind).await.map_err(infra)?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::classification::value, p);
        self.reports.check(total)?;
        let t = |id: &str| report::text(lang, id);
        let lines = rows
            .iter()
            .map(|c| {
                vec![
                    c.code.as_str().to_owned(),
                    c.attrs.description.clone().unwrap_or_default(),
                    t(if c.active {
                        "report-active"
                    } else {
                        "report-inactive"
                    }),
                ]
            })
            .collect();
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

    /// 測試用：調整同步產生報表的筆數上限
    pub fn with_report_limit(mut self, limit: u64) -> Self {
        self.reports.limit = limit;
        self
    }

    /// 複製（ADR 0013 SA5；aooi300 原本沒有）：同一類別的新代號
    pub async fn copy(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        code: &str,
        new_code: &str,
    ) -> Result<ClassificationCodeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let plant = PlantId::parse(plant)?;
            let source = self.load(&plant, kind, code).await?;
            let c = source.copy_as(Code::parse(new_code)?, &actor.user_id, &actor.dept, today());
            match self.codes.insert_code(&plant, &c).await {
                Ok(()) => Ok(to_dto(
                    &self.load(&plant, c.kind.code(), c.code.as_str()).await?,
                )),
                Err(RepoError::Conflict) => Err(DomainError::CodeAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        code: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<ClassificationCodeDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let plant = PlantId::parse(plant)?;
            let mut c = self.load(&plant, kind, code).await?;
            c.stamp.check(version)?;
            c.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&plant, &c).await
        })
        .await
    }

    /// 該類別的全部分類碼（TipTop 單身），依代號排序
    pub async fn list(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
    ) -> Result<Vec<ClassificationCodeDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let kind = CodeKind::parse(kind)?;
        let rows = self.codes.list_codes(&plant, kind).await.map_err(infra)?;
        Ok(rows.iter().map(to_dto).collect())
    }

    /// 新增（aooi300.4gl:362-424）：資料有效預設 Y；同一類別內代號不可重複
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        input: ClassificationCodeInput,
    ) -> Result<ClassificationCodeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let plant = PlantId::parse(plant)?;
            if !self.master.plant_active(&plant).await.map_err(infra)? {
                return Err(DomainError::PlantNotFound.into());
            }
            let kind = CodeKind::parse(kind)?;
            let code = Code::parse(&input.code)?;
            let attrs = attrs(kind, input)?;
            let c =
                ClassificationCode::new(kind, code, attrs, &actor.user_id, &actor.dept, today());
            match self.codes.insert_code(&plant, &c).await {
                Ok(()) => Ok(to_dto(
                    &self.load(&plant, c.kind.code(), c.code.as_str()).await?,
                )),
                Err(RepoError::Conflict) => Err(DomainError::CodeAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 修改說明等欄位；代號建立後不可修改（以網址的代號為準）
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        code: &str,
        input: ClassificationCodeInput,
    ) -> Result<ClassificationCodeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let mut c = self.load(&plant, kind, code).await?;
            c.stamp.check(input.version)?;
            c.update(attrs(c.kind, input)?, &actor.user_id, today());
            self.save(&plant, &c).await
        })
        .await
    }

    /// 資料有效（azfacti，aooi300.4gl:523-529）
    pub async fn set_active(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<ClassificationCodeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let mut c = self.load(&plant, kind, code).await?;
            c.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            c.set_active(active, &actor.user_id, today());
            self.save(&plant, &c).await
        })
        .await
    }

    /// 刪除（aooi300.4gl:531-548）：只刪該類別的代號（CC4）。
    /// 已被料件引用者不可刪除（CC5）——料件的分群欄位移植後由 repository 回 InUse
    pub async fn delete(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let plant = PlantId::parse(plant)?;
            let c = self.load(&plant, kind, code).await?;
            c.stamp.check(version)?;
            // 已確認不可刪除，須先取消確認（ADR 0013 SA9-2）
            c.confirmation.check_deletable()?;
            let (kind, code) = (c.kind, c.code);
            match self.codes.delete_code(&plant, kind, &code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::CodeNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::CodeInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    async fn load(
        &self,
        plant: &PlantId,
        kind: &str,
        code: &str,
    ) -> Result<ClassificationCode, AppError> {
        let kind = CodeKind::parse(kind)?;
        let code = Code::parse(code).map_err(|_| DomainError::CodeNotFound)?;
        self.codes
            .get_code(plant, kind, &code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::CodeNotFound.into())
    }

    async fn save(
        &self,
        plant: &PlantId,
        c: &ClassificationCode,
    ) -> Result<ClassificationCodeDto, AppError> {
        match self.codes.update_code(plant, c).await {
            Ok(()) => {}
            Err(RepoError::Stale) => return Err(DomainError::RecordChanged.into()),
            Err(e) => return Err(infra(e)),
        }
        Ok(to_dto(
            &self.load(plant, c.kind.code(), c.code.as_str()).await?,
        ))
    }

    /// 異動記錄（ADR 0012 §8，功能 history）
    pub async fn history(
        &self,
        actor: &Actor,
        plant: &str,
        kind: &str,
        code: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        self.authorize(actor, "history").await?;
        let plant = PlantId::parse(plant)?;
        let kind = CodeKind::parse(kind)?;
        let code = Code::parse(code).map_err(|_| DomainError::CodeNotFound)?;
        self.history
            .read(
                Some(&plant),
                &["classification_code"],
                &[("kind", kind.code()), ("code", code.as_str())],
                lang,
            )
            .await
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn attrs(kind: CodeKind, i: ClassificationCodeInput) -> Result<CodeAttrs, AppError> {
    Ok(CodeAttrs::new(
        kind,
        i.description,
        i.variance_account,
        i.variance_account2,
        i.cost_nature,
    )?)
}

fn to_dto(c: &ClassificationCode) -> ClassificationCodeDto {
    ClassificationCodeDto {
        kind: c.kind.code().to_owned(),
        code: c.code.as_str().to_owned(),
        description: c.attrs.description.clone(),
        variance_account: c.attrs.variance_account.clone(),
        variance_account2: c.attrs.variance_account2.clone(),
        cost_nature: c.attrs.cost_nature,
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
    AppError::Infra(e.to_string())
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_max_len_matches_domain() {
        use ttrs_domain::classification::max_len as d;
        use ttrs_shared::classification_max_len as s;
        assert_eq!(
            (d::CODE, d::DESCRIPTION, d::ACCOUNT),
            (s::CODE, s::DESCRIPTION, s::ACCOUNT)
        );
    }
}
