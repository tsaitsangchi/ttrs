//! 編碼類別（TipTop: gei_file 編碼類別資料單頭檔、gel_file 編碼類別資料單身檔；aooi402 編碼類別資料維護作業）
//! 規格：specs/features/aoo/encoding_class.feature（EC1–EC11）
//!
//! 一個類別由多「段」組成：固定值、獨立段、流水號、欄位值、年、季、月、週、日（gel04）。
//! 各型態的檢查需要外部資料（性質的類別、獨立段長度、年對照的長度），由應用層查好放進 [`SegmentContext`]。

use chrono::NaiveDate;

use crate::{error::DomainError, record::Stamp};

/// 編碼類別最多 10 字（gei01 varchar2(10)，EC3）
pub const CODE_MAX: usize = 10;
/// 名稱、段次說明最多 80 字（gei02、gel06）
pub const NAME_MAX: usize = 80;
/// 每段長度上限（EC4）
pub const LENGTH_MAX: i16 = 30;
/// 季、月、週、日固定 2 碼（:974-981，aoo-121）
const DATE_LENGTH: i16 = 2;
/// 固定值不可含的字元（:1019 aoo-111；TTRS 改為含有就擋並加上空白，EC5）
const INVALID_CHARS: [char; 5] = ['*', '%', '_', '?', ' '];

/// 編碼類別代碼：去頭尾空白、區分大小寫、1–10 字（EC3）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassCode(String);

impl ClassCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::EncodingClassCodeEmpty);
        }
        if v.chars().count() > CODE_MAX {
            return Err(DomainError::EncodingClassCodeTooLong { max: CODE_MAX });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 段的型態 gel04（aooi402.per ITEMS 1–9）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SegmentType {
    /// 1 固定值
    Fixed,
    /// 2 獨立段（aooi403）
    Independent,
    /// 3 流水號
    Serial,
    /// 4 欄位值（料件主檔欄位；第一版不開放，EC6）
    Field,
    /// 5 年（aooi404）
    Year,
    /// 6 季
    Quarter,
    /// 7 月
    Month,
    /// 8 週
    Week,
    /// 9 日
    Day,
}

impl SegmentType {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        Ok(match raw.trim() {
            "1" => Self::Fixed,
            "2" => Self::Independent,
            "3" => Self::Serial,
            "4" => Self::Field,
            "5" => Self::Year,
            "6" => Self::Quarter,
            "7" => Self::Month,
            "8" => Self::Week,
            "9" => Self::Day,
            _ => return Err(DomainError::EncodingClassTypeInvalid),
        })
    }

    /// TipTop gel04 的值
    pub fn code(self) -> &'static str {
        match self {
            Self::Fixed => "1",
            Self::Independent => "2",
            Self::Serial => "3",
            Self::Field => "4",
            Self::Year => "5",
            Self::Quarter => "6",
            Self::Month => "7",
            Self::Week => "8",
            Self::Day => "9",
        }
    }

    /// 只有批號、序號性質可用的型態（:959-963 aic-048）
    fn lot_serial_only(self) -> bool {
        matches!(
            self,
            Self::Field | Self::Year | Self::Quarter | Self::Month | Self::Week | Self::Day
        )
    }

    fn is_date_part(self) -> bool {
        matches!(self, Self::Quarter | Self::Month | Self::Week | Self::Day)
    }
}

/// 檢查各段需要的外部資料（應用層查好）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SegmentContext {
    /// 類別的性質是批號或序號（編碼性質類別 5、6）
    pub lot_or_serial: bool,
    /// 存在且有效的獨立段與其長度（aooi403）
    pub independent_segments: Vec<(String, i16)>,
    /// 編碼年對照（aooi404）中有效資料的長度
    pub year_lengths: Vec<i16>,
}

/// 新增或修改一段時輸入的內容
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SegmentInput {
    pub description: Option<String>,
    pub length: Option<i16>,
    pub kind: String,
    /// 固定值或獨立段編號（gel05）
    pub value: Option<String>,
    pub show_name: bool,
}

