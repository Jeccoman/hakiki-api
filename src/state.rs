//! Shared application state.
//!
//! All verification sources are available here. The registry of configured
//! sources is also loadable from the DB.

use std::sync::Arc;

use sqlx::PgPool;

use crate::sources::bank_mock::BankMockSource;
use crate::sources::brela_mock::BrelaMockSource;
use crate::sources::nida_mock::NidaMockSource;
use crate::sources::telco::TelcoSource;
use crate::sources::tra_mock::TraMockSource;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub telco: Arc<TelcoSource>,
    pub nida: Arc<NidaMockSource>,
    pub brela: Arc<BrelaMockSource>,
    pub tra: Arc<TraMockSource>,
    pub bank: Arc<BankMockSource>,
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            telco: Arc::new(TelcoSource),
            nida: Arc::new(NidaMockSource),
            brela: Arc::new(BrelaMockSource),
            tra: Arc::new(TraMockSource),
            bank: Arc::new(BankMockSource),
        }
    }
}
