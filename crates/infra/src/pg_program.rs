//! p_zz 程式登錄的 PostgreSQL 實作（migrations/0003_program_registry.sql）。
//! 子表以 ON UPDATE / ON DELETE CASCADE 連動，改代號與刪除只需操作 program 表。

use async_trait::async_trait;
use sqlx::FromRow;
use ttrs_app::{
    error::RepoError,
    ports::{AuditEntry, ProgramName, ProgramRepository},
};
use ttrs_domain::program::{
    IdleAction, IdleControl, ModuleCode, Program, ProgramNo, ReportExtMode, ReportType,
    RuntimeSettings,
};

use crate::pg::{PgStore, db_err};

#[derive(FromRow)]
struct ProgramRow {
    program_no: String,
    module_code: String,
    program_type: Option<String>,
    report_type: String,
    idle_control: String,
    idle_seconds: Option<i32>,
    idle_action: Option<String>,
    pass_book_no: bool,
    deferred_print_count: i32,
    current_print_count: i32,
    report_width: Option<i32>,
    run_command: Option<String>,
    window_style: Option<String>,
    report_ext_mode: Option<String>,
    print_setting: Option<String>,
    print_default: Option<String>,
    quick_run_data: Option<String>,
    help_enabled: bool,
    log_errors: bool,
    created_by: String,
    created_dept: String,
    updated_by: Option<String>,
}

fn bad(what: &str, v: &str) -> RepoError {
    RepoError::Other(format!("資料庫內容不合法：{what}={v}"))
}

fn decode<T>(what: &str, v: &str, f: fn(&str) -> Option<T>) -> Result<T, RepoError> {
    f(v).ok_or_else(|| bad(what, v))
}

fn decode_opt<T>(
    what: &str,
    v: Option<String>,
    f: fn(&str) -> Option<T>,
) -> Result<Option<T>, RepoError> {
    v.map(|v| decode(what, &v, f)).transpose()
}

impl TryFrom<ProgramRow> for Program {
    type Error = RepoError;

    fn try_from(r: ProgramRow) -> Result<Self, Self::Error> {
        let module = match ModuleCode::parse(&r.module_code) {
            Ok(m) => m,
            Err(_) => return Err(bad("module_code", &r.module_code)),
        };
        Ok(Program {
            program_no: ProgramNo::parse(&r.program_no)
                .map_err(|_| bad("program_no", &r.program_no))?,
            module,
            program_type: r.program_type,
            report_type: decode("report_type", &r.report_type, ReportType::from_code)?,
            idle_control: decode("idle_control", &r.idle_control, IdleControl::from_code)?,
            idle_seconds: r.idle_seconds,
            idle_action: decode_opt("idle_action", r.idle_action, IdleAction::from_code)?,
            pass_book_no: r.pass_book_no,
            deferred_print_count: r.deferred_print_count,
            current_print_count: r.current_print_count,
            report_width: r.report_width,
            runtime: RuntimeSettings {
                run_command: r.run_command,
                window_style: r.window_style,
                report_ext_mode: decode_opt(
                    "report_ext_mode",
                    r.report_ext_mode,
                    ReportExtMode::from_code,
                )?,
                print_setting: r.print_setting,
                print_default: r.print_default,
                quick_run_data: r.quick_run_data,
                help_enabled: r.help_enabled,
                log_errors: r.log_errors,
            },
            created_by: r.created_by,
            created_dept: r.created_dept,
            updated_by: r.updated_by,
        })
    }
}

