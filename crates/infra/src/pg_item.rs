//! 料件（item ← TipTop ima_file；item_maintenance.feature）。工廠 schema（ADR 0007）；欄位由第一階段 71 欄產生
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::ItemRepository, query};
use ttrs_domain::{
    authorization::DataScope,
    item::{ActiveCode, Item, ItemAttrs, ItemNo, ItemSystem, ItemText, StatusCode},
    lang::Lang,
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
    item_no: String,
    name: Option<String>,
    spec: Option<String>,
    group_code: Option<String>,
    source_code: String,
    configurable_item_no: Option<String>,
    current_version: Option<String>,
    catalog_ref: Option<String>,
    supplier_control: String,
    product_class: Option<String>,
    drawing_no: Option<String>,
    apply_dept: Option<String>,
    software: bool,
    engineering: bool,
    insert_position: bool,
    insert_qpa_check: bool,
    material_category: Option<String>,
    coproduct: bool,
    inspection: bool,
    repetitive: bool,
    pcn: Option<String>,
    abc_code: String,
    consumable: bool,
    replenish_policy: Option<String>,
    economic_order_qty: Decimal,
    average_order_qty: Decimal,
    safety_stock: Option<Decimal>,
    safety_period: Option<Decimal>,
    max_stock_qty: Option<Decimal>,
    shelf_life_days: Option<i32>,
    stock_unit: String,
    warehouse: Option<String>,
    location: Option<String>,
    stock_keeper: Option<String>,
    second_unit: Option<String>,
    pricing_unit: Option<String>,
    other_group1: Option<String>,
    other_group2: Option<String>,
    other_group3: Option<String>,
    other_group4: Option<String>,
    account: Option<String>,
    account2: Option<String>,
    bonded: bool,
    full_name: Option<String>,
    english_name: Option<String>,
    planned_discontinue_on: Option<NaiveDate>,
    life_cycle: Option<String>,
    carton_barcode: Option<String>,
    active_code: String,
    status_code: String,
    order_invalid: bool,
    phase_out: bool,
    unit_usage: String,
    low_level_code: i32,
    bom_changed: Option<String>,
    coproduct_this_month: bool,
    new_part_status: String,
    data_source: String,
    item_created_on: NaiveDate,
    first_shipped_on: Option<NaiveDate>,
    first_received_on: Option<NaiveDate>,
    slow_moving_on: Option<NaiveDate>,
    last_period_purchase_on: Option<NaiveDate>,
    last_received_on: Option<NaiveDate>,
    last_issued_on: Option<NaiveDate>,
    last_txn_on: Option<NaiveDate>,
    last_count_on: Option<NaiveDate>,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

macro_rules! select_item {
    ($rest:literal) => {
        concat!("SELECT item_no, name, spec, group_code, source_code, configurable_item_no, current_version, catalog_ref, supplier_control, product_class, drawing_no, apply_dept, software, engineering, insert_position, insert_qpa_check, material_category, coproduct, inspection, repetitive, pcn, abc_code, consumable, replenish_policy, economic_order_qty, average_order_qty, safety_stock, safety_period, max_stock_qty, shelf_life_days, stock_unit, warehouse, location, stock_keeper, second_unit, pricing_unit, other_group1, other_group2, other_group3, other_group4, account, account2, bonded, full_name, english_name, planned_discontinue_on, life_cycle, carton_barcode, active_code, status_code, order_invalid, phase_out, unit_usage, low_level_code, bom_changed, coproduct_this_month, new_part_status, data_source, item_created_on, first_shipped_on, first_received_on, slow_moving_on, last_period_purchase_on, last_received_on, last_issued_on, last_txn_on, last_count_on, created_by, created_dept, created_on, updated_by, updated_on, version, created_at, updated_at FROM item ", $rest)
    };
}

impl TryFrom<Row> for Item {
    type Error = RepoError;

    fn try_from(r: Row) -> Result<Self, RepoError> {
        let bad = |what: &str| RepoError::Other(format!("料件資料不正確：{} {what}", r.item_no));
        Ok(Item {
            no: ItemNo::parse(&r.item_no).map_err(|_| bad("料號"))?,
            active_code: ActiveCode::parse(&r.active_code).ok_or_else(|| bad("資料有效碼"))?,
            status_code: StatusCode::parse(&r.status_code).ok_or_else(|| bad("狀態碼"))?,
            attrs: ItemAttrs {
                name: r.name,
                spec: r.spec,
                group_code: r.group_code,
                configurable_item_no: r.configurable_item_no,
                current_version: r.current_version,
                catalog_ref: r.catalog_ref,
                product_class: r.product_class,
                drawing_no: r.drawing_no,
                apply_dept: r.apply_dept,
                software: r.software,
                engineering: r.engineering,
                insert_position: r.insert_position,
                insert_qpa_check: r.insert_qpa_check,
                material_category: r.material_category,
                coproduct: r.coproduct,
                inspection: r.inspection,
                repetitive: r.repetitive,
                pcn: r.pcn,
                consumable: r.consumable,
                replenish_policy: r.replenish_policy,
                safety_stock: r.safety_stock,
                safety_period: r.safety_period,
                max_stock_qty: r.max_stock_qty,
                shelf_life_days: r.shelf_life_days,
                warehouse: r.warehouse,
                location: r.location,
                stock_keeper: r.stock_keeper,
                second_unit: r.second_unit,
                pricing_unit: r.pricing_unit,
                account: r.account,
                account2: r.account2,
                bonded: r.bonded,
                full_name: r.full_name,
                english_name: r.english_name,
                planned_discontinue_on: r.planned_discontinue_on,
                life_cycle: r.life_cycle,
                carton_barcode: r.carton_barcode,
                source_code: Some(r.source_code),
                supplier_control: Some(r.supplier_control),
                abc_code: Some(r.abc_code),
                economic_order_qty: Some(r.economic_order_qty),
                average_order_qty: Some(r.average_order_qty),
                stock_unit: Some(r.stock_unit),
                other_groups: [
                    r.other_group1,
                    r.other_group2,
                    r.other_group3,
                    r.other_group4,
                ],
            },
            system: ItemSystem {
                phase_out: r.phase_out,
                unit_usage: r.unit_usage,
                low_level_code: r.low_level_code,
                bom_changed: r.bom_changed,
                coproduct_this_month: r.coproduct_this_month,
                new_part_status: r.new_part_status,
                data_source: r.data_source,
                item_created_on: r.item_created_on,
                first_shipped_on: r.first_shipped_on,
                first_received_on: r.first_received_on,
                slow_moving_on: r.slow_moving_on,
                last_period_purchase_on: r.last_period_purchase_on,
                last_received_on: r.last_received_on,
                last_issued_on: r.last_issued_on,
                last_txn_on: r.last_txn_on,
                last_count_on: r.last_count_on,
            },
            order_invalid: r.order_invalid,
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
impl ItemRepository for PgStore {
    async fn list_items(&self, plant: &PlantId) -> Result<Vec<Item>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<Row> = sqlx::query_as(select_item!("ORDER BY item_no COLLATE \"C\""))
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        rows.into_iter().map(Item::try_from).collect()
    }

    async fn get_item(&self, plant: &PlantId, no: &ItemNo) -> Result<Option<Item>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let row: Option<Row> = sqlx::query_as(select_item!("WHERE item_no = $1"))
            .bind(no.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        row.map(Item::try_from).transpose()
    }

    async fn query_items(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        scope: &DataScope,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Item>, u64), RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok((Vec::new(), 0));
        };
        let (rows, total) = query_page_in::<Row>(
            &mut tx,
            "SELECT COUNT(*) FROM item i WHERE TRUE",
            select_item!("i WHERE TRUE"),
            |qb| {
                push_scope(qb, scope, "i.created_by", "i.created_dept");
                push_filters(qb, filters, item_column)
            },
            r#" ORDER BY i.item_no COLLATE "C""#,
            offset,
            limit,
        )
        .await?;
        tx.commit().await.map_err(db_err)?;
        let items = rows
            .into_iter()
            .map(Item::try_from)
            .collect::<Result<_, _>>()?;
        Ok((items, total))
    }

    async fn insert_item(&self, plant: &PlantId, i: &Item) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        item_insert(&mut tx, i).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_item(&self, plant: &PlantId, i: &Item) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        item_update(&mut tx, i).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete_item(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        // 多語言品名以外鍵 ON DELETE CASCADE 一併刪除；有庫存異動時外鍵擋下（InUse）
        let n = sqlx::query("DELETE FROM item WHERE item_no = $1")
            .bind(no.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }

    async fn has_transactions(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(false);
        };
        let found =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM inventory_txn WHERE item_no = $1)")
                .bind(no.as_str())
                .fetch_one(&mut *tx)
                .await
                .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(found)
    }

    async fn item_texts(&self, plant: &PlantId, no: &ItemNo) -> Result<Vec<ItemText>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT lang, name, spec FROM item_text WHERE item_no = $1 ORDER BY lang",
        )
        .bind(no.as_str())
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        rows.into_iter()
            .map(|(lang, name, spec)| {
                Ok(ItemText {
                    lang: Lang::parse(&lang)
                        .map_err(|_| RepoError::Other(format!("料件語系不正確：{lang}")))?,
                    name,
                    spec,
                })
            })
            .collect()
    }

    async fn set_item_text(
        &self,
        plant: &PlantId,
        no: &ItemNo,
        t: &ItemText,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        if t.name.is_none() && t.spec.is_none() {
            sqlx::query("DELETE FROM item_text WHERE item_no = $1 AND lang = $2")
                .bind(no.as_str())
                .bind(t.lang.as_str())
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        } else {
            sqlx::query(
                "INSERT INTO item_text (item_no, lang, name, spec) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (item_no, lang) DO UPDATE SET name = EXCLUDED.name, spec = EXCLUDED.spec",
            )
            .bind(no.as_str())
            .bind(t.lang.as_str())
            .bind(&t.name)
            .bind(&t.spec)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }
}

