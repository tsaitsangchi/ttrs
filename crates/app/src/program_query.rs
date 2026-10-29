//! p_zz 的 QBE 查詢欄位（p_zz.4gl :286-290 CONSTRUCT 的 27 個欄位；zz04 可執行功能不可查 :292）
//!
//! 欄位鍵值與 ProgramDto 欄位名稱相同；infra 依鍵值對應資料庫欄位，記憶體版用 `values` 取值比對。

use ttrs_domain::{
    DomainError,
    program::{IdleAction, IdleControl, Program, ReportExtMode, ReportType},
    qbe::{self, FieldType, QbeFilter, QbeValue},
};
use ttrs_shared::{IdleActionDto, IdleControlDto, QbeCondition, ReportExtModeDto, ReportTypeDto};

use crate::ports::ProgramName;

/// (鍵值, 型別)
const FIELDS: [(&str, FieldType); 27] = [
    ("program_no", FieldType::Text),
    ("module_code", FieldType::Text),
    ("name", FieldType::Text),
    ("program_group", FieldType::Text),
    ("window_style", FieldType::Text),
    ("program_type", FieldType::Text),
    ("run_command", FieldType::Text),
    ("fixed_run_condition", FieldType::Text),
    ("designed_on", FieldType::Date),
    ("print_method", FieldType::Text),
    ("report_type", FieldType::Text),
    ("report_ext_mode", FieldType::Text),
    ("report_width", FieldType::Integer),
    ("current_print_count", FieldType::Integer),
    ("deferred_print_count", FieldType::Integer),
    ("print_option", FieldType::Flag),
    ("idle_control", FieldType::Text),
    ("idle_seconds", FieldType::Integer),
    ("idle_action", FieldType::Text),
    ("log_errors", FieldType::Flag),
    ("allow_key_update", FieldType::Flag),
    ("help_enabled", FieldType::Flag),
    ("remark", FieldType::Text),
    ("created_by", FieldType::Text),
    ("updated_by", FieldType::Text),
    ("created_dept", FieldType::Text),
    ("updated_on", FieldType::Date),
];

/// 把畫面條件轉成已解析的篩選；欄位不在白名單時回傳 `QbeFieldNotAllowed`
pub fn filters(conditions: &[QbeCondition]) -> Result<Vec<QbeFilter>, DomainError> {
    let mut out = Vec::new();
    for c in conditions {
        let (key, ty) = FIELDS
            .iter()
            .copied()
            .find(|(k, _)| *k == c.field)
            .ok_or_else(|| DomainError::QbeFieldNotAllowed(c.field.clone()))?;
        if let Some(f) = qbe::parse(key, ty, &c.expr, |v| code_of(key, v))? {
            out.push(f);
        }
    }
    Ok(out)
}

/// 下拉欄位：畫面顯示文字 → TipTop 代碼；不是顯示文字就當作代碼本身
fn code_of(field: &str, value: &str) -> String {
    fn find<T: Copy, D: Copy>(
        all: &[T],
        dto: fn(T) -> D,
        label: fn(D) -> &'static str,
        code: fn(T) -> &'static str,
        value: &str,
    ) -> Option<String> {
        all.iter()
            .find(|v| label(dto(**v)) == value)
            .map(|v| code(*v).to_owned())
    }
    let mapped = match field {
        "report_type" => find(
            &REPORT_TYPES,
            report_type_dto,
            ReportTypeDto::label,
            ReportType::code,
            value,
        ),
        "report_ext_mode" => find(
            &EXT_MODES,
            ext_mode_dto,
            ReportExtModeDto::label,
            ReportExtMode::code,
            value,
        ),
        "idle_control" => find(
            &IDLE_CONTROLS,
            idle_control_dto,
            IdleControlDto::label,
            IdleControl::code,
            value,
        ),
        "idle_action" => find(
            &IDLE_ACTIONS,
            idle_action_dto,
            IdleActionDto::label,
            IdleAction::code,
            value,
        ),
        _ => None,
    };
    mapped.unwrap_or_else(|| value.to_owned())
}

