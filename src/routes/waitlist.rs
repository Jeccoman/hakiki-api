use axum::{extract::State, Json};
use chrono::Utc;
use uuid::Uuid;

use crate::domain::{JoinWaitlist, WaitlistEntry};
use crate::error::AppError;
use crate::state::AppState;

/// Capture a design partner / waitlist signup from the marketing site.
pub async fn join(
    State(state): State<AppState>,
    Json(body): Json<JoinWaitlist>,
) -> Result<Json<WaitlistEntry>, AppError> {
    if !body.email.contains('@') {
        return Err(AppError::BadRequest("a valid email is required".into()));
    }

    let entry = WaitlistEntry {
        id: Uuid::new_v4(),
        company: body.company,
        email: body.email,
        sector: body.sector,
        created_at: Utc::now(),
    };

    sqlx::query(
        "insert into waitlist_entries (id, company, email, sector, created_at)
         values ($1, $2, $3, $4, $5)",
    )
    .bind(entry.id)
    .bind(&entry.company)
    .bind(&entry.email)
    .bind(&entry.sector)
    .bind(entry.created_at)
    .execute(&state.pool)
    .await?;

    Ok(Json(entry))
}
