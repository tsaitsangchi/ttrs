//! ttrs-admin：伺服器端緊急重設（ADR 0010 §B）。只能在伺服器上以 DATABASE_URL 連線執行，沒有 HTTP API。

use std::{process::ExitCode, sync::Arc};

use ttrs_api::admin_cli::{self, USAGE};
use ttrs_app::Services;
use ttrs_infra::PgStore;

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
