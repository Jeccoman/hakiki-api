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

pub async fn me(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
) -> Result<Json<DashboardMe>, AppError> {
    let member = OrganizationMember {
        id: ctx.member_id,
        organization_id: ctx.organization_id,
        email: ctx.email.clone(),
        role: ctx.role.clone(),
        created_at: Utc::now(),
    };

    let org_row: Option<(String, String, String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as("SELECT name, slug, status, created_at, updated_at FROM organizations WHERE id = $1")
            .bind(ctx.organization_id)
            .fetch_optional(&state.pool)
            .await?;
    let (name, slug, status, created_at, updated_at) = org_row.ok_or(AppError::NotFound)?;

    let organization = Organization {
        id: ctx.organization_id,
        name,
        slug,
        status: match status.as_str() {
            "active" => crate::domain::OrgStatus::Active,
            "suspended" => crate::domain::OrgStatus::Suspended,
            _ => crate::domain::OrgStatus::Deleted,
        },
        created_at,
        updated_at,
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

    let rejected: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM verifications WHERE organization_id = $1 AND status = 'rejected'")
        .bind(org_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or((0,));

    let avg_risk: Option<f64> = sqlx::query_scalar("SELECT AVG(risk_score)::float8 FROM verifications WHERE organization_id = $1 AND risk_score IS NOT NULL")
        .bind(org_id)
        .fetch_optional(&state.pool)
        .await
        .unwrap_or(None);

    // Delivered / terminal deliveries; None when there is nothing to measure yet.
    let webhook_counts: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE status = 'delivered'), COUNT(*) FILTER (WHERE status IN ('delivered', 'failed'))
         FROM webhook_deliveries WHERE organization_id = $1",
    )
    .bind(org_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or((0, 0));
    let webhook_success_rate = if webhook_counts.1 > 0 {
        Some(webhook_counts.0 as f64 / webhook_counts.1 as f64)
    } else {
        None
    };

    let recent_audit_events: Vec<(String, Option<String>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT event_type, actor, created_at FROM audit_events WHERE organization_id = $1 ORDER BY created_at DESC LIMIT 8",
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let recent_review_cases: Vec<(Uuid, Uuid, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, verification_id, status, priority, created_at FROM review_cases WHERE organization_id = $1 ORDER BY created_at DESC LIMIT 5",
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Ok(Json(json!({
        "total_verifications": total.0,
        "approved_count": approved.0,
        "needs_review_count": needs_review.0,
        "rejected_count": rejected.0,
        "average_risk_score": avg_risk,
        "webhook_success_rate": webhook_success_rate,
        "recent_audit_events": recent_audit_events.into_iter()
            .map(|(event_type, actor, created_at)| json!({ "event_type": event_type, "actor": actor, "created_at": created_at }))
            .collect::<Vec<_>>(),
        "recent_review_cases": recent_review_cases.into_iter()
            .map(|(id, vid, status, priority, created_at)| json!({ "id": id, "verification_id": vid, "status": status, "priority": priority, "created_at": created_at }))
            .collect::<Vec<_>>(),
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

// --- Review cases (dashboard, org-scoped, member as actor) ---

pub async fn list_review_cases(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
) -> Result<Json<Vec<crate::domain::ReviewCase>>, AppError> {
    let rows: Vec<(Uuid, Uuid, String, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as(
            "SELECT id, verification_id, status, priority, created_at, resolved_at
             FROM review_cases WHERE organization_id = $1 ORDER BY created_at DESC LIMIT 100",
        )
        .bind(ctx.organization_id)
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, vid, status, priority, created, resolved)| crate::domain::ReviewCase {
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

pub async fn get_review_case(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let row: Option<(Uuid, Uuid, String, String, Option<String>, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as(
            "SELECT id, verification_id, status, priority, resolution, created_at, resolved_at
             FROM review_cases WHERE id = $1 AND organization_id = $2",
        )
        .bind(id)
        .bind(ctx.organization_id)
        .fetch_optional(&state.pool)
        .await?;

    let (rid, vid, status, priority, resolution, created_at, resolved_at) =
        row.ok_or(AppError::NotFound)?;

    let notes: Vec<(String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT author, note, created_at FROM review_notes WHERE review_case_id = $1 ORDER BY created_at",
    )
    .bind(rid)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(json!({
        "id": rid,
        "verification_id": vid,
        "status": status,
        "priority": priority,
        "resolution": resolution,
        "created_at": created_at,
        "resolved_at": resolved_at,
        "notes": notes.into_iter()
            .map(|(author, note, created)| json!({ "author": author, "note": note, "created_at": created }))
            .collect::<Vec<_>>(),
    })))
}

async fn resolve_review_case(
    state: &AppState,
    ctx: &DashboardContext,
    review_id: Uuid,
    outcome: &str, // "approved" | "rejected"
    reason: Option<String>,
) -> Result<(), AppError> {
    let now = Utc::now();
    let org_id = ctx.organization_id;

    let updated = sqlx::query(
        "UPDATE review_cases SET status = $1, resolved_at = $2, resolution = $3 WHERE id = $4 AND organization_id = $5",
    )
    .bind(outcome)
    .bind(now)
    .bind(reason.clone().unwrap_or_else(|| format!("{outcome} via dashboard")))
    .bind(review_id)
    .bind(org_id)
    .execute(&state.pool)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    let vid_row: Option<(Uuid,)> =
        sqlx::query_as("SELECT verification_id FROM review_cases WHERE id = $1 AND organization_id = $2")
            .bind(review_id)
            .bind(org_id)
            .fetch_optional(&state.pool)
            .await?;

    if let Some((vid,)) = vid_row {
        sqlx::query(
            "UPDATE verifications SET status = $1, decision = $1, completed_at = $2, updated_at = $2 WHERE id = $3 AND organization_id = $4",
        )
        .bind(outcome)
        .bind(now)
        .bind(vid)
        .bind(org_id)
        .execute(&state.pool)
        .await?;

        sqlx::query(
            "INSERT INTO audit_events (id, organization_id, verification_id, event_type, actor, payload, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(vid)
        .bind(format!("review.{outcome}"))
        .bind(format!("dashboard:{}", ctx.email))
        .bind(json!({ "review_case": review_id.to_string(), "reason": reason }))
        .bind(now)
        .execute(&state.pool)
        .await?;
    }

    Ok(())
}

pub async fn approve_review_case(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<crate::domain::ReviewDecisionBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    resolve_review_case(&state, &ctx, id, "approved", body.reason).await?;
    Ok(Json(json!({ "status": "approved" })))
}

pub async fn reject_review_case(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<crate::domain::ReviewDecisionBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    resolve_review_case(&state, &ctx, id, "rejected", body.reason).await?;
    Ok(Json(json!({ "status": "rejected" })))
}

pub async fn add_review_note(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<crate::domain::CreateReviewNote>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Scope check before writing the note.
    let exists: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM review_cases WHERE id = $1 AND organization_id = $2")
            .bind(id)
            .bind(ctx.organization_id)
            .fetch_optional(&state.pool)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    sqlx::query(
        "INSERT INTO review_notes (id, review_case_id, organization_id, author, note, created_at) VALUES ($1,$2,$3,$4,$5,now())",
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind(ctx.organization_id)
    .bind(&ctx.email)
    .bind(&body.note)
    .execute(&state.pool)
    .await?;

    Ok(Json(json!({ "note_added": true })))
}

// --- Webhook deliveries (dashboard, org-scoped) ---

pub async fn list_webhook_deliveries(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let rows: Vec<(Uuid, Option<Uuid>, String, String, i32, Option<i32>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, verification_id, event_type, status, attempts, response_code, created_at
             FROM webhook_deliveries WHERE organization_id = $1 ORDER BY created_at DESC LIMIT 100",
        )
        .bind(ctx.organization_id)
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, vid, event_type, status, attempts, code, created)| {
                json!({
                    "id": id,
                    "verification_id": vid,
                    "event_type": event_type,
                    "status": status,
                    "attempts": attempts,
                    "response_code": code,
                    "created_at": created,
                })
            })
            .collect(),
    ))
}

pub async fn resend_webhook_delivery(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let updated = sqlx::query(
        "UPDATE webhook_deliveries SET status='pending', next_attempt_at = now() WHERE id = $1 AND organization_id = $2",
    )
    .bind(id)
    .bind(ctx.organization_id)
    .execute(&state.pool)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "resend_queued": true })))
}

// --- API request logs (dashboard, org-scoped) ---

pub async fn list_api_requests(
    State(state): State<AppState>,
    Extension(ctx): Extension<DashboardContext>,
) -> Result<Json<Vec<crate::domain::ApiRequestLog>>, AppError> {
    let rows: Vec<(Uuid, String, String, Option<i32>, Option<i32>, Option<String>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, method, path, status_code, latency_ms, request_id, created_at
             FROM api_requests WHERE organization_id = $1 ORDER BY created_at DESC LIMIT 100",
        )
        .bind(ctx.organization_id)
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, method, path, status, latency, req_id, created)| crate::domain::ApiRequestLog {
                id,
                method,
                path,
                status_code: status,
                latency_ms: latency,
                request_id: req_id,
                created_at: created,
            })
            .collect(),
    ))
}

// --- Source registry (dashboard) ---

pub async fn list_sources(
    State(state): State<AppState>,
    Extension(_ctx): Extension<DashboardContext>,
) -> Result<Json<Vec<crate::domain::VerificationSourceInfo>>, AppError> {
    let rows: Vec<(String, String, String, String, bool, bool, Option<String>)> = sqlx::query_as(
        "SELECT code, name, category, status, supports_person, supports_business, description
         FROM verification_sources ORDER BY code",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(code, name, category, status, sp, sb, desc)| crate::domain::VerificationSourceInfo {
                code,
                name,
                category,
                status,
                supports_person: sp,
                supports_business: sb,
                description: desc,
            })
            .collect(),
    ))
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
    let rows: Vec<(Uuid, String, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, partner_name, environment, COALESCE(display_prefix, ''), created_at
         FROM api_keys WHERE organization_id = $1 AND revoked_at IS NULL ORDER BY created_at DESC LIMIT 50"
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, partner_name, environment, display_prefix, created_at)| json!({
            "id": id,
            "partner_name": partner_name,
            "environment": environment,
            "display_prefix": display_prefix,
            "created_at": created_at,
        }))
        .collect::<Vec<_>>())))
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