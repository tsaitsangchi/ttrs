//! 分類碼（TipTop: aooi300 共用程式 / azf_file）；工廠層，每廠的 schema 各一份（ADR 0007，CC8）
//! 規格：specs/features/aoo/classification_code.feature（CC1–CC9）
//!
//! 第一階段只開放料件用到的類別（CC2）：8 材料類別（aooi305，ima109）、D／E／F／G 其他分群碼一～四
//! （aooi309–aooi312，ima09–ima12）。

use chrono::NaiveDate;

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
    /// 碼類別（azf02），對應 TipTop 作業代號（aooi300.4gl:106-140）：
    /// 8 材料類別（aooi305）、D／E／F／G 其他分群碼一～四（aooi309–aooi312；G 另有存貨差異科目與成本性質）
    CodeKind { Material = "8", Group1 = "D", Group2 = "E", Group3 = "F", Group4 = "G" }
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
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
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
            created_by: actor.to_owned(),
            created_dept: actor_dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
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
            CodeKind::parse("2").unwrap_err().code(),
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
