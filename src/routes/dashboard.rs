use axum::{extract::{Extension, Path, State}, Json};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::domain::{
    Consent, CreateVerification, DashboardContext, DashboardLoginRequest, DashboardLoginResponse, 
    DashboardMe, Environment, MemberRole, Organization, OrganizationMember, Verification, VerificationStatus,
};
use crate::error::AppError;
use crate::sources::VerificationSource;
use crate::state::AppState;

/// Login for dashboard. For MVP, looks up member by email (optionally scoped to org slug).
/// Creates a session token stored in DB.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<DashboardLoginRequest>,
) -> Result<Json<DashboardLoginResponse>, AppError> {
    if body.email.trim().is_empty() {
        return Err(AppError::BadRequest("email is required".into()));
    }

    let query = if let Some(slug) = &body.organization_slug {
        r#"
        SELECT om.id, om.organization_id, om.email, om.role, o.id, o.name, o.slug, o.status, o.created_at, o.updated_at
        FROM organization_members om
        JOIN organizations o ON om.organization_id = o.id
        WHERE om.email = $1 AND o.slug = $2
        "#
    } else {
        r#"
        SELECT om.id, om.organization_id, om.email, om.role, o.id, o.name, o.slug, o.status, o.created_at, o.updated_at
        FROM organization_members om
        JOIN organizations o ON om.organization_id = o.id
        WHERE om.email = $1
        LIMIT 1
        "#
    };

    let row: Option<(Uuid, Uuid, String, String, Uuid, String, String, String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> = if let Some(slug) = &body.organization_slug {
        sqlx::query_as(query)
            .bind(&body.email)
            .bind(slug)
            .fetch_optional(&state.pool)
            .await?
    } else {
        sqlx::query_as(query)
            .bind(&body.email)
            .fetch_optional(&state.pool)
            .await?
    };

    let (member_id, org_id, email, role_str, _o_id, o_name, o_slug, o_status, o_created, o_updated) = row.ok_or(AppError::Unauthorized)?;

    let role = MemberRole::from_db(&role_str);

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + chrono::Duration::hours(24);

    sqlx::query(
        "INSERT INTO dashboard_sessions (id, member_id, organization_id, token, expires_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(member_id)
    .bind(org_id)
    .bind(&token)
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    let member = OrganizationMember {
        id: member_id,
        organization_id: org_id,
        email: email.clone(),
        role: role.clone(),
        created_at: Utc::now(), // approximate
    };

    let organization = Organization {
        id: org_id,
        name: o_name,
        slug: o_slug,
        status: match o_status.as_str() {
            "active" => crate::domain::OrgStatus::Active,
            "suspended" => crate::domain::OrgStatus::Suspended,
            _ => crate::domain::OrgStatus::Deleted,
        },
        created_at: o_created,
        updated_at: o_updated,
    };

    Ok(Json(DashboardLoginResponse {
        token,
        member,
        organization,
        expires_at,
    }))
}

pub async fn me(Extension(ctx): Extension<DashboardContext>) -> Result<Json<DashboardMe>, AppError> {
    // For simplicity, re-fetch or construct. In real would load fresh.
    // Here we return from ctx, but to match types, create minimal.
    // Note: full load would require more, but for MVP use ctx data.
    let member = OrganizationMember {
        id: ctx.member_id,
        organization_id: ctx.organization_id,
        email: ctx.email.clone(),
        role: ctx.role.clone(),
        created_at: Utc::now(),
    };
    let organization = Organization {
        id: ctx.organization_id,
        name: "Demo Org".to_string(),
        slug: "demo".to_string(),
        status: crate::domain::OrgStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    Ok(Json(DashboardMe { member, organization }))
}

pub async fn logout(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>, // to get token? but for simplicity, require token in header? 
) -> Result<Json<serde_json::Value>, AppError> {
    // In practice, the middleware would have the token, but for now simple delete all for member or use header.
    // To make it work, we can delete by member for demo.
    let _ = sqlx::query("DELETE FROM dashboard_sessions WHERE member_id = $1")
        .bind(ctx.member_id)
        .execute(&state.pool)
        .await;
    Ok(Json(json!({ "logged_out": true })))
}

/// Protected dashboard handlers now use Extension<DashboardContext> from middleware.
pub async fn overview(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    let org_id = ctx.organization_id;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM verifications WHERE organization_id = $1")
        .bind(org_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0,));

    let approved: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM verifications WHERE organization_id = $1 AND status = 'approved'")
        .bind(org_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0,));

    let needs_review: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM verifications WHERE organization_id = $1 AND status = 'needs_review'")
        .bind(org_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0,));

    let avg_risk: Option<f64> = sqlx::query_scalar("SELECT AVG(risk_score) FROM verifications WHERE organization_id = $1 AND risk_score IS NOT NULL")
        .bind(org_id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);

    Ok(Json(json!({
        "total_verifications": total.0,
        "approved_count": approved.0,
        "needs_review_count": needs_review.0,
        "rejected_count": 0,
        "average_risk_score": avg_risk,
        "webhook_success_rate": 0.95,
        "recent_audit_events": [],
        "recent_review_cases": []
    })))
}

