//! p_zz 程式登錄的 PostgreSQL 實作（migrations/0003_program_registry.sql）。
//! 子表以 ON UPDATE / ON DELETE CASCADE 連動，改代號與刪除只需操作 program 表。

use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::{FromRow, Postgres, QueryBuilder};
use ttrs_app::{
    error::RepoError,
    ports::{AuditEntry, ProgramName, ProgramRepository},
};
use ttrs_domain::{
    program::{
        IdleAction, IdleControl, ModuleCode, OtherSettings, Program, ProgramNo, ReportExtMode,
        ReportType, RuntimeSettings,
    },
    qbe::{PatternPart, QbeFilter, QbeTerm, QbeValue},
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
    program_group: Option<String>,
    print_method: Option<String>,
    fixed_run_condition: Option<String>,
    help_enabled: bool,
    log_errors: bool,
    default_actions: Option<String>,
    print_option: bool,
    allow_key_update: bool,
    remark: Option<String>,
    designed_on: NaiveDate,
    created_by: String,
    created_dept: String,
    updated_by: Option<String>,
    updated_on: NaiveDate,
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
                program_group: r.program_group,
                print_method: r.print_method,
                fixed_run_condition: r.fixed_run_condition,
                help_enabled: r.help_enabled,
                log_errors: r.log_errors,
            },
            other: OtherSettings {
                default_actions: r.default_actions,
                print_option: r.print_option,
                allow_key_update: r.allow_key_update,
                remark: r.remark,
            },
            designed_on: r.designed_on,
            created_by: r.created_by,
            created_dept: r.created_dept,
            updated_by: r.updated_by,
            updated_on: r.updated_on,
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
                    report_ext_mode, program_group, print_method, fixed_run_condition,
                    help_enabled, log_errors, default_actions, print_option, allow_key_update,
                    remark, designed_on, created_by, created_dept, updated_by, updated_on
             FROM program WHERE program_no = $1",
        )
        .bind(no.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .map(Program::try_from)
        .transpose()
    }

    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Program>, u64), RepoError> {
        let mut count = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM program p WHERE TRUE");
        push_filters(&mut count, filters)?;
        let total: i64 = count
            .build_query_scalar()
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?;

        let mut rows = QueryBuilder::<Postgres>::new(
            "SELECT program_no, module_code, program_type, report_type, idle_control,
                    idle_seconds, idle_action, pass_book_no, deferred_print_count,
                    current_print_count, report_width, run_command, window_style,
                    report_ext_mode, program_group, print_method, fixed_run_condition,
                    help_enabled, log_errors, default_actions, print_option, allow_key_update,
                    remark, designed_on, created_by, created_dept, updated_by, updated_on
             FROM program p WHERE TRUE",
        );
        push_filters(&mut rows, filters)?;
        // 位元組順序排序（同 Oracle BINARY 排序與記憶體版）
        rows.push(r#" ORDER BY p.program_no COLLATE "C" LIMIT "#);
        rows.push_bind(i64::try_from(limit).unwrap_or(i64::MAX));
        rows.push(" OFFSET ");
        rows.push_bind(i64::try_from(offset).unwrap_or(i64::MAX));
        let programs = rows
            .build_query_as::<ProgramRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(Program::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok((programs, u64::try_from(total).unwrap_or(0)))
    }

    async fn list(&self) -> Result<Vec<Program>, RepoError> {
        sqlx::query_as::<_, ProgramRow>(
            "SELECT program_no, module_code, program_type, report_type, idle_control,
                    idle_seconds, idle_action, pass_book_no, deferred_print_count,
                    current_print_count, report_width, run_command, window_style,
                    report_ext_mode, program_group, print_method, fixed_run_condition,
                    help_enabled, log_errors, default_actions, print_option, allow_key_update,
                    remark, designed_on, created_by, created_dept, updated_by, updated_on
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
                    window_style, report_ext_mode, program_group, print_method,
                    fixed_run_condition, help_enabled, log_errors, created_by, created_dept,
                    updated_by, default_actions, print_option, allow_key_update, remark,
                    designed_on, updated_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                     $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28)",
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
        .bind(&r.program_group)
        .bind(&r.print_method)
        .bind(&r.fixed_run_condition)
        .bind(r.help_enabled)
        .bind(r.log_errors)
        .bind(&p.created_by)
        .bind(&p.created_dept)
        .bind(&p.updated_by)
        .bind(&p.other.default_actions)
        .bind(p.other.print_option)
        .bind(p.other.allow_key_update)
        .bind(&p.other.remark)
        .bind(p.designed_on)
        .bind(p.updated_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;

        for n in names {
            sqlx::query(
                "INSERT INTO program_name (program_no, lang, name, report_title)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(p.program_no.as_str())
            .bind(&n.lang)
            .bind(&n.name)
            .bind(&n.report_title)
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
                    program_group = $15, print_method = $16, fixed_run_condition = $17,
                    help_enabled = $18, log_errors = $19, updated_by = $20,
                    default_actions = $21, print_option = $22, allow_key_update = $23,
                    remark = $24, designed_on = $25, updated_on = $26, updated_at = now()
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
        .bind(&r.program_group)
        .bind(&r.print_method)
        .bind(&r.fixed_run_condition)
        .bind(r.help_enabled)
        .bind(r.log_errors)
        .bind(&p.updated_by)
        .bind(&p.other.default_actions)
        .bind(p.other.print_option)
        .bind(p.other.allow_key_update)
        .bind(&p.other.remark)
        .bind(p.designed_on)
        .bind(p.updated_on)
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
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT lang, name, report_title FROM program_name
             WHERE program_no = $1 ORDER BY lang",
        )
        .bind(no.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|(lang, name, report_title)| ProgramName {
                lang,
                name,
                report_title,
            })
            .collect())
    }

    async fn set_name(&self, no: &ProgramNo, name: &ProgramName) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO program_name (program_no, lang, name, report_title)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (program_no, lang)
             DO UPDATE SET name = EXCLUDED.name, report_title = EXCLUDED.report_title",
        )
        .bind(no.as_str())
        .bind(&name.lang)
        .bind(&name.name)
        .bind(&name.report_title)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
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

// ----- QBE → SQL：欄位名稱只來自下方白名單，條件值一律 push_bind -----

/// 查詢鍵值 → (資料庫欄位, 是否文字)；鍵值清單見 ttrs_app::program_query
fn column(field: &str) -> Option<(&'static str, bool)> {
    Some(match field {
        "program_no" => ("p.program_no", true),
        "module_code" => ("p.module_code", true),
        "program_group" => ("p.program_group", true),
        "window_style" => ("p.window_style", true),
        "program_type" => ("p.program_type", true),
        "run_command" => ("p.run_command", true),
        "fixed_run_condition" => ("p.fixed_run_condition", true),
        "designed_on" => ("p.designed_on", false),
        "print_method" => ("p.print_method", true),
        "report_type" => ("p.report_type", true),
        "report_ext_mode" => ("p.report_ext_mode", true),
        "report_width" => ("p.report_width", false),
        "current_print_count" => ("p.current_print_count", false),
        "deferred_print_count" => ("p.deferred_print_count", false),
        "print_option" => ("p.print_option", false),
        "idle_control" => ("p.idle_control", true),
        "idle_seconds" => ("p.idle_seconds", false),
        "idle_action" => ("p.idle_action", true),
        "log_errors" => ("p.log_errors", false),
        "allow_key_update" => ("p.allow_key_update", false),
        "help_enabled" => ("p.help_enabled", false),
        "remark" => ("p.remark", true),
        "created_by" => ("p.created_by", true),
        "updated_by" => ("p.updated_by", true),
        "created_dept" => ("p.created_dept", true),
        "updated_on" => ("p.updated_on", false),
        _ => return None,
    })
}

fn push_filters(qb: &mut QueryBuilder<Postgres>, filters: &[QbeFilter]) -> Result<(), RepoError> {
    for f in filters {
        qb.push(" AND (");
        for (i, term) in f.terms.iter().enumerate() {
            if i > 0 {
                qb.push(" OR ");
            }
            if f.field == "name" {
                // 程式名稱多語系（gaz_file）：任一語系符合即符合；空值＝沒有任何名稱
                if *term == QbeTerm::IsNull {
                    qb.push(
                        "NOT EXISTS (SELECT 1 FROM program_name n \
                         WHERE n.program_no = p.program_no AND n.name <> '')",
                    );
                } else {
                    qb.push("EXISTS (SELECT 1 FROM program_name n WHERE n.program_no = p.program_no AND ");
                    push_term(qb, "n.name", true, term);
                    qb.push(")");
                }
            } else {
                let (col, is_text) = column(f.field)
                    .ok_or_else(|| RepoError::Other(format!("未定義的查詢欄位：{}", f.field)))?;
                push_term(qb, col, is_text, term);
            }
        }
        qb.push(")");
    }
    Ok(())
}

fn push_term(qb: &mut QueryBuilder<Postgres>, col: &'static str, is_text: bool, term: &QbeTerm) {
    // 文字以位元組順序比較、區分大小寫（QBE2）
    let collate = if is_text { r#" COLLATE "C""# } else { "" };
    let mut cmp = |op: &str, v: &QbeValue| {
        qb.push(col).push(collate).push(op);
        push_value(qb, v);
    };
    match term {
        QbeTerm::IsNull => {
            if is_text {
                qb.push("(")
                    .push(col)
                    .push(" IS NULL OR ")
                    .push(col)
                    .push(" = '')");
            } else {
                qb.push(col).push(" IS NULL");
            }
        }
        QbeTerm::Eq(v) => cmp(" = ", v),
        QbeTerm::Ne(v) => cmp(" <> ", v),
        QbeTerm::Lt(v) => cmp(" < ", v),
        QbeTerm::Le(v) => cmp(" <= ", v),
        QbeTerm::Gt(v) => cmp(" > ", v),
        QbeTerm::Ge(v) => cmp(" >= ", v),
        QbeTerm::Range(lo, hi) => {
            cmp(" BETWEEN ", lo);
            qb.push(" AND ");
            push_value(qb, hi);
        }
        QbeTerm::Like(parts) => {
            qb.push(col).push(collate).push(r" LIKE ");
            qb.push_bind(like_pattern(parts));
            qb.push(r" ESCAPE '\'");
        }
    }
}

fn push_value(qb: &mut QueryBuilder<Postgres>, v: &QbeValue) {
    match v {
        QbeValue::Text(s) => qb.push_bind(s.clone()),
        QbeValue::Integer(n) => qb.push_bind(*n),
        QbeValue::Date(d) => qb.push_bind(*d),
        QbeValue::Flag(b) => qb.push_bind(*b),
    };
}

/// `*` → `%`、`?` → `_`；字面的 `%` `_` `\` 以反斜線跳脫
fn like_pattern(parts: &[PatternPart]) -> String {
    let mut out = String::new();
    for p in parts {
        match p {
            PatternPart::AnyMany => out.push('%'),
            PatternPart::AnyOne => out.push('_'),
            PatternPart::Literal(s) => {
                for c in s.chars() {
                    if matches!(c, '%' | '_' | '\\') {
                        out.push('\\');
                    }
                    out.push(c);
                }
            }
        }
    }
    out
}
