//! 報表資料（docs/adr/0011-reports-and-printing.md）：與輸出格式無關，由 infra 轉成 PDF（Typst）或 Excel。
//! 報表文字依介面語系產生（RP12），放 `crates/app/locales/<語系>/report.ftl`，找不到時退回 zh-TW。

use ttrs_domain::lang::{DEFAULT_LANG, Lang};
use ttrs_shared::QbeCondition;

/// 輸出格式；決定檢查的功能代號（列印 output、匯出 exporttoexcel，ADR 0011 RP9）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Pdf,
    Excel,
}

impl ReportFormat {
    pub fn action(self) -> &'static str {
        match self {
            Self::Pdf => "output",
            Self::Excel => "exporttoexcel",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Excel => "xlsx",
        }
    }
}

/// 報表固定文字（表頭、頁次、條件）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportLabels {
    pub plant: String,
    pub printed_by: String,
    pub printed_at: String,
    /// 含 {page}、{total} 的頁次格式
    pub page: String,
    pub condition: String,
}

/// 一份報表：表頭、欄名、明細（皆為顯示用文字）、查詢條件說明
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportData {
    /// 程式代號（檔名、稽核）
    pub program: String,
    pub title: String,
    pub plant: String,
    pub plant_name: String,
    pub printed_by: String,
    /// 列印時間（YYYY/MM/DD HH:MM）
    pub printed_at: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// zz05＝Y 且有條件時的查詢條件說明（例如「部門編號 = S*」）
    pub condition: Option<String>,
    pub labels: ReportLabels,
    pub lang: Lang,
}

const REPORT_FTL: [(&str, &str); 3] = [
    ("zh-TW", include_str!("../locales/zh-TW/report.ftl")),
    ("en", include_str!("../locales/en/report.ftl")),
    ("zh-CN", include_str!("../locales/zh-CN/report.ftl")),
];

/// 報表文字；找不到時退回 zh-TW，再找不到回傳 ID
pub fn text(lang: &Lang, id: &str) -> String {
    let find = |l: &str| {
        let (_, src) = REPORT_FTL.iter().find(|(x, _)| *x == l)?;
        crate::mail::parse(src)
            .into_iter()
            .find(|(k, _)| k == id)
            .map(|(_, v)| v)
    };
    find(lang.as_str())
        .or_else(|| find(DEFAULT_LANG))
        .unwrap_or_else(|| id.to_owned())
}

pub fn labels(lang: &Lang) -> ReportLabels {
    ReportLabels {
        plant: text(lang, "report-label-plant"),
        printed_by: text(lang, "report-label-printed-by"),
        printed_at: text(lang, "report-label-printed-at"),
        page: text(lang, "report-label-page")
            .replace("{ $page }", "{page}")
            .replace("{ $total }", "{total}"),
        condition: text(lang, "report-label-condition"),
    }
}

/// 查詢條件說明（TipTop cl_wcchp 把欄位代號換成欄位名稱）：「欄位名稱 = 條件」，以「，」分隔；
/// 條件本身以比較符號開頭時不再加「=」
pub fn describe_conditions(
    conditions: &[QbeCondition],
    label_of: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let parts: Vec<String> = conditions
        .iter()
        .filter(|c| !c.expr.trim().is_empty())
        .map(|c| {
            let label = label_of(&c.field).unwrap_or_else(|| c.field.clone());
            let expr = c.expr.trim();
            if expr.starts_with(['=', '<', '>', '!']) {
                format!("{label} {expr}")
            } else {
                format!("{label} = {expr}")
            }
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join("，"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(src: &str) -> Vec<String> {
        crate::mail::parse(src)
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    }

    #[test]
    fn locales_have_same_ids() {
        let base = ids(REPORT_FTL[0].1);
        for (lang, src) in &REPORT_FTL[1..] {
            assert_eq!(ids(src), base, "{lang} 的報表文字 ID 與 zh-TW 不一致");
        }
    }

    #[test]
    fn describes_conditions() {
        let c = |f: &str, e: &str| QbeCondition {
            field: f.into(),
            expr: e.into(),
        };
        let label = |f: &str| (f == "dept_code").then(|| "部門編號".to_owned());
        assert_eq!(
            describe_conditions(&[c("dept_code", "S*")], label).as_deref(),
            Some("部門編號 = S*")
        );
        assert_eq!(
            describe_conditions(&[c("dept_code", ">=M")], label).as_deref(),
            Some("部門編號 >=M")
        );
        assert_eq!(describe_conditions(&[], label), None);
    }

    #[test]
    fn page_label_has_placeholders() {
        let l = labels(&Lang::parse("en").unwrap());
        assert_eq!(l.page, "Page {page} of {total}");
    }
}
