//! API-key authentication for partner-facing endpoints.
//!
//! Keys arrive in the `X-API-Key` header. We hash the presented key and look
//! the hash up in `api_keys`; an unknown or revoked key is a 401. The raw key
//! is never stored or logged.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};

use crate::error::AppError;
use crate::state::AppState;

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn require_api_key(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let presented = req
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let hash = sha256_hex(presented);

    let found: Option<(String,)> = sqlx::query_as(
        "select partner_name from api_keys where key_hash = $1 and revoked_at is null",
    )
    .bind(&hash)
    .fetch_optional(&state.pool)
    .await?;

    if found.is_none() {
        return Err(AppError::Unauthorized);
    }

    // Best-effort usage timestamp; don't fail the request if this write fails.
    let _ = sqlx::query("update api_keys set last_used_at = now() where key_hash = $1")
        .bind(&hash)
        .execute(&state.pool)
        .await;

    Ok(next.run(req).await)
}
