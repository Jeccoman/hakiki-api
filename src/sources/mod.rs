//! Verification sources.
//!
//! Every data source (telco, NIDA, BRELA, TRA, banks...) implements this trait.
//! Sandbox/mock adapters live here until real integrations are added.

pub mod bank_mock;
pub mod brela_mock;
pub mod nida_mock;
pub mod telco;
pub mod tra_mock;

use async_trait::async_trait;

use crate::domain::{NormalizedCheckResult, Subject};

#[async_trait]
pub trait VerificationSource: Send + Sync {
    fn name(&self) -> &'static str;
    async fn verify(&self, subject: &Subject) -> NormalizedCheckResult;
}
