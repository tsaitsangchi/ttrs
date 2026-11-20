//! Misc Issue 雜項發料（TipTop aimt301 → saimt370 參數 1 / ina_file、inb_file → misc_issue、misc_issue_line）
//! 規格：specs/features/aim/misc_issue.feature（MI-1～MI-19；確認事項 MT1–MT18）。寫入一律經 change_log::run（ADR 0012）。
//!
//! 與雜項收料、雜項報廢各自獨立（Key User 2026-11-20）。本作業特有的規則：
//! 須已有庫存資料（mfg6069）、不可超過庫存與可用量（mfg1303、客製 aim-425 扣除其他未過帳雜發）、檢查有效日期（aim-400）、
//! 部門依庫存參數「雜發須輸入部門」（sma847 第 1 碼，aim-330）、確認時須仍有庫存資料（asf-507）。

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
    inventory_transaction::{self, InventoryTransaction, TxnHeader, TxnKind, TxnLine},
    item::{ActiveCode, ItemNo},
    plant::PlantId,
    user::UserId,
    warehouse::{StorageClass, WarehouseCode},
};
use ttrs_shared::{
    ChangeDto, InventoryTxnCopyInput, InventoryTxnDto, InventoryTxnInput, InventoryTxnLineDto,
    InventoryTxnLineInput, Page, QbeCondition, QueryRequest,
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
        ItemDates, ItemRepository, PostingWrite, SystemParameterRepository, UserRepository,
        WarehouseRepository,
    },
    query,
    report::{self, ReportData, ReportFormat},
};

pub const PROGRAM: &str = "misc-issue";
const KIND: TxnKind = TxnKind::Issue;
const HEAD_TABLE: &str = "misc_issue";
const LINE_TABLE: &str = "misc_issue_line";

const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "misc-issue-report-title",
    columns: &[
        ("doc_no", "misc-col-doc-no"),
        ("posting_date", "misc-col-posting-date"),
        ("dept", "misc-col-dept"),
        ("applicant", "misc-col-applicant"),
        ("status", "misc-col-status"),
        ("line_no", "misc-col-line-no"),
        ("item_no", "misc-col-item-no"),
        ("item_name", "misc-col-item-name"),
        ("warehouse", "misc-col-warehouse"),
        ("location", "misc-col-location"),
        ("lot", "misc-col-lot"),
        ("unit", "misc-col-unit"),
        ("qty", "misc-col-qty"),
        ("reason", "misc-col-reason"),
    ],
};

/// 本作業用到的資料存取
#[derive(Clone)]
pub struct MiscIssueRepos {
    pub authz: Arc<dyn Authorizer>,
    pub txns: Arc<dyn InventoryTransactionRepository>,
    pub balances: Arc<dyn InventoryBalanceRepository>,
    pub docs: Arc<dyn DocumentTypeRepository>,
    pub params: Arc<dyn SystemParameterRepository>,
    pub inventory_params: Arc<dyn InventoryParameterRepository>,
    pub periods: Arc<dyn AccountingPeriodRepository>,
    pub users: Arc<dyn UserRepository>,
    pub depts: Arc<dyn DepartmentRepository>,
    pub employees: Arc<dyn EmployeeRepository>,
    pub items: Arc<dyn ItemRepository>,
    pub warehouses: Arc<dyn WarehouseRepository>,
    pub codes: Arc<dyn ClassificationCodeRepository>,
    /// 過帳（PT9：單據、庫存明細、異動記錄、料件日期同一交易）
    pub posting: Arc<dyn InventoryPostingRepository>,
    pub history: HistoryReader,
    pub reports: ListReporter,
}

#[derive(Clone)]
pub struct MiscIssueService {
    r: MiscIssueRepos,
}

impl MiscIssueService {
    pub fn new(r: MiscIssueRepos) -> Self {
        Self { r }
    }

