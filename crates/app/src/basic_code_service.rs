//! 基本代碼維護 use case：區域別（aooi100 / gea_file）、國別（aooi090 / geb_file）、地區（aooi110 / geo_file）、
//! 交運方式（aooi160 / ged_file）、報表抬頭（aooi210 / azh_file）。
//! 規格：specs/features/aoo/region_country_area_maintenance.feature（RC1–RC7）、shipping_method_maintenance.feature（SM1–SM6）、
//! report_title_maintenance.feature（RH1–RH6）。
//!
//! 共用 schema。TipTop 皆為多筆單身維護：查詢 query、維護 detail（新增列 add_row、刪除列 delete_row）；
//! 依 ADR 0013 補上複製 reproduce、確認 confirm／取消確認 notconfirm、列印 output、匯出 Excel exporttoexcel、異動記錄 history。

use std::{collections::HashMap, sync::Arc};

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    basic_code::{BasicCode, BasicCodeAttrs, BasicCodeKind, BasicCodeValue},
    lang::Lang,
};
use ttrs_shared::{
    BasicCodeDto, BasicCodeInput, BasicCodeKindDto, BasicCodeNameDto, Page, QbeCondition,
    QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::BasicCodeRepository,
    query,
    report::{self, ReportData, ReportFormat},
};

/// 資料表、鍵值欄位、隸屬欄位（PostgreSQL 與記憶體版的異動記錄共用；migrations/0041_basic_codes.sql）
pub fn storage(kind: BasicCodeKind) -> (&'static str, &'static str, Option<&'static str>) {
    match kind {
        BasicCodeKind::Region => ("region", "region_code", None),
        BasicCodeKind::Country => ("country", "country_code", Some("region_code")),
        BasicCodeKind::Area => ("area", "area_code", Some("country_code")),
        BasicCodeKind::ShippingMethod => ("shipping_method", "method_code", None),
        BasicCodeKind::ReportTitle => ("report_title", "title_code", None),
    }
}

pub fn kind_of(dto: BasicCodeKindDto) -> BasicCodeKind {
    match dto {
        BasicCodeKindDto::Region => BasicCodeKind::Region,
        BasicCodeKindDto::Country => BasicCodeKind::Country,
        BasicCodeKindDto::Area => BasicCodeKind::Area,
        BasicCodeKindDto::ShippingMethod => BasicCodeKind::ShippingMethod,
        BasicCodeKindDto::ReportTitle => BasicCodeKind::ReportTitle,
    }
}

/// 清單報表（ADR 0013 SA3）：代號、名稱、隸屬（國別、地區）、資料有效
fn report_spec(kind: BasicCodeKind) -> &'static ListSpec {
    const REGION: ListSpec = ListSpec {
        program: "region-maintenance",
        title: "region-report-title",
        columns: &[
            ("code", "region-col-code"),
            ("name", "region-col-name"),
            ("active", "basic-code-col-active"),
        ],
    };
    const COUNTRY: ListSpec = ListSpec {
        program: "country-maintenance",
        title: "country-report-title",
        columns: &[
            ("code", "country-col-code"),
            ("name", "country-col-name"),
            ("parent", "country-col-parent"),
            ("active", "basic-code-col-active"),
        ],
    };
    const AREA: ListSpec = ListSpec {
        program: "area-maintenance",
        title: "area-report-title",
        columns: &[
            ("code", "area-col-code"),
            ("name", "area-col-name"),
            ("parent", "area-col-parent"),
            ("active", "basic-code-col-active"),
        ],
    };
    const SHIPPING: ListSpec = ListSpec {
        program: "shipping-method-maintenance",
        title: "shipping-method-report-title",
        columns: &[
            ("code", "shipping-method-col-code"),
            ("name", "shipping-method-col-name"),
            ("active", "basic-code-col-active"),
        ],
    };
    const TITLE: ListSpec = ListSpec {
        program: "report-title-maintenance",
        title: "report-title-report-title",
        columns: &[
            ("code", "report-title-col-code"),
            ("name", "report-title-col-name"),
            ("active", "basic-code-col-active"),
        ],
    };
    match kind {
        BasicCodeKind::Region => &REGION,
        BasicCodeKind::Country => &COUNTRY,
        BasicCodeKind::Area => &AREA,
        BasicCodeKind::ShippingMethod => &SHIPPING,
        BasicCodeKind::ReportTitle => &TITLE,
    }
}

