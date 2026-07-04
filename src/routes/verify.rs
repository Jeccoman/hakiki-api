use axum::{
    extract::{Extension, Path, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::domain::{
    AuthContext, CheckKind, CheckResult, CreateVerification, Decision, Environment,
    NormalizedCheckResult, Outcome, RiskAssessment, RiskLevel, Verification,
    VerificationStatus,
};
use crate::error::AppError;
use crate::risk::score_verification;
use crate::sources::VerificationSource;
use crate::state::AppState;

/// Create a verification with full lifecycle.
/// - Records consent
/// - Runs requested sources (sandbox mocks)
/// - Normalizes results
/// - Scores risk
/// - Decides status
/// - Persists audit events + risk + flags
/// - Creates review case when needed
pub async fn create(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    headers: HeaderMap,
    Json(req): Json<CreateVerification>,
) -> Result<Json<Verification>, AppError> {
    if req.checks.is_empty() {
        return Err(AppError::BadRequest("at least one check is required".into()));
    }

    let verification_id = Uuid::new_v4();
    let org_id = ctx.organization_id;
    let now = Utc::now();

    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // 1. Execute sources (map old simple trait results into normalized)
    let mut normalized_results: Vec<NormalizedCheckResult> = Vec::new();
    let mut legacy_results: Vec<CheckResult> = Vec::new();

    for check in &req.checks {
        let norm = match check {
            CheckKind::TelcoIdentity => state.telco.as_ref().verify(&req.subject).await,
            CheckKind::Nida => state.nida.as_ref().verify(&req.subject).await,
            CheckKind::Brela => state.brela.as_ref().verify(&req.subject).await,
            CheckKind::Tra => state.tra.as_ref().verify(&req.subject).await,
            CheckKind::BankAccount => state.bank.as_ref().verify(&req.subject).await,
        };

        // Legacy simple view for response compatibility
        legacy_results.push(CheckResult {
            check: norm.check_kind,
            outcome: norm.outcome.clone(),
            detail: norm.message.clone().unwrap_or_else(|| norm.source.clone()),
            confidence: norm.confidence,
            message: norm.message.clone(),
        });

        normalized_results.push(norm);
    }

    // 2. Risk scoring
    let assessment: RiskAssessment = score_verification(
        &normalized_results,
        req.subject.full_name.as_deref(),
        req.subject.phone.as_deref(),
    );

    // 3. Decide lifecycle status
    let (status, decision, completed_at) = match assessment.decision {
        Decision::Approved => (VerificationStatus::Approved, Some(Decision::Approved), Some(now)),
        Decision::NeedsReview => (VerificationStatus::NeedsReview, Some(Decision::NeedsReview), None),
        Decision::Rejected => (VerificationStatus::Rejected, Some(Decision::Rejected), Some(now)),
        Decision::Failed => (VerificationStatus::Failed, Some(Decision::Failed), Some(now)),
    };

    let risk_score = assessment.score;
    let risk_level = assessment.level.clone();

    // Persist using shared function
    crate::db::persist_created_verification(
        &state.pool,
        verification_id,
        org_id,
        &req.subject,
        status.clone(),
        decision.clone(),
        risk_score,
        risk_level.clone(),
        now,
        completed_at,
        ctx.environment,
        &normalized_results,
        &assessment,
        &req.checks,
        "system:engine",
        &req.consent,
    ).await?;

    let request_hash = crate::auth::sha256_hex(&serde_json::to_string(&req).unwrap_or_default());

    // Build response (rich but compatible)
    let verification = Verification {
        id: verification_id,
        status,
        subject: req.subject,
        results: legacy_results,
        created_at: now,
        decision: Some(assessment.decision),
        decision_reason: None,
        risk_score: Some(risk_score),
        risk_level: Some(assessment.level),
        completed_at,
        expires_at: Some(now + Duration::days(90)),
        failure_reason: None,
        environment: Some(Environment::Test),
    };

    // Store idempotency result if key was provided (best effort)
    if let Some(key) = &idempotency_key {
        let resp_json = serde_json::to_value(&verification).unwrap_or(serde_json::json!({}));
        let _ = sqlx::query(
            r#"
            INSERT INTO idempotency_keys (id, organization_id, key, request_hash, response_body, status_code, created_at)
            VALUES ($1, $2, $3, $4, $5, 200, now())
            ON CONFLICT (organization_id, key) DO NOTHING
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(key)
        .bind(&request_hash)
        .bind(resp_json)
        .execute(&state.pool)
        .await;
    }

    Ok(Json(verification))
}

/// Retrieve a verification with its results.
pub async fn get_one(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Verification>, AppError> {
    let org_id = ctx.organization_id;

    // Load header with new columns + enforce org scoping
    let row: Option<(
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        Option<i32>,
        Option<String>,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT subject_kind, subject_reference, subject_full_name, subject_phone,
               status, decision, decision_reason, risk_score, risk_level,
               created_at, completed_at, failure_reason, environment
        FROM verifications WHERE id = $1 AND organization_id = $2
        "#,
    )
    .bind(id)
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await?;

    let (
        kind,
        reference,
        full_name,
        phone,
        status_str,
        decision_str,
        decision_reason,
        risk_score,
        risk_level_str,
        created_at,
        completed_at,
        failure_reason,
        env_str,
    ) = row.ok_or(AppError::NotFound)?;

    // Load results (enhanced columns)
    let result_rows: Vec<(
        String,
        String,
        String,
        Option<f32>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT check_kind, outcome, detail, confidence, message FROM check_results
         WHERE verification_id = $1 ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    let results = result_rows
        .into_iter()
        .map(|(ck, out, detail, conf, msg)| CheckResult {
            check: CheckKind::from_db(&ck),
            outcome: Outcome::from_db(&out),
            detail,
            confidence: conf,
            message: msg,
        })
        .collect();

    Ok(Json(Verification {
        id,
        status: VerificationStatus::from_db(&status_str),
        subject: crate::domain::Subject {
            kind: crate::domain::SubjectKind::from_db(&kind),
            reference,
            full_name,
            phone,
        },
        results,
        created_at,
        decision: decision_str.and_then(|s| Decision::from_db(&s)),
        decision_reason,
        risk_score,
        risk_level: risk_level_str.and_then(|s| RiskLevel::from_db(&s)),
        completed_at,
        expires_at: None,
        failure_reason,
        environment: env_str.map(|e| Environment::from_db(&e)),
    }))
}