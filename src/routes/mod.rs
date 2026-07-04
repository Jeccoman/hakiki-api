pub mod api_requests;
pub mod contact;
pub mod dashboard;
pub mod health;
pub mod reviews;
pub mod sources;
pub mod verify;
pub mod waitlist;
pub mod webhooks;

use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};

use crate::auth::{require_api_key, require_dashboard_auth};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    // Protected routes (require valid X-API-Key)
    let protected = Router::new()
        // Core verification
        .route("/v1/verifications", post(verify::create))
        .route("/v1/verifications/{id}", get(verify::get_one))
        // Sources
        .route("/v1/sources", get(sources::list))
        // API request logs
        .route("/v1/api-requests", get(api_requests::list))
        // Webhooks management
        .route("/v1/webhook-endpoints", get(webhooks::list).post(webhooks::create))
        .route("/v1/webhook-endpoints/{id}", delete(webhooks::delete))
        .route("/v1/webhook-endpoints/{id}/test", post(webhooks::test))
        .route("/v1/webhook-deliveries", get(webhooks::deliveries))
        .route("/v1/webhook-deliveries/{id}/resend", post(webhooks::resend))
        // Reviews
        .route("/v1/review-cases", get(reviews::list))
        .route("/v1/review-cases/{id}", get(reviews::get_one))
        .route("/v1/review-cases/{id}/approve", post(reviews::approve))
        .route("/v1/review-cases/{id}/reject", post(reviews::reject))
        .route("/v1/review-cases/{id}/notes", post(reviews::add_note))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_key,
        ));

    // Public endpoints
    let public = Router::new()
        .route("/health", get(health::health))
        .route("/v1/waitlist", post(waitlist::join))
        .route("/v1/contact", post(contact::submit))
        .route("/dashboard/auth/login", post(dashboard::login));

    // Dashboard routes protected by dashboard auth
    let protected_dashboard = Router::new()
        .route("/dashboard/me", get(dashboard::me))
        .route("/dashboard/auth/logout", post(dashboard::logout))
        .route("/dashboard/overview", get(dashboard::overview))
        .route("/dashboard/verifications", get(dashboard::list_verifications).post(dashboard::create_verification))
        .route("/dashboard/verifications/{id}", get(dashboard::get_verification_detail))
        .route("/dashboard/api-keys", get(dashboard::list_api_keys).post(dashboard::create_api_key))
        .route("/dashboard/review-cases", get(dashboard::list_review_cases))
        .route("/dashboard/review-cases/{id}", get(dashboard::get_review_case))
        .route("/dashboard/review-cases/{id}/approve", post(dashboard::approve_review_case))
        .route("/dashboard/review-cases/{id}/reject", post(dashboard::reject_review_case))
        .route("/dashboard/review-cases/{id}/notes", post(dashboard::add_review_note))
        .route("/dashboard/webhook-deliveries", get(dashboard::list_webhook_deliveries))
        .route("/dashboard/webhook-deliveries/{id}/resend", post(dashboard::resend_webhook_delivery))
        .route("/dashboard/api-requests", get(dashboard::list_api_requests))
        .route("/dashboard/sources", get(dashboard::list_sources))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_dashboard_auth,
        ));

    protected.merge(public).merge(protected_dashboard).with_state(state)
}
