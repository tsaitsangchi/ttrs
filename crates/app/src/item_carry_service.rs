//! 料件拋轉（TipTop aimi100 carry、i100sub_carry、s_aimi100_carry）與子公司收到料件的限制（s_dc_ud_flag）
//! 規格：specs/features/aim/item_carry.feature（IC1–IC13）
//!
//! 在料件（資料類別 1）的來源營運中心，把一筆已確認的料件拋到拋轉設定（aooi602）的營運中心：
//! 依拋轉條件篩選、套用預設值、檢查參照；對方沒有就新增，已有依「更新已存在資料」覆蓋或略過；一個營運中心一個交易（IC7）。
//! 功能代號沿用 TipTop：item-maintenance 的 carry、qry_carry_history（IC11）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    basic_carry::{CarryAction, CarryLogEntry},
    carry_setting::CarrySetting,
    classification::{Code, CodeKind},
    department::DeptCode,
    employee::EmployeeId,
    group_center::{DATA_CENTER, DataType, GroupCenter},
    item::{ActiveCode, Item, ItemNo},
    item_group::ItemGroupCode,
    plant::PlantId,
    product_class::ProductClassCode,
};
use ttrs_shared::{ItemCarryInfoDto, ItemCarryRequest, ItemCarryResultDto, ItemCarryResultRow};

use crate::{
    Actor, AppError, Authorizer,
    error::RepoError,
    mail::{Mail, MailTemplate, Mailer},
    ports::{
        CarryLogRepository, CarrySettingRepository, CarryWrite, ClassificationCodeRepository,
        DepartmentRepository, EmployeeRepository, GroupCenterRepository, ItemCarryBatch,
        ItemCarryRepository, ItemGroupRepository, ItemRepository, ItemUnitConversionRepository,
        ProductClassRepository,
    },
};

/// 權限檢查用的程式代號（料件維護的功能 carry，IC11）
pub const PROGRAM: &str = "item-maintenance";
/// 拋轉紀錄的程式代號（TipTop 作業代號，IC8）
const ITEM_LOG: &str = "aimi100";
const CONVERSION_LOG: &str = "aooi103";

fn item_type() -> DataType {
    DataType::parse("1").unwrap_or_else(|_| unreachable!("資料類別 1 一定合法"))
}

/// 料件拋轉需要的資料存取
#[derive(Clone)]
pub struct ItemCarryRepos {
    pub centers: Arc<dyn GroupCenterRepository>,
    pub carries: Arc<dyn CarrySettingRepository>,
    pub logs: Arc<dyn CarryLogRepository>,
    pub writer: Arc<dyn ItemCarryRepository>,
    pub items: Arc<dyn ItemRepository>,
    pub conversions: Arc<dyn ItemUnitConversionRepository>,
    pub groups: Arc<dyn ItemGroupRepository>,
    pub classes: Arc<dyn ProductClassRepository>,
    pub codes: Arc<dyn ClassificationCodeRepository>,
    pub employees: Arc<dyn EmployeeRepository>,
    pub depts: Arc<dyn DepartmentRepository>,
}

#[derive(Clone)]
pub struct ItemCarryService {
    authz: Arc<dyn Authorizer>,
    repos: ItemCarryRepos,
    mailer: Option<Arc<dyn Mailer>>,
}

/// 拋到一個營運中心的結果
enum Outcome {
    Written(CarryAction, Vec<(String, String, CarryAction)>),
    Skipped,
    NotMatched,
}

impl ItemCarryService {
    pub fn new(authz: Arc<dyn Authorizer>, repos: ItemCarryRepos) -> Self {
        Self {
            authz,
            repos,
            mailer: None,
        }
    }

    /// 拋轉通知信（同 basic_data_carry.feature BC8）
    pub fn with_mailer(mut self, mailer: Arc<dyn Mailer>) -> Self {
        self.mailer = Some(mailer);
        self
    }

