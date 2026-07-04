use axum::{extract::State, Json};
use uuid::Uuid;

use crate::domain::ApiRequestLog;
use crate::error::AppError;
use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<ApiRequestLog>>, AppError> {
    // For now return recent requests (partner can only see their own in future with better org model)
    let rows: Vec<(Uuid, String, String, Option<i32>, Option<i32>, Option<String>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            r#"
            SELECT id, method, path, status_code, latency_ms, request_id, created_at
            FROM api_requests
            ORDER BY created_at DESC
            LIMIT 100
            "#,
        )
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