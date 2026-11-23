//! Multi-Plant Stock Query 料件多工廠庫存數量查詢（TipTop aimq404 / ima_file、img_file、azp_file）
//! 規格：specs/features/aim/multi_plant_stock_query.feature（MP-1～MP-4；確認事項 MP1–MP8）。唯讀查詢，沒有寫入。
//!
//! 料件清單取自目前營運中心；明細列出同一料號在使用者可使用且有效的各營運中心的庫存（MP3，與 TipTop 刻意不同）。
//! 單身條件只過濾明細，沒有符合時料件仍列出（MP2）。

use std::collections::BTreeMap;
use std::sync::Arc;

use rust_decimal::Decimal;
use ttrs_domain::{
    inventory_balance::InventoryBalance, item::Item, plant::PlantId, sign_in::sign_in_plants,
    user::UserId,
};
use ttrs_shared::{MultiPlantStockDto, MultiPlantStockLineDto, Page, QbeCondition, QueryRequest};

use crate::{
    Actor, AppError, Authorizer,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{InventoryBalanceRepository, ItemRepository, PlantRepository, UserRepository},
    query,
    report::{self, ReportData, ReportFormat},
};

pub const PROGRAM: &str = "multi-plant-stock-query";

const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "mp-report-title",
    columns: &[
        ("item_no", "mp-col-item-no"),
        ("name", "mp-col-name"),
        ("spec", "mp-col-spec"),
        ("plant", "mp-col-plant"),
        ("warehouse", "mp-col-warehouse"),
        ("location", "mp-col-location"),
        ("lot", "mp-col-lot"),
        ("usable", "mp-col-usable"),
        ("unit", "mp-col-unit"),
        ("qty", "mp-col-qty"),
        ("expiry", "mp-col-expiry"),
        ("idle_on", "mp-col-idle-on"),
    ],
};

#[derive(Clone)]
pub struct MultiPlantStockQueryService {
    authz: Arc<dyn Authorizer>,
    items: Arc<dyn ItemRepository>,
    balances: Arc<dyn InventoryBalanceRepository>,
    plants: Arc<dyn PlantRepository>,
    users: Arc<dyn UserRepository>,
    reports: ListReporter,
}

impl MultiPlantStockQueryService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        items: Arc<dyn ItemRepository>,
        balances: Arc<dyn InventoryBalanceRepository>,
        plants: Arc<dyn PlantRepository>,
        users: Arc<dyn UserRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            items,
            balances,
            plants,
            users,
            reports,
        }
    }

    /// QBE 查詢（q404_cs、q404_b_fill；MP2–MP5）：依料號排序，以料件分頁
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<MultiPlantStockDto>, AppError> {
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

    /// 列印 output、匯出 Excel exporttoexcel（MP7）：每筆明細一列，範圍＝目前查詢條件
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
        let t = |id: &str| report::text(lang, id);
        let date = |d: Option<chrono::NaiveDate>| d.map(|x| x.to_string()).unwrap_or_default();
        let mut rows: Vec<Vec<String>> = Vec::new();
        for s in &all {
            for l in &s.lines {
                rows.push(vec![
                    s.item_no.clone(),
                    s.name.clone().unwrap_or_default(),
                    s.spec.clone().unwrap_or_default(),
                    l.plant.clone(),
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

    async fn search(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: &[QbeCondition],
        action: &'static str,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<MultiPlantStockDto>, u64), AppError> {
        // 套用本作業的料件資料權限（imauser、imagrup；MP2）
        let scope = self.authz.authorize(actor, PROGRAM, action).await?.scope();
        let plant = PlantId::parse(plant)?;
        let (item_conds, line_conds): (Vec<QbeCondition>, Vec<QbeCondition>) = conditions
            .iter()
            .cloned()
            .partition(|c| query::multi_plant_stock::ITEM_FIELDS.contains(&c.field.as_str()));
        let item_filters = query::item::filters(&item_conds)?;
        let line_filters = query::multi_plant_stock::filters(&line_conds)?;
        let (items, total) = self
            .items
            .query_items(&plant, &item_filters, &scope, offset, limit)
            .await
            .map_err(infra)?;

        // 同一料號在各可使用營運中心的庫存，依營運中心排序
        let all_paging = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };
        let mut by_item: BTreeMap<String, Vec<(String, InventoryBalance)>> = BTreeMap::new();
        for p in self.visible_plants(actor).await? {
            let mut balances = self
                .balances
                .list_all_inventory_balances(&p)
                .await
                .map_err(infra)?;
            balances.sort_by(|a, b| a.key.cmp(&b.key));
            let (hits, _) = query::page_in_memory(
                &balances,
                &line_filters,
                query::multi_plant_stock::value,
                all_paging,
            );
            for b in hits {
                by_item
                    .entry(b.key.item.clone())
                    .or_default()
                    .push((p.as_str().to_owned(), b));
            }
        }
        let out = items
            .iter()
            .map(|i| dto(i, by_item.get(i.no.as_str()).map_or(&[], Vec::as_slice)))
            .collect();
        Ok((out, total))
    }

    /// 使用者可使用且有效的營運中心，依代號排序（MP3，規則同登入選單 SQ13）；找不到使用者時一個都不列
    async fn visible_plants(&self, actor: &Actor) -> Result<Vec<PlantId>, AppError> {
        let Ok(id) = UserId::parse(&actor.user_id) else {
            return Ok(Vec::new());
        };
        let Some(user) = self.users.get(&id).await.map_err(infra)? else {
            return Ok(Vec::new());
        };
        let allowed = self.users.plants(&id).await.map_err(infra)?;
        let all = self.plants.list().await.map_err(infra)?;
        let mut ids: Vec<PlantId> = sign_in_plants(&user, &allowed, &all)
            .into_iter()
            .map(|p| p.id.clone())
            .collect();
        ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        Ok(ids)
    }
}

fn dto(item: &Item, lines: &[(String, InventoryBalance)]) -> MultiPlantStockDto {
    let a = &item.attrs;
    MultiPlantStockDto {
        item_no: item.no.as_str().to_owned(),
        name: a.name.clone(),
        spec: a.spec.clone(),
        current_version: a.current_version.clone(),
        source_code: a.source_code.clone(),
        group_code: a.group_code.clone(),
        abc_code: a.abc_code.clone(),
        total: lines
            .iter()
            .map(|(_, b)| b.qty)
            .sum::<Decimal>()
            .normalize(),
        lines: lines
            .iter()
            .map(|(plant, b)| MultiPlantStockLineDto {
                plant: plant.clone(),
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
