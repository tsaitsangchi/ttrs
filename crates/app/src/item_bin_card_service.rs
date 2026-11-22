//! Item Bin Card 料件 BIN 卡查詢（TipTop aimq231 / img_file、tlf_file、ima_file；imk_file 月結尚未移植）
//! 規格：specs/features/aim/item_bin_card.feature（BN-1～BN-4；確認事項 BN1–BN7）。唯讀查詢，沒有寫入。
//!
//! 一張 BIN 卡＝一個倉儲批：期初庫存＋所選期別起始日起的每一筆異動與累計結存（BN4）。
//! 期初庫存以「目前庫存 －期初日之後所有異動的淨數量」反推（BN3；月結移植後改讀月結庫存）。

use std::sync::Arc;

use rust_decimal::Decimal;
use ttrs_domain::{
    accounting_period::Calendar, inventory_balance::InventoryBalance, item::Item, plant::PlantId,
};
use ttrs_shared::{BinCardDto, BinCardLineDto, BinCardQuery, Page, QbeCondition};

use crate::{
    Actor, AppError, Authorizer,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{
        AccountingPeriodRepository, InventoryBalanceRepository, InventoryPostingRepository,
        ItemRepository,
    },
    query,
    report::{self, ReportData, ReportFormat},
};

pub const PROGRAM: &str = "item-bin-card";

const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "bn-report-title",
    columns: &[
        ("item_no", "bn-col-item-no"),
        ("warehouse", "bn-col-warehouse"),
        ("location", "bn-col-location"),
        ("lot", "bn-col-lot"),
        ("txn_date", "bn-col-txn-date"),
        ("program", "bn-col-program"),
        ("doc_no", "bn-col-doc-no"),
        ("doc_line", "bn-col-doc-line"),
        ("qty", "bn-col-qty"),
        ("unit", "bn-col-unit"),
        ("balance", "bn-col-balance"),
    ],
};

#[derive(Clone)]
pub struct ItemBinCardService {
    authz: Arc<dyn Authorizer>,
    items: Arc<dyn ItemRepository>,
    balances: Arc<dyn InventoryBalanceRepository>,
    posting: Arc<dyn InventoryPostingRepository>,
    periods: Arc<dyn AccountingPeriodRepository>,
    reports: ListReporter,
}

impl ItemBinCardService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        items: Arc<dyn ItemRepository>,
        balances: Arc<dyn InventoryBalanceRepository>,
        posting: Arc<dyn InventoryPostingRepository>,
        periods: Arc<dyn AccountingPeriodRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            items,
            balances,
            posting,
            periods,
            reports,
        }
    }

    /// QBE 查詢（q231_curs、q231_b_fill；BN2–BN4）：一個倉儲批一筆，依料號、倉庫、儲位、批號排序並分頁
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        q: BinCardQuery,
    ) -> Result<Page<BinCardDto>, AppError> {
        let all = self
            .search(
                actor,
                plant,
                q.year,
                q.period,
                &q.request.conditions,
                "query",
            )
            .await?;
        let p = query::paging(&q.request);
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

    /// 列印 output、匯出 Excel exporttoexcel（BN6）：每個倉儲批先一列期初，再每筆異動一列
    #[allow(clippy::too_many_arguments)]
    pub async fn report_in(
        &self,
        actor: &Actor,
        plant: &str,
        year: i32,
        period: i16,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        let all = self
            .search(actor, plant, year, period, &conditions, format.action())
            .await?;
        let plant = PlantId::parse(plant)?;
        let opening = report::text(lang, "bn-opening");
        let mut rows: Vec<Vec<String>> = Vec::new();
        for c in &all {
            rows.push(vec![
                c.item_no.clone(),
                c.warehouse.clone(),
                c.location.clone(),
                c.lot.clone(),
                String::new(),
                opening.clone(),
                String::new(),
                String::new(),
                String::new(),
                c.unit.clone(),
                c.opening.normalize().to_string(),
            ]);
            for l in &c.lines {
                rows.push(vec![
                    c.item_no.clone(),
                    c.warehouse.clone(),
                    c.location.clone(),
                    c.lot.clone(),
                    l.txn_date.to_string(),
                    report::text(lang, &format!("bn-program-{}", l.program)),
                    l.doc_no.clone(),
                    l.doc_line.to_string(),
                    l.qty.normalize().to_string(),
                    l.unit.clone(),
                    l.balance.normalize().to_string(),
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
        year: i32,
        period: i16,
        conditions: &[QbeCondition],
        action: &'static str,
    ) -> Result<Vec<BinCardDto>, AppError> {
        // 套用本作業的料件資料權限（BN2）
        let scope = self.authz.authorize(actor, PROGRAM, action).await?.scope();
        let plant = PlantId::parse(plant)?;
        // 年度期別須存在（期別不可超過該年度的期間數，agl-020）；期初日＝該期起始日（s_azn01）
        let years = self.periods.list_accounting_years().await.map_err(infra)?;
        let from = Calendar::new(&years).period(year, period)?.start;
        let (item_conds, stock_conds): (Vec<QbeCondition>, Vec<QbeCondition>) = conditions
            .iter()
            .cloned()
            .partition(|c| query::bin_card::ITEM_FIELDS.contains(&c.field.as_str()));
        let item_filters = query::item::filters(&item_conds)?;
        let stock_filters = query::bin_card::filters(&stock_conds)?;
        let (items, _) = self
            .items
            .query_items(&plant, &item_filters, &scope, 0, u64::MAX)
            .await
            .map_err(infra)?;
        let mut balances = self
            .balances
            .list_all_inventory_balances(&plant)
            .await
            .map_err(infra)?;
        balances.sort_by(|a, b| a.key.cmp(&b.key));
        let all_paging = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };
        let (balances, _) = query::page_in_memory(
            &balances,
            &stock_filters,
            query::bin_card::value,
            all_paging,
        );
        let mut out = Vec::new();
        for b in &balances {
            let Some(item) = items.iter().find(|i| i.no.as_str() == b.key.item) else {
                continue;
            };
            let records = self
                .posting
                .inventory_txn_records_since(&plant, &b.key, from)
                .await
                .map_err(infra)?;
            out.push(card(item, b, year, period, &records));
        }
        Ok(out)
    }
}

fn card(
    item: &Item,
    b: &InventoryBalance,
    year: i32,
    period: i16,
    records: &[ttrs_domain::inventory_balance::InventoryTxnRecord],
) -> BinCardDto {
    let signed =
        |r: &ttrs_domain::inventory_balance::InventoryTxnRecord| r.qty * Decimal::from(r.direction);
    // 期初＝目前庫存－期初日之後所有異動的淨數量（BN3）
    let opening = b.qty - records.iter().map(signed).sum::<Decimal>();
    let mut balance = opening;
    let lines = records
        .iter()
        .map(|r| {
            let qty = signed(r);
            balance += qty;
            BinCardLineDto {
                txn_date: r.txn_date,
                program: r.program.clone(),
                doc_no: r.doc_no.clone(),
                doc_line: r.doc_line,
                qty: qty.normalize(),
                unit: r.unit.clone(),
                balance: balance.normalize(),
            }
        })
        .collect();
    BinCardDto {
        item_no: item.no.as_str().to_owned(),
        name: item.attrs.name.clone(),
        spec: item.attrs.spec.clone(),
        source_code: item.attrs.source_code.clone(),
        warehouse: b.key.warehouse.clone(),
        location: b.key.location.clone(),
        lot: b.key.lot.clone(),
        unit: b.unit.clone(),
        year,
        period,
        opening: opening.normalize(),
        current: b.qty.normalize(),
        lines,
    }
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}
