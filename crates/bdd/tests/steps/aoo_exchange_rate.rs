//! 匯率 — specs/features/aoo/exchange_rate_monthly.feature（MR）、exchange_rate_daily.feature（DR）、
//! exchange_rate_daily_copy.feature（CP）。鍵值是兩段（幣別＋年月、幣別＋日期），標準功能的步驟也寫在這裡。

use chrono::NaiveDate;
use cucumber::{gherkin::Step, given, then, when};
use rust_decimal::Decimal;
use ttrs_app::ports::SystemParameterRepository;
use ttrs_domain::exchange_rate::RateKind;
use ttrs_shared::{DailyRateDto, DailyRateInput, MonthlyRateDto, MonthlyRateInput, RatesDto};

use crate::{Backend, ErpWorld, steps::common::rows};

fn day(s: &str) -> NaiveDate {
    s.parse().unwrap_or_else(|_| panic!("日期格式錯誤：{s}"))
}

fn dec(s: &str) -> Decimal {
    s.parse().unwrap_or_else(|_| panic!("數值格式錯誤：{s}"))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

/// 規格上的匯率名稱 → 欄位；月底重評價另外處理
fn set_rate(r: &mut RatesDto, label: &str, v: Decimal) -> bool {
    match label {
        "銀行買入" => r.bank_buy = v,
        "銀行賣出" => r.bank_sell = v,
        "銀行中價" => r.bank_mid = v,
        "海關旬" => r.customs_period = v,
        "海關買入" => r.customs_buy = v,
        "海關賣出" => r.customs_sell = v,
        _ => return false,
    }
    true
}

fn get_rate(r: &RatesDto, label: &str) -> Option<Decimal> {
    Some(match label {
        "銀行買入" => r.bank_buy,
        "銀行賣出" => r.bank_sell,
        "銀行中價" => r.bank_mid,
        "海關旬" => r.customs_period,
        "海關買入" => r.customs_buy,
        "海關賣出" => r.customs_sell,
        _ => return None,
    })
}

/// 「銀行買入 32.1，銀行賣出 32.3」→ 匯率與月底重評價
fn parse_rates(text: &str) -> (RatesDto, Decimal) {
    let mut r = RatesDto::default();
    let mut reval = Decimal::ZERO;
    for part in text.split('，') {
        let (label, value) = part
            .trim()
            .rsplit_once(' ')
            .unwrap_or_else(|| panic!("無法解析：{part}"));
        let v = dec(value.trim());
        if label == "月底重評價" {
            reval = v;
        } else if !set_rate(&mut r, label, v) {
            panic!("未知欄位：{label}");
        }
    }
    (r, reval)
}

fn assert_dec(actual: Decimal, expected: &str, label: &str) {
    assert_eq!(actual, dec(expected), "{label}");
}

fn ok(w: &ErpWorld, what: &str) {
    if let Some(Err(e)) = &w.last {
        panic!("{what}失敗：{e}");
    }
}

// ----- 共用前提 -----

#[given(expr = "本國幣別為 {string}")]
async fn base_currency(w: &mut ErpWorld, code: String) {
    match &w.backend {
        Backend::Memory(store) => store.set_base_currency(&code).await.expect("本國幣別"),
        Backend::Pg(pool) => {
            sqlx::query("UPDATE system_parameter SET base_currency = $1")
                .bind(&code)
                .execute(pool)
                .await
                .expect("本國幣別");
        }
    }
}

#[given(expr = "幣別 {string} 已停用")]
async fn currency_inactive(w: &mut ErpWorld, code: String) {
    w.currencies
        .set_active(&w.actor, &code, false, None)
        .await
        .unwrap_or_else(|e| panic!("停用幣別 {code} 失敗：{e}"));
}

#[given(expr = "今天是 {string}")]
async fn today_is(w: &mut ErpWorld, d: String) {
    w.rates = w.rates.clone().with_today(day(&d));
}

// ----- 每月匯率 -----

async fn monthly(w: &ErpWorld, cur: &str, period: &str) -> MonthlyRateDto {
    w.rates
        .get_monthly(&w.actor, cur, period)
        .await
        .unwrap_or_else(|e| panic!("查詢每月匯率 {cur} {period} 失敗：{e}"))
}

async fn add_monthly(w: &mut ErpWorld, cur: &str, period: &str, rates: RatesDto, reval: Decimal) {
    let req = MonthlyRateInput {
        currency_code: cur.to_owned(),
        period: period.to_owned(),
        rates,
        revaluation: reval,
        version: None,
    };
    let r = w.rates.create_monthly(&w.actor, req).await;
    w.last = Some(r.map(drop));
}

/// 表格：幣別、年月與各匯率（general_parameters.feature GP-10）
#[given(expr = "已有每月匯率:")]
async fn has_monthly_table(w: &mut ErpWorld, step: &Step) {
    let table = step.table.as_ref().expect("此步驟需要表格");
    let header = table.rows[0].clone();
    for r in rows(step) {
        let mut cur = String::new();
        let mut period = String::new();
        let mut rates = RatesDto::default();
        let mut reval = Decimal::ZERO;
        for (label, v) in header.iter().zip(&r) {
            match label.as_str() {
                "幣別" => cur = v.clone(),
                "年月" => period = v.clone(),
                "月底重評價" => reval = dec(v),
                other => assert!(set_rate(&mut rates, other, dec(v)), "未知欄位：{other}"),
            }
        }
        add_monthly(w, &cur, &period, rates, reval).await;
        ok(w, &format!("建立每月匯率 {cur} {period} "));
    }
}

#[given(regex = r#"^已有每月匯率 "(\S+)" "(\S+)"$"#)]
async fn has_monthly(w: &mut ErpWorld, cur: String, period: String) {
    let rates = RatesDto {
        bank_buy: dec("32.1"),
        bank_sell: dec("32.3"),
        bank_mid: dec("32.2"),
        ..Default::default()
    };
    add_monthly(w, &cur, &period, rates, Decimal::ZERO).await;
    ok(w, &format!("建立每月匯率 {cur} {period} "));
}

#[when(regex = r#"^使用者新增每月匯率 "(\S+)" "(\S+)"，(.+)$"#)]
async fn new_monthly(w: &mut ErpWorld, cur: String, period: String, rest: String) {
    let (rates, reval) = parse_rates(&rest);
    add_monthly(w, &cur, &period, rates, reval).await;
}

#[then(regex = r#"^每月匯率 "(\S+)" "(\S+)" 應為:$"#)]
async fn monthly_is(w: &mut ErpWorld, cur: String, period: String, step: &Step) {
    let m = monthly(w, &cur, &period).await;
    for r in rows(step) {
        match r[0].as_str() {
            "資料有效" => assert_eq!(yes_no(m.active), r[1], "資料有效"),
            "月底重評價" => assert_dec(m.revaluation, &r[1], "月底重評價"),
            label => assert_dec(
                get_rate(&m.rates, label).unwrap_or_else(|| panic!("未知欄位：{label}")),
                &r[1],
                label,
            ),
        }
    }
}

#[then(
    regex = r#"^每月匯率 "(\S+)" "(\S+)" 的(銀行買入|銀行賣出|銀行中價|海關旬|海關買入|海關賣出|月底重評價)應為 ([\d.]+)$"#
)]
async fn monthly_rate_is(w: &mut ErpWorld, cur: String, period: String, label: String, v: String) {
    let m = monthly(w, &cur, &period).await;
    let actual = if label == "月底重評價" {
        m.revaluation
    } else {
        get_rate(&m.rates, &label).expect("匯率")
    };
    assert_dec(actual, &v, &label);
}

