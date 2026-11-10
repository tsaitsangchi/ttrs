//! 基本代碼：結構相同的單表代碼作業共用一套規則（代號、名稱、隸屬上層代碼、資料有效、確認）。
//!
//! | 種類 | TipTop 程式 / 資料表 | 規格 |
//! |---|---|---|
//! | 區域別 Region | aooi100 / gea_file | region_country_area_maintenance.feature（RC1–RC7） |
//! | 國別 Country（隸屬區域 geb03） | aooi090 / geb_file | 同上 |
//! | 地區 Area（隸屬國別 geo03） | aooi110 / geo_file | 同上 |
//! | 交運方式 ShippingMethod | aooi160 / ged_file | shipping_method_maintenance.feature（SM1–SM6） |
//! | 報表抬頭 ReportTitle（名稱多語言） | aooi210 / azh_file | report_title_maintenance.feature（RH1–RH6） |
//!
//! 全部放共用 schema（RC2、SM2、RH3）。代號最多 10 字、區分大小寫、建立後不可修改；名稱最多 80 字。

use std::fmt;

use chrono::NaiveDate;

use crate::{DomainError, confirmation::Confirmation, lang::Lang, record::Stamp};

/// 代號最多 10 字（gea01、geb01、geo01、ged01、azh01 皆為 varchar2(10)）
pub const CODE_MAX: usize = 10;
/// 名稱最多 80 字（gea02、geb02、geo02、ged02、azh02）
pub const NAME_MAX: usize = 80;

/// 基本代碼的種類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BasicCodeKind {
    /// 區域別（aooi100 / gea_file）
    Region,
    /// 國別（aooi090 / geb_file）
    Country,
    /// 地區（aooi110 / geo_file）
    Area,
    /// 交運方式（aooi160 / ged_file）
    ShippingMethod,
    /// 報表抬頭（aooi210 / azh_file）
    ReportTitle,
}

impl BasicCodeKind {
    pub const ALL: [Self; 5] = [
        Self::Region,
        Self::Country,
        Self::Area,
        Self::ShippingMethod,
        Self::ReportTitle,
    ];

    /// 權限檢查用的程式代號（RC1、SM1、RH1）
    pub fn program(self) -> &'static str {
        match self {
            Self::Region => "region-maintenance",
            Self::Country => "country-maintenance",
            Self::Area => "area-maintenance",
            Self::ShippingMethod => "shipping-method-maintenance",
            Self::ReportTitle => "report-title-maintenance",
        }
    }

    /// API 路徑用的名稱（/api/basic-codes/{slug}）
    pub fn slug(self) -> &'static str {
        match self {
            Self::Region => "regions",
            Self::Country => "countries",
            Self::Area => "areas",
            Self::ShippingMethod => "shipping-methods",
            Self::ReportTitle => "report-titles",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.slug() == slug)
    }

    pub fn from_program(program: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.program() == program)
    }

    /// 隸屬的上層代碼：國別隸屬區域（geb03，aooi090.4gl:260-267）、地區隸屬國別（geo03，aooi110.4gl:239-246）
    pub fn parent(self) -> Option<Self> {
        match self {
            Self::Country => Some(Self::Region),
            Self::Area => Some(Self::Country),
            _ => None,
        }
    }

    /// 引用本種類的下層代碼（刪除檢查，RC4）
    pub fn child(self) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.parent() == Some(self))
    }

    /// 名稱是否多語言（RH4：只有報表抬頭）
    pub fn multilingual(self) -> bool {
        self == Self::ReportTitle
    }

    /// 錯誤代碼的前綴（DomainError::code）
    pub fn error_prefix(self) -> &'static str {
        match self {
            Self::Region => "REGION",
            Self::Country => "COUNTRY",
            Self::Area => "AREA",
            Self::ShippingMethod => "SHIPPING_METHOD",
            Self::ReportTitle => "REPORT_TITLE",
        }
    }
}

/// 錯誤訊息中的名稱
impl fmt::Display for BasicCodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Region => "區域",
            Self::Country => "國別",
            Self::Area => "地區",
            Self::ShippingMethod => "交運方式",
            Self::ReportTitle => "報表抬頭",
        })
    }
}

/// 代號：去頭尾空白、區分大小寫、1–10 字（RC5、SM5）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BasicCodeValue(String);

