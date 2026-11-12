//! 會計期間（TipTop: azm_file 會計期間參數設定檔；aoos020 會計期間參數設定、aoos030 週期期間產生、aooq010 週期期間查詢）
//! 規格：specs/features/aoo/accounting_periods.feature（PD1–PD12）
//!
//! azm_file 一年一列 41 欄改為表頭（年度、期數）＋一期一列（起訖日、季別）。每日明細 azn_file 不另存，
//! 日期所屬的年度、季、期、週由 [`Calendar`] 即時計算（PD3）；週別：年度第一天為第 1 週，每逢星期日加 1（PD4）。

use chrono::{Datelike, Months, NaiveDate, TimeDelta};

use crate::{DomainError, confirmation::Confirmation, record::Stamp};

/// 一期
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    /// 期別 1–13
    pub no: i16,
    pub start: NaiveDate,
    pub end: NaiveDate,
    /// 季別 1–4
    pub quarter: i16,
}

/// 預設各期（aoos020.4gl:684-744、:770-818；PD5）：
/// 12 期——第 1 期到該月底，之後每期一個月；13 期——每期 28 天，第 13 期到起始日加一年減一天。
/// 季別 1,1,1,2,2,2,3,3,3,4,4,4（第 13 期為 4）
pub fn default_periods(first_start: NaiveDate, count: i16) -> Vec<Period> {
    let quarter = |no: i16| ((no - 1) / 3 + 1).min(4);
    let mut out = Vec::new();
    if count == 13 {
        let year_end = first_start + Months::new(12) - TimeDelta::days(1);
        for no in 1..=13i16 {
            let start = first_start + TimeDelta::days(28 * i64::from(no - 1));
            let end = if no == 13 {
                year_end
            } else {
                start + TimeDelta::days(27)
            };
            out.push(Period {
                no,
                start,
                end,
                quarter: quarter(no),
            });
        }
    } else {
        let month1 = first_start.with_day(1).expect("月初");
        let mut start = first_start;
        for no in 1..=12i16 {
            let next = month1 + Months::new(u32::try_from(no).expect("期別"));
            out.push(Period {
                no,
                start,
                end: next - TimeDelta::days(1),
                quarter: quarter(no),
            });
            start = next;
        }
    }
    out
}

