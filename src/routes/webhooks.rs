use axum::{
    extract::{Extension, Path, State},
    Json,
};
use uuid::Uuid;

use crate::domain::{AuthContext, CreateWebhookEndpoint, WebhookEndpoint};
use crate::error::AppError;
use crate::state::AppState;

/// List webhook endpoints for the authenticated partner (via current API key).
pub async fn list(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Result<Json<Vec<WebhookEndpoint>>, AppError> {
    let org_id = ctx.organization_id;
    let rows: Vec<(Uuid, String, Vec<String>, bool, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, url, events, active, created_at FROM webhook_endpoints WHERE organization_id = $1 ORDER BY created_at DESC LIMIT 50",
    )
    .bind(org_id)
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
    Extension(ctx): Extension<AuthContext>,
    Json(body): Json<CreateWebhookEndpoint>,
) -> Result<Json<WebhookEndpoint>, AppError> {
    if !body.url.starts_with("http") {
        return Err(AppError::BadRequest("url must start with http/https".into()));
    }

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let org_id = ctx.organization_id;

    sqlx::query(
        "INSERT INTO webhook_endpoints (id, organization_id, api_key_id, url, secret, events, active, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, true, $7, $7)",
    )
    .bind(id)
    .bind(org_id)
    .bind(ctx.api_key_id)
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

pub async fn delete(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let org_id = ctx.organization_id;
    let res = sqlx::query("DELETE FROM webhook_endpoints WHERE id = $1 AND organization_id = $2")
        .bind(id)
        .bind(org_id)
        .execute(&state.pool)
        .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn test(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let org_id = ctx.organization_id;
    // Stub: record a test delivery with org
    let delivery_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO webhook_deliveries (id, organization_id, webhook_endpoint_id, event_type, payload, status, attempts, created_at)
         VALUES ($1, $2, $3, 'verification.test', $4, 'pending', 0, now())",
    )
    .bind(delivery_id)
    .bind(org_id)
    .bind(id)
    .bind(serde_json::json!({ "test": true, "message": "This is a test webhook payload" }))
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "queued": true, "delivery_id": delivery_id })))
}

pub async fn deliveries(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    let org_id = ctx.organization_id;
    // Simple list of recent deliveries scoped
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM webhook_deliveries WHERE organization_id = $1")
        .bind(org_id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "total": count.0 })))
}

pub async fn resend(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let org_id = ctx.organization_id;
    let updated = sqlx::query(
        "UPDATE webhook_deliveries SET status='pending', next_attempt_at = now() WHERE id = $1 AND organization_id = $2"
    )
    .bind(id)
    .bind(org_id)
    .execute(&state.pool)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(serde_json::json!({ "resend_queued": true })))
}