impl BasicCodeValue {
    pub fn parse(kind: BasicCodeKind, raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::BasicCodeEmpty(kind));
        }
        if v.chars().count() > CODE_MAX {
            return Err(DomainError::BasicCodeTooLong {
                kind,
                max: CODE_MAX,
            });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 名稱的其他語系（RH4；zh-TW 存在 name）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicCodeName {
    pub lang: Lang,
    pub name: String,
}

/// 可修改的欄位
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BasicCodeAttrs {
    /// 名稱（繁中，翻譯來源）
    pub name: Option<String>,
    /// 隸屬的上層代號（只有國別、地區；可空白，RC3）
    pub parent: Option<String>,
    /// 名稱的其他語系（只有報表抬頭，RH4）
    pub names: Vec<BasicCodeName>,
}

impl BasicCodeAttrs {
    /// 名稱最多 80 字；沒有上層的種類不可帶隸屬代號；不是多語言的種類忽略其他語系
    pub fn new(
        kind: BasicCodeKind,
        name: Option<String>,
        parent: Option<String>,
        names: Vec<(Lang, String)>,
    ) -> Result<Self, DomainError> {
        let clean = |v: Option<String>| v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        let name = clean(name);
        let too_long = |n: &str| n.chars().count() > NAME_MAX;
        if name.as_deref().is_some_and(too_long) {
            return Err(DomainError::FieldTooLong {
                field: "名稱",
                max: NAME_MAX,
            });
        }
        let parent = match kind.parent() {
            Some(_) => clean(parent),
            None => None,
        };
        let mut texts = Vec::new();
        if kind.multilingual() {
            for (lang, text) in names {
                let text = text.trim().to_owned();
                if lang.is_default() || text.is_empty() {
                    continue;
                }
                if too_long(&text) {
                    return Err(DomainError::FieldTooLong {
                        field: "名稱",
                        max: NAME_MAX,
                    });
                }
                texts.retain(|t: &BasicCodeName| t.lang != lang);
                texts.push(BasicCodeName { lang, name: text });
            }
            texts.sort_by(|a, b| a.lang.cmp(&b.lang));
        }
        Ok(Self {
            name,
            parent,
            names: texts,
        })
    }

    /// 顯示名稱，退回規則（ADR 0005）：指定語系 → zh-TW → 空白
    pub fn name_in(&self, lang: &Lang) -> Option<String> {
        if !lang.is_default()
            && let Some(t) = self.names.iter().find(|t| &t.lang == lang)
        {
            return Some(t.name.clone());
        }
        self.name.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicCode {
    pub kind: BasicCodeKind,
    pub code: BasicCodeValue,
    pub attrs: BasicCodeAttrs,
    /// 資料有效碼（geaacti、gebacti、geoacti；ged_file、azh_file 沒有，TTRS 加上，SM3、RH5）
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9）
    pub confirmation: Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl BasicCode {
    /// 新增：資料有效預設 Y（aooi090.4gl:220 等）、未確認
    pub fn new(
        kind: BasicCodeKind,
        code: BasicCodeValue,
        attrs: BasicCodeAttrs,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            kind,
            code,
            attrs,
            active: true,
            confirmation: Confirmation::default(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn update(&mut self, attrs: BasicCodeAttrs, user: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(user, today);
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製（ADR 0013 SA5）：新代號，名稱與隸屬帶入；資料所有者為目前使用者、未確認
    pub fn copy_as(&self, code: BasicCodeValue, user: &str, dept: &str, today: NaiveDate) -> Self {
        Self::new(self.kind, code, self.attrs.clone(), user, dept, today)
    }

    /// 確認／取消確認（ADR 0013 SA9）
    pub fn set_confirmed(
        &mut self,
        confirm: bool,
        user: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), DomainError> {
        if confirm {
            self.confirmation.confirm(user, now)?;
        } else {
            self.confirmation.unconfirm()?;
        }
        self.touch(user, now.date_naive());
        Ok(())
    }

    /// 隸屬的上層代碼須存在且有效（aooi090.4gl:260-267、aooi110.4gl:239-246：aoo-061、9028）
    pub fn check_parent(
        kind: BasicCodeKind,
        parent: Option<&BasicCode>,
    ) -> Result<(), DomainError> {
        let Some(pk) = kind.parent() else {
            return Ok(());
        };
        match parent {
            None => Err(DomainError::BasicCodeNotFound(pk)),
            Some(p) if !p.active => Err(DomainError::BasicCodeInactive(pk)),
            Some(_) => Ok(()),
        }
    }

    fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(BasicCodeKind::Country.parent(), Some(BasicCodeKind::Region));
        assert_eq!(BasicCodeKind::Region.child(), Some(BasicCodeKind::Country));
        assert_eq!(BasicCodeKind::Area.child(), None);
        for k in BasicCodeKind::ALL {
            assert_eq!(BasicCodeKind::from_slug(k.slug()), Some(k));
            assert_eq!(BasicCodeKind::from_program(k.program()), Some(k));
        }
    }

    #[test]
    fn code_rules() {
        let k = BasicCodeKind::ShippingMethod;
        assert_eq!(BasicCodeValue::parse(k, " Air ").unwrap().as_str(), "Air");
        assert_eq!(
            BasicCodeValue::parse(k, ""),
            Err(DomainError::BasicCodeEmpty(k))
        );
        assert_eq!(
            BasicCodeValue::parse(k, "AIRFREIGHT1"),
            Err(DomainError::BasicCodeTooLong { kind: k, max: 10 })
        );
    }

    #[test]
    fn names_fall_back_to_zh_tw() {
        let en = Lang::parse("en").unwrap();
        let cn = Lang::parse("zh-CN").unwrap();
        let a = BasicCodeAttrs::new(
            BasicCodeKind::ReportTitle,
            Some("資產負債表".into()),
            None,
            vec![(en.clone(), "Balance Sheet".into())],
        )
        .unwrap();
        assert_eq!(a.name_in(&en).as_deref(), Some("Balance Sheet"));
        assert_eq!(a.name_in(&cn).as_deref(), Some("資產負債表"));
        // 不是多語言的種類忽略其他語系、沒有上層的種類忽略隸屬
        let r = BasicCodeAttrs::new(
            BasicCodeKind::Region,
            Some("亞洲".into()),
            Some("X".into()),
            vec![(en, "Asia".into())],
        )
        .unwrap();
        assert!(r.names.is_empty() && r.parent.is_none());
    }
}
