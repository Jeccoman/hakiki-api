//! Database bootstrap: build the connection pool and run migrations on start.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::sha256_hex;
use crate::domain::{
    CheckKind, Consent, CreateVerification, Decision, Environment, NormalizedCheckResult, 
    RiskAssessment, RiskLevel, Subject, VerificationStatus,
};
use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde_json::json;

pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    // Migrations are embedded from ./migrations at compile time, so the binary
    // is self-contained — no separate migration step to run on deploy.
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

/// Ensures a demo organization exists for the marketing site demo.
pub async fn ensure_demo_organization(pool: &PgPool) -> Result<Uuid, sqlx::Error> {
    let org_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

    sqlx::query(
        r#"
        INSERT INTO organizations (id, name, slug, status)
        VALUES ($1, 'Hakiki Demo', 'demo', 'active')
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(org_id)
    .execute(pool)
    .await?;

    // Ensure a demo member
    sqlx::query(
        r#"
        INSERT INTO organization_members (id, organization_id, email, role)
        VALUES ($1, $2, 'demo@hakiki.test', 'admin')
        ON CONFLICT (organization_id, email) DO NOTHING
        "#,
    )
    .bind(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap())
    .bind(org_id)
    .execute(pool)
    .await?;

    Ok(org_id)
}

/// Seed polished demo data for the demo org if no verifications exist yet.
pub async fn seed_demo_data_if_needed(pool: &PgPool) -> Result<(), sqlx::Error> {
    let org_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM verifications WHERE organization_id = $1")
        .bind(org_id)
        .fetch_one(pool)
        .await?;

    if count.0 > 0 {
        return Ok(());
    }

    let now = chrono::Utc::now();

    // Demo 1: Approved person
    let v1 = Uuid::new_v4();
    sqlx::query(r#"
        INSERT INTO verifications (id, organization_id, subject_kind, subject_reference, subject_full_name, subject_phone, status, decision, risk_score, risk_level, created_at, completed_at, environment, updated_at)
        VALUES ($1, $2, 'person', '19900101123456789', 'Amina Juma', '+255712345678', 'approved', 'approved', 12, 'low', $3, $3, 'test', $3)
    "#)
    .bind(v1).bind(org_id).bind(now).execute(pool).await?;

    sqlx::query("INSERT INTO consents (id, organization_id, verification_id, method, granted_at) VALUES ($1, $2, $3, 'ussd_otp', $4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v1).bind(now).execute(pool).await?;

    sqlx::query(r#"
        INSERT INTO check_results (id, organization_id, verification_id, check_kind, outcome, detail, confidence, created_at)
        VALUES ($1, $2, $3, 'telco_identity', 'match', 'SIM registration matches', 0.95, $4),
               ($5, $2, $3, 'nida', 'match', 'NIDA record found', 0.92, $4)
    "#)
    .bind(Uuid::new_v4()).bind(org_id).bind(v1).bind(now)
    .bind(Uuid::new_v4())
    .execute(pool).await?;

    sqlx::query("INSERT INTO risk_scores (id, organization_id, verification_id, score, level, decision, created_at) VALUES ($1,$2,$3,12,'low','approved',$4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v1).bind(now).execute(pool).await?;

    sqlx::query("INSERT INTO audit_events (id, organization_id, verification_id, event_type, actor, payload, created_at) VALUES ($1,$2,$3,'verification.created','system:seed', $4, $5)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v1).bind(json!({"demo":true})).bind(now).execute(pool).await?;

    // Demo 2: Needs review
    let v2 = Uuid::new_v4();
    sqlx::query(r#"
        INSERT INTO verifications (id, organization_id, subject_kind, subject_reference, subject_full_name, subject_phone, status, decision, risk_score, risk_level, created_at, environment, updated_at)
        VALUES ($1, $2, 'person', '19851234567890123', 'Jacob Mwatonoka', '+255754321098', 'needs_review', 'needs_review', 55, 'medium', $3, 'test', $3)
    "#)
    .bind(v2).bind(org_id).bind(now).execute(pool).await?;

    sqlx::query("INSERT INTO consents (id, organization_id, verification_id, method, granted_at) VALUES ($1, $2, $3, 'web_form', $4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v2).bind(now).execute(pool).await?;

    sqlx::query(r#"
        INSERT INTO check_results (id, organization_id, verification_id, check_kind, outcome, detail, confidence, created_at)
        VALUES ($1, $2, $3, 'telco_identity', 'match', 'SIM ok', 0.88, $4),
               ($5, $2, $3, 'nida', 'no_match', 'Partial match', 0.65, $4)
    "#)
    .bind(Uuid::new_v4()).bind(org_id).bind(v2).bind(now)
    .bind(Uuid::new_v4())
    .execute(pool).await?;

    sqlx::query("INSERT INTO risk_scores (id, organization_id, verification_id, score, level, decision, created_at) VALUES ($1,$2,$3,55,'medium','needs_review',$4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v2).bind(now).execute(pool).await?;

    sqlx::query("INSERT INTO review_cases (id, organization_id, verification_id, status, priority, created_at) VALUES ($1,$2,$3,'open','medium',$4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v2).bind(now).execute(pool).await?;

    sqlx::query("INSERT INTO audit_events (id, organization_id, verification_id, event_type, actor, created_at) VALUES ($1,$2,$3,'verification.created','system:seed',$4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v2).bind(now).execute(pool).await?;

    // Demo 3: Rejected high risk
    let v3 = Uuid::new_v4();
    sqlx::query(r#"
        INSERT INTO verifications (id, organization_id, subject_kind, subject_reference, subject_full_name, subject_phone, status, decision, risk_score, risk_level, created_at, completed_at, environment, updated_at)
        VALUES ($1, $2, 'person', '19780555123456789', 'Musa Ally', '+255789012345', 'rejected', 'rejected', 82, 'high', $3, $3, 'test', $3)
    "#)
    .bind(v3).bind(org_id).bind(now).execute(pool).await?;

    sqlx::query("INSERT INTO consents (id, organization_id, verification_id, method, granted_at) VALUES ($1, $2, $3, 'ussd_otp', $4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v3).bind(now).execute(pool).await?;

    sqlx::query(r#"
        INSERT INTO check_results (id, organization_id, verification_id, check_kind, outcome, detail, confidence, created_at)
        VALUES ($1, $2, $3, 'telco_identity', 'no_match', 'No SIM match', 0.4, $4)
    "#)
    .bind(Uuid::new_v4()).bind(org_id).bind(v3).bind(now)
    .execute(pool).await?;

    sqlx::query("INSERT INTO risk_scores (id, organization_id, verification_id, score, level, decision, created_at) VALUES ($1,$2,$3,82,'high','rejected',$4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v3).bind(now).execute(pool).await?;

    sqlx::query("INSERT INTO audit_events (id, organization_id, verification_id, event_type, actor, created_at) VALUES ($1,$2,$3,'verification.created','system:seed',$4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v3).bind(now).execute(pool).await?;

    // Demo 4: Business KYB
    let v4 = Uuid::new_v4();
    sqlx::query(r#"
        INSERT INTO verifications (id, organization_id, subject_kind, subject_reference, subject_full_name, status, decision, risk_score, risk_level, created_at, completed_at, environment, updated_at)
        VALUES ($1, $2, 'business', 'TZ-123456789', 'Zanzibar Foods Ltd', 'approved', 'approved', 18, 'low', $3, $3, 'test', $3)
    "#)
    .bind(v4).bind(org_id).bind(now).execute(pool).await?;

    sqlx::query("INSERT INTO consents (id, organization_id, verification_id, method, granted_at) VALUES ($1, $2, $3, 'signed_mandate', $4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v4).bind(now).execute(pool).await?;

    sqlx::query(r#"
        INSERT INTO check_results (id, organization_id, verification_id, check_kind, outcome, detail, confidence, created_at)
        VALUES ($1, $2, $3, 'brela', 'match', 'Business registered', 0.90, $4),
               ($5, $2, $3, 'tra', 'match', 'TIN active', 0.85, $4)
    "#)
    .bind(Uuid::new_v4()).bind(org_id).bind(v4).bind(now)
    .bind(Uuid::new_v4())
    .execute(pool).await?;

    sqlx::query("INSERT INTO risk_scores (id, organization_id, verification_id, score, level, decision, created_at) VALUES ($1,$2,$3,18,'low','approved',$4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v4).bind(now).execute(pool).await?;

    sqlx::query("INSERT INTO audit_events (id, organization_id, verification_id, event_type, actor, created_at) VALUES ($1,$2,$3,'verification.created','system:seed',$4)")
        .bind(Uuid::new_v4()).bind(org_id).bind(v4).bind(now).execute(pool).await?;

    tracing::info!("Seeded polished demo verification data for org {}", org_id);
    Ok(())
}

/// Idempotently register an API key (by name) and attach to demo org.
/// Used to seed the publishable demo key...
pub async fn upsert_api_key(pool: &PgPool, name: &str, raw_key: &str) -> Result<(), sqlx::Error> {
    let demo_org_id = ensure_demo_organization(pool).await?;

    sqlx::query(
        r#"
        INSERT INTO api_keys (id, organization_id, partner_name, key_hash, environment)
        VALUES ($1, $2, $3, $4, 'test')
        ON CONFLICT (key_hash) DO UPDATE SET organization_id = EXCLUDED.organization_id
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(demo_org_id)
    .bind(name)
    .bind(sha256_hex(raw_key))
    .execute(pool)
    .await?;
    Ok(())
}

/// Persists a fully processed verification (after sources and risk scoring).
/// Shared between API and dashboard paths.
pub async fn persist_created_verification(
    pool: &PgPool,
    verification_id: Uuid,
    org_id: Uuid,
    subject: &Subject,
    status: VerificationStatus,
    decision: Option<Decision>,
    risk_score: i32,
    risk_level: RiskLevel,
    now: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    env: Environment,
    normalized_results: &[NormalizedCheckResult],
    assessment: &RiskAssessment,
    req_checks: &[CheckKind],
    actor: &str,
    consent: &Consent,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    // Insert verification header
    sqlx::query(
        r#"
        INSERT INTO verifications
          (id, organization_id, subject_kind, subject_reference, subject_full_name, subject_phone,
           status, decision, decision_reason, risk_score, risk_level,
           created_at, completed_at, environment, updated_at)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
        "#,
    )
    .bind(verification_id)
    .bind(org_id)
    .bind(subject.kind.as_db())
    .bind(&subject.reference)
    .bind(&subject.full_name)
    .bind(&subject.phone)
    .bind(status.as_db())
    .bind(decision.as_ref().map(|d| d.as_db()))
    .bind(None::<String>)
    .bind(risk_score)
    .bind(risk_level.as_db())
    .bind(now)
    .bind(completed_at)
    .bind(env.as_db())
    .bind(now)
    .execute(&mut *tx)
    .await?;

    // Consent
    sqlx::query(
        "INSERT INTO consents (id, organization_id, verification_id, method, granted_at) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(Uuid::new_v4())
    .bind(org_id)
    .bind(verification_id)
    .bind(&consent.method)
    .bind(consent.granted_at)
    .execute(&mut *tx)
    .await?;

    // Check results
    for norm in normalized_results {
        sqlx::query(
            r#"
            INSERT INTO check_results
              (id, organization_id, verification_id, check_kind, outcome, detail, confidence, message,
               normalized_fields, raw_response, created_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(verification_id)
        .bind(norm.check_kind.as_db())
        .bind(norm.outcome.as_db())
        .bind(norm.message.clone().unwrap_or_default())
        .bind(norm.confidence)
        .bind(&norm.message)
        .bind(&norm.normalized_fields)
        .bind(&norm.raw_response)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    // Risk score
    sqlx::query(
        "INSERT INTO risk_scores (id, organization_id, verification_id, score, level, decision, created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(Uuid::new_v4())
    .bind(org_id)
    .bind(verification_id)
    .bind(risk_score)
    .bind(risk_level.as_db())
    .bind(assessment.decision.as_db())
    .bind(now)
    .execute(&mut *tx)
    .await?;

    for flag in &assessment.flags {
        sqlx::query(
            "INSERT INTO risk_flags (id, organization_id, verification_id, code, severity, message, source, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(verification_id)
        .bind(&flag.code)
        .bind(&flag.severity)
        .bind(&flag.message)
        .bind(&flag.source)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    // Audit created
    sqlx::query(
        "INSERT INTO audit_events (id, organization_id, verification_id, event_type, actor, payload, created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(Uuid::new_v4())
    .bind(org_id)
    .bind(verification_id)
    .bind("verification.created")
    .bind(actor)
    .bind(serde_json::json!({
        "checks": req_checks.iter().map(|c| c.as_db()).collect::<Vec<_>>(),
        "risk_score": risk_score,
        "decision": assessment.decision.as_db(),
    }))
    .bind(now)
    .execute(&mut *tx)
    .await?;

    // Review case if needed
    if status == VerificationStatus::NeedsReview {
        sqlx::query(
            "INSERT INTO review_cases (id, organization_id, verification_id, status, priority, created_at)
             VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(verification_id)
        .bind("open")
        .bind("medium")
        .bind(now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO audit_events (id, organization_id, verification_id, event_type, actor, payload, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(verification_id)
        .bind("review.case_created")
        .bind(actor)
        .bind(serde_json::json!({ "reason": "risk_score_or_flags" }))
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
