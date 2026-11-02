use std::sync::Arc;

use tracing_subscriber::EnvFilter;
use ttrs_api::{AppState, actor_from_env, cors_from_env, router};
use ttrs_app::{
    DataDictionaryService, ItemService, LegacyCatalogService, PermissionClassService,
    ProgramService, UserService, memory::InMemoryStore,
};
use ttrs_infra::PgStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .init();

    // 有 DATABASE_URL 就用 PostgreSQL（並自動 migrate），否則用記憶體資料方便前端開發
    let (items, programs, dictionary, legacy, users, classes) = match std::env::var("DATABASE_URL")
    {
        Ok(url) => {
            let pool = ttrs_infra::connect(&url).await?;
            ttrs_infra::migrate(&pool).await?;
            tracing::info!("using PostgreSQL");
            let store = Arc::new(PgStore::new(pool));
            (
                ItemService::new(store.clone(), store.clone()),
                ProgramService::new(store.clone()),
                DataDictionaryService::new(store.clone(), store.clone(), store.clone()),
                LegacyCatalogService::new(store.clone()),
                UserService::new(store.clone(), store.clone(), store.clone(), store.clone()),
                PermissionClassService::new(store.clone(), store.clone(), store),
            )
        }
        Err(_) => {
            tracing::warn!("DATABASE_URL not set, using in-memory store");
            let store = Arc::new(InMemoryStore::seeded());
            (
                ItemService::new(store.clone(), store.clone()),
                ProgramService::new(store.clone()),
                // 記憶體模式沒有真正的資料庫結構，資料字典只能看到空清單
                DataDictionaryService::new(store.clone(), store.clone(), store.clone()),
                LegacyCatalogService::new(store.clone()),
                UserService::new(store.clone(), store.clone(), store.clone(), store.clone()),
                PermissionClassService::new(store.clone(), store.clone(), store),
            )
        }
    };

    let addr = std::env::var("TTRS_API_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{addr}");
    let actor = actor_from_env();
    tracing::info!(user = %actor.user_id, "acting as development user (no login yet)");
    let mut app = router(AppState {
        items,
        programs,
        dictionary,
        legacy,
        users,
        classes,
        actor,
    });
    if let Some(cors) = cors_from_env() {
        tracing::info!("CORS enabled from TTRS_CORS_ORIGINS");
        app = app.layer(cors);
    }
    axum::serve(listener, app).await?;
    Ok(())
}
