//! 程式登錄資料
//! TipTop 對應：p_zz（程式資料建立）／ zz_file（名稱 gaz_file、權限 zy_file、選單 zm_file）
//! 規格：specs/features/azz/program_registry.feature

use crate::DomainError;

/// 共用程式模組，不可作為作業程式的所屬模組（p_zz.4gl:254-268）
const SHARED_MODULES: [&str; 6] = ["SUB", "CSUB", "LIB", "CLIB", "QRY", "CQRY"];
/// 適用「傳遞帳別」的總帳模組（p_zz.4gl:932-938）
const LEDGER_MODULES: [&str; 4] = ["AGL", "CGL", "GGL", "CGGL"];
/// 選單不是模組資料，而是 p_zz 額外加入的固定選項
pub const MENU_MODULE: &str = "MENU";

/// 程式代號：去除前後空白，保留大小寫（TipTop 程式代號為小寫，如 aimi100）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProgramNo(String);

impl ProgramNo {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::ProgramNoEmpty);
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 標準命名「a 開頭、7 碼」（如 aimi100）：第 4 碼為程式類別（p_zz.4gl:917-918）
    fn is_standard(&self) -> bool {
        self.0.starts_with('a') && self.0.chars().count() == 7
    }

    fn derived_type(&self) -> Option<String> {
        self.is_standard()
            .then(|| {
                self.0
                    .chars()
                    .nth(3)
                    .map(|c| c.to_ascii_uppercase().to_string())
            })
            .flatten()
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
    pub print_setting: Option<String>,
    pub print_default: Option<String>,
    pub quick_run_data: Option<String>,
    pub help_enabled: bool,
    pub log_errors: bool,
}

/// 使用者可輸入的欄位；`None` 表示套用 p_zz 預設值
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
    pub created_by: String,
    pub created_dept: String,
    pub updated_by: Option<String>,
}

impl Program {
    /// 新增程式（p_zz.4gl:775-806、:901-965）
    pub fn new(
        program_no: ProgramNo,
        module: ModuleCode,
        attrs: ProgramAttrs,
        created_by: &str,
        created_dept: &str,
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
            created_by: created_by.to_owned(),
            created_dept: created_dept.to_owned(),
            updated_by: None,
        };
        p.apply(attrs)?;
        Ok(p)
    }

    /// 修改欄位並記錄修改者（p_zz.4gl:1338、:1378）
    pub fn update(
        &mut self,
        module: ModuleCode,
        attrs: ProgramAttrs,
        updated_by: &str,
    ) -> Result<(), DomainError> {
        let mut next = self.clone();
        next.module = module;
        next.apply(attrs)?;
        next.updated_by = Some(updated_by.to_owned());
        *self = next;
        Ok(())
    }

    /// 複製為新代號：建立者改為目前使用者，執行指令依新代號重產（p_zz.4gl:2045-2056）
    pub fn copy_as(&self, new_no: ProgramNo, user: &str, dept: &str) -> Self {
        let mut p = self.clone();
        p.runtime.run_command = Some(standard_run_command(p.module.as_str(), new_no.as_str()));
        p.program_no = new_no;
        p.created_by = user.to_owned();
        p.created_dept = dept.to_owned();
        p.updated_by = Some(user.to_owned());
        p
    }

    /// 執行指令經由 p_query 執行（p_query 本身除外）時，須有同代號的查詢定義（p_zz.4gl:973-982）
    pub fn requires_query_definition(&self) -> bool {
        self.program_no.as_str() != "p_query"
            && self
                .runtime
                .run_command
                .as_deref()
                .is_some_and(|c| c.contains("p_query"))
    }

    fn apply(&mut self, a: ProgramAttrs) -> Result<(), DomainError> {
        self.program_type = a
            .program_type
            .map(|t| t.trim().to_uppercase())
            .filter(|t| !t.is_empty())
            .or_else(|| self.program_no.derived_type());
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
        // 標準命名且未給執行指令時，自動產生（p_zz.4gl:917-926）
        if self
            .runtime
            .run_command
            .as_deref()
            .is_none_or(|c| c.trim().is_empty())
        {
            self.runtime.run_command = self.program_no.is_standard().then(|| {
                let system: String = self.program_no.as_str().chars().take(3).collect();
                standard_run_command(&system, self.program_no.as_str())
            });
        }
        Ok(())
    }
}

/// Unix 格式的執行指令，如 `$FGLRUN $AIMi/aimi100`（Windows 格式不採用，見規格檔頭）
fn standard_run_command(system: &str, program_no: &str) -> String {
    format!("$FGLRUN ${}i/{program_no}", system.to_uppercase())
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
        )
    }

    #[test]
    fn derives_type_and_run_command_from_standard_no() {
        let p = new("aimi100", "AIM", ProgramAttrs::default()).unwrap();
        assert_eq!(p.program_type.as_deref(), Some("I"));
        assert_eq!(
            p.runtime.run_command.as_deref(),
            Some("$FGLRUN $AIMi/aimi100")
        );
    }

    #[test]
    fn non_standard_no_has_no_type() {
        let p = new("p_query", "AZZ", ProgramAttrs::default()).unwrap();
        assert_eq!(p.program_type, None);
        assert_eq!(p.runtime.run_command, None);
    }

    #[test]
    fn report_program_requires_report_type() {
        assert_eq!(
            new("aimr400", "AIM", ProgramAttrs::default()),
            Err(DomainError::ReportTypeRequired)
        );
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
