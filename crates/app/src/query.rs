//! 各維護作業的 QBE 查詢欄位（TipTop 各作業 CONSTRUCT 的欄位白名單）；條件語法見 ttrs_domain::qbe
//!
//! 程式維護（p_zz）見 program_query.rs。本檔：使用者 p_zx、營運中心 aooi901、權限類別 p_zw、程式執行權限 p_zy。
//! 欄位鍵值與該作業 DTO 的欄位名稱相同；infra 依鍵值對應資料庫欄位，記憶體版用各模組的 `value` 取值比對。

use ttrs_domain::{
    DomainError,
    qbe::{self, FieldType, QbeFilter, QbeValue},
};
use ttrs_shared::{QbeCondition, QueryRequest};

/// 每頁筆數上限
pub const MAX_PAGE_SIZE: u32 = 500;

/// 條件值的大小寫轉換：欄位一律存大寫或小寫時，條件跟著轉（UQ2、PQ2）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// 區分大小寫（QBE2）
    Exact,
    Upper,
    Lower,
}

/// 可查詢欄位
#[derive(Debug, Clone, Copy)]
pub struct Field {
    pub key: &'static str,
    pub ty: FieldType,
    pub case: Case,
}

const fn field(key: &'static str, ty: FieldType) -> Field {
    Field {
        key,
        ty,
        case: Case::Exact,
    }
}

const fn cased(key: &'static str, case: Case) -> Field {
    Field {
        key,
        ty: FieldType::Text,
        case,
    }
}

/// 把畫面條件轉成已解析的篩選；欄位不在白名單時回傳 `QbeFieldNotAllowed`。
/// `code_of(欄位, 值)` 把下拉欄位的顯示文字轉成代碼（同程式維護）
pub fn filters(
    conditions: &[QbeCondition],
    fields: &[Field],
    code_of: fn(&str, &str) -> String,
) -> Result<Vec<QbeFilter>, DomainError> {
    let mut out = Vec::new();
    for c in conditions {
        let f = fields
            .iter()
            .find(|f| f.key == c.field)
            .ok_or_else(|| DomainError::QbeFieldNotAllowed(c.field.clone()))?;
        let expr = match f.case {
            Case::Exact => c.expr.clone(),
            Case::Upper => c.expr.to_uppercase(),
            Case::Lower => c.expr.to_lowercase(),
        };
        if let Some(parsed) = qbe::parse(f.key, f.ty, &expr, |v| code_of(f.key, v))? {
            out.push(parsed);
        }
    }
    Ok(out)
}

/// 不轉換：條件值即存檔值
fn as_is(_field: &str, value: &str) -> String {
    value.to_owned()
}

/// 分頁：頁次從 1 開始；未給時第 1 頁、每頁 `QueryRequest::DEFAULT_PAGE_SIZE` 筆（QBE7）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paging {
    pub page: u32,
    pub page_size: u32,
    pub offset: u64,
}

pub fn paging(req: &QueryRequest) -> Paging {
    let page = req.page.unwrap_or(1).max(1);
    let page_size = req
        .page_size
        .unwrap_or(QueryRequest::DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    Paging {
        page,
        page_size,
        offset: u64::from(page - 1) * u64::from(page_size),
    }
}

/// 記憶體版比對用的欄位值
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    One(Option<QbeValue>),
    /// 多值欄位（允許執行功能，YQ4）
    Set(Vec<QbeValue>),
}

/// 記憶體版：全部篩選都符合（AND）
pub fn matches_all(filters: &[QbeFilter], value: impl Fn(&str) -> FieldValue) -> bool {
    filters.iter().all(|f| match value(f.field) {
        FieldValue::One(v) => f.matches(v.as_ref()),
        FieldValue::Set(vs) => f.matches_set(&vs),
    })
}

fn text(v: &str) -> FieldValue {
    FieldValue::One(Some(QbeValue::Text(v.to_owned())))
}

