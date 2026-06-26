use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::{
    CheckKind, CheckResult, CreateVerification, Outcome, Subject, SubjectKind, Verification,
    VerificationStatus,
};
use crate::error::AppError;
use crate::sources::VerificationSource;
use crate::state::AppState;

/// Run a verification and persist it as an append-only audit record (the
/// verification, the consent that authorised it, and every check result).
/// Telco is live; the rest report `unsupported` until their integration lands.
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

    let mut tx = state.pool.begin().await?;

    sqlx::query(
        "insert into verifications
           (id, subject_kind, subject_reference, subject_full_name, subject_phone, status, created_at)
         values ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(verification.id)
    .bind(verification.subject.kind.as_db())
    .bind(&verification.subject.reference)
    .bind(&verification.subject.full_name)
    .bind(&verification.subject.phone)
    .bind(verification.status.as_db())
    .bind(verification.created_at)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "insert into consents (id, verification_id, method, granted_at)
         values ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(verification.id)
    .bind(&req.consent.method)
    .bind(req.consent.granted_at)
    .execute(&mut *tx)
    .await?;

    for r in &verification.results {
        sqlx::query(
            "insert into check_results (id, verification_id, check_kind, outcome, detail)
             values ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind(verification.id)
        .bind(r.check.as_db())
        .bind(r.outcome.as_db())
        .bind(&r.detail)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(Json(verification))
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Verification>, AppError> {
    let row: Option<(String, String, Option<String>, Option<String>, String, DateTime<Utc>)> =
        sqlx::query_as(
            "select subject_kind, subject_reference, subject_full_name, subject_phone, status, created_at
             from verifications where id = $1",
        )
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;

    let (kind, reference, full_name, phone, status, created_at) = row.ok_or(AppError::NotFound)?;

    let result_rows: Vec<(String, String, String)> = sqlx::query_as(
        "select check_kind, outcome, detail from check_results
         where verification_id = $1 order by created_at",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    let results = result_rows
        .into_iter()
        .map(|(check, outcome, detail)| CheckResult {
            check: CheckKind::from_db(&check),
            outcome: Outcome::from_db(&outcome),
            detail,
        })
        .collect();

    Ok(Json(Verification {
        id,
        status: VerificationStatus::from_db(&status),
        subject: Subject {
            kind: SubjectKind::from_db(&kind),
            reference,
            full_name,
            phone,
        },
        results,
        created_at,
    }))
}
