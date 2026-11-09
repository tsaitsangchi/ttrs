use std::sync::Arc;

use tracing_subscriber::EnvFilter;
use ttrs_api::{AppState, actor_from_env, cors_from_env, router};
use ttrs_app::{Services, mail::MailSettings, memory::InMemoryStore};
use ttrs_infra::{PgStore, SmtpMailer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .init();

    // 有 DATABASE_URL 就用 PostgreSQL（並自動 migrate），否則用記憶體資料方便前端開發
    let services = match std::env::var("DATABASE_URL") {
        Ok(url) => {
            let pool = ttrs_infra::connect(&url).await?;
            ttrs_infra::migrate(&pool).await?;
            tracing::info!("using PostgreSQL");
            Services::new(Arc::new(PgStore::new(pool)))
        }
        Err(_) => {
            // 記憶體模式沒有真正的資料庫結構，資料字典只能看到空清單
            tracing::warn!("DATABASE_URL not set, using in-memory store");
            // 起始目錄同 migrations/0026（側欄樹狀選單，menu_maintenance.feature MN14）
            let store = InMemoryStore::seeded();
            store.add_default_menus();
            Services::new(Arc::new(store))
        }
    };

    let services = with_mail(services)?;

    let addr = std::env::var("TTRS_API_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{addr}");
    // 以使用者代號＋Web 密碼登入（ADR 0009）；TTRS_AUTH=fixed 時不登入、固定以 TTRS_USER 執行（本機截圖用）
    let state = if std::env::var("TTRS_AUTH").is_ok_and(|v| v == "fixed") {
        let actor = actor_from_env();
        tracing::warn!(user = %actor.user_id, "sign-in disabled: acting as fixed user");
        AppState::new(services, actor)
    } else {
        let secure = std::env::var("TTRS_COOKIE_SECURE").is_ok_and(|v| v == "1");
        AppState::with_sign_in(services, secure)
    };
    let mut app = router(state);
    if let Some(cors) = cors_from_env() {
        tracing::info!("CORS enabled from TTRS_CORS_ORIGINS");
        app = app.layer(cors);
    }
    axum::serve(listener, app).await?;
    Ok(())
}

/// 忘記密碼的寄信設定（ADR 0010 R4、R9）：TTRS_SMTP_* 與 TTRS_PUBLIC_URL 都設定才開放
fn with_mail(mut services: Services) -> Result<Services, Box<dyn std::error::Error>> {
    let Some(mailer) = SmtpMailer::from_env()? else {
        tracing::info!("TTRS_SMTP_HOST not set: forgot-password disabled");
        return Ok(services);
    };
    let public_url = std::env::var("TTRS_PUBLIC_URL")
        .ok()
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .ok_or("TTRS_SMTP_HOST 已設定，但 TTRS_PUBLIC_URL 未設定或不是 http(s) 網址")?;
    tracing::info!(%public_url, "forgot-password enabled");
    services.password_reset = services.password_reset.with_mail(MailSettings {
        mailer: Arc::new(mailer),
        public_url,
    });
    Ok(services)
}
