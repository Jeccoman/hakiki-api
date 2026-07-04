mod auth;
mod db;
mod domain;
mod error;
mod middleware;
mod risk;
mod routes;
mod sources;
mod state;
mod webhook_worker;

use std::net::SocketAddr;

use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    // Load .env if present (no-op in production where env is set directly).
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hakiki_api=debug,tower_http=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set (see .env.example)");
    let pool = db::connect_and_migrate(&database_url)
        .await
        .expect("failed to connect to database / run migrations");

    // Seed demo organization + member + API key for marketing site + dashboard demo.
    let _ = db::ensure_demo_organization(&pool).await;
    if let Ok(demo_key) = std::env::var("HAKIKI_DEMO_API_KEY") {
        db::upsert_api_key(&pool, "public-demo", &demo_key)
            .await
            .expect("failed to seed demo API key");
        tracing::info!("seeded demo organization + 'public-demo' API key");
    }

    // Seed polished demo verification data for impressive demo
    if let Err(e) = db::seed_demo_data_if_needed(&pool).await {
        tracing::warn!("Demo data seed skipped or failed: {}", e);
    }

    let app_state = state::AppState::new(pool.clone());

    // Start background webhook delivery worker
    crate::webhook_worker::start_webhook_worker(app_state.clone());

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::mirror_request())
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any)
        .allow_credentials(true);

    let app = routes::router(app_state.clone())
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            crate::middleware::request_id_and_logging,
        ))
        .layer(cors);

    // Bind 127.0.0.1 locally; containers set BIND_ADDR=0.0.0.0:8080.
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let addr: SocketAddr = bind_addr.parse().expect("invalid BIND_ADDR");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("Hakiki API listening on http://{addr}");
    axum::serve(listener, app).await.unwrap();
}
