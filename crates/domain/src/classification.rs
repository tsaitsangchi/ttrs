//! 分類碼（TipTop: aooi300 共用程式 / azf_file）；工廠層，每廠的 schema 各一份（ADR 0007，CC8）
//! 規格：specs/features/aoo/classification_code.feature（CC1–CC9）
//!
//! 第一階段開放料件用到的類別（CC2）：8 材料類別（aooi305，ima109）、D／E／F／G 其他分群碼一～四
//! （aooi309–aooi312，ima09–ima12）；批次 1 加入客製 aooi080 的其他類別（OC1）：2 理由碼、6 檢驗碼、
//! A 保稅原料領用、R 業務請求狀態、S 業務處理結果（classification_code_other_kinds.feature）。

use chrono::NaiveDate;

use crate::record::Stamp;
use crate::{
    DomainError,
    lang::normalize_text,
    program::{check_length, coded_enum},
};

/// 欄位上限（azf_file.sch）
pub mod max_len {
    pub const CODE: usize = 10; // azf01
    pub const DESCRIPTION: usize = 80; // azf03
    pub const ACCOUNT: usize = 24; // azf05、azf051
}

coded_enum!(
    /// 碼類別（azf02），對應 TipTop 作業代號（aooi300.4gl:106-140、topcust/coo/4gl/aooi080.4gl:274-315）：
    /// 8 材料類別（aooi305）、D／E／F／G 其他分群碼一～四（aooi309–aooi312；G 另有存貨差異科目與成本性質）、
    /// 2 理由碼（aooi301）、6 檢驗碼（aooi304）、A 保稅原料領用（aooi306）、R 業務請求狀態、S 業務處理結果（客製 aooi080）。
    /// 3、5、H、Z 不開放（OC2）
    CodeKind {
        Material = "8",
        Group1 = "D",
        Group2 = "E",
        Group3 = "F",
        Group4 = "G",
        Reason = "2",
        Inspection = "6",
        BondedIssue = "A",
        RequestStatus = "R",
        ResultStatus = "S",
    }
);

impl CodeKind {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        Self::from_code(&raw.trim().to_uppercase())
            .ok_or_else(|| DomainError::CodeKindInvalid(raw.trim().to_owned()))
    }
}

/// 碼別代號（azf01）：必填、最多 10 字；同一類別內不可重複（aooi300.4gl:411-424）
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Code(String);

impl Code {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::CodeEmpty);
        }
        if v.chars().count() > max_len::CODE {
            return Err(DomainError::CodeTooLong { max: max_len::CODE });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 可修改的欄位；存貨差異科目與成本性質只有類別 G 使用（aooi300.4gl:197-220），
/// 會計科目尚未移植，先存代碼不檢查（CC3）
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CodeAttrs {
    pub description: Option<String>,
    /// azf05 存貨差異科目
    pub variance_account: Option<String>,
    /// azf051 存貨差異科目二
    pub variance_account2: Option<String>,
    /// azf06 成本性質
    pub cost_nature: Option<i32>,
}

impl CodeAttrs {
    pub fn new(
        kind: CodeKind,
        description: Option<String>,
        variance_account: Option<String>,
        variance_account2: Option<String>,
        cost_nature: Option<i32>,
    ) -> Result<Self, DomainError> {
        let description = normalize_text(description);
        check_length("說明", description.as_deref(), max_len::DESCRIPTION)?;
        if kind != CodeKind::Group4 {
            return Ok(Self {
                description,
                ..Self::default()
            });
        }
        let variance_account = normalize_text(variance_account);
        let variance_account2 = normalize_text(variance_account2);
        check_length(
            "存貨差異科目",
            variance_account.as_deref(),
            max_len::ACCOUNT,
        )?;
        check_length(
            "存貨差異科目二",
            variance_account2.as_deref(),
            max_len::ACCOUNT,
        )?;
        Ok(Self {
            description,
            variance_account,
            variance_account2,
            cost_nature,
        })
    }
}

/// 分類碼（azf_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassificationCode {
    pub kind: CodeKind,
    pub code: Code,
    pub attrs: CodeAttrs,
    /// azfacti 資料有效碼
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；azf_file 無此欄）
    pub confirmation: crate::confirmation::Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本與建立、修改時間（ADR 0012 §1）
    pub stamp: Stamp,
}

impl ClassificationCode {
    /// 新增：資料有效預設 Y（aooi300.4gl:367）；資料所有者／部門＝目前使用者
    pub fn new(
        kind: CodeKind,
        code: Code,
        attrs: CodeAttrs,
        actor: &str,
        actor_dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            kind,
            code,
            attrs,
            active: true,
            confirmation: crate::confirmation::Confirmation::default(),
            created_by: actor.to_owned(),
            created_dept: actor_dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn update(&mut self, attrs: CodeAttrs, actor: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(actor, today);
    }

    pub fn set_active(&mut self, active: bool, actor: &str, today: NaiveDate) {
        self.active = active;
        self.touch(actor, today);
    }

    /// 複製（ADR 0013 SA5；aooi300 原本沒有）：同一類別的新代號，其餘欄位帶入；資料所有者為目前使用者、未確認
    pub fn copy_as(&self, code: Code, actor: &str, actor_dept: &str, today: NaiveDate) -> Self {
        Self::new(
            self.kind,
            code,
            self.attrs.clone(),
            actor,
            actor_dept,
            today,
        )
    }

    /// 確認／取消確認（ADR 0013 SA9）
    pub fn set_confirmed(
        &mut self,
        confirm: bool,
        actor: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), DomainError> {
        if confirm {
            self.confirmation.confirm(actor, now)?;
        } else {
            self.confirmation.unconfirm()?;
        }
        self.touch(actor, now.date_naive());
        Ok(())
    }

    fn touch(&mut self, actor: &str, today: NaiveDate) {
        self.updated_by = Some(actor.to_owned());
        self.updated_on = Some(today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_and_code_rules() {
        assert_eq!(CodeKind::parse("d").unwrap(), CodeKind::Group1);
        assert_eq!(
            CodeKind::parse("3").unwrap_err().code(),
            "CODE_KIND_INVALID"
        );
        assert_eq!(Code::parse(" x01 ").unwrap().as_str(), "x01");
        assert_eq!(Code::parse("").unwrap_err().code(), "CODE_EMPTY");
        assert_eq!(
            Code::parse("ABCDEFGHIJK").unwrap_err().code(),
            "CODE_TOO_LONG"
        );
    }

    #[test]
    fn cost_fields_only_for_group4() {
        let a = CodeAttrs::new(
            CodeKind::Group1,
            Some("x".into()),
            Some("5101".into()),
            None,
            Some(1),
        )
        .unwrap();
        assert_eq!(a.variance_account, None);
        assert_eq!(a.cost_nature, None);
        let g = CodeAttrs::new(CodeKind::Group4, None, Some("5101".into()), None, Some(1)).unwrap();
        assert_eq!(g.variance_account.as_deref(), Some("5101"));
    }
}
