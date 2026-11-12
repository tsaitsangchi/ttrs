//! 每日匯率排程（TipTop aooi070 ★ 背景執行 aooi070.4gl:141-160；specs/features/aoo/exchange_rate_import.feature IM8）
//!
//! 步驟與規則在 `ExchangeRateService::run_daily_job`（台銀即期匯入 → 海關匯入 → 每日匯率複製 → 昨天的月底重評價）。
//! 這裡只負責：服務帳號、失敗通知的收件人、每天定時執行（API 伺服器）與手動執行（`ttrs-admin rate-job`）的輸出。
//!
//! 環境變數：
//! - `TTRS_RATE_JOB_AT`：每天執行的時間 HH:MM（伺服器當地時間）；未設定時 API 伺服器不排程
//! - `TTRS_RATE_JOB_USER`：服務帳號，預設 `ttrs-batch`（權限類別 ADMIN，ADR 0006 §6）
//! - `TTRS_ADMIN_MAIL`：排程有步驟失敗時的收件人（需 SMTP，ADR 0010）

use std::sync::Arc;

use chrono::{Local, NaiveDateTime, NaiveTime, TimeDelta};
use ttrs_app::{Actor, ActorKind, ExchangeRateService, mail::Mailer};
use ttrs_shared::RateJobStep;

/// 服務帳號預設代號
pub const DEFAULT_JOB_USER: &str = "ttrs-batch";

/// 排程的執行者：服務帳號（ActorKind::Service），權限類別 ADMIN
pub fn job_actor() -> Actor {
    let user_id = std::env::var("TTRS_RATE_JOB_USER")
        .ok()
        .map(|u| u.trim().to_owned())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| DEFAULT_JOB_USER.to_owned());
    Actor {
        user_id,
        dept: String::new(),
        permission_class: "ADMIN".to_owned(),
        kind: ActorKind::Service,
    }
}

/// 有寄信工具且設定了 `TTRS_ADMIN_MAIL` 時，排程失敗寄信給管理員
pub fn with_job_mail(
    rates: ExchangeRateService,
    mailer: Option<Arc<dyn Mailer>>,
) -> ExchangeRateService {
    let to = std::env::var("TTRS_ADMIN_MAIL")
        .ok()
        .map(|m| m.trim().to_owned())
        .filter(|m| !m.is_empty());
    match (mailer, to) {
        (Some(mailer), Some(to)) => {
            tracing::info!(%to, "rate job failures will be mailed");
            rates.with_job_mail(mailer, &to)
        }
        (None, Some(_)) => {
            tracing::warn!("TTRS_ADMIN_MAIL 已設定，但 TTRS_SMTP_HOST 未設定：排程失敗不會寄信");
            rates
        }
        _ => rates,
    }
}

/// 解析 `TTRS_RATE_JOB_AT`（HH:MM）；未設定或空白為 None
pub fn parse_job_time(value: Option<&str>) -> Result<Option<NaiveTime>, String> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) => NaiveTime::parse_from_str(v, "%H:%M")
            .map(Some)
            .map_err(|_| format!("TTRS_RATE_JOB_AT 須為 HH:MM（如 16:30）：{v}")),
    }
}

/// 下一次執行的時間：今天的指定時間還沒到就是今天，否則明天
pub fn next_run(now: NaiveDateTime, at: NaiveTime) -> NaiveDateTime {
    let today = now.date().and_time(at);
    if today > now {
        today
    } else {
        today + TimeDelta::days(1)
    }
}

/// 每天在指定時間執行排程（背景工作，跟著 API 伺服器啟動與結束）
pub fn spawn_daily(rates: ExchangeRateService, at: NaiveTime) {
    tracing::info!(%at, "daily exchange rate job scheduled");
    tokio::spawn(async move {
        loop {
            let now = Local::now().naive_local();
            let wait = (next_run(now, at) - now)
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(60));
            tokio::time::sleep(wait).await;
            let steps = rates.run_daily_job(&job_actor()).await;
            if steps.iter().all(|s| s.ok) {
                tracing::info!(result = %report(&steps), "daily exchange rate job finished");
            } else {
                tracing::warn!(result = %report(&steps), "daily exchange rate job had failures");
            }
            // 避免執行時間少於一秒時同一分鐘內重複執行
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
}

/// 排程結果的文字（每步一行）
pub fn report(steps: &[RateJobStep]) -> String {
    steps
        .iter()
        .map(|s| {
            format!(
                "{}：{}（{}）",
                s.step,
                if s.ok { "成功" } else { "失敗" },
                s.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(d: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, d)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    #[test]
    fn job_time() {
        assert_eq!(parse_job_time(None), Ok(None));
        assert_eq!(parse_job_time(Some(" ")), Ok(None));
        assert_eq!(
            parse_job_time(Some("16:30")),
            Ok(NaiveTime::from_hms_opt(16, 30, 0))
        );
        assert!(parse_job_time(Some("4pm")).is_err());
    }

    #[test]
    fn next_run_today_or_tomorrow() {
        let t = NaiveTime::from_hms_opt(16, 30, 0).unwrap();
        assert_eq!(next_run(at(11, 9, 0), t), at(11, 16, 30));
        assert_eq!(next_run(at(11, 16, 30), t), at(12, 16, 30));
        assert_eq!(next_run(at(11, 23, 59), t), at(12, 16, 30));
    }

    #[test]
    fn service_actor() {
        let a = job_actor();
        assert_eq!(a.kind, ActorKind::Service);
        assert_eq!(a.permission_class, "ADMIN");
    }

    #[test]
    fn report_lines() {
        let steps = vec![
            RateJobStep {
                step: "台銀匯入".into(),
                ok: true,
                message: "匯入 USD".into(),
            },
            RateJobStep {
                step: "海關匯入".into(),
                ok: false,
                message: "無法取得".into(),
            },
        ];
        assert_eq!(
            report(&steps),
            "台銀匯入：成功（匯入 USD）\n海關匯入：失敗（無法取得）"
        );
    }
}
