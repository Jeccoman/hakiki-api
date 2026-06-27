mod auth;
mod db;
mod domain;
mod error;
mod routes;
mod sources;
mod state;

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

    // Seed a publishable demo key (used by the marketing-site live demo) if one
    // is configured. Safe to expose while sources are mocked; real partner keys
    // are issued separately and kept secret.
    if let Ok(demo_key) = std::env::var("HAKIKI_DEMO_API_KEY") {
        db::upsert_api_key(&pool, "public-demo", &demo_key)
            .await
            .expect("failed to seed demo API key");
        tracing::info!("seeded 'public-demo' API key");
    }

    let app = routes::router(state::AppState::new(pool))
        .layer(TraceLayer::new_for_http())
        // Permissive CORS is fine for local dev with the Next.js frontend;
        // tighten to the real origin before any public deployment.
        .layer(CorsLayer::permissive());

    // Bind 127.0.0.1 locally; containers set BIND_ADDR=0.0.0.0:8080.
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let addr: SocketAddr = bind_addr.parse().expect("invalid BIND_ADDR");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("Hakiki API listening on http://{addr}");
    axum::serve(listener, app).await.unwrap();
}
