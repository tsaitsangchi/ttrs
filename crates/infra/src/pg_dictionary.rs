//! 資料字典的 PostgreSQL 實作（migrations/0008_data_dictionary.sql）。
//! 資料表結構由系統目錄（pg_catalog）唯讀取得，只看目前 schema（current_schema()）。

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{
    error::RepoError,
    ports::{
        ColumnSchema, ConstraintKind, ConstraintSchema, DictionaryRepository, IndexSchema,
        MigrationSource, SchemaCatalog, TableSchema,
    },
};
use ttrs_domain::record::Stamp;
use ttrs_domain::{
    dictionary::{ColumnText, Lang, TableDictionary, TableText, TableType},
    program::ModuleCode,
};

use crate::{
    MIGRATOR,
    pg::{PgStore, db_err},
};

fn bad(what: &str, v: &str) -> RepoError {
    RepoError::Other(format!("資料庫內容不合法：{what}={v}"))
}

/// format_type() 的結果轉成較短的寫法，如 character varying(20) → varchar(20)
fn normalize_type(t: &str) -> String {
    let t = t.trim();
    for (long, short) in [
        ("character varying", "varchar"),
        ("timestamp with time zone", "timestamptz"),
        ("timestamp without time zone", "timestamp"),
        ("character", "char"),
    ] {
        if let Some(rest) = t.strip_prefix(long) {
            return format!("{short}{rest}");
        }
    }
    t.to_owned()
}

// ----- 結構（唯讀） -----

