use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

use crate::domain::{CreateWebhookEndpoint, WebhookEndpoint};
use crate::error::AppError;
use crate::state::AppState;

/// List webhook endpoints for the authenticated partner (via current API key).
/// For simplicity we return all for now; production would scope by api_key.
pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<WebhookEndpoint>>, AppError> {
    let rows: Vec<(Uuid, String, Vec<String>, bool, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, url, events, active, created_at FROM webhook_endpoints ORDER BY created_at DESC LIMIT 50",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, url, events, active, created)| WebhookEndpoint {
                id,
                url,
                events,
                active,
                created_at: created,
            })
            .collect(),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateWebhookEndpoint>,
) -> Result<Json<WebhookEndpoint>, AppError> {
    if !body.url.starts_with("http") {
        return Err(AppError::BadRequest("url must start with http/https".into()));
    }

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query(
        "INSERT INTO webhook_endpoints (id, api_key_id, url, secret, events, active, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, true, $6, $6)",
    )
    .bind(id)
    .bind(Uuid::new_v4()) // placeholder - in real flow we'd resolve from authenticated key
    .bind(&body.url)
    .bind(&body.secret)
    .bind(&body.events)
    .bind(now)
    .execute(&state.pool)
    .await?;

    Ok(Json(WebhookEndpoint {
        id,
        url: body.url,
        events: body.events,
        active: true,
        created_at: now,
    }))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<serde_json::Value>, AppError> {
    let res = sqlx::query("DELETE FROM webhook_endpoints WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn test(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<serde_json::Value>, AppError> {
    // Stub: record a test delivery
    let delivery_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO webhook_deliveries (id, webhook_endpoint_id, event_type, payload, status, attempts, created_at)
         VALUES ($1, $2, 'verification.test', $3, 'pending', 0, now())",
    )
    .bind(delivery_id)
    .bind(id)
    .bind(serde_json::json!({ "test": true, "message": "This is a test webhook payload" }))
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "queued": true, "delivery_id": delivery_id })))
}

pub async fn deliveries(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    // Simple list of recent deliveries
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM webhook_deliveries")
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "total": count.0 })))
}