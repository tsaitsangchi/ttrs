//! 會計期間維護 — specs/features/aoo/accounting_periods.feature（PD，TipTop aoos020 / azm_file、aoos030、aooq010）

use chrono::NaiveDate;
use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{AccountingPeriodLine, AccountingYearInput, GeneralParametersInput};

use crate::{ErpWorld, steps::common::rows};

fn day(s: &str) -> NaiveDate {
    s.trim_matches('"')
        .parse()
        .unwrap_or_else(|_| panic!("日期格式錯誤：{s}"))
}

async fn defaults(w: &ErpWorld, year: i32, start: &str) -> Vec<AccountingPeriodLine> {
    w.periods
        .defaults(&w.actor, year, day(start))
        .await
        .unwrap_or_else(|e| panic!("取預設各期失敗：{e}"))
}

async fn create(w: &mut ErpWorld, year: i32, periods: Vec<AccountingPeriodLine>) {
    let input = AccountingYearInput {
        year,
        periods,
        version: None,
    };
    let r = w.periods.create(&w.actor, input).await;
    w.last = Some(r.map(drop));
}

fn ok(w: &ErpWorld, what: &str) {
    if let Some(Err(e)) = &w.last {
        panic!("{what}失敗：{e}");
    }
}

#[given(regex = r"^整體管理系統參數的會計期間為 (\d+) 期$")]
async fn param_periods(w: &mut ErpWorld, n: i16) {
    let cur = w.general.get(&w.actor).await.expect("參數");
    let input = GeneralParametersInput {
        accounting_periods: n,
        base_currency: cur.base_currency,
        rate_basis: cur.rate_basis,
        auto_item_number: None,
        version: Some(cur.version),
    };
    let actor = w.actor.clone();
    w.general
        .update(&actor, input)
        .await
        .unwrap_or_else(|e| panic!("設定會計期間失敗：{e}"));
}

#[when(regex = r#"^使用者新增會計年度 (\d+)，第 1 期起始日 "(\S+)"，各期用預設值$"#)]
async fn create_default(w: &mut ErpWorld, year: i32, start: String) {
    let periods = defaults(w, year, &start).await;
    create(w, year, periods).await;
}

#[given(regex = r#"^已有會計年度 (\d+)，第 1 期起始日 "(\S+)"$"#)]
async fn has_year(w: &mut ErpWorld, year: i32, start: String) {
    let periods = defaults(w, year, &start).await;
    create(w, year, periods).await;
    ok(w, &format!("建立會計年度 {year} "));
}

#[when(
    regex = r#"^使用者新增會計年度 (\d+)，第 1 期起始日 "(\S+)"，第 (\d+) 期改為 (\S+) 至 (\S+)、季別 (\d+)$"#
)]
async fn create_changed(
    w: &mut ErpWorld,
    year: i32,
    start: String,
    no: i16,
    from: String,
    to: String,
    quarter: i16,
) {
    let mut periods = defaults(w, year, &start).await;
    let p = periods
        .iter_mut()
        .find(|p| p.period == no)
        .expect("沒有此期");
    p.start_date = day(&from);
    p.end_date = day(&to);
    p.quarter = quarter;
    create(w, year, periods).await;
}

#[when(
    regex = r#"^使用者新增會計年度 (\d+)，第 1 期起始日 "(\S+)"，另加第 13 期 "(\S+)" 至 "(\S+)"$"#
)]
async fn create_extra(w: &mut ErpWorld, year: i32, start: String, from: String, to: String) {
    let mut periods = defaults(w, year, &start).await;
    periods.push(AccountingPeriodLine {
        period: 13,
        start_date: day(&from),
        end_date: day(&to),
        quarter: 4,
    });
    create(w, year, periods).await;
}

#[then(regex = r"^會計年度 (\d+) 應為 (\d+) 期$")]
async fn period_count(w: &mut ErpWorld, year: i32, n: i16) {
    ok(w, "新增會計年度");
    let y = w.periods.get(&w.actor, year).await.expect("會計年度");
    assert_eq!(y.period_count, n);
    assert_eq!(y.periods.len(), usize::try_from(n).unwrap());
}

