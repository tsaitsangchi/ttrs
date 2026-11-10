//! 料件分群（item_group ← TipTop imz_file；item_group.feature）。工廠 schema；欄位由 imz 第一階段 41 欄產生
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::ItemGroupRepository, query};
use ttrs_domain::{
    authorization::DataScope,
    item_group::{ItemGroup, ItemGroupAttrs, ItemGroupCode},
    plant::PlantId,
    qbe::{FieldType, QbeFilter},
    record::Stamp,
};

use crate::{
    PgStore,
    pg::db_err,
    qbe_sql::{Column, push_filters, push_scope, query_page_in},
};

#[derive(FromRow)]
struct Row {
    group_code: String,
    description: Option<String>,
    source_code: Option<String>,
    stock_unit: Option<String>,
    catalog_ref: Option<String>,
    supplier_control: String,
    drawing_no: Option<String>,
    software: bool,
    engineering: bool,
    insert_qpa_check: bool,
    insert_position: bool,
    material_category: Option<String>,
    coproduct: bool,
    inspection: bool,
    repetitive: bool,
    abc_code: Option<String>,
    consumable: bool,
    replenish_policy: Option<String>,
    reorder_point: Option<Decimal>,
    reorder_qty: Option<Decimal>,
    safety_stock: Option<Decimal>,
    safety_period: Option<Decimal>,
    shelf_life_days: Option<i32>,
    mrp_bucket_days: Option<i32>,
    warehouse: Option<String>,
    location: Option<String>,
    stock_keeper: Option<String>,
    sales_stat_unit: Option<String>,
    second_unit: Option<String>,
    pricing_unit: Option<String>,
    other_group1: Option<String>,
    other_group2: Option<String>,
    other_group3: Option<String>,
    other_group4: Option<String>,
    account: Option<String>,
    account2: Option<String>,
    bonded: bool,
    bonded_class: Option<String>,
    tariff_no: Option<String>,
    bonded_type: Option<String>,
    product_class: Option<String>,
    economic_order_qty: Option<Decimal>,
    average_order_qty: Option<Decimal>,
    active: bool,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

macro_rules! select_group {
    ($rest:literal) => {
        concat!("SELECT group_code, description, source_code, stock_unit, catalog_ref, supplier_control, drawing_no, software, engineering, insert_qpa_check, insert_position, material_category, coproduct, inspection, repetitive, abc_code, consumable, replenish_policy, reorder_point, reorder_qty, safety_stock, safety_period, shelf_life_days, mrp_bucket_days, warehouse, location, stock_keeper, sales_stat_unit, second_unit, pricing_unit, other_group1, other_group2, other_group3, other_group4, account, account2, bonded, bonded_class, tariff_no, bonded_type, product_class, economic_order_qty, average_order_qty, active, created_by, created_dept, created_on, updated_by, updated_on, version, created_at, updated_at FROM item_group ", $rest)
    };
}

impl TryFrom<Row> for ItemGroup {
    type Error = RepoError;

