//! Shared application state.
//!
//! Storage is in-memory for now (HashMap/Vec behind locks) so the API runs with
//! zero infra. Swapping these for a Postgres-backed repository later is a
//! contained change — the routes only touch `AppState`, not the store directly.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use uuid::Uuid;

use crate::domain::{Verification, WaitlistEntry};
use crate::sources::telco::TelcoSource;

#[derive(Clone)]
pub struct AppState {
    pub verifications: Arc<RwLock<HashMap<Uuid, Verification>>>,
    pub waitlist: Arc<RwLock<Vec<WaitlistEntry>>>,
    pub telco: Arc<TelcoSource>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            verifications: Arc::new(RwLock::new(HashMap::new())),
            waitlist: Arc::new(RwLock::new(Vec::new())),
            telco: Arc::new(TelcoSource),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
