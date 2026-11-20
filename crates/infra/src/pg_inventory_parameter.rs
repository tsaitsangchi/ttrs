//! 庫存參數（manufacturing_parameter ← sma_file；inventory_parameters.feature）。工廠層、每廠一筆
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::InventoryParameterRepository};
use ttrs_domain::{
    inventory_parameter::{
        InventoryParameters, InventorySettings, PostingDateBasis, StorageControl,
    },
    plant::PlantId,
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct Row {
    storage_control: String,
    fast_turnover: Decimal,
    average_turnover: Decimal,
    posting_date_basis: String,
    misc_receipt_new_location: bool,
    stock_query_window: bool,
    plant_transfer_same_no: bool,
    warehouse_transfer_same_no: bool,
    misc_issue_dept: bool,
    misc_receipt_dept: bool,
    add_storage_freely: bool,
    current_year: Option<i32>,
    current_period: Option<i16>,
    closing_date: Option<NaiveDate>,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

fn to_domain(r: Row) -> Result<InventoryParameters, RepoError> {
    let bad = |e: ttrs_domain::DomainError| RepoError::Other(format!("庫存參數資料不正確：{e}"));
    Ok(InventoryParameters {
        settings: InventorySettings {
            storage_control: StorageControl::parse(&r.storage_control).map_err(bad)?,
            fast_turnover: r.fast_turnover,
            average_turnover: r.average_turnover,
            posting_date_basis: PostingDateBasis::parse(&r.posting_date_basis).map_err(bad)?,
            misc_receipt_new_location: r.misc_receipt_new_location,
            stock_query_window: r.stock_query_window,
            plant_transfer_same_no: r.plant_transfer_same_no,
            warehouse_transfer_same_no: r.warehouse_transfer_same_no,
            misc_issue_dept: r.misc_issue_dept,
            misc_receipt_dept: r.misc_receipt_dept,
            add_storage_freely: r.add_storage_freely,
            current_year: r.current_year,
            current_period: r.current_period,
            closing_date: r.closing_date,
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

#[async_trait]
impl InventoryParameterRepository for PgStore {
    async fn inventory_parameters(
        &self,
        plant: &PlantId,
    ) -> Result<Option<InventoryParameters>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let row: Option<Row> = sqlx::query_as(
            "SELECT storage_control, fast_turnover, average_turnover, posting_date_basis, misc_receipt_new_location,
                    stock_query_window, plant_transfer_same_no, warehouse_transfer_same_no, misc_issue_dept,
                    misc_receipt_dept, add_storage_freely, current_year, current_period, closing_date, created_by,
                    created_dept, created_on, updated_by, updated_on, version, created_at, updated_at
               FROM manufacturing_parameter WHERE parameter_key = '0'",
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        row.map(to_domain).transpose()
    }

    async fn insert_inventory_parameters(
        &self,
        plant: &PlantId,
        p: &InventoryParameters,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let v = &p.settings;
        sqlx::query(
            "INSERT INTO manufacturing_parameter (storage_control, fast_turnover, average_turnover, posting_date_basis,
                    misc_receipt_new_location, stock_query_window, plant_transfer_same_no, warehouse_transfer_same_no,
                    misc_issue_dept, misc_receipt_dept, add_storage_freely, created_by, created_dept, created_on,
                    current_year, current_period, closing_date)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
        )
        .bind(v.storage_control.code())
        .bind(v.fast_turnover)
        .bind(v.average_turnover)
        .bind(v.posting_date_basis.code())
        .bind(v.misc_receipt_new_location)
        .bind(v.stock_query_window)
        .bind(v.plant_transfer_same_no)
        .bind(v.warehouse_transfer_same_no)
        .bind(v.misc_issue_dept)
        .bind(v.misc_receipt_dept)
        .bind(v.add_storage_freely)
        .bind(&p.created_by)
        .bind(&p.created_dept)
        .bind(p.created_on)
        .bind(v.current_year)
        .bind(v.current_period)
        .bind(v.closing_date)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_inventory_parameters(
        &self,
        plant: &PlantId,
        p: &InventoryParameters,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let v = &p.settings;
        let n = sqlx::query(
            "UPDATE manufacturing_parameter SET storage_control = $1, fast_turnover = $2, average_turnover = $3,
                    posting_date_basis = $4, misc_receipt_new_location = $5, stock_query_window = $6,
                    plant_transfer_same_no = $7, warehouse_transfer_same_no = $8, misc_issue_dept = $9,
                    misc_receipt_dept = $10, add_storage_freely = $11, updated_by = $12, updated_on = $13,
                    current_year = $15, current_period = $16, closing_date = $17
              WHERE parameter_key = '0' AND version = $14",
        )
        .bind(v.storage_control.code())
        .bind(v.fast_turnover)
        .bind(v.average_turnover)
        .bind(v.posting_date_basis.code())
        .bind(v.misc_receipt_new_location)
        .bind(v.stock_query_window)
        .bind(v.plant_transfer_same_no)
        .bind(v.warehouse_transfer_same_no)
        .bind(v.misc_issue_dept)
        .bind(v.misc_receipt_dept)
        .bind(v.add_storage_freely)
        .bind(&p.updated_by)
        .bind(p.updated_on)
        .bind(p.stamp.version)
        .bind(v.current_year)
        .bind(v.current_period)
        .bind(v.closing_date)
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
}
