//! 程式維護資料（Program Maintenance）
//! TipTop 對應：p_zz（程式資料建立）／ zz_file（名稱 gaz_file、權限 zy_file、選單 zm_file）
//! 規格：specs/features/azz/program_maintenance.feature

use chrono::NaiveDate;

use crate::{
    DomainError,
    lang::{Lang, LangText, non_empty, normalize_text},
};

/// 共用程式模組，不可作為作業程式的所屬模組（p_zz.4gl:254-268）
const SHARED_MODULES: [&str; 6] = ["SUB", "CSUB", "LIB", "CLIB", "QRY", "CQRY"];
/// 適用「傳遞帳別」的總帳模組（p_zz.4gl:932-938）
const LEDGER_MODULES: [&str; 4] = ["AGL", "CGL", "GGL", "CGGL"];
/// 欄位長度上限，取自 TipTop 資料表定義（tiptop/azz/sch/zz_file.sch、gaz_file.sch 的 varchar2 長度），
/// 以字元計（TipTop 的長度語意為位元組或字元未定，取寬鬆者）
pub mod max_len {
    pub const PROGRAM_NO: usize = 40; // 英文程式代號（ADR 0008；TipTop zz01 為 20）
    pub const TIPTOP_PROGRAM_NO: usize = 20; // zz01
    pub const MODULE_CODE: usize = 10; // zz011
    pub const DEFAULT_ACTIONS: usize = 3000; // zz04
    pub const RUN_COMMAND: usize = 60; // zz08
    pub const PROGRAM_GROUP: usize = 10; // zz10
    pub const FIXED_PRINT_CONDITION: usize = 80; // zz21
    pub const FIXED_PRINT_METHOD: usize = 1; // zz22
    pub const WINDOW_STYLE: usize = 20; // zz27
    pub const REMARK: usize = 255; // zz31
    pub const NAME: usize = 500; // gaz03
    pub const REPORT_TITLE: usize = 255; // gaz06
}

/// 檢查文字長度（字元數）；`field` 為畫面欄位名稱，用於錯誤訊息
pub fn check_length(
    field: &'static str,
    value: Option<&str>,
    max: usize,
) -> Result<(), DomainError> {
    match value {
        Some(v) if v.chars().count() > max => Err(DomainError::FieldTooLong { field, max }),
        _ => Ok(()),
    }
}

/// 選單不是模組資料，而是 TipTop p_zz 額外加入的固定選項
pub const MENU_MODULE: &str = "MENU";

/// 程式代號：英文代號（docs/adr/0008-english-program-codes.md，P1）——小寫英文、數字與連字號，
/// 英文字母開頭，最多 40 字；輸入轉小寫。TipTop 代號另存 `Program::tiptop_program_no`
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProgramNo(String);

impl ProgramNo {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim().to_lowercase();
        if v.is_empty() {
            return Err(DomainError::ProgramNoEmpty);
        }
        if v.chars().count() > max_len::PROGRAM_NO {
            return Err(DomainError::ProgramNoTooLong {
                max: max_len::PROGRAM_NO,
            });
        }
        let valid = v.starts_with(|c: char| c.is_ascii_lowercase())
            && v.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !valid {
            return Err(DomainError::ProgramNoInvalid(raw.trim().to_owned()));
        }
        Ok(Self(v))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 所屬模組代號：轉大寫。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleCode(String);

impl ModuleCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim().to_uppercase();
        if v.is_empty() {
            return Err(DomainError::ModuleEmpty);
        }
        check_length("模組代號", Some(&v), max_len::MODULE_CODE)?;
        if SHARED_MODULES.contains(&v.as_str()) {
            return Err(DomainError::ModuleNotAllowed(v));
        }
        Ok(Self(v))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_menu(&self) -> bool {
        self.0 == MENU_MODULE
    }

    fn is_ledger(&self) -> bool {
        LEDGER_MODULES.contains(&self.0.as_str())
    }
}

