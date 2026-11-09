//! 「狀態」頁籤（docs/adr/0012-change-history.md §9）：資料有效、資料所有者、資料所有部門、建立日期、最後修改者、最後修改日期
//! 統一放在各維護作業的「狀態」頁籤，比照 TipTop 4fd 的 info／Status 頁（例如 tiptop/azz/4fd/p_zx.4fd、tiptop/aim/4fd/aimi100.4fd：
//! xxxuser、xxxmodu、xxxgrup、xxxdate、xxxacti）。
//!
//! - 單筆表單：`StatusForm` 放在「狀態」頁籤內。
//! - 表格（多筆）：表格上方 `Tabs`（資料／狀態）切換欄位，狀態頁用 `StatusHeaders`、`StatusQbeCells`、`StatusCells`。

use dioxus::prelude::*;
use ttrs_shared::{DateTime, NaiveDate, Utc};

use crate::components::history::local_time;

use crate::components::material::{Chip, TextField, active_field_class, active_tone};
use crate::components::qbe::{HINT_DATE, HINT_FLAG, HINT_NUMBER, Q, Qbe};
use crate::i18n::t;

/// 頁籤名稱的翻譯 ID
pub const TAB_STATUS: &str = "common-tab-status";
/// 表格作業「資料」頁籤的翻譯 ID
pub const TAB_DATA: &str = "common-tab-data";

/// 狀態欄位
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusField {
    /// 資料有效（xxxacti）
    Active,
    /// 資料所有者（xxxuser）
    Owner,
    /// 資料所有部門（xxxgrup）
    OwnerDept,
    /// 建立日期
    CreatedOn,
    /// 最後修改者（xxxmodu）
    UpdatedBy,
    /// 最後修改日期（xxxdate）
    UpdatedOn,
    /// 版本號（ADR 0012 §4）
    Version,
}

/// 共同欄位齊全的表
pub const ALL: &[StatusField] = &[
    StatusField::Active,
    StatusField::Owner,
    StatusField::OwnerDept,
    StatusField::CreatedOn,
    StatusField::UpdatedBy,
    StatusField::UpdatedOn,
    StatusField::Version,
];

impl StatusField {
    pub fn label_id(self) -> &'static str {
        match self {
            Self::Active => "common-field-active",
            Self::Owner => "common-field-owner",
            Self::OwnerDept => "common-field-owner-dept",
            Self::CreatedOn => "common-field-created-at",
            Self::UpdatedBy => "common-field-updated-by",
            Self::UpdatedOn => "common-field-updated-at",
            Self::Version => "common-field-version",
        }
    }

    /// 查詢欄位鍵值（與後端 QBE 欄位一致）
    pub fn key(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Owner => "created_by",
            Self::OwnerDept => "created_dept",
            Self::CreatedOn => "created_on",
            Self::UpdatedBy => "updated_by",
            Self::UpdatedOn => "updated_on",
            Self::Version => "version",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Active => HINT_FLAG,
            Self::CreatedOn | Self::UpdatedOn => HINT_DATE,
            Self::Version => HINT_NUMBER,
            _ => "",
        }
    }

    /// TipTop 欄位代號字尾（4fd 的欄位名稱，供畫面元素 id 使用）
    fn suffix(self) -> &'static str {
        match self {
            Self::Active => "acti",
            Self::Owner => "user",
            Self::OwnerDept => "grup",
            Self::CreatedOn => "crat",
            Self::UpdatedBy => "modu",
            Self::UpdatedOn => "date",
            Self::Version => "ver",
        }
    }
}

/// 一筆資料的狀態欄位值
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecordStatus {
    pub active: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: Option<NaiveDate>,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 建立、最後修改時間（到秒，ADR 0012 §1）；以瀏覽器時區顯示
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub version: Option<i32>,
}

impl RecordStatus {
    fn text(&self, f: StatusField) -> String {
        // 一律顯示到時:分:秒；只記了日期的舊資料（ADR 0012 之前修改）顯示當日 00:00:00
        let date = |d: Option<NaiveDate>| d.map(|d| d.format("%Y/%m/%d 00:00:00").to_string());
        match f {
            StatusField::Active => t(if self.active {
                "common-status-active"
            } else {
                "common-status-inactive"
            }),
            StatusField::Owner => self.created_by.clone(),
            StatusField::OwnerDept => self.created_dept.clone(),
            // 建立時間、最後修改時間到秒；舊資料沒有時間時顯示日期
            StatusField::CreatedOn => self
                .created_at
                .map(local_time)
                .or_else(|| date(self.created_on))
                .unwrap_or_default(),
            StatusField::UpdatedBy => self.updated_by.clone().unwrap_or_default(),
            StatusField::UpdatedOn => self
                .updated_at
                .map(local_time)
                .or_else(|| date(self.updated_on))
                .unwrap_or_default(),
            StatusField::Version => self.version.map(|v| v.to_string()).unwrap_or_default(),
        }
    }
}

/// DTO → 狀態欄位
pub trait HasStatus {
    fn status(&self) -> RecordStatus;
}

macro_rules! has_status {
    ($($dto:ty),* $(,)?) => {$(
        impl HasStatus for $dto {
            fn status(&self) -> RecordStatus {
                RecordStatus {
                    active: self.active,
                    created_by: self.created_by.clone(),
                    created_dept: self.created_dept.clone(),
                    created_on: Some(self.created_on),
                    updated_by: self.updated_by.clone(),
                    updated_on: self.updated_on,
                    created_at: self.created_at,
                    updated_at: self.updated_at,
                    version: Some(self.version),
                }
            }
        }
    )*};
}