    /// 目前營運中心所屬的料件資料中心與拋轉設定（aimi100.4gl:7618-7640，aoo-036）
    async fn context(&self, plant: &PlantId) -> Result<(GroupCenter, CarrySetting), AppError> {
        let t = item_type();
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
            .ok_or(DomainError::ItemCarryNotSource)?;
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

    /// 拋轉畫面資料（carry 前先取得拋轉營運中心，IC1）
    pub async fn info(&self, actor: &Actor, plant: &str) -> Result<ItemCarryInfoDto, AppError> {
        self.authz.authorize(actor, PROGRAM, "carry").await?;
        let plant = PlantId::parse(plant)?;
        let (center, setting) = self.context(&plant).await?;
        Ok(ItemCarryInfoDto {
            center_code: center.code.as_str().to_owned(),
            mode: setting.mode.code(),
            targets: setting.targets.iter().map(|t| t.plant.clone()).collect(),
        })
    }

    /// 拋轉（carry；aimi100.4gl:7591-7668）
    pub async fn carry(
        &self,
        actor: &Actor,
        plant: &str,
        no: &str,
        req: ItemCarryRequest,
    ) -> Result<ItemCarryResultDto, AppError> {
        self.authz.authorize(actor, PROGRAM, "carry").await?;
        // AI 代表使用者時不得執行拋轉（IC11）
        if actor.kind == crate::ActorKind::Ai {
            return Err(DomainError::PermissionDenied {
                program: PROGRAM.to_owned(),
                action: "carry".to_owned(),
                reason: ttrs_domain::authorization::DenyReason::AiNotAllowed,
            }
            .into());
        }
        let plant = PlantId::parse(plant)?;
        let item = self.load(&plant, no).await?;
        if item.active_code != ActiveCode::Confirmed {
            return Err(DomainError::ItemCarryNotConfirmed.into());
        }
        let (center, setting) = self.context(&plant).await?;
        let all: Vec<String> = setting.targets.iter().map(|t| t.plant.clone()).collect();
        let targets = match req.targets {
            None => all,
            Some(list) => {
                let list: Vec<String> = list
                    .iter()
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_uppercase())
                    .collect();
                if list.is_empty() {
                    return Err(DomainError::BasicCarryNothingSelected.into());
                }
                if list.iter().any(|p| !all.contains(p)) {
                    return Err(DomainError::CarryTargetNotFound.into());
                }
                if setting.mode.code() != "3" {
                    return Err(DomainError::BasicCarryTargetsFixed.into());
                }
                list
            }
        };
        self.run(actor, &plant, &item, &center, &setting, &targets)
            .await
    }

    /// 確認時自動拋轉（i100sub_carry）：拋轉方式 1 才拋給全部拋轉營運中心；不是來源營運中心或沒有設定時不拋（IC3）
    pub async fn auto_carry(
        &self,
        actor: &Actor,
        plant: &PlantId,
        item: &Item,
    ) -> Option<ItemCarryResultDto> {
        if actor.kind == crate::ActorKind::Ai {
            return None;
        }
        let (center, setting) = self.context(plant).await.ok()?;
        if setting.mode.code() != "1" {
            return None;
        }
        let targets: Vec<String> = setting.targets.iter().map(|t| t.plant.clone()).collect();
        self.run(actor, plant, item, &center, &setting, &targets)
            .await
            .ok()
    }

