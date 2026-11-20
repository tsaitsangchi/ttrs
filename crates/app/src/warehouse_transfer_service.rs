//! Warehouse Transfer 庫存直接調撥（TipTop aimt324 / imm_file、imn_file → warehouse_transfer、warehouse_transfer_line；
//! 過帳還原 aimp378）。規格：specs/features/aim/warehouse_transfer.feature（WT-1～WT-15；確認事項 WT1–WT14）。
//! 寫入一律經 change_log::run（ADR 0012）。
//!
//! 本作業的規則：單別性質 4 調撥；以調撥日編號與做期間檢查（輸入與過帳時都檢查，過帳不可改日期，WT3(a)）；
//! 撥出須有庫存、不超過可用量（扣除未過帳雜發與調撥，WT6）；撥入依存放位置管制、可收到新倉儲批（WT5）；
//! 過帳每筆單身兩筆異動記錄（撥出 50 → 99、撥入 99 → 50，WT9）。

use std::sync::Arc;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use ttrs_domain::{
    DomainError,
    accounting_period::Calendar,
    classification::{Code, CodeKind},
    department::DeptCode,
    doc_no,
    document_type::{DocTypeCode, DocumentType},
    employee::EmployeeId,
    inventory_balance::{BalanceKey, InventoryBalance, InventoryTxnRecord},
    inventory_parameter::{InventorySettings, StorageControl},
    inventory_transaction,
    item::{ActiveCode, ItemNo},
    plant::PlantId,
    user::UserId,
    warehouse::WarehouseCode,
    warehouse_transfer::{self, TransferHeader, TransferLine, WarehouseTransfer},
};
use ttrs_shared::{
    ChangeDto, Page, QbeCondition, QueryRequest, WarehouseTransferCopyInput, WarehouseTransferDto,
    WarehouseTransferInput, WarehouseTransferLineDto, WarehouseTransferLineInput,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    change_log::HistoryReader,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{
        AccountingPeriodRepository, ClassificationCodeRepository, DepartmentRepository,
        DocumentTypeRepository, EmployeeRepository, InventoryBalanceRepository,
        InventoryParameterRepository, InventoryPostingRepository, InventoryTransactionRepository,
        ItemDates, ItemRepository, ItemStorageRepository, PostedDoc, PostingWrite,
        SystemParameterRepository, UserRepository, WarehouseRepository,
        WarehouseTransferRepository,
    },
    query,
    report::{self, ReportData, ReportFormat},
};

pub const PROGRAM: &str = warehouse_transfer::PROGRAM;
const HEAD_TABLE: &str = "warehouse_transfer";
const LINE_TABLE: &str = "warehouse_transfer_line";

const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "wt-report-title",
    columns: &[
        ("doc_no", "wt-col-doc-no"),
        ("transfer_date", "wt-col-transfer-date"),
        ("dept", "wt-col-dept"),
        ("applicant", "wt-col-applicant"),
        ("status", "wt-col-status"),
        ("line_no", "wt-col-line-no"),
        ("item_no", "wt-col-item-no"),
        ("item_name", "wt-col-item-name"),
        ("from", "wt-col-from"),
        ("to", "wt-col-to"),
        ("unit", "wt-col-unit"),
        ("qty", "wt-col-qty"),
        ("reason", "wt-col-reason"),
    ],
};

/// 本作業用到的資料存取
#[derive(Clone)]
pub struct WarehouseTransferRepos {
    pub authz: Arc<dyn Authorizer>,
    pub transfers: Arc<dyn WarehouseTransferRepository>,
    /// 可用量扣除未過帳雜發（WT6）
    pub misc: Arc<dyn InventoryTransactionRepository>,
    pub balances: Arc<dyn InventoryBalanceRepository>,
    pub posting: Arc<dyn InventoryPostingRepository>,
    pub docs: Arc<dyn DocumentTypeRepository>,
    pub params: Arc<dyn SystemParameterRepository>,
    pub inventory_params: Arc<dyn InventoryParameterRepository>,
    pub periods: Arc<dyn AccountingPeriodRepository>,
    pub users: Arc<dyn UserRepository>,
    pub depts: Arc<dyn DepartmentRepository>,
    pub employees: Arc<dyn EmployeeRepository>,
    pub items: Arc<dyn ItemRepository>,
    pub warehouses: Arc<dyn WarehouseRepository>,
    pub item_storages: Arc<dyn ItemStorageRepository>,
    pub codes: Arc<dyn ClassificationCodeRepository>,
    pub history: HistoryReader,
    pub reports: ListReporter,
}

#[derive(Clone)]
pub struct WarehouseTransferService {
    r: WarehouseTransferRepos,
}

impl WarehouseTransferService {
    pub fn new(r: WarehouseTransferRepos) -> Self {
        Self { r }
    }

