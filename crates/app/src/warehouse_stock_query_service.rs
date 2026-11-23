//! Warehouse Stock Query 倉庫料件數量查詢（TipTop aimq401 / imd_file、img_file、ima_file）
//! 規格：specs/features/aim/warehouse_stock_query.feature（WK-1～WK-4；確認事項 WK1–WK7）。唯讀查詢，沒有寫入。
//!
//! 倉庫條件與庫存明細條件分開：沒有明細條件時列出符合的所有倉庫（含沒有庫存的倉庫）；
//! 有明細條件時只列出有符合明細的倉庫、只顯示符合的明細（WK2）。資料權限套用在倉庫（imduser、imdgrup）。

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use ttrs_domain::{
    authorization::DataScope, inventory_balance::InventoryBalance, item::Item, plant::PlantId,
};
use ttrs_shared::{Page, QbeCondition, QueryRequest, WarehouseStockDto, WarehouseStockLineDto};

use crate::{
    Actor, AppError, Authorizer,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{InventoryBalanceRepository, ItemRepository, WarehouseRepository},
    query,
    report::{ReportData, ReportFormat},
};

pub const PROGRAM: &str = "warehouse-stock-query";

const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "wk-report-title",
    columns: &[
        ("warehouse", "wk-col-warehouse"),
        ("warehouse_name", "wk-col-warehouse-name"),
        ("item_no", "wk-col-item-no"),
        ("name", "wk-col-name"),
        ("spec", "wk-col-spec"),
        ("location", "wk-col-location"),
        ("lot", "wk-col-lot"),
        ("unit", "wk-col-unit"),
        ("qty", "wk-col-qty"),
        ("expiry", "wk-col-expiry"),
        ("idle_on", "wk-col-idle-on"),
    ],
};

#[derive(Clone)]
pub struct WarehouseStockQueryService {
    authz: Arc<dyn Authorizer>,
    warehouses: Arc<dyn WarehouseRepository>,
    items: Arc<dyn ItemRepository>,
    balances: Arc<dyn InventoryBalanceRepository>,
    reports: ListReporter,
}

impl WarehouseStockQueryService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        warehouses: Arc<dyn WarehouseRepository>,
        items: Arc<dyn ItemRepository>,
        balances: Arc<dyn InventoryBalanceRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            warehouses,
            items,
            balances,
            reports,
        }
    }

    /// QBE 查詢（q401_cs、q401_b_fill；WK2、WK3）：依倉庫排序，以倉庫分頁
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<WarehouseStockDto>, AppError> {
        let all = self.search(actor, plant, &req.conditions, "query").await?;
        let p = query::paging(&req);
        let total = all.len() as u64;
        let items = all
            .into_iter()
            .skip(usize::try_from(p.offset).unwrap_or(usize::MAX))
            .take(p.page_size as usize)
            .collect();
        Ok(Page {
            items,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印 output、匯出 Excel exporttoexcel（WK6）：每筆庫存明細一列，範圍＝目前查詢條件
    pub async fn report_in(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        let all = self
            .search(actor, plant, &conditions, format.action())
            .await?;
        let plant = PlantId::parse(plant)?;
        let date = |d: Option<chrono::NaiveDate>| d.map(|x| x.to_string()).unwrap_or_default();
        let mut rows: Vec<Vec<String>> = Vec::new();
        for s in &all {
            for l in &s.lines {
                rows.push(vec![
                    s.warehouse.clone(),
                    s.name.clone().unwrap_or_default(),
                    l.item_no.clone(),
                    l.name.clone().unwrap_or_default(),
                    l.spec.clone().unwrap_or_default(),
                    l.location.clone(),
                    l.lot.clone(),
                    l.unit.clone(),
                    l.qty.normalize().to_string(),
                    date(l.expiry),
                    date(l.idle_on),
                ]);
            }
        }
        self.reports.check(rows.len() as u64)?;
        self.reports
            .build(
                actor,
                Some(&plant),
                &REPORT,
                rows,
                &conditions,
                format,
                lang,
            )
            .await
    }

    /// 符合條件的倉庫與其庫存明細（依倉庫排序）
    async fn search(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: &[QbeCondition],
        action: &'static str,
    ) -> Result<Vec<WarehouseStockDto>, AppError> {
        // 套用本作業的倉庫資料權限（imduser、imdgrup；WK2）
        let scope = self.authz.authorize(actor, PROGRAM, action).await?.scope();
        let plant = PlantId::parse(plant)?;
        let (header_conds, line_conds): (Vec<QbeCondition>, Vec<QbeCondition>) = conditions
            .iter()
            .cloned()
            .partition(|c| query::warehouse_stock::HEADER_FIELDS.contains(&c.field.as_str()));
        let header_filters = query::warehouse_stock::header_filters(&header_conds)?;
        let line_filters = query::warehouse_stock::line_filters(&line_conds)?;
        let all_paging = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };

        let mut warehouses: Vec<_> = self
            .warehouses
            .list_warehouses(&plant)
            .await
            .map_err(infra)?
            .into_iter()
            .filter(|w| scope.includes(&w.created_by, &w.created_dept))
            .collect();
        warehouses.sort_by(|a, b| a.code.as_str().cmp(b.code.as_str()));
        let (warehouses, _) = query::page_in_memory(
            &warehouses,
            &header_filters,
            query::warehouse_stock::header_value,
            all_paging,
        );

        // 品名、規格不受料件資料權限限制（TipTop 以 OUTER 關聯 ima_file，WK2）
        let (items, _) = self
            .items
            .query_items(&plant, &[], &DataScope::unrestricted(), 0, u64::MAX)
            .await
            .map_err(infra)?;
        let items: HashMap<&str, &Item> = items.iter().map(|i| (i.no.as_str(), i)).collect();

        let mut by_warehouse: BTreeMap<String, Vec<InventoryBalance>> = BTreeMap::new();
        for b in self
            .balances
            .list_all_inventory_balances(&plant)
            .await
            .map_err(infra)?
        {
            by_warehouse
                .entry(b.key.warehouse.clone())
                .or_default()
                .push(b);
        }

        let mut out = Vec::new();
        for w in &warehouses {
            let mut balances = by_warehouse.remove(w.code.as_str()).unwrap_or_default();
            // 依料號、儲位、批號排序（WK3）
            balances.sort_by(|a, b| {
                (&a.key.item, &a.key.location, &a.key.lot).cmp(&(
                    &b.key.item,
                    &b.key.location,
                    &b.key.lot,
                ))
            });
            let (lines, _) = query::page_in_memory(
                &balances,
                &line_filters,
                query::warehouse_stock::line_value,
                all_paging,
            );
            if !line_filters.is_empty() && lines.is_empty() {
                continue;
            }
            out.push(WarehouseStockDto {
                warehouse: w.code.as_str().to_owned(),
                name: w.attrs.name.clone(),
                lines: lines.iter().map(|b| line(b, &items)).collect(),
            });
        }
        Ok(out)
    }
}

fn line(b: &InventoryBalance, items: &HashMap<&str, &Item>) -> WarehouseStockLineDto {
    let item = items.get(b.key.item.as_str());
    WarehouseStockLineDto {
        item_no: b.key.item.clone(),
        name: item.and_then(|i| i.attrs.name.clone()),
        spec: item.and_then(|i| i.attrs.spec.clone()),
        location: b.key.location.clone(),
        lot: b.key.lot.clone(),
        unit: b.unit.clone(),
        qty: b.qty.normalize(),
        expiry: b.expiry,
        idle_on: b.idle_on,
    }
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}