    async fn run(
        &self,
        actor: &Actor,
        source: &PlantId,
        item: &Item,
        center: &GroupCenter,
        setting: &CarrySetting,
        targets: &[String],
    ) -> Result<ItemCarryResultDto, AppError> {
        let texts = self
            .repos
            .items
            .item_texts(source, &item.no)
            .await
            .map_err(infra)?;
        let conversions = self
            .repos
            .conversions
            .list_item_unit_conversions(source, Some(&item.no))
            .await
            .map_err(infra)?;
        let today = chrono::Local::now().date_naive();
        let mut rows = Vec::new();
        let mut mail_warnings = Vec::new();
        for target in targets {
            let target_id = PlantId::parse(target)?;
            let outcome = crate::change_log::run(actor, PROGRAM, "carry", async {
                self.carry_one(
                    actor,
                    source,
                    &target_id,
                    item,
                    &texts,
                    &conversions,
                    setting,
                    today,
                )
                .await
            })
            .await;
            let (outcome_code, error) = match outcome {
                Ok(Outcome::Written(action, keys)) => {
                    let now = chrono::Utc::now();
                    let entries: Vec<CarryLogEntry> = keys
                        .into_iter()
                        .map(|(program, key, action)| CarryLogEntry {
                            center: center.code.clone(),
                            data_type: item_type(),
                            plant: target.clone(),
                            program,
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
                    if let Some(w) = self.notify(setting, target, item, action).await {
                        mail_warnings.push(w);
                    }
                    (
                        if action == CarryAction::Insert {
                            "inserted"
                        } else {
                            "updated"
                        },
                        None,
                    )
                }
                Ok(Outcome::Skipped) => ("skipped", None),
                Ok(Outcome::NotMatched) => ("not_matched", None),
                Err(e) => ("failed", Some(e.to_string())),
            };
            rows.push(ItemCarryResultRow {
                plant: target.clone(),
                outcome: outcome_code.to_owned(),
                error,
            });
        }
        Ok(ItemCarryResultDto {
            rows,
            mail_warnings,
        })
    }

    /// 拋到一個營運中心（s_aimi100_carry）
    #[allow(clippy::too_many_arguments)]
    async fn carry_one(
        &self,
        actor: &Actor,
        source: &PlantId,
        target: &PlantId,
        item: &Item,
        texts: &[ttrs_domain::item::ItemText],
        conversions: &[ttrs_domain::unit_conversion::ItemUnitConversion],
        setting: &CarrySetting,
        today: NaiveDate,
    ) -> Result<Outcome, AppError> {
        let r = &self.repos;
        let Some(cfg) = setting.targets.iter().find(|t| t.plant == target.as_str()) else {
            return Err(DomainError::CarryTargetNotFound.into());
        };
        // 拋轉條件（gew05，IC4）
        let a = &item.attrs;
        let value_of = |field: &str| -> String {
            match field {
                "item_no" => item.no.as_str().to_owned(),
                "group_code" => a.group_code.clone().unwrap_or_default(),
                "material_category" => a.material_category.clone().unwrap_or_default(),
                "source_code" => a.source_code.clone().unwrap_or_default(),
                "product_class" => a.product_class.clone().unwrap_or_default(),
                _ => String::new(),
            }
        };
        if !cfg.accepts(value_of) {
            return Ok(Outcome::NotMatched);
        }
        // 預設值（gez，IC5）：只換分群碼、材料類別的值
        let mut src = item.clone();
        for d in &cfg.defaults {
            match d.field.as_str() {
                "group_code" => src.attrs.group_code = Some(d.value.clone()),
                "material_category" => src.attrs.material_category = Some(d.value.clone()),
                _ => {}
            }
        }
        self.check_refs(target, &src).await?;
        let existing = r.items.get_item(target, &item.no).await.map_err(infra)?;
        let mut keys = Vec::new();
        let write = match (existing, cfg.update_existing) {
            (Some(_), false) => return Ok(Outcome::Skipped),
            (None, _) => {
                // 新增：資料所有者為執行拋轉的使用者，確認狀態沿用來源；資料來源為來源營運中心（IC6(b)(c)）
                let mut n = src.clone();
                n.system.data_source = source.as_str().to_owned();
                n.created_by = actor.user_id.clone();
                n.created_dept = actor.dept.clone();
                n.created_on = today;
                n.updated_by = None;
                n.updated_on = None;
                n.stamp = ttrs_domain::record::Stamp::default();
                keys.push((
                    ITEM_LOG.to_owned(),
                    item.no.as_str().to_owned(),
                    CarryAction::Insert,
                ));
                CarryWrite::Insert(n)
            }
            (Some(old), true) => {
                // 覆蓋：保留對方的資料所有者、所有部門、建立時間、確認狀態與版本
                let mut n = old.clone();
                n.attrs = src.attrs.clone();
                n.order_invalid = src.order_invalid;
                n.system.data_source = source.as_str().to_owned();
                n.updated_by = Some(actor.user_id.clone());
                n.updated_on = Some(today);
                keys.push((
                    ITEM_LOG.to_owned(),
                    item.no.as_str().to_owned(),
                    CarryAction::Update,
                ));
                CarryWrite::Update(n)
            }
        };
        let action = match &write {
            CarryWrite::Insert(_) => CarryAction::Insert,
            CarryWrite::Update(_) => CarryAction::Update,
        };
        // 單位換算（smd，IC6(d)）：對方沒有就新增，已有依「更新已存在資料」覆蓋或略過
        let theirs = r
            .conversions
            .list_item_unit_conversions(target, Some(&item.no))
            .await
            .map_err(infra)?;
        let mut conv_writes = Vec::new();
        for c in conversions {
            let key = format!(
                "{}+{}+{}",
                item.no.as_str(),
                c.conversion.from_unit.as_str(),
                c.conversion.to_unit.as_str()
            );
            match theirs.iter().find(|x| {
                x.conversion.from_unit == c.conversion.from_unit
                    && x.conversion.to_unit == c.conversion.to_unit
            }) {
                None => {
                    let mut n = c.clone();
                    n.created_by = actor.user_id.clone();
                    n.created_dept = actor.dept.clone();
                    n.created_on = today;
                    n.updated_by = None;
                    n.updated_on = None;
                    n.stamp = ttrs_domain::record::Stamp::default();
                    keys.push((CONVERSION_LOG.to_owned(), key, CarryAction::Insert));
                    conv_writes.push(CarryWrite::Insert(n));
                }
                Some(old) if cfg.update_existing => {
                    let mut n = old.clone();
                    n.conversion = c.conversion.clone();
                    n.active = c.active;
                    n.updated_by = Some(actor.user_id.clone());
                    n.updated_on = Some(today);
                    keys.push((CONVERSION_LOG.to_owned(), key, CarryAction::Update));
                    conv_writes.push(CarryWrite::Update(n));
                }
                Some(_) => {}
            }
        }
        let batch = ItemCarryBatch {
            item: write,
            texts: texts.to_vec(),
            conversions: conv_writes,
        };
        r.writer
            .apply_item_carry(target, &batch)
            .await
            .map_err(|e| AppError::Infra(e.to_string()))?;
        Ok(Outcome::Written(action, keys))
    }

    /// 參照檢查（s_aimi100_chk_rel_ima06，IC7）：分群碼、產品分類、分類碼、倉管員、申請部門須在對方營運中心存在
    async fn check_refs(&self, target: &PlantId, item: &Item) -> Result<(), AppError> {
        let r = &self.repos;
        let a = &item.attrs;
        let key = item.no.as_str();
        let missing = |field: &str, value: &str| -> AppError {
            DomainError::BasicCarryReferenceMissing {
                item: "item".to_owned(),
                key: key.to_owned(),
                field: field.to_owned(),
                value: value.to_owned(),
            }
            .into()
        };
        if let Some(g) = &a.group_code {
            let ok = match ItemGroupCode::parse(g) {
                Ok(c) => r
                    .groups
                    .get_item_group(target, &c)
                    .await
                    .map_err(infra)?
                    .is_some(),
                Err(_) => false,
            };
            if !ok {
                return Err(missing("group_code", g));
            }
        }
        if let Some(p) = &a.product_class {
            let ok = match ProductClassCode::parse(p) {
                Ok(c) => r
                    .classes
                    .get_product_class(target, &c)
                    .await
                    .map_err(infra)?
                    .is_some(),
                Err(_) => false,
            };
            if !ok {
                return Err(missing("product_class", p));
            }
        }
        let codes = [("8", &a.material_category, "material_category")]
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
        for (kind, value, field) in codes {
            if let Some(v) = value {
                let kind = CodeKind::parse(kind)?;
                let ok = match Code::parse(v) {
                    Ok(c) => r
                        .codes
                        .get_code(target, kind, &c)
                        .await
                        .map_err(infra)?
                        .is_some(),
                    Err(_) => false,
                };
                if !ok {
                    return Err(missing(field, v));
                }
            }
        }
        if let Some(e) = &a.stock_keeper {
            let ok = match EmployeeId::parse(e) {
                Ok(id) => r.employees.get(target, &id).await.map_err(infra)?.is_some(),
                Err(_) => false,
            };
            if !ok {
                return Err(missing("stock_keeper", e));
            }
        }
        if let Some(d) = &a.apply_dept {
            let ok = match DeptCode::parse(d) {
                Ok(c) => r.depts.get(target, &c).await.map_err(infra)?.is_some(),
                Err(_) => false,
            };
            if !ok {
                return Err(missing("apply_dept", d));
            }
        }
        Ok(())
    }

    /// 拋轉歷史（qry_carry_history；aimi100.4gl:1042-1056 開 aooq604 帶入料號）：這筆料件與其單位換算的拋轉紀錄，新到舊
    pub async fn history(
        &self,
        actor: &Actor,
        no: &str,
    ) -> Result<Vec<ttrs_shared::CarryLogDto>, AppError> {
        self.authz
            .authorize(actor, PROGRAM, "qry_carry_history")
            .await?;
        let no = no.trim();
        let prefix = format!("{no}+");
        let mut rows: Vec<CarryLogEntry> = self
            .repos
            .logs
            .list_carry_logs()
            .await
            .map_err(infra)?
            .into_iter()
            .filter(|l| {
                l.data_type == item_type()
                    && ((l.program == ITEM_LOG && l.record_key == no)
                        || (l.program == CONVERSION_LOG && l.record_key.starts_with(&prefix)))
            })
            .collect();
        rows.sort_by_key(|l| std::cmp::Reverse(l.carried_at));
        Ok(rows
            .iter()
            .map(|l| ttrs_shared::CarryLogDto {
                center_code: l.center.as_str().to_owned(),
                data_type: l.data_type.code(),
                plant: l.plant.clone(),
                program: l.program.clone(),
                record_key: l.record_key.clone(),
                action: l.action.code().to_owned(),
                carried_at: l.carried_at,
                carried_by: l.carried_by.clone(),
            })
            .collect())
    }

    /// 子公司收到的料件能否修改、刪除（s_dc_ud_flag，IC9）：回傳（可修改, 可刪除）
    pub async fn rights(&self, plant: &PlantId, item: &Item) -> Result<(bool, bool), AppError> {
        let source = item.system.data_source.trim();
        if source.is_empty() || source == plant.as_str() {
            return Ok((true, true));
        }
        let t = item_type();
        let centers = self
            .repos
            .centers
            .list_group_centers()
            .await
            .map_err(infra)?;
        let Some(center) = centers.iter().find(|c| {
            c.data_types
                .iter()
                .any(|d| d.data_type == t && d.plant == source)
        }) else {
            return Ok((true, true));
        };
        let setting = self
            .repos
            .carries
            .get_carry_setting(&center.code, t)
            .await
            .map_err(infra)?;
        Ok(setting
            .and_then(|s| {
                s.targets
                    .iter()
                    .find(|x| x.plant == plant.as_str())
                    .map(|x| (x.update_existing, x.deletable))
            })
            .unwrap_or((true, true)))
    }

    async fn notify(
        &self,
        setting: &CarrySetting,
        plant: &str,
        item: &Item,
        action: CarryAction,
    ) -> Option<String> {
        let mailer = self.mailer.as_ref()?;
        let to = setting
            .targets
            .iter()
            .find(|t| t.plant == plant)
            .and_then(|t| t.notify.clone())?;
        let lang = ttrs_domain::lang::Lang::default_lang();
        let detail = format!(
            "{} {}：{}",
            crate::report::text(&lang, "basic-carry-item-item"),
            item.no.as_str(),
            crate::report::text(&lang, &format!("carry-history-action-{}", action.code()))
        );
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

    async fn load(&self, plant: &PlantId, no: &str) -> Result<Item, AppError> {
        let no = ItemNo::parse(no).map_err(|_| DomainError::ItemNotFound)?;
        self.repos
            .items
            .get_item(plant, &no)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::ItemNotFound.into())
    }
}

fn infra(e: RepoError) -> AppError {
    match e {
        RepoError::Stale => DomainError::RecordChanged.into(),
        e => AppError::Infra(e.to_string()),
    }
}
