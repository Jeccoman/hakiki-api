//! Shared application state.
//!
//! Backed by a Postgres connection pool. Routes hold a clone of `AppState`
//! (cheap — `PgPool` and `Arc` are reference-counted) and never construct a
//! connection themselves.

use std::sync::Arc;

use sqlx::PgPool;

use crate::sources::telco::TelcoSource;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub telco: Arc<TelcoSource>,
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            telco: Arc::new(TelcoSource),
        }
    }
}
