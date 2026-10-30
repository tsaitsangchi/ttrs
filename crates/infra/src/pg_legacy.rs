//! TipTop 對照目錄（legacy_catalog.feature）的 PostgreSQL 實作；表結構見 migrations/0009_legacy_catalog.sql

use async_trait::async_trait;
use ttrs_app::{
    error::RepoError,
    ports::{LegacyCatalogRepository, LegacyTableSummary},
};
use ttrs_domain::legacy_catalog::{LegacyColumn, LegacyTable};

use crate::{PgStore, pg::db_err};

type TableRow = (String, String, String, Option<String>, String);
type ColumnRow = (
    String,
    String,
    String,
    bool,
    String,
    Option<String>,
    bool,
    bool,
);

impl PgStore {
    async fn load_legacy(&self, row: Option<TableRow>) -> Result<Option<LegacyTable>, RepoError> {
        let Some((legacy_name, name, title, purpose, module)) = row else {
            return Ok(None);
        };
        let columns: Vec<ColumnRow> = sqlx::query_as(
            "SELECT legacy_name, name, data_type, primary_key, description, notes, unused, verify
             FROM legacy_column WHERE legacy_table = $1 ORDER BY position",
        )
        .bind(&legacy_name)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(Some(LegacyTable {
            legacy_name,
            name,
            title,
            purpose,
            module,
            columns: columns
                .into_iter()
                .map(
                    |(
                        legacy_name,
                        name,
                        data_type,
                        primary_key,
                        description,
                        notes,
                        unused,
                        verify,
                    )| {
                        LegacyColumn {
                            legacy_name,
                            name,
                            data_type,
                            primary_key,
                            description,
                            notes,
                            unused,
                            verify,
                        }
                    },
                )
                .collect(),
        }))
    }
}

#[async_trait]
impl LegacyCatalogRepository for PgStore {
    async fn replace_all(&self, tables: &[LegacyTable]) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query("DELETE FROM legacy_table")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        for t in tables {
            sqlx::query(
                "INSERT INTO legacy_table (legacy_name, name, title, purpose, module)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(&t.legacy_name)
            .bind(&t.name)
            .bind(&t.title)
            .bind(&t.purpose)
            .bind(&t.module)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
            for (i, c) in t.columns.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO legacy_column (legacy_table, legacy_name, position, name, data_type,
                                                primary_key, description, notes, unused, verify)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                )
                .bind(&t.legacy_name)
                .bind(&c.legacy_name)
                .bind(i32::try_from(i).unwrap_or(i32::MAX))
                .bind(&c.name)
                .bind(&c.data_type)
                .bind(c.primary_key)
                .bind(&c.description)
                .bind(&c.notes)
                .bind(c.unused)
                .bind(c.verify)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
            }
        }
        tx.commit().await.map_err(db_err)
    }

    async fn list_tables(&self) -> Result<Vec<LegacyTableSummary>, RepoError> {
        let rows: Vec<(String, String, String, String, i64)> = sqlx::query_as(
            "SELECT t.legacy_name, t.name, t.title, t.module,
                    (SELECT count(*) FROM legacy_column c WHERE c.legacy_table = t.legacy_name)
             FROM legacy_table t ORDER BY t.legacy_name COLLATE \"C\"",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|(legacy_name, name, title, module, n)| LegacyTableSummary {
                legacy_name,
                name,
                title,
                module,
                column_count: u32::try_from(n).unwrap_or(u32::MAX),
            })
            .collect())
    }

    async fn get(&self, legacy_name: &str) -> Result<Option<LegacyTable>, RepoError> {
        let row: Option<TableRow> = sqlx::query_as(
            "SELECT legacy_name, name, title, purpose, module FROM legacy_table WHERE legacy_name = $1",
        )
        .bind(legacy_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        self.load_legacy(row).await
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<LegacyTable>, RepoError> {
        let row: Option<TableRow> = sqlx::query_as(
            "SELECT legacy_name, name, title, purpose, module FROM legacy_table WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        self.load_legacy(row).await
    }
}