/// 單身一段（gel02–gel07）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassSegment {
    /// gel02 段次
    pub no: i32,
    /// gel06 段次說明
    pub description: Option<String>,
    /// gel03 長度
    pub length: i16,
    /// gel04 型態
    pub kind: SegmentType,
    /// gel05 固定值（型態 1）或獨立段編號（型態 2）；其他型態為空
    pub value: Option<String>,
    /// gel07 名稱顯示否
    pub show_name: bool,
}

fn text_of(raw: Option<String>, field: &'static str) -> Result<Option<String>, DomainError> {
    let v = raw.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    if v.as_ref().is_some_and(|n| n.chars().count() > NAME_MAX) {
        return Err(DomainError::FieldTooLong {
            field,
            max: NAME_MAX,
        });
    }
    Ok(v)
}

/// 單頭可修改的欄位：名稱、性質
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassAttrs {
    pub name: String,
    /// gei03 性質（aooi401 的編碼性質代碼）；存在且有效由應用層檢查（aoo-131）
    pub nature: String,
}

impl ClassAttrs {
    pub fn new(name: Option<String>, nature: &str) -> Result<Self, DomainError> {
        let name = text_of(name, "名稱")?.ok_or(DomainError::EncodingClassNameEmpty)?;
        let nature = nature.trim();
        if nature.is_empty() {
            return Err(DomainError::EncodingClassNatureInvalid);
        }
        Ok(Self {
            name,
            nature: nature.to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodingClass {
    pub code: ClassCode,
    pub attrs: ClassAttrs,
    /// 單身，依段次排序
    pub segments: Vec<ClassSegment>,
    /// geiacti 資料有效碼
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；gei_file 無此欄）
    pub confirmation: crate::confirmation::Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl EncodingClass {
    /// 新增：資料有效 Y（:351-395）
    pub fn new(
        code: ClassCode,
        attrs: ClassAttrs,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            code,
            attrs,
            segments: Vec::new(),
            active: true,
            confirmation: crate::confirmation::Confirmation::default(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    /// 總段數（gei04，由單身算出，EC3）
    pub fn segment_count(&self) -> i32 {
        i32::try_from(self.segments.len()).unwrap_or(i32::MAX)
    }

    /// 總長度（gei05，由單身算出，EC3）
    pub fn total_length(&self) -> i32 {
        self.segments.iter().map(|s| i32::from(s.length)).sum()
    }

    fn check_active(&self) -> Result<(), DomainError> {
        if self.active {
            Ok(())
        } else {
            Err(DomainError::EncodingClassInactive)
        }
    }

    /// 修改名稱、性質（:397-459）：停用的不可修改；改性質時既有各段須仍符合新性質（EC5）
    pub fn update(
        &mut self,
        attrs: ClassAttrs,
        ctx: &SegmentContext,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        for s in &self.segments {
            check_allowed(s.kind, ctx)?;
        }
        self.attrs = attrs;
        self.touch(user, today);
        Ok(())
    }

    /// 依型態檢查一段（EC5），回傳整理後的段；`except` 為正在修改的段次
    fn build(
        &self,
        no: i32,
        input: SegmentInput,
        ctx: &SegmentContext,
        except: Option<i32>,
    ) -> Result<ClassSegment, DomainError> {
        let kind = SegmentType::parse(&input.kind)?;
        check_allowed(kind, ctx)?;
        if kind == SegmentType::Field {
            return Err(DomainError::EncodingClassTypeNotSupported);
        }
        let value = input
            .value
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        let length = match kind {
            SegmentType::Independent => {
                let code = value
                    .clone()
                    .ok_or(DomainError::EncodingClassSegmentInvalid)?;
                ctx.independent_segments
                    .iter()
                    .find(|(c, _)| *c == code)
                    .map(|(_, l)| *l)
                    .ok_or(DomainError::EncodingClassSegmentInvalid)?
            }
            k if k.is_date_part() => match input.length {
                None => DATE_LENGTH,
                Some(DATE_LENGTH) => DATE_LENGTH,
                Some(_) => return Err(DomainError::EncodingClassDateLengthInvalid),
            },
            _ => match input.length {
                Some(l) if (1..=LENGTH_MAX).contains(&l) => l,
                _ => {
                    return Err(DomainError::EncodingClassSegmentLengthInvalid { max: LENGTH_MAX });
                }
            },
        };
        let value = match kind {
            SegmentType::Fixed => {
                let v = value.ok_or(DomainError::EncodingClassFixedEmpty)?;
                if v.chars().any(|c| INVALID_CHARS.contains(&c)) {
                    return Err(DomainError::EncodingClassFixedInvalidChar);
                }
                if v.chars().count() != usize::try_from(length).unwrap_or(0) {
                    return Err(DomainError::EncodingClassFixedLengthMismatch { length });
                }
                Some(v)
            }
            SegmentType::Independent => value,
            _ => None,
        };
        if kind == SegmentType::Serial
            && self
                .segments
                .iter()
                .any(|s| s.kind == SegmentType::Serial && Some(s.no) != except)
        {
            return Err(DomainError::EncodingClassSerialDuplicate);
        }
        if kind == SegmentType::Year && !ctx.year_lengths.contains(&length) {
            return Err(DomainError::EncodingClassYearLengthNotDefined { length });
        }
        Ok(ClassSegment {
            no,
            description: text_of(input.description, "段次說明")?,
            length,
            kind,
            value,
            show_name: input.show_name,
        })
    }

    fn next_no(&self) -> i32 {
        self.segments.iter().map(|s| s.no).max().unwrap_or(0) + 1
    }

    /// 新增一段（:885-925）：段次未給時為最大段次＋1；段次須為正整數、不可重複
    pub fn add_segment(
        &mut self,
        no: Option<i32>,
        input: SegmentInput,
        ctx: &SegmentContext,
        user: &str,
        today: NaiveDate,
    ) -> Result<i32, DomainError> {
        self.check_active()?;
        let no = no.unwrap_or_else(|| self.next_no());
        if no < 1 {
            return Err(DomainError::EncodingClassLineInvalid);
        }
        if self.segments.iter().any(|s| s.no == no) {
            return Err(DomainError::EncodingClassLineDuplicate);
        }
        let seg = self.build(no, input, ctx, None)?;
        self.segments.push(seg);
        self.segments.sort_by_key(|s| s.no);
        self.touch(user, today);
        Ok(no)
    }

    /// 插入一段（:1289-1322 addno）：原本這個段次以後的段次全部加 1，再放在這個段次
    pub fn insert_segment(
        &mut self,
        at: i32,
        input: SegmentInput,
        ctx: &SegmentContext,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        if at < 1 {
            return Err(DomainError::EncodingClassLineInvalid);
        }
        let seg = self.build(at, input, ctx, None)?;
        for s in self.segments.iter_mut().filter(|s| s.no >= at) {
            s.no += 1;
        }
        self.segments.push(seg);
        self.segments.sort_by_key(|s| s.no);
        self.touch(user, today);
        Ok(())
    }

    /// 修改一段（段次不變）
    pub fn update_segment(
        &mut self,
        no: i32,
        input: SegmentInput,
        ctx: &SegmentContext,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        if !self.segments.iter().any(|s| s.no == no) {
            return Err(DomainError::EncodingClassLineNotFound);
        }
        let seg = self.build(no, input, ctx, Some(no))?;
        if let Some(s) = self.segments.iter_mut().find(|s| s.no == no) {
            *s = seg;
        }
        self.touch(user, today);
        Ok(())
    }

    /// 刪除一段（:1078-1102）
    pub fn remove_segment(
        &mut self,
        no: i32,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        let before = self.segments.len();
        self.segments.retain(|s| s.no != no);
        if self.segments.len() == before {
            return Err(DomainError::EncodingClassLineNotFound);
        }
        self.touch(user, today);
        Ok(())
    }

    /// 段次重排（:1251-1287 lineno_from_1_resort）：依目前順序重編成 1、2、3…
    pub fn renumber(&mut self, user: &str, today: NaiveDate) -> Result<(), DomainError> {
        self.check_active()?;
        self.segments.sort_by_key(|s| s.no);
        for (i, s) in self.segments.iter_mut().enumerate() {
            s.no = i32::try_from(i).unwrap_or(i32::MAX - 1) + 1;
        }
        self.touch(user, today);
        Ok(())
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製單頭與單身（:1588-1682）：資料所有者為目前使用者、資料有效、未確認
    pub fn copy_as(&self, code: ClassCode, user: &str, dept: &str, today: NaiveDate) -> Self {
        let mut c = Self::new(code, self.attrs.clone(), user, dept, today);
        c.segments = self.segments.clone();
        c
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

    /// 修改單頭或單身都更新單頭的最後修改者與日期（FUN-5B0136，EC7(b)）
    fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

/// 批號、序號性質才能用型態 4–9；批號、序號性質不能用獨立段（:959-991 aic-048）
fn check_allowed(kind: SegmentType, ctx: &SegmentContext) -> Result<(), DomainError> {
    let ok = if kind.lot_serial_only() {
        ctx.lot_or_serial
    } else if kind == SegmentType::Independent {
        !ctx.lot_or_serial
    } else {
        true
    };
    if ok {
        Ok(())
    } else {
        Err(DomainError::EncodingClassTypeNotAllowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 17).unwrap()
    }

    fn class() -> EncodingClass {
        EncodingClass::new(
            ClassCode::parse("FG").unwrap(),
            ClassAttrs::new(Some("成品".into()), "A1").unwrap(),
            "u",
            "d",
            today(),
        )
    }

    fn seg(kind: &str, length: Option<i16>, value: Option<&str>) -> SegmentInput {
        SegmentInput {
            kind: kind.into(),
            length,
            value: value.map(Into::into),
            ..Default::default()
        }
    }

    #[test]
    fn item_class() {
        let ctx = SegmentContext {
            lot_or_serial: false,
            independent_segments: vec![("COLOR".into(), 2)],
            year_lengths: vec![1],
        };
        let mut c = class();
        c.add_segment(None, seg("1", Some(2), Some("FG")), &ctx, "u", today())
            .unwrap();
        c.add_segment(None, seg("2", None, Some("COLOR")), &ctx, "u", today())
            .unwrap();
        c.add_segment(None, seg("3", Some(4), None), &ctx, "u", today())
            .unwrap();
        assert_eq!((c.segment_count(), c.total_length()), (3, 8));
        assert_eq!(
            c.add_segment(None, seg("3", Some(3), None), &ctx, "u", today()),
            Err(DomainError::EncodingClassSerialDuplicate)
        );
        assert_eq!(
            c.add_segment(None, seg("7", Some(2), None), &ctx, "u", today()),
            Err(DomainError::EncodingClassTypeNotAllowed)
        );
        c.insert_segment(2, seg("1", Some(1), Some("-")), &ctx, "u", today())
            .unwrap();
        let kinds: Vec<_> = c.segments.iter().map(|s| (s.no, s.kind.code())).collect();
        assert_eq!(kinds, [(1, "1"), (2, "1"), (3, "2"), (4, "3")]);
        c.remove_segment(1, "u", today()).unwrap();
        c.renumber("u", today()).unwrap();
        let nos: Vec<_> = c.segments.iter().map(|s| s.no).collect();
        assert_eq!(nos, [1, 2, 3]);
    }

    #[test]
    fn lot_class() {
        let ctx = SegmentContext {
            lot_or_serial: true,
            independent_segments: vec![("COLOR".into(), 2)],
            year_lengths: vec![1],
        };
        let mut c = class();
        assert_eq!(
            c.add_segment(None, seg("5", Some(2), None), &ctx, "u", today()),
            Err(DomainError::EncodingClassYearLengthNotDefined { length: 2 })
        );
        assert_eq!(
            c.add_segment(None, seg("7", Some(3), None), &ctx, "u", today()),
            Err(DomainError::EncodingClassDateLengthInvalid)
        );
        assert_eq!(
            c.add_segment(None, seg("4", Some(2), Some("ima01")), &ctx, "u", today()),
            Err(DomainError::EncodingClassTypeNotSupported)
        );
        assert_eq!(
            c.add_segment(None, seg("2", None, Some("COLOR")), &ctx, "u", today()),
            Err(DomainError::EncodingClassTypeNotAllowed)
        );
        assert!(
            c.add_segment(None, seg("5", Some(1), None), &ctx, "u", today())
                .is_ok()
        );
    }
}
