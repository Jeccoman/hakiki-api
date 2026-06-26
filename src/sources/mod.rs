//! Verification sources. Every external system Hakiki brokers (telco, NIDA,
//! BRELA, TRA, banks) implements the same trait, so adding a source later is
//! a new file — not a change to the request pipeline.

pub mod telco;

use async_trait::async_trait;

use crate::domain::{CheckResult, Subject};

#[async_trait]
pub trait VerificationSource: Send + Sync {
    fn name(&self) -> &'static str;
    async fn verify(&self, subject: &Subject) -> CheckResult;
}