#[async_trait]
impl SchemaCatalog for PgStore {
    async fn table_names(&self) -> Result<Vec<String>, RepoError> {
        sqlx::query_scalar(
            "SELECT c.relname::text FROM pg_class c
             JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = current_schema() AND c.relkind IN ('r', 'p')
               AND c.relname <> '_sqlx_migrations'
             ORDER BY c.relname",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)
    }

    async fn table(&self, name: &str) -> Result<Option<TableSchema>, RepoError> {
        let columns: Vec<(String, String, bool, Option<String>)> = sqlx::query_as(
            "SELECT a.attname::text, format_type(a.atttypid, a.atttypmod), a.attnotnull,
                    pg_get_expr(d.adbin, d.adrelid)
             FROM pg_attribute a
             JOIN pg_class c ON c.oid = a.attrelid
             JOIN pg_namespace n ON n.oid = c.relnamespace
             LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
             WHERE n.nspname = current_schema() AND c.relname = $1 AND c.relkind IN ('r', 'p')
               AND a.attnum > 0 AND NOT a.attisdropped
             ORDER BY a.attnum",
        )
        .bind(name)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        if columns.is_empty() {
            return Ok(None);
        }

        // (名稱, 類型, 欄位, 參照表, 參照欄位, 參照的主鍵/唯一限制, 檢查條件)
        type ConstraintRow = (
            String,
            String,
            Vec<String>,
            Option<String>,
            Vec<String>,
            Option<String>,
            Option<String>,
        );
        let constraints: Vec<ConstraintRow> =
            sqlx::query_as(
                "SELECT con.conname::text, con.contype::text,
                        ARRAY(SELECT a.attname::text FROM unnest(con.conkey) WITH ORDINALITY k(n, ord)
                              JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = k.n
                              ORDER BY k.ord),
                        rc.relname::text,
                        ARRAY(SELECT a.attname::text FROM unnest(con.confkey) WITH ORDINALITY k(n, ord)
                              JOIN pg_attribute a ON a.attrelid = con.confrelid AND a.attnum = k.n
                              ORDER BY k.ord),
                        (SELECT rk.conname::text FROM pg_constraint rk
                          WHERE con.contype = 'f' AND rk.conrelid = con.confrelid
                            AND rk.conindid = con.conindid AND rk.contype IN ('p', 'u')),
                        CASE WHEN con.contype = 'c' THEN pg_get_constraintdef(con.oid) END
                 FROM pg_constraint con
                 JOIN pg_class c ON c.oid = con.conrelid
                 JOIN pg_namespace n ON n.oid = c.relnamespace
                 LEFT JOIN pg_class rc ON rc.oid = con.confrelid
                 WHERE n.nspname = current_schema() AND c.relname = $1
                   AND con.contype IN ('p', 'f', 'u', 'c')
                 ORDER BY con.conname",
            )
            .bind(name)
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;

        // 不列出由限制自動建立的索引（主鍵、唯一）
        let indexes: Vec<(String, bool, Vec<String>)> = sqlx::query_as(
            "SELECT i.relname::text, ix.indisunique,
                    ARRAY(SELECT a.attname::text FROM unnest(ix.indkey::int2[]) WITH ORDINALITY k(n, ord)
                          JOIN pg_attribute a ON a.attrelid = ix.indrelid AND a.attnum = k.n
                          ORDER BY k.ord)
             FROM pg_index ix
             JOIN pg_class i ON i.oid = ix.indexrelid
             JOIN pg_class t ON t.oid = ix.indrelid
             JOIN pg_namespace n ON n.oid = t.relnamespace
             WHERE n.nspname = current_schema() AND t.relname = $1
               AND NOT EXISTS (SELECT 1 FROM pg_constraint con WHERE con.conindid = ix.indexrelid)
             ORDER BY i.relname",
        )
        .bind(name)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(Some(TableSchema {
            name: name.to_owned(),
            columns: columns
                .into_iter()
                .map(|(name, data_type, not_null, default)| ColumnSchema {
                    name,
                    data_type: normalize_type(&data_type),
                    not_null,
                    default,
                })
                .collect(),
            indexes: indexes
                .into_iter()
                .map(|(name, unique, columns)| IndexSchema {
                    name,
                    unique,
                    columns,
                })
                .collect(),
            constraints: constraints
                .into_iter()
                .filter_map(
                    |(name, kind, columns, ref_table, ref_columns, ref_constraint, check)| {
                        let kind = match kind.as_str() {
                            "p" => ConstraintKind::PrimaryKey,
                            "f" => ConstraintKind::ForeignKey,
                            "u" => ConstraintKind::Unique,
                            "c" => ConstraintKind::Check,
                            _ => return None,
                        };
                        Some(ConstraintSchema {
                            name,
                            kind,
                            columns,
                            ref_table,
                            ref_columns,
                            ref_constraint,
                            check_clause: check,
                        })
                    },
                )
                .collect(),
        }))
    }

    async fn migrations(&self) -> Result<Vec<MigrationSource>, RepoError> {
        Ok(MIGRATOR
            .iter()
            .filter(|m| m.migration_type.is_up_migration())
            .map(|m| MigrationSource {
                version: m.version,
                description: m.description.to_string(),
                sql: m.sql.as_str().to_owned(),
            })
            .collect())
    }
}

// ----- 字典與多語言說明 -----

#[derive(FromRow)]
struct DictRow {
    table_name: String,
    module_code: String,
    table_type: Option<String>,
    upstream: Option<String>,
    downstream: Option<String>,
    created_by: String,
    created_on: NaiveDate,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

impl TryFrom<DictRow> for TableDictionary {
    type Error = RepoError;

    fn try_from(r: DictRow) -> Result<Self, Self::Error> {
        Ok(TableDictionary {
            module: ModuleCode::parse(&r.module_code)
                .map_err(|_| bad("module_code", &r.module_code))?,
            table_type: r
                .table_type
                .map(|t| TableType::from_code(&t).ok_or_else(|| bad("table_type", &t)))
                .transpose()?,
            table_name: r.table_name,
            upstream: r.upstream,
            downstream: r.downstream,
            created_by: r.created_by,
            created_on: r.created_on,
            stamp: Stamp {
                version: r.version,
                created_at: Some(r.created_at),
                updated_at: r.updated_at,
            },
        })
    }
}

fn lang(v: &str) -> Result<Lang, RepoError> {
    Lang::parse(v).map_err(|_| bad("lang", v))
}

#[async_trait]
impl DictionaryRepository for PgStore {
    async fn get(&self, table: &str) -> Result<Option<TableDictionary>, RepoError> {
        sqlx::query_as::<_, DictRow>(
            "SELECT table_name, module_code, table_type, upstream, downstream, created_by, created_on, version, created_at, updated_at
             FROM table_dictionary WHERE table_name = $1",
        )
        .bind(table)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .map(TableDictionary::try_from)
        .transpose()
    }

