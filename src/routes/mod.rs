pub mod api_requests;
pub mod contact;
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

use crate::auth::require_api_key;
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
        .route("/v1/contact", post(contact::submit));

    protected.merge(public).with_state(state)
}
