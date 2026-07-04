//! API-key authentication for partner-facing endpoints.
//!
//! Keys arrive in the `X-API-Key` header.
//! We now resolve full AuthContext (org_id, environment, etc.) and attach it
//! to request extensions so downstream handlers and routes can use it.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::{AuthContext, DashboardContext, Environment, MemberRole};
use crate::error::AppError;
use crate::state::AppState;

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

pub async fn require_api_key(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let presented = req
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let hash = sha256_hex(presented);

    // Fetch key with organization context
    let row: Option<(Uuid, String, String, String)> = sqlx::query_as(
        r#"
        SELECT k.id, k.partner_name, k.environment, COALESCE(k.organization_id::text, '00000000-0000-0000-0000-000000000001')
        FROM api_keys k
        WHERE k.key_hash = $1 AND k.revoked_at IS NULL
        "#,
    )
    .bind(&hash)
    .fetch_optional(&state.pool)
    .await?;

    let (key_id, partner_name, env_str, org_id_str) = row.ok_or(AppError::Unauthorized)?;

    let org_id = Uuid::parse_str(&org_id_str).unwrap_or_else(|_| Uuid::nil());
    let environment = Environment::from_db(&env_str);

    // Best-effort last used
    let _ = sqlx::query("update api_keys set last_used_at = now() where key_hash = $1")
        .bind(&hash)
        .execute(&state.pool)
        .await;

    // Attach context to request extensions (Axum pattern)
    let ctx = AuthContext {
        api_key_id: key_id,
        organization_id: org_id,
        partner_name,
        environment,
    };
    req.extensions_mut().insert(ctx.clone());

    let mut res = next.run(req).await;
    // Expose the context to outer layers (request logging runs outside this
    // route_layer and only sees the response).
    res.extensions_mut().insert(ctx);
    Ok(res)
}

/// Dashboard auth using session token from header "X-Dashboard-Token".
/// Looks up in dashboard_sessions table, attaches DashboardContext.
pub async fn require_dashboard_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get("x-dashboard-token")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let row: Option<(Uuid, Uuid, String, String, String)> = sqlx::query_as(
        r#"
        SELECT ds.member_id, ds.organization_id, om.email, om.role, o.slug
        FROM dashboard_sessions ds
        JOIN organization_members om ON ds.member_id = om.id
        JOIN organizations o ON ds.organization_id = o.id
        WHERE ds.token = $1 AND ds.expires_at > now()
        "#,
    )
    .bind(token)
    .fetch_optional(&state.pool)
    .await?;

    let (member_id, org_id, email, role_str, _slug) = row.ok_or(AppError::Unauthorized)?;

    let role = MemberRole::from_db(&role_str);

    let ctx = DashboardContext {
        member_id,
        organization_id: org_id,
        email,
        role,
    };
    req.extensions_mut().insert(ctx);

    Ok(next.run(req).await)
}