/// 一個會計年度（azm_file 一列）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountingYear {
    /// azm01 年度
    pub year: i32,
    /// azm02 會計期間分類方式：12 或 13 期（新增時取整體管理系統參數，PD6）
    pub period_count: i16,
    pub periods: Vec<Period>,
    /// 確認（ADR 0013 SA9；azm_file 無此欄）
    pub confirmation: Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl AccountingYear {
    /// 新增：檢查各期（[`validate`]）
    pub fn new(
        year: i32,
        period_count: i16,
        mut periods: Vec<Period>,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        periods.sort_by_key(|p| p.no);
        validate(year, period_count, &periods)?;
        Ok(Self {
            year,
            period_count,
            periods,
            confirmation: Confirmation::default(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        })
    }

    /// 修改各期（PD8，TipTop 沒有）：已確認不可修改
    pub fn update(
        &mut self,
        mut periods: Vec<Period>,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        if self.confirmation.confirmed {
            return Err(DomainError::RecordAlreadyConfirmed);
        }
        periods.sort_by_key(|p| p.no);
        validate(self.year, self.period_count, &periods)?;
        self.periods = periods;
        self.touch(user, today);
        Ok(())
    }

    /// 複製為另一個年度（PD9）：各期日期依年度差平移
    pub fn copy_as(
        &self,
        year: i32,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        let diff = year - self.year;
        let shift = |d: NaiveDate| {
            let m = Months::new(diff.unsigned_abs() * 12);
            if diff >= 0 {
                d.checked_add_months(m)
            } else {
                d.checked_sub_months(m)
            }
            .ok_or(DomainError::AccountingPeriodYearInvalid)
        };
        let mut periods = Vec::new();
        for (i, p) in self.periods.iter().enumerate() {
            let start = shift(p.start)?;
            // 月底對月底（如 2 月 28 日 → 閏年 2 月 29 日），維持各期連續
            let next_start = match self.periods.get(i + 1) {
                Some(n) => shift(n.start)?,
                None => shift(p.end + TimeDelta::days(1))?,
            };
            periods.push(Period {
                no: p.no,
                start,
                end: next_start - TimeDelta::days(1),
                quarter: p.quarter,
            });
        }
        Self::new(year, self.period_count, periods, user, dept, today)
    }

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

    /// 第一天、最後一天
    pub fn span(&self) -> (NaiveDate, NaiveDate) {
        (
            self.periods.first().map_or(NaiveDate::MIN, |p| p.start),
            self.periods.last().map_or(NaiveDate::MIN, |p| p.end),
        )
    }

    fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

/// 各期檢查（aoos020.4gl:357-681、aoos021.4gl:1497-1520；PD7）
pub fn validate(year: i32, period_count: i16, periods: &[Period]) -> Result<(), DomainError> {
    if period_count != 12 && period_count != 13 {
        return Err(DomainError::AccountingPeriodsInvalid(period_count));
    }
    // 每一期都必填、12 期制不可有第 13 期（aoo-058）
    let nos: Vec<i16> = periods.iter().map(|p| p.no).collect();
    if nos != (1..=period_count).collect::<Vec<_>>() {
        return Err(DomainError::AccountingPeriodCountInvalid(period_count));
    }
    for p in periods {
        if p.end < p.start {
            return Err(DomainError::AccountingPeriodDateInvalid(p.no));
        }
        if !(1..=4).contains(&p.quarter) {
            return Err(DomainError::AccountingQuarterInvalid(p.no));
        }
        // 期間日期須在年度或下一年度（aoo-020）
        for d in [p.start, p.end] {
            if d.year() != year && d.year() != year + 1 {
                return Err(DomainError::AccountingPeriodYearInvalid);
            }
        }
    }
    for w in periods.windows(2) {
        if w[1].start != w[0].end + TimeDelta::days(1) {
            return Err(DomainError::AccountingPeriodsNotContinuous(w[1].no));
        }
        if w[1].quarter < w[0].quarter {
            return Err(DomainError::AccountingQuarterInvalid(w[1].no));
        }
    }
    Ok(())
}

/// 日期所屬的會計期間（azn_file 一列；s_gactpd）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountingDay {
    pub date: NaiveDate,
    pub year: i32,
    pub quarter: i16,
    pub period: i16,
    pub week: i16,
}

/// 會計行事曆：由各年度的期間即時計算（取代 azn_file 與 aoos030，PD3）
pub struct Calendar<'a> {
    years: &'a [AccountingYear],
}

impl<'a> Calendar<'a> {
    pub fn new(years: &'a [AccountingYear]) -> Self {
        Self { years }
    }

    /// 日期 → 年度、季、期、週（s_yp、s_gactpd；PD4：年度第一天為第 1 週，每逢星期日加 1）
    pub fn locate(&self, date: NaiveDate) -> Result<AccountingDay, DomainError> {
        for y in self.years {
            if let Some(p) = y.periods.iter().find(|p| p.start <= date && date <= p.end) {
                let (first, _) = y.span();
                return Ok(AccountingDay {
                    date,
                    year: y.year,
                    quarter: p.quarter,
                    period: p.no,
                    week: week_of(first, date),
                });
            }
        }
        Err(DomainError::AccountingPeriodNotFound)
    }

    /// 年度＋期別 → 該期（s_azm、s_azn01；PD11）
    pub fn period(&self, year: i32, no: i16) -> Result<Period, DomainError> {
        self.years
            .iter()
            .find(|y| y.year == year)
            .and_then(|y| y.periods.iter().find(|p| p.no == no))
            .copied()
            .ok_or(DomainError::AccountingPeriodNotFound)
    }