has_status!(
    ttrs_shared::UserDto,
    ttrs_shared::PermissionClassDto,
    ttrs_shared::PlantDto,
    ttrs_shared::DepartmentDto,
    ttrs_shared::EmployeeDto,
    ttrs_shared::ClassificationCodeDto,
    ttrs_shared::UnitDto,
    ttrs_shared::ProductClassDto,
    ttrs_shared::ItemGroupDto,
    ttrs_shared::ItemDto,
);

macro_rules! has_status_no_active {
    ($($dto:ty),* $(,)?) => {$(
        impl HasStatus for $dto {
            fn status(&self) -> RecordStatus {
                RecordStatus {
                    active: true,
                    created_by: self.created_by.clone(),
                    created_dept: self.created_dept.clone(),
                    created_on: Some(self.created_on),
                    updated_by: self.updated_by.clone(),
                    updated_on: self.updated_on,
                    created_at: self.created_at,
                    updated_at: self.updated_at,
                    version: Some(self.version),
                }
            }
        }
    )*};
}

// 程式權限、使用者特殊權限的單身沒有資料有效碼（zy_file、zxw_file 無 acti 欄位）
has_status_no_active!(
    ttrs_shared::ProgramPermissionDto,
    ttrs_shared::UserPermissionDto
);

/// 程式資料（zz_file）：沒有資料有效碼與建立日期；zzdate 為最後建立／修改日期
impl HasStatus for ttrs_shared::ProgramDto {
    fn status(&self) -> RecordStatus {
        RecordStatus {
            active: true,
            created_by: self.created_by.clone(),
            created_dept: self.created_dept.clone(),
            created_on: None,
            updated_by: self.updated_by.clone(),
            updated_on: Some(self.updated_on),
            created_at: self.created_at,
            updated_at: self.updated_at,
            version: Some(self.version),
        }
    }
}

/// 程式權限、使用者特殊權限單身的欄位
pub const NO_ACTIVE: &[StatusField] = &[
    StatusField::Owner,
    StatusField::OwnerDept,
    StatusField::CreatedOn,
    StatusField::UpdatedBy,
    StatusField::UpdatedOn,
    StatusField::Version,
];

/// 程式資料的欄位（zzuser、zzgrup、zzmodu、zzdate；建立時間與版本為 TTRS 新增）
pub const PROGRAM: &[StatusField] = &[
    StatusField::Owner,
    StatusField::OwnerDept,
    StatusField::CreatedOn,
    StatusField::UpdatedBy,
    StatusField::UpdatedOn,
    StatusField::Version,
];

/// 單筆表單的「狀態」頁籤內容。`queryable` 是後端可查詢的欄位，查詢模式只顯示這些欄位；
/// `prefix` 為 TipTop 表的欄位字首（例如 zx），畫面元素 id 用 `zxuser`、`zxacti`…；`children` 放在最後（作業特有的狀態欄位）
#[component]
pub fn StatusForm(
    status: Option<RecordStatus>,
    #[props(default = ALL)] fields: &'static [StatusField],
    #[props(default)] queryable: &'static [StatusField],
    #[props(default)] query: bool,
    qbe: Option<Qbe>,
    prefix: &'static str,
    #[props(default)] children: Element,
) -> Element {
    let shown: Vec<StatusField> = fields
        .iter()
        .copied()
        .filter(|f| !query || queryable.contains(f))
        .collect();
    rsx! {
        if query && shown.is_empty() {
            p { class: "muted", {t("query-msg-tab-not-queryable")} }
        } else {
            div { class: "form-grid cols-4",
                for f in shown {
                    if let (true, Some(qbe)) = (query, qbe) {
                        Q { key: "{f.key()}", field: f.key(), label: t(f.label_id()), hint: f.hint(), query: true, qbe,
                            div {}
                        }
                    } else {
                        TextField {
                            key: "{f.key()}",
                            id: "{prefix}{f.suffix()}",
                            label: t(f.label_id()),
                            readonly: true,
                            class: if f == StatusField::Active { active_field_class(status.as_ref().map(|s| s.active)) } else { "" },
                            value: status.as_ref().map(|s| s.text(f)).unwrap_or_default(),
                        }
                    }
                }
                if !query {
                    {children}
                }
            }
        }
    }
}

/// 表格「狀態」頁的欄位標題
#[component]
pub fn StatusHeaders(#[props(default = ALL)] fields: &'static [StatusField]) -> Element {
    rsx! {
        for f in fields.iter().copied() {
            th { key: "{f.key()}", {t(f.label_id())} }
        }
    }
}

/// 表格「狀態」頁的查詢列：可查詢的欄位放輸入框，其他欄位留空
#[component]
pub fn StatusQbeCells(
    #[props(default = ALL)] fields: &'static [StatusField],
    #[props(default)] queryable: &'static [StatusField],
    qbe: Qbe,
) -> Element {
    rsx! {
        for f in fields.iter().copied() {
            th { key: "{f.key()}",
                if queryable.contains(&f) {
                    Q { field: f.key(), label: t(f.label_id()), hint: f.hint(), query: true, qbe, div {} }
                }
            }
        }
    }
}

/// 表格「狀態」頁的欄位值；資料有效以色塊顯示
#[component]
pub fn StatusCells(
    status: RecordStatus,
    #[props(default = ALL)] fields: &'static [StatusField],
) -> Element {
    rsx! {
        for f in fields.iter().copied() {
            if f == StatusField::Active {
                td { key: "{f.key()}",
                    Chip { label: status.text(f), tone: active_tone(status.active) }
                }
            } else {
                td { key: "{f.key()}", class: if matches!(f, StatusField::Owner | StatusField::OwnerDept | StatusField::UpdatedBy) { "mono" }, {status.text(f)} }
            }
        }
    }
}