    async fn list(&self) -> Result<Vec<TableDictionary>, RepoError> {
        sqlx::query_as::<_, DictRow>(
            "SELECT table_name, module_code, table_type, upstream, downstream, created_by, created_on, version, created_at, updated_at
             FROM table_dictionary ORDER BY table_name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?
        .into_iter()
        .map(TableDictionary::try_from)
        .collect()
    }

    async fn insert(&self, d: &TableDictionary) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO table_dictionary
                (table_name, module_code, table_type, upstream, downstream, created_by, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(&d.table_name)
        .bind(d.module.as_str())
        .bind(d.table_type.map(TableType::code))
        .bind(&d.upstream)
        .bind(&d.downstream)
        .bind(&d.created_by)
        .bind(d.created_on)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update(&self, d: &TableDictionary) -> Result<(), RepoError> {
        let n = sqlx::query(
            "UPDATE table_dictionary SET module_code = $2, table_type = $3, upstream = $4,
                    downstream = $5, updated_at = now()
             WHERE table_name = $1 AND version = $6",
        )
        .bind(&d.table_name)
        .bind(d.module.as_str())
        .bind(d.table_type.map(TableType::code))
        .bind(&d.upstream)
        .bind(&d.downstream)
        .bind(d.stamp.version)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        Ok(())
    }

    async fn delete(&self, table: &str) -> Result<(), RepoError> {
        // 多語言說明由外鍵 ON DELETE CASCADE 一併刪除（D3）
        sqlx::query("DELETE FROM table_dictionary WHERE table_name = $1")
            .bind(table)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn set_table_text(&self, table: &str, t: &TableText) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO table_dictionary_text (table_name, lang, name, purpose, remark)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (table_name, lang)
             DO UPDATE SET name = EXCLUDED.name, purpose = EXCLUDED.purpose, remark = EXCLUDED.remark",
        )
        .bind(table)
        .bind(t.lang.as_str())
        .bind(&t.name)
        .bind(&t.purpose)
        .bind(&t.remark)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn table_texts(&self, table: &str) -> Result<Vec<TableText>, RepoError> {
        let rows: Vec<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT lang, name, purpose, remark FROM table_dictionary_text
             WHERE table_name = $1 ORDER BY lang",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.into_iter()
            .map(|(l, name, purpose, remark)| {
                Ok(TableText {
                    lang: lang(&l)?,
                    name,
                    purpose,
                    remark,
                })
            })
            .collect()
    }

    async fn set_column_text(&self, table: &str, t: &ColumnText) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO column_dictionary_text
                (table_name, column_name, lang, name, description, extra)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (table_name, column_name, lang)
             DO UPDATE SET name = EXCLUDED.name, description = EXCLUDED.description,
                           extra = EXCLUDED.extra, updated_on = CURRENT_DATE",
        )
        .bind(table)
        .bind(&t.column_name)
        .bind(t.lang.as_str())
        .bind(&t.name)
        .bind(&t.description)
        .bind(&t.extra)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn column_texts(&self, table: &str) -> Result<Vec<ColumnText>, RepoError> {
        let rows: Vec<(
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT column_name, lang, name, description, extra FROM column_dictionary_text
                 WHERE table_name = $1 ORDER BY column_name, lang",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.into_iter()
            .map(|(column_name, l, name, description, extra)| {
                Ok(ColumnText {
                    column_name,
                    lang: lang(&l)?,
                    name,
                    description,
                    extra,
                })
            })
            .collect()
    }

    async fn module_exists(&self, module: &ModuleCode) -> Result<bool, RepoError> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM module WHERE module_code = $1)")
            .bind(module.as_str())
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }
}
