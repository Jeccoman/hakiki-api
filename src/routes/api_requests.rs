use axum::{extract::{Extension, State}, Json};
use uuid::Uuid;

use crate::domain::{ApiRequestLog, AuthContext};
use crate::error::AppError;
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Result<Json<Vec<ApiRequestLog>>, AppError> {
    let org_id = ctx.organization_id;

    let rows: Vec<(Uuid, String, String, Option<i32>, Option<i32>, Option<String>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            r#"
            SELECT id, method, path, status_code, latency_ms, request_id, created_at
            FROM api_requests
            WHERE organization_id = $1
            ORDER BY created_at DESC
            LIMIT 100
            "#,
        )
        .bind(org_id)
        .fetch_all(&state.pool)
        .await?;

    let logs = rows
        .into_iter()
        .map(|(id, method, path, status, latency, req_id, created)| ApiRequestLog {
            id,
            method,
            path,
            status_code: status,
            latency_ms: latency,
            request_id: req_id,
            created_at: created,
        })
        .collect();

    Ok(Json(logs))
}