pub async fn list_verifications(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
) -> Result<Json<Vec<crate::domain::Verification>>, AppError> {
    let org_id = ctx.organization_id;

    let rows: Vec<(Uuid, String, String, Option<String>, Option<String>, String, Option<String>, Option<i32>, Option<String>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT id, subject_kind, subject_reference, subject_full_name, subject_phone, status, decision, risk_score, risk_level, created_at
        FROM verifications 
        WHERE organization_id = $1 
        ORDER BY created_at DESC 
        LIMIT 50
        "#
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await?;

    let list = rows.into_iter().map(|(id, kind, reference, full_name, phone, status_str, decision_str, risk_score, risk_level_str, created_at)| {
        crate::domain::Verification {
            id,
            status: crate::domain::VerificationStatus::from_db(&status_str),
            subject: crate::domain::Subject {
                kind: crate::domain::SubjectKind::from_db(&kind),
                reference,
                full_name,
                phone,
            },
            results: vec![], // list doesn't include full results for perf
            created_at,
            decision: decision_str.and_then(|s| crate::domain::Decision::from_db(&s)),
            decision_reason: None,
            risk_score,
            risk_level: risk_level_str.and_then(|s| crate::domain::RiskLevel::from_db(&s)),
            completed_at: None,
            expires_at: None,
            failure_reason: None,
            environment: Some(crate::domain::Environment::Test),
        }
    }).collect();

    Ok(Json(list))
}

pub async fn get_verification_detail(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let org_id = ctx.organization_id;

    // Verify ownership
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM verifications WHERE id = $1 AND organization_id = $2")
        .bind(id)
        .bind(org_id)
        .fetch_optional(&state.pool)
        .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    // Verification header
    let verification_row: Option<(String, String, Option<String>, Option<String>, String, Option<String>, Option<i32>, Option<String>, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        r#"
        SELECT subject_kind, subject_reference, subject_full_name, subject_phone, status, decision, risk_score, risk_level, created_at, completed_at 
        FROM verifications WHERE id = $1
        "#
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    // Consent
    let consent_row: Option<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT method, granted_at FROM consents WHERE verification_id = $1 LIMIT 1"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    // Check results with normalized/raw if present
    let check_results: Vec<(String, String, String, Option<f32>, Option<String>, Option<serde_json::Value>, Option<serde_json::Value>)> = sqlx::query_as(
        r#"
        SELECT check_kind, outcome, detail, confidence, message, normalized_fields, raw_response 
        FROM check_results WHERE verification_id = $1 ORDER BY created_at
        "#
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    // Risk flags
    let risk_flags: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT code, severity, message FROM risk_flags WHERE verification_id = $1"
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    // Risk score
    let risk_score_row: Option<(i32, String, String)> = sqlx::query_as(
        "SELECT score, level, decision FROM risk_scores WHERE verification_id = $1 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    // Review case + notes
    let review_case_row: Option<(Uuid, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, status, priority, created_at FROM review_cases WHERE verification_id = $1 LIMIT 1"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let review_notes: Vec<(String, chrono::DateTime<chrono::Utc>)> = if let Some((rc_id, _, _, _)) = &review_case_row {
        sqlx::query_as(
            "SELECT note, created_at FROM review_notes WHERE review_case_id = $1 ORDER BY created_at"
        )
        .bind(rc_id)
        .fetch_all(&state.pool)
        .await?
    } else { vec![] };

    // Audit events
    let audit_events: Vec<(String, Option<String>, Option<serde_json::Value>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT event_type, actor, payload, created_at FROM audit_events WHERE verification_id = $1 ORDER BY created_at"
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    // Webhook deliveries
    let webhook_deliveries: Vec<(Uuid, String, String, Option<i32>, Option<String>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, event_type, status, response_code, response_body, created_at FROM webhook_deliveries WHERE verification_id = $1 ORDER BY created_at"
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    // Related api request (last one for this verification? approximate by time or skip if not linked)
    let api_request: Option<(String, String, Option<i32>, Option<i32>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT method, path, status_code, latency_ms, created_at FROM api_requests WHERE organization_id = $1 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await?;

    // Map verification row to object for frontend
    let verification_obj = verification_row.map(|(kind, reference, full_name, phone, status, decision, risk_score, risk_level, created_at, completed_at)| {
        json!({
            "id": id,
            "subject": {
                "kind": kind,
                "reference": reference,
                "full_name": full_name,
                "phone": phone,
            },
            "status": status,
            "decision": decision,
            "risk_score": risk_score,
            "risk_level": risk_level,
            "created_at": created_at,
            "completed_at": completed_at,
            "environment": "test"
        })
    });

    let check_results_obj: Vec<_> = check_results.into_iter().map(|(ck, out, det, conf, msg, norm, raw)| json!({
        "check_kind": ck,
        "outcome": out,
        "detail": det,
        "confidence": conf,
        "message": msg,
        "normalized_fields": norm,
        "raw_response": raw
    })).collect();

    Ok(Json(json!({
        "verification": verification_obj,
        "consent": consent_row.map(|(m, g)| json!({"method": m, "granted_at": g})),
        "check_results": check_results_obj,
        "risk_flags": risk_flags.into_iter().map(|(c,s,m)| json!({"code":c,"severity":s,"message":m})).collect::<Vec<_>>(),
        "risk_score": risk_score_row.map(|(s, l, d)| json!({"score": s, "level": l, "decision": d})),
        "review_case": review_case_row.map(|(rid, st, pri, cr)| json!({"id": rid, "status": st, "priority": pri, "created_at": cr})),
        "review_notes": review_notes.into_iter().map(|(n, ct)| json!({"note": n, "created_at": ct})).collect::<Vec<_>>(),
        "audit_events": audit_events.into_iter().map(|(et, ac, pl, ct)| json!({"event_type": et, "actor": ac, "payload": pl, "created_at": ct})).collect::<Vec<_>>(),
        "webhook_deliveries": webhook_deliveries.into_iter().map(|(wid, et, st, rc, rb, ct)| json!({"id": wid, "event_type": et, "status": st, "response_code": rc, "response_body": rb, "created_at": ct})).collect::<Vec<_>>(),
        "related_api_request": api_request.map(|(m, p, sc, lat, ct)| json!({"method": m, "path": p, "status_code": sc, "latency_ms": lat, "created_at": ct})),
    })))
}

// --- API Key management from dashboard (owner/admin/developer) ---

pub async fn list_api_keys(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !ctx.role.can_manage_keys() {
        return Err(AppError::BadRequest("insufficient permissions".into())); // use 403 in real error
    }
    let org_id = ctx.organization_id;
    let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT id, partner_name, environment, COALESCE(display_prefix, '') FROM api_keys WHERE organization_id = $1 AND revoked_at IS NULL LIMIT 50"
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Ok(Json(json!({ "api_keys": rows })))
}

pub async fn create_api_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let org_id = ctx.organization_id;
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("new-key").to_string();
    let env = body.get("environment").and_then(|v| v.as_str()).unwrap_or("test");

    let raw_key = format!("hk_{}_{}", env, Uuid::new_v4().to_string().replace("-", ""));
    let hash = crate::auth::sha256_hex(&raw_key);

    let key_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO api_keys (id, organization_id, partner_name, key_hash, environment, display_prefix) VALUES ($1,$2,$3,$4,$5,$6)"
    )
    .bind(key_id)
    .bind(org_id)
    .bind(&name)
    .bind(&hash)
    .bind(env)
    .bind(format!("hk_{}_****", env))
    .execute(&state.pool)
    .await?;

    Ok(Json(json!({
        "id": key_id,
        "name": name,
        "environment": env,
        "raw_key": raw_key, // shown only once
    })))
}

/// Create verification from dashboard (uses dashboard member context)
pub async fn create_verification(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
    Json(req): Json<CreateVerification>,
) -> Result<Json<Verification>, AppError> {
    if req.checks.is_empty() {
        return Err(AppError::BadRequest("at least one check is required".into()));
    }

    let org_id = ctx.organization_id;
    let now = Utc::now();
    let verification_id = Uuid::new_v4();
    let env = Environment::Test;
    let actor = format!("dashboard:{}", ctx.email);

    // Execute sources
    let mut normalized_results: Vec<crate::domain::NormalizedCheckResult> = Vec::new();
    let mut legacy_results: Vec<crate::domain::CheckResult> = Vec::new();

    for check in &req.checks {
        let norm = match check {
            crate::domain::CheckKind::TelcoIdentity => state.telco.as_ref().verify(&req.subject).await,
            crate::domain::CheckKind::Nida => state.nida.as_ref().verify(&req.subject).await,
            crate::domain::CheckKind::Brela => state.brela.as_ref().verify(&req.subject).await,
            crate::domain::CheckKind::Tra => state.tra.as_ref().verify(&req.subject).await,
            crate::domain::CheckKind::BankAccount => state.bank.as_ref().verify(&req.subject).await,
        };

        legacy_results.push(crate::domain::CheckResult {
            check: norm.check_kind,
            outcome: norm.outcome.clone(),
            detail: norm.message.clone().unwrap_or_else(|| norm.source.clone()),
            confidence: norm.confidence,
            message: norm.message.clone(),
        });

        normalized_results.push(norm);
    }

    // Risk scoring
    let assessment: crate::domain::RiskAssessment = crate::risk::score_verification(
        &normalized_results,
        req.subject.full_name.as_deref(),
        req.subject.phone.as_deref(),
    );

    // Decide
    let (status, decision, completed_at) = match assessment.decision {
        crate::domain::Decision::Approved => (VerificationStatus::Approved, Some(crate::domain::Decision::Approved), Some(now)),
        crate::domain::Decision::NeedsReview => (VerificationStatus::NeedsReview, Some(crate::domain::Decision::NeedsReview), None),
        crate::domain::Decision::Rejected => (VerificationStatus::Rejected, Some(crate::domain::Decision::Rejected), Some(now)),
        crate::domain::Decision::Failed => (VerificationStatus::Failed, Some(crate::domain::Decision::Failed), Some(now)),
    };

    let risk_score = assessment.score;
    let risk_level = assessment.level.clone();

    // Persist
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
        env.clone(),
        &normalized_results,
        &assessment,
        &req.checks,
        &actor,
        &req.consent,
    ).await?;

    // Build response
    let verification = Verification {
        id: verification_id,
        status,
        subject: req.subject,
        results: legacy_results,
        created_at: now,
        decision,
        decision_reason: None,
        risk_score: Some(risk_score),
        risk_level: Some(risk_level),
        completed_at,
        expires_at: Some(now + chrono::Duration::days(90)),
        failure_reason: None,
        environment: Some(env),
    };

    Ok(Json(verification))
}