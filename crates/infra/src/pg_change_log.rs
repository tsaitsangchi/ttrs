//! 資料異動記錄查詢（change_log，docs/adr/0012-change-history.md）；寫入由觸發器 ttrs_change_log 完成。
//! 欄位值以 jsonb_each_text 取文字，與記憶體後端（ttrs_app::change_log::text）相同。

use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use ttrs_app::{
    change_log::{ChangeRecord, Row},
    error::RepoError,
    ports::ChangeLogRepository,
};
use ttrs_domain::plant::PlantId;

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct ChangeRow {
    change_id: i64,
    occurred_at: DateTime<Utc>,
    table_name: String,
    operation: String,
    changed_fields: Vec<String>,
    version: Option<i32>,
    user_id: String,
    user_dept: Option<String>,
    actor_kind: String,
    on_behalf_of: Option<String>,
    program: Option<String>,
    action: Option<String>,
    request_id: Option<String>,
    client_ip: Option<String>,
    user_agent: Option<String>,
    reason: Option<String>,
    key_cols: Vec<String>,
    key_vals: Vec<Option<String>>,
    has_before: bool,
    before_cols: Vec<String>,
    before_vals: Vec<Option<String>>,
    has_after: bool,
    after_cols: Vec<String>,
    after_vals: Vec<Option<String>>,
}

fn row(has: bool, cols: Vec<String>, vals: Vec<Option<String>>) -> Option<Row> {
    has.then(|| cols.into_iter().zip(vals).collect())
}

impl From<ChangeRow> for ChangeRecord {
    fn from(r: ChangeRow) -> Self {
        ChangeRecord {
            change_id: r.change_id,
            occurred_at: r.occurred_at,
            table_name: r.table_name,
            record_key: r
                .key_cols
                .into_iter()
                .zip(r.key_vals)
                .map(|(k, v)| (k, v.unwrap_or_default()))
                .collect(),
            operation: r.operation,
            before: row(r.has_before, r.before_cols, r.before_vals),
            after: row(r.has_after, r.after_cols, r.after_vals),
            changed_fields: r.changed_fields,
            version: r.version,
            user_id: r.user_id,
            user_dept: r.user_dept,
            actor_kind: r.actor_kind,
            on_behalf_of: r.on_behalf_of,
            program: r.program,
            action: r.action,
            request_id: r.request_id,
            client_ip: r.client_ip,
            user_agent: r.user_agent,
            reason: r.reason,
        }
    }
}

#[async_trait]
impl ChangeLogRepository for PgStore {
    async fn history(
        &self,
        plant: Option<&PlantId>,
        table: &str,
        key: &BTreeMap<String, String>,
    ) -> Result<Vec<ChangeRecord>, RepoError> {
        // 共用層的表（plant 為 None）在第二階段加入 public.change_log
        let Some(plant) = plant else {
            return Ok(Vec::new());
        };
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let key = serde_json::to_string(key).map_err(|e| RepoError::Other(e.to_string()))?;
        let rows: Vec<ChangeRow> = sqlx::query_as(
            "SELECT change_id, occurred_at, table_name, operation, changed_fields, version,
                    user_id, user_dept, actor_kind, on_behalf_of, program, action,
                    request_id::text AS request_id, host(client_ip) AS client_ip, user_agent, reason,
                    ARRAY(SELECT k FROM jsonb_object_keys(record_key) k ORDER BY k) AS key_cols,
                    ARRAY(SELECT record_key ->> k FROM jsonb_object_keys(record_key) k ORDER BY k) AS key_vals,
                    before IS NOT NULL AS has_before,
                    ARRAY(SELECT e.key FROM jsonb_each_text(before) e ORDER BY e.key) AS before_cols,
                    ARRAY(SELECT e.value FROM jsonb_each_text(before) e ORDER BY e.key) AS before_vals,
                    after IS NOT NULL AS has_after,
                    ARRAY(SELECT e.key FROM jsonb_each_text(after) e ORDER BY e.key) AS after_cols,
                    ARRAY(SELECT e.value FROM jsonb_each_text(after) e ORDER BY e.key) AS after_vals
               FROM change_log
              WHERE table_name = $1 AND record_key = $2::jsonb
              ORDER BY change_id",
        )
        .bind(table)
        .bind(key)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(rows.into_iter().map(ChangeRecord::from).collect())
    }
}
