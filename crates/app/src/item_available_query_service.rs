//! Item Available Query 料件數量查詢（TipTop aimq402 / ima_file、img_file）
//! 規格：specs/features/aim/item_available_query.feature（IA-1～IA-3；確認事項 IA1–IA7）。唯讀查詢，沒有寫入。
//!
//! 一個料件一列。可用量（ima262）由可用倉儲的庫存明細即時加總（IA3）。

use std::collections::HashMap;
use std::sync::Arc;

use rust_decimal::Decimal;
use ttrs_domain::{item::Item, plant::PlantId};
use ttrs_shared::{ItemAvailableDto, Page, QbeCondition, QueryRequest};

use crate::{
    Actor, AppError, Authorizer,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{InventoryBalanceRepository, ItemRepository},
    query,
    report::{ReportData, ReportFormat},
};

pub const PROGRAM: &str = "item-available-query";

const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "ia-report-title",
    columns: &[
        ("item_no", "ia-col-item-no"),
        ("name", "ia-col-name"),
        ("spec", "ia-col-spec"),
        ("unit", "ia-col-unit"),
        ("available", "ia-col-available"),
        ("current_version", "ia-col-version"),
        ("source_code", "ia-col-source"),
        ("group_code", "ia-col-group"),
        ("abc_code", "ia-col-abc"),
    ],
};

#[derive(Clone)]
pub struct ItemAvailableQueryService {
    authz: Arc<dyn Authorizer>,
    items: Arc<dyn ItemRepository>,
    balances: Arc<dyn InventoryBalanceRepository>,
    reports: ListReporter,
}

impl ItemAvailableQueryService {
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

    /// QBE 查詢（q402_q、q402_bf；IA2、IA3）：依料號排序並分頁
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<ItemAvailableDto>, AppError> {
        let p = query::paging(&req);
        let (items, total) = self
            .search(
                actor,
                plant,
                &req.conditions,
                "query",
                p.offset,
                u64::from(p.page_size),
            )
            .await?;
        Ok(Page {
            items,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印 output、匯出 Excel exporttoexcel（IA6）：每個料件一列，範圍＝目前查詢條件
    pub async fn report_in(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        let (all, _) = self
            .search(actor, plant, &conditions, format.action(), 0, u64::MAX)
            .await?;
        let plant = PlantId::parse(plant)?;
        let text = |v: &Option<String>| v.clone().unwrap_or_default();
        let rows: Vec<Vec<String>> = all
            .iter()
            .map(|d| {
                vec![
                    d.item_no.clone(),
                    text(&d.name),
                    text(&d.spec),
                    text(&d.unit),
                    d.available.normalize().to_string(),
                    text(&d.current_version),
                    text(&d.source_code),
                    text(&d.group_code),
                    text(&d.abc_code),
                ]
            })
            .collect();
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

    async fn search(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: &[QbeCondition],
        action: &'static str,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<ItemAvailableDto>, u64), AppError> {
        // 套用本作業的料件資料權限（imauser、imagrup；IA2）
        let scope = self.authz.authorize(actor, PROGRAM, action).await?.scope();
        let plant = PlantId::parse(plant)?;
        let filters = query::item_available::filters(conditions)?;
        let (items, total) = self
            .items
            .query_items(&plant, &filters, &scope, offset, limit)
            .await
            .map_err(infra)?;
        let mut available: HashMap<String, Decimal> = HashMap::new();
        for b in self
            .balances
            .list_all_inventory_balances(&plant)
            .await
            .map_err(infra)?
        {
            if b.usable {
                *available.entry(b.key.item).or_default() += b.qty;
            }
        }
        let out = items.iter().map(|i| dto(i, &available)).collect();
        Ok((out, total))
    }
}

fn dto(item: &Item, available: &HashMap<String, Decimal>) -> ItemAvailableDto {
    let a = &item.attrs;
    ItemAvailableDto {
        item_no: item.no.as_str().to_owned(),
        name: a.name.clone(),
        spec: a.spec.clone(),
        unit: a.stock_unit.clone(),
        available: available
            .get(item.no.as_str())
            .copied()
            .unwrap_or_default()
            .normalize(),
        current_version: a.current_version.clone(),
        source_code: a.source_code.clone(),
        group_code: a.group_code.clone(),
        abc_code: a.abc_code.clone(),
    }
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}
