//! Item Stock Query 料件庫存數量查詢（TipTop aimq400 / ima_file、img_file）
//! 規格：specs/features/aim/item_stock_query.feature（SQ-1～SQ-4；確認事項 SQ1–SQ8）。唯讀查詢，沒有寫入。
//!
//! 料件的條件（料號、品名、規格）與庫存明細的條件（倉庫、儲位、批號、庫存量）分開：有庫存明細條件時只列出
//! 有符合明細的料件、只顯示符合的明細（SQ2）；三個合計依料件全部的庫存明細即時加總（SQ3）。

use std::collections::BTreeMap;
use std::sync::Arc;

use rust_decimal::Decimal;
use ttrs_domain::{inventory_balance::InventoryBalance, item::Item, plant::PlantId};
use ttrs_shared::{ItemStockDto, ItemStockLineDto, Page, QbeCondition, QueryRequest};

use crate::{
    Actor, AppError, Authorizer,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{InventoryBalanceRepository, ItemRepository},
    query,
    report::{self, ReportData, ReportFormat},
};

pub const PROGRAM: &str = "item-stock-query";

const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "sq-report-title",
    columns: &[
        ("item_no", "sq-col-item-no"),
        ("name", "sq-col-name"),
        ("spec", "sq-col-spec"),
        ("warehouse", "sq-col-warehouse"),
        ("location", "sq-col-location"),
        ("lot", "sq-col-lot"),
        ("usable", "sq-col-usable"),
        ("unit", "sq-col-unit"),
        ("qty", "sq-col-qty"),
        ("expiry", "sq-col-expiry"),
        ("idle_on", "sq-col-idle-on"),
    ],
};

#[derive(Clone)]
pub struct ItemStockQueryService {
    authz: Arc<dyn Authorizer>,
    items: Arc<dyn ItemRepository>,
    balances: Arc<dyn InventoryBalanceRepository>,
    reports: ListReporter,
}

impl ItemStockQueryService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        items: Arc<dyn ItemRepository>,
        balances: Arc<dyn InventoryBalanceRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            items,
            balances,
            reports,
        }
    }

    /// QBE 查詢（q400_cs、q400_b_fill；SQ2–SQ4）：依料號排序，以料件分頁
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<ItemStockDto>, AppError> {
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

    /// 列印 output、匯出 Excel exporttoexcel（SQ7）：每筆庫存明細一列，範圍＝目前查詢條件
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
        let t = |id: &str| report::text(lang, id);
        let date = |d: Option<chrono::NaiveDate>| d.map(|x| x.to_string()).unwrap_or_default();
        let mut rows: Vec<Vec<String>> = Vec::new();
        for s in &all {
            for l in &s.lines {
                rows.push(vec![
                    s.item_no.clone(),
                    s.name.clone().unwrap_or_default(),
                    s.spec.clone().unwrap_or_default(),
                    l.warehouse.clone(),
                    l.location.clone(),
                    l.lot.clone(),
                    t(if l.usable { "report-yes" } else { "report-no" }),
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

    /// 符合條件的料件與其庫存（依料號排序）
    async fn search(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: &[QbeCondition],
        action: &'static str,
    ) -> Result<Vec<ItemStockDto>, AppError> {
        // 套用本作業的料件資料權限（imauser、imagrup；SQ2）
        let scope = self.authz.authorize(actor, PROGRAM, action).await?.scope();
        let plant = PlantId::parse(plant)?;
        let (item_conds, stock_conds): (Vec<QbeCondition>, Vec<QbeCondition>) = conditions
            .iter()
            .cloned()
            .partition(|c| query::item_stock::ITEM_FIELDS.contains(&c.field.as_str()));
        let item_filters = query::item::filters(&item_conds)?;
        let stock_filters = query::item_stock::filters(&stock_conds)?;
        let (items, _) = self
            .items
            .query_items(&plant, &item_filters, &scope, 0, u64::MAX)
            .await
            .map_err(infra)?;
        let mut by_item: BTreeMap<String, Vec<InventoryBalance>> = BTreeMap::new();
        for b in self
            .balances
            .list_all_inventory_balances(&plant)
            .await
            .map_err(infra)?
        {
            by_item.entry(b.key.item.clone()).or_default().push(b);
        }
        let all_paging = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };
        let mut out = Vec::new();
        for item in &items {
            let mut balances = by_item.remove(item.no.as_str()).unwrap_or_default();
            balances.sort_by(|a, b| a.key.cmp(&b.key));
            let (lines, _) = query::page_in_memory(
                &balances,
                &stock_filters,
                query::item_stock::value,
                all_paging,
            );
            if !stock_filters.is_empty() && lines.is_empty() {
                continue;
            }
            out.push(dto(item, &balances, &lines));
        }
        Ok(out)
    }
}

fn dto(item: &Item, all: &[InventoryBalance], lines: &[InventoryBalance]) -> ItemStockDto {
    let sum = |f: &dyn Fn(&InventoryBalance) -> bool| -> Decimal {
        all.iter()
            .filter(|b| f(b))
            .map(|b| b.qty)
            .sum::<Decimal>()
            .normalize()
    };
    let a = &item.attrs;
    ItemStockDto {
        item_no: item.no.as_str().to_owned(),
        name: a.name.clone(),
        spec: a.spec.clone(),
        current_version: a.current_version.clone(),
        source_code: a.source_code.clone(),
        group_code: a.group_code.clone(),
        abc_code: a.abc_code.clone(),
        mrp_available: sum(&|b| b.mrp_usable),
        available: sum(&|b| b.usable),
        unavailable: sum(&|b| !b.usable),
        total: lines.iter().map(|b| b.qty).sum::<Decimal>().normalize(),
        lines: lines
            .iter()
            .map(|b| ItemStockLineDto {
                warehouse: b.key.warehouse.clone(),
                location: b.key.location.clone(),
                lot: b.key.lot.clone(),
                usable: b.usable,
                unit: b.unit.clone(),
                qty: b.qty.normalize(),
                expiry: b.expiry,
                idle_on: b.idle_on,
            })
            .collect(),
    }
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}
