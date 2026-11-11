//! 匯率（TipTop: azj_file 幣別每月匯率檔／aooi060、azk_file 幣別每日匯率檔／aooi070、s_exrate 匯率取用）
//! 規格：specs/features/aoo/exchange_rate_monthly.feature（MR1–MR10）、exchange_rate_daily.feature（ER1–ER5、DR1–DR9）、
//! exchange_rate_daily_copy.feature（CP1–CP7）
//!
//! 共用 schema（ER1）。匯率一律 Decimal、存到小數 10 位（ER3、DR6）。

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;

use crate::{DomainError, confirmation::Confirmation, currency::CurrencyCode, record::Stamp};

/// 匯率存到小數 10 位（NUMERIC(20,10)，DR6）
pub const RATE_SCALE: u32 = 10;

/// 年月（azj02 varchar2(6) YYYYMM；aooi060.4gl:1850-1871 i060_chkym，MR2）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RatePeriod {
    year: i32,
    month: u32,
}

impl RatePeriod {
    /// 4 位數年＋01–12 月，共 6 位數字
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.len() != 6 || !v.bytes().all(|b| b.is_ascii_digit()) {
            return Err(DomainError::ExchangeRatePeriodInvalid);
        }
        let year: i32 = v[..4]
            .parse()
            .map_err(|_| DomainError::ExchangeRatePeriodInvalid)?;
        let month: u32 = v[4..]
            .parse()
            .map_err(|_| DomainError::ExchangeRatePeriodInvalid)?;
        if !(1..=12).contains(&month) {
            return Err(DomainError::ExchangeRatePeriodInvalid);
        }
        Ok(Self { year, month })
    }

    /// 日期所屬的年月
    pub fn of(date: NaiveDate) -> Self {
        Self {
            year: date.year(),
            month: date.month(),
        }
    }

    /// 該月第一天與最後一天（aooi070.4gl:756-768）
    pub fn range(&self) -> (NaiveDate, NaiveDate) {
        let first = NaiveDate::from_ymd_opt(self.year, self.month, 1).expect("月初");
        let next = if self.month == 12 {
            NaiveDate::from_ymd_opt(self.year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(self.year, self.month + 1, 1)
        }
        .expect("下月初");
        (first, next.pred_opt().expect("月底"))
    }

    pub fn contains(&self, date: NaiveDate) -> bool {
        Self::of(date) == *self
    }
}

impl std::fmt::Display for RatePeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}{:02}", self.year, self.month)
    }
}

/// 六種匯率（azk03、azk04、azk041、azk05、azk051、azk052；azj03、azj04、azj041、azj05、azj051、azj052）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rates {
    /// 銀行買入
    pub bank_buy: Decimal,
    /// 銀行賣出
    pub bank_sell: Decimal,
    /// 銀行中價
    pub bank_mid: Decimal,
    /// 海關旬匯率（每月匯率為月平均，azj05）
    pub customs_period: Decimal,
    /// 海關買入
    pub customs_buy: Decimal,
    /// 海關賣出
    pub customs_sell: Decimal,
}

impl Rates {
    /// 全部為 1（本國幣別，MR4、DR2）
    pub fn unit() -> Self {
        Self::all(Decimal::ONE)
    }

    pub fn all(v: Decimal) -> Self {
        Self {
            bank_buy: v,
            bank_sell: v,
            bank_mid: v,
            customs_period: v,
            customs_buy: v,
            customs_sell: v,
        }
    }

    /// 不可小於 0（aooi070.4gl:540-616、aooi060.4gl:652-770；MR3、DR2）；存到小數 10 位
    pub fn checked(self) -> Result<Self, DomainError> {
        for (name, v) in self.named() {
            if v.is_sign_negative() && !v.is_zero() {
                return Err(DomainError::ExchangeRateNegative(name));
            }
        }
        Ok(self.map(round))
    }