#[async_trait]
impl ProgramRepository for PgStore {
    async fn get(&self, no: &ProgramNo) -> Result<Option<Program>, RepoError> {
        sqlx::query_as::<_, ProgramRow>(
            "SELECT program_no, module_code, program_type, report_type, idle_control,
                    idle_seconds, idle_action, pass_book_no, deferred_print_count,
                    current_print_count, report_width, run_command, window_style,
                    report_ext_mode, print_setting, print_default, quick_run_data,
                    help_enabled, log_errors, created_by, created_dept, updated_by
             FROM program WHERE program_no = $1",
        )
        .bind(no.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .map(Program::try_from)
        .transpose()
    }

    async fn list(&self) -> Result<Vec<Program>, RepoError> {
        sqlx::query_as::<_, ProgramRow>(
            "SELECT program_no, module_code, program_type, report_type, idle_control,
                    idle_seconds, idle_action, pass_book_no, deferred_print_count,
                    current_print_count, report_width, run_command, window_style,
                    report_ext_mode, print_setting, print_default, quick_run_data,
                    help_enabled, log_errors, created_by, created_dept, updated_by
             FROM program ORDER BY program_no",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?
        .into_iter()
        .map(Program::try_from)
        .collect()
    }

    async fn insert(
        &self,
        p: &Program,
        names: &[ProgramName],
        grant_to: Option<&str>,
    ) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let r = &p.runtime;
        sqlx::query(
            "INSERT INTO program (program_no, module_code, program_type, report_type,
                    idle_control, idle_seconds, idle_action, pass_book_no,
                    deferred_print_count, current_print_count, report_width, run_command,
                    window_style, report_ext_mode, print_setting, print_default,
                    quick_run_data, help_enabled, log_errors, created_by, created_dept,
                    updated_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                     $16, $17, $18, $19, $20, $21, $22)",
        )
        .bind(p.program_no.as_str())
        .bind(p.module.as_str())
        .bind(&p.program_type)
        .bind(p.report_type.code())
        .bind(p.idle_control.code())
        .bind(p.idle_seconds)
        .bind(p.idle_action.map(IdleAction::code))
        .bind(p.pass_book_no)
        .bind(p.deferred_print_count)
        .bind(p.current_print_count)
        .bind(p.report_width)
        .bind(&r.run_command)
        .bind(&r.window_style)
        .bind(r.report_ext_mode.map(ReportExtMode::code))
        .bind(&r.print_setting)
        .bind(&r.print_default)
        .bind(&r.quick_run_data)
        .bind(r.help_enabled)
        .bind(r.log_errors)
        .bind(&p.created_by)
        .bind(&p.created_dept)
        .bind(&p.updated_by)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;

        for n in names {
            sqlx::query("INSERT INTO program_name (program_no, lang, name) VALUES ($1, $2, $3)")
                .bind(p.program_no.as_str())
                .bind(&n.lang)
                .bind(&n.name)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        if let Some(class) = grant_to {
            sqlx::query(
                "INSERT INTO program_permission (class_code, program_no) VALUES ($1, $2)
                 ON CONFLICT DO NOTHING",
            )
            .bind(class)
            .bind(p.program_no.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn update(&self, p: &Program) -> Result<(), RepoError> {
        let r = &p.runtime;
        sqlx::query(
            "UPDATE program SET module_code = $2, program_type = $3, report_type = $4,
                    idle_control = $5, idle_seconds = $6, idle_action = $7, pass_book_no = $8,
                    deferred_print_count = $9, current_print_count = $10, report_width = $11,
                    run_command = $12, window_style = $13, report_ext_mode = $14,
                    print_setting = $15, print_default = $16, quick_run_data = $17,
                    help_enabled = $18, log_errors = $19, updated_by = $20, updated_at = now()
             WHERE program_no = $1",
        )
        .bind(p.program_no.as_str())
        .bind(p.module.as_str())
        .bind(&p.program_type)
        .bind(p.report_type.code())
        .bind(p.idle_control.code())
        .bind(p.idle_seconds)
        .bind(p.idle_action.map(IdleAction::code))
        .bind(p.pass_book_no)
        .bind(p.deferred_print_count)
        .bind(p.current_print_count)
        .bind(p.report_width)
        .bind(&r.run_command)
        .bind(&r.window_style)
        .bind(r.report_ext_mode.map(ReportExtMode::code))
        .bind(&r.print_setting)
        .bind(&r.print_default)
        .bind(&r.quick_run_data)
        .bind(r.help_enabled)
        .bind(r.log_errors)
        .bind(&p.updated_by)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn change_no(&self, old: &ProgramNo, new: &ProgramNo) -> Result<(), RepoError> {
        sqlx::query("UPDATE program SET program_no = $2 WHERE program_no = $1")
            .bind(old.as_str())
            .bind(new.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn delete(&self, no: &ProgramNo, audit: &AuditEntry) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query("DELETE FROM program WHERE program_no = $1")
            .bind(no.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query(
            "INSERT INTO audit_log (program, user_id, key, action) VALUES ($1, $2, $3, $4)",
        )
        .bind(&audit.program)
        .bind(&audit.user_id)
        .bind(&audit.key)
        .bind(&audit.action)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn names(&self, no: &ProgramNo) -> Result<Vec<ProgramName>, RepoError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT lang, name FROM program_name WHERE program_no = $1 ORDER BY lang",
        )
        .bind(no.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|(lang, name)| ProgramName { lang, name })
            .collect())
    }

    async fn grant(&self, permission_class: &str, no: &ProgramNo) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO program_permission (class_code, program_no) VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(permission_class)
        .bind(no.as_str())
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn permission_classes(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError> {
        sqlx::query_scalar(
            "SELECT class_code FROM program_permission WHERE program_no = $1 ORDER BY class_code",
        )
        .bind(no.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)
    }

    async fn menus_containing(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError> {
        sqlx::query_scalar("SELECT menu_no FROM menu_item WHERE program_no = $1 ORDER BY menu_no")
            .bind(no.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)
    }

    async fn audit_entries(&self, key: &str) -> Result<Vec<AuditEntry>, RepoError> {
        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT program, user_id, key, action FROM audit_log WHERE key = $1 ORDER BY audit_id",
        )
        .bind(key)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|(program, user_id, key, action)| AuditEntry {
                program,
                user_id,
                key,
                action,
            })
            .collect())
    }

    async fn module_exists(&self, module: &ModuleCode) -> Result<bool, RepoError> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM module WHERE module_code = $1)")
            .bind(module.as_str())
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }

    async fn permission_class_exists(&self, class: &str) -> Result<bool, RepoError> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM permission_class WHERE class_code = $1)")
            .bind(class)
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }

    async fn query_definition_exists(&self, query: &str) -> Result<bool, RepoError> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM query_definition WHERE query_no = $1)")
            .bind(query)
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }
}
