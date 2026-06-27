use axum::{extract::State, Json};
use chrono::Utc;
use uuid::Uuid;

use crate::domain::{ContactMessage, CreateContact};
use crate::error::AppError;
use crate::state::AppState;

/// Capture a contact-form enquiry from the marketing site, including the
/// free-text message (which the waitlist endpoint does not store).
pub async fn submit(
    State(state): State<AppState>,
    Json(body): Json<CreateContact>,
) -> Result<Json<ContactMessage>, AppError> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }
    if !body.email.contains('@') {
        return Err(AppError::BadRequest("a valid email is required".into()));
    }

    let entry = ContactMessage {
        id: Uuid::new_v4(),
        name: body.name,
        email: body.email,
        organisation: body.organisation,
        message: body.message,
        created_at: Utc::now(),
    };

    sqlx::query(
        "insert into contact_messages (id, name, email, organisation, message, created_at)
         values ($1, $2, $3, $4, $5, $6)",
    )
    .bind(entry.id)
    .bind(&entry.name)
    .bind(&entry.email)
    .bind(&entry.organisation)
    .bind(&entry.message)
    .bind(entry.created_at)
    .execute(&state.pool)
    .await?;

    Ok(Json(entry))
}
