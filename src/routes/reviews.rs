use axum::{
    extract::{Extension, Path, State},
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::domain::{AuthContext, ReviewCase, ReviewDecisionBody};
use crate::error::AppError;
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Result<Json<Vec<ReviewCase>>, AppError> {
    let org_id = ctx.organization_id;
    let rows: Vec<(Uuid, Uuid, String, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as(
            "SELECT id, verification_id, status, priority, created_at, resolved_at
             FROM review_cases WHERE organization_id = $1 ORDER BY created_at DESC LIMIT 100",
        )
        .bind(org_id)
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, vid, status, priority, created, resolved)| ReviewCase {
                id,
                verification_id: vid,
                status,
                priority,
                created_at: created,
                resolved_at: resolved,
            })
            .collect(),
    ))
}

pub async fn get_one(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<ReviewCase>, AppError> {
    let org_id = ctx.organization_id;
    let row: Option<(Uuid, Uuid, String, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as(
            "SELECT id, verification_id, status, priority, created_at, resolved_at FROM review_cases WHERE id = $1 AND organization_id = $2",
        )
        .bind(id)
        .bind(org_id)
        .fetch_optional(&state.pool)
        .await?;

    let (rid, vid, status, prio, created, resolved) = row.ok_or(AppError::NotFound)?;
    Ok(Json(ReviewCase {
        id: rid,
        verification_id: vid,
        status,
        priority: prio,
        created_at: created,
        resolved_at: resolved,
    }))
}

pub async fn approve(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<ReviewDecisionBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let now = Utc::now();
    let org_id = ctx.organization_id;

    // Update review case with scope
    let updated = sqlx::query(
        "UPDATE review_cases SET status = 'approved', resolved_at = $1, resolution = $2 WHERE id = $3 AND organization_id = $4",
    )
    .bind(now)
    .bind(body.reason.clone().unwrap_or_else(|| "Approved via API".into()))
    .bind(id)
    .bind(org_id)
    .execute(&state.pool)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    // Get verification id
    let vid_row: Option<(Uuid,)> = sqlx::query_as("SELECT verification_id FROM review_cases WHERE id = $1 AND organization_id = $2")
        .bind(id)
        .bind(org_id)
        .fetch_optional(&state.pool)
        .await?;

    if let Some((vid,)) = vid_row {
        // Update verification to approved with scope
        sqlx::query(
            "UPDATE verifications SET status = 'approved', decision = 'approved', completed_at = $1, updated_at = $1 WHERE id = $2 AND organization_id = $3",
        )
        .bind(now)
        .bind(vid)
        .bind(org_id)
        .execute(&state.pool)
        .await?;

        let actor = format!("member:{}", ctx.partner_name); // or use real when dashboard
        sqlx::query(
            "INSERT INTO audit_events (id, organization_id, verification_id, event_type, actor, payload, created_at)
             VALUES ($1, $2, $3, 'review.approved', $4, $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(vid)
        .bind(actor)
        .bind(serde_json::json!({ "review_case": id.to_string(), "reason": body.reason }))
        .bind(now)
        .execute(&state.pool)
        .await?;
    }

    Ok(Json(serde_json::json!({ "status": "approved" })))
}

pub async fn reject(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ReviewDecisionBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let now = Utc::now();

    let updated = sqlx::query(
        "UPDATE review_cases SET status = 'rejected', resolved_at = $1, resolution = $2 WHERE id = $3",
    )
    .bind(now)
    .bind(body.reason.clone().unwrap_or_else(|| "Rejected via API".into()))
    .bind(id)
    .execute(&state.pool)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    if let Some((vid,)) = sqlx::query_as::<_, (Uuid,)>("SELECT verification_id FROM review_cases WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
    {
        sqlx::query(
            "UPDATE verifications SET status = 'rejected', decision = 'rejected', completed_at = $1, updated_at = $1 WHERE id = $2",
        )
        .bind(now)
        .bind(vid)
        .execute(&state.pool)
        .await?;

        sqlx::query(
            "INSERT INTO audit_events (id, verification_id, event_type, actor, payload, created_at)
             VALUES ($1,$2,'review.rejected','api',$3,$4)",
        )
        .bind(Uuid::new_v4())
        .bind(vid)
        .bind(serde_json::json!({ "review_case": id.to_string() }))
        .bind(now)
        .execute(&state.pool)
        .await?;
    }

    Ok(Json(serde_json::json!({ "status": "rejected" })))
}

pub async fn add_note(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<crate::domain::CreateReviewNote>,
) -> Result<Json<serde_json::Value>, AppError> {
    sqlx::query(
        "INSERT INTO review_notes (id, review_case_id, author, note, created_at) VALUES ($1,$2,$3,$4,now())",
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind("api-key")
    .bind(&body.note)
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "note_added": true })))
}