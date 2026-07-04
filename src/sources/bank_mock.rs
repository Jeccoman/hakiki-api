//! Bank account verification - currently coming_soon / unsupported in sandbox.

use async_trait::async_trait;
use serde_json::json;

use super::VerificationSource;
use crate::domain::{CheckKind, NormalizedCheckResult, Outcome, Subject};

#[derive(Default)]
pub struct BankMockSource;

#[async_trait]
impl VerificationSource for BankMockSource {
    fn name(&self) -> &'static str {
        "bank_account"
    }

    async fn verify(&self, _subject: &Subject) -> NormalizedCheckResult {
        NormalizedCheckResult {
            source: self.name().to_string(),
            check_kind: CheckKind::BankAccount,
            outcome: Outcome::Unsupported,
            confidence: None,
            message: Some("Bank account verification is coming soon".into()),
            normalized_fields: None,
            raw_response: Some(json!({ "status": "coming_soon" })),
        }
    }
}