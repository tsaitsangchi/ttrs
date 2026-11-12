//! ttrs-admin：伺服器端緊急重設（ADR 0010 §B）與每日匯率排程（exchange_rate_import.feature IM8）。
//! 只能在伺服器上以 DATABASE_URL 連線執行，沒有 HTTP API。

use std::{process::ExitCode, sync::Arc};

use ttrs_api::admin_cli::{self, USAGE};
use ttrs_api::rate_job;
use ttrs_app::{Services, mail::Mailer};
use ttrs_infra::{PgStore, SmtpMailer};

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL 未設定\n\n{USAGE}");
        return ExitCode::from(2);
    };
    let pool = match ttrs_infra::connect(&url).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("無法連線資料庫：{e}");
            return ExitCode::FAILURE;
        }
    };
    let services = Services::new(Arc::new(PgStore::new(pool)));
    if args.first().map(String::as_str) == Some("rate-job") {
        return run_rate_job(services).await;
    }
    // 稽核紀錄的執行者：以 sudo 執行時記錄原本的使用者
    let operator = ["SUDO_USER", "USER", "USERNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "unknown".into());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match admin_cli::run(&services.emergency, &args, &operator).await {
        Ok(out) => {
            println!("{}", out.message);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// 立即執行每日匯率排程（cron 或手動）；有步驟失敗時結束碼為 1
async fn run_rate_job(services: Services) -> ExitCode {
    let mailer = match SmtpMailer::from_env() {
        Ok(m) => m.map(|m| Arc::new(m) as Arc<dyn Mailer>),
        Err(e) => {
            eprintln!("寄信設定不正確：{e}");
            return ExitCode::from(2);
        }
    };
    let rates = rate_job::with_job_mail(services.rates, mailer);
    let steps = rates.run_daily_job(&rate_job::job_actor()).await;
    println!("{}", rate_job::report(&steps));
    if steps.iter().all(|s| s.ok) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