#[derive(Clone)]
pub struct BasicCodeService {
    authz: Arc<dyn Authorizer>,
    codes: Arc<dyn BasicCodeRepository>,
    reports: ListReporter,
}

impl BasicCodeService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        codes: Arc<dyn BasicCodeRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            codes,
            reports,
        }
    }

    /// QBE 查詢（ADR 0013 SA1）：依代號排序並分頁；名稱依 req.lang 顯示
    pub async fn query(
        &self,
        actor: &Actor,
        kind: BasicCodeKind,
        req: QueryRequest,
    ) -> Result<Page<BasicCodeDto>, AppError> {
        self.authorize(actor, kind, "query").await?;
        let filters = query::basic_code::filters(&req.conditions)?;
        let p = query::paging(&req);
        let lang = lang_of(req.lang.as_deref())?;
        let all = self.codes.list_basic_codes(kind).await.map_err(infra)?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::basic_code::value, p);
        let parents = self.parent_names(kind, &lang).await?;
        Ok(Page {
            items: rows.iter().map(|c| to_dto(c, &lang, &parents)).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 全部代碼（畫面下拉選單、開窗 q_gea、q_geb 用），依代號排序
    pub async fn list(
        &self,
        actor: &Actor,
        kind: BasicCodeKind,
        lang: Option<&str>,
    ) -> Result<Vec<BasicCodeDto>, AppError> {
        self.authorize(actor, kind, "query").await?;
        let lang = lang_of(lang)?;
        let parents = self.parent_names(kind, &lang).await?;
        Ok(self
            .codes
            .list_basic_codes(kind)
            .await
            .map_err(infra)?
            .iter()
            .map(|c| to_dto(c, &lang, &parents))
            .collect())
    }

    pub async fn get(
        &self,
        actor: &Actor,
        kind: BasicCodeKind,
        code: &str,
        lang: Option<&str>,
    ) -> Result<BasicCodeDto, AppError> {
        self.authorize(actor, kind, "query").await?;
        let lang = lang_of(lang)?;
        self.dto(&self.load(kind, code).await?, &lang).await
    }

    /// 列印 output、匯出 Excel exporttoexcel（ADR 0013 SA3；TipTop 有 output、exporttoexcel）
    pub async fn report_in(
        &self,
        actor: &Actor,
        kind: BasicCodeKind,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, kind, format.action()).await?;
        let filters = query::basic_code::filters(&conditions)?;
        let all = self.codes.list_basic_codes(kind).await.map_err(infra)?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::basic_code::value, p);
        self.reports.check(total)?;
        let parents = self.parent_names(kind, lang).await?;
        let t = |id: &str| report::text(lang, id);
        let lines = rows
            .iter()
            .map(|c| {
                let mut line = vec![
                    c.code.as_str().to_owned(),
                    c.attrs.name_in(lang).unwrap_or_default(),
                ];
                if kind.parent().is_some() {
                    line.push(parent_label(c.attrs.parent.as_deref(), &parents));
                }
                line.push(t(if c.active {
                    "report-active"
                } else {
                    "report-inactive"
                }));
                line
            })
            .collect();
        self.reports
            .build(
                actor,
                None,
                report_spec(kind),
                lines,
                &conditions,
                format,
                lang,
            )
            .await
    }

    /// 測試用：調整同步產生報表的筆數上限
    pub fn with_report_limit(mut self, limit: u64) -> Self {
        self.reports.limit = limit;
        self
    }

    /// 新增（aooi100.4gl:216-230、aooi090.4gl:232-259、aooi110.4gl:224-238、aooi160.4gl:229-242、aooi210.4gl:266-280）：
    /// 資料有效 Y；代號不可重複（-239）；隸屬代碼須存在且有效（aoo-061、9028）
    pub async fn create(
        &self,
        actor: &Actor,
        kind: BasicCodeKind,
        input: BasicCodeInput,
    ) -> Result<BasicCodeDto, AppError> {
        crate::change_log::run(actor, kind.program(), "detail", async {
            self.authorize(actor, kind, "detail").await?.add_row()?;
            let code = BasicCodeValue::parse(kind, &input.code)?;
            let attrs = attrs_of(kind, input)?;
            self.check_parent(kind, attrs.parent.as_deref()).await?;
            let c = BasicCode::new(kind, code, attrs, &actor.user_id, &actor.dept, today());
            self.insert(&c).await
        })
        .await
    }

    /// 修改名稱、隸屬；代號建立後不可修改（RC5）。隸屬有變更時才檢查（TipTop AFTER FIELD）
    pub async fn update(
        &self,
        actor: &Actor,
        kind: BasicCodeKind,
        code: &str,
        input: BasicCodeInput,
    ) -> Result<BasicCodeDto, AppError> {
        crate::change_log::run(actor, kind.program(), "detail", async {
            self.authorize(actor, kind, "detail").await?;
            let mut c = self.load(kind, code).await?;
            c.stamp.check(input.version)?;
            let attrs = attrs_of(kind, input)?;
            if attrs.parent != c.attrs.parent {
                self.check_parent(kind, attrs.parent.as_deref()).await?;
            }
            c.update(attrs, &actor.user_id, today());
            self.save(&c).await
        })
        .await
    }

    /// 停用／啟用（資料有效碼）
    pub async fn set_active(
        &self,
        actor: &Actor,
        kind: BasicCodeKind,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<BasicCodeDto, AppError> {
        crate::change_log::run(actor, kind.program(), "detail", async {
            self.authorize(actor, kind, "detail").await?;
            let mut c = self.load(kind, code).await?;
            c.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            c.set_active(active, &actor.user_id, today());
            self.save(&c).await
        })
        .await
    }

    /// 複製（ADR 0013 SA5）：名稱與隸屬帶入；隸屬代碼須仍有效
    pub async fn copy(
        &self,
        actor: &Actor,
        kind: BasicCodeKind,
        code: &str,
        new_code: &str,
    ) -> Result<BasicCodeDto, AppError> {
        crate::change_log::run(actor, kind.program(), "reproduce", async {
            self.authorize(actor, kind, "reproduce").await?;
            let source = self.load(kind, code).await?;
            self.check_parent(kind, source.attrs.parent.as_deref())
                .await?;
            let c = source.copy_as(
                BasicCodeValue::parse(kind, new_code)?,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            self.insert(&c).await
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        kind: BasicCodeKind,
        code: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<BasicCodeDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, kind.program(), action, async {
            self.authorize(actor, kind, action).await?;
            let mut c = self.load(kind, code).await?;
            c.stamp.check(version)?;
            c.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&c).await
        })
        .await
    }

    /// 刪除：已確認不可刪除（SA9-2）；被下層代碼引用時拒絕，請改為停用（RC4；TipTop 不檢查）
    pub async fn delete(
        &self,
        actor: &Actor,
        kind: BasicCodeKind,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, kind.program(), "detail", async {
            self.authorize(actor, kind, "detail").await?.delete_row()?;
            let c = self.load(kind, code).await?;
            c.stamp.check(version)?;
            // 先檢查引用再檢查確認，與單位、權限類別一致
            if let Some(child) = kind.child() {
                let used = self
                    .codes
                    .list_basic_codes(child)
                    .await
                    .map_err(infra)?
                    .iter()
                    .any(|x| x.attrs.parent.as_deref() == Some(c.code.as_str()));
                if used {
                    return Err(DomainError::BasicCodeInUse(kind).into());
                }
            }
            c.confirmation.check_deletable()?;
            match self.codes.delete_basic_code(kind, &c.code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::BasicCodeNotFound(kind).into()),
                Err(RepoError::InUse) => Err(DomainError::BasicCodeInUse(kind).into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    async fn check_parent(
        &self,
        kind: BasicCodeKind,
        parent: Option<&str>,
    ) -> Result<(), AppError> {
        let (Some(pk), Some(parent)) = (kind.parent(), parent) else {
            return Ok(());
        };
        let found = match BasicCodeValue::parse(pk, parent) {
            Ok(code) => self.codes.get_basic_code(pk, &code).await.map_err(infra)?,
            Err(_) => None,
        };
        Ok(BasicCode::check_parent(kind, found.as_ref())?)
    }

    async fn insert(&self, c: &BasicCode) -> Result<BasicCodeDto, AppError> {
        match self.codes.insert_basic_code(c).await {
            Ok(()) => {
                let saved = self.load(c.kind, c.code.as_str()).await?;
                self.dto(&saved, &Lang::default_lang()).await
            }
            Err(RepoError::Conflict) => Err(DomainError::BasicCodeAlreadyExists(c.kind).into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, c: &BasicCode) -> Result<BasicCodeDto, AppError> {
        self.codes.update_basic_code(c).await.map_err(infra)?;
        let saved = self.load(c.kind, c.code.as_str()).await?;
        self.dto(&saved, &Lang::default_lang()).await
    }

    async fn load(&self, kind: BasicCodeKind, code: &str) -> Result<BasicCode, AppError> {
        let code =
            BasicCodeValue::parse(kind, code).map_err(|_| DomainError::BasicCodeNotFound(kind))?;
        self.codes
            .get_basic_code(kind, &code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::BasicCodeNotFound(kind).into())
    }

    async fn dto(&self, c: &BasicCode, lang: &Lang) -> Result<BasicCodeDto, AppError> {
        let parents = self.parent_names(c.kind, lang).await?;
        Ok(to_dto(c, lang, &parents))
    }

    /// 上層代碼 → 名稱（顯示隸屬名稱用）
    async fn parent_names(
        &self,
        kind: BasicCodeKind,
        lang: &Lang,
    ) -> Result<HashMap<String, String>, AppError> {
        let Some(pk) = kind.parent() else {
            return Ok(HashMap::new());
        };
        Ok(self
            .codes
            .list_basic_codes(pk)
            .await
            .map_err(infra)?
            .into_iter()
            .filter_map(|p| {
                let name = p.attrs.name_in(lang)?;
                Some((p.code.as_str().to_owned(), name))
            })
            .collect())
    }

    async fn authorize(
        &self,
        actor: &Actor,
        kind: BasicCodeKind,
        action: &'static str,
    ) -> Result<Grant, AppError> {
        self.authz.authorize(actor, kind.program(), action).await
    }
}

fn attrs_of(kind: BasicCodeKind, input: BasicCodeInput) -> Result<BasicCodeAttrs, AppError> {
    let mut names = Vec::new();
    for n in input.names {
        names.push((Lang::parse(&n.lang)?, n.name));
    }
    Ok(BasicCodeAttrs::new(kind, input.name, input.parent, names)?)
}

fn lang_of(lang: Option<&str>) -> Result<Lang, AppError> {
    Ok(match lang {
        Some(l) if !l.trim().is_empty() => Lang::parse(l)?,
        _ => Lang::default_lang(),
    })
}

fn parent_label(parent: Option<&str>, names: &HashMap<String, String>) -> String {
    match parent {
        None => String::new(),
        Some(p) => match names.get(p) {
            Some(n) => format!("{p} {n}"),
            None => p.to_owned(),
        },
    }
}

fn to_dto(c: &BasicCode, lang: &Lang, parents: &HashMap<String, String>) -> BasicCodeDto {
    let mut names = Vec::new();
    if c.kind.multilingual() {
        if let Some(n) = &c.attrs.name {
            names.push(BasicCodeNameDto {
                lang: Lang::default_lang().as_str().to_owned(),
                name: n.clone(),
            });
        }
        names.extend(c.attrs.names.iter().map(|t| BasicCodeNameDto {
            lang: t.lang.as_str().to_owned(),
            name: t.name.clone(),
        }));
    }
    BasicCodeDto {
        code: c.code.as_str().to_owned(),
        name: c.attrs.name_in(lang),
        parent: c.attrs.parent.clone(),
        parent_name: c
            .attrs
            .parent
            .as_ref()
            .and_then(|p| parents.get(p).cloned()),
        names,
        active: c.active,
        confirmed: c.confirmation.confirmed,
        confirmed_by: c.confirmation.by.clone(),
        confirmed_at: c.confirmation.at,
        created_by: c.created_by.clone(),
        created_dept: c.created_dept.clone(),
        created_on: c.created_on,
        updated_by: c.updated_by.clone(),
        updated_on: c.updated_on,
        version: c.stamp.version,
        created_at: c.stamp.created_at,
        updated_at: c.stamp.updated_at,
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
