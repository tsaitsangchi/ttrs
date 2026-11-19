//! Item Maintenance 料件基本資料維護 use case（TipTop: aimi100 / ima_file，以客製 topcust/cim 為準）
//! 規格：specs/features/aim/item_maintenance.feature（IM1–IM31、情境 IM-1～IM-24）
//!
//! 工廠層（IM9）。功能代號照 aimi100（menu :7725-7958）：insert、query、modify、delete、invalid（作廢）、reproduce、
//! confirm、notconfirm、order_invalid（客製）、update_item（多語言品名）、history。修改、刪除、作廢依資料擁有者套用資料權限。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    classification::{Code, CodeKind},
    department::DeptCode,
    employee::EmployeeId,
    item::{Item, ItemAttrs, ItemNo, ItemText},
    item_group::ItemGroupCode,
    lang::Lang,
    plant::PlantId,
    product_class::ProductClassCode,
    unit::UnitCode,
};
use ttrs_shared::{
    ChangeDto, CopyItemRequest, ItemDto, ItemInput, ItemTextDto, Page, QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    change_log::HistoryReader,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{
        ClassificationCodeRepository, DepartmentRepository, EmployeeRepository,
        ItemGroupRepository, ItemRepository, MasterDataRepository, ProductClassRepository,
        UnitRepository,
    },
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "item-maintenance";

/// 其他分群碼一～四對應的分類碼類別（mfg1306）
const OTHER_GROUP_KINDS: [&str; 4] = ["D", "E", "F", "G"];

/// 清單報表（ADR 0013 SA3；item_maintenance_report.feature）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "item-report-title",
    columns: &[
        ("item_no", "item-col-item-no"),
        ("name", "item-col-name"),
        ("spec", "item-col-spec"),
        ("group_code", "item-col-group"),
        ("source_code", "item-col-source"),
        ("stock_unit", "item-col-stock-unit"),
        ("product_class", "item-col-product-class"),
        ("active_code", "item-col-active-code"),
    ],
};

/// 料件 use case 需要的資料存取
#[derive(Clone)]
pub struct ItemRepos {
    pub items: Arc<dyn ItemRepository>,
    pub groups: Arc<dyn ItemGroupRepository>,
    pub units: Arc<dyn UnitRepository>,
    pub classes: Arc<dyn ProductClassRepository>,
    pub codes: Arc<dyn ClassificationCodeRepository>,
    pub employees: Arc<dyn EmployeeRepository>,
    pub departments: Arc<dyn DepartmentRepository>,
    pub master: Arc<dyn MasterDataRepository>,
    /// 資料控管原則的欄位條件（s_field_chk，data_control.feature DC7）
    pub data_control: Arc<dyn crate::ports::DataControlRepository>,
    /// 主要倉庫、儲位（warehouse.feature WH9）
    pub warehouses: Arc<dyn crate::ports::WarehouseRepository>,
    /// 列印、匯出 Excel 的共用組裝（ADR 0013 SA3）
    pub reports: ListReporter,
}

#[derive(Clone)]
pub struct ItemService {
    authz: Arc<dyn Authorizer>,
    repos: ItemRepos,
    history: HistoryReader,
    /// 料件拋轉（item_carry.feature）：確認時自動拋轉、子公司收到料件的限制
    carry: Option<crate::ItemCarryService>,
}

impl ItemService {
    pub fn new(authz: Arc<dyn Authorizer>, repos: ItemRepos, history: HistoryReader) -> Self {
        Self {
            authz,
            repos,
            history,
            carry: None,
        }
    }

    /// 接上料件拋轉（item_carry.feature IC3、IC9）
    pub fn with_carry(mut self, carry: crate::ItemCarryService) -> Self {
        self.carry = Some(carry);
        self
    }