fn opt_text(v: Option<&str>) -> FieldValue {
    FieldValue::One(v.map(|s| QbeValue::Text(s.to_owned())))
}

fn flag(v: bool) -> FieldValue {
    FieldValue::One(Some(QbeValue::Flag(v)))
}

fn date(v: Option<chrono::NaiveDate>) -> FieldValue {
    FieldValue::One(v.map(QbeValue::Date))
}

/// 使用者維護（p_zx.4gl:118-121 CONSTRUCT；UQ1–UQ3）
pub mod user {
    use ttrs_domain::{qbe::FieldType, user::User};
    use ttrs_shared::QbeCondition;

    use super::*;

    /// CONSTRUCT 中有 TTRS 對應的 15 個欄位＋公司帳號（UQ1）
    pub const FIELDS: [Field; 16] = [
        field("user_id", FieldType::Text),
        field("name", FieldType::Text),
        cased("dept_code", Case::Upper),
        field("permission_class", FieldType::Text),
        field("menu", FieldType::Text),
        field("multi_plant", FieldType::Flag),
        cased("default_plant", Case::Upper),
        field("email", FieldType::Text),
        cased("idp_username", Case::Lower),
        field("valid_until", FieldType::Date),
        field("lang", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, &FIELDS, as_is)
    }

    pub fn value(u: &User, field: &str) -> FieldValue {
        let a = &u.attrs;
        match field {
            "user_id" => text(u.user_id.as_str()),
            "name" => opt_text(a.name.as_deref()),
            "dept_code" => opt_text(a.dept_code.as_deref()),
            "permission_class" => text(&a.permission_class),
            "menu" => text(&a.menu),
            "multi_plant" => flag(a.multi_plant),
            "default_plant" => text(a.default_plant.as_str()),
            "email" => opt_text(a.email.as_deref()),
            "idp_username" => opt_text(a.idp_username.as_deref()),
            "valid_until" => date(a.valid_until),
            "lang" => text(a.lang.as_str()),
            "active" => flag(u.active),
            "created_by" => text(&u.created_by),
            "created_dept" => text(&u.created_dept),
            "updated_by" => opt_text(u.updated_by.as_deref()),
            "updated_on" => date(u.updated_on),
            _ => FieldValue::One(None),
        }
    }
}

/// 營運中心維護（aooi901.4gl:122-124 CONSTRUCT；PQ1、PQ2）
pub mod plant {
    use ttrs_domain::{plant::Plant, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    /// CONSTRUCT 中有 TTRS 對應的 6 個欄位＋資料有效（PQ1）＋資料庫代碼（PQ5，存小寫）
    pub const FIELDS: [Field; 8] = [
        cased("plant_id", Case::Upper),
        cased("schema_name", Case::Lower),
        field("name", FieldType::Text),
        field("time_zone", FieldType::Text),
        field("address", FieldType::Text),
        field("phone", FieldType::Text),
        field("fax", FieldType::Text),
        field("active", FieldType::Flag),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, &FIELDS, as_is)
    }

    pub fn value(p: &Plant, field: &str) -> FieldValue {
        let a = &p.attrs;
        match field {
            "plant_id" => text(p.id.as_str()),
            "schema_name" => text(p.schema.as_str()),
            "name" => text(&a.name),
            "time_zone" => text(&a.time_zone),
            "address" => opt_text(a.address.as_deref()),
            "phone" => opt_text(a.phone.as_deref()),
            "fax" => opt_text(a.fax.as_deref()),
            "active" => flag(p.active),
            _ => FieldValue::One(None),
        }
    }
}

/// 權限類別維護（p_zw.4gl:192 CONSTRUCT；WQ1–WQ3）
pub mod permission_class {
    use ttrs_domain::{
        permission_class::{ClassIdleControl, PermissionClass},
        program::IdleAction,
        qbe::{FieldType, QbeValue},
    };
    use ttrs_shared::{ClassIdleControlDto, IdleActionDto, QbeCondition};

