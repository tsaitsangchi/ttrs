//! 程式登錄維護 use case（TipTop: p_zz）
//! 規格：specs/features/azz/program_registry.feature

use std::sync::Arc;

use ttrs_domain::{
    DomainError,
    program::{
        IdleAction, IdleControl, ModuleCode, Program, ProgramAttrs, ProgramNo, ReportExtMode,
        ReportType, RuntimeSettings,
    },
};
use ttrs_shared::{
    AuditEntryDto, IdleActionDto, IdleControlDto, ProgramDto, ProgramInput, ProgramNameDto,
    ReportExtModeDto, ReportTypeDto,
};

use crate::{
    AppError,
    error::RepoError,
    ports::{AuditEntry, ProgramName, ProgramRepository},
};

/// 程式名稱的預設語系（輸入畫面上的「程式名稱」）
pub const DEFAULT_LANG: &str = "zh-TW";
/// 寫入異動紀錄時的作業代號
const AUDIT_PROGRAM: &str = "p_zz";

/// 執行操作的使用者（TipTop: g_user / g_grup / 權限類別 g_clas）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Actor {
    pub user_id: String,
    pub dept: String,
    pub permission_class: String,
}

#[derive(Clone)]
pub struct ProgramService {
    repo: Arc<dyn ProgramRepository>,
}

impl ProgramService {
    pub fn new(repo: Arc<dyn ProgramRepository>) -> Self {
        Self { repo }
    }