    /// 子公司收到的料件：不可修改（aoo-045）／不可刪除（aoo-044）（IC9）
    async fn check_carried(
        &self,
        plant: &PlantId,
        item: &Item,
        delete: bool,
    ) -> Result<(), AppError> {
        let Some(c) = &self.carry else { return Ok(()) };
        let (can_update, can_delete) = c.rights(plant, item).await?;
        if delete && !can_delete {
            return Err(DomainError::ItemCarriedNotDeletable.into());
        }
        if !delete && !can_update {
            return Err(DomainError::ItemCarriedNotEditable.into());
        }
        Ok(())
    }

    pub async fn list(&self, actor: &Actor, plant: &str) -> Result<Vec<ItemDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let mut out = Vec::new();
        for i in self.repos.items.list_items(&plant).await.map_err(infra)? {
            out.push(self.to_dto(&plant, &i, None).await?);
        }
        Ok(out)
    }

    /// 查詢一筆；`lang` 決定品名、規格的語系（指定語系 → zh-TW，ADR 0005）
    pub async fn get(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        lang: Option<&str>,
    ) -> Result<ItemDto, AppError> {
        let grant = self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let lang = lang.map(Lang::parse).transpose()?;
        let i = self.load(&plant, no).await?;
        // 被資料權限過濾掉的料件也不能直接查看（IQ4，同 AQ6）
        grant.record(&i.created_by, &i.created_dept)?;
        self.to_dto(&plant, &i, lang.as_ref()).await
    }

    /// 測試用：調整同步產生報表的筆數上限
    pub fn with_report_limit(mut self, limit: u64) -> Self {
        self.repos.reports.limit = limit;
        self
    }

    /// 列印、匯出 Excel（i100_out，aimi100.4gl:4488-4506；item_maintenance_report.feature）：依查詢條件（含資料權限）、
    /// 料號排序；品名、規格依報表語系；超過上限 → REPORT_TOO_LARGE；寫入稽核紀錄
    pub async fn report_in(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: Vec<ttrs_shared::QbeCondition>,
        format: ReportFormat,
        lang: &Lang,
    ) -> Result<ReportData, AppError> {
        let scope = self.authorize(actor, format.action()).await?.scope();
        let plant = PlantId::parse(plant)?;
        let filters = crate::query::item::filters(&conditions)?;
        let reports = &self.repos.reports;
        let (rows, total) = self
            .repos
            .items
            .query_items(&plant, &filters, &scope, 0, reports.fetch_limit())
            .await
            .map_err(infra)?;
        reports.check(total)?;
        let t = |id: &str| report::text(lang, id);
        let mut lines = Vec::with_capacity(rows.len());
        for i in &rows {
            let d = self.to_dto(&plant, i, Some(lang)).await?;
            let f = d.fields;
            lines.push(vec![
                f.item_no,
                f.name.unwrap_or_default(),
                f.spec.unwrap_or_default(),
                f.group_code.unwrap_or_default(),
                f.source_code.unwrap_or_default(),
                f.stock_unit.unwrap_or_default(),
                f.product_class.unwrap_or_default(),
                t(&format!("item-active-{}", d.active_code.to_lowercase())),
            ]);
        }
        reports
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

    /// QBE 查詢（i100_curs、i100_declare_curs，aimi100.4gl:354-730；IQ1–IQ10）：
    /// 依資料權限過濾、依料號排序並分頁；結果的品名、規格依 `req.lang` 顯示（IQ3）
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<ItemDto>, AppError> {
        let scope = self.authorize(actor, "query").await?.scope();
        let plant = PlantId::parse(plant)?;
        let lang = req.lang.as_deref().map(Lang::parse).transpose()?;
        let filters = crate::query::item::filters(&req.conditions)?;
        let p = crate::query::paging(&req);
        let (rows, total) = self
            .repos
            .items
            .query_items(&plant, &filters, &scope, p.offset, u64::from(p.page_size))
            .await
            .map_err(infra)?;
        let mut items = Vec::with_capacity(rows.len());
        for i in &rows {
            items.push(self.to_dto(&plant, i, lang.as_ref()).await?);
        }
        Ok(Page {
            items,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 分群碼的預設值（i100_set_rel_ima06；IM25）：畫面詢問後填入表單，後端不自動覆蓋
    pub async fn group_defaults(
        &self,
        actor: &Actor,
        plant: &str,
        group: &str,
    ) -> Result<ItemInput, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let code = ItemGroupCode::parse(group).map_err(|_| DomainError::ItemGroupNotFound)?;
        let g = self
            .repos
            .groups
            .get_item_group(&plant, &code)
            .await
            .map_err(infra)?
            .ok_or(DomainError::ItemGroupNotFound)?;
        if !g.active {
            return Err(DomainError::ItemGroupInactive.into());
        }
        Ok(to_input(
            "",
            &ItemAttrs::from_group(code.as_str(), &g.attrs),
        ))
    }

    /// 新增（i100_i、i100_default）：處理中／開立
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        input: ItemInput,
    ) -> Result<ItemDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let plant = PlantId::parse(plant)?;
            if !self
                .repos
                .master
                .plant_active(&plant)
                .await
                .map_err(infra)?
            {
                return Err(DomainError::PlantNotFound.into());
            }
            let no = ItemNo::parse(&input.item_no)?;
            // MISC 開頭須先有 MISC（aim-806）
            if no.needs_misc_master() {
                let misc = ItemNo::parse("MISC")?;
                if self
                    .repos
                    .items
                    .get_item(&plant, &misc)
                    .await
                    .map_err(infra)?
                    .is_none()
                {
                    return Err(DomainError::ItemMiscRequired.into());
                }
            }
            let attrs = self.attrs(&plant, &no, input).await?;
            // 主要倉庫須有效、主要儲位須屬於該倉庫（mfg1100、mfg1101，WH9）
            crate::warehouse_service::check_storage(
                self.repos.warehouses.as_ref(),
                &plant,
                attrs.warehouse.as_deref(),
                attrs.location.as_deref(),
            )
            .await?;
            // 受控營運中心自己新增料件時檢查料號、分群碼、材料類別（aimi100.4gl:6208、6310、5549，DC7）
            self.field_check(&plant, "item_no", no.as_str()).await?;
            self.field_check(
                &plant,
                "group_code",
                attrs.group_code.as_deref().unwrap_or_default(),
            )
            .await?;
            self.field_check(
                &plant,
                "material_category",
                attrs.material_category.as_deref().unwrap_or_default(),
            )
            .await?;
            let item = Item::new(
                no,
                attrs,
                plant.as_str(),
                &actor.user_id,
                &actor.dept,
                today(),
            );
            self.insert(&plant, &item).await
        })
        .await
    }

    /// 修改（i100_u）：無效不可修改（mfg1000）；料號建立後不可修改
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        input: ItemInput,
    ) -> Result<ItemDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            let grant = self.authorize(actor, "modify").await?;
            let plant = PlantId::parse(plant)?;
            let mut item = self.load(&plant, no).await?;
            grant.record(&item.created_by, &item.created_dept)?;
            item.stamp.check(input.version)?;
            // 子公司收到的料件不可修改（i100_u aoo-045，IC9）
            self.check_carried(&plant, &item, false).await?;
            if !item.active() {
                return Err(DomainError::ItemInactive.into());
            }
            let no = item.no.clone();
            let attrs = self.attrs(&plant, &no, input).await?;
            // 主要倉庫、儲位：只檢查有變動的（WH9）
            if (&attrs.warehouse, &attrs.location) != (&item.attrs.warehouse, &item.attrs.location)
            {
                crate::warehouse_service::check_storage(
                    self.repos.warehouses.as_ref(),
                    &plant,
                    attrs.warehouse.as_deref(),
                    attrs.location.as_deref(),
                )
                .await?;
            }
            // 修改時只檢查有變動的分群碼、材料類別（DC7；既有資料不因條件後加而無法修改）
            if attrs.group_code != item.attrs.group_code {
                self.field_check(
                    &plant,
                    "group_code",
                    attrs.group_code.as_deref().unwrap_or_default(),
                )
                .await?;
            }
            if attrs.material_category != item.attrs.material_category {
                self.field_check(
                    &plant,
                    "material_category",
                    attrs.material_category.as_deref().unwrap_or_default(),
                )
                .await?;
            }
            item.update(attrs, &actor.user_id, today())?;
            self.save(&plant, &item).await
        })
        .await
    }

    /// 確認（aimi100_sub i100sub_y_chk／y_upd）
    pub async fn confirm(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        version: Option<i32>,
    ) -> Result<ItemDto, AppError> {
        let mut dto = self
            .change(actor, plant, no, version, "confirm", |i, u, d| {
                i.confirm(u, d)
            })
            .await?;
        // 拋轉方式 1：確認成功後自動拋轉；拋轉失敗不影響確認（i100sub_carry，IC3）
        if let Some(c) = &self.carry {
            let plant = PlantId::parse(plant)?;
            if let Some(item) = self
                .repos
                .items
                .get_item(&plant, &ItemNo::parse(no)?)
                .await
                .map_err(infra)?
            {
                dto.carry_result = c.auto_carry(actor, &plant, &item).await;
            }
        }
        Ok(dto)
    }

    /// 取消確認（i100_notconfirm）
    pub async fn unconfirm(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        version: Option<i32>,
    ) -> Result<ItemDto, AppError> {
        self.change(actor, plant, no, version, "notconfirm", |i, u, d| {
            i.unconfirm(u, d)
        })
        .await
    }

    /// 作廢／恢復（i100_x，切換）：已有庫存異動時不可作廢（IM28）
    pub async fn toggle_void(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        version: Option<i32>,
    ) -> Result<ItemDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "invalid", async {
            let grant = self.authorize(actor, "invalid").await?;
            let plant = PlantId::parse(plant)?;
            let mut item = self.load(&plant, no).await?;
            grant.record(&item.created_by, &item.created_dept)?;
            item.stamp.check(version)?;
            // 子公司收到的料件不可作廢（i100_x aoo-045，IC9）
            self.check_carried(&plant, &item, false).await?;
            if item.active() && self.has_transactions(&plant, &item.no).await? {
                return Err(DomainError::ItemHasTransactions.into());
            }
            crate::change_log::set_operation(if item.active() {
                "deactivate"
            } else {
                "activate"
            });
            item.toggle_void(&actor.user_id, today());
            self.save(&plant, &item).await
        })
        .await
    }

    /// 刪除（i100_r）：只限處理中、沒有庫存異動；連同多語言品名一併刪除
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
            let item = self.load(&plant, no).await?;
            grant.record(&item.created_by, &item.created_dept)?;
            item.stamp.check(version)?;
            // 子公司收到的料件依「子公司可刪除」（i100_r aoo-044，IC9）
            self.check_carried(&plant, &item, true).await?;
            if self.has_transactions(&plant, &item.no).await? {
                return Err(DomainError::ItemHasTransactions.into());
            }
            item.check_deletable()?;
            match self.repos.items.delete_item(&plant, &item.no).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::ItemNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::ItemHasTransactions.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 複製（i100_copy）：新料號不可重複；MISC 規則同新增
    pub async fn copy(
        &self,
        actor: &Actor,
        plant: &str,
        from: &str,
        req: CopyItemRequest,
    ) -> Result<ItemDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let plant = PlantId::parse(plant)?;
            let item = self.load(&plant, from).await?;
            let no = ItemNo::parse(&req.new_item_no)?;
            if no.needs_misc_master() {
                let misc = ItemNo::parse("MISC")?;
                if self
                    .repos
                    .items
                    .get_item(&plant, &misc)
                    .await
                    .map_err(infra)?
                    .is_none()
                {
                    return Err(DomainError::ItemMiscRequired.into());
                }
            }
            // 複製的新料號也要符合資料控管條件（aimi100.4gl:6624，DC7）
            self.field_check(&plant, "item_no", no.as_str()).await?;
            let copy = item.copy_as(no, req.name, &actor.user_id, &actor.dept, today());
            self.insert(&plant, &copy).await
        })
        .await
    }

    /// 訂單無效切換（客製 order_invalid）
    pub async fn toggle_order_invalid(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        version: Option<i32>,
    ) -> Result<ItemDto, AppError> {
        self.change(actor, plant, no, version, "order_invalid", |i, u, d| {
            i.toggle_order_invalid(u, d);
            Ok(())
        })
        .await
    }

    /// 多語言品名、規格（update_item；IM7）：zh-TW 為料件本身的品名、規格，請用修改
    pub async fn set_text(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        text: ItemTextDto,
    ) -> Result<ItemDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "update_item", async {
            let grant = self.authorize(actor, "update_item").await?;
            let plant = PlantId::parse(plant)?;
            let item = self.load(&plant, no).await?;
            grant.record(&item.created_by, &item.created_dept)?;
            self.check_carried(&plant, &item, false).await?;
            let lang = Lang::parse(&text.lang)?;
            if lang.is_default() {
                return Err(DomainError::LanguageNotSupported(text.lang).into());
            }
            let t = ItemText {
                lang,
                name: ttrs_domain::lang::normalize_text(text.name),
                spec: ttrs_domain::lang::normalize_text(text.spec),
            };
            self.repos
                .items
                .set_item_text(&plant, &item.no, &t)
                .await
                .map_err(infra)?;
            self.to_dto(&plant, &item, None).await
        })
        .await
    }

    /// 異動記錄（ADR 0012 §8，功能 history）
    pub async fn history(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        self.authorize(actor, "history").await?;
        let plant = PlantId::parse(plant)?;
        let no = ItemNo::parse(no).map_err(|_| DomainError::ItemNotFound)?;
        self.history
            .read(
                Some(&plant),
                &["item", "item_text"],
                &[("item_no", no.as_str())],
                lang,
            )
            .await
    }

    /// 狀態類的異動：載入、資料權限、版本、套用、儲存
    async fn change(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        version: Option<i32>,
        action: &'static str,
        f: impl FnOnce(&mut Item, &str, NaiveDate) -> Result<(), DomainError>,
    ) -> Result<ItemDto, AppError> {
        crate::change_log::run(actor, PROGRAM, action, async {
            let grant = self.authorize(actor, action).await?;
            let plant = PlantId::parse(plant)?;
            let mut item = self.load(&plant, no).await?;
            grant.record(&item.created_by, &item.created_dept)?;
            item.stamp.check(version)?;
            // 子公司收到的料件不可取消確認（i100_notconfirm aoo-045，IC9）
            if action == "notconfirm" {
                self.check_carried(&plant, &item, false).await?;
            }
            f(&mut item, &actor.user_id, today())?;
            self.save(&plant, &item).await
        })
        .await
    }

    async fn has_transactions(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, AppError> {
        self.repos
            .items
            .has_transactions(plant, no)
            .await
            .map_err(infra)
    }

    /// 欄位檢查：值域與長度在 domain；引用的主檔在此檢查（IM26）
    async fn attrs(
        &self,
        plant: &PlantId,
        no: &ItemNo,
        i: ItemInput,
    ) -> Result<ItemAttrs, AppError> {
        let attrs = from_input(i).validated(no.is_misc())?;
        // 分群碼：須存在且有效（mfg3179、9028）
        if let Some(g) = &attrs.group_code {
            let group = match ItemGroupCode::parse(g) {
                Ok(code) => self
                    .repos
                    .groups
                    .get_item_group(plant, &code)
                    .await
                    .map_err(infra)?,
                Err(_) => None,
            };
            match group {
                None => return Err(DomainError::ItemGroupNotFound.into()),
                Some(g) if !g.active => return Err(DomainError::ItemGroupInactive.into()),
                Some(_) => {}
            }
        }
        // 單位：須為有效的單位（asf-031、mfg1200）
        for u in attrs.units() {
            let ok = match UnitCode::parse(u) {
                Ok(code) => self
                    .repos
                    .units
                    .get_unit(&code)
                    .await
                    .map_err(infra)?
                    .is_some_and(|x| x.active),
                Err(_) => false,
            };
            if !ok {
                return Err(DomainError::UnitNotFound.into());
            }
        }
        // 產品分類（客製必填，aim-372；mfg3179）
        if let Some(c) = &attrs.product_class {
            let ok = match ProductClassCode::parse(c) {
                Ok(code) => self
                    .repos
                    .classes
                    .get_product_class(plant, &code)
                    .await
                    .map_err(infra)?
                    .is_some_and(|x| x.active),
                Err(_) => false,
            };
            if !ok {
                return Err(DomainError::ProductClassNotFound.into());
            }
        }
        // 其他分群碼一～四（mfg1306）、材料類別（類別 8）
        for (kind, code) in OTHER_GROUP_KINDS
            .iter()
            .zip(attrs.other_groups.iter())
            .filter_map(|(k, c)| c.as_deref().map(|c| (*k, c)))
            .chain(attrs.material_category.as_deref().map(|c| ("8", c)))
        {
            self.check_code(plant, kind, code).await?;
        }
        // 倉管員（aoo-001）
        if let Some(e) = &attrs.stock_keeper
            && self.employee_name(plant, e).await?.is_none()
        {
            return Err(DomainError::ItemStockKeeperInvalid.into());
        }
        // 申請部門（客製，gem_file）
        if let Some(d) = &attrs.apply_dept
            && self.dept_name(plant, d).await?.is_none()
        {
            return Err(DomainError::ItemApplyDeptInvalid.into());
        }
        Ok(attrs)
    }

    async fn check_code(&self, plant: &PlantId, kind: &str, code: &str) -> Result<(), AppError> {
        let kind = CodeKind::parse(kind)?;
        let ok = match Code::parse(code) {
            Ok(c) => self
                .repos
                .codes
                .get_code(plant, kind, &c)
                .await
                .map_err(infra)?
                .is_some_and(|x| x.active),
            Err(_) => false,
        };
        if ok {
            Ok(())
        } else {
            Err(DomainError::CodeNotFound.into())
        }
    }

    /// 有效員工的姓名
    async fn employee_name(&self, plant: &PlantId, id: &str) -> Result<Option<String>, AppError> {
        Ok(match EmployeeId::parse(id) {
            Ok(id) => self
                .repos
                .employees
                .get(plant, &id)
                .await
                .map_err(infra)?
                .filter(|e| e.active)
                .map(|e| e.attrs.name),
            Err(_) => None,
        })
    }

    /// 有效部門的名稱
    async fn dept_name(&self, plant: &PlantId, code: &str) -> Result<Option<String>, AppError> {
        Ok(match DeptCode::parse(code) {
            Ok(c) => self
                .repos
                .departments
                .get(plant, &c)
                .await
                .map_err(infra)?
                .filter(|d| d.active)
                .map(|d| d.attrs.name),
            Err(_) => None,
        })
    }

    async fn insert(&self, plant: &PlantId, item: &Item) -> Result<ItemDto, AppError> {
        match self.repos.items.insert_item(plant, item).await {
            Ok(()) => {
                let saved = self.load(plant, item.no.as_str()).await?;
                self.to_dto(plant, &saved, None).await
            }
            Err(RepoError::Conflict) => Err(DomainError::ItemAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, plant: &PlantId, item: &Item) -> Result<ItemDto, AppError> {
        self.repos
            .items
            .update_item(plant, item)
            .await
            .map_err(infra)?;
        let saved = self.load(plant, item.no.as_str()).await?;
        self.to_dto(plant, &saved, None).await
    }

    async fn load(&self, plant: &PlantId, no: &str) -> Result<Item, AppError> {
        let no = ItemNo::parse(no).map_err(|_| DomainError::ItemNotFound)?;
        self.repos
            .items
            .get_item(plant, &no)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::ItemNotFound.into())
    }

    async fn to_dto(
        &self,
        plant: &PlantId,
        i: &Item,
        lang: Option<&Lang>,
    ) -> Result<ItemDto, AppError> {
        let a = &i.attrs;
        let texts = self
            .repos
            .items
            .item_texts(plant, &i.no)
            .await
            .map_err(infra)?;
        let mut fields = to_input(i.no.as_str(), a);
        // 指定語系 → zh-TW（ADR 0005 的退回規則）
        if let Some(t) = lang.and_then(|l| texts.iter().find(|t| &t.lang == l)) {
            if t.name.is_some() {
                fields.name = t.name.clone();
            }
            if t.spec.is_some() {
                fields.spec = t.spec.clone();
            }
        }
        let group_description = match a.group_code.as_deref().map(ItemGroupCode::parse) {
            Some(Ok(code)) => self
                .repos
                .groups
                .get_item_group(plant, &code)
                .await
                .map_err(infra)?
                .and_then(|g| g.attrs.description),
            _ => None,
        };
        let product_class_name = match a.product_class.as_deref().map(ProductClassCode::parse) {
            Some(Ok(code)) => self
                .repos
                .classes
                .get_product_class(plant, &code)
                .await
                .map_err(infra)?
                .and_then(|c| c.attrs.name),
            _ => None,
        };
        let stock_keeper_name = match &a.stock_keeper {
            Some(e) => self.employee_name(plant, e).await?,
            None => None,
        };
        let apply_dept_name = match &a.apply_dept {
            Some(d) => self.dept_name(plant, d).await?,
            None => None,
        };
        let s = &i.system;
        Ok(ItemDto {
            carry_result: None,
            fields,
            active_code: i.active_code.code().to_owned(),
            status_code: i.status_code.code().to_owned(),
            order_invalid: i.order_invalid,
            phase_out: s.phase_out,
            unit_usage: s.unit_usage.clone(),
            low_level_code: s.low_level_code,
            bom_changed: s.bom_changed.clone(),
            coproduct_this_month: s.coproduct_this_month,
            new_part_status: s.new_part_status.clone(),
            data_source: s.data_source.clone(),
            item_created_on: s.item_created_on,
            first_shipped_on: s.first_shipped_on,
            first_received_on: s.first_received_on,
            slow_moving_on: s.slow_moving_on,
            last_period_purchase_on: s.last_period_purchase_on,
            last_received_on: s.last_received_on,
            last_issued_on: s.last_issued_on,
            last_txn_on: s.last_txn_on,
            last_count_on: s.last_count_on,
            group_description,
            product_class_name,
            stock_keeper_name,
            apply_dept_name,
            texts: texts
                .into_iter()
                .map(|t| ItemTextDto {
                    lang: t.lang.as_str().to_owned(),
                    name: t.name,
                    spec: t.spec,
                })
                .collect(),
            active: i.active(),
            created_by: i.created_by.clone(),
            created_dept: i.created_dept.clone(),
            created_on: i.created_on,
            updated_by: i.updated_by.clone(),
            updated_on: i.updated_on,
            version: i.stamp.version,
            created_at: i.stamp.created_at,
            updated_at: i.stamp.updated_at,
        })
    }

    /// 資料控管欄位條件（s_field_chk，aoo-043）：資料類別 1 料件
    async fn field_check(&self, plant: &PlantId, field: &str, value: &str) -> Result<(), AppError> {
        crate::data_control_service::check_field(
            self.repos.data_control.as_ref(),
            "1",
            plant,
            field,
            value,
        )
        .await
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn from_input(i: ItemInput) -> ItemAttrs {
    ItemAttrs {
        name: i.name,
        spec: i.spec,
        group_code: i.group_code,
        source_code: i.source_code,
        configurable_item_no: i.configurable_item_no,
        current_version: i.current_version,
        catalog_ref: i.catalog_ref,
        supplier_control: i.supplier_control,
        product_class: i.product_class,
        drawing_no: i.drawing_no,
        apply_dept: i.apply_dept,
        software: i.software,
        engineering: i.engineering,
        insert_position: i.insert_position,
        insert_qpa_check: i.insert_qpa_check,
        material_category: i.material_category,
        coproduct: i.coproduct,
        inspection: i.inspection,
        repetitive: i.repetitive,
        pcn: i.pcn,
        abc_code: i.abc_code,
        consumable: i.consumable,
        replenish_policy: i.replenish_policy,
        economic_order_qty: i.economic_order_qty,
        average_order_qty: i.average_order_qty,
        safety_stock: i.safety_stock,
        safety_period: i.safety_period,
        max_stock_qty: i.max_stock_qty,
        shelf_life_days: i.shelf_life_days,
        stock_unit: i.stock_unit,
        warehouse: i.warehouse,
        location: i.location,
        stock_keeper: i.stock_keeper,
        second_unit: i.second_unit,
        pricing_unit: i.pricing_unit,
        other_groups: [
            i.other_group1,
            i.other_group2,
            i.other_group3,
            i.other_group4,
        ],
        account: i.account,
        account2: i.account2,
        bonded: i.bonded,
        full_name: i.full_name,
        english_name: i.english_name,
        planned_discontinue_on: i.planned_discontinue_on,
        life_cycle: i.life_cycle,
        carton_barcode: i.carton_barcode,
    }
}

fn to_input(no: &str, a: &ItemAttrs) -> ItemInput {
    let [g1, g2, g3, g4] = a.other_groups.clone();
    ItemInput {
        item_no: no.to_owned(),
        name: a.name.clone(),
        spec: a.spec.clone(),
        group_code: a.group_code.clone(),
        source_code: a.source_code.clone(),
        configurable_item_no: a.configurable_item_no.clone(),
        current_version: a.current_version.clone(),
        catalog_ref: a.catalog_ref.clone(),
        supplier_control: a.supplier_control.clone(),
        product_class: a.product_class.clone(),
        drawing_no: a.drawing_no.clone(),
        apply_dept: a.apply_dept.clone(),
        software: a.software,
        engineering: a.engineering,
        insert_position: a.insert_position,
        insert_qpa_check: a.insert_qpa_check,
        material_category: a.material_category.clone(),
        coproduct: a.coproduct,
        inspection: a.inspection,
        repetitive: a.repetitive,
        pcn: a.pcn.clone(),
        abc_code: a.abc_code.clone(),
        consumable: a.consumable,
        replenish_policy: a.replenish_policy.clone(),
        economic_order_qty: a.economic_order_qty,
        average_order_qty: a.average_order_qty,
        safety_stock: a.safety_stock,
        safety_period: a.safety_period,
        max_stock_qty: a.max_stock_qty,
        shelf_life_days: a.shelf_life_days,
        stock_unit: a.stock_unit.clone(),
        warehouse: a.warehouse.clone(),
        location: a.location.clone(),
        stock_keeper: a.stock_keeper.clone(),
        second_unit: a.second_unit.clone(),
        pricing_unit: a.pricing_unit.clone(),
        other_group1: g1,
        other_group2: g2,
        other_group3: g3,
        other_group4: g4,
        account: a.account.clone(),
        account2: a.account2.clone(),
        bonded: a.bonded,
        full_name: a.full_name.clone(),
        english_name: a.english_name.clone(),
        planned_discontinue_on: a.planned_discontinue_on,
        life_cycle: a.life_cycle.clone(),
        carton_barcode: a.carton_barcode.clone(),
        version: None,
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
