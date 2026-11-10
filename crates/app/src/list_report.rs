//! 通用清單報表（docs/adr/0013-standard-maintenance-actions.md SA3）：各維護作業的列印 output、匯出 Excel exporttoexcel
//! 共用的組裝——表頭（營運中心名稱）、zz05 列印選擇條件、筆數上限、稽核紀錄。各作業只提供欄位與明細列。
//! 版面與輸出格式見 docs/adr/0011-reports-and-printing.md（PDF：Typst、Excel：rust_xlsxwriter）。

use std::sync::Arc;

use ttrs_domain::{
    lang::Lang,
    plant::PlantId,
    program::ProgramNo,
    report::{MAX_SYNC_ROWS, check_size},
};
use ttrs_shared::QbeCondition;

use crate::{
    Actor, AppError,
    error::RepoError,
    ports::{AuditEntry, AuditLog, PlantRepository, ProgramRepository},
    report::{self, ReportData, ReportFormat},
};

/// 一支作業的清單報表定義
pub struct ListSpec {
    /// 程式代號（檔名、稽核、zz05）
    pub program: &'static str,
    /// 報表名稱的報表文字 ID（crates/app/locales/<語系>/report.ftl）
    pub title: &'static str,
    /// （查詢欄位鍵值, 欄名的報表文字 ID）；欄名也用於列印條件說明
    pub columns: &'static [(&'static str, &'static str)],
}

#[derive(Clone)]
pub struct ListReporter {
    plants: Arc<dyn PlantRepository>,
    programs: Arc<dyn ProgramRepository>,
    audit: Arc<dyn AuditLog>,
    /// 同步產生報表的筆數上限（ADR 0011 RP6）
    pub limit: u64,
}

impl ListReporter {
    pub fn new(
        plants: Arc<dyn PlantRepository>,
        programs: Arc<dyn ProgramRepository>,
        audit: Arc<dyn AuditLog>,
    ) -> Self {
        Self {
            plants,
            programs,
            audit,
            limit: MAX_SYNC_ROWS,
        }
    }

    /// 查詢時多取一筆，用來判斷是否超過上限
    pub fn fetch_limit(&self) -> u64 {
        self.limit + 1
    }

    /// 符合條件的總筆數超過上限 → REPORT_TOO_LARGE
    pub fn check(&self, total: u64) -> Result<(), AppError> {
        Ok(check_size(total, self.limit)?)
    }

    /// 組成報表並寫入稽核紀錄（程式、使用者、營運中心、條件、格式、筆數）
    #[allow(clippy::too_many_arguments)]
    pub async fn build(
        &self,
        actor: &Actor,
        plant: &PlantId,
        spec: &ListSpec,
        rows: Vec<Vec<String>>,
        conditions: &[QbeCondition],
        format: ReportFormat,
        lang: &Lang,
    ) -> Result<ReportData, AppError> {
        let t = |id: &str| report::text(lang, id);
        // zz05 列印選擇條件：程式資料設定為是時，把查詢條件以欄名印在報表最後
        let print_conditions = match ProgramNo::parse(spec.program) {
            Ok(no) => self
                .programs
                .get(&no)
                .await
                .map_err(infra)?
                .is_some_and(|p| p.other.print_conditions),
            Err(_) => false,
        };
        let condition = if print_conditions {
            report::describe_conditions(conditions, |f| {
                spec.columns
                    .iter()
                    .find(|(k, _)| *k == f)
                    .map(|(_, id)| t(id))
            })
        } else {
            None
        };
        let plant_name = self
            .plants
            .get(plant)
            .await
            .map_err(infra)?
            .map(|p| p.attrs.name)
            .unwrap_or_default();
        let data = ReportData {
            program: spec.program.to_owned(),
            title: t(spec.title),
            plant: plant.as_str().to_owned(),
            plant_name,
            printed_by: actor.user_id.clone(),
            printed_at: chrono::Local::now().format("%Y/%m/%d %H:%M").to_string(),
            columns: spec.columns.iter().map(|(_, id)| t(id)).collect(),
            rows,
            condition,
            labels: report::labels(lang),
            lang: lang.clone(),
        };
        let all =
            report::describe_conditions(conditions, |f| Some(f.to_owned())).unwrap_or_default();
        self.audit
            .record(&AuditEntry {
                program: spec.program.to_owned(),
                user_id: actor.user_id.clone(),
                key: plant.as_str().to_owned(),
                action: format.action().to_owned(),
                detail: Some(format!(
                    "條件={all}；格式={}；筆數={}",
                    format.code(),
                    data.rows.len()
                )),
            })
            .await
            .map_err(infra)?;
        Ok(data)
    }
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}