    fn named(&self) -> [(&'static str, Decimal); 6] {
        [
            ("銀行買入", self.bank_buy),
            ("銀行賣出", self.bank_sell),
            ("銀行中價", self.bank_mid),
            ("海關旬", self.customs_period),
            ("海關買入", self.customs_buy),
            ("海關賣出", self.customs_sell),
        ]
    }

    fn map(self, f: impl Fn(Decimal) -> Decimal) -> Self {
        Self {
            bank_buy: f(self.bank_buy),
            bank_sell: f(self.bank_sell),
            bank_mid: f(self.bank_mid),
            customs_period: f(self.customs_period),
            customs_buy: f(self.customs_buy),
            customs_sell: f(self.customs_sell),
        }
    }

    /// 依匯率種類取值（s_exrate p_rate）
    pub fn get(&self, kind: RateKind) -> Decimal {
        match kind {
            RateKind::BankBuy => self.bank_buy,
            RateKind::BankSell => self.bank_sell,
            RateKind::BankMid => self.bank_mid,
            RateKind::CustomsPeriod => self.customs_period,
            RateKind::CustomsBuy => self.customs_buy,
            RateKind::CustomsSell => self.customs_sell,
        }
    }

    /// 統一匯率（aooi070.4gl:1447-1471 cal_rate，客製 10605060；DR7）：
    /// 銀行買入、賣出改為銀行中價；海關旬、海關賣出改為海關買入
    pub fn unified(self) -> Self {
        Self {
            bank_buy: self.bank_mid,
            bank_sell: self.bank_mid,
            customs_period: self.customs_buy,
            customs_sell: self.customs_buy,
            ..self
        }
    }

    /// 平均（aooi070.4gl:769-779 i070_mth；DR6 小數 10 位四捨五入）；沒有資料回 None（DR5 (a)）
    pub fn average<'a>(list: impl IntoIterator<Item = &'a Rates>) -> Option<Self> {
        let mut sum = Self::default();
        let mut n = 0u32;
        for r in list {
            sum = Self {
                bank_buy: sum.bank_buy + r.bank_buy,
                bank_sell: sum.bank_sell + r.bank_sell,
                bank_mid: sum.bank_mid + r.bank_mid,
                customs_period: sum.customs_period + r.customs_period,
                customs_buy: sum.customs_buy + r.customs_buy,
                customs_sell: sum.customs_sell + r.customs_sell,
            };
            n += 1;
        }
        if n == 0 {
            return None;
        }
        let n = Decimal::from(n);
        Some(sum.map(|v| round(v / n)))
    }
}

fn round(v: Decimal) -> Decimal {
    v.round_dp(RATE_SCALE).normalize()
}

/// 匯率種類（s_exrate p_rate：B 銀行買入、S 銀行賣出、M 銀行中價、P 海關旬、C 海關買入、D 海關賣出）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateKind {
    BankBuy,
    BankSell,
    BankMid,
    CustomsPeriod,
    CustomsBuy,
    CustomsSell,
}

impl RateKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::BankBuy => "B",
            Self::BankSell => "S",
            Self::BankMid => "M",
            Self::CustomsPeriod => "P",
            Self::CustomsBuy => "C",
            Self::CustomsSell => "D",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "B" => Self::BankBuy,
            "S" => Self::BankSell,
            "M" => Self::BankMid,
            "P" => Self::CustomsPeriod,
            "C" => Self::CustomsBuy,
            "D" => Self::CustomsSell,
            _ => return None,
        })
    }
}

/// 兩幣別換算（s_exrate.4gl：原幣匯率 / 目的幣匯率）；本國幣別以 1 代入（ER5）。目的幣匯率為 0 時找不到
pub fn convert(from: Decimal, to: Decimal) -> Result<Decimal, DomainError> {
    if to.is_zero() {
        return Err(DomainError::ExchangeRateNotFound);
    }
    Ok(round(from / to))
}

/// 共用的狀態欄位（資料有效、所有者、確認、版本；ADR 0012、0013）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateRecord {
    pub active: bool,
    pub confirmation: Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl RateRecord {
    /// 新增：資料有效 Y、所有者、部門、日期（aooi060.4gl:417-420）
    pub fn new(user: &str, dept: &str, today: NaiveDate) -> Self {
        Self {
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

    pub fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
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

    /// 停用的資料不可修改（aooi060.4gl:1027-1030 aoo-062；MR6）
    pub fn check_modifiable(&self) -> Result<(), DomainError> {
        if self.active {
            Ok(())
        } else {
            Err(DomainError::ExchangeRateInactive)
        }
    }
}

/// 每月匯率（azj_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthlyRate {
    /// azj01 幣別
    pub currency: CurrencyCode,
    /// azj02 年月
    pub period: RatePeriod,
    /// azj03、azj04、azj041、azj05（海關旬月平均，MR10）、azj051、azj052
    pub rates: Rates,
    /// azj07 月底重評價匯率
    pub revaluation: Decimal,
    pub record: RateRecord,
}

impl MonthlyRate {
    /// 新增（aooi060.4gl:397-587）；本國幣別一律為 1（MR4）
    pub fn new(
        currency: CurrencyCode,
        period: RatePeriod,
        rates: Rates,
        revaluation: Decimal,
        base: bool,
        record: RateRecord,
    ) -> Result<Self, DomainError> {
        let mut x = Self {
            currency,
            period,
            rates: Rates::default(),
            revaluation: Decimal::ZERO,
            record,
        };
        x.set_rates(rates, revaluation, base)?;
        Ok(x)
    }

    /// 設定匯率；不可小於 0（MR3），本國幣別一律為 1（MR4）
    pub fn set_rates(
        &mut self,
        rates: Rates,
        revaluation: Decimal,
        base: bool,
    ) -> Result<(), DomainError> {
        let rates = rates.checked()?;
        if revaluation.is_sign_negative() && !revaluation.is_zero() {
            return Err(DomainError::ExchangeRateNegative("月底重評價"));
        }
        if base {
            self.rates = Rates::unit();
            self.revaluation = Decimal::ONE;
        } else {
            self.rates = rates;
            self.revaluation = round(revaluation);
        }
        Ok(())
    }

    /// 每日匯率月平均同步（aooi070.4gl:780-791 i070_mth；DR5）：銀行買入、賣出、中價、海關買入、賣出；
    /// 海關旬、月底重評價不動
    pub fn apply_daily_average(&mut self, avg: &Rates, base: bool) {
        let rates = Rates {
            customs_period: self.rates.customs_period,
            ..*avg
        };
        self.rates = if base { Rates::unit() } else { rates };
    }

    /// 月底重評價（aooi060.4gl:1589-1719 re_rate，客製 S202604008；MR7）：
    /// 六種匯率為該月每日平均，月底重評價為當天的銀行中價
    pub fn apply_revaluation(&mut self, avg: &Rates, day_mid: Decimal, base: bool) {
        if base {
            self.rates = Rates::unit();
            self.revaluation = Decimal::ONE;
        } else {
            self.rates = *avg;
            self.revaluation = round(day_mid);
        }
    }

    /// 複製（aooi060.4gl:1344-1463）：新幣別＋新年月，所有者為目前使用者、資料有效、未確認
    pub fn copy_as(
        &self,
        currency: CurrencyCode,
        period: RatePeriod,
        base: bool,
        record: RateRecord,
    ) -> Result<Self, DomainError> {
        Self::new(currency, period, self.rates, self.revaluation, base, record)
    }

    /// 月平均同步、月底重評價可否覆寫（DR5 (b)）：已確認或已停用的不覆寫
    pub fn overwritable(&self) -> bool {
        self.record.active && !self.record.confirmation.confirmed
    }
}

/// 每日匯率（azk_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyRate {
    /// azk01 幣別
    pub currency: CurrencyCode,
    /// azk02 日期
    pub date: NaiveDate,
    pub rates: Rates,
    pub record: RateRecord,
}

impl DailyRate {
    /// 新增（aooi070.4gl:415-459）；本國幣別一律為 1（DR2）
    pub fn new(
        currency: CurrencyCode,
        date: NaiveDate,
        rates: Rates,
        base: bool,
        record: RateRecord,
    ) -> Result<Self, DomainError> {
        let mut x = Self {
            currency,
            date,
            rates: Rates::default(),
            record,
        };
        x.set_rates(rates, base)?;
        Ok(x)
    }

    pub fn set_rates(&mut self, rates: Rates, base: bool) -> Result<(), DomainError> {
        let rates = rates.checked()?;
        self.rates = if base { Rates::unit() } else { rates };
        Ok(())
    }

