//! Basic Data Carry 基本資料拋轉（TipTop aoop603 / s_basic_data_carry）與 Carry History Query 拋轉歷史查詢（aooq604 / gex_file）
//! 規格：specs/features/aoo/basic_data_carry.feature（BC1–BC12）
//!
//! 在基本資料（資料類別 0）的來源營運中心執行，把勾選的基本資料拋到拋轉設定（aooi602）的營運中心：
//! 對方沒有就新增，已有時依「更新已存在資料」覆蓋或略過；一個營運中心一個交易（BC6(d)）。
//! 功能代號：basic-data-carry 的 query、carry；carry-history-query 的 query、output、exporttoexcel（BC10）。

use std::{collections::HashSet, sync::Arc};

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    basic_carry::{BasicItem, CarryAction, CarryLogEntry},
    carry_setting::CarrySetting,
    classification::{ClassificationCode, CodeKind},
    group_center::{DATA_CENTER, DataType, GroupCenter},
    plant::PlantId,
};
use ttrs_shared::{
    BasicCarryInfoDto, BasicCarryRequest, BasicCarryResultDto, BasicCarryResultRow, CarryLogDto,
    Page, QbeCondition, QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    mail::{Mail, MailTemplate, Mailer},
    ports::{
        BasicCarryBatch, BasicCarryRepository, CarryLogRepository, CarrySettingRepository,
        CarryWrite, ClassificationCodeRepository, DepartmentRepository, EmployeeRepository,
        GroupCenterRepository, ItemGroupRepository, ProductClassRepository,
    },
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "basic-data-carry";
pub const HISTORY_PROGRAM: &str = "carry-history-query";

/// 拋轉歷史報表（BC9）
const HISTORY_REPORT: ListSpec = ListSpec {
    program: HISTORY_PROGRAM,
    title: "carry-history-report-title",
    columns: &[
        ("center_code", "carry-history-col-center"),
        ("data_type", "carry-history-col-data-type"),
        ("plant", "carry-history-col-plant"),
        ("program", "carry-history-col-program"),
        ("record_key", "carry-history-col-key"),
        ("action", "carry-history-col-action"),
        ("carried_at", "carry-history-col-at"),
        ("carried_by", "carry-history-col-by"),
    ],
};

/// 基本資料的資料類別 0
fn basic_type() -> DataType {
    DataType::parse("0").unwrap_or_else(|_| unreachable!("資料類別 0 一定合法"))
}

/// 拋轉需要的資料存取
#[derive(Clone)]
pub struct BasicCarryRepos {
    pub centers: Arc<dyn GroupCenterRepository>,
    pub carries: Arc<dyn CarrySettingRepository>,
    pub logs: Arc<dyn CarryLogRepository>,
    pub writer: Arc<dyn BasicCarryRepository>,
    pub depts: Arc<dyn DepartmentRepository>,
    pub employees: Arc<dyn EmployeeRepository>,
    pub codes: Arc<dyn ClassificationCodeRepository>,
    pub classes: Arc<dyn ProductClassRepository>,
    pub groups: Arc<dyn ItemGroupRepository>,
}

#[derive(Clone)]
pub struct BasicCarryService {
    authz: Arc<dyn Authorizer>,
    repos: BasicCarryRepos,
    reports: ListReporter,
    mailer: Option<Arc<dyn Mailer>>,
}

/// 來源營運中心的資料（依 BC4 的順序）
#[derive(Default)]
struct Source {
    departments: Vec<ttrs_domain::department::Department>,
    employees: Vec<ttrs_domain::employee::Employee>,
    codes: Vec<(BasicItem, ClassificationCode)>,
    product_classes: Vec<ttrs_domain::product_class::ProductClass>,
    item_groups: Vec<ttrs_domain::item_group::ItemGroup>,
}

/// 一個營運中心、一個資料項目的計數
#[derive(Default, Clone)]
struct Count {
    inserted: u32,
    updated: u32,
    skipped: u32,
}

/// 新增時：資料所有者為執行拋轉的使用者；確認狀態、資料有效沿用來源（BC6(b)）
macro_rules! for_insert {
    ($src:expr, $actor:expr, $today:expr) => {{
        let mut n = $src.clone();
        n.created_by = $actor.user_id.clone();
        n.created_dept = $actor.dept.clone();
        n.created_on = $today;
        n.updated_by = None;
        n.updated_on = None;
        n.stamp = ttrs_domain::record::Stamp::default();
        n
    }};
}

/// 覆蓋時：業務欄位與資料有效照來源；資料所有者、所有部門、建立時間、確認狀態、版本保留對方原值（BC6(b)）
macro_rules! for_update {
    ($old:expr, $src:expr, $actor:expr, $today:expr, [$($f:ident),*]) => {{
        let mut n = $old.clone();
        $( n.$f = $src.$f.clone(); )*
        n.active = $src.active;
        n.updated_by = Some($actor.user_id.clone());
        n.updated_on = Some($today);
        n
    }};
}

impl BasicCarryService {
    pub fn new(authz: Arc<dyn Authorizer>, repos: BasicCarryRepos, reports: ListReporter) -> Self {
        Self {
            authz,
            repos,
            reports,
            mailer: None,
        }
    }

    /// 拋轉通知信（BC8）；沒有設定寄信服務時不寄
    pub fn with_mailer(mut self, mailer: Arc<dyn Mailer>) -> Self {
        self.mailer = Some(mailer);
        self
    }

    /// 目前營運中心所屬的基本資料資料中心與拋轉設定（:71-76、:137-160、:209-217）
    async fn context(&self, plant: &PlantId) -> Result<(GroupCenter, CarrySetting), AppError> {
        let t = basic_type();
        let center = self
            .repos
            .centers
            .list_group_centers()
            .await
            .map_err(infra)?
            .into_iter()
            .find(|c| {
                c.category == DATA_CENTER
                    && c.active
                    && c.data_types
                        .iter()
                        .any(|d| d.data_type == t && d.plant == plant.as_str())
            })
            .ok_or(DomainError::BasicCarryNotSource)?;
        let setting = self
            .repos
            .carries
            .get_carry_setting(&center.code, t)
            .await
            .map_err(infra)?
            .filter(|s| !s.targets.is_empty())
            .ok_or(DomainError::BasicCarryNoTarget)?;
        Ok((center, setting))
    }

    /// 開啟畫面（query）：資料中心、可拋轉的資料、拋轉營運中心（BC1–BC3）
    pub async fn info(&self, actor: &Actor, plant: &str) -> Result<BasicCarryInfoDto, AppError> {
        self.authorize(actor, PROGRAM, "query").await?;
        let plant = PlantId::parse(plant)?;
        let (center, setting) = self.context(&plant).await?;
        Ok(BasicCarryInfoDto {
            center_code: center.code.as_str().to_owned(),
            center_name: center.name.clone(),
            mode: setting.mode.code(),
            items: BasicItem::ALL.iter().map(|i| i.code().to_owned()).collect(),
            targets: setting.targets.iter().map(|t| t.plant.clone()).collect(),
        })
    }

    /// 執行拋轉（carry；:302-330、s_basic_data_carry）
    pub async fn carry(
        &self,
        actor: &Actor,
        plant: &str,
        req: BasicCarryRequest,
    ) -> Result<BasicCarryResultDto, AppError> {
        self.authorize(actor, PROGRAM, "carry").await?;
        // AI 代表使用者時不得執行拋轉（BC10）
        if actor.kind == crate::ActorKind::Ai {
            return Err(DomainError::PermissionDenied {
                program: PROGRAM.to_owned(),
                action: "carry".to_owned(),
                reason: ttrs_domain::authorization::DenyReason::AiNotAllowed,
            }
            .into());
        }
        let source_plant = PlantId::parse(plant)?;
        let (center, setting) = self.context(&source_plant).await?;
        let mut items = req
            .items
            .iter()
            .filter(|s| !s.trim().is_empty())
            .map(|s| BasicItem::parse(s))
            .collect::<Result<Vec<_>, _>>()?;
        items.sort();
        items.dedup();
        let all: Vec<String> = setting.targets.iter().map(|t| t.plant.clone()).collect();
        let targets: Vec<String> = match &req.targets {
            None => all.clone(),
            Some(list) => {
                let list: Vec<String> = list
                    .iter()
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_uppercase())
                    .collect();
                if list.is_empty() {
                    return Err(DomainError::BasicCarryNothingSelected.into());
                }
                if let Some(bad) = list.iter().find(|p| !all.contains(p)) {
                    let _ = bad;
                    return Err(DomainError::CarryTargetNotFound.into());
                }
                // 拋轉方式 1、2 拋給全部拋轉營運中心，不能另外挑選（s_dc_sel_db，BC5）
                if setting.mode.code() != "3" {
                    return Err(DomainError::BasicCarryTargetsFixed.into());
                }
                list
            }
        };
        if items.is_empty() || targets.is_empty() {
            return Err(DomainError::BasicCarryNothingSelected.into());
        }
        let source = self.load(&source_plant, &items).await?;
        let today = chrono::Local::now().date_naive();
        let mut rows = Vec::new();
        let mut mail_warnings = Vec::new();
        for target in &targets {
            let update_existing = setting
                .targets
                .iter()
                .find(|t| &t.plant == target)
                .is_some_and(|t| t.update_existing);
            let target_id = PlantId::parse(target)?;
            let outcome = crate::change_log::run(actor, PROGRAM, "carry", async {
                let (batch, counts, logs) = self
                    .plan(&target_id, &items, &source, update_existing, actor, today)
                    .await?;
                match self
                    .repos
                    .writer
                    .apply_basic_carry(&target_id, &batch)
                    .await
                {
                    Ok(()) => Ok((counts, logs)),
                    Err(e) => Err(AppError::Infra(e.to_string())),
                }
            })
            .await;
            match outcome {
                Ok((counts, keys)) => {
                    let now = chrono::Utc::now();
                    let entries: Vec<CarryLogEntry> = keys
                        .into_iter()
                        .map(|(item, key, action)| CarryLogEntry {
                            center: center.code.clone(),
                            data_type: basic_type(),
                            plant: target.clone(),
                            program: item.program().to_owned(),
                            record_key: key,
                            action,
                            carried_at: now,
                            carried_by: actor.user_id.clone(),
                        })
                        .collect();
                    self.repos
                        .logs
                        .append_carry_logs(&entries)
                        .await
                        .map_err(infra)?;
                    for (item, c) in items.iter().zip(counts.iter()) {
                        rows.push(BasicCarryResultRow {
                            plant: target.clone(),
                            item: item.code().to_owned(),
                            inserted: c.inserted,
                            updated: c.updated,
                            skipped: c.skipped,
                            ok: true,
                            error: None,
                        });
                    }
                    if let Some(w) = self.notify(&setting, target, &items, &counts).await {
                        mail_warnings.push(w);
                    }
                }
                Err(e) => {
                    let reason = e.to_string();
                    for item in &items {
                        rows.push(BasicCarryResultRow {
                            plant: target.clone(),
                            item: item.code().to_owned(),
                            inserted: 0,
                            updated: 0,
                            skipped: 0,
                            ok: false,
                            error: Some(reason.clone()),
                        });
                    }
                }
            }
        }
        Ok(BasicCarryResultDto {
            rows,
            mail_warnings,
        })
    }

    /// 讀取來源營運中心的資料
    async fn load(&self, plant: &PlantId, items: &[BasicItem]) -> Result<Source, AppError> {
        let r = &self.repos;
        let mut s = Source::default();
        for item in items {
            match item {
                BasicItem::Department => {
                    s.departments = r
                        .depts
                        .query_departments(plant, &[], 0, u64::MAX)
                        .await
                        .map_err(infra)?
                        .0
                }
                BasicItem::Employee => {
                    s.employees = r
                        .employees
                        .query_employees(plant, &[], 0, u64::MAX)
                        .await
                        .map_err(infra)?
                        .0
                }
                BasicItem::ProductClass => {
                    s.product_classes =
                        r.classes.list_product_classes(plant).await.map_err(infra)?
                }
                BasicItem::ItemGroup => {
                    s.item_groups = r.groups.list_item_groups(plant).await.map_err(infra)?
                }
                other => {
                    let kind = CodeKind::parse(other.code_kind().unwrap_or_default())?;
                    for c in r.codes.list_codes(plant, kind).await.map_err(infra)? {
                        s.codes.push((*other, c));
                    }
                }
            }
        }
        Ok(s)
    }

    /// 拋到一個營運中心的寫入計畫與參照檢查（BC6(a)–(c)）；參照可以是對方已有的或這次一起拋過去的
    #[allow(clippy::type_complexity)]
    async fn plan(
        &self,
        target: &PlantId,
        items: &[BasicItem],
        src: &Source,
        update_existing: bool,
        actor: &Actor,
        today: NaiveDate,
    ) -> Result<
        (
            BasicCarryBatch,
            Vec<Count>,
            Vec<(BasicItem, String, CarryAction)>,
        ),
        AppError,
    > {
        let r = &self.repos;
        let mut batch = BasicCarryBatch::default();
        let mut counts = vec![Count::default(); items.len()];
        let mut logs = Vec::new();
        let idx = |i: BasicItem| items.iter().position(|x| *x == i).unwrap_or(0);
        let mut decide = |item: BasicItem, key: String, exists: bool| -> Option<bool> {
            let c = &mut counts[idx(item)];
            match (exists, update_existing) {
                (false, _) => {
                    c.inserted += 1;
                    logs.push((item, key, CarryAction::Insert));
                    Some(false)
                }
                (true, true) => {
                    c.updated += 1;
                    logs.push((item, key, CarryAction::Update));
                    Some(true)
                }
                (true, false) => {
                    c.skipped += 1;
                    None
                }
            }
        };
        let missing = |item: BasicItem, key: &str, field: &str, value: &str| -> AppError {
            DomainError::BasicCarryReferenceMissing {
                item: item.code().to_owned(),
                key: key.to_owned(),
                field: field.to_owned(),
                value: value.to_owned(),
            }
            .into()
        };

        // 部門（成本中心可參照自己這批的部門）
        let mut depts: HashSet<String> = r
            .depts
            .query_departments(target, &[], 0, u64::MAX)
            .await
            .map_err(infra)?
            .0
            .iter()
            .map(|d| d.code.as_str().to_owned())
            .collect();
        depts.extend(src.departments.iter().map(|d| d.code.as_str().to_owned()));
        for d in &src.departments {
            if let Some(cc) = &d.accounting.cost_center
                && !depts.contains(cc.as_str())
            {
                return Err(missing(
                    BasicItem::Department,
                    d.code.as_str(),
                    "cost_center",
                    cc.as_str(),
                ));
            }
            let old = r.depts.get(target, &d.code).await.map_err(infra)?;
            match decide(
                BasicItem::Department,
                d.code.as_str().to_owned(),
                old.is_some(),
            ) {
                Some(false) => batch
                    .departments
                    .push(CarryWrite::Insert(for_insert!(d, actor, today))),
                Some(true) => {
                    let old = old.unwrap_or_else(|| d.clone());
                    batch.departments.push(CarryWrite::Update(for_update!(
                        old,
                        d,
                        actor,
                        today,
                        [attrs, accounting]
                    )))
                }
                None => {}
            }
        }
        // 員工（部門須存在）
        let mut employees: HashSet<String> = r
            .employees
            .query_employees(target, &[], 0, u64::MAX)
            .await
            .map_err(infra)?
            .0
            .iter()
            .map(|e| e.id.as_str().to_owned())
            .collect();
        for e in &src.employees {
            if let Some(dept) = &e.attrs.dept_code
                && !depts.contains(dept.as_str())
            {
                return Err(missing(
                    BasicItem::Employee,
                    e.id.as_str(),
                    "dept_code",
                    dept.as_str(),
                ));
            }
            let old = r.employees.get(target, &e.id).await.map_err(infra)?;
            match decide(BasicItem::Employee, e.id.as_str().to_owned(), old.is_some()) {
                Some(false) => batch
                    .employees
                    .push(CarryWrite::Insert(for_insert!(e, actor, today))),
                Some(true) => {
                    let old = old.unwrap_or_else(|| e.clone());
                    batch.employees.push(CarryWrite::Update(for_update!(
                        old,
                        e,
                        actor,
                        today,
                        [attrs]
                    )))
                }
                None => {}
            }
        }
        employees.extend(src.employees.iter().map(|e| e.id.as_str().to_owned()));
        // 分類碼
        let mut codes: HashSet<(String, String)> = HashSet::new();
        for kind in ["8", "D", "E", "F", "G"] {
            let k = CodeKind::parse(kind)?;
            for c in r.codes.list_codes(target, k).await.map_err(infra)? {
                codes.insert((kind.to_owned(), c.code.as_str().to_owned()));
            }
        }
        for (item, c) in &src.codes {
            let kind = item.code_kind().unwrap_or_default();
            let old = r
                .codes
                .get_code(target, c.kind, &c.code)
                .await
                .map_err(infra)?;
            match decide(*item, c.code.as_str().to_owned(), old.is_some()) {
                Some(false) => batch
                    .codes
                    .push(CarryWrite::Insert(for_insert!(c, actor, today))),
                Some(true) => {
                    let old = old.unwrap_or_else(|| c.clone());
                    batch.codes.push(CarryWrite::Update(for_update!(
                        old,
                        c,
                        actor,
                        today,
                        [attrs]
                    )))
                }
                None => {}
            }
            codes.insert((kind.to_owned(), c.code.as_str().to_owned()));
        }
        // 產品分類（成本中心須為部門）
        let mut classes: HashSet<String> = r
            .classes
            .list_product_classes(target)
            .await
            .map_err(infra)?
            .iter()
            .map(|c| c.code.as_str().to_owned())
            .collect();
        for p in &src.product_classes {
            if let Some(cc) = &p.attrs.cost_center
                && !depts.contains(cc.as_str())
            {
                return Err(missing(
                    BasicItem::ProductClass,
                    p.code.as_str(),
                    "cost_center",
                    cc.as_str(),
                ));
            }
            let old = r
                .classes
                .get_product_class(target, &p.code)
                .await
                .map_err(infra)?;
            match decide(
                BasicItem::ProductClass,
                p.code.as_str().to_owned(),
                old.is_some(),
            ) {
                Some(false) => batch
                    .product_classes
                    .push(CarryWrite::Insert(for_insert!(p, actor, today))),
                Some(true) => {
                    let old = old.unwrap_or_else(|| p.clone());
                    batch.product_classes.push(CarryWrite::Update(for_update!(
                        old,
                        p,
                        actor,
                        today,
                        [attrs]
                    )))
                }
                None => {}
            }
            classes.insert(p.code.as_str().to_owned());
        }
        // 料件分群（產品分類、分類碼、倉管員須存在）
        for g in &src.item_groups {
            let key = g.code.as_str();
            let a = &g.attrs;
            if let Some(pc) = &a.product_class
                && !classes.contains(pc)
            {
                return Err(missing(BasicItem::ItemGroup, key, "product_class", pc));
            }
            let refs = [("8", &a.material_category, "material_category")]
                .into_iter()
                .chain(
                    ["D", "E", "F", "G"]
                        .into_iter()
                        .zip(a.other_groups.iter())
                        .zip([
                            "other_group1",
                            "other_group2",
                            "other_group3",
                            "other_group4",
                        ])
                        .map(|((k, v), f)| (k, v, f)),
                );
            for (kind, value, field) in refs {
                if let Some(v) = value
                    && !codes.contains(&(kind.to_owned(), v.clone()))
                {
                    return Err(missing(BasicItem::ItemGroup, key, field, v));
                }
            }
            if let Some(k) = &a.stock_keeper
                && !employees.contains(k)
            {
                return Err(missing(BasicItem::ItemGroup, key, "stock_keeper", k));
            }
            let old = r
                .groups
                .get_item_group(target, &g.code)
                .await
                .map_err(infra)?;
            match decide(BasicItem::ItemGroup, key.to_owned(), old.is_some()) {
                Some(false) => batch
                    .item_groups
                    .push(CarryWrite::Insert(for_insert!(g, actor, today))),
                Some(true) => {
                    let old = old.unwrap_or_else(|| g.clone());
                    batch.item_groups.push(CarryWrite::Update(for_update!(
                        old,
                        g,
                        actor,
                        today,
                        [attrs]
                    )))
                }
                None => {}
            }
        }
        Ok((batch, counts, logs))
    }

    /// 通知信（BC8）：寄給該拋轉營運中心的通知人員；失敗回傳提示，不影響拋轉結果
    async fn notify(
        &self,
        setting: &CarrySetting,
        plant: &str,
        items: &[BasicItem],
        counts: &[Count],
    ) -> Option<String> {
        let mailer = self.mailer.as_ref()?;
        let to = setting
            .targets
            .iter()
            .find(|t| t.plant == plant)
            .and_then(|t| t.notify.clone())?;
        let lang = ttrs_domain::lang::Lang::default_lang();
        let detail = items
            .iter()
            .zip(counts)
            .map(|(i, c)| {
                format!(
                    "{}：{} {}、{} {}、{} {}",
                    report::text(&lang, &format!("basic-carry-item-{}", i.code())),
                    report::text(&lang, "basic-carry-col-inserted"),
                    c.inserted,
                    report::text(&lang, "basic-carry-col-updated"),
                    c.updated,
                    report::text(&lang, "basic-carry-col-skipped"),
                    c.skipped
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let date = chrono::Local::now().date_naive().to_string();
        for addr in to.split(';') {
            let mail = Mail::render(
                addr,
                &lang,
                MailTemplate::BasicCarryDone,
                &[
                    ("plant", plant),
                    ("center", setting.center.as_str()),
                    ("date", date.as_str()),
                    ("detail", detail.as_str()),
                ],
            );
            if let Err(e) = mailer.send(&mail).await {
                return Some(format!("{plant} {addr}: {e}"));
            }
        }
        None
    }

    // ----- 拋轉歷史查詢（aooq604） -----

    async fn history_rows(&self) -> Result<Vec<CarryLogDto>, AppError> {
        let mut all = self.repos.logs.list_carry_logs().await.map_err(infra)?;
        all.sort_by(|a, b| {
            b.carried_at
                .cmp(&a.carried_at)
                .then(b.record_key.cmp(&a.record_key))
        });
        Ok(all.iter().map(log_dto).collect())
    }

    /// 查詢拋轉歷史（BC9）：依拋轉時間新到舊
    pub async fn history(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<CarryLogDto>, AppError> {
        self.authorize(actor, HISTORY_PROGRAM, "query").await?;
        let filters = query::carry_log::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.history_rows().await?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::carry_log::value, p);
        Ok(Page {
            items: rows,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印、匯出拋轉歷史（BC9）
    pub async fn history_report(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, HISTORY_PROGRAM, format.action())
            .await?;
        let filters = query::carry_log::filters(&conditions)?;
        let all = self.history_rows().await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::carry_log::value, p);
        self.reports.check(total)?;
        let t = |id: &str| report::text(lang, id);
        let lines = rows
            .iter()
            .map(|l| {
                vec![
                    l.center_code.clone(),
                    l.data_type.clone(),
                    l.plant.clone(),
                    l.program.clone(),
                    l.record_key.clone(),
                    t(&format!("carry-history-action-{}", l.action)),
                    l.carried_at
                        .with_timezone(&chrono::Local)
                        .format("%Y/%m/%d %H:%M:%S")
                        .to_string(),
                    l.carried_by.clone(),
                ]
            })
            .collect();
        self.reports
            .build(
                actor,
                None,
                &HISTORY_REPORT,
                lines,
                &conditions,
                format,
                lang,
            )
            .await
    }

    async fn authorize(
        &self,
        actor: &Actor,
        program: &'static str,
        action: &'static str,
    ) -> Result<Grant, AppError> {
        self.authz.authorize(actor, program, action).await
    }
}

fn log_dto(l: &CarryLogEntry) -> CarryLogDto {
    CarryLogDto {
        center_code: l.center.as_str().to_owned(),
        data_type: l.data_type.code(),
        plant: l.plant.clone(),
        program: l.program.clone(),
        record_key: l.record_key.clone(),
        action: l.action.code().to_owned(),
        carried_at: l.carried_at,
        carried_by: l.carried_by.clone(),
    }
}

fn infra(e: RepoError) -> AppError {
    match e {
        RepoError::Stale => DomainError::RecordChanged.into(),
        e => AppError::Infra(e.to_string()),
    }
}
