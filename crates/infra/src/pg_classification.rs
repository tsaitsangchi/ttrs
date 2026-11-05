//! 分類碼維護（aooi300 / azf_file）的 PostgreSQL 實作。
//! 表在營運中心的工廠 schema（migrations/plant/0003_classification_code.sql），每個方法在工廠交易內執行。

use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::FromRow;
use ttrs_app::{
    error::RepoError,
    ports::{ClassificationCode, ClassificationCodeRepository, Code, CodeKind},
};
use ttrs_domain::{classification::CodeAttrs, plant::PlantId};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct CodeRow {
    kind: String,
    code: String,
    description: Option<String>,
    variance_account: Option<String>,
    variance_account2: Option<String>,
    cost_nature: Option<i32>,
    active: bool,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
}

macro_rules! select_code {
    ($rest:literal) => {
        concat!(
            "SELECT kind, code, description, variance_account, variance_account2, cost_nature, ",
            "active, created_by, created_dept, created_on, updated_by, updated_on FROM classification_code ",
            $rest
        )
    };
}

fn bad(what: &str) -> RepoError {
    RepoError::Other(format!("資料庫內容不合法：{what}"))
}

impl TryFrom<CodeRow> for ClassificationCode {
    type Error = RepoError;

    fn try_from(r: CodeRow) -> Result<Self, RepoError> {
        Ok(ClassificationCode {
            kind: CodeKind::from_code(r.kind.trim()).ok_or_else(|| bad(&r.kind))?,
            code: Code::parse(&r.code).map_err(|_| bad(&r.code))?,
            attrs: CodeAttrs {
                description: r.description,
                variance_account: r.variance_account,
                variance_account2: r.variance_account2,
                cost_nature: r.cost_nature,
            },
            active: r.active,
            created_by: r.created_by,
            created_dept: r.created_dept,
            created_on: r.created_on,
            updated_by: r.updated_by,
            updated_on: r.updated_on,
        })
    }
}

#[async_trait]
impl ClassificationCodeRepository for PgStore {
    async fn list_codes(
        &self,
        plant: &PlantId,
        kind: CodeKind,
    ) -> Result<Vec<ClassificationCode>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        sqlx::query_as::<_, CodeRow>(select_code!("WHERE kind = $1 ORDER BY code COLLATE \"C\""))
            .bind(kind.code())
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(ClassificationCode::try_from)
            .collect()
    }

    async fn get_code(
        &self,
        plant: &PlantId,
        kind: CodeKind,
        code: &Code,
    ) -> Result<Option<ClassificationCode>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        sqlx::query_as::<_, CodeRow>(select_code!("WHERE kind = $1 AND code = $2"))
            .bind(kind.code())
            .bind(code.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?
            .map(ClassificationCode::try_from)
            .transpose()
    }

    async fn insert_code(&self, plant: &PlantId, c: &ClassificationCode) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        sqlx::query(
            "INSERT INTO classification_code (kind, code, description, variance_account, variance_account2,
                 cost_nature, active, created_by, created_dept, created_on, updated_by, updated_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(c.kind.code())
        .bind(c.code.as_str())
        .bind(&c.attrs.description)
        .bind(&c.attrs.variance_account)
        .bind(&c.attrs.variance_account2)
        .bind(c.attrs.cost_nature)
        .bind(c.active)
        .bind(&c.created_by)
        .bind(&c.created_dept)
        .bind(c.created_on)
        .bind(&c.updated_by)
        .bind(c.updated_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_code(&self, plant: &PlantId, c: &ClassificationCode) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        sqlx::query(
            "UPDATE classification_code SET description = $3, variance_account = $4, variance_account2 = $5,
                 cost_nature = $6, active = $7, updated_by = $8, updated_on = $9
             WHERE kind = $1 AND code = $2",
        )
        .bind(c.kind.code())
        .bind(c.code.as_str())
        .bind(&c.attrs.description)
        .bind(&c.attrs.variance_account)
        .bind(&c.attrs.variance_account2)
        .bind(c.attrs.cost_nature)
        .bind(c.active)
        .bind(&c.updated_by)
        .bind(c.updated_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    /// 只刪該類別的代號（CC4，修正 TipTop aooi300.4gl:536 未限定類別）
    async fn delete_code(
        &self,
        plant: &PlantId,
        kind: CodeKind,
        code: &Code,
    ) -> Result<bool, RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let n = sqlx::query("DELETE FROM classification_code WHERE kind = $1 AND code = $2")
            .bind(kind.code())
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}
