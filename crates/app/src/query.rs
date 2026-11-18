//! 各維護作業的 QBE 查詢欄位（TipTop 各作業 CONSTRUCT 的欄位白名單）；條件語法見 ttrs_domain::qbe
//!
//! 程式維護（p_zz）見 program_query.rs。本檔：使用者 p_zx、營運中心 aooi901、權限類別 p_zw、程式執行權限 p_zy。
//! 欄位鍵值與該作業 DTO 的欄位名稱相同；infra 依鍵值對應資料庫欄位，記憶體版用各模組的 `value` 取值比對。

use ttrs_domain::{
    DomainError,
    qbe::{self, FieldType, QbeFilter, QbeTerm, QbeValue},
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
    /// 多語系欄位（料件品名、規格，IQ3）：繁中本身＋各語系翻譯；任一語系符合即符合，空值只看繁中
    Translated(Option<QbeValue>, Vec<QbeValue>),
}

/// 記憶體版：全部篩選都符合（AND）
pub fn matches_all(filters: &[QbeFilter], value: impl Fn(&str) -> FieldValue) -> bool {
    filters.iter().all(|f| match value(f.field) {
        FieldValue::One(v) => f.matches(v.as_ref()),
        FieldValue::Set(vs) => f.matches_set(&vs),
        FieldValue::Translated(base, texts) => f.terms.iter().any(|t| match t {
            QbeTerm::IsNull => t.matches(
                base.as_ref()
                    .filter(|v| !matches!(v, QbeValue::Text(s) if s.is_empty())),
            ),
            _ => base.iter().chain(texts.iter()).any(|v| t.matches(Some(v))),
        }),
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

fn opt_dec(v: Option<rust_decimal::Decimal>) -> FieldValue {
    FieldValue::One(v.map(QbeValue::Decimal))
}

fn opt_int(v: Option<i32>) -> FieldValue {
    FieldValue::One(v.map(|n| QbeValue::Integer(i64::from(n))))
}

fn int(v: i32) -> FieldValue {
    FieldValue::One(Some(QbeValue::Integer(i64::from(v))))
}

/// 狀態欄位的值（記憶體版比對）；不是狀態欄位時為 None
#[allow(clippy::too_many_arguments)]
fn status_value(
    field: &str,
    active: Option<bool>,
    created_by: &str,
    created_dept: &str,
    created_on: Option<chrono::NaiveDate>,
    updated_by: Option<&str>,
    updated_on: Option<chrono::NaiveDate>,
    version: i32,
) -> Option<FieldValue> {
    Some(match field {
        "active" => FieldValue::One(active.map(QbeValue::Flag)),
        "created_by" => text(created_by),
        "created_dept" => text(created_dept),
        "created_on" => date(created_on),
        "updated_by" => opt_text(updated_by),
        "updated_on" => date(updated_on),
        "version" => int(version),
        _ => return None,
    })
}

/// 使用者維護（p_zx.4gl:118-121 CONSTRUCT；UQ1–UQ3）
pub mod user {
    use ttrs_domain::{qbe::FieldType, user::User};
    use ttrs_shared::QbeCondition;

    use super::*;

    /// CONSTRUCT 中有 TTRS 對應的 15 個欄位＋公司帳號（UQ1）
    pub const FIELDS: [Field; 20] = [
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
        // 確認（ADR 0013 SA9）
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("created_on", FieldType::Date),
        field("version", FieldType::Integer),
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
            "confirmed" => flag(u.confirmation.confirmed),
            "confirmed_by" => opt_text(u.confirmation.by.as_deref()),
            "active" => flag(u.active),
            "created_by" => text(&u.created_by),
            "created_dept" => text(&u.created_dept),
            "updated_by" => opt_text(u.updated_by.as_deref()),
            "updated_on" => date(u.updated_on),
            // 狀態頁籤的欄位（ADR 0012 §9）
            other => status_value(
                other,
                Some(u.active),
                &u.created_by,
                &u.created_dept,
                Some(u.created_on),
                u.updated_by.as_deref(),
                u.updated_on,
                u.stamp.version,
            )
            .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 營運中心維護（aooi901.4gl:122-124 CONSTRUCT；PQ1、PQ2）
pub mod plant {
    use ttrs_domain::{plant::Plant, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    /// CONSTRUCT 中有 TTRS 對應的 6 個欄位＋資料有效（PQ1）＋資料庫代碼（PQ5，存小寫）
    pub const FIELDS: [Field; 16] = [
        cased("plant_id", Case::Upper),
        cased("schema_name", Case::Lower),
        field("name", FieldType::Text),
        field("time_zone", FieldType::Text),
        field("address", FieldType::Text),
        field("phone", FieldType::Text),
        field("fax", FieldType::Text),
        // 確認（ADR 0013 SA9）
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
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
            "confirmed" => flag(p.confirmation.confirmed),
            "confirmed_by" => opt_text(p.confirmation.by.as_deref()),
            "active" => flag(p.active),
            // 狀態頁籤的欄位（ADR 0012 §9）
            other => status_value(
                other,
                Some(p.active),
                &p.created_by,
                &p.created_dept,
                Some(p.created_on),
                p.updated_by.as_deref(),
                p.updated_on,
                p.stamp.version,
            )
            .unwrap_or(FieldValue::One(None)),
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
    pub const FIELDS: [Field; 15] = [
        field("class_code", FieldType::Text),
        field("name", FieldType::Text),
        field("default_menu", FieldType::Text),
        field("idle_control", FieldType::Text),
        field("idle_action", FieldType::Text),
        field("idle_seconds", FieldType::Integer),
        // 確認（ADR 0013 SA9）
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
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
            "confirmed" => flag(c.confirmation.confirmed),
            "confirmed_by" => opt_text(c.confirmation.by.as_deref()),
            "active" => flag(c.active),
            // 狀態頁籤的欄位（ADR 0012 §9）
            other => status_value(
                other,
                Some(c.active),
                &c.created_by,
                &c.created_dept,
                Some(c.created_on),
                c.updated_by.as_deref(),
                c.updated_on,
                c.stamp.version,
            )
            .unwrap_or(FieldValue::One(None)),
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
    pub const FIELDS: [Field; 12] = [
        field(HEADER, FieldType::Text),
        field("program_no", FieldType::Text),
        field("actions", FieldType::Text),
        field("other_user_data", FieldType::Text),
        field("other_dept_data", FieldType::Text),
        field("detail_rows", FieldType::Text),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
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
            // 狀態頁籤的欄位（ADR 0012 §9）
            other => status_value(
                other,
                None,
                &p.created_by,
                &p.created_dept,
                Some(p.created_on),
                p.updated_by.as_deref(),
                p.updated_on,
                p.stamp.version,
            )
            .unwrap_or(FieldValue::One(None)),
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

    pub const FIELDS: [Field; 16] = [
        cased("dept_code", Case::Upper),
        field("name", FieldType::Text),
        field("full_name", FieldType::Text),
        field("accounting_dept", FieldType::Flag),
        cased("expense_type", Case::Upper),
        field("control_type", FieldType::Text),
        cased("cost_center", Case::Upper),
        // 確認（ADR 0013 SA9）
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
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
            "confirmed" => flag(d.confirmation.confirmed),
            "confirmed_by" => opt_text(d.confirmation.by.as_deref()),
            "active" => flag(d.active),
            // 狀態頁籤的欄位（ADR 0012 §9）
            other => status_value(
                other,
                Some(d.active),
                &d.created_by,
                &d.created_dept,
                Some(d.created_on),
                d.updated_by.as_deref(),
                d.updated_on,
                d.stamp.version,
            )
            .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 員工維護（aooi040，EQ8）：CONSTRUCT 的 7 個欄位（aooi040.4gl:628-630）
pub mod employee {
    use ttrs_domain::{employee::Employee, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: [Field; 15] = [
        cased("employee_id", Case::Upper),
        field("name", FieldType::Text),
        cased("dept_code", Case::Upper),
        field("title", FieldType::Text),
        field("extension", FieldType::Text),
        field("email", FieldType::Text),
        // 確認（ADR 0013 SA9）
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
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
            "confirmed" => flag(e.confirmation.confirmed),
            "confirmed_by" => opt_text(e.confirmation.by.as_deref()),
            "active" => flag(e.active),
            // 狀態頁籤的欄位（ADR 0012 §9）
            other => status_value(
                other,
                Some(e.active),
                &e.created_by,
                &e.created_dept,
                Some(e.created_on),
                e.updated_by.as_deref(),
                e.updated_on,
                e.stamp.version,
            )
            .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 料件基本資料維護（aimi100 i100_curs，topcust/cim/4gl/aimi100.4gl:354-656；IQ1–IQ10）
pub mod item {
    use ttrs_domain::{
        item::{Item, ItemText},
        qbe::{FieldType, QbeValue},
    };
    use ttrs_shared::QbeCondition;

    use super::*;

    /// CONSTRUCT 中有 TTRS 對應的欄位＋「狀態」頁籤（IQ1）；多屬性、批號序號、自訂欄位不在其中（IQ10）。
    /// 料件編號區分大小寫（IQ2）
    pub const FIELDS: &[Field] = &[
        field("item_no", FieldType::Text),
        field("name", FieldType::Text),
        field("spec", FieldType::Text),
        field("group_code", FieldType::Text),
        field("source_code", FieldType::Text),
        field("configurable_item_no", FieldType::Text),
        field("current_version", FieldType::Text),
        field("catalog_ref", FieldType::Text),
        field("supplier_control", FieldType::Text),
        field("product_class", FieldType::Text),
        field("drawing_no", FieldType::Text),
        field("apply_dept", FieldType::Text),
        field("software", FieldType::Flag),
        field("engineering", FieldType::Flag),
        field("insert_position", FieldType::Flag),
        field("insert_qpa_check", FieldType::Flag),
        field("material_category", FieldType::Text),
        field("coproduct", FieldType::Flag),
        field("inspection", FieldType::Flag),
        field("repetitive", FieldType::Flag),
        field("pcn", FieldType::Text),
        field("abc_code", FieldType::Text),
        field("consumable", FieldType::Flag),
        field("replenish_policy", FieldType::Text),
        field("economic_order_qty", FieldType::Decimal),
        field("average_order_qty", FieldType::Decimal),
        field("safety_stock", FieldType::Decimal),
        field("safety_period", FieldType::Decimal),
        field("max_stock_qty", FieldType::Decimal),
        field("shelf_life_days", FieldType::Integer),
        field("stock_unit", FieldType::Text),
        field("warehouse", FieldType::Text),
        field("location", FieldType::Text),
        field("stock_keeper", FieldType::Text),
        field("second_unit", FieldType::Text),
        field("pricing_unit", FieldType::Text),
        field("other_group1", FieldType::Text),
        field("other_group2", FieldType::Text),
        field("other_group3", FieldType::Text),
        field("other_group4", FieldType::Text),
        field("account", FieldType::Text),
        field("account2", FieldType::Text),
        field("bonded", FieldType::Flag),
        field("full_name", FieldType::Text),
        field("english_name", FieldType::Text),
        field("planned_discontinue_on", FieldType::Date),
        field("life_cycle", FieldType::Text),
        field("carton_barcode", FieldType::Text),
        field("active_code", FieldType::Text),
        field("status_code", FieldType::Text),
        field("order_invalid", FieldType::Flag),
        field("phase_out", FieldType::Flag),
        field("low_level_code", FieldType::Integer),
        field("bom_changed", FieldType::Text),
        field("coproduct_this_month", FieldType::Flag),
        field("new_part_status", FieldType::Text),
        field("data_source", FieldType::Text),
        field("item_created_on", FieldType::Date),
        field("first_shipped_on", FieldType::Date),
        field("first_received_on", FieldType::Date),
        field("slow_moving_on", FieldType::Date),
        field("last_period_purchase_on", FieldType::Date),
        field("last_received_on", FieldType::Date),
        field("last_issued_on", FieldType::Date),
        field("last_txn_on", FieldType::Date),
        field("last_count_on", FieldType::Date),
        // 狀態頁籤（ADR 0012 §9）
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    fn translated(
        base: Option<&str>,
        texts: &[ItemText],
        pick: fn(&ItemText) -> Option<&str>,
    ) -> FieldValue {
        FieldValue::Translated(
            base.map(|s| QbeValue::Text(s.to_owned())),
            texts
                .iter()
                .filter_map(pick)
                .map(|s| QbeValue::Text(s.to_owned()))
                .collect(),
        )
    }

    /// 記憶體版比對用；`texts` 為各語系品名、規格（IQ3）
    pub fn value(i: &Item, texts: &[ItemText], field: &str) -> FieldValue {
        let a = &i.attrs;
        let s = &i.system;
        let t = |v: &Option<String>| opt_text(v.as_deref());
        match field {
            "item_no" => text(i.no.as_str()),
            "name" => translated(a.name.as_deref(), texts, |x| x.name.as_deref()),
            "spec" => translated(a.spec.as_deref(), texts, |x| x.spec.as_deref()),
            "group_code" => t(&a.group_code),
            "source_code" => t(&a.source_code),
            "configurable_item_no" => t(&a.configurable_item_no),
            "current_version" => t(&a.current_version),
            "catalog_ref" => t(&a.catalog_ref),
            "supplier_control" => t(&a.supplier_control),
            "product_class" => t(&a.product_class),
            "drawing_no" => t(&a.drawing_no),
            "apply_dept" => t(&a.apply_dept),
            "software" => flag(a.software),
            "engineering" => flag(a.engineering),
            "insert_position" => flag(a.insert_position),
            "insert_qpa_check" => flag(a.insert_qpa_check),
            "material_category" => t(&a.material_category),
            "coproduct" => flag(a.coproduct),
            "inspection" => flag(a.inspection),
            "repetitive" => flag(a.repetitive),
            "pcn" => t(&a.pcn),
            "abc_code" => t(&a.abc_code),
            "consumable" => flag(a.consumable),
            "replenish_policy" => t(&a.replenish_policy),
            "economic_order_qty" => opt_dec(a.economic_order_qty),
            "average_order_qty" => opt_dec(a.average_order_qty),
            "safety_stock" => opt_dec(a.safety_stock),
            "safety_period" => opt_dec(a.safety_period),
            "max_stock_qty" => opt_dec(a.max_stock_qty),
            "shelf_life_days" => opt_int(a.shelf_life_days),
            "stock_unit" => t(&a.stock_unit),
            "warehouse" => t(&a.warehouse),
            "location" => t(&a.location),
            "stock_keeper" => t(&a.stock_keeper),
            "second_unit" => t(&a.second_unit),
            "pricing_unit" => t(&a.pricing_unit),
            "other_group1" => t(&a.other_groups[0]),
            "other_group2" => t(&a.other_groups[1]),
            "other_group3" => t(&a.other_groups[2]),
            "other_group4" => t(&a.other_groups[3]),
            "account" => t(&a.account),
            "account2" => t(&a.account2),
            "bonded" => flag(a.bonded),
            "full_name" => t(&a.full_name),
            "english_name" => t(&a.english_name),
            "planned_discontinue_on" => date(a.planned_discontinue_on),
            "life_cycle" => t(&a.life_cycle),
            "carton_barcode" => t(&a.carton_barcode),
            "active_code" => text(i.active_code.code()),
            "status_code" => text(i.status_code.code()),
            "order_invalid" => flag(i.order_invalid),
            "phase_out" => flag(s.phase_out),
            "low_level_code" => int(s.low_level_code),
            "bom_changed" => t(&s.bom_changed),
            "coproduct_this_month" => flag(s.coproduct_this_month),
            "new_part_status" => text(&s.new_part_status),
            "data_source" => text(&s.data_source),
            "item_created_on" => date(Some(s.item_created_on)),
            "first_shipped_on" => date(s.first_shipped_on),
            "first_received_on" => date(s.first_received_on),
            "slow_moving_on" => date(s.slow_moving_on),
            "last_period_purchase_on" => date(s.last_period_purchase_on),
            "last_received_on" => date(s.last_received_on),
            "last_issued_on" => date(s.last_issued_on),
            "last_txn_on" => date(s.last_txn_on),
            "last_count_on" => date(s.last_count_on),
            other => status_value(
                other,
                Some(i.active()),
                &i.created_by,
                &i.created_dept,
                Some(i.created_on),
                i.updated_by.as_deref(),
                i.updated_on,
                i.stamp.version,
            )
            .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 料件分群維護（aimi110_curs，tiptop/aim/4gl/aimi110.4gl:194-230；GQ1–GQ3）
pub mod item_group {
    use ttrs_domain::{item_group::ItemGroup, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    /// CONSTRUCT 中有 TTRS 對應的欄位＋「狀態」頁籤（GQ1）；ICD、批號序號、單位使用方式、自訂欄位不在其中（GQ3）
    pub const FIELDS: &[Field] = &[
        field("group_code", FieldType::Text),
        field("description", FieldType::Text),
        field("source_code", FieldType::Text),
        field("stock_unit", FieldType::Text),
        field("catalog_ref", FieldType::Text),
        field("supplier_control", FieldType::Text),
        field("drawing_no", FieldType::Text),
        field("software", FieldType::Flag),
        field("engineering", FieldType::Flag),
        field("insert_qpa_check", FieldType::Flag),
        field("insert_position", FieldType::Flag),
        field("material_category", FieldType::Text),
        field("coproduct", FieldType::Flag),
        field("inspection", FieldType::Flag),
        field("repetitive", FieldType::Flag),
        field("abc_code", FieldType::Text),
        field("consumable", FieldType::Flag),
        field("replenish_policy", FieldType::Text),
        field("reorder_point", FieldType::Decimal),
        field("reorder_qty", FieldType::Decimal),
        field("safety_stock", FieldType::Decimal),
        field("safety_period", FieldType::Decimal),
        field("shelf_life_days", FieldType::Integer),
        field("mrp_bucket_days", FieldType::Integer),
        field("warehouse", FieldType::Text),
        field("location", FieldType::Text),
        field("stock_keeper", FieldType::Text),
        field("sales_stat_unit", FieldType::Text),
        field("second_unit", FieldType::Text),
        field("pricing_unit", FieldType::Text),
        field("other_group1", FieldType::Text),
        field("other_group2", FieldType::Text),
        field("other_group3", FieldType::Text),
        field("other_group4", FieldType::Text),
        field("account", FieldType::Text),
        field("account2", FieldType::Text),
        field("bonded", FieldType::Flag),
        field("bonded_class", FieldType::Text),
        field("tariff_no", FieldType::Text),
        field("bonded_type", FieldType::Text),
        field("product_class", FieldType::Text),
        field("economic_order_qty", FieldType::Decimal),
        field("average_order_qty", FieldType::Decimal),
        // 確認（ADR 0013 SA9）
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        // 狀態頁籤（ADR 0012 §9）
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(g: &ItemGroup, field: &str) -> FieldValue {
        let a = &g.attrs;
        let t = |v: &Option<String>| opt_text(v.as_deref());
        match field {
            "group_code" => text(g.code.as_str()),
            "description" => t(&a.description),
            "source_code" => t(&a.source_code),
            "stock_unit" => t(&a.stock_unit),
            "catalog_ref" => t(&a.catalog_ref),
            "supplier_control" => text(&a.supplier_control),
            "drawing_no" => t(&a.drawing_no),
            "software" => flag(a.software),
            "engineering" => flag(a.engineering),
            "insert_qpa_check" => flag(a.insert_qpa_check),
            "insert_position" => flag(a.insert_position),
            "material_category" => t(&a.material_category),
            "coproduct" => flag(a.coproduct),
            "inspection" => flag(a.inspection),
            "repetitive" => flag(a.repetitive),
            "abc_code" => t(&a.abc_code),
            "consumable" => flag(a.consumable),
            "replenish_policy" => t(&a.replenish_policy),
            "reorder_point" => opt_dec(a.reorder_point),
            "reorder_qty" => opt_dec(a.reorder_qty),
            "safety_stock" => opt_dec(a.safety_stock),
            "safety_period" => opt_dec(a.safety_period),
            "shelf_life_days" => opt_int(a.shelf_life_days),
            "mrp_bucket_days" => opt_int(a.mrp_bucket_days),
            "warehouse" => t(&a.warehouse),
            "location" => t(&a.location),
            "stock_keeper" => t(&a.stock_keeper),
            "sales_stat_unit" => t(&a.sales_stat_unit),
            "second_unit" => t(&a.second_unit),
            "pricing_unit" => t(&a.pricing_unit),
            "other_group1" => t(&a.other_groups[0]),
            "other_group2" => t(&a.other_groups[1]),
            "other_group3" => t(&a.other_groups[2]),
            "other_group4" => t(&a.other_groups[3]),
            "account" => t(&a.account),
            "account2" => t(&a.account2),
            "bonded" => flag(a.bonded),
            "bonded_class" => t(&a.bonded_class),
            "tariff_no" => t(&a.tariff_no),
            "bonded_type" => t(&a.bonded_type),
            "product_class" => t(&a.product_class),
            "economic_order_qty" => opt_dec(a.economic_order_qty),
            "average_order_qty" => opt_dec(a.average_order_qty),
            "confirmed" => flag(g.confirmation.confirmed),
            "confirmed_by" => opt_text(g.confirmation.by.as_deref()),
            other => status_value(
                other,
                Some(g.active),
                &g.created_by,
                &g.created_dept,
                Some(g.created_on),
                g.updated_by.as_deref(),
                g.updated_on,
                g.stamp.version,
            )
            .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 資料量小的作業（單位、產品分類、分類碼）在 app 層對完整清單做 QBE 過濾與分頁（ADR 0013 SA1）：
/// 記憶體與 PostgreSQL 兩種後端共用同一份比對規則。`items` 須已依鍵值（位元組順序）排序
pub fn page_in_memory<T: Clone>(
    items: &[T],
    filters: &[QbeFilter],
    value: impl Fn(&T, &str) -> FieldValue,
    p: Paging,
) -> (Vec<T>, u64) {
    let hits: Vec<T> = items
        .iter()
        .filter(|x| matches_all(filters, |f| value(x, f)))
        .cloned()
        .collect();
    let total = hits.len() as u64;
    let rows = hits
        .into_iter()
        .skip(usize::try_from(p.offset).unwrap_or(usize::MAX))
        .take(p.page_size as usize)
        .collect();
    (rows, total)
}

/// 確認欄位的值（ADR 0013 SA9）；不是確認欄位時為 None
fn confirm_value(field: &str, c: &ttrs_domain::confirmation::Confirmation) -> Option<FieldValue> {
    Some(match field {
        "confirmed" => flag(c.confirmed),
        "confirmed_by" => opt_text(c.by.as_deref()),
        _ => return None,
    })
}

/// 單位維護（aooi101.4gl CONSTRUCT gfe01–gfe03、gfeacti；ADR 0013 SA1）
pub mod unit {
    use ttrs_domain::{qbe::FieldType, unit::Unit};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: &[Field] = &[
        field("unit_code", FieldType::Text),
        field("name", FieldType::Text),
        field("decimals", FieldType::Integer),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(u: &Unit, field: &str) -> FieldValue {
        match field {
            "unit_code" => text(u.code.as_str()),
            "name" => opt_text(u.attrs.name.as_deref()),
            "decimals" => int(i32::from(u.attrs.decimals)),
            other => confirm_value(other, &u.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(u.active),
                        &u.created_by,
                        &u.created_dept,
                        Some(u.created_on),
                        u.updated_by.as_deref(),
                        u.updated_on,
                        u.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 專案維護（aooi200.4gl:103 CONSTRUCT gja01–gja04、gjauser、gjagrup、gjamodu、gjadate、gjaacti；ADR 0013 SA1）
pub mod project {
    use ttrs_domain::{project::Project, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: &[Field] = &[
        field("project_code", FieldType::Text),
        field("name", FieldType::Text),
        field("start_on", FieldType::Date),
        field("end_on", FieldType::Date),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(x: &Project, field: &str) -> FieldValue {
        let a = &x.attrs;
        match field {
            "project_code" => text(x.code.as_str()),
            "name" => opt_text(a.name.as_deref()),
            "start_on" => date(a.start_on),
            "end_on" => date(a.end_on),
            other => confirm_value(other, &x.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(x.active),
                        &x.created_by,
                        &x.created_dept,
                        Some(x.created_on),
                        x.updated_by.as_deref(),
                        x.updated_on,
                        x.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 稅別維護（aooi150.4gl:617 CONSTRUCT gec01、gec02、gec011、gec03、gec031、gec05、gec06、gec04、gec07、gec08、gecacti）
pub mod tax_type {
    use ttrs_domain::{qbe::FieldType, tax_type::TaxType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: &[Field] = &[
        field("tax_code", FieldType::Text),
        field("name", FieldType::Text),
        field("direction", FieldType::Text),
        field("account", FieldType::Text),
        field("account2", FieldType::Text),
        field("invoice_copies", FieldType::Text),
        field("tax_class", FieldType::Text),
        field("rate", FieldType::Decimal),
        field("include_tax", FieldType::Flag),
        field("filing_format", FieldType::Text),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(t: &TaxType, field: &str) -> FieldValue {
        let a = &t.attrs;
        match field {
            "tax_code" => text(t.key.code.as_str()),
            "direction" => text(t.key.direction.code()),
            "name" => opt_text(a.name.as_deref()),
            "account" => opt_text(a.account.as_deref()),
            "account2" => opt_text(a.account2.as_deref()),
            "invoice_copies" => opt_text(a.invoice_copies.map(|c| c.code())),
            "tax_class" => opt_text(a.tax_class.map(|c| c.code())),
            "rate" => opt_dec(Some(a.rate)),
            "include_tax" => flag(a.include_tax),
            "filing_format" => text(&a.filing_format),
            other => confirm_value(other, &t.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(t.active),
                        &t.created_by,
                        &t.created_dept,
                        Some(t.created_on),
                        t.updated_by.as_deref(),
                        t.updated_on,
                        t.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 幣別維護（aooi050.4gl:395 CONSTRUCT azi01、azi02、azi03、azi04、azi05、azi07、azi10、aziacti；ADR 0013 SA1）
pub mod currency {
    use ttrs_domain::{currency::Currency, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: &[Field] = &[
        field("currency_code", FieldType::Text),
        field("name", FieldType::Text),
        field("price_decimals", FieldType::Integer),
        field("amount_decimals", FieldType::Integer),
        field("total_decimals", FieldType::Integer),
        field("rate_decimals", FieldType::Integer),
        field("rate_method", FieldType::Text),
        field("import_code", FieldType::Text),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(c: &Currency, field: &str) -> FieldValue {
        let a = &c.attrs;
        match field {
            "currency_code" => text(c.code.as_str()),
            "name" => opt_text(a.name.as_deref()),
            "price_decimals" => int(i32::from(a.price_decimals)),
            "amount_decimals" => int(i32::from(a.amount_decimals)),
            "total_decimals" => int(i32::from(a.total_decimals)),
            "rate_decimals" => int(i32::from(a.rate_decimals)),
            "rate_method" => text(a.rate_method.code()),
            "import_code" => opt_text(a.import_code.as_deref()),
            other => confirm_value(other, &c.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(c.active),
                        &c.created_by,
                        &c.created_dept,
                        Some(c.created_on),
                        c.updated_by.as_deref(),
                        c.updated_on,
                        c.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 會計年度（aoos020.4gl:105-112 的 CONSTRUCT；ADR 0013 SA1 擴及狀態欄位）。各期的日期以「期間含此日期」查詢（date）
pub mod accounting_year {
    use ttrs_domain::{accounting_period::AccountingYear, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: &[Field] = &[
        field("year", FieldType::Integer),
        field("period_count", FieldType::Integer),
        field("start_date", FieldType::Date),
        field("end_date", FieldType::Date),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(y: &AccountingYear, field: &str) -> FieldValue {
        let (first, last) = y.span();
        match field {
            "year" => int(y.year),
            "period_count" => int(i32::from(y.period_count)),
            "start_date" => FieldValue::One(Some(QbeValue::Date(first))),
            "end_date" => FieldValue::One(Some(QbeValue::Date(last))),
            other => confirm_value(other, &y.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        None,
                        &y.created_by,
                        &y.created_dept,
                        Some(y.created_on),
                        y.updated_by.as_deref(),
                        y.updated_on,
                        y.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 單位換算（aooi102.4gl:115-178 的 CONSTRUCT；ADR 0013 SA1 擴及狀態欄位）
pub mod encoding_nature {
    use ttrs_domain::{encoding_nature::EncodingNature, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    /// aooi401.4gl:471-517 的查詢欄位（geh03、geh05 不移植，EN5）加上確認與「狀態」欄位
    pub const FIELDS: &[Field] = &[
        field("nature_code", FieldType::Text),
        field("name", FieldType::Text),
        field("category", FieldType::Text),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(n: &EncodingNature, field: &str) -> FieldValue {
        match field {
            "nature_code" => text(n.code.as_str()),
            "name" => opt_text(n.attrs.name.as_deref()),
            "category" => text(n.attrs.category.code()),
            other => confirm_value(other, &n.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(n.active),
                        &n.created_by,
                        &n.created_dept,
                        Some(n.created_on),
                        n.updated_by.as_deref(),
                        n.updated_on,
                        n.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

pub mod data_control {
    use ttrs_domain::qbe::FieldType;
    use ttrs_shared::{DataControlDto, QbeCondition};

    use super::*;

    /// aooi601.4gl:120-168 的查詢欄位：資料類別、營運中心、資料中心否、資料中心代碼，欄位條件的欄位、條件、值
    pub const FIELDS: &[Field] = &[
        field("data_type", FieldType::Text),
        field("plant", FieldType::Text),
        field("is_data_center", FieldType::Flag),
        field("center_code", FieldType::Text),
        field("field", FieldType::Text),
        field("condition", FieldType::Text),
        field("values", FieldType::Text),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    /// 單身欄位以多值比對：符合任一列即列出該資料類別（aooi601.4gl:207-219）
    pub fn value(d: &DataControlDto, field: &str) -> FieldValue {
        let texts =
            |vals: Vec<String>| FieldValue::Set(vals.into_iter().map(QbeValue::Text).collect());
        match field {
            "data_type" => text(&d.data_type),
            "plant" => texts(d.plants.iter().map(|p| p.plant.clone()).collect()),
            "is_data_center" => FieldValue::Set(
                d.plants
                    .iter()
                    .map(|p| QbeValue::Flag(p.is_data_center))
                    .collect(),
            ),
            "center_code" => texts(
                d.plants
                    .iter()
                    .filter_map(|p| p.center_code.clone())
                    .collect(),
            ),
            "field" | "condition" | "values" => texts(
                d.plants
                    .iter()
                    .flat_map(|p| p.rules.iter())
                    .map(|r| match field {
                        "field" => r.field.clone(),
                        "condition" => r.condition.clone(),
                        _ => r.values.clone(),
                    })
                    .collect(),
            ),
            _ => FieldValue::One(None),
        }
    }
}

pub mod group_center {
    use ttrs_domain::{group_center::GroupCenter, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    /// aooi600.4gl 的查詢欄位 geu00、geu01、geu02、geuacti（GC10）加上確認與「狀態」欄位
    pub const FIELDS: &[Field] = &[
        field("category", FieldType::Text),
        field("center_code", FieldType::Text),
        field("name", FieldType::Text),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(c: &GroupCenter, field: &str) -> FieldValue {
        match field {
            "category" => text(&c.category.code()),
            "center_code" => text(c.code.as_str()),
            "name" => opt_text(c.name.as_deref()),
            other => confirm_value(other, &c.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(c.active),
                        &c.created_by,
                        &c.created_dept,
                        Some(c.created_on),
                        c.updated_by.as_deref(),
                        c.updated_on,
                        c.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

pub mod encoding_segment {
    use ttrs_domain::{encoding_segment::EncodingSegment, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    /// aooi403.4gl:108-205 的查詢欄位：單頭 gej01、gej02、gej03 與狀態欄位，單身 gek02、gek03、gek04
    /// （單身條件符合任一列即符合）
    pub const FIELDS: &[Field] = &[
        field("segment_code", FieldType::Text),
        field("name", FieldType::Text),
        field("length", FieldType::Integer),
        field("line", FieldType::Integer),
        field("value", FieldType::Text),
        field("value_name", FieldType::Text),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(s: &EncodingSegment, field: &str) -> FieldValue {
        match field {
            "segment_code" => text(s.code.as_str()),
            "name" => opt_text(s.attrs.name.as_deref()),
            "length" => int(i32::from(s.attrs.length)),
            "line" => FieldValue::Set(
                s.values
                    .iter()
                    .map(|v| QbeValue::Integer(i64::from(v.line)))
                    .collect(),
            ),
            "value" => FieldValue::Set(
                s.values
                    .iter()
                    .map(|v| QbeValue::Text(v.value.clone()))
                    .collect(),
            ),
            "value_name" => FieldValue::Set(
                s.values
                    .iter()
                    .filter_map(|v| v.name.clone().map(QbeValue::Text))
                    .collect(),
            ),
            other => confirm_value(other, &s.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(s.active),
                        &s.created_by,
                        &s.created_dept,
                        Some(s.created_on),
                        s.updated_by.as_deref(),
                        s.updated_on,
                        s.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

pub mod encoding_class {
    use ttrs_domain::{encoding_class::EncodingClass, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    /// aooi402.4gl:127-226 的查詢欄位：單頭 gei01–gei03 與狀態欄位，單身 gel02、gel06、gel03、gel04、gel05、gel07
    /// （單身條件符合任一段即符合）
    pub const FIELDS: &[Field] = &[
        field("class_code", FieldType::Text),
        field("name", FieldType::Text),
        field("nature_code", FieldType::Text),
        field("seq_no", FieldType::Integer),
        field("description", FieldType::Text),
        field("length", FieldType::Integer),
        field("segment_type", FieldType::Text),
        field("value", FieldType::Text),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(c: &EncodingClass, field: &str) -> FieldValue {
        let set = |f: &dyn Fn(&ttrs_domain::encoding_class::ClassSegment) -> Option<QbeValue>| {
            FieldValue::Set(c.segments.iter().filter_map(f).collect())
        };
        match field {
            "class_code" => text(c.code.as_str()),
            "name" => text(&c.attrs.name),
            "nature_code" => text(&c.attrs.nature),
            "seq_no" => set(&|s| Some(QbeValue::Integer(i64::from(s.no)))),
            "description" => set(&|s| s.description.clone().map(QbeValue::Text)),
            "length" => set(&|s| Some(QbeValue::Integer(i64::from(s.length)))),
            "segment_type" => set(&|s| Some(QbeValue::Text(s.kind.code().to_owned()))),
            "value" => set(&|s| s.value.clone().map(QbeValue::Text)),
            other => confirm_value(other, &c.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(c.active),
                        &c.created_by,
                        &c.created_dept,
                        Some(c.created_on),
                        c.updated_by.as_deref(),
                        c.updated_on,
                        c.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

pub mod encoding_year {
    use ttrs_domain::{encoding_year::EncodingYear, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    /// aooi404.4gl:338-381 的查詢欄位（gff01、gff02、gff03）加上確認與「狀態」欄位
    pub const FIELDS: &[Field] = &[
        field("year", FieldType::Integer),
        field("length", FieldType::Integer),
        field("code_value", FieldType::Text),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(y: &EncodingYear, field: &str) -> FieldValue {
        match field {
            "year" => int(y.year),
            "length" => int(i32::from(y.length)),
            "code_value" => text(&y.code_value),
            other => confirm_value(other, &y.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(y.active),
                        &y.created_by,
                        &y.created_dept,
                        Some(y.created_on),
                        y.updated_by.as_deref(),
                        y.updated_on,
                        y.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

pub mod unit_conversion {
    use ttrs_domain::{qbe::FieldType, unit_conversion::UnitConversion};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: &[Field] = &[
        field("from_unit", FieldType::Text),
        field("to_unit", FieldType::Text),
        field("from_quantity", FieldType::Decimal),
        field("to_quantity", FieldType::Decimal),
        field("note", FieldType::Text),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(c: &UnitConversion, field: &str) -> FieldValue {
        let v = &c.conversion;
        match field {
            "from_unit" => text(v.from_unit.as_str()),
            "to_unit" => text(v.to_unit.as_str()),
            "from_quantity" => opt_dec(Some(v.from_quantity)),
            "to_quantity" => opt_dec(Some(v.to_quantity)),
            "note" => opt_text(v.note.as_deref()),
            other => confirm_value(other, &c.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(c.active),
                        &c.created_by,
                        &c.created_dept,
                        Some(c.created_on),
                        c.updated_by.as_deref(),
                        c.updated_on,
                        c.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 基本代碼（區域別 aooi100:353、國別 aooi090:419、地區 aooi110:399、交運方式 aooi160:355、報表抬頭 aooi210:118
/// 的 CONSTRUCT；ADR 0013 SA1 擴及狀態欄位）。欄位：代號 code、名稱 name、隸屬 parent
pub mod basic_code {
    use ttrs_domain::{basic_code::BasicCode, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: &[Field] = &[
        field("code", FieldType::Text),
        field("name", FieldType::Text),
        field("parent", FieldType::Text),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(c: &BasicCode, field: &str) -> FieldValue {
        match field {
            "code" => text(c.code.as_str()),
            "name" => opt_text(c.attrs.name.as_deref()),
            "parent" => opt_text(c.attrs.parent.as_deref()),
            other => confirm_value(other, &c.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(c.active),
                        &c.created_by,
                        &c.created_dept,
                        Some(c.created_on),
                        c.updated_by.as_deref(),
                        c.updated_on,
                        c.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 產品分類維護（axmi110.4gl:477-541 CONSTRUCT oba01、oba02、oba10、oba11、oba111；ADR 0013 SA1 擴及全部欄位）
pub mod product_class {
    use ttrs_domain::{product_class::ProductClass, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: &[Field] = &[
        field("class_code", FieldType::Text),
        field("name", FieldType::Text),
        field("period_format", FieldType::Text),
        field("periods", FieldType::Integer),
        field("cost_center", FieldType::Text),
        field("revenue_account", FieldType::Text),
        field("revenue_account2", FieldType::Text),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(c: &ProductClass, field: &str) -> FieldValue {
        let a = &c.attrs;
        match field {
            "class_code" => text(c.code.as_str()),
            "name" => opt_text(a.name.as_deref()),
            "period_format" => opt_text(a.period_format.as_deref()),
            "periods" => opt_int(a.periods),
            "cost_center" => opt_text(a.cost_center.as_ref().map(|d| d.as_str())),
            "revenue_account" => opt_text(a.revenue_account.as_deref()),
            "revenue_account2" => opt_text(a.revenue_account2.as_deref()),
            other => confirm_value(other, &c.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(c.active),
                        &c.created_by,
                        &c.created_dept,
                        Some(c.created_on),
                        c.updated_by.as_deref(),
                        c.updated_on,
                        c.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 分類碼維護（aooi300 CONSTRUCT azf01、azf03、azf05、azf051、azf06、azfacti；ADR 0013 SA1）：同一類別內查詢
pub mod classification {
    use ttrs_domain::{classification::ClassificationCode, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: &[Field] = &[
        field("code", FieldType::Text),
        field("description", FieldType::Text),
        field("variance_account", FieldType::Text),
        field("variance_account2", FieldType::Text),
        field("cost_nature", FieldType::Integer),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(c: &ClassificationCode, field: &str) -> FieldValue {
        let a = &c.attrs;
        match field {
            "code" => text(c.code.as_str()),
            "description" => opt_text(a.description.as_deref()),
            "variance_account" => opt_text(a.variance_account.as_deref()),
            "variance_account2" => opt_text(a.variance_account2.as_deref()),
            "cost_nature" => opt_int(a.cost_nature),
            other => confirm_value(other, &c.confirmation)
                .or_else(|| {
                    status_value(
                        other,
                        Some(c.active),
                        &c.created_by,
                        &c.created_dept,
                        Some(c.created_on),
                        c.updated_by.as_deref(),
                        c.updated_on,
                        c.stamp.version,
                    )
                })
                .unwrap_or(FieldValue::One(None)),
        }
    }
}

/// 匯率的六個匯率欄位與狀態欄位（共用於每月、每日匯率）
mod rate_fields {
    use ttrs_domain::exchange_rate::{RateRecord, Rates};

    use super::*;

    pub fn rate(r: &Rates, field: &str) -> Option<FieldValue> {
        Some(opt_dec(Some(match field {
            "bank_buy" => r.bank_buy,
            "bank_sell" => r.bank_sell,
            "bank_mid" => r.bank_mid,
            "customs_period" => r.customs_period,
            "customs_buy" => r.customs_buy,
            "customs_sell" => r.customs_sell,
            _ => return None,
        })))
    }

    pub fn record(x: &RateRecord, field: &str) -> FieldValue {
        confirm_value(field, &x.confirmation)
            .or_else(|| {
                status_value(
                    field,
                    Some(x.active),
                    &x.created_by,
                    &x.created_dept,
                    Some(x.created_on),
                    x.updated_by.as_deref(),
                    x.updated_on,
                    x.stamp.version,
                )
            })
            .unwrap_or(FieldValue::One(None))
    }
}

/// 每月匯率（aooi060.4gl:201-264 CONSTRUCT azj01、azj02、azj03、azj04、azj041、azj051、azj052、azj07、azjacti 等）
pub mod monthly_rate {
    use ttrs_domain::{exchange_rate::MonthlyRate, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: &[Field] = &[
        field("currency_code", FieldType::Text),
        field("period", FieldType::Text),
        field("bank_buy", FieldType::Decimal),
        field("bank_sell", FieldType::Decimal),
        field("bank_mid", FieldType::Decimal),
        field("customs_period", FieldType::Decimal),
        field("customs_buy", FieldType::Decimal),
        field("customs_sell", FieldType::Decimal),
        field("revaluation", FieldType::Decimal),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(r: &MonthlyRate, field: &str) -> FieldValue {
        match field {
            "currency_code" => text(r.currency.as_str()),
            "period" => text(&r.period.to_string()),
            "revaluation" => opt_dec(Some(r.revaluation)),
            other => rate_fields::rate(&r.rates, other)
                .unwrap_or_else(|| rate_fields::record(&r.record, other)),
        }
    }
}

/// 每日匯率（aooi070.4gl:201-274 CONSTRUCT azk01、azk02、azk03、azk04、azk041、azk05、azk051、azk052）
pub mod daily_rate {
    use ttrs_domain::{exchange_rate::DailyRate, qbe::FieldType};
    use ttrs_shared::QbeCondition;

    use super::*;

    pub const FIELDS: &[Field] = &[
        field("currency_code", FieldType::Text),
        field("rate_date", FieldType::Date),
        field("bank_buy", FieldType::Decimal),
        field("bank_sell", FieldType::Decimal),
        field("bank_mid", FieldType::Decimal),
        field("customs_period", FieldType::Decimal),
        field("customs_buy", FieldType::Decimal),
        field("customs_sell", FieldType::Decimal),
        field("confirmed", FieldType::Flag),
        field("confirmed_by", FieldType::Text),
        field("active", FieldType::Flag),
        field("created_by", FieldType::Text),
        field("created_dept", FieldType::Text),
        field("created_on", FieldType::Date),
        field("updated_by", FieldType::Text),
        field("updated_on", FieldType::Date),
        field("version", FieldType::Integer),
    ];

    pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
        super::filters(conditions, FIELDS, as_is)
    }

    pub fn value(r: &DailyRate, field: &str) -> FieldValue {
        match field {
            "currency_code" => text(r.currency.as_str()),
            "rate_date" => date(Some(r.date)),
            other => rate_fields::rate(&r.rates, other)
                .unwrap_or_else(|| rate_fields::record(&r.record, other)),
        }
    }
}
