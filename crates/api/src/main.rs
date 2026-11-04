use std::sync::Arc;

use tracing_subscriber::EnvFilter;
use ttrs_api::{AppState, actor_from_env, cors_from_env, router};
use ttrs_app::{Services, memory::InMemoryStore};
use ttrs_infra::PgStore;

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
            Services::new(Arc::new(InMemoryStore::seeded()))
        }
    };

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
