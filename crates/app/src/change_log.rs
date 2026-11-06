//! 資料異動記錄（docs/adr/0012-change-history.md；specs/features/azz/change_history.feature）。
//! 取代 TipTop azo_file（各程式自行寫入的重要資料修改記錄）與 cl_log 文字記錄檔。
//!
//! - **操作情境**（§3）：use case 以 [`run`] 包住寫入，記下操作者、程式、功能、請求代號；
//!   PostgreSQL 在交易開始時寫進 `ttrs.*` 設定供觸發器讀取，記憶體後端直接讀 [`current`]。
//!   API 以 [`with_request`] 帶入請求代號、來源位址與瀏覽器。
//! - **異動記錄**（§2）：改前／改後整筆內容（欄位值一律為文字，與 PostgreSQL `jsonb_each_text` 相同）。

use std::{cell::RefCell, collections::BTreeMap, future::Future};

use chrono::{DateTime, Utc};

use crate::authorization::{Actor, ActorKind};

/// 一筆資料的欄位值（欄位名稱 → 文字；NULL 為 None）
pub type Row = BTreeMap<String, Option<String>>;

/// 不列入「變動欄位」的管理欄位（每次修改都會變）
pub const META_COLUMNS: [&str; 5] = [
    "version",
    "updated_by",
    "updated_on",
    "updated_at",
    "created_at",
];

/// 遮蔽後的值（§7）
pub const MASK: &str = "***";

/// 一個 API 請求的資訊（同一請求內的多筆異動共用 request_id，AU15）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestInfo {
    pub request_id: String,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
}

/// 操作情境（§2 的 user_id … reason）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeContext {
    pub user_id: String,
    pub user_dept: String,
    /// user／ai／service
    pub actor_kind: &'static str,
    pub on_behalf_of: Option<String>,
    pub program: String,
    pub action: String,
    /// 業務動作（activate、deactivate…）；有值時取代 update（§3）
    pub operation: Option<String>,
    pub request: RequestInfo,
    pub reason: Option<String>,
}

tokio::task_local! {
    static REQUEST: RequestInfo;
    static CURRENT: RefCell<ChangeContext>;
}

/// API 請求範圍（中介層呼叫）
pub async fn with_request<F: Future>(info: RequestInfo, f: F) -> F::Output {
    REQUEST.scope(info, f).await
}

/// 在操作情境內執行寫入；沒有 API 請求範圍時（例如測試、背景作業）自動產生請求代號
pub async fn run<F: Future>(actor: &Actor, program: &str, action: &str, f: F) -> F::Output {
    let request = REQUEST
        .try_with(Clone::clone)
        .unwrap_or_else(|_| RequestInfo {
            request_id: new_request_id(),
            ..RequestInfo::default()
        });
    let (actor_kind, on_behalf_of) = match actor.kind {
        ActorKind::Person => ("user", None),
        ActorKind::Ai => ("ai", Some(actor.user_id.clone())),
        ActorKind::Service => ("service", None),
    };
    let ctx = ChangeContext {
        user_id: actor.user_id.clone(),
        user_dept: actor.dept.clone(),
        actor_kind,
        on_behalf_of,
        program: program.to_owned(),
        action: action.to_owned(),
        operation: None,
        request,
        reason: None,
    };
    CURRENT.scope(RefCell::new(ctx), f).await
}

/// 標示業務動作（啟用、停用…），在 [`run`] 範圍內呼叫
pub fn set_operation(operation: &str) {
    let _ = CURRENT.try_with(|c| c.borrow_mut().operation = Some(operation.to_owned()));
}

/// 目前的操作情境；不在 [`run`] 範圍內時為 None（直接 SQL 的情形，記為 db）
pub fn current() -> Option<ChangeContext> {
    CURRENT.try_with(|c| c.borrow().clone()).ok()
}