/// 以 TipTop 原始代碼存檔，方便舊資料轉入
macro_rules! coded_enum {
    ($(#[$m:meta])* $name:ident { $($variant:ident = $code:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub fn code(self) -> &'static str {
                match self { $(Self::$variant => $code),+ }
            }

            pub fn from_code(code: &str) -> Option<Self> {
                match code { $($code => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

pub(crate) use coded_enum;

coded_enum!(
    /// 報表類型（zz14）
    ReportType { Voucher = "1", Detail = "2", Statistics = "3", Tracking = "4", None = "5" }
);
coded_enum!(
    /// 閒置控制（zz28）
    IdleControl { Off = "1", ByProgram = "2", BySystem = "3" }
);
coded_enum!(
    /// 逾時處理（zz29）
    IdleAction { WarnOnly = "1", WarnThenClose = "2", Close = "3" }
);
coded_enum!(
    /// 報表副檔名（zz06）
    ReportExtMode { Fixed = "1", Dynamic = "2" }
);

/// Genero 執行環境欄位：原樣保存、不套規則（Q1）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeSettings {
    pub run_command: Option<String>,
    pub window_style: Option<String>,
    pub report_ext_mode: Option<ReportExtMode>,
    pub program_group: Option<String>,
    pub fixed_print_method: Option<String>,
    pub fixed_print_condition: Option<String>,
    pub sql_explain: bool,
    pub log_errors: bool,
}

/// 其他設定：原樣保存、不套規則（zz04 可執行功能、zz05 列印選項、zz13 維護作業可修改鍵值、zz31 其他備註）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OtherSettings {
    /// 可執行功能清單（格式待確認，見 legacy-map zz04）
    pub default_actions: Option<String>,
    pub print_conditions: bool,
    pub allow_key_update: bool,
    pub remark: Option<String>,
}

/// 使用者可輸入的欄位；`None` 表示套用預設值（比照 TipTop p_zz）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramAttrs {
    pub program_type: Option<String>,
    pub report_type: Option<ReportType>,
    pub idle_control: Option<IdleControl>,
    pub idle_seconds: Option<i32>,
    pub idle_action: Option<IdleAction>,
    pub pass_book_no: bool,
    pub deferred_print_count: Option<i32>,
    pub current_print_count: Option<i32>,
    pub report_width: Option<i32>,
    pub runtime: RuntimeSettings,
    pub other: OtherSettings,
    /// 設計日期（zz09），未給時為今天
    pub designed_on: Option<NaiveDate>,
    /// TipTop 程式代號（zz01，P2）：選填，只用於轉檔與對照
    pub tiptop_program_no: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub program_no: ProgramNo,
    pub module: ModuleCode,
    pub program_type: Option<String>,
    pub report_type: ReportType,
    pub idle_control: IdleControl,
    pub idle_seconds: Option<i32>,
    pub idle_action: Option<IdleAction>,
    pub pass_book_no: bool,
    pub deferred_print_count: i32,
    pub current_print_count: i32,
    pub report_width: Option<i32>,
    pub runtime: RuntimeSettings,
    pub other: OtherSettings,
    pub designed_on: NaiveDate,
    /// TipTop 程式代號（zz01，P2）
    pub tiptop_program_no: Option<String>,
    pub created_by: String,
    pub created_dept: String,
    pub updated_by: Option<String>,
    /// 最後建立/修改日期（zzdate）
    pub updated_on: NaiveDate,
}

impl Program {
    /// 新增程式（p_zz.4gl:775-806、:901-965）
    pub fn new(
        program_no: ProgramNo,
        module: ModuleCode,
        attrs: ProgramAttrs,
        created_by: &str,
        created_dept: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        let mut p = Self {
            program_no,
            module,
            program_type: None,
            report_type: ReportType::None,
            idle_control: IdleControl::BySystem,
            idle_seconds: None,
            idle_action: None,
            pass_book_no: false,
            deferred_print_count: 0,
            current_print_count: 0,
            report_width: None,
            runtime: RuntimeSettings::default(),
            other: OtherSettings::default(),
            designed_on: today,
            tiptop_program_no: None,
            created_by: created_by.to_owned(),
            created_dept: created_dept.to_owned(),
            updated_by: None,
            updated_on: today,
        };
        p.apply(attrs, today)?;
        Ok(p)
    }

    /// 修改欄位並記錄修改者（p_zz.4gl:1338、:1378）
    pub fn update(
        &mut self,
        module: ModuleCode,
        attrs: ProgramAttrs,
        updated_by: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        let mut next = self.clone();
        next.module = module;
        next.apply(attrs, today)?;
        next.updated_by = Some(updated_by.to_owned());
        next.updated_on = today;
        *self = next;
        Ok(())
    }

    /// 複製為新代號：建立者改為目前使用者，日期改為今天（p_zz.4gl:2045-2056）；執行指令沿用來源、
    /// TipTop 程式代號不複製（不可重複）（P2、P4）
    pub fn copy_as(&self, new_no: ProgramNo, user: &str, dept: &str, today: NaiveDate) -> Self {
        let mut p = self.clone();
        p.tiptop_program_no = None;
        p.program_no = new_no;
        p.created_by = user.to_owned();
        p.created_dept = dept.to_owned();
        p.updated_by = Some(user.to_owned());
        p.designed_on = today;
        p.updated_on = today;
        p
    }

    /// 執行指令為「p_query <查詢單代號>」時的查詢單代號，須已有查詢定義（p_zz.4gl:973-982，P5）
    pub fn query_definition_id(&self) -> Option<&str> {
        let mut words = self.runtime.run_command.as_deref()?.split_whitespace();
        words.find(|w| *w == "p_query")?;
        words.next()
    }

    fn apply(&mut self, a: ProgramAttrs, today: NaiveDate) -> Result<(), DomainError> {
        check_lengths(&a)?;
        // 程式類別必填；選單為 M（P3，原 p_zz.4gl:917-918 由代號推導）
        self.program_type = if self.module.is_menu() {
            Some("M".to_owned())
        } else {
            Some(
                a.program_type
                    .map(|t| t.trim().to_uppercase())
                    .filter(|t| !t.is_empty())
                    .ok_or(DomainError::ProgramTypeRequired)?,
            )
        };
        self.tiptop_program_no = a
            .tiptop_program_no
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty());
        self.report_type = a.report_type.unwrap_or(ReportType::None);
        if self.program_type.as_deref() == Some("R") && self.report_type == ReportType::None {
            return Err(DomainError::ReportTypeRequired);
        }

        self.idle_control = a.idle_control.unwrap_or(IdleControl::BySystem);
        // 只有「由程式」才保留閒置秒數與逾時處理
        (self.idle_seconds, self.idle_action) = if self.idle_control == IdleControl::ByProgram {
            (a.idle_seconds, a.idle_action)
        } else {
            (None, None)
        };

        self.pass_book_no = a.pass_book_no && self.module.is_ledger();
        self.deferred_print_count = a.deferred_print_count.unwrap_or(0);
        self.current_print_count = a.current_print_count.unwrap_or(0);
        self.report_width = a.report_width;

        self.runtime = a.runtime;
        // 報表副檔名預設「固定」（p_zz.4gl:791）
        self.runtime.report_ext_mode = self.runtime.report_ext_mode.or(Some(ReportExtMode::Fixed));
        self.other = a.other;
        // 執行指令照輸入保存，不再依標準命名自動產生（P4）
        self.designed_on = a.designed_on.unwrap_or(today);
        Ok(())
    }
}

fn check_lengths(a: &ProgramAttrs) -> Result<(), DomainError> {
    let r = &a.runtime;
    let o = &a.other;
    for (field, value, max) in [
        ("執行指令", r.run_command.as_deref(), max_len::RUN_COMMAND),
        (
            "TipTop 程式代號",
            a.tiptop_program_no.as_deref(),
            max_len::TIPTOP_PROGRAM_NO,
        ),
        (
            "程式群組",
            r.program_group.as_deref(),
            max_len::PROGRAM_GROUP,
        ),
        (
            "固定列印條件",
            r.fixed_print_condition.as_deref(),
            max_len::FIXED_PRINT_CONDITION,
        ),
        (
            "固定列印方式",
            r.fixed_print_method.as_deref(),
            max_len::FIXED_PRINT_METHOD,
        ),
        ("視窗型態", r.window_style.as_deref(), max_len::WINDOW_STYLE),
        (
            "可執行功能",
            o.default_actions.as_deref(),
            max_len::DEFAULT_ACTIONS,
        ),
        ("其他備註", o.remark.as_deref(), max_len::REMARK),
    ] {
        check_length(field, value, max)?;
    }
    Ok(())
}

/// 某語系的程式名稱與報表抬頭（TipTop gaz_file：gaz03、gaz06）；名稱可空白（L1）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramName {
    pub lang: Lang,
    pub name: String,
    pub report_title: Option<String>,
}