    /// QBE 查詢（t370_cs）；依單號排序
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<InventoryTxnDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let filters = query::inventory_transaction::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.sorted(&plant).await?;
        let (rows, total) =
            query::page_in_memory(&all, &filters, query::inventory_transaction::value, p);
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
    ) -> Result<InventoryTxnDto, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let t = self.load(&plant, no).await?;
        self.dto(&plant, &t).await
    }

    /// 新增（insert；t370_a）：單頭與單身一起存（MT10）；單別「立即確認」時有單身就自動確認
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        input: InventoryTxnInput,
    ) -> Result<InventoryTxnDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let plant = PlantId::parse(plant)?;
            let d = self.doc_type(&plant, &input.doc_no, &actor.user_id).await?;
            let header = self.header(&plant, actor, &input, None).await?;
            let lines = self
                .lines(
                    &plant,
                    &d,
                    &header,
                    input.lines.as_deref().unwrap_or_default(),
                    "",
                )
                .await?;
            let no = self
                .assign_no(&plant, &d, &input.doc_no, header.input_date)
                .await?;
            let t = InventoryTransaction::new(
                no,
                KIND,
                header,
                lines,
                &actor.user_id,
                &actor.dept,
                today(),
            )?;
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

    /// 修改單頭與單身（modify、detail；t370_u、t370_b）：已確認 9023、已作廢 9024 不可
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        input: InventoryTxnInput,
    ) -> Result<InventoryTxnDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            // 改單頭檢查 modify（t370_u）、改單身檢查 detail（t370_b）
            let head = input.input_date.is_some()
                || input.posting_date.is_some()
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
            let lines = match &input.lines {
                Some(l) => self.lines(&plant, &d, &header, l, &t.no).await?,
                None => t.lines.clone(),
            };
            t.update(header, lines, &actor.user_id, today())?;
            let t = self.save(&plant, &t).await?;
            self.dto(&plant, &t).await
        })
        .await
    }

    /// 刪除（delete；t370_r）：已確認、已作廢不可；連同單身
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
                .txns
                .delete_inventory_transaction(&plant, KIND, &t.no)
                .await
                .map_err(infra)
        })
        .await
    }

    /// 作廢／取消作廢（void；t370_x）：已確認、已過帳不可
    pub async fn set_void(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        void: bool,
        version: Option<i32>,
    ) -> Result<InventoryTxnDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "void", async {
            let grant = self.authorize(actor, "void").await?;
            let plant = PlantId::parse(plant)?;
            let mut t = self.load(&plant, no).await?;
            grant.record(&t.created_by, &t.created_dept)?;
            t.stamp.check(version)?;
            crate::change_log::set_operation(if void { "void" } else { "unvoid" });
            t.set_void(void, &actor.user_id, today())?;
            let t = self.save(&plant, &t).await?;
            self.dto(&plant, &t).await
        })
        .await
    }

    /// 確認（confirm；t370_y_chk、t370_y_upd）／取消確認（undo_confirm；t370_z）
    pub async fn confirm(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<InventoryTxnDto, AppError> {
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

    /// 複製（reproduce；t370_copy，MT11）：新單別或完整單號、輸入日、扣帳日；單身一併複製、不重檢
    pub async fn copy(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        input: InventoryTxnCopyInput,
    ) -> Result<InventoryTxnDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let plant = PlantId::parse(plant)?;
            let source = self.load(&plant, no).await?;
            let d = self.doc_type(&plant, &input.doc_no, &actor.user_id).await?;
            let input_date = input.input_date.unwrap_or_else(today);
            let posting_date = input.posting_date.unwrap_or(input_date);
            let new_no = self
                .assign_no(&plant, &d, &input.doc_no, input_date)
                .await?;
            let t = source.copy_as(
                new_no,
                input_date,
                posting_date,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            let t = self.insert(&plant, &d, &input.doc_no, t).await?;
            self.dto(&plant, &t).await
        })
        .await
    }

    /// 異動記錄：單頭與單身（ADR 0012 §8；MT18）
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

    /// 列印 output、匯出 Excel exporttoexcel（MT12）：每筆單身一列
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
        let filters = query::inventory_transaction::filters(&conditions)?;
        let all = self.sorted(&plant).await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };
        let (rows, _) =
            query::page_in_memory(&all, &filters, query::inventory_transaction::value, p);
        let t = |id: &str| report::text(lang, id);
        let mut lines: Vec<Vec<String>> = Vec::new();
        for x in &rows {
            let h = &x.header;
            let status = t(&format!("misc-status-{}", x.status.code().to_lowercase()));
            for l in &x.lines {
                let name = self.item_text(&plant, &l.item).await?.0;
                lines.push(vec![
                    x.no.clone(),
                    h.posting_date.to_string(),
                    h.dept
                        .as_ref()
                        .map(|d| d.as_str().to_owned())
                        .unwrap_or_default(),
                    h.applicant.clone().unwrap_or_default(),
                    status.clone(),
                    l.line_no.to_string(),
                    l.item.clone(),
                    name.unwrap_or_default(),
                    l.warehouse.clone(),
                    l.location.clone(),
                    l.lot.clone(),
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

    /// 庫存過帳（stock_post；t370_s_chk、t370_s_upd、t370_s1；misc_issue_posting.feature PT2–PT9）：
    /// 可改扣帳日（預設單據扣帳日）；期間檢查（PT3）；逐筆扣庫存（雜發改最近發料日）並寫異動記錄，任何一筆失敗整張不過帳
    pub async fn post(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        posting_date: Option<NaiveDate>,
        version: Option<i32>,
    ) -> Result<InventoryTxnDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "stock_post", async {
            self.authorize(actor, "stock_post").await?;
            crate::change_log::set_operation("stock_post");
            let plant = PlantId::parse(plant)?;
            let mut t = self.load(&plant, no).await?;
            t.stamp.check(version)?;
            let date = posting_date.unwrap_or(t.header.posting_date);
            self.check_period(&plant, date, true).await?;
            t.post(date, &actor.user_id, today())?;
            let d = self.posting_doc_type(&plant, &t.no).await?;
            let mut balances: Vec<(Option<Decimal>, InventoryBalance)> = Vec::new();
            let mut records = Vec::new();
            let mut items: Vec<ItemDates> = Vec::new();
            for l in &t.lines {
                // 寫異動記錄時再檢查單別倉庫設限（s_tlf → s_chksmz，PT7）
                d.check_storage(
                    &l.warehouse,
                    Some(l.location.as_str()).filter(|x| !x.is_empty()),
                )?;
                let key = BalanceKey::new(&l.item, &l.warehouse, &l.location, &l.lot);
                let i = match balances.iter().position(|(_, b)| b.key == key) {
                    Some(i) => i,
                    None => {
                        let b = self
                            .r
                            .balances
                            .get_inventory_balance(&plant, &key)
                            .await
                            .map_err(infra)?
                            .ok_or(DomainError::InventoryBalanceNotFound)?;
                        balances.push((Some(b.qty), b));
                        balances.len() - 1
                    }
                };
                balances[i].1.take(l.qty, date, true)?;
                let balance_after = balances[i].1.qty;
                let wh = WarehouseCode::parse(&l.warehouse)
                    .map_err(|_| DomainError::WarehouseNotFound)?;
                let cost_group = self
                    .r
                    .warehouses
                    .get_warehouse(&plant, &wh)
                    .await
                    .map_err(infra)?
                    .and_then(|w| w.attrs.cost_group);
                records.push(InventoryTxnRecord {
                    key,
                    doc_no: t.no.clone(),
                    doc_line: l.line_no,
                    doc_type: d.code.as_str().to_owned(),
                    direction: -1,
                    source: 50,
                    dest: 90,
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
                });
                if !items.iter().any(|x| x.item == l.item) {
                    items.push(self.item_dates(&plant, &l.item, date).await?);
                }
            }
            let w = PostingWrite {
                doc: t.clone(),
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

    /// 過帳還原（undo_post；aimp379，PT8）：須已過帳、未作廢；扣帳日不可小於等於成本關帳日；
    /// 庫存加回、刪除這張單的異動記錄，單據回到已確認、未過帳
    pub async fn undo_post(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        version: Option<i32>,
    ) -> Result<InventoryTxnDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "undo_post", async {
            self.authorize(actor, "undo_post").await?;
            crate::change_log::set_operation("undo_post");
            let plant = PlantId::parse(plant)?;
            let mut t = self.load(&plant, no).await?;
            t.stamp.check(version)?;
            t.unpost(&actor.user_id, today())?;
            // aimp379 只檢查成本關帳日（mfg9999）
            self.check_period(&plant, t.header.posting_date, false)
                .await?;
            let mut balances: Vec<(Option<Decimal>, InventoryBalance)> = Vec::new();
            for l in &t.lines {
                let key = BalanceKey::new(&l.item, &l.warehouse, &l.location, &l.lot);
                let i = match balances.iter().position(|(_, b)| b.key == key) {
                    Some(i) => i,
                    None => {
                        let b = self
                            .r
                            .balances
                            .get_inventory_balance(&plant, &key)
                            .await
                            .map_err(infra)?
                            .ok_or(DomainError::InventoryBalanceNotFound)?;
                        balances.push((Some(b.qty), b));
                        balances.len() - 1
                    }
                };
                balances[i].1.undo(l.qty)?;
            }
            let w = PostingWrite {
                doc: t.clone(),
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

    /// 本單的庫存異動記錄（tlf_file；PT6），依項次排序
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

    // ----- 內部 -----

    /// 扣帳日的期間檢查（PT3）；`full` 為否時只檢查成本關帳日（過帳還原）
    async fn check_period(
        &self,
        plant: &PlantId,
        date: NaiveDate,
        full: bool,
    ) -> Result<(), AppError> {
        let settings = self
            .r
            .inventory_params
            .inventory_parameters(plant)
            .await
            .map_err(infra)?
            .map(|p| p.settings)
            .unwrap_or_default();
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

    /// 過帳時的單別（只讀，不再檢查使用者設限；PT7 的倉庫設限在逐筆時檢查）
    async fn posting_doc_type(&self, plant: &PlantId, no: &str) -> Result<DocumentType, AppError> {
        let code = DocTypeCode::existing(doc_no::doc_type_part(no))?;
        self.r
            .docs
            .get_document_type(plant, &code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::DocumentTypeNotFound.into())
    }

    /// 料件的最近異動日期（s_udima；PT5）
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
            last_issued_on: Some(date),
            last_txn_on: later.then_some(date),
            ..ItemDates::default()
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

    /// 確認（t370_y_chk）：狀態、須有單身；每一筆的倉儲批須仍有庫存資料（asf-507，MT8(d)）
    async fn confirm_doc(
        &self,
        plant: &PlantId,
        mut t: InventoryTransaction,
        actor: &Actor,
    ) -> Result<InventoryTransaction, AppError> {
        t.confirm(&actor.user_id, today())?;
        for l in &t.lines {
            let key = BalanceKey::new(&l.item, &l.warehouse, &l.location, &l.lot);
            if self
                .r
                .balances
                .get_inventory_balance(plant, &key)
                .await
                .map_err(infra)?
                .is_none()
            {
                return Err(DomainError::InventoryBalanceNotFound.into());
            }
        }
        self.save(plant, &t).await
    }

    /// 單身（t370_b）：共同檢查之後，雜發須有庫存資料、未過有效日期、不超過庫存與可用量（MT8(a)(c)、aim-425）
    async fn lines(
        &self,
        plant: &PlantId,
        d: &DocumentType,
        header: &TxnHeader,
        input: &[InventoryTxnLineInput],
        exclude_no: &str,
    ) -> Result<Vec<TxnLine>, AppError> {
        let mut out: Vec<TxnLine> = Vec::with_capacity(input.len());
        for l in input {
            let qty = inventory_transaction::check_qty(l.qty)?;
            let line = self.line(plant, d, l, &out).await?;
            let key = BalanceKey::new(&line.item, &line.warehouse, &line.location, &line.lot);
            let balance = self
                .r
                .balances
                .get_inventory_balance(plant, &key)
                .await
                .map_err(infra)?
                .ok_or(DomainError::InventoryBalanceNotFound)?;
            if balance.expiry.is_some_and(|e| e < header.posting_date) {
                return Err(DomainError::InventoryBalanceExpired.into());
            }
            let pending = self
                .r
                .txns
                .pending_issue_qty(plant, &key, exclude_no)
                .await
                .map_err(infra)?;
            if qty > balance.qty || qty + pending > balance.qty {
                return Err(DomainError::InventoryInsufficient.into());
            }
            out.push(TxnLine {
                unit: balance.unit.clone(),
                qty,
                ..line
            });
        }
        inventory_transaction::check_lines(&out)?;
        Ok(out)
    }

    /// 單身一列的共同檢查（t370_check_inb04、t370_chk_inb05、s_chksmz、t370_chk_inb15；MT5、MT7）：
    /// 料件存在且已確認、不是 MISC；倉庫存在、有效、一般倉；單別倉庫設限；理由碼。
    /// 倉庫空白時帶料件的主要倉庫、儲位（ima35、ima36）；單位先帶料件的庫存單位
    async fn line(
        &self,
        plant: &PlantId,
        d: &DocumentType,
        l: &InventoryTxnLineInput,
        done: &[TxnLine],
    ) -> Result<TxnLine, AppError> {
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
        let given = l
            .warehouse
            .as_deref()
            .map(str::trim)
            .filter(|w| !w.is_empty());
        let (wh, loc) = match given {
            Some(w) => (w.to_owned(), l.location.clone().unwrap_or_default()),
            None => (
                item.attrs.warehouse.clone().unwrap_or_default(),
                l.location
                    .clone()
                    .or_else(|| item.attrs.location.clone())
                    .unwrap_or_default(),
            ),
        };
        let code = WarehouseCode::parse(&wh).map_err(|_| DomainError::WarehouseNotFound)?;
        let warehouse = self
            .r
            .warehouses
            .get_warehouse(plant, &code)
            .await
            .map_err(infra)?
            .ok_or(DomainError::WarehouseNotFound)?;
        if !warehouse.active {
            return Err(DomainError::WarehouseInactive.into());
        }
        if warehouse.attrs.flags.class != StorageClass::Store {
            return Err(DomainError::InventoryTransactionWarehouseClass.into());
        }
        let loc = loc.trim().to_owned();
        let lot = l.lot.clone().unwrap_or_default().trim().to_owned();
        inventory_transaction::check_storage_text(&loc, &lot)?;
        d.check_storage(code.as_str(), Some(loc.as_str()).filter(|x| !x.is_empty()))?;
        let reason = match l.reason.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
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
                Some(r.to_owned())
            }
            None if d.attrs.budget_control => {
                return Err(DomainError::InventoryTransactionReasonRequired.into());
            }
            None => None,
        };
        let line_no = if l.line_no == 0 {
            InventoryTransaction::next_line_no(done)
        } else {
            l.line_no
        };
        Ok(TxnLine {
            line_no,
            item: item.no.as_str().to_owned(),
            warehouse: code.as_str().to_owned(),
            location: loc,
            lot,
            unit: item.attrs.stock_unit.clone().unwrap_or_default(),
            qty: l.qty,
            reason,
        })
    }

    /// 單頭（t370_a_default、t370_i；MT6）：申請人須為有效員工；部門依「雜發須輸入部門」（aim-330）
    async fn header(
        &self,
        plant: &PlantId,
        actor: &Actor,
        input: &InventoryTxnInput,
        base: Option<&TxnHeader>,
    ) -> Result<TxnHeader, AppError> {
        let input_date = input
            .input_date
            .or(base.map(|b| b.input_date))
            .unwrap_or_else(today);
        let posting_date = input
            .posting_date
            .or(base.map(|b| b.posting_date))
            .unwrap_or(input_date);
        // 申請人：未給時新增帶與登入使用者同代號的員工（MT6(b)）
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
        // 部門：新增時未給 → 申請人的部門，沒有時登入部門（t370_ina11 'a'）；修改時沿用
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
            None => {
                let required = self
                    .r
                    .inventory_params
                    .inventory_parameters(plant)
                    .await
                    .map_err(infra)?
                    .map(|p| p.settings)
                    .unwrap_or_default()
                    .misc_issue_dept;
                if required {
                    return Err(DomainError::InventoryTransactionDeptRequired.into());
                }
                None
            }
        };
        let memo = input
            .memo
            .clone()
            .or_else(|| base.and_then(|b| b.memo.clone()));
        Ok(TxnHeader {
            input_date,
            posting_date,
            dept,
            applicant,
            memo,
        }
        .validated()?)
    }

    /// 單別檢查（s_check_no → s_mfgslip；MT5）：存在、aim 雜發性質 1、有效、使用者／部門設限
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
        d.check_use(KIND.doc_kind(), user, dept.as_deref())?;
        Ok(d)
    }

    /// 單號（s_auto_assign_no；MT4）：只給單別時自動編號，給完整單號時檢查格式；不自動編號的單別須給完整單號
    async fn assign_no(
        &self,
        plant: &PlantId,
        d: &DocumentType,
        input: &str,
        input_date: NaiveDate,
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
        let day = Calendar::new(&years).locate(input_date)?;
        let (prefix, seq) = doc_no::prefix(&d.code, &d.attrs.numbering, &day, serial);
        let max = self
            .r
            .txns
            .max_doc_no(plant, KIND, &prefix)
            .await
            .map_err(infra)?;
        Ok(doc_no::next(&prefix, seq, max.as_deref())?)
    }

    /// 新增；自動編號撞號時重新取號（TipTop 鎖單別列避免重號，MT4(c)）
    async fn insert(
        &self,
        plant: &PlantId,
        d: &DocumentType,
        input_no: &str,
        mut t: InventoryTransaction,
    ) -> Result<InventoryTransaction, AppError> {
        for _ in 0..5 {
            match self.r.txns.insert_inventory_transaction(plant, &t).await {
                Ok(()) => return self.load(plant, &t.no).await,
                Err(RepoError::Conflict) if input_no.trim() == d.code.as_str() => {
                    t.no = self
                        .assign_no(plant, d, input_no, t.header.input_date)
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
        t: &InventoryTransaction,
    ) -> Result<InventoryTransaction, AppError> {
        self.r
            .txns
            .update_inventory_transaction(plant, t)
            .await
            .map_err(infra)?;
        self.load(plant, &t.no).await
    }

    async fn load(&self, plant: &PlantId, no: &str) -> Result<InventoryTransaction, AppError> {
        self.r
            .txns
            .get_inventory_transaction(plant, KIND, no.trim())
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::InventoryTransactionNotFound.into())
    }

    async fn sorted(&self, plant: &PlantId) -> Result<Vec<InventoryTransaction>, AppError> {
        let mut all = self
            .r
            .txns
            .list_inventory_transactions(plant, KIND)
            .await
            .map_err(infra)?;
        all.sort_by(|a, b| a.no.cmp(&b.no));
        Ok(all)
    }

    async fn item_text(
        &self,
        plant: &PlantId,
        no: &str,
    ) -> Result<(Option<String>, Option<String>), AppError> {
        let Ok(no) = ItemNo::parse(no) else {
            return Ok((None, None));
        };
        Ok(self
            .r
            .items
            .get_item(plant, &no)
            .await
            .map_err(infra)?
            .map(|i| (i.attrs.name, i.attrs.spec))
            .unwrap_or_default())
    }

    async fn dto(
        &self,
        plant: &PlantId,
        t: &InventoryTransaction,
    ) -> Result<InventoryTxnDto, AppError> {
        let mut lines = Vec::with_capacity(t.lines.len());
        for l in &t.lines {
            let (item_name, item_spec) = self.item_text(plant, &l.item).await?;
            lines.push(InventoryTxnLineDto {
                line_no: l.line_no,
                item_no: l.item.clone(),
                item_name,
                item_spec,
                warehouse: l.warehouse.clone(),
                location: l.location.clone(),
                lot: l.lot.clone(),
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
        Ok(InventoryTxnDto {
            doc_no: t.no.clone(),
            kind: KIND.slug().to_owned(),
            input_date: h.input_date,
            posting_date: h.posting_date,
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