    use super::*;

    /// CONSTRUCT 的 7 個欄位（WQ1）
    pub const FIELDS: [Field; 7] = [
        field("class_code", FieldType::Text),
        field("name", FieldType::Text),
        field("default_menu", FieldType::Text),
        field("idle_control", FieldType::Text),
        field("idle_action", FieldType::Text),
        field("idle_seconds", FieldType::Integer),
        field("active", FieldType::Flag),
    ];

    const CONTROLS: [(ClassIdleControlDto, ClassIdleControl); 3] = [
        (ClassIdleControlDto::Off, ClassIdleControl::Off),
        (ClassIdleControlDto::ByClass, ClassIdleControl::ByClass),
        (ClassIdleControlDto::BySystem, ClassIdleControl::BySystem),
    ];
    const ACTIONS: [(IdleActionDto, IdleAction); 3] = [
        (IdleActionDto::WarnOnly, IdleAction::WarnOnly),
        (IdleActionDto::WarnThenClose, IdleAction::WarnThenClose),
        (IdleActionDto::Close, IdleAction::Close),
    ];

    /// 下拉欄位：畫面顯示文字 → 代碼；不是顯示文字就當作代碼本身（WQ2）
    fn code_of(field: &str, value: &str) -> String {
        let mapped = match field {
            "idle_control" => CONTROLS
                .iter()
                .find(|(d, _)| d.label() == value)
                .map(|(_, c)| c.code()),
            "idle_action" => ACTIONS
                .iter()
                .find(|(d, _)| d.label() == value)
                .map(|(_, a)| a.code()),
            _ => None,
        };
        mapped.map_or_else(|| value.to_owned(), str::to_owned)
    }

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, &FIELDS, code_of)
    }

    pub fn value(c: &PermissionClass, field: &str) -> FieldValue {
        let a = &c.attrs;
        match field {
            "class_code" => text(c.code.as_str()),
            "name" => text(&a.name),
            "default_menu" => opt_text(a.default_menu.as_deref()),
            "idle_control" => text(a.idle_control.code()),
            "idle_action" => opt_text(a.idle_action.map(IdleAction::code)),
            "idle_seconds" => {
                FieldValue::One(a.idle_seconds.map(|s| QbeValue::Integer(i64::from(s))))
            }
            "active" => flag(c.active),
            _ => FieldValue::One(None),
        }
    }
}

/// 程式執行權限維護（p_zy.4gl:154 CONSTRUCT；YQ1–YQ5）
pub mod program_permission {
    use ttrs_domain::{
        permission::ProgramPermission,
        qbe::{FieldType, QbeValue},
    };
    use ttrs_shared::QbeCondition;

    use super::*;

    /// 單頭（zy01）
    pub const HEADER: &str = "class_code";

    /// CONSTRUCT 的 6 個欄位（YQ1）：單頭權限類別＋單身 5 個欄位
    pub const FIELDS: [Field; 6] = [
        field(HEADER, FieldType::Text),
        field("program_no", FieldType::Text),
        field("actions", FieldType::Text),
        field("other_user_data", FieldType::Text),
        field("other_dept_data", FieldType::Text),
        field("detail_rows", FieldType::Text),
    ];

    /// 解析條件並分成（單頭, 單身）篩選
    pub fn filters(
        conditions: &[QbeCondition],
    ) -> Result<(Vec<QbeFilter>, Vec<QbeFilter>), DomainError> {
        Ok(super::filters(conditions, &FIELDS, as_is)?
            .into_iter()
            .partition(|f| f.field == HEADER))
    }

