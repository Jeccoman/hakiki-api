use axum::{extract::State, Json};

use crate::domain::VerificationSourceInfo;
use crate::error::AppError;
use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<VerificationSourceInfo>>, AppError> {
    let rows: Vec<(String, String, String, String, bool, bool, Option<String>)> = sqlx::query_as(
        "SELECT code, name, category, status, supports_person, supports_business, description
         FROM verification_sources ORDER BY code",
    )
    .fetch_all(&state.pool)
    .await?;

    let sources = rows
        .into_iter()
        .map(|(code, name, category, status, sp, sb, desc)| VerificationSourceInfo {
            code,
            name,
            category,
            status,
            supports_person: sp,
            supports_business: sb,
            description: desc,
        })
        .collect();

    Ok(Json(sources))
}