#[then(regex = r"^會計年度 (\d+) 的各期應包含:$")]
async fn periods_include(w: &mut ErpWorld, year: i32, step: &Step) {
    ok(w, "會計年度");
    let y = w.periods.get(&w.actor, year).await.expect("會計年度");
    for r in rows(step) {
        let no: i16 = r[0].parse().unwrap();
        let p = y
            .periods
            .iter()
            .find(|p| p.period == no)
            .unwrap_or_else(|| panic!("沒有第 {no} 期"));
        assert_eq!(p.start_date, day(&r[1]), "第 {no} 期起始日");
        assert_eq!(p.end_date, day(&r[2]), "第 {no} 期截止日");
        assert_eq!(p.quarter.to_string(), r[3], "第 {no} 期季別");
    }
}

#[when(
    regex = r#"^使用者將會計年度 (\d+) 第 1 期改為 "(\S+)" 至 "(\S+)"、第 2 期改為 "(\S+)" 至 "(\S+)"$"#
)]
async fn modify(w: &mut ErpWorld, year: i32, a: String, b: String, c: String, d: String) {
    let y = w.periods.get(&w.actor, year).await.expect("會計年度");
    let mut periods = y.periods.clone();
    periods[0].start_date = day(&a);
    periods[0].end_date = day(&b);
    periods[1].start_date = day(&c);
    periods[1].end_date = day(&d);
    let input = AccountingYearInput {
        year,
        periods,
        version: Some(y.version),
    };
    let r = w.periods.update(&w.actor, year, input).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r"^使用者確認會計年度 (\d+)$")]
async fn confirm(w: &mut ErpWorld, year: i32) {
    let r = w.periods.confirm(&w.actor, year, true, None).await;
    w.last = Some(r.map(drop));
    ok(w, "確認會計年度");
}

#[when(regex = r"^使用者刪除會計年度 (\d+)$")]
async fn delete(w: &mut ErpWorld, year: i32) {
    let r = w.periods.delete(&w.actor, year, None).await;
    w.last = Some(r);
}

#[when(regex = r"^使用者將會計年度 (\d+) 複製為 (\d+)$")]
async fn copy(w: &mut ErpWorld, from: i32, to: i32) {
    let r = w.periods.copy(&w.actor, from, to).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "查詢日期 {string} 所屬的會計期間")]
async fn locate(w: &mut ErpWorld, d: String) {
    let r = w.periods.locate(&w.actor, day(&d)).await;
    w.accounting_days = r.as_ref().ok().map(|x| vec![x.clone()]);
    w.last = Some(r.map(drop));
}

#[then(regex = r"^所屬會計期間應為年度 (\d+)、第 (\d+) 季、第 (\d+) 期、第 (\d+) 週$")]
async fn located(w: &mut ErpWorld, year: i32, q: i16, p: i16, wk: i16) {
    ok(w, "查詢會計期間");
    let d = &w.accounting_days.as_ref().expect("尚未查詢")[0];
    assert_eq!(
        (d.year, d.quarter, d.period, d.week),
        (year, q, p, wk),
        "{d:?}"
    );
}

#[when(regex = r"^查詢會計年度 (\d+) 第 (\d+) 期的起訖日$")]
async fn period_dates(w: &mut ErpWorld, year: i32, no: i16) {
    let r = w.periods.period_dates(&w.actor, year, no).await;
    w.period_range = r.as_ref().ok().map(|p| (p.start_date, p.end_date));
    w.last = Some(r.map(drop));
}

#[then(expr = "起訖日應為 {string} 至 {string}")]
async fn range_is(w: &mut ErpWorld, a: String, b: String) {
    ok(w, "查詢起訖日");
    assert_eq!(w.period_range, Some((day(&a), day(&b))));
}

#[when(expr = "查詢 {string} 至 {string} 的週期期間")]
async fn days(w: &mut ErpWorld, a: String, b: String) {
    let r = w.periods.days(&w.actor, day(&a), day(&b)).await;
    w.accounting_days = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

#[then(expr = "週期期間應為:")]
async fn days_are(w: &mut ErpWorld, step: &Step) {
    ok(w, "週期期間查詢");
    let list = w.accounting_days.clone().expect("尚未查詢");
    let expected = rows(step);
    assert_eq!(list.len(), expected.len(), "{list:?}");
    for (d, r) in list.iter().zip(expected) {
        let actual = vec![
            d.date.to_string(),
            d.year.to_string(),
            d.quarter.to_string(),
            d.period.to_string(),
            d.week.to_string(),
        ];
        assert_eq!(actual, r);
    }
}