    /// 單身欄位值；允許執行功能逐一比對（YQ4）
    pub fn value(p: &ProgramPermission, field: &str) -> FieldValue {
        let a = &p.attrs;
        match field {
            HEADER => text(p.class.as_str()),
            "program_no" => text(p.program.as_str()),
            "actions" => FieldValue::Set(a.actions.iter().cloned().map(QbeValue::Text).collect()),
            "other_user_data" => text(a.other_user.code()),
            "other_dept_data" => text(a.other_dept.code()),
            "detail_rows" => text(a.detail.code()),
            _ => FieldValue::One(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cond(field: &str, expr: &str) -> QbeCondition {
        QbeCondition {
            field: field.into(),
            expr: expr.into(),
        }
    }

    #[test]
    fn case_conversion_and_whitelist() {
        let f = plant::filters(&[cond("plant_id", "tw*")]).unwrap();
        assert!(f[0].matches(Some(&QbeValue::Text("TW01".into()))));
        let err = plant::filters(&[cond("database_code", "x")]).unwrap_err();
        assert_eq!(err.code(), "QBE_FIELD_NOT_ALLOWED");
    }

    #[test]
    fn idle_label_or_code() {
        for expr in ["依權限類別", "2"] {
            let f = permission_class::filters(&[cond("idle_control", expr)]).unwrap();
            assert!(f[0].matches(Some(&QbeValue::Text("2".into()))), "{expr}");
        }
    }

    #[test]
    fn header_and_detail_split() {
        let (h, d) = program_permission::filters(&[
            cond("class_code", "A*"),
            cond("program_no", "item-maintenance"),
        ])
        .unwrap();
        assert_eq!((h.len(), d.len()), (1, 1));
    }

    #[test]
    fn paging_defaults() {
        let p = paging(&QueryRequest::default());
        assert_eq!((p.page, p.page_size, p.offset), (1, 50, 0));
    }
}

/// 部門維護（aooi030，DQ8）：CONSTRUCT 的 8 個欄位（aooi030.4gl:888-891）
pub mod department {
    use ttrs_domain::{department::Department, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: [Field; 8] = [
        cased("dept_code", Case::Upper),
        field("name", FieldType::Text),
        field("full_name", FieldType::Text),
        field("accounting_dept", FieldType::Flag),
        cased("expense_type", Case::Upper),
        field("control_type", FieldType::Text),
        cased("cost_center", Case::Upper),
        field("active", FieldType::Flag),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, &FIELDS, as_is)
    }

    pub fn value(d: &Department, field: &str) -> FieldValue {
        let a = &d.accounting;
        match field {
            "dept_code" => text(d.code.as_str()),
            "name" => text(&d.attrs.name),
            "full_name" => opt_text(d.attrs.full_name.as_deref()),
            "accounting_dept" => flag(a.accounting_dept),
            "expense_type" => opt_text(a.expense_type.map(|e| e.code())),
            "control_type" => text(a.control_type.code()),
            "cost_center" => opt_text(a.cost_center.as_ref().map(|c| c.as_str())),
            "active" => flag(d.active),
            _ => FieldValue::One(None),
        }
    }
}

/// 員工維護（aooi040，EQ8）：CONSTRUCT 的 7 個欄位（aooi040.4gl:628-630）
pub mod employee {
    use ttrs_domain::{employee::Employee, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: [Field; 7] = [
        cased("employee_id", Case::Upper),
        field("name", FieldType::Text),
        cased("dept_code", Case::Upper),
        field("title", FieldType::Text),
        field("extension", FieldType::Text),
        field("email", FieldType::Text),
        field("active", FieldType::Flag),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, &FIELDS, as_is)
    }

    pub fn value(e: &Employee, field: &str) -> FieldValue {
        let a = &e.attrs;
        match field {
            "employee_id" => text(e.id.as_str()),
            "name" => text(&a.name),
            "dept_code" => opt_text(a.dept_code.as_ref().map(|d| d.as_str())),
            "title" => opt_text(a.title.as_deref()),
            "extension" => opt_text(a.extension.as_deref()),
            "email" => opt_text(a.email.as_deref()),
            "active" => flag(e.active),
            _ => FieldValue::One(None),
        }
    }
}
