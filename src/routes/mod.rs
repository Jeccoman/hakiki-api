pub mod health;
pub mod verify;
pub mod waitlist;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/verifications", post(verify::create))
        .route("/v1/verifications/{id}", get(verify::get_one))
        .route("/v1/waitlist", post(waitlist::join))
        .with_state(state)
}
