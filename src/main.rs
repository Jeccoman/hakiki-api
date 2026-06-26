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
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "hakiki_api=debug,tower_http=debug,info".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = routes::router(state::AppState::new())
        .layer(TraceLayer::new_for_http())
        // Permissive CORS is fine for local dev with the Next.js frontend;
        // tighten to the real origin before any public deployment.
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("Hakiki API listening on http://{addr}");
    axum::serve(listener, app).await.unwrap();
}
