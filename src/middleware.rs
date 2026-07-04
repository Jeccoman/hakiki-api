//! Request ID + API request logging middleware.
//!
//! - Accepts or generates X-Request-Id
//! - Records basic request telemetry into api_requests table (best effort)
//! - Does not block on logging failures

use axum::{
    body::Body,
    extract::State,
    http::{Request, Response},
    middleware::Next,
};
use std::time::Instant;
use uuid::Uuid;

use crate::state::AppState;

pub async fn request_id_and_logging(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let start = Instant::now();

    // Request ID
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Inject for downstream if needed
    req.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap_or_else(|_| "invalid".parse().unwrap()),
    );

    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .or_else(|| req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()))
        .unwrap_or("unknown")
        .to_string();
    let ua = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let response = next.run(req).await;

    let latency = start.elapsed().as_millis() as i32;
    let status = response.status().as_u16() as i32;

    // Best-effort insert (do not fail the response)
    let _ = sqlx::query(
        r#"
        INSERT INTO api_requests
          (id, method, path, status_code, latency_ms, request_id, ip_address, user_agent, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&method)
    .bind(&path)
    .bind(status)
    .bind(latency)
    .bind(&request_id)
    .bind(&ip)
    .bind(&ua)
    .execute(&state.pool)
    .await;

    // Add response header
    let mut resp = response;
    resp.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap_or_else(|_| "1".parse().unwrap()),
    );

    resp
}