const REPORT_TYPES: [ReportType; 5] = [
    ReportType::Voucher,
    ReportType::Detail,
    ReportType::Statistics,
    ReportType::Tracking,
    ReportType::None,
];
const EXT_MODES: [ReportExtMode; 2] = [ReportExtMode::Fixed, ReportExtMode::Dynamic];
const IDLE_CONTROLS: [IdleControl; 3] = [
    IdleControl::Off,
    IdleControl::ByProgram,
    IdleControl::BySystem,
];
const IDLE_ACTIONS: [IdleAction; 3] = [
    IdleAction::WarnOnly,
    IdleAction::WarnThenClose,
    IdleAction::Close,
];

fn report_type_dto(v: ReportType) -> ReportTypeDto {
    ReportTypeDto::ALL[REPORT_TYPES.iter().position(|x| *x == v).unwrap_or(4)]
}
fn ext_mode_dto(v: ReportExtMode) -> ReportExtModeDto {
    ReportExtModeDto::ALL[EXT_MODES.iter().position(|x| *x == v).unwrap_or(0)]
}
fn idle_control_dto(v: IdleControl) -> IdleControlDto {
    IdleControlDto::ALL[IDLE_CONTROLS.iter().position(|x| *x == v).unwrap_or(2)]
}
fn idle_action_dto(v: IdleAction) -> IdleActionDto {
    IdleActionDto::ALL[IDLE_ACTIONS.iter().position(|x| *x == v).unwrap_or(0)]
}

/// 記憶體版比對用：取出欄位值。程式名稱多語系時回傳每一筆（任一符合即符合，同 :343 UNIQUE）
pub fn values(p: &Program, names: &[ProgramName], field: &str) -> Vec<Option<QbeValue>> {
    let text = |v: &Option<String>| v.clone().map(QbeValue::Text);
    let code = |c: &str| Some(QbeValue::Text(c.to_owned()));
    let int = |v: Option<i32>| v.map(|n| QbeValue::Integer(i64::from(n)));
    let r = &p.runtime;
    let o = &p.other;
    let one = match field {
        "program_no" => code(p.program_no.as_str()),
        "module_code" => code(p.module.as_str()),
        "name" => {
            return if names.is_empty() {
                vec![None]
            } else {
                names.iter().map(|n| code(&n.name)).collect()
            };
        }
        "program_group" => text(&r.program_group),
        "window_style" => text(&r.window_style),
        "program_type" => text(&p.program_type),
        "run_command" => text(&r.run_command),
        "fixed_run_condition" => text(&r.fixed_run_condition),
        "designed_on" => Some(QbeValue::Date(p.designed_on)),
        "print_method" => text(&r.print_method),
        "report_type" => code(p.report_type.code()),
        "report_ext_mode" => r.report_ext_mode.and_then(|m| code(m.code())),
        "report_width" => int(p.report_width),
        "current_print_count" => int(Some(p.current_print_count)),
        "deferred_print_count" => int(Some(p.deferred_print_count)),
        "print_option" => Some(QbeValue::Flag(o.print_option)),
        "idle_control" => code(p.idle_control.code()),
        "idle_seconds" => int(p.idle_seconds),
        "idle_action" => p.idle_action.and_then(|a| code(a.code())),
        "log_errors" => Some(QbeValue::Flag(r.log_errors)),
        "allow_key_update" => Some(QbeValue::Flag(o.allow_key_update)),
        "help_enabled" => Some(QbeValue::Flag(r.help_enabled)),
        "remark" => text(&o.remark),
        "created_by" => code(&p.created_by),
        "updated_by" => text(&p.updated_by),
        "created_dept" => code(&p.created_dept),
        "updated_on" => Some(QbeValue::Date(p.updated_on)),
        _ => None,
    };
    vec![one]
}

/// 記憶體版：程式是否符合全部篩選（AND）
pub fn matches_all(p: &Program, names: &[ProgramName], filters: &[QbeFilter]) -> bool {
    filters.iter().all(|f| {
        values(p, names, f.field)
            .iter()
            .any(|v| f.matches(v.as_ref()))
    })
}