/// 查詢鍵值 → 欄位：白名單（ttrs_app::query::item::FIELDS）的鍵值即欄位名稱；品名、規格比對各語系（IQ3）
fn item_column(key: &str) -> Option<Column> {
    const TEXT: &str = "EXISTS (SELECT 1 FROM item_text t WHERE t.item_no = i.item_no AND ";
    Some(match key {
        "name" => Column::Translated {
            base: "i.name",
            exists: TEXT,
            text: "t.name",
        },
        "spec" => Column::Translated {
            base: "i.spec",
            exists: TEXT,
            text: "t.spec",
        },
        // 資料有效＝資料有效碼不是 N（IM27）
        "active" => Column::Scalar("(i.active_code <> 'N')", false),
        _ => {
            let f = query::item::FIELDS.iter().find(|f| f.key == key)?;
            Column::Scalar(f.key, f.ty == FieldType::Text)
        }
    })
}

/// 在呼叫端的交易內寫入（料件拋轉一個營運中心一個交易，item_carry.feature IC7）
pub(crate) async fn item_insert(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    i: &Item,
) -> Result<(), RepoError> {
    let a = &i.attrs;
    let s = &i.system;
    sqlx::query(
            "INSERT INTO item (item_no, name, spec, group_code, source_code, configurable_item_no, current_version, catalog_ref, supplier_control, product_class, drawing_no, apply_dept, software, engineering, insert_position, insert_qpa_check, material_category, coproduct, inspection, repetitive, pcn, abc_code, consumable, replenish_policy, economic_order_qty, average_order_qty, safety_stock, safety_period, max_stock_qty, shelf_life_days, stock_unit, warehouse, location, stock_keeper, second_unit, pricing_unit, other_group1, other_group2, other_group3, other_group4, account, account2, bonded, full_name, english_name, planned_discontinue_on, life_cycle, carton_barcode, active_code, status_code, order_invalid, phase_out, unit_usage, low_level_code, bom_changed, coproduct_this_month, new_part_status, data_source, item_created_on, first_shipped_on, first_received_on, slow_moving_on, last_period_purchase_on, last_received_on, last_issued_on, last_txn_on, last_count_on, created_by, created_dept, created_on) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38, $39, $40, $41, $42, $43, $44, $45, $46, $47, $48, $49, $50, $51, $52, $53, $54, $55, $56, $57, $58, $59, $60, $61, $62, $63, $64, $65, $66, $67, $68, $69, $70)",
        )
        .bind(i.no.as_str())
        .bind(&a.name)
        .bind(&a.spec)
        .bind(&a.group_code)
        .bind(a.source_code.as_deref().unwrap_or("P"))
        .bind(&a.configurable_item_no)
        .bind(&a.current_version)
        .bind(&a.catalog_ref)
        .bind(a.supplier_control.as_deref().unwrap_or("0"))
        .bind(&a.product_class)
        .bind(&a.drawing_no)
        .bind(&a.apply_dept)
        .bind(a.software)
        .bind(a.engineering)
        .bind(a.insert_position)
        .bind(a.insert_qpa_check)
        .bind(&a.material_category)
        .bind(a.coproduct)
        .bind(a.inspection)
        .bind(a.repetitive)
        .bind(&a.pcn)
        .bind(a.abc_code.as_deref().unwrap_or("A"))
        .bind(a.consumable)
        .bind(&a.replenish_policy)
        .bind(a.economic_order_qty.unwrap_or(Decimal::ONE))
        .bind(a.average_order_qty.unwrap_or(Decimal::ONE))
        .bind(a.safety_stock)
        .bind(a.safety_period)
        .bind(a.max_stock_qty)
        .bind(a.shelf_life_days)
        .bind(a.stock_unit.as_deref().unwrap_or_default())
        .bind(&a.warehouse)
        .bind(&a.location)
        .bind(&a.stock_keeper)
        .bind(&a.second_unit)
        .bind(&a.pricing_unit)
        .bind(&a.other_groups[0])
        .bind(&a.other_groups[1])
        .bind(&a.other_groups[2])
        .bind(&a.other_groups[3])
        .bind(&a.account)
        .bind(&a.account2)
        .bind(a.bonded)
        .bind(&a.full_name)
        .bind(&a.english_name)
        .bind(a.planned_discontinue_on)
        .bind(&a.life_cycle)
        .bind(&a.carton_barcode)
        .bind(i.active_code.code())
        .bind(i.status_code.code())
        .bind(i.order_invalid)
        .bind(s.phase_out)
        .bind(&s.unit_usage)
        .bind(s.low_level_code)
        .bind(&s.bom_changed)
        .bind(s.coproduct_this_month)
        .bind(&s.new_part_status)
        .bind(&s.data_source)
        .bind(s.item_created_on)
        .bind(s.first_shipped_on)
        .bind(s.first_received_on)
        .bind(s.slow_moving_on)
        .bind(s.last_period_purchase_on)
        .bind(s.last_received_on)
        .bind(s.last_issued_on)
        .bind(s.last_txn_on)
        .bind(s.last_count_on)
        .bind(&i.created_by)
        .bind(&i.created_dept)
        .bind(i.created_on)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
    Ok(())
}