impl ProgramName {
    /// 含語系與長度檢核（各語系各自檢核）
    pub fn new(
        lang: &str,
        name: Option<String>,
        report_title: Option<String>,
    ) -> Result<Self, DomainError> {
        let n = Self {
            lang: Lang::parse(lang)?,
            name: normalize_text(name).unwrap_or_default(),
            report_title: normalize_text(report_title),
        };
        check_length("程式名稱", non_empty(&n.name).as_deref(), max_len::NAME)?;
        check_length("報表抬頭", n.report_title.as_deref(), max_len::REPORT_TITLE)?;
        Ok(n)
    }

    /// 名稱與報表抬頭皆空白
    pub fn is_blank(&self) -> bool {
        self.name.is_empty() && self.report_title.is_none()
    }
}

impl LangText for ProgramName {
    fn lang(&self) -> &Lang {
        &self.lang
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(no: &str, module: &str, attrs: ProgramAttrs) -> Result<Program, DomainError> {
        Program::new(
            ProgramNo::parse(no)?,
            ModuleCode::parse(module)?,
            attrs,
            "tiptop",
            "MIS",
            NaiveDate::from_ymd_opt(2026, 10, 29).unwrap(),
        )
    }

    fn typed(t: &str) -> ProgramAttrs {
        ProgramAttrs {
            program_type: Some(t.into()),
            ..ProgramAttrs::default()
        }
    }

    #[test]
    fn english_program_no_rules() {
        assert_eq!(
            ProgramNo::parse(" Item-Maintenance ").unwrap().as_str(),
            "item-maintenance"
        );
        for bad in ["item_maintenance", "1item", "item maintenance"] {
            assert_eq!(
                ProgramNo::parse(bad).unwrap_err().code(),
                "PROGRAM_NO_INVALID",
                "{bad}"
            );
        }
    }

    #[test]
    fn program_type_required_except_menu() {
        assert_eq!(
            new("item-maintenance", "AIM", ProgramAttrs::default()),
            Err(DomainError::ProgramTypeRequired)
        );
        let menu = new("menu-aim", "MENU", ProgramAttrs::default()).unwrap();
        assert_eq!(menu.program_type.as_deref(), Some("M"));
        let p = new("item-maintenance", "AIM", typed("i")).unwrap();
        assert_eq!(p.runtime.run_command, None);
    }

    #[test]
    fn report_program_requires_report_type() {
        assert_eq!(
            new("item-list-report", "AIM", typed("R")),
            Err(DomainError::ReportTypeRequired)
        );
    }

    #[test]
    fn query_definition_from_run_command() {
        let mut a = typed("Q");
        a.runtime.run_command = Some("$FGLRUN p_query aimq900".into());
        let p = new("item-query", "AIM", a).unwrap();
        assert_eq!(p.query_definition_id(), Some("aimq900"));
    }

    #[test]
    fn shared_modules_rejected() {
        assert_eq!(
            ModuleCode::parse("csub"),
            Err(DomainError::ModuleNotAllowed("CSUB".into()))
        );
    }

    #[test]
    fn coded_enum_round_trip() {
        assert_eq!(
            ReportType::from_code(ReportType::Detail.code()),
            Some(ReportType::Detail)
        );
        assert_eq!(IdleControl::from_code("9"), None);
    }
}
