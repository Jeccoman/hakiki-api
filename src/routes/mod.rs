pub mod contact;
pub mod health;
pub mod verify;
pub mod waitlist;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::auth::require_api_key;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    // Verification endpoints require a partner API key.
    let protected = Router::new()
        .route("/v1/verifications", post(verify::create))
        .route("/v1/verifications/{id}", get(verify::get_one))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_key,
        ));

    // Health and the marketing-site waitlist stay public.
    let public = Router::new()
        .route("/health", get(health::health))
        .route("/v1/waitlist", post(waitlist::join))
        .route("/v1/contact", post(contact::submit));

    protected.merge(public).with_state(state)
}