    fn try_from(r: Row) -> Result<Self, RepoError> {
        Ok(ItemGroup {
            code: ItemGroupCode::parse(&r.group_code)
                .map_err(|_| RepoError::Other(format!("料件分群資料不正確：{}", r.group_code)))?,
            attrs: ItemGroupAttrs {
                description: r.description,
                source_code: r.source_code,
                stock_unit: r.stock_unit,
                catalog_ref: r.catalog_ref,
                supplier_control: r.supplier_control,
                drawing_no: r.drawing_no,
                software: r.software,
                engineering: r.engineering,
                insert_qpa_check: r.insert_qpa_check,
                insert_position: r.insert_position,
                material_category: r.material_category,
                coproduct: r.coproduct,
                inspection: r.inspection,
                repetitive: r.repetitive,
                abc_code: r.abc_code,
                consumable: r.consumable,
                replenish_policy: r.replenish_policy,
                reorder_point: r.reorder_point,
                reorder_qty: r.reorder_qty,
                safety_stock: r.safety_stock,
                safety_period: r.safety_period,
                shelf_life_days: r.shelf_life_days,
                mrp_bucket_days: r.mrp_bucket_days,
                warehouse: r.warehouse,
                location: r.location,
                stock_keeper: r.stock_keeper,
                sales_stat_unit: r.sales_stat_unit,
                second_unit: r.second_unit,
                pricing_unit: r.pricing_unit,
                account: r.account,
                account2: r.account2,
                bonded: r.bonded,
                bonded_class: r.bonded_class,
                tariff_no: r.tariff_no,
                bonded_type: r.bonded_type,
                product_class: r.product_class,
                economic_order_qty: r.economic_order_qty,
                average_order_qty: r.average_order_qty,
                other_groups: [
                    r.other_group1,
                    r.other_group2,
                    r.other_group3,
                    r.other_group4,
                ],
            },
            active: r.active,
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
impl ItemGroupRepository for PgStore {
    async fn list_item_groups(&self, plant: &PlantId) -> Result<Vec<ItemGroup>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<Row> = sqlx::query_as(select_group!("ORDER BY group_code COLLATE \"C\""))
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        rows.into_iter().map(ItemGroup::try_from).collect()
    }

    async fn get_item_group(
        &self,
        plant: &PlantId,
        code: &ItemGroupCode,
    ) -> Result<Option<ItemGroup>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let row: Option<Row> = sqlx::query_as(select_group!("WHERE group_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        row.map(ItemGroup::try_from).transpose()
    }

    async fn query_item_groups(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        scope: &DataScope,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<ItemGroup>, u64), RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok((Vec::new(), 0));
        };
        let (rows, total) = query_page_in::<Row>(
            &mut tx,
            "SELECT COUNT(*) FROM item_group g WHERE TRUE",
            select_group!("g WHERE TRUE"),
            |qb| {
                push_scope(qb, scope, "g.created_by", "g.created_dept");
                push_filters(qb, filters, group_column)
            },
            r#" ORDER BY g.group_code COLLATE "C""#,
            offset,
            limit,
        )
        .await?;
        tx.commit().await.map_err(db_err)?;
        let groups = rows
            .into_iter()
            .map(ItemGroup::try_from)
            .collect::<Result<_, _>>()?;
        Ok((groups, total))
    }

    async fn insert_item_group(&self, plant: &PlantId, g: &ItemGroup) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &g.attrs;
        sqlx::query(
            "INSERT INTO item_group (group_code, description, source_code, stock_unit, catalog_ref, supplier_control, drawing_no, software, engineering, insert_qpa_check, insert_position, material_category, coproduct, inspection, repetitive, abc_code, consumable, replenish_policy, reorder_point, reorder_qty, safety_stock, safety_period, shelf_life_days, mrp_bucket_days, warehouse, location, stock_keeper, sales_stat_unit, second_unit, pricing_unit, other_group1, other_group2, other_group3, other_group4, account, account2, bonded, bonded_class, tariff_no, bonded_type, active, created_by, created_dept, created_on, product_class, economic_order_qty, average_order_qty) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38, $39, $40, $41, $42, $43, $44, $45, $46, $47)",
        )
        .bind(g.code.as_str())
        .bind(&a.description)
        .bind(&a.source_code)
        .bind(&a.stock_unit)
        .bind(&a.catalog_ref)
        .bind(&a.supplier_control)
        .bind(&a.drawing_no)
        .bind(a.software)
        .bind(a.engineering)
        .bind(a.insert_qpa_check)
        .bind(a.insert_position)
        .bind(&a.material_category)
        .bind(a.coproduct)
        .bind(a.inspection)
        .bind(a.repetitive)
        .bind(&a.abc_code)
        .bind(a.consumable)
        .bind(&a.replenish_policy)
        .bind(a.reorder_point)
        .bind(a.reorder_qty)
        .bind(a.safety_stock)
        .bind(a.safety_period)
        .bind(a.shelf_life_days)
        .bind(a.mrp_bucket_days)
        .bind(&a.warehouse)
        .bind(&a.location)
        .bind(&a.stock_keeper)
        .bind(&a.sales_stat_unit)
        .bind(&a.second_unit)
        .bind(&a.pricing_unit)
        .bind(&a.other_groups[0])
        .bind(&a.other_groups[1])
        .bind(&a.other_groups[2])
        .bind(&a.other_groups[3])
        .bind(&a.account)
        .bind(&a.account2)
        .bind(a.bonded)
        .bind(&a.bonded_class)
        .bind(&a.tariff_no)
        .bind(&a.bonded_type)
        .bind(g.active)
        .bind(&g.created_by)
        .bind(&g.created_dept)
        .bind(g.created_on)
        .bind(&a.product_class)
        .bind(a.economic_order_qty)
        .bind(a.average_order_qty)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_item_group(&self, plant: &PlantId, g: &ItemGroup) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &g.attrs;
        let n = sqlx::query(
            "UPDATE item_group SET description = $2, source_code = $3, stock_unit = $4, catalog_ref = $5, supplier_control = $6, drawing_no = $7, software = $8, engineering = $9, insert_qpa_check = $10, insert_position = $11, material_category = $12, coproduct = $13, inspection = $14, repetitive = $15, abc_code = $16, consumable = $17, replenish_policy = $18, reorder_point = $19, reorder_qty = $20, safety_stock = $21, safety_period = $22, shelf_life_days = $23, mrp_bucket_days = $24, warehouse = $25, location = $26, stock_keeper = $27, sales_stat_unit = $28, second_unit = $29, pricing_unit = $30, other_group1 = $31, other_group2 = $32, other_group3 = $33, other_group4 = $34, account = $35, account2 = $36, bonded = $37, bonded_class = $38, tariff_no = $39, bonded_type = $40, active = $41, updated_by = $42, updated_on = $43, product_class = $45, economic_order_qty = $46, average_order_qty = $47
             WHERE group_code = $1 AND version = $44",
        )
        .bind(g.code.as_str())
        .bind(&a.description)
        .bind(&a.source_code)
        .bind(&a.stock_unit)
        .bind(&a.catalog_ref)
        .bind(&a.supplier_control)
        .bind(&a.drawing_no)
        .bind(a.software)
        .bind(a.engineering)
        .bind(a.insert_qpa_check)
        .bind(a.insert_position)
        .bind(&a.material_category)
        .bind(a.coproduct)
        .bind(a.inspection)
        .bind(a.repetitive)
        .bind(&a.abc_code)
        .bind(a.consumable)
        .bind(&a.replenish_policy)
        .bind(a.reorder_point)
        .bind(a.reorder_qty)
        .bind(a.safety_stock)
        .bind(a.safety_period)
        .bind(a.shelf_life_days)
        .bind(a.mrp_bucket_days)
        .bind(&a.warehouse)
        .bind(&a.location)
        .bind(&a.stock_keeper)
        .bind(&a.sales_stat_unit)
        .bind(&a.second_unit)
        .bind(&a.pricing_unit)
        .bind(&a.other_groups[0])
        .bind(&a.other_groups[1])
        .bind(&a.other_groups[2])
        .bind(&a.other_groups[3])
        .bind(&a.account)
        .bind(&a.account2)
        .bind(a.bonded)
        .bind(&a.bonded_class)
        .bind(&a.tariff_no)
        .bind(&a.bonded_type)
        .bind(g.active)
        .bind(&g.updated_by)
        .bind(g.updated_on)
        .bind(g.stamp.version)
        .bind(&a.product_class)
        .bind(a.economic_order_qty)
        .bind(a.average_order_qty)
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

    async fn delete_item_group(
        &self,
        plant: &PlantId,
        code: &ItemGroupCode,
    ) -> Result<bool, RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let n = sqlx::query("DELETE FROM item_group WHERE group_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}

/// 查詢鍵值 → 欄位：白名單（ttrs_app::query::item_group::FIELDS）的鍵值即欄位名稱
fn group_column(key: &str) -> Option<Column> {
    let f = query::item_group::FIELDS.iter().find(|f| f.key == key)?;
    Some(Column::Scalar(f.key, f.ty == FieldType::Text))
}
