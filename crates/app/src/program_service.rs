//! Program Maintenance 程式維護 use case（TipTop: p_zz）
//! 規格：specs/features/azz/program_maintenance.feature

use std::sync::Arc;

use chrono::NaiveDate;

use ttrs_domain::{
    DomainError,
    program::{
        IdleAction, IdleControl, ModuleCode, OtherSettings, Program, ProgramAttrs, ProgramNo,
        ReportExtMode, ReportType, RuntimeSettings, check_length, max_len,
    },
};
use ttrs_shared::{
    AuditEntryDto, IdleActionDto, IdleControlDto, Page, ProgramDto, ProgramInput, ProgramNameDto,
    QueryRequest, ReportExtModeDto, ReportTypeDto,
};

use crate::{
    AppError,
    error::RepoError,
    ports::{AuditEntry, ProgramName, ProgramRepository},
};

/// 程式名稱的預設語系（輸入畫面上的「程式名稱」）
pub const DEFAULT_LANG: &str = "zh-TW";
/// 查詢每頁筆數上限
const MAX_PAGE_SIZE: u32 = 500;
/// 寫入異動紀錄時的作業代號
const AUDIT_PROGRAM: &str = "program-maintenance";

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
        let program = Program::new(
            no,
            module,
            attrs(&input),
            &actor.user_id,
            &actor.dept,
            today(),
        )?;
        self.check_query_definition(&program).await?;

        let names: Vec<ProgramName> = default_name(&input)?.into_iter().collect();
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

    /// QBE 查詢（p_zz_curs :281-359），依程式代號排序並分頁
    pub async fn query(&self, req: QueryRequest) -> Result<Page<ProgramDto>, AppError> {
        let filters = crate::program_query::filters(&req.conditions)?;
        let page = req.page.unwrap_or(1).max(1);
        let page_size = req
            .page_size
            .unwrap_or(QueryRequest::DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        let offset = u64::from(page - 1) * u64::from(page_size);
        let (programs, total) = self
            .repo
            .query(&filters, offset, u64::from(page_size))
            .await
            .map_err(infra)?;
        let mut items = Vec::with_capacity(programs.len());
        for p in &programs {
            items.push(self.to_dto(p).await?);
        }
        Ok(Page {
            items,
            total,
            page,
            page_size,
        })
    }

    /// 修改程式（p_zz_u :1313-1407）。`input.program_no` 與原代號不同時一併改代號，
    /// 連動名稱、選單與權限（:1388-1392；Q2：一律允許，TipTop 沒有獨立的改代號功能）。
    pub async fn update(
        &self,
        actor: &Actor,
        no: &str,
        input: ProgramInput,
    ) -> Result<ProgramDto, AppError> {
        let old = ProgramNo::parse(no)?;
        let mut program = self.load(&old).await?;
        let new = if input.program_no.trim().is_empty() {
            old.clone()
        } else {
            ProgramNo::parse(&input.program_no)?
        };
        // 先套用新代號再檢核，程式類別推導與 p_query 查詢定義都以新代號為準
        program.program_no = new.clone();
        let module = self.module(&input.module_code).await?;
        program.update(module, attrs(&input), &actor.user_id, today())?;
        let name = default_name(&input)?;
        self.check_query_definition(&program).await?;

        if new != old {
            match self.repo.change_no(&old, &new).await {
                Ok(()) => {}
                Err(RepoError::Conflict) => return Err(DomainError::ProgramAlreadyExists.into()),
                Err(e) => return Err(infra(e)),
            }
        }
        self.repo.update(&program).await.map_err(infra)?;
        // 名稱與報表抬頭（TipTop 另以 modify_program_name / modify_report_title 更新 gaz_file）
        if let Some(name) = name {
            self.repo
                .set_name(&program.program_no, &name)
                .await
                .map_err(infra)?;
        }
        self.to_dto(&program).await
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
        let program = source.copy_as(to, &actor.user_id, &actor.dept, today());
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
                report_title: n.report_title,
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
        let (name, report_title) = names
            .into_iter()
            .find(|n| n.lang == DEFAULT_LANG)
            .map_or((None, None), |n| {
                (Some(n.name).filter(|s| !s.is_empty()), n.report_title)
            });
        let r = &p.runtime;
        let o = &p.other;
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
            program_group: r.program_group.clone(),
            fixed_print_method: r.fixed_print_method.clone(),
            fixed_print_condition: r.fixed_print_condition.clone(),
            sql_explain: r.sql_explain,
            log_errors: r.log_errors,
            default_actions: o.default_actions.clone(),
            print_conditions: o.print_conditions,
            allow_key_update: o.allow_key_update,
            remark: o.remark.clone(),
            report_title,
            designed_on: p.designed_on,
            created_by: p.created_by.clone(),
            created_dept: p.created_dept.clone(),
            updated_by: p.updated_by.clone(),
            updated_on: p.updated_on,
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
            program_group: i.program_group.clone(),
            fixed_print_method: i.fixed_print_method.clone(),
            fixed_print_condition: i.fixed_print_condition.clone(),
            sql_explain: i.sql_explain,
            log_errors: i.log_errors,
        },
        other: OtherSettings {
            default_actions: i.default_actions.clone(),
            print_conditions: i.print_conditions,
            allow_key_update: i.allow_key_update,
            remark: i.remark.clone(),
        },
        designed_on: i.designed_on,
    }
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// 預設語系的名稱與報表抬頭（含長度檢核）；兩者皆空白時不建立
fn default_name(i: &ProgramInput) -> Result<Option<ProgramName>, DomainError> {
    let clean = |v: &Option<String>| {
        v.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let (name, report_title) = (clean(&i.name), clean(&i.report_title));
    check_length("程式名稱", name.as_deref(), max_len::NAME)?;
    check_length("報表抬頭", report_title.as_deref(), max_len::REPORT_TITLE)?;
    Ok(
        (name.is_some() || report_title.is_some()).then(|| ProgramName {
            lang: DEFAULT_LANG.to_owned(),
            name: name.unwrap_or_default(),
            report_title,
        }),
    )
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

#[cfg(test)]
mod tests {
    use ttrs_domain::program::max_len as d;
    use ttrs_shared::program_max_len as s;

    /// 畫面的 maxlength 必須與 domain 檢核一致
    #[test]
    fn shared_max_len_matches_domain() {
        let pairs = [
            (s::PROGRAM_NO, d::PROGRAM_NO),
            (s::MODULE_CODE, d::MODULE_CODE),
            (s::DEFAULT_ACTIONS, d::DEFAULT_ACTIONS),
            (s::RUN_COMMAND, d::RUN_COMMAND),
            (s::PROGRAM_GROUP, d::PROGRAM_GROUP),
            (s::FIXED_PRINT_CONDITION, d::FIXED_PRINT_CONDITION),
            (s::FIXED_PRINT_METHOD, d::FIXED_PRINT_METHOD),
            (s::WINDOW_STYLE, d::WINDOW_STYLE),
            (s::REMARK, d::REMARK),
            (s::NAME, d::NAME),
            (s::REPORT_TITLE, d::REPORT_TITLE),
        ];
        for (shared, domain) in pairs {
            assert_eq!(shared, domain);
        }
    }
}