    /// 日期區間內每一天（aooq010 週期期間查詢）；有任何一天不在會計期間時回錯誤
    pub fn days(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<AccountingDay>, DomainError> {
        if to < from || (to - from).num_days() > 3700 {
            return Err(DomainError::AccountingPeriodDateRangeInvalid);
        }
        from.iter_days()
            .take_while(|d| *d <= to)
            .map(|d| self.locate(d))
            .collect()
    }
}

/// 第幾週：第一天為第 1 週，之後每個星期日加 1（第一天本身是星期日不加）
fn week_of(first: NaiveDate, date: NaiveDate) -> i16 {
    let days = (date - first).num_days();
    // first 之後到 date 之間（含 date）的星期日數
    let to_sunday = i64::from((7 - first.weekday().num_days_from_sunday()) % 7);
    let to_sunday = if to_sunday == 0 { 7 } else { to_sunday };
    let sundays = if days < to_sunday {
        0
    } else {
        (days - to_sunday) / 7 + 1
    };
    i16::try_from(sundays + 1).unwrap_or(i16::MAX)
}

/// 兩個年度的期間是否重疊（PD7）
pub fn overlaps(a: &AccountingYear, b: &AccountingYear) -> bool {
    let (a1, a2) = a.span();
    let (b1, b2) = b.span();
    a1 <= b2 && b1 <= a2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    fn year(y: i32, start: &str, count: i16) -> AccountingYear {
        AccountingYear::new(
            y,
            count,
            default_periods(d(start), count),
            "tiptop",
            "",
            d("2026-11-12"),
        )
        .unwrap()
    }

    #[test]
    fn twelve_period_defaults() {
        let p = default_periods(d("2026-07-16"), 12);
        assert_eq!((p[0].start, p[0].end), (d("2026-07-16"), d("2026-07-31")));
        assert_eq!((p[11].start, p[11].end), (d("2027-06-01"), d("2027-06-30")));
        assert_eq!(
            p.iter().map(|x| x.quarter).collect::<Vec<_>>(),
            [1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4]
        );
    }

    #[test]
    fn thirteen_period_defaults() {
        let p = default_periods(d("2026-01-01"), 13);
        assert_eq!((p[11].start, p[11].end), (d("2026-11-05"), d("2026-12-02")));
        assert_eq!(
            (p[12].start, p[12].end, p[12].quarter),
            (d("2026-12-03"), d("2026-12-31"), 4)
        );
    }

    #[test]
    fn weeks_start_on_sunday() {
        let y = [year(2026, "2026-01-01", 12)];
        let c = Calendar::new(&y);
        for (date, week) in [
            ("2026-01-01", 1),
            ("2026-01-03", 1),
            ("2026-01-04", 2),
            ("2026-03-15", 12),
            ("2026-12-31", 53),
        ] {
            assert_eq!(c.locate(d(date)).unwrap().week, week, "{date}");
        }
        // 第一天是星期日
        assert_eq!(week_of(d("2026-03-01"), d("2026-03-01")), 1);
        assert_eq!(week_of(d("2026-03-01"), d("2026-03-07")), 1);
        assert_eq!(week_of(d("2026-03-01"), d("2026-03-08")), 2);
    }

    #[test]
    fn copy_keeps_month_ends() {
        let y = year(2027, "2027-01-01", 12);
        let c = y.copy_as(2028, "tiptop", "", d("2026-11-12")).unwrap();
        assert_eq!(c.periods[1].end, d("2028-02-29"));
    }

    #[test]
    fn checks() {
        let mut p = default_periods(d("2026-01-01"), 12);
        p[2].start = d("2026-03-05");
        assert_eq!(
            validate(2026, 12, &p).unwrap_err().code(),
            "ACCOUNTING_PERIODS_NOT_CONTINUOUS"
        );
        let p = default_periods(d("2026-01-01"), 12);
        assert_eq!(
            validate(2026, 13, &p).unwrap_err().code(),
            "ACCOUNTING_PERIOD_COUNT_INVALID"
        );
    }
}