/// 在呼叫端的交易內寫入（料件拋轉一個營運中心一個交易，item_carry.feature IC7）
pub(crate) async fn item_update(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    i: &Item,
) -> Result<(), RepoError> {
    let a = &i.attrs;
    let s = &i.system;
    let n = sqlx::query(
            "UPDATE item SET name = $2, spec = $3, group_code = $4, source_code = $5, configurable_item_no = $6, current_version = $7, catalog_ref = $8, supplier_control = $9, product_class = $10, drawing_no = $11, apply_dept = $12, software = $13, engineering = $14, insert_position = $15, insert_qpa_check = $16, material_category = $17, coproduct = $18, inspection = $19, repetitive = $20, pcn = $21, abc_code = $22, consumable = $23, replenish_policy = $24, economic_order_qty = $25, average_order_qty = $26, safety_stock = $27, safety_period = $28, max_stock_qty = $29, shelf_life_days = $30, stock_unit = $31, warehouse = $32, location = $33, stock_keeper = $34, second_unit = $35, pricing_unit = $36, other_group1 = $37, other_group2 = $38, other_group3 = $39, other_group4 = $40, account = $41, account2 = $42, bonded = $43, full_name = $44, english_name = $45, planned_discontinue_on = $46, life_cycle = $47, carton_barcode = $48, active_code = $49, status_code = $50, order_invalid = $51, phase_out = $52, unit_usage = $53, low_level_code = $54, bom_changed = $55, coproduct_this_month = $56, new_part_status = $57, data_source = $58, item_created_on = $59, first_shipped_on = $60, first_received_on = $61, slow_moving_on = $62, last_period_purchase_on = $63, last_received_on = $64, last_issued_on = $65, last_txn_on = $66, last_count_on = $67, updated_by = $68, updated_on = $69
             WHERE item_no = $1 AND version = $70",
        )
        .bind(i.no.as_str())
        .bind(&a.name)
        .bind(&a.spec)
        .bind(&a.group_code)
        .bind(a.source_code.as_deref().unwrap_or("P"))
        .bind(&a.configurable_item_no)
        .bind(&a.current_version)
        .bind(&a.catalog_ref)
        .bind(a.supplier_control.as_deref().unwrap_or("0"))
        .bind(&a.product_class)
        .bind(&a.drawing_no)
        .bind(&a.apply_dept)
        .bind(a.software)
        .bind(a.engineering)
        .bind(a.insert_position)
        .bind(a.insert_qpa_check)
        .bind(&a.material_category)
        .bind(a.coproduct)
        .bind(a.inspection)
        .bind(a.repetitive)
        .bind(&a.pcn)
        .bind(a.abc_code.as_deref().unwrap_or("A"))
        .bind(a.consumable)
        .bind(&a.replenish_policy)
        .bind(a.economic_order_qty.unwrap_or(Decimal::ONE))
        .bind(a.average_order_qty.unwrap_or(Decimal::ONE))
        .bind(a.safety_stock)
        .bind(a.safety_period)
        .bind(a.max_stock_qty)
        .bind(a.shelf_life_days)
        .bind(a.stock_unit.as_deref().unwrap_or_default())
        .bind(&a.warehouse)
        .bind(&a.location)
        .bind(&a.stock_keeper)
        .bind(&a.second_unit)
        .bind(&a.pricing_unit)
        .bind(&a.other_groups[0])
        .bind(&a.other_groups[1])
        .bind(&a.other_groups[2])
        .bind(&a.other_groups[3])
        .bind(&a.account)
        .bind(&a.account2)
        .bind(a.bonded)
        .bind(&a.full_name)
        .bind(&a.english_name)
        .bind(a.planned_discontinue_on)
        .bind(&a.life_cycle)
        .bind(&a.carton_barcode)
        .bind(i.active_code.code())
        .bind(i.status_code.code())
        .bind(i.order_invalid)
        .bind(s.phase_out)
        .bind(&s.unit_usage)
        .bind(s.low_level_code)
        .bind(&s.bom_changed)
        .bind(s.coproduct_this_month)
        .bind(&s.new_part_status)
        .bind(&s.data_source)
        .bind(s.item_created_on)
        .bind(s.first_shipped_on)
        .bind(s.first_received_on)
        .bind(s.slow_moving_on)
        .bind(s.last_period_purchase_on)
        .bind(s.last_received_on)
        .bind(s.last_issued_on)
        .bind(s.last_txn_on)
        .bind(s.last_count_on)
        .bind(&i.updated_by)
        .bind(i.updated_on)
        .bind(i.stamp.version)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?
        .rows_affected();
    // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
    if n == 0 {
        return Err(RepoError::Stale);
    }
    Ok(())
}