    /// 新增程式，並自動授權給建立者的權限類別（p_zz.4gl:851 p_zz_gen）
    pub async fn create(&self, actor: &Actor, input: ProgramInput) -> Result<ProgramDto, AppError> {
        let no = ProgramNo::parse(&input.program_no)?;
        let module = self.module(&input.module_code).await?;
        let program = Program::new(no, module, attrs(&input), &actor.user_id, &actor.dept)?;
        self.check_query_definition(&program).await?;

        let names: Vec<ProgramName> = input
            .name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(|n| ProgramName {
                lang: DEFAULT_LANG.to_owned(),
                name: n.to_owned(),
            })
            .into_iter()
            .collect();
        // 選單不是可執行程式，不授權
        let grant_to = (!program.module.is_menu() && !actor.permission_class.is_empty())
            .then_some(actor.permission_class.as_str());

        match self.repo.insert(&program, &names, grant_to).await {
            Ok(()) => self.to_dto(&program).await,
            Err(RepoError::Conflict) => Err(DomainError::ProgramAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    pub async fn get(&self, no: &str) -> Result<ProgramDto, AppError> {
        let program = self.load(&ProgramNo::parse(no)?).await?;
        self.to_dto(&program).await
    }

    pub async fn list(&self) -> Result<Vec<ProgramDto>, AppError> {
        let programs = self.repo.list().await.map_err(infra)?;
        let mut out = Vec::with_capacity(programs.len());
        for p in &programs {
            out.push(self.to_dto(p).await?);
        }
        Ok(out)
    }

    /// 修改程式欄位（代號另用 `change_no`）
    pub async fn update(
        &self,
        actor: &Actor,
        no: &str,
        input: ProgramInput,
    ) -> Result<ProgramDto, AppError> {
        let mut program = self.load(&ProgramNo::parse(no)?).await?;
        let module = self.module(&input.module_code).await?;
        program.update(module, attrs(&input), &actor.user_id)?;
        self.check_query_definition(&program).await?;
        self.repo.update(&program).await.map_err(infra)?;
        self.to_dto(&program).await
    }

    /// 修改程式代號，連動名稱、選單與權限（Q2：一律允許）
    pub async fn change_no(&self, actor: &Actor, old: &str, new: &str) -> Result<(), AppError> {
        let old = ProgramNo::parse(old)?;
        let new = ProgramNo::parse(new)?;
        let mut program = self.load(&old).await?;
        if old == new {
            return Ok(());
        }
        match self.repo.change_no(&old, &new).await {
            Ok(()) => {}
            Err(RepoError::Conflict) => return Err(DomainError::ProgramAlreadyExists.into()),
            Err(e) => return Err(infra(e)),
        }
        program.program_no = new;
        program.updated_by = Some(actor.user_id.clone());
        self.repo.update(&program).await.map_err(infra)
    }

    /// 刪除程式，連帶刪除名稱、權限、選單並寫異動紀錄（Q3：一律連帶刪除）
    pub async fn delete(&self, actor: &Actor, no: &str) -> Result<(), AppError> {
        let no = ProgramNo::parse(no)?;
        self.load(&no).await?;
        let audit = AuditEntry {
            program: AUDIT_PROGRAM.to_owned(),
            user_id: actor.user_id.clone(),
            key: no.as_str().to_owned(),
            action: "delete".to_owned(),
        };
        self.repo.delete(&no, &audit).await.map_err(infra)
    }

    /// 複製程式與名稱（p_zz.4gl:1973-2101）
    pub async fn copy(&self, actor: &Actor, from: &str, to: &str) -> Result<ProgramDto, AppError> {
        let from = ProgramNo::parse(from)?;
        let to = ProgramNo::parse(to)?;
        let source = self.load(&from).await?;
        let names = self.repo.names(&from).await.map_err(infra)?;
        let program = source.copy_as(to, &actor.user_id, &actor.dept);
        match self.repo.insert(&program, &names, None).await {
            Ok(()) => self.to_dto(&program).await,
            Err(RepoError::Conflict) => Err(DomainError::ProgramAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    /// 為權限類別設定程式權限（p_zz.4gl:1589-1603）
    pub async fn grant(&self, no: &str, permission_class: &str) -> Result<(), AppError> {
        let program = self.load(&ProgramNo::parse(no)?).await?;
        if program.module.is_menu() {
            return Err(DomainError::MenuHasNoPermission.into());
        }
        let class = permission_class.trim();
        if !self
            .repo
            .permission_class_exists(class)
            .await
            .map_err(infra)?
        {
            return Err(DomainError::PermissionClassNotFound.into());
        }
        self.repo
            .grant(class, &program.program_no)
            .await
            .map_err(infra)
    }

    pub async fn permission_classes(&self, no: &str) -> Result<Vec<String>, AppError> {
        let no = ProgramNo::parse(no)?;
        self.repo.permission_classes(&no).await.map_err(infra)
    }

    pub async fn menus_containing(&self, no: &str) -> Result<Vec<String>, AppError> {
        let no = ProgramNo::parse(no)?;
        self.repo.menus_containing(&no).await.map_err(infra)
    }

    pub async fn names(&self, no: &str) -> Result<Vec<ProgramNameDto>, AppError> {
        let no = ProgramNo::parse(no)?;
        let names = self.repo.names(&no).await.map_err(infra)?;
        Ok(names
            .into_iter()
            .map(|n| ProgramNameDto {
                lang: n.lang,
                name: n.name,
            })
            .collect())
    }

    pub async fn audit_entries(&self, key: &str) -> Result<Vec<AuditEntryDto>, AppError> {
        let entries = self.repo.audit_entries(key).await.map_err(infra)?;
        Ok(entries
            .into_iter()
            .map(|a| AuditEntryDto {
                program: a.program,
                user_id: a.user_id,
                key: a.key,
                action: a.action,
            })
            .collect())
    }

    /// 解析模組並確認存在；MENU 是固定選項，不查模組資料
    async fn module(&self, raw: &str) -> Result<ModuleCode, AppError> {
        let module = ModuleCode::parse(raw)?;
        if !module.is_menu() && !self.repo.module_exists(&module).await.map_err(infra)? {
            return Err(DomainError::ModuleNotFound.into());
        }
        Ok(module)
    }

    async fn check_query_definition(&self, program: &Program) -> Result<(), AppError> {
        if program.requires_query_definition()
            && !self
                .repo
                .query_definition_exists(program.program_no.as_str())
                .await
                .map_err(infra)?
        {
            return Err(DomainError::QueryDefinitionNotFound.into());
        }
        Ok(())
    }

    async fn load(&self, no: &ProgramNo) -> Result<Program, AppError> {
        self.repo
            .get(no)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::ProgramNotFound.into())
    }

    async fn to_dto(&self, p: &Program) -> Result<ProgramDto, AppError> {
        let names = self.repo.names(&p.program_no).await.map_err(infra)?;
        let name = names
            .into_iter()
            .find(|n| n.lang == DEFAULT_LANG)
            .map(|n| n.name);
        let r = &p.runtime;
        Ok(ProgramDto {
            program_no: p.program_no.as_str().to_owned(),
            module_code: p.module.as_str().to_owned(),
            program_type: p.program_type.clone(),
            name,
            report_type: report_type_dto(p.report_type),
            idle_control: idle_control_dto(p.idle_control),
            idle_seconds: p.idle_seconds,
            idle_action: p.idle_action.map(idle_action_dto),
            pass_book_no: p.pass_book_no,
            deferred_print_count: p.deferred_print_count,
            current_print_count: p.current_print_count,
            report_width: p.report_width,
            run_command: r.run_command.clone(),
            window_style: r.window_style.clone(),
            report_ext_mode: r.report_ext_mode.map(report_ext_mode_dto),
            print_setting: r.print_setting.clone(),
            print_default: r.print_default.clone(),
            quick_run_data: r.quick_run_data.clone(),
            help_enabled: r.help_enabled,
            log_errors: r.log_errors,
            created_by: p.created_by.clone(),
            created_dept: p.created_dept.clone(),
            updated_by: p.updated_by.clone(),
        })
    }
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}

fn attrs(i: &ProgramInput) -> ProgramAttrs {
    ProgramAttrs {
        program_type: i.program_type.clone(),
        report_type: i.report_type.map(report_type),
        idle_control: i.idle_control.map(idle_control),
        idle_seconds: i.idle_seconds,
        idle_action: i.idle_action.map(idle_action),
        pass_book_no: i.pass_book_no,
        deferred_print_count: i.deferred_print_count,
        current_print_count: i.current_print_count,
        report_width: i.report_width,
        runtime: RuntimeSettings {
            run_command: i.run_command.clone(),
            window_style: i.window_style.clone(),
            report_ext_mode: i.report_ext_mode.map(report_ext_mode),
            print_setting: i.print_setting.clone(),
            print_default: i.print_default.clone(),
            quick_run_data: i.quick_run_data.clone(),
            help_enabled: i.help_enabled,
            log_errors: i.log_errors,
        },
    }
}

/// 同名變體的雙向轉換（domain ↔ DTO）
macro_rules! map_enum {
    ($to_domain:ident, $to_dto:ident, $dto:ident, $domain:ident, [$($v:ident),+]) => {
        fn $to_domain(v: $dto) -> $domain {
            match v { $($dto::$v => $domain::$v),+ }
        }
        fn $to_dto(v: $domain) -> $dto {
            match v { $($domain::$v => $dto::$v),+ }
        }
    };
}

map_enum!(
    report_type,
    report_type_dto,
    ReportTypeDto,
    ReportType,
    [Voucher, Detail, Statistics, Tracking, None]
);
map_enum!(
    idle_control,
    idle_control_dto,
    IdleControlDto,
    IdleControl,
    [Off, ByProgram, BySystem]
);
map_enum!(
    idle_action,
    idle_action_dto,
    IdleActionDto,
    IdleAction,
    [WarnOnly, WarnThenClose, Close]
);
map_enum!(
    report_ext_mode,
    report_ext_mode_dto,
    ReportExtModeDto,
    ReportExtMode,
    [Fixed, Dynamic]
);
