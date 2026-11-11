//! 匯率來源實際下載（exchange_rate_import.feature IM1）。需連外網，預設不執行：
//! `cargo test -p ttrs-infra --test rate_feed_live -- --ignored --nocapture`

use ttrs_app::ports::RateFeed;
use ttrs_domain::exchange_rate::feed::BankRateType;
use ttrs_infra::PgStore;

fn store() -> PgStore {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .expect("lazy pool");
    PgStore::new(pool)
}

#[tokio::test]
#[ignore = "需連外網"]
async fn customs_sources() {
    let s = store();
    let (start, end, quotes) = s.customs_current().await.expect("當旬");
    println!(
        "當旬 {start}～{end}：{} 筆，例：{:?}",
        quotes.len(),
        quotes.first()
    );
    assert!(quotes.len() >= 5);
    let history = s.customs_history().await.expect("歷史");
    println!("歷史 {} 筆，最後：{:?}", history.len(), history.last());
    assert!(history.len() > 1000);
}

#[tokio::test]
#[ignore = "需連外網；台銀網站可能以機器人驗證擋下"]
async fn bank_source() {
    match store().bank_quotes(BankRateType::Spot).await {
        Ok(q) => println!("台銀 {} 筆，例：{:?}", q.len(), q.first()),
        Err(e) => println!("台銀無法取得：{e}"),
    }
}