    /// QBE 查詢（t324_cs）；依單號排序
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<WarehouseTransferDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let filters = query::warehouse_transfer::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.sorted(&plant).await?;
        let (rows, total) =
            query::page_in_memory(&all, &filters, query::warehouse_transfer::value, p);
        let mut items = Vec::with_capacity(rows.len());
        for t in rows {
            items.push(self.dto(&plant, &t).await?);
        }
        Ok(Page {
            items,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    pub async fn get(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
    ) -> Result<WarehouseTransferDto, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let t = self.load(&plant, no).await?;
        self.dto(&plant, &t).await
    }

    /// 新增（insert；t324_a）：單別「立即確認」時有單身就自動確認（同 MT10）
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        input: WarehouseTransferInput,
    ) -> Result<WarehouseTransferDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let plant = PlantId::parse(plant)?;
            let d = self.doc_type(&plant, &input.doc_no, &actor.user_id).await?;
            let header = self.header(&plant, actor, &input, None).await?;
            self.check_period(&plant, header.transfer_date, true)
                .await?;
            let lines = self
                .lines(&plant, &d, input.lines.as_deref().unwrap_or_default(), "")
                .await?;
            let no = self
                .assign_no(&plant, &d, &input.doc_no, header.transfer_date)
                .await?;
            let t =
                WarehouseTransfer::new(no, header, lines, &actor.user_id, &actor.dept, today())?;
            let t = self.insert(&plant, &d, &input.doc_no, t).await?;
            if d.attrs.confirm_now && !t.lines.is_empty() {
                crate::change_log::set_operation("confirm");
                return match self.confirm_doc(&plant, t.clone(), actor).await {
                    Ok(t) => self.dto(&plant, &t).await,
                    Err(e) => {
                        let mut dto = self.dto(&plant, &t).await?;
                        dto.auto_confirm_error = Some(e.code().to_owned());
                        Ok(dto)
                    }
                };
            }
            self.dto(&plant, &t).await
        })
        .await
    }

    /// 修改單頭與單身（modify、detail；t324_u、t324_b）：已確認 9023、已作廢 9024 不可
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        input: WarehouseTransferInput,
    ) -> Result<WarehouseTransferDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            let head = input.transfer_date.is_some()
                || input.dept.is_some()
                || input.applicant.is_some()
                || input.memo.is_some();
            if input.lines.is_some() && !head {
                crate::change_log::set_operation("detail");
            }
            let mut grant = None;
            if head || input.lines.is_none() {
                grant = Some(self.authorize(actor, "modify").await?);
            }
            if input.lines.is_some() {
                grant = Some(self.authorize(actor, "detail").await?);
            }
            let grant = grant.expect("至少檢查一個功能");
            let plant = PlantId::parse(plant)?;
            let mut t = self.load(&plant, no).await?;
            grant.record(&t.created_by, &t.created_dept)?;
            t.stamp.check(input.version)?;
            t.check_editable()?;
            let d = self.doc_type(&plant, &t.no, &actor.user_id).await?;
            let header = self.header(&plant, actor, &input, Some(&t.header)).await?;
            if header.transfer_date != t.header.transfer_date {
                self.check_period(&plant, header.transfer_date, true)
                    .await?;
            }
            let lines = match &input.lines {
                Some(l) => self.lines(&plant, &d, l, &t.no).await?,
                None => t.lines.clone(),
            };
            t.update(header, lines, &actor.user_id, today())?;
            let t = self.save(&plant, &t).await?;
            self.dto(&plant, &t).await
        })
        .await
    }

    /// 刪除（delete；t324_r）：已確認、已作廢不可；連同單身
    pub async fn delete(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            let grant = self.authorize(actor, "delete").await?;
            let plant = PlantId::parse(plant)?;
            let t = self.load(&plant, no).await?;
            grant.record(&t.created_by, &t.created_dept)?;
            t.stamp.check(version)?;
            t.check_editable()?;
            self.r
                .transfers
                .delete_warehouse_transfer(&plant, &t.no)
                .await
                .map_err(infra)
        })
        .await
    }

    /// 作廢／取消作廢（void；t324_x，WT11）：已確認、已過帳不可；調撥日不可小於等於成本關帳日（mfg9084）
    pub async fn set_void(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        void: bool,
        version: Option<i32>,
    ) -> Result<WarehouseTransferDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "void", async {
            let grant = self.authorize(actor, "void").await?;
            let plant = PlantId::parse(plant)?;
            let mut t = self.load(&plant, no).await?;
            grant.record(&t.created_by, &t.created_dept)?;
            t.stamp.check(version)?;
            crate::change_log::set_operation(if void { "void" } else { "unvoid" });
            t.set_void(void, &actor.user_id, today())?;
            self.check_period(&plant, t.header.transfer_date, false)
                .await?;
            let t = self.save(&plant, &t).await?;
            self.dto(&plant, &t).await
        })
        .await
    }

    /// 確認（confirm；t324sub_y_chk、t324sub_y_upd）／取消確認（undo_confirm；t324_z）
    pub async fn confirm(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<WarehouseTransferDto, AppError> {
        let action = if confirm { "confirm" } else { "undo_confirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            crate::change_log::set_operation(action);
            let plant = PlantId::parse(plant)?;
            let mut t = self.load(&plant, no).await?;
            t.stamp.check(version)?;
            let t = if confirm {
                self.confirm_doc(&plant, t, actor).await?
            } else {
                t.unconfirm(&actor.user_id, today())?;
                self.save(&plant, &t).await?
            };
            self.dto(&plant, &t).await
        })
        .await
    }

    /// 複製（reproduce；WT12）：新單別或完整單號、調撥日；單身一併複製、不重檢
    pub async fn copy(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        input: WarehouseTransferCopyInput,
    ) -> Result<WarehouseTransferDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let plant = PlantId::parse(plant)?;
            let source = self.load(&plant, no).await?;
            let d = self.doc_type(&plant, &input.doc_no, &actor.user_id).await?;
            let date = input.transfer_date.unwrap_or_else(today);
            self.check_period(&plant, date, true).await?;
            let new_no = self.assign_no(&plant, &d, &input.doc_no, date).await?;
            let t = source.copy_as(new_no, date, &actor.user_id, &actor.dept, today());
            let t = self.insert(&plant, &d, &input.doc_no, t).await?;
            self.dto(&plant, &t).await
        })
        .await
    }

    /// 過帳（post；t324_s，WT9）：以調撥日做期間檢查；撥出扣庫存、撥入加庫存（沒有時新建、最高存量檢查）；
    /// 每筆單身兩筆異動記錄；任何一筆失敗整張不過帳
    pub async fn post(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        version: Option<i32>,
    ) -> Result<WarehouseTransferDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "post", async {
            self.authorize(actor, "post").await?;
            crate::change_log::set_operation("post");
            let plant = PlantId::parse(plant)?;
            let mut t = self.load(&plant, no).await?;
            t.stamp.check(version)?;
            let date = t.header.transfer_date;
            self.check_period(&plant, date, true).await?;
            t.post(&actor.user_id, today())?;
            let d = self.posting_doc_type(&plant, &t.no).await?;
            let mut balances: Vec<(Option<Decimal>, InventoryBalance)> = Vec::new();
            let mut records = Vec::new();
            let mut items: Vec<ItemDates> = Vec::new();
            for l in &t.lines {
                // 寫異動記錄時再檢查單別倉庫設限（s_tlf → s_chksmz，PT7）
                d.check_storage(
                    &l.from_warehouse,
                    Some(l.from_location.as_str()).filter(|x| !x.is_empty()),
                )?;
                d.check_storage(
                    &l.to_warehouse,
                    Some(l.to_location.as_str()).filter(|x| !x.is_empty()),
                )?;
                // 撥出（t324_t：s_upimg -1）
                let from = l.from_key();
                let i = self.existing_balance(&plant, &mut balances, &from).await?;
                balances[i].1.take(l.qty, date, true)?;
                let from_after = balances[i].1.qty;
                // 撥入（t324_t2：s_upimg +1；沒有庫存資料時新建，PR1）
                let to = l.to_key();
                let j = match balances.iter().position(|(_, b)| b.key == to) {
                    Some(j) => j,
                    None => {
                        match self
                            .r
                            .balances
                            .get_inventory_balance(&plant, &to)
                            .await
                            .map_err(infra)?
                        {
                            Some(b) => balances.push((Some(b.qty), b)),
                            None => {
                                let b =
                                    self.new_storage(&plant, to.clone(), l, &t.no, date).await?;
                                balances.push((None, b));
                            }
                        }
                        balances.len() - 1
                    }
                };
                balances[j].1.put(l.qty, date);
                let to_after = balances[j].1.qty;
                self.check_max_qty(&plant, &to, &balances).await?;
                let from_cost = self.cost_group(&plant, &l.from_warehouse).await?;
                let to_cost = self.cost_group(&plant, &l.to_warehouse).await?;
                let record =
                    |key: BalanceKey,
                     direction: i16,
                     source: i16,
                     dest: i16,
                     balance_after: Decimal,
                     cost_group: Option<String>| InventoryTxnRecord {
                        key,
                        doc_no: t.no.clone(),
                        doc_line: l.line_no,
                        doc_type: d.code.as_str().to_owned(),
                        direction,
                        source,
                        dest,
                        txn_date: date,
                        qty: l.qty,
                        unit: l.unit.clone(),
                        balance_after,
                        program: PROGRAM.to_owned(),
                        reason: l.reason.clone(),
                        memo: t.header.memo.clone(),
                        dept: t.header.dept.as_ref().map(|x| x.as_str().to_owned()),
                        cost_group,
                        created_by: actor.user_id.clone(),
                        created_at: Some(chrono::Utc::now()),
                    };
                // 撥出 50 → 99、撥入 99 → 50（t324_log_2）
                records.push(record(from, -1, 50, 99, from_after, from_cost));
                records.push(record(to, 1, 99, 50, to_after, to_cost));
                if !items.iter().any(|x| x.item == l.item) {
                    items.push(self.item_dates(&plant, &l.item, date).await?);
                }
            }
            let w = PostingWrite {
                doc: PostedDoc::Transfer(t.clone()),
                balances,
                add_records: records,
                remove_records: false,
                items,
            };
            self.apply(&plant, &w).await?;
            let t = self.load(&plant, &t.no).await?;
            self.dto(&plant, &t).await
        })
        .await
    }

    /// 過帳還原（undo_post；aimp378，WT10）：調撥日不可小於等於成本關帳日；撥入扣回（不可小於 0）、撥出加回、
    /// 刪除異動記錄，單據回到已確認、未過帳
    pub async fn undo_post(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        version: Option<i32>,
    ) -> Result<WarehouseTransferDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "undo_post", async {
            self.authorize(actor, "undo_post").await?;
            crate::change_log::set_operation("undo_post");
            let plant = PlantId::parse(plant)?;
            let mut t = self.load(&plant, no).await?;
            t.stamp.check(version)?;
            t.unpost(&actor.user_id, today())?;
            self.check_period(&plant, t.header.transfer_date, false)
                .await?;
            let mut balances: Vec<(Option<Decimal>, InventoryBalance)> = Vec::new();
            for l in &t.lines {
                let to = l.to_key();
                let j = self.existing_balance(&plant, &mut balances, &to).await?;
                balances[j].1.undo(-l.qty)?;
                let from = l.from_key();
                let i = self.existing_balance(&plant, &mut balances, &from).await?;
                balances[i].1.undo(l.qty)?;
            }
            let w = PostingWrite {
                doc: PostedDoc::Transfer(t.clone()),
                balances,
                add_records: Vec::new(),
                remove_records: true,
                items: Vec::new(),
            };
            self.apply(&plant, &w).await?;
            let t = self.load(&plant, &t.no).await?;
            self.dto(&plant, &t).await
        })
        .await
    }

    /// 本單的庫存異動記錄，依項次、方向排序
    pub async fn txn_records(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
    ) -> Result<Vec<InventoryTxnRecord>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        self.r
            .posting
            .inventory_txn_records(&plant, PROGRAM, no.trim())
            .await
            .map_err(infra)
    }

    /// 撥入到新的倉儲批時提示（同雜收 MT8(b)）：參數為是時回傳撥入還沒有庫存資料的單身列
    pub async fn new_storages(
        &self,
        actor: &Actor,
        plant: &str,
        lines: Vec<WarehouseTransferLineInput>,
    ) -> Result<Vec<usize>, AppError> {
        self.authorize(actor, "insert").await?;
        let plant = PlantId::parse(plant)?;
        if !self.settings(&plant).await?.misc_receipt_new_location {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            let from_lot = l.from_lot.clone().unwrap_or_default();
            let to_lot = l
                .to_lot
                .clone()
                .filter(|x| !x.trim().is_empty())
                .unwrap_or(from_lot);
            let key = BalanceKey::new(
                l.item_no.trim(),
                l.to_warehouse.trim(),
                &l.to_location.clone().unwrap_or_default(),
                &to_lot,
            );
            if self
                .r
                .balances
                .get_inventory_balance(&plant, &key)
                .await
                .map_err(infra)?
                .is_none()
            {
                out.push(i);
            }
        }
        Ok(out)
    }

    /// 異動記錄：單頭與單身（ADR 0012 §8）
    pub async fn history(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        self.authorize(actor, "history").await?;
        let plant = PlantId::parse(plant)?;
        self.r
            .history
            .read(
                Some(&plant),
                &[HEAD_TABLE, LINE_TABLE],
                &[("doc_no", no.trim())],
                lang,
            )
            .await
    }

    /// 列印 output、匯出 Excel exporttoexcel（WT12）：每筆單身一列
    pub async fn report_in(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let plant = PlantId::parse(plant)?;
        let filters = query::warehouse_transfer::filters(&conditions)?;
        let all = self.sorted(&plant).await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };
        let (rows, _) = query::page_in_memory(&all, &filters, query::warehouse_transfer::value, p);
        let t = |id: &str| report::text(lang, id);
        let storage = |w: &str, l: &str, lot: &str| {
            [w, l, lot]
                .iter()
                .filter(|x| !x.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join("/")
        };
        let mut lines: Vec<Vec<String>> = Vec::new();
        for x in &rows {
            let h = &x.header;
            let status = t(&format!("wt-status-{}", x.status.code().to_lowercase()));
            for l in &x.lines {
                let name = self.item_name(&plant, &l.item).await?;
                lines.push(vec![
                    x.no.clone(),
                    h.transfer_date.to_string(),
                    h.dept
                        .as_ref()
                        .map(|d| d.as_str().to_owned())
                        .unwrap_or_default(),
                    h.applicant.clone().unwrap_or_default(),
                    status.clone(),
                    l.line_no.to_string(),
                    l.item.clone(),
                    name.unwrap_or_default(),
                    storage(&l.from_warehouse, &l.from_location, &l.from_lot),
                    storage(&l.to_warehouse, &l.to_location, &l.to_lot),
                    l.unit.clone(),
                    l.qty.normalize().to_string(),
                    l.reason.clone().unwrap_or_default(),
                ]);
            }
        }
        self.r.reports.check(lines.len() as u64)?;
        self.r
            .reports
            .build(
                actor,
                Some(&plant),
                &REPORT,
                lines,
                &conditions,
                format,
                lang,
            )
            .await
    }

    // ----- 內部 -----

    /// 確認（t324sub_y_chk；WT8）：狀態、須有單身；每筆都有撥出與撥入倉庫（單身檢查已保證）；撥出仍須有庫存資料
    async fn confirm_doc(
        &self,
        plant: &PlantId,
        mut t: WarehouseTransfer,
        actor: &Actor,
    ) -> Result<WarehouseTransfer, AppError> {
        t.confirm(&actor.user_id, today())?;
        for l in &t.lines {
            if self
                .r
                .balances
                .get_inventory_balance(plant, &l.from_key())
                .await
                .map_err(infra)?
                .is_none()
            {
                return Err(DomainError::InventoryBalanceNotFound.into());
            }
        }
        self.save(plant, &t).await
    }

    /// 單身（t324_b；WT4–WT7）
    async fn lines(
        &self,
        plant: &PlantId,
        d: &DocumentType,
        input: &[WarehouseTransferLineInput],
        exclude_no: &str,
    ) -> Result<Vec<TransferLine>, AppError> {
        let settings = self.settings(plant).await?;
        let mut out: Vec<TransferLine> = Vec::with_capacity(input.len());
        for l in input {
            let qty = inventory_transaction::check_qty(l.qty)?;
            let no = ItemNo::parse(&l.item_no).map_err(|_| DomainError::ItemNotFound)?;
            let item = self
                .r
                .items
                .get_item(plant, &no)
                .await
                .map_err(infra)?
                .ok_or(DomainError::ItemNotFound)?;
            if item.active_code != ActiveCode::Confirmed {
                return Err(DomainError::ItemInactive.into());
            }
            if item.no.as_str().to_uppercase().starts_with("MISC") {
                return Err(DomainError::InventoryTransactionItemMisc.into());
            }
            // 撥出（WT4）：倉庫有效、單別倉庫設限、須有庫存資料、有效日期未過
            let from_wh = self.active_warehouse(plant, &l.from_warehouse).await?;
            let from_loc = l
                .from_location
                .clone()
                .unwrap_or_default()
                .trim()
                .to_owned();
            let from_lot = l.from_lot.clone().unwrap_or_default().trim().to_owned();
            inventory_transaction::check_storage_text(&from_loc, &from_lot)?;
            d.check_storage(
                from_wh.as_str(),
                Some(from_loc.as_str()).filter(|x| !x.is_empty()),
            )?;
            let from_key =
                BalanceKey::new(item.no.as_str(), from_wh.as_str(), &from_loc, &from_lot);
            let balance = self
                .r
                .balances
                .get_inventory_balance(plant, &from_key)
                .await
                .map_err(infra)?
                .ok_or(DomainError::InventoryBalanceNotFound)?;
            if balance.expiry.is_some_and(|e| e < today()) {
                return Err(DomainError::InventoryBalanceExpired.into());
            }
            // 數量與可用量（WT6）：不可大於庫存；另扣除其他未過帳的雜發與調撥
            let pending_issue = self
                .r
                .misc
                .pending_issue_qty(plant, &from_key, "")
                .await
                .map_err(infra)?;
            let pending_transfer = self
                .r
                .transfers
                .pending_transfer_qty(plant, &from_key, exclude_no)
                .await
                .map_err(infra)?;
            if qty > balance.qty || qty + pending_issue + pending_transfer > balance.qty {
                return Err(DomainError::InventoryInsufficient.into());
            }
            // 撥入（WT5）：倉庫有效、存放位置管制、有庫存資料時有效日期未過；批號空白時帶撥出批號
            let to_wh = self.active_warehouse(plant, &l.to_warehouse).await?;
            let to_loc = l.to_location.clone().unwrap_or_default().trim().to_owned();
            let to_lot = l
                .to_lot
                .clone()
                .map(|x| x.trim().to_owned())
                .filter(|x| !x.is_empty())
                .unwrap_or_else(|| from_lot.clone());
            inventory_transaction::check_storage_text(&to_loc, &to_lot)?;
            self.check_storage_control(plant, &settings, &item.no, to_wh.as_str(), &to_loc)
                .await?;
            let to_key = BalanceKey::new(item.no.as_str(), to_wh.as_str(), &to_loc, &to_lot);
            if let Some(b) = self
                .r
                .balances
                .get_inventory_balance(plant, &to_key)
                .await
                .map_err(infra)?
                && b.expiry.is_some_and(|e| e < today())
            {
                return Err(DomainError::InventoryBalanceExpired.into());
            }
            let reason = self.reason(plant, d, l.reason.as_deref()).await?;
            let line_no = if l.line_no == 0 {
                WarehouseTransfer::next_line_no(&out)
            } else {
                l.line_no
            };
            out.push(TransferLine {
                line_no,
                item: item.no.as_str().to_owned(),
                from_warehouse: from_wh.as_str().to_owned(),
                from_location: from_loc,
                from_lot,
                to_warehouse: to_wh.as_str().to_owned(),
                to_location: to_loc,
                to_lot,
                unit: balance.unit.clone(),
                qty,
                reason,
            });
        }
        warehouse_transfer::check_lines(&out)?;
        Ok(out)
    }

    /// 倉庫須存在、有效（s_stkchk 'A'：mfg6076、mfg1100）
    async fn active_warehouse(
        &self,
        plant: &PlantId,
        code: &str,
    ) -> Result<WarehouseCode, AppError> {
        let code = WarehouseCode::parse(code.trim()).map_err(|_| DomainError::WarehouseNotFound)?;
        let w = self
            .r
            .warehouses
            .get_warehouse(plant, &code)
            .await
            .map_err(infra)?
            .ok_or(DomainError::WarehouseNotFound)?;
        if !w.active {
            return Err(DomainError::WarehouseInactive.into());
        }
        Ok(code)
    }

    /// 料件存放位置管制（sma42；s_imfchk1 mfg9036、s_imfchk mfg6095；WT5(b)）
    async fn check_storage_control(
        &self,
        plant: &PlantId,
        settings: &InventorySettings,
        item: &ItemNo,
        warehouse: &str,
        location: &str,
    ) -> Result<(), AppError> {
        let with_location = match settings.storage_control {
            StorageControl::Free => return Ok(()),
            StorageControl::Warehouse => false,
            StorageControl::WarehouseLocation => true,
        };
        let storages = self
            .r
            .item_storages
            .list_item_storages(plant, Some(item))
            .await
            .map_err(infra)?;
        let ok = storages.iter().any(|s| {
            s.warehouse.as_str() == warehouse
                && (!with_location || s.location.as_ref().map_or("", |x| x.as_str()) == location)
        });
        if ok {
            Ok(())
        } else {
            Err(DomainError::WarehouseTransferStorageNotAssigned.into())
        }
    }

    /// 理由碼（imn28；WT7）：分類碼類別 2 且有效；單別預算控管時必填
    async fn reason(
        &self,
        plant: &PlantId,
        d: &DocumentType,
        raw: Option<&str>,
    ) -> Result<Option<String>, AppError> {
        match raw.map(str::trim).filter(|r| !r.is_empty()) {
            Some(r) => {
                let c =
                    Code::parse(r).map_err(|_| DomainError::InventoryTransactionReasonNotFound)?;
                let ok = self
                    .r
                    .codes
                    .get_code(plant, CodeKind::Reason, &c)
                    .await
                    .map_err(infra)?
                    .is_some_and(|x| x.active);
                if !ok {
                    return Err(DomainError::InventoryTransactionReasonNotFound.into());
                }
                Ok(Some(r.to_owned()))
            }
            None if d.attrs.budget_control => {
                Err(DomainError::InventoryTransactionReasonRequired.into())
            }
            None => Ok(None),
        }
    }

    /// 單頭（t324_a、t324_i；WT3）
    async fn header(
        &self,
        plant: &PlantId,
        actor: &Actor,
        input: &WarehouseTransferInput,
        base: Option<&TransferHeader>,
    ) -> Result<TransferHeader, AppError> {
        let transfer_date = input
            .transfer_date
            .or(base.map(|b| b.transfer_date))
            .unwrap_or_else(today);
        let applicant = match (&input.applicant, base) {
            (Some(a), _) => Some(a.trim().to_owned()).filter(|a| !a.is_empty()),
            (None, Some(b)) => b.applicant.clone(),
            (None, None) => match EmployeeId::parse(&actor.user_id) {
                Ok(id) => self
                    .r
                    .employees
                    .get(plant, &id)
                    .await
                    .map_err(infra)?
                    .filter(|e| e.active)
                    .map(|e| e.id.as_str().to_owned()),
                Err(_) => None,
            },
        };
        let mut applicant_dept = None;
        if let Some(a) = &applicant {
            let id = EmployeeId::parse(a).map_err(|_| DomainError::EmployeeNotFound)?;
            let e = self
                .r
                .employees
                .get(plant, &id)
                .await
                .map_err(infra)?
                .ok_or(DomainError::EmployeeNotFound)?;
            if !e.active {
                return Err(DomainError::InventoryTransactionApplicantInactive.into());
            }
            applicant_dept = e.attrs.dept_code.clone();
        }
        let dept_raw = match (&input.dept, base) {
            (Some(d), _) => Some(d.trim().to_owned()),
            (None, Some(b)) => b.dept.as_ref().map(|d| d.as_str().to_owned()),
            (None, None) => applicant_dept
                .map(|d| d.as_str().to_owned())
                .or_else(|| Some(actor.dept.clone())),
        }
        .filter(|d| !d.is_empty());
        let dept = match dept_raw {
            Some(code) => {
                let code = DeptCode::parse(&code).map_err(|_| DomainError::DepartmentNotFound)?;
                let d = self
                    .r
                    .depts
                    .get(plant, &code)
                    .await
                    .map_err(infra)?
                    .ok_or(DomainError::DepartmentNotFound)?;
                if !d.active {
                    return Err(DomainError::DepartmentInactive.into());
                }
                Some(code)
            }
            None => None,
        };
        let memo = input
            .memo
            .clone()
            .or_else(|| base.and_then(|b| b.memo.clone()));
        Ok(TransferHeader {
            transfer_date,
            dept,
            applicant,
            memo,
        }
        .validated()?)
    }

    async fn settings(&self, plant: &PlantId) -> Result<InventorySettings, AppError> {
        Ok(self
            .r
            .inventory_params
            .inventory_parameters(plant)
            .await
            .map_err(infra)?
            .map(|p| p.settings)
            .unwrap_or_default())
    }

    /// 期間檢查（PT3）；`full` 為否時只檢查成本關帳日（作廢 mfg9084、過帳還原）
    async fn check_period(
        &self,
        plant: &PlantId,
        date: NaiveDate,
        full: bool,
    ) -> Result<(), AppError> {
        let settings = self.settings(plant).await?;
        let day = if full && settings.needs_period() {
            let years = self
                .r
                .periods
                .list_accounting_years()
                .await
                .map_err(infra)?;
            Some(Calendar::new(&years).locate(date)?)
        } else {
            None
        };
        Ok(settings.check_posting_date(date, day.as_ref())?)
    }

    /// 單別檢查（s_check_no "4"；WT2）：存在、aim 調撥性質 4、有效、使用者／部門設限
    async fn doc_type(
        &self,
        plant: &PlantId,
        doc_no_or_code: &str,
        user: &str,
    ) -> Result<DocumentType, AppError> {
        let code = DocTypeCode::existing(doc_no::doc_type_part(doc_no_or_code.trim()))?;
        let d = self
            .r
            .docs
            .get_document_type(plant, &code)
            .await
            .map_err(infra)?
            .filter(|d| d.attrs.system == "aim")
            .ok_or(DomainError::DocumentTypeNotFound)?;
        let dept = match UserId::parse(user) {
            Ok(id) => self
                .r
                .users
                .get(&id)
                .await
                .map_err(infra)?
                .and_then(|u| u.attrs.dept_code),
            Err(_) => None,
        };
        d.check_use(warehouse_transfer::DOC_KIND, user, dept.as_deref())?;
        Ok(d)
    }

    async fn posting_doc_type(&self, plant: &PlantId, no: &str) -> Result<DocumentType, AppError> {
        let code = DocTypeCode::existing(doc_no::doc_type_part(no))?;
        self.r
            .docs
            .get_document_type(plant, &code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::DocumentTypeNotFound.into())
    }

    /// 單號（s_auto_assign_no，以調撥日編號；WT2）
    async fn assign_no(
        &self,
        plant: &PlantId,
        d: &DocumentType,
        input: &str,
        date: NaiveDate,
    ) -> Result<String, AppError> {
        let serial = self
            .r
            .params
            .general_parameters()
            .await
            .map_err(infra)?
            .map_or(8, |p| p.doc_serial_length);
        let input = input.trim();
        if input != d.code.as_str() {
            return Ok(doc_no::check_manual(input, &d.code, serial)?);
        }
        if !d.attrs.auto_number {
            return Err(DomainError::DocNoInvalid { len: serial }.into());
        }
        let years = self
            .r
            .periods
            .list_accounting_years()
            .await
            .map_err(infra)?;
        let day = Calendar::new(&years).locate(date)?;
        let (prefix, seq) = doc_no::prefix(&d.code, &d.attrs.numbering, &day, serial);
        let max = self
            .r
            .transfers
            .max_transfer_no(plant, &prefix)
            .await
            .map_err(infra)?;
        Ok(doc_no::next(&prefix, seq, max.as_deref())?)
    }

    async fn insert(
        &self,
        plant: &PlantId,
        d: &DocumentType,
        input_no: &str,
        mut t: WarehouseTransfer,
    ) -> Result<WarehouseTransfer, AppError> {
        for _ in 0..5 {
            match self.r.transfers.insert_warehouse_transfer(plant, &t).await {
                Ok(()) => return self.load(plant, &t.no).await,
                Err(RepoError::Conflict) if input_no.trim() == d.code.as_str() => {
                    t.no = self
                        .assign_no(plant, d, input_no, t.header.transfer_date)
                        .await?;
                }
                Err(RepoError::Conflict) => {
                    return Err(DomainError::InventoryTransactionAlreadyExists.into());
                }
                Err(e) => return Err(infra(e)),
            }
        }
        Err(DomainError::InventoryTransactionAlreadyExists.into())
    }

    async fn save(
        &self,
        plant: &PlantId,
        t: &WarehouseTransfer,
    ) -> Result<WarehouseTransfer, AppError> {
        self.r
            .transfers
            .update_warehouse_transfer(plant, t)
            .await
            .map_err(infra)?;
        self.load(plant, &t.no).await
    }

    async fn load(&self, plant: &PlantId, no: &str) -> Result<WarehouseTransfer, AppError> {
        self.r
            .transfers
            .get_warehouse_transfer(plant, no.trim())
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::InventoryTransactionNotFound.into())
    }

    async fn sorted(&self, plant: &PlantId) -> Result<Vec<WarehouseTransfer>, AppError> {
        let mut all = self
            .r
            .transfers
            .list_warehouse_transfers(plant)
            .await
            .map_err(infra)?;
        all.sort_by(|a, b| a.no.cmp(&b.no));
        Ok(all)
    }

    /// 尚未暫存時讀取一筆須已存在的庫存明細
    async fn existing_balance(
        &self,
        plant: &PlantId,
        balances: &mut Vec<(Option<Decimal>, InventoryBalance)>,
        key: &BalanceKey,
    ) -> Result<usize, AppError> {
        if let Some(i) = balances.iter().position(|(_, b)| b.key == *key) {
            return Ok(i);
        }
        let b = self
            .r
            .balances
            .get_inventory_balance(plant, key)
            .await
            .map_err(infra)?
            .ok_or(DomainError::InventoryBalanceNotFound)?;
        balances.push((Some(b.qty), b));
        Ok(balances.len() - 1)
    }

    /// 撥入新建的庫存明細（s_add_img、s_upimg；PR1）
    async fn new_storage(
        &self,
        plant: &PlantId,
        key: BalanceKey,
        l: &TransferLine,
        no: &str,
        date: NaiveDate,
    ) -> Result<InventoryBalance, AppError> {
        let item_no = ItemNo::parse(&l.item).map_err(|_| DomainError::ItemNotFound)?;
        let item = self
            .r
            .items
            .get_item(plant, &item_no)
            .await
            .map_err(infra)?
            .ok_or(DomainError::ItemNotFound)?;
        let wh =
            WarehouseCode::parse(&l.to_warehouse).map_err(|_| DomainError::WarehouseNotFound)?;
        let warehouse = self
            .r
            .warehouses
            .get_warehouse(plant, &wh)
            .await
            .map_err(infra)?
            .ok_or(DomainError::WarehouseNotFound)?;
        let flags = warehouse
            .locations
            .iter()
            .find(|x| x.code.as_str() == l.to_location)
            .map(|x| x.flags.clone())
            .unwrap_or_else(|| warehouse.attrs.flags.clone());
        Ok(ttrs_domain::inventory_balance::new_balance(
            key,
            &l.unit,
            date,
            item.attrs.shelf_life_days,
            flags.usable,
            flags.mrp_usable,
            flags.bonded,
            no,
            l.line_no,
        ))
    }

    /// 撥入的最高存量（s_hqty；PR2）
    async fn check_max_qty(
        &self,
        plant: &PlantId,
        key: &BalanceKey,
        pending: &[(Option<Decimal>, InventoryBalance)],
    ) -> Result<(), AppError> {
        let item_no = ItemNo::parse(&key.item).map_err(|_| DomainError::ItemNotFound)?;
        let max = self
            .r
            .item_storages
            .list_item_storages(plant, Some(&item_no))
            .await
            .map_err(infra)?
            .into_iter()
            .find(|s| {
                s.warehouse.as_str() == key.warehouse
                    && s.location.as_ref().map_or("", |x| x.as_str()) == key.location
            })
            .and_then(|s| s.max_qty);
        let Some(max) = max else {
            return Ok(());
        };
        let stored = self
            .r
            .balances
            .list_inventory_balances(plant, &key.item, &key.warehouse, &key.location)
            .await
            .map_err(infra)?;
        let same = |b: &InventoryBalance| {
            b.key.item == key.item
                && b.key.warehouse == key.warehouse
                && b.key.location == key.location
        };
        let mut total: Decimal = stored
            .iter()
            .filter(|b| !pending.iter().any(|(_, p)| p.key == b.key))
            .map(|b| b.qty)
            .sum();
        total += pending
            .iter()
            .filter(|(_, b)| same(b))
            .map(|(_, b)| b.qty)
            .sum::<Decimal>();
        if total > max {
            return Err(DomainError::InventoryMaxQtyExceeded.into());
        }
        Ok(())
    }

    async fn cost_group(
        &self,
        plant: &PlantId,
        warehouse: &str,
    ) -> Result<Option<String>, AppError> {
        let wh = WarehouseCode::parse(warehouse).map_err(|_| DomainError::WarehouseNotFound)?;
        Ok(self
            .r
            .warehouses
            .get_warehouse(plant, &wh)
            .await
            .map_err(infra)?
            .and_then(|w| w.attrs.cost_group))
    }

    /// 料件的最近出庫、入庫、異動日（s_udima 撥出 -1、撥入 +1；WT9(c)）
    async fn item_dates(
        &self,
        plant: &PlantId,
        item: &str,
        date: NaiveDate,
    ) -> Result<ItemDates, AppError> {
        let no = ItemNo::parse(item).map_err(|_| DomainError::ItemNotFound)?;
        let cur = self
            .r
            .items
            .get_item(plant, &no)
            .await
            .map_err(infra)?
            .ok_or(DomainError::ItemNotFound)?;
        let later = cur.system.last_txn_on.is_none_or(|d| d < date);
        Ok(ItemDates {
            item: item.to_owned(),
            last_received_on: Some(date),
            last_issued_on: Some(date),
            last_txn_on: later.then_some(date),
        })
    }

    async fn apply(&self, plant: &PlantId, w: &PostingWrite) -> Result<(), AppError> {
        self.r
            .posting
            .apply_posting(plant, w)
            .await
            .map_err(|e| match e {
                RepoError::Conflict => DomainError::InventoryTransactionPosted.into(),
                e => infra(e),
            })
    }

    async fn item_name(&self, plant: &PlantId, no: &str) -> Result<Option<String>, AppError> {
        let Ok(no) = ItemNo::parse(no) else {
            return Ok(None);
        };
        Ok(self
            .r
            .items
            .get_item(plant, &no)
            .await
            .map_err(infra)?
            .and_then(|i| i.attrs.name))
    }

    async fn dto(
        &self,
        plant: &PlantId,
        t: &WarehouseTransfer,
    ) -> Result<WarehouseTransferDto, AppError> {
        let mut lines = Vec::with_capacity(t.lines.len());
        for l in &t.lines {
            lines.push(WarehouseTransferLineDto {
                line_no: l.line_no,
                item_no: l.item.clone(),
                item_name: self.item_name(plant, &l.item).await?,
                from_warehouse: l.from_warehouse.clone(),
                from_location: l.from_location.clone(),
                from_lot: l.from_lot.clone(),
                to_warehouse: l.to_warehouse.clone(),
                to_location: l.to_location.clone(),
                to_lot: l.to_lot.clone(),
                unit: l.unit.clone(),
                qty: l.qty.normalize(),
                reason: l.reason.clone(),
            });
        }
        let h = &t.header;
        let print_now = match DocTypeCode::existing(doc_no::doc_type_part(&t.no)) {
            Ok(code) => self
                .r
                .docs
                .get_document_type(plant, &code)
                .await
                .map_err(infra)?
                .is_some_and(|d| d.attrs.print_now),
            Err(_) => false,
        };
        Ok(WarehouseTransferDto {
            doc_no: t.no.clone(),
            transfer_date: h.transfer_date,
            dept: h.dept.as_ref().map(|d| d.as_str().to_owned()),
            applicant: h.applicant.clone(),
            memo: h.memo.clone(),
            status: t.status.code().to_owned(),
            posted: t.posted,
            lines,
            print_now,
            auto_confirm_error: None,
            created_by: t.created_by.clone(),
            created_dept: t.created_dept.clone(),
            created_on: t.created_on,
            updated_by: t.updated_by.clone(),
            updated_on: t.updated_on,
            version: t.stamp.version,
            created_at: t.stamp.created_at,
            updated_at: t.stamp.updated_at,
        })
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.r.authz.authorize(actor, PROGRAM, action).await
    }
}

fn infra(e: RepoError) -> AppError {
    match e {
        RepoError::Stale => DomainError::RecordChanged.into(),
        e => AppError::Infra(e.to_string()),
    }
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}
