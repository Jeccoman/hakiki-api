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
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::domain::AuthContext;
use crate::state::AppState;

static RATE_LIMIT: once_cell::sync::Lazy<Mutex<HashMap<String, (u32, u64)>>> = once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

// Simple in-memory rate limiter per API key (key = api_key_id or presented key)
fn check_rate_limit(key: &str, limit_per_min: u32) -> (bool, u32, u64) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let minute = now / 60;
    let mut map = RATE_LIMIT.lock().unwrap();
    let entry = map.entry(key.to_string()).or_insert((0, minute));
    if entry.1 != minute {
        *entry = (0, minute);
    }
    entry.0 += 1;
    let remaining = if entry.0 > limit_per_min { 0 } else { limit_per_min - entry.0 };
    let exceeded = entry.0 > limit_per_min;
    (exceeded, remaining, (minute + 1) * 60 - now)
}

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

    // Try to get auth context for org scoping (may not be present for public routes)
    let (org_id, api_key_id) = if let Some(ctx) = req.extensions().get::<AuthContext>() {
        (Some(ctx.organization_id), Some(ctx.api_key_id))
    } else {
        (None, None)
    };

    // Rate limiting if API key context present
    let limit = std::env::var("HAKIKI_RATE_LIMIT_PER_MINUTE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100u32);

    let mut exceeded = false;
    let mut remaining = limit;
    let mut reset = 60u64;

    if let Some(key_id) = api_key_id {
        let key_str = key_id.to_string();
        let (ex, rem, rst) = check_rate_limit(&key_str, limit);
        exceeded = ex;
        remaining = rem;
        reset = rst;
    }

    if exceeded {
        let mut resp = Response::builder()
            .status(429)
            .header("X-RateLimit-Limit", limit.to_string())
            .header("X-RateLimit-Remaining", "0")
            .header("X-RateLimit-Reset", reset.to_string())
            .header("Retry-After", reset.to_string())
            .body(Body::from(r#"{"error":"rate limit exceeded"}"#))
            .unwrap();
        resp.headers_mut().insert(
            "x-request-id",
            request_id.parse().unwrap_or_else(|_| "1".parse().unwrap()),
        );
        return resp;
    }

    let response = next.run(req).await;

    let latency = start.elapsed().as_millis() as i32;
    let status = response.status().as_u16() as i32;

    // Best-effort insert (do not fail the response)
    let _ = sqlx::query(
        r#"
        INSERT INTO api_requests
          (id, api_key_id, organization_id, method, path, status_code, latency_ms, request_id, ip_address, user_agent, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(api_key_id)
    .bind(org_id)
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
    resp.headers_mut().insert("X-RateLimit-Limit", limit.to_string().parse().unwrap());
    resp.headers_mut().insert("X-RateLimit-Remaining", remaining.to_string().parse().unwrap());
    resp.headers_mut().insert("X-RateLimit-Reset", reset.to_string().parse().unwrap());

    resp
}