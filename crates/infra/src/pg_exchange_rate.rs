//! 匯率（monthly_exchange_rate ← TipTop azj_file、daily_exchange_rate ← azk_file）與系統參數（system_parameter ← aza_file）。
//! 共用 schema（ER1、ER2）；exchange_rate_*.feature
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use ttrs_app::{
    error::RepoError,
    ports::{ExchangeRateRepository, SystemParameterRepository},
};
use ttrs_domain::{
    confirmation::Confirmation,
    currency::CurrencyCode,
    exchange_rate::{DailyRate, MonthlyRate, RatePeriod, RateRecord, Rates},
    general_parameter::{AccountingPeriods, GeneralParameters, RateBasis},
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct MonthlyRow {
    currency_code: String,
    period: String,
    bank_buy: Decimal,
    bank_sell: Decimal,
    bank_mid: Decimal,
    customs_period: Decimal,
    customs_buy: Decimal,
    customs_sell: Decimal,
    revaluation: Decimal,
    active: bool,
    confirmed: bool,
    confirmed_by: Option<String>,
    confirmed_at: Option<DateTime<Utc>>,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct DailyRow {
    currency_code: String,
    rate_date: NaiveDate,
    bank_buy: Decimal,
    bank_sell: Decimal,
    bank_mid: Decimal,
    customs_period: Decimal,
    customs_buy: Decimal,
    customs_sell: Decimal,
    active: bool,
    confirmed: bool,
    confirmed_by: Option<String>,
    confirmed_at: Option<DateTime<Utc>>,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

macro_rules! select_monthly {
    ($rest:literal) => {
        concat!(
            "SELECT currency_code, period, bank_buy, bank_sell, bank_mid, customs_period, customs_buy, ",
            "customs_sell, revaluation, active, confirmed, confirmed_by, confirmed_at, created_by, created_dept, ",
            "created_on, updated_by, updated_on, version, created_at, updated_at FROM monthly_exchange_rate ",
            $rest
        )
    };
}

macro_rules! select_daily {
    ($rest:literal) => {
        concat!(
            "SELECT currency_code, rate_date, bank_buy, bank_sell, bank_mid, customs_period, customs_buy, ",
            "customs_sell, active, confirmed, confirmed_by, confirmed_at, created_by, created_dept, ",
            "created_on, updated_by, updated_on, version, created_at, updated_at FROM daily_exchange_rate ",
            $rest
        )
    };
}

fn bad(v: &str) -> RepoError {
    RepoError::Other(format!("匯率資料不正確：{v}"))
}

#[allow(clippy::too_many_arguments)]
fn record(
    active: bool,
    confirmed: bool,
    confirmed_by: Option<String>,
    confirmed_at: Option<DateTime<Utc>>,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
) -> RateRecord {
    RateRecord {
        active,
        confirmation: Confirmation {
            confirmed,
            by: confirmed_by,
            at: confirmed_at,
        },
        created_by,
        created_dept,
        created_on,
        updated_by,
        updated_on,
        stamp: Stamp {
            version,
            created_at: Some(created_at),
            updated_at,
        },
    }
}

fn rates(a: Decimal, b: Decimal, c: Decimal, d: Decimal, e: Decimal, f: Decimal) -> Rates {
    Rates {
        bank_buy: a.normalize(),
        bank_sell: b.normalize(),
        bank_mid: c.normalize(),
        customs_period: d.normalize(),
        customs_buy: e.normalize(),
        customs_sell: f.normalize(),
    }
}

impl TryFrom<MonthlyRow> for MonthlyRate {
    type Error = RepoError;

    fn try_from(r: MonthlyRow) -> Result<Self, RepoError> {
        Ok(MonthlyRate {
            currency: CurrencyCode::parse(&r.currency_code).map_err(|_| bad(&r.currency_code))?,
            period: RatePeriod::parse(&r.period).map_err(|_| bad(&r.period))?,
            rates: rates(
                r.bank_buy,
                r.bank_sell,
                r.bank_mid,
                r.customs_period,
                r.customs_buy,
                r.customs_sell,
            ),
            revaluation: r.revaluation.normalize(),
            record: record(
                r.active,
                r.confirmed,
                r.confirmed_by,
                r.confirmed_at,
                r.created_by,
                r.created_dept,
                r.created_on,
                r.updated_by,
                r.updated_on,
                r.version,
                r.created_at,
                r.updated_at,
            ),
        })
    }
}

impl TryFrom<DailyRow> for DailyRate {
    type Error = RepoError;

    fn try_from(r: DailyRow) -> Result<Self, RepoError> {
        Ok(DailyRate {
            currency: CurrencyCode::parse(&r.currency_code).map_err(|_| bad(&r.currency_code))?,
            date: r.rate_date,
            rates: rates(
                r.bank_buy,
                r.bank_sell,
                r.bank_mid,
                r.customs_period,
                r.customs_buy,
                r.customs_sell,
            ),
            record: record(
                r.active,
                r.confirmed,
                r.confirmed_by,
                r.confirmed_at,
                r.created_by,
                r.created_dept,
                r.created_on,
                r.updated_by,
                r.updated_on,
                r.version,
                r.created_at,
                r.updated_at,
            ),
        })
    }
}

#[async_trait]
impl ExchangeRateRepository for PgStore {
    async fn list_monthly_rates(&self) -> Result<Vec<MonthlyRate>, RepoError> {
        sqlx::query_as::<_, MonthlyRow>(select_monthly!(
            "ORDER BY currency_code COLLATE \"C\", period"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?
        .into_iter()
        .map(MonthlyRate::try_from)
        .collect()
    }

    async fn get_monthly_rate(
        &self,
        currency: &CurrencyCode,
        period: RatePeriod,
    ) -> Result<Option<MonthlyRate>, RepoError> {
        sqlx::query_as::<_, MonthlyRow>(select_monthly!("WHERE currency_code = $1 AND period = $2"))
            .bind(currency.as_str())
            .bind(period.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .map(MonthlyRate::try_from)
            .transpose()
    }

    async fn insert_monthly_rate(&self, r: &MonthlyRate) -> Result<(), RepoError> {
        let (a, x) = (&r.rates, &r.record);
        sqlx::query(
            "INSERT INTO monthly_exchange_rate (currency_code, period, bank_buy, bank_sell, bank_mid,
                 customs_period, customs_buy, customs_sell, revaluation, active, created_by, created_dept,
                 created_on, confirmed, confirmed_by, confirmed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)",
        )
        .bind(r.currency.as_str())
        .bind(r.period.to_string())
        .bind(a.bank_buy)
        .bind(a.bank_sell)
        .bind(a.bank_mid)
        .bind(a.customs_period)
        .bind(a.customs_buy)
        .bind(a.customs_sell)
        .bind(r.revaluation)
        .bind(x.active)
        .bind(&x.created_by)
        .bind(&x.created_dept)
        .bind(x.created_on)
        .bind(x.confirmation.confirmed)
        .bind(&x.confirmation.by)
        .bind(x.confirmation.at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_monthly_rate(&self, r: &MonthlyRate) -> Result<(), RepoError> {
        let (a, x) = (&r.rates, &r.record);
        let n = sqlx::query(
            "UPDATE monthly_exchange_rate SET bank_buy = $3, bank_sell = $4, bank_mid = $5,
                 customs_period = $6, customs_buy = $7, customs_sell = $8, revaluation = $9, active = $10,
                 updated_by = $11, updated_on = $12, confirmed = $14, confirmed_by = $15, confirmed_at = $16
             WHERE currency_code = $1 AND period = $2 AND version = $13",
        )
        .bind(r.currency.as_str())
        .bind(r.period.to_string())
        .bind(a.bank_buy)
        .bind(a.bank_sell)
        .bind(a.bank_mid)
        .bind(a.customs_period)
        .bind(a.customs_buy)
        .bind(a.customs_sell)
        .bind(r.revaluation)
        .bind(x.active)
        .bind(&x.updated_by)
        .bind(x.updated_on)
        .bind(x.stamp.version)
        .bind(x.confirmation.confirmed)
        .bind(&x.confirmation.by)
        .bind(x.confirmation.at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        Ok(())
    }

    async fn delete_monthly_rate(
        &self,
        currency: &CurrencyCode,
        period: RatePeriod,
    ) -> Result<bool, RepoError> {
        let n = sqlx::query(
            "DELETE FROM monthly_exchange_rate WHERE currency_code = $1 AND period = $2",
        )
        .bind(currency.as_str())
        .bind(period.to_string())
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        Ok(n > 0)
    }

    async fn list_daily_rates(
        &self,
        currency: Option<&CurrencyCode>,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<DailyRate>, RepoError> {
        sqlx::query_as::<_, DailyRow>(select_daily!(
            "WHERE ($1::varchar IS NULL OR currency_code = $1)
               AND ($2::date IS NULL OR rate_date >= $2)
               AND ($3::date IS NULL OR rate_date <= $3)
             ORDER BY currency_code COLLATE \"C\", rate_date"
        ))
        .bind(currency.map(CurrencyCode::as_str))
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?
        .into_iter()
        .map(DailyRate::try_from)
        .collect()
    }

    async fn get_daily_rate(
        &self,
        currency: &CurrencyCode,
        date: NaiveDate,
    ) -> Result<Option<DailyRate>, RepoError> {
        sqlx::query_as::<_, DailyRow>(select_daily!("WHERE currency_code = $1 AND rate_date = $2"))
            .bind(currency.as_str())
            .bind(date)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .map(DailyRate::try_from)
            .transpose()
    }

    async fn insert_daily_rate(&self, r: &DailyRate) -> Result<(), RepoError> {
        let (a, x) = (&r.rates, &r.record);
        sqlx::query(
            "INSERT INTO daily_exchange_rate (currency_code, rate_date, bank_buy, bank_sell, bank_mid,
                 customs_period, customs_buy, customs_sell, active, created_by, created_dept,
                 created_on, confirmed, confirmed_by, confirmed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
        )
        .bind(r.currency.as_str())
        .bind(r.date)
        .bind(a.bank_buy)
        .bind(a.bank_sell)
        .bind(a.bank_mid)
        .bind(a.customs_period)
        .bind(a.customs_buy)
        .bind(a.customs_sell)
        .bind(x.active)
        .bind(&x.created_by)
        .bind(&x.created_dept)
        .bind(x.created_on)
        .bind(x.confirmation.confirmed)
        .bind(&x.confirmation.by)
        .bind(x.confirmation.at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_daily_rate(&self, r: &DailyRate) -> Result<(), RepoError> {
        let (a, x) = (&r.rates, &r.record);
        let n = sqlx::query(
            "UPDATE daily_exchange_rate SET bank_buy = $3, bank_sell = $4, bank_mid = $5,
                 customs_period = $6, customs_buy = $7, customs_sell = $8, active = $9,
                 updated_by = $10, updated_on = $11, confirmed = $13, confirmed_by = $14, confirmed_at = $15
             WHERE currency_code = $1 AND rate_date = $2 AND version = $12",
        )
        .bind(r.currency.as_str())
        .bind(r.date)
        .bind(a.bank_buy)
        .bind(a.bank_sell)
        .bind(a.bank_mid)
        .bind(a.customs_period)
        .bind(a.customs_buy)
        .bind(a.customs_sell)
        .bind(x.active)
        .bind(&x.updated_by)
        .bind(x.updated_on)
        .bind(x.stamp.version)
        .bind(x.confirmation.confirmed)
        .bind(&x.confirmation.by)
        .bind(x.confirmation.at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        if n == 0 {
            return Err(RepoError::Stale);
        }
        Ok(())
    }

    async fn delete_daily_rate(
        &self,
        currency: &CurrencyCode,
        date: NaiveDate,
    ) -> Result<bool, RepoError> {
        let n = sqlx::query(
            "DELETE FROM daily_exchange_rate WHERE currency_code = $1 AND rate_date = $2",
        )
        .bind(currency.as_str())
        .bind(date)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        Ok(n > 0)
    }

    async fn has_any_rate(&self) -> Result<bool, RepoError> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM monthly_exchange_rate) OR EXISTS (SELECT 1 FROM daily_exchange_rate)",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)
    }
}

/// system_parameter 的一列（aza_file；general_parameters.feature）
#[derive(FromRow)]
struct GeneralRow {
    accounting_periods: i16,
    base_currency: Option<String>,
    rate_basis: String,
    auto_item_number: bool,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

impl GeneralRow {
    fn into_domain(self) -> Result<GeneralParameters, RepoError> {
        let bad = |e: ttrs_domain::DomainError| RepoError::Other(e.to_string());
        Ok(GeneralParameters {
            accounting_periods: AccountingPeriods::parse(self.accounting_periods).map_err(bad)?,
            base_currency: CurrencyCode::parse(self.base_currency.as_deref().unwrap_or("TWD"))
                .map_err(bad)?,
            rate_basis: RateBasis::parse(&self.rate_basis).map_err(bad)?,
            auto_item_number: self.auto_item_number,
            created_by: self.created_by,
            created_dept: self.created_dept,
            created_on: self.created_on,
            updated_by: self.updated_by,
            updated_on: self.updated_on,
            stamp: Stamp {
                version: self.version,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },
        })
    }
}

#[async_trait]
impl SystemParameterRepository for PgStore {
    async fn base_currency(&self) -> Result<Option<String>, RepoError> {
        let v: Option<Option<String>> = sqlx::query_scalar(
            "SELECT base_currency FROM system_parameter WHERE parameter_key = '0'",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(v.flatten())
    }

    async fn set_base_currency(&self, code: &str) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO system_parameter (parameter_key, base_currency) VALUES ('0', $1)
             ON CONFLICT (parameter_key) DO UPDATE SET base_currency = EXCLUDED.base_currency",
        )
        .bind(code)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn general_parameters(&self) -> Result<Option<GeneralParameters>, RepoError> {
        let row: Option<GeneralRow> = sqlx::query_as(
            "SELECT accounting_periods, base_currency, rate_basis, auto_item_number, created_by, created_dept, created_on,
                    updated_by, updated_on, version, created_at, updated_at
               FROM system_parameter WHERE parameter_key = '0'",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        row.map(GeneralRow::into_domain).transpose()
    }

    async fn update_general_parameters(&self, p: &GeneralParameters) -> Result<(), RepoError> {
        let n = sqlx::query(
            "UPDATE system_parameter SET accounting_periods = $1, base_currency = $2, rate_basis = $3,
                    updated_by = $4, updated_on = $5, auto_item_number = $7
              WHERE parameter_key = '0' AND version = $6",
        )
        .bind(p.accounting_periods.get())
        .bind(p.base_currency.as_str())
        .bind(p.rate_basis.code())
        .bind(&p.updated_by)
        .bind(p.updated_on)
        .bind(p.stamp.version)
        .bind(p.auto_item_number)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        if n == 0 {
            return Err(RepoError::Stale);
        }
        Ok(())
    }
}

// ----- 匯率自動匯入的來源（exchange_rate_import.feature IM1；網址可用環境變數改） -----

use ttrs_app::ports::RateFeed;
use ttrs_domain::{
    DomainError,
    exchange_rate::feed::{self, BankRateType, HistoryQuote, Quote},
};

/// 台銀牌告匯率 CSV
const BOT_URL: &str = "https://rate.bot.com.tw/xrt/flcsv/0/day";
/// 關務署當旬報關匯率（政府資料開放平臺 6708）
const CUSTOMS_URL: &str =
    "https://portal.sw.nat.gov.tw/APGQ/GC331!downLoad?formBean.downLoadFile=CURRENT_JSON";
/// 關務署歷史報關匯率
const CUSTOMS_HISTORY_URL: &str =
    "https://portal.sw.nat.gov.tw/APGQ/GC331!downLoad?formBean.downLoadFile=HISTORY_TXT";

fn url_from_env(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

/// 下載（逾時 60 秒）；HTTPS 用 rustls＋ring
async fn download(what: &str, url: &str) -> Result<Vec<u8>, DomainError> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let fail = |e: String| DomainError::ExchangeRateSourceUnavailable(what.to_owned(), e);
    let res = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent("TTRS exchange rate import")
        .build()
        .map_err(|e| fail(e.to_string()))?
        .get(url)
        .send()
        .await
        .map_err(|e| fail(e.to_string()))?;
    if !res.status().is_success() {
        return Err(fail(format!("HTTP {}", res.status())));
    }
    Ok(res.bytes().await.map_err(|e| fail(e.to_string()))?.to_vec())
}

fn ymd(v: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(v.trim(), "%Y%m%d").ok()
}

#[async_trait]
impl RateFeed for PgStore {
    async fn bank_quotes(&self, kind: BankRateType) -> Result<Vec<Quote>, DomainError> {
        let body = download("台銀牌告匯率", &url_from_env("TTRS_RATE_BOT_URL", BOT_URL)).await?;
        feed::parse_bot_csv(&String::from_utf8_lossy(&body), kind)
    }

    async fn customs_current(&self) -> Result<(NaiveDate, NaiveDate, Vec<Quote>), DomainError> {
        let what = "關務署當旬報關匯率";
        let body = download(what, &url_from_env("TTRS_RATE_CUSTOMS_URL", CUSTOMS_URL)).await?;
        let invalid = || DomainError::ExchangeRateSourceInvalid(what.to_owned());
        let v: serde_json::Value = serde_json::from_slice(&body).map_err(|_| invalid())?;
        let date = |k: &str| v.get(k).and_then(|x| x.as_str()).and_then(ymd);
        let (Some(start), Some(end)) = (date("start"), date("end")) else {
            return Err(invalid());
        };
        let num = |x: &serde_json::Value, k: &str| -> Option<Decimal> {
            x.get(k)?.as_str()?.trim().parse().ok()
        };
        let quotes = v
            .get("items")
            .and_then(|i| i.as_array())
            .ok_or_else(invalid)?
            .iter()
            .filter_map(|x| {
                Some(Quote {
                    code: x.get("code")?.as_str()?.trim().to_owned(),
                    buy: num(x, "buyValue")?,
                    sell: num(x, "sellValue")?,
                })
            })
            .collect();
        Ok((start, end, quotes))
    }

    async fn customs_history(&self) -> Result<Vec<HistoryQuote>, DomainError> {
        let body = download(
            "關務署歷史報關匯率",
            &url_from_env("TTRS_RATE_CUSTOMS_HISTORY_URL", CUSTOMS_HISTORY_URL),
        )
        .await?;
        // Big5 檔；資料列都是 ASCII
        Ok(feed::parse_customs_history(&String::from_utf8_lossy(&body)))
    }
}