/// UUID v4 格式的請求代號
pub fn new_request_id() -> String {
    let mut b = [0u8; 16];
    // 亂數失敗時退回時間戳，請求代號只用於串接記錄
    if getrandom::fill(&mut b).is_err() {
        let n = Utc::now().timestamp_nanos_opt().unwrap_or_default();
        b[..8].copy_from_slice(&n.to_le_bytes());
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// 一筆異動記錄（change_log）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRecord {
    pub change_id: i64,
    pub occurred_at: DateTime<Utc>,
    pub table_name: String,
    pub record_key: BTreeMap<String, String>,
    /// insert／update／delete 或業務動作
    pub operation: String,
    pub before: Option<Row>,
    pub after: Option<Row>,
    pub changed_fields: Vec<String>,
    pub version: Option<i32>,
    pub user_id: String,
    pub user_dept: Option<String>,
    /// user／ai／service／db
    pub actor_kind: String,
    pub on_behalf_of: Option<String>,
    pub program: Option<String>,
    pub action: Option<String>,
    pub request_id: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub reason: Option<String>,
}

/// 變動欄位：新增、刪除為全部欄位；修改為值不同的欄位。管理欄位不列入（與觸發器 ttrs_change_log 相同）
pub fn changed_fields(before: Option<&Row>, after: Option<&Row>) -> Vec<String> {
    let mut cols: Vec<String> = match (before, after) {
        (Some(b), Some(a)) => a
            .iter()
            .filter(|(k, v)| b.get(*k) != Some(v))
            .map(|(k, _)| k.clone())
            .collect(),
        (Some(r), None) | (None, Some(r)) => r.keys().cloned().collect(),
        (None, None) => Vec::new(),
    };
    cols.retain(|c| !META_COLUMNS.contains(&c.as_str()));
    cols
}

/// 遮蔽個資欄位（§7、CH6）：有值時以 *** 取代
pub fn mask(row: &mut Row, masked: &[&str]) {
    for c in masked {
        if let Some(v @ Some(_)) = row.get_mut(*c) {
            *v = Some(MASK.to_owned());
        }
    }
}

/// 欄位值的文字表示（與 PostgreSQL jsonb_each_text 相同）
pub mod text {
    use chrono::NaiveDate;

    pub fn s(v: &str) -> Option<String> {
        Some(v.to_owned())
    }
    pub fn opt(v: Option<&str>) -> Option<String> {
        v.map(str::to_owned)
    }
    pub fn b(v: bool) -> Option<String> {
        Some(v.to_string())
    }
    pub fn date(v: NaiveDate) -> Option<String> {
        Some(v.format("%Y-%m-%d").to_string())
    }
    pub fn opt_date(v: Option<NaiveDate>) -> Option<String> {
        v.map(|d| d.format("%Y-%m-%d").to_string())
    }
    pub fn int(v: i64) -> Option<String> {
        Some(v.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pairs: &[(&str, Option<&str>)]) -> Row {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.map(str::to_owned)))
            .collect()
    }

    #[test]
    fn changed_fields_skip_meta_columns() {
        let b = row(&[("name", Some("A")), ("version", Some("1")), ("x", None)]);
        let a = row(&[("name", Some("B")), ("version", Some("2")), ("x", None)]);
        assert_eq!(changed_fields(Some(&b), Some(&a)), vec!["name"]);
        assert_eq!(changed_fields(None, Some(&a)), vec!["name", "x"]);
    }

    #[test]
    fn mask_only_non_null() {
        let mut r = row(&[("email", Some("a@x")), ("phone", None)]);
        mask(&mut r, &["email", "phone"]);
        assert_eq!(r, row(&[("email", Some(MASK)), ("phone", None)]));
    }

    #[test]
    fn request_id_is_uuid_v4() {
        let id = new_request_id();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
    }
}

/// 查詢異動記錄並轉為畫面用的 DTO（§8；欄位名稱用資料字典的欄位說明，CH5）
#[derive(Clone)]
pub struct HistoryReader {
    changes: std::sync::Arc<dyn crate::ports::ChangeLogRepository>,
    dictionary: std::sync::Arc<dyn crate::ports::DictionaryRepository>,
}

impl HistoryReader {
    pub fn new(
        changes: std::sync::Arc<dyn crate::ports::ChangeLogRepository>,
        dictionary: std::sync::Arc<dyn crate::ports::DictionaryRepository>,
    ) -> Self {
        Self {
            changes,
            dictionary,
        }
    }

    pub async fn read(
        &self,
        plant: Option<&ttrs_domain::plant::PlantId>,
        table: &str,
        key: &[(&str, &str)],
        lang: Option<&str>,
    ) -> Result<Vec<ttrs_shared::ChangeDto>, crate::AppError> {
        let infra = |e: crate::error::RepoError| crate::AppError::Infra(e.to_string());
        let lang = match lang {
            Some(l) => ttrs_domain::lang::Lang::parse(l)?,
            None => ttrs_domain::lang::Lang::default_lang(),
        };
        let key: BTreeMap<String, String> = key
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let records = self
            .changes
            .history(plant, table, &key)
            .await
            .map_err(infra)?;
        let texts = self.dictionary.column_texts(table).await.map_err(infra)?;
        let label = |col: &str| {
            let pick = |l: &str| {
                texts
                    .iter()
                    .find(|t| t.column_name == col && t.lang.as_str() == l)
                    .and_then(|t| t.name.clone())
                    .filter(|n| !n.trim().is_empty())
            };
            pick(lang.as_str())
                .or_else(|| pick(ttrs_domain::lang::DEFAULT_LANG))
                .unwrap_or_else(|| col.to_owned())
        };
        Ok(records
            .into_iter()
            .map(|r| {
                let cols: Vec<String> = match r.operation.as_str() {
                    "insert" | "delete" => r
                        .after
                        .as_ref()
                        .or(r.before.as_ref())
                        .map(|row| {
                            row.keys()
                                .filter(|k| !META_COLUMNS.contains(&k.as_str()))
                                .cloned()
                                .collect()
                        })
                        .unwrap_or_default(),
                    _ => r.changed_fields.clone(),
                };
                let value = |row: &Option<Row>, c: &str| {
                    row.as_ref().and_then(|x| x.get(c).cloned()).flatten()
                };
                ttrs_shared::ChangeDto {
                    change_id: r.change_id,
                    occurred_at: r.occurred_at,
                    table_name: r.table_name.clone(),
                    operation: r.operation.clone(),
                    version: r.version,
                    user_id: r.user_id.clone(),
                    user_dept: r.user_dept.clone(),
                    actor_kind: r.actor_kind.clone(),
                    on_behalf_of: r.on_behalf_of.clone(),
                    program: r.program.clone(),
                    action: r.action.clone(),
                    request_id: r.request_id.clone(),
                    client_ip: r.client_ip.clone(),
                    fields: cols
                        .iter()
                        .map(|c| ttrs_shared::ChangeFieldDto {
                            column: c.clone(),
                            label: label(c),
                            before: value(&r.before, c),
                            after: value(&r.after, c),
                        })
                        .collect(),
                }
            })
            .collect())
    }
}