    /// 複製（aooi070.4gl:1111-1211）、每日匯率複製（aoop010.4gl:280-293）：新幣別／日期，未確認、資料有效
    pub fn copy_as(
        &self,
        currency: CurrencyCode,
        date: NaiveDate,
        base: bool,
        record: RateRecord,
    ) -> Result<Self, DomainError> {
        Self::new(currency, date, self.rates, base, record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(v: &str) -> Decimal {
        v.parse().unwrap()
    }

    #[test]
    fn period() {
        assert_eq!(RatePeriod::parse("202611").unwrap().to_string(), "202611");
        for bad in ["202613", "202600", "2026-11", "20261", "abcdef"] {
            assert_eq!(
                RatePeriod::parse(bad),
                Err(DomainError::ExchangeRatePeriodInvalid),
                "{bad}"
            );
        }
        let (a, b) = RatePeriod::parse("202612").unwrap().range();
        assert_eq!(
            (a.to_string(), b.to_string()),
            ("2026-12-01".into(), "2026-12-31".into())
        );
        let (_, b) = RatePeriod::parse("202802").unwrap().range();
        assert_eq!(b.to_string(), "2028-02-29");
    }

    #[test]
    fn negative() {
        let r = Rates {
            customs_period: d("-0.1"),
            ..Rates::default()
        };
        assert_eq!(
            r.checked(),
            Err(DomainError::ExchangeRateNegative("海關旬"))
        );
    }

    #[test]
    fn average_rounds_to_ten_places() {
        let a = Rates::all(d("1"));
        let b = Rates::all(d("2"));
        let c = Rates::all(d("2"));
        assert_eq!(
            Rates::average([&a, &b, &c]).unwrap().bank_mid,
            d("1.6666666667")
        );
        assert_eq!(Rates::average(std::iter::empty()), None);
    }

    #[test]
    fn unify() {
        let r = Rates {
            bank_buy: d("32.0"),
            bank_sell: d("32.2"),
            bank_mid: d("32.1"),
            customs_period: d("31"),
            customs_buy: d("32"),
            customs_sell: d("32.3"),
        }
        .unified();
        assert_eq!((r.bank_buy, r.bank_sell), (d("32.1"), d("32.1")));
        assert_eq!((r.customs_period, r.customs_sell), (d("32"), d("32")));
    }

    #[test]
    fn convert_rates() {
        assert_eq!(convert(d("32"), d("0.2")).unwrap(), d("160"));
        assert_eq!(
            convert(d("1"), Decimal::ZERO),
            Err(DomainError::ExchangeRateNotFound)
        );
    }
}

/// 匯率自動匯入的來源解析（aooi070 auto_import；exchange_rate_import.feature IM1–IM9）
pub mod feed {
    use chrono::{Datelike, NaiveDate};
    use rust_decimal::Decimal;

    use crate::DomainError;

    /// 來源的一筆報價：來源幣別代號、買入、賣出
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Quote {
        pub code: String,
        pub buy: Decimal,
        pub sell: Decimal,
    }

    /// 台銀牌告種類（aooi0701.4fd：1 現金、2 即期；IM3）
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BankRateType {
        Cash,
        Spot,
    }

    /// 台銀牌告匯率 CSV（rate.bot.com.tw/xrt/flcsv/0/day）：幣別、「本行買入」、現金、即期、遠期…、「本行賣出」、現金、即期、遠期…。
    /// 以「本行賣出」的位置找賣出欄（aooi070.4gl:1620-1650 對應文字檔第 3／4、13／14 欄）。
    /// 沒有任何資料列（例如收到網站驗證頁）時回 EXCHANGE_RATE_SOURCE_INVALID
    pub fn parse_bot_csv(text: &str, kind: BankRateType) -> Result<Vec<Quote>, DomainError> {
        let invalid = || DomainError::ExchangeRateSourceInvalid("台銀牌告匯率".into());
        let mut out = Vec::new();
        // 標題列、空白列沒有「本行賣出」，略過
        for line in text.lines() {
            let cols: Vec<&str> = line.split(',').map(str::trim).collect();
            let Some(k) = cols.iter().position(|c| *c == "本行賣出") else {
                continue;
            };
            if cols[0].is_empty() {
                continue;
            }
            let offset = match kind {
                BankRateType::Cash => 1,
                BankRateType::Spot => 2,
            };
            let num = |i: usize| -> Result<Decimal, DomainError> {
                cols.get(i)
                    .filter(|v| !v.is_empty())
                    .map_or(Ok(Decimal::ZERO), |v| v.parse().map_err(|_| invalid()))
            };
            out.push(Quote {
                code: cols[0].to_owned(),
                buy: num(1 + offset)?,
                sell: num(k + offset)?,
            });
        }
        if out.is_empty() {
            return Err(invalid());
        }
        Ok(out)
    }

    /// 日期所屬的旬（aooi070.4gl:1872-1880）：1（1–10 日）、2（11–20 日）、3（21 日至月底）與該旬的起訖日
    pub fn ten_day(date: NaiveDate) -> (u32, NaiveDate, NaiveDate) {
        let (y, m) = (date.year(), date.month());
        let ymd = |d| NaiveDate::from_ymd_opt(y, m, d).expect("日期");
        match date.day() {
            1..=10 => (1, ymd(1), ymd(10)),
            11..=20 => (2, ymd(11), ymd(20)),
            _ => (3, ymd(21), super::RatePeriod::of(date).range().1),
        }
    }

    /// 關務署歷史檔一列（HISTORY_TXT，固定寬度；aooi070.4gl:1900-1925）：
    /// 幣別 1–3、民國年 4–6、月 7–8、旬 9、賣出 10–17（3 位整數＋5 位小數）、買入 18–25
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct HistoryQuote {
        pub year: i32,
        pub month: u32,
        pub period: u32,
        pub quote: Quote,
    }

    pub fn parse_customs_history(text: &str) -> Vec<HistoryQuote> {
        text.lines()
            .filter_map(|line| {
                let l = line.trim_end();
                if l.len() < 25 || !l.is_char_boundary(25) {
                    return None;
                }
                let num = |a: usize, b: usize| -> Option<Decimal> {
                    format!("{}.{}", &l[a..a + 3], &l[a + 3..b]).parse().ok()
                };
                Some(HistoryQuote {
                    year: l[3..6].parse::<i32>().ok()? + 1911,
                    month: l[6..8].parse().ok()?,
                    period: l[8..9].parse().ok()?,
                    quote: Quote {
                        code: l[0..3].to_owned(),
                        sell: num(9, 17)?,
                        buy: num(17, 25)?,
                    },
                })
            })
            .collect()
    }

    /// 當旬檔（CURRENT_JSON）不是指定日所屬的旬、或少於 5 筆時，改用歷史檔該旬的資料（aooi070.4gl:1990-2005）。
    /// 歷史檔也沒有該旬時沿用當旬檔（仍是指定日的旬才用）
    pub fn choose_customs(
        date: NaiveDate,
        current: Option<(NaiveDate, NaiveDate, Vec<Quote>)>,
        history: impl FnOnce() -> Vec<HistoryQuote>,
    ) -> Vec<Quote> {
        let covers = |(s, e, _): &(NaiveDate, NaiveDate, Vec<Quote>)| *s <= date && date <= *e;
        if let Some(c) = &current
            && covers(c)
            && c.2.len() >= 5
        {
            return c.2.clone();
        }
        let (p, ..) = ten_day(date);
        let from_history: Vec<Quote> = history()
            .into_iter()
            .filter(|h| h.year == date.year() && h.month == date.month() && h.period == p)
            .map(|h| h.quote)
            .collect();
        if !from_history.is_empty() {
            return from_history;
        }
        current.filter(covers).map(|c| c.2).unwrap_or_default()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn d(v: &str) -> Decimal {
            v.parse().unwrap()
        }

        #[test]
        fn bot_csv() {
            let text = "幣別,匯率,現金,即期,遠期10天,遠期30天,遠期60天,遠期90天,遠期120天,遠期150天,遠期180天,匯率,現金,即期\n\
                        USD,本行買入,31.70000,32.02500,32.0,31.9,31.9,31.8,31.8,31.7,31.7,本行賣出,32.37000,32.12500\n";
            let cash = parse_bot_csv(text, BankRateType::Cash).unwrap();
            assert_eq!((cash[0].buy, cash[0].sell), (d("31.7"), d("32.37")));
            let spot = parse_bot_csv(text, BankRateType::Spot).unwrap();
            assert_eq!((spot[0].buy, spot[0].sell), (d("32.025"), d("32.125")));
            assert!(parse_bot_csv("<!DOCTYPE html>", BankRateType::Spot).is_err());
        }

        #[test]
        fn history_line() {
            // 關務署 HISTORY_TXT 實際資料（2026-11 第 2 旬 USD：賣出 32.93、買入 32.83）
            let h = parse_customs_history("USD1151120329300003283000\r\n");
            assert_eq!((h[0].year, h[0].month, h[0].period), (2026, 11, 2));
            assert_eq!((h[0].quote.buy, h[0].quote.sell), (d("32.83"), d("32.93")));
        }

        #[test]
        fn ten_days() {
            let day = |s: &str| s.parse::<NaiveDate>().unwrap();
            assert_eq!(ten_day(day("2026-11-10")).0, 1);
            assert_eq!(ten_day(day("2026-11-11")).0, 2);
            assert_eq!(ten_day(day("2026-02-28")).2, day("2026-02-28"));
        }
    }
}