#[when(regex = r#"^使用者將每月匯率 "(\S+)" "(\S+)" 的(\S+)改為 ([\d.-]+)$"#)]
async fn change_monthly(w: &mut ErpWorld, cur: String, period: String, label: String, v: String) {
    let mut req = monthly(w, &cur, &period).await.to_input();
    if label == "月底重評價" {
        req.revaluation = dec(&v);
    } else {
        assert!(
            set_rate(&mut req.rates, &label, dec(&v)),
            "未知欄位：{label}"
        );
    }
    let r = w.rates.update_monthly(&w.actor, &cur, &period, req).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(停用|啟用)每月匯率 "(\S+)" "(\S+)"$"#)]
async fn toggle_monthly(w: &mut ErpWorld, op: String, cur: String, period: String) {
    let r = w
        .rates
        .set_monthly_active(&w.actor, &cur, &period, op == "啟用", None)
        .await;
    w.last = Some(r.map(drop));
}

#[then(regex = r#"^每月匯率 "(\S+)" "(\S+)" 的資料有效應為 "(是|否)"$"#)]
async fn monthly_active(w: &mut ErpWorld, cur: String, period: String, expected: String) {
    assert_eq!(yes_no(monthly(w, &cur, &period).await.active), expected);
}

#[when(regex = r#"^使用者刪除每月匯率 "(\S+)" "(\S+)"$"#)]
async fn delete_monthly(w: &mut ErpWorld, cur: String, period: String) {
    let r = w.rates.delete_monthly(&w.actor, &cur, &period, None).await;
    w.last = Some(r);
}

#[then(regex = r#"^每月匯率 "(\S+)" "(\S+)" 應不存在$"#)]
async fn monthly_absent(w: &mut ErpWorld, cur: String, period: String) {
    let r = w.rates.get_monthly(&w.actor, &cur, &period).await;
    assert!(r.is_err(), "每月匯率 {cur} {period} 應不存在");
}

#[given(regex = r#"^每月匯率 "(\S+)" "(\S+)" 已確認$"#)]
async fn monthly_confirmed(w: &mut ErpWorld, cur: String, period: String) {
    w.rates
        .confirm_monthly(&w.actor, &cur, &period, true, None)
        .await
        .unwrap_or_else(|e| panic!("確認每月匯率 {cur} {period} 失敗：{e}"));
}

#[when(regex = r#"^使用者(確認|取消確認)每月匯率 "(\S+)" "(\S+)"$"#)]
async fn confirm_monthly(w: &mut ErpWorld, op: String, cur: String, period: String) {
    let r = w
        .rates
        .confirm_monthly(&w.actor, &cur, &period, op == "確認", None)
        .await;
    w.last = Some(r.map(drop));
}

#[then(regex = r#"^每月匯率 "(\S+)" "(\S+)" 的確認狀態應為:$"#)]
async fn monthly_confirm_state(w: &mut ErpWorld, cur: String, period: String, step: &Step) {
    let m = monthly(w, &cur, &period).await;
    confirm_rows(step, m.confirmed, m.confirmed_by.as_deref());
}

fn confirm_rows(step: &Step, confirmed: bool, by: Option<&str>) {
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "已確認" => yes_no(confirmed).to_owned(),
            "確認人" => by.unwrap_or_default().to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[when(regex = r#"^使用者將每月匯率 "(\S+)" "(\S+)" 複製為 "(\S+)" "(\S+)"$"#)]
async fn copy_monthly(
    w: &mut ErpWorld,
    cur: String,
    period: String,
    to: String,
    to_period: String,
) {
    let r = w
        .rates
        .copy_monthly(&w.actor, &cur, &period, &to, &to_period)
        .await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者以下列條件查詢每月匯率:")]
async fn query_monthly(w: &mut ErpWorld, step: &Step) {
    let req = crate::steps::common::query_request(step, query_key, None);
    let r = w.rates.query_monthly(&w.actor, req).await;
    crate::steps::common::set_query_result(w, r, MonthlyRateDto::key);
}

fn query_key(label: &str) -> &'static str {
    match label {
        "幣別" => "currency_code",
        "年月" => "period",
        "日期" => "rate_date",
        "銀行中價" => "bank_mid",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

#[when(expr = "使用者以日期 {string} 執行月底重評價")]
async fn revaluate(w: &mut ErpWorld, d: String) {
    let r = w.rates.revaluate(&w.actor, day(&d)).await;
    w.last = Some(r.map(drop));
}

// ----- 每日匯率 -----

async fn daily(w: &ErpWorld, cur: &str, d: &str) -> DailyRateDto {
    w.rates
        .get_daily(&w.actor, cur, day(d))
        .await
        .unwrap_or_else(|e| panic!("查詢每日匯率 {cur} {d} 失敗：{e}"))
}

async fn add_daily(w: &mut ErpWorld, cur: &str, d: &str, rates: RatesDto) {
    let req = DailyRateInput {
        currency_code: cur.to_owned(),
        rate_date: day(d),
        rates,
        version: None,
    };
    let r = w.rates.create_daily(&w.actor, req).await;
    w.last = Some(r.map(drop));
}

#[given(regex = r#"^已有每日匯率 "(\S+)" "(\S+)"$"#)]
async fn has_daily(w: &mut ErpWorld, cur: String, d: String) {
    let rates = RatesDto {
        bank_buy: dec("32.0"),
        bank_sell: dec("32.2"),
        bank_mid: dec("32.1"),
        customs_period: dec("32.0"),
        customs_buy: dec("32.0"),
        customs_sell: dec("32.1"),
    };
    add_daily(w, &cur, &d, rates).await;
    ok(w, &format!("建立每日匯率 {cur} {d} "));
}

#[given(expr = "已有每日匯率:")]
async fn has_daily_table(w: &mut ErpWorld, step: &Step) {
    let table = step.table.as_ref().expect("此步驟需要表格");
    let header = table.rows[0].clone();
    for r in rows(step) {
        let mut cur = String::new();
        let mut d = String::new();
        let mut rates = RatesDto::default();
        for (label, v) in header.iter().zip(&r) {
            match label.as_str() {
                "幣別" => cur = v.clone(),
                "日期" => d = v.clone(),
                other => assert!(set_rate(&mut rates, other, dec(v)), "未知欄位：{other}"),
            }
        }
        add_daily(w, &cur, &d, rates).await;
        ok(w, &format!("建立每日匯率 {cur} {d} "));
    }
}

#[when(regex = r#"^使用者新增每日匯率 "(\S+)" "(\S+)"，(.+)$"#)]
async fn new_daily(w: &mut ErpWorld, cur: String, d: String, rest: String) {
    let (rates, _) = parse_rates(&rest);
    add_daily(w, &cur, &d, rates).await;
}

#[then(regex = r#"^每日匯率 "(\S+)" "(\S+)" 應為:$"#)]
async fn daily_is(w: &mut ErpWorld, cur: String, d: String, step: &Step) {
    let x = daily(w, &cur, &d).await;
    for r in rows(step) {
        match r[0].as_str() {
            "資料有效" => assert_eq!(yes_no(x.active), r[1], "資料有效"),
            label => assert_dec(
                get_rate(&x.rates, label).unwrap_or_else(|| panic!("未知欄位：{label}")),
                &r[1],
                label,
            ),
        }
    }
}

#[then(
    regex = r#"^每日匯率 "(\S+)" "(\S+)" 的(銀行買入|銀行賣出|銀行中價|海關旬|海關買入|海關賣出)應為 ([\d.]+)$"#
)]
async fn daily_rate_is(w: &mut ErpWorld, cur: String, d: String, label: String, v: String) {
    let x = daily(w, &cur, &d).await;
    assert_dec(get_rate(&x.rates, &label).expect("匯率"), &v, &label);
}

#[when(expr = "使用者查詢幣別 {string} 的預設每日匯率")]
async fn defaults(w: &mut ErpWorld, cur: String) {
    let r = w.rates.daily_defaults(&w.actor, &cur).await;
    w.rate_defaults = r.as_ref().ok().copied();
    w.last = Some(r.map(drop));
}

#[then(expr = "預設每日匯率應為:")]
async fn defaults_are(w: &mut ErpWorld, step: &Step) {
    let d = w.rate_defaults.expect("尚未查詢預設匯率");
    for r in rows(step) {
        assert_dec(get_rate(&d, &r[0]).expect("匯率"), &r[1], &r[0]);
    }
}

#[when(regex = r#"^使用者將每日匯率 "(\S+)" "(\S+)" 的(\S+)改為 ([\d.-]+)$"#)]
async fn change_daily(w: &mut ErpWorld, cur: String, d: String, label: String, v: String) {
    let mut req = daily(w, &cur, &d).await.to_input();
    assert!(
        set_rate(&mut req.rates, &label, dec(&v)),
        "未知欄位：{label}"
    );
    let r = w.rates.update_daily(&w.actor, &cur, day(&d), req).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(停用|啟用)每日匯率 "(\S+)" "(\S+)"$"#)]
async fn toggle_daily(w: &mut ErpWorld, op: String, cur: String, d: String) {
    let r = w
        .rates
        .set_daily_active(&w.actor, &cur, day(&d), op == "啟用", None)
        .await;
    w.last = Some(r.map(drop));
}

#[given(regex = r#"^每日匯率 "(\S+)" "(\S+)" 已停用$"#)]
async fn daily_inactive(w: &mut ErpWorld, cur: String, d: String) {
    w.rates
        .set_daily_active(&w.actor, &cur, day(&d), false, None)
        .await
        .unwrap_or_else(|e| panic!("停用每日匯率 {cur} {d} 失敗：{e}"));
}

#[when(regex = r#"^使用者刪除每日匯率 "(\S+)" "(\S+)"$"#)]
async fn delete_daily(w: &mut ErpWorld, cur: String, d: String) {
    let r = w.rates.delete_daily(&w.actor, &cur, day(&d), None).await;
    w.last = Some(r);
}

#[given(regex = r#"^每日匯率 "(\S+)" "(\S+)" 已確認$"#)]
async fn daily_confirmed(w: &mut ErpWorld, cur: String, d: String) {
    w.rates
        .confirm_daily(&w.actor, &cur, day(&d), true, None)
        .await
        .unwrap_or_else(|e| panic!("確認每日匯率 {cur} {d} 失敗：{e}"));
}

#[then(regex = r#"^每日匯率 "(\S+)" "(\S+)" 的確認狀態應為:$"#)]
async fn daily_confirm_state(w: &mut ErpWorld, cur: String, d: String, step: &Step) {
    let x = daily(w, &cur, &d).await;
    confirm_rows(step, x.confirmed, x.confirmed_by.as_deref());
}

#[when(regex = r#"^使用者將每日匯率 "(\S+)" "(\S+)" 複製為 "(\S+)" "(\S+)"$"#)]
async fn copy_daily(w: &mut ErpWorld, cur: String, d: String, to: String, to_d: String) {
    let r = w
        .rates
        .copy_daily(&w.actor, &cur, day(&d), &to, day(&to_d))
        .await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者以下列條件查詢每日匯率:")]
async fn query_daily(w: &mut ErpWorld, step: &Step) {
    let req = crate::steps::common::query_request(step, query_key, None);
    let r = w.rates.query_daily(&w.actor, req).await;
    crate::steps::common::set_query_result(w, r, DailyRateDto::key);
}

#[when(expr = "使用者對日期 {string} 執行統一匯率")]
async fn unify(w: &mut ErpWorld, d: String) {
    let r = w.rates.unify(&w.actor, day(&d)).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^取用 "(\S+)" 對 "(\S+)" 於 "(\S+)" 的 (\S+) 匯率$"#)]
async fn lookup(w: &mut ErpWorld, from: String, to: String, d: String, kind: String) {
    let kind = match kind.as_str() {
        "銀行買入" => RateKind::BankBuy,
        "銀行賣出" => RateKind::BankSell,
        "銀行中價" => RateKind::BankMid,
        "海關旬" => RateKind::CustomsPeriod,
        "海關買入" => RateKind::CustomsBuy,
        "海關賣出" => RateKind::CustomsSell,
        other => panic!("未知匯率種類：{other}"),
    };
    let r = w.rates.rate(&from, &to, day(&d), kind).await;
    w.rate_value = r.as_ref().ok().copied();
    w.last = Some(r.map(drop));
}

#[then(expr = "取得的匯率應為 {word}")]
async fn rate_value(w: &mut ErpWorld, v: String) {
    ok(w, "取用匯率");
    assert_dec(w.rate_value.expect("尚未取用匯率"), &v, "匯率");
}

// ----- 每日匯率複製（aoop010） -----

#[when(expr = "使用者執行每日匯率複製，補到 {string}")]
async fn fill(w: &mut ErpWorld, until: String) {
    let r = w.rates.fill(&w.actor, day(&until)).await;
    w.rate_fill = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

#[then(expr = "複製結果應為:")]
async fn fill_is(w: &mut ErpWorld, step: &Step) {
    let lines = w.rate_fill.clone().expect("尚未執行每日匯率複製");
    let table = step.table.as_ref().expect("此步驟需要表格");
    let header = table.rows[0].clone();
    let expected = rows(step);
    assert_eq!(lines.len(), expected.len(), "筆數：{lines:?}");
    for (line, r) in lines.iter().zip(&expected) {
        for (label, v) in header.iter().zip(r) {
            match label.as_str() {
                "幣別" => assert_eq!(&line.currency_code, v),
                "日期" => assert_eq!(line.rate_date, day(v)),
                other => assert_dec(get_rate(&line.rates, other).expect("匯率"), v, other),
            }
        }
    }
}

#[then("複製結果應為空")]
async fn fill_empty(w: &mut ErpWorld) {
    let lines = w.rate_fill.clone().expect("尚未執行每日匯率複製");
    assert!(lines.is_empty(), "應無複製結果：{lines:?}");
}

// ----- 匯率自動匯入與每日排程（exchange_rate_import.feature） -----

use ttrs_domain::exchange_rate::feed::{BankRateType, HistoryQuote, Quote};

fn quotes(step: &Step) -> Vec<Quote> {
    rows(step)
        .iter()
        .map(|r| Quote {
            code: r[0].clone(),
            buy: dec(&r[1]),
            sell: dec(&r[2]),
        })
        .collect()
}

/// 「2026-11」第 n 旬的起訖日
fn ten_day(month: &str, n: u32) -> (NaiveDate, NaiveDate) {
    let first = day(&format!("{month}-01"));
    let d = first + chrono::Days::new(u64::from((n - 1) * 10));
    let (_, s, e) = ttrs_domain::exchange_rate::feed::ten_day(d);
    (s, e)
}

#[given(expr = "台銀即期匯率為:")]
async fn bot_quotes(w: &mut ErpWorld, step: &Step) {
    w.rate_feed.set_bank_quotes(Some(quotes(step)));
}

#[given("台銀匯率來源無法取得")]
async fn bot_unavailable(w: &mut ErpWorld) {
    w.rate_feed.set_bank_quotes(None);
}

#[given("海關匯率來源無法取得")]
async fn customs_unavailable(w: &mut ErpWorld) {
    w.rate_feed.set_customs_current(None);
}

#[given(regex = r#"^海關 "(\S+)" 第 (\d) 旬匯率為:$"#)]
async fn customs_quotes(w: &mut ErpWorld, month: String, n: u32, step: &Step) {
    let (s, e) = ten_day(&month, n);
    w.rate_feed.set_customs_current(Some((s, e, quotes(step))));
}

#[given(regex = r#"^海關當旬檔為 "(\S+)" 至 "(\S+)":$"#)]
async fn customs_current(w: &mut ErpWorld, start: String, end: String, step: &Step) {
    w.rate_feed
        .set_customs_current(Some((day(&start), day(&end), quotes(step))));
}

#[given(regex = r#"^海關歷史檔有 "(\S+)" 第 (\d) 旬匯率:$"#)]
async fn customs_history(w: &mut ErpWorld, month: String, n: u32, step: &Step) {
    let (s, _) = ten_day(&month, n);
    let lines = quotes(step)
        .into_iter()
        .map(|quote| HistoryQuote {
            year: chrono::Datelike::year(&s),
            month: chrono::Datelike::month(&s),
            period: n,
            quote,
        })
        .collect();
    w.rate_feed.add_customs_history(lines);
}

#[given(expr = "幣別 {string} 的匯入代號為 {string}")]
async fn import_code(w: &mut ErpWorld, code: String, source: String) {
    let mut req = w
        .currencies
        .get(&w.actor, &code)
        .await
        .unwrap_or_else(|e| panic!("查詢幣別 {code} 失敗：{e}"))
        .to_input();
    req.import_code = Some(source);
    w.currencies
        .update(&w.actor, &code, req)
        .await
        .unwrap_or_else(|e| panic!("設定匯入代號失敗：{e}"));
}

fn set_import(w: &mut ErpWorld, r: Result<ttrs_shared::RateImportResult, ttrs_app::AppError>) {
    w.rate_import = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

#[when(regex = r"^使用者匯入台銀匯率(並同意覆寫)?$")]
async fn import_bot(w: &mut ErpWorld, overwrite: String) {
    let r = w
        .rates
        .import_bank(&w.actor, BankRateType::Spot, !overwrite.is_empty(), None)
        .await;
    set_import(w, r);
}

#[when(regex = r"^使用者上傳台銀匯率 CSV 匯入(現金|即期)匯率:$")]
async fn upload_bot(w: &mut ErpWorld, kind: String, step: &Step) {
    let kind = if kind == "現金" {
        BankRateType::Cash
    } else {
        BankRateType::Spot
    };
    let csv = step.docstring.clone().expect("此步驟需要文字內容");
    let r = w.rates.import_bank(&w.actor, kind, true, Some(&csv)).await;
    set_import(w, r);
}

#[when(regex = r"^使用者匯入海關匯率(並同意覆寫)?$")]
async fn import_customs(w: &mut ErpWorld, _overwrite: String) {
    let r = w.rates.import_customs(&w.actor).await;
    set_import(w, r);
}

#[then(expr = "匯入結果的略過幣別應為 {string}")]
async fn import_skipped(w: &mut ErpWorld, expected: String) {
    ok(w, "匯入");
    let r = w.rate_import.clone().expect("尚未匯入");
    assert_eq!(r.skipped.join(","), expected);
}

#[then(expr = "匯入結果的警告應包含 {string}")]
async fn import_warning(w: &mut ErpWorld, code: String) {
    ok(w, "匯入");
    let r = w.rate_import.clone().expect("尚未匯入");
    assert!(r.warnings.contains(&code), "警告：{:?}", r.warnings);
}

#[then(regex = r#"^每日匯率 "(\S+)" "(\S+)" 應不存在$"#)]
async fn daily_absent(w: &mut ErpWorld, cur: String, d: String) {
    let r = w.rates.get_daily(&w.actor, &cur, day(&d)).await;
    assert!(r.is_err(), "每日匯率 {cur} {d} 應不存在");
}

#[given(expr = "管理員信箱為 {string}")]
async fn admin_mail(w: &mut ErpWorld, to: String) {
    w.rates = w
        .rates
        .clone()
        .with_job_mail(std::sync::Arc::new(w.outbox.clone()), &to);
}

#[when("服務帳號執行每日匯率排程")]
async fn run_job(w: &mut ErpWorld) {
    let actor = ttrs_app::Actor {
        user_id: "ttrs-batch".into(),
        dept: String::new(),
        permission_class: "ADMIN".into(),
        kind: ttrs_app::authorization::ActorKind::Service,
    };
    w.rate_job = Some(w.rates.run_daily_job(&actor).await);
}

#[then(regex = r"^排程結果應(成功|失敗)$")]
async fn job_result(w: &mut ErpWorld, expected: String) {
    let steps = w.rate_job.clone().expect("尚未執行排程");
    let ok = steps.iter().all(|s| s.ok);
    assert_eq!(ok, expected == "成功", "排程結果：{steps:?}");
}

#[then(regex = r#"^"(\S+)" 應收到主旨含 "(\S+)" 的信$"#)]
async fn job_mail_sent(w: &mut ErpWorld, to: String, subject: String) {
    let sent = w.outbox.sent();
    assert!(
        sent.iter()
            .any(|m| m.to == to && m.subject.contains(&subject)),
        "寄出的信：{:?}",
        sent.iter().map(|m| (&m.to, &m.subject)).collect::<Vec<_>>()
    );
}
