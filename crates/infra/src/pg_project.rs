//! 專案（project ← TipTop gja_file；project_maintenance.feature）。工廠 schema（PJ2）
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::ProjectRepository};
use ttrs_domain::{
    confirmation::Confirmation,
    plant::PlantId,
    project::{Project, ProjectAttrs, ProjectCode},
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct Row {
    project_code: String,
    name: Option<String>,
    start_on: Option<NaiveDate>,
    end_on: Option<NaiveDate>,
    active: bool,
    confirmed: bool,
    confirmed_by: Option<String>,
    confirmed_at: Option<DateTime<Utc>>,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

macro_rules! select_project {
    ($rest:literal) => {
        concat!(
            "SELECT project_code, name, start_on, end_on, active, confirmed, confirmed_by, confirmed_at, ",
            "created_by, created_dept, created_on, updated_by, updated_on, version, created_at, updated_at ",
            "FROM project ",
            $rest
        )
    };
}

impl TryFrom<Row> for Project {
    type Error = RepoError;

    fn try_from(r: Row) -> Result<Self, RepoError> {
        Ok(Project {
            code: ProjectCode::parse(&r.project_code)
                .map_err(|_| RepoError::Other(format!("專案資料不正確：{}", r.project_code)))?,
            attrs: ProjectAttrs {
                name: r.name,
                start_on: r.start_on,
                end_on: r.end_on,
            },
            active: r.active,
            confirmation: Confirmation {
                confirmed: r.confirmed,
                by: r.confirmed_by,
                at: r.confirmed_at,
            },
            created_by: r.created_by,
            created_dept: r.created_dept,
            created_on: r.created_on,
            updated_by: r.updated_by,
            updated_on: r.updated_on,
            stamp: Stamp {
                version: r.version,
                created_at: Some(r.created_at),
                updated_at: r.updated_at,
            },
        })
    }
}

#[async_trait]
impl ProjectRepository for PgStore {
    async fn list_projects(&self, plant: &PlantId) -> Result<Vec<Project>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<Row> = sqlx::query_as(select_project!("ORDER BY project_code COLLATE \"C\""))
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        rows.into_iter().map(Project::try_from).collect()
    }

    async fn get_project(
        &self,
        plant: &PlantId,
        code: &ProjectCode,
    ) -> Result<Option<Project>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let row: Option<Row> = sqlx::query_as(select_project!("WHERE project_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        row.map(Project::try_from).transpose()
    }

    async fn insert_project(&self, plant: &PlantId, x: &Project) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &x.attrs;
        sqlx::query(
            "INSERT INTO project (project_code, name, start_on, end_on, active, created_by, created_dept,
                 created_on, confirmed, confirmed_by, confirmed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(x.code.as_str())
        .bind(&a.name)
        .bind(a.start_on)
        .bind(a.end_on)
        .bind(x.active)
        .bind(&x.created_by)
        .bind(&x.created_dept)
        .bind(x.created_on)
        .bind(x.confirmation.confirmed)
        .bind(&x.confirmation.by)
        .bind(x.confirmation.at)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_project(&self, plant: &PlantId, x: &Project) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &x.attrs;
        let n = sqlx::query(
            "UPDATE project SET name = $2, start_on = $3, end_on = $4, active = $5, updated_by = $6,
                 updated_on = $7, confirmed = $9, confirmed_by = $10, confirmed_at = $11
             WHERE project_code = $1 AND version = $8",
        )
        .bind(x.code.as_str())
        .bind(&a.name)
        .bind(a.start_on)
        .bind(a.end_on)
        .bind(x.active)
        .bind(&x.updated_by)
        .bind(x.updated_on)
        .bind(x.stamp.version)
        .bind(x.confirmation.confirmed)
        .bind(&x.confirmation.by)
        .bind(x.confirmation.at)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        tx.commit().await.map_err(db_err)
    }

    async fn delete_project(&self, plant: &PlantId, code: &ProjectCode) -> Result<bool, RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let n = sqlx::query("DELETE FROM project WHERE project_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}
