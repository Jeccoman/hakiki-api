use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::domain::{
    CheckKind, CheckResult, CreateVerification, Outcome, Verification, VerificationStatus,
};
use crate::error::AppError;
use crate::sources::VerificationSource;
use crate::state::AppState;

/// Run a verification. Each requested check is routed to its source; telco is
/// live, the rest report `unsupported` until their integration lands.
pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateVerification>,
) -> Result<Json<Verification>, AppError> {
    if req.checks.is_empty() {
        return Err(AppError::BadRequest("at least one check is required".into()));
    }

    let mut results = Vec::with_capacity(req.checks.len());
    for check in &req.checks {
        let result = match check {
            CheckKind::TelcoIdentity => state.telco.verify(&req.subject).await,
            other => CheckResult {
                check: *other,
                outcome: Outcome::Unsupported,
                detail: "Source not yet connected — telco is the first integration".into(),
            },
        };
        results.push(result);
    }

    let verification = Verification {
        id: Uuid::new_v4(),
        status: VerificationStatus::Completed,
        subject: req.subject,
        results,
        created_at: Utc::now(),
    };

    state
        .verifications
        .write()
        .unwrap()
        .insert(verification.id, verification.clone());

    Ok(Json(verification))
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Verification>, AppError> {
    state
        .verifications
        .read()
        .unwrap()
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(AppError::NotFound)
}
