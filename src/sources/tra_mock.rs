//! TRA (Tax) sandbox mock.

use async_trait::async_trait;
use serde_json::json;

use super::VerificationSource;
use crate::domain::{CheckKind, NormalizedCheckResult, Outcome, Subject};

#[derive(Default)]
pub struct TraMockSource;

#[async_trait]
impl VerificationSource for TraMockSource {
    fn name(&self) -> &'static str {
        "tra"
    }

    async fn verify(&self, subject: &Subject) -> NormalizedCheckResult {
        let ref_id = &subject.reference;
        // Simple TIN format check: 9-11 digits or starts with T
        let valid_tin = (ref_id.len() >= 9 && ref_id.len() <= 11 && ref_id.chars().all(|c| c.is_ascii_digit()))
            || ref_id.to_uppercase().starts_with('T');

        let outcome = if valid_tin { Outcome::Match } else { Outcome::NoMatch };

        NormalizedCheckResult {
            source: self.name().to_string(),
            check_kind: CheckKind::Tra,
            outcome,
            confidence: Some(if valid_tin { 0.81 } else { 0.3 }),
            message: Some(if valid_tin {
                "TIN record located (sandbox)".into()
            } else {
                "No TIN match or invalid format (sandbox)".into()
            }),
            normalized_fields: Some(json!({ "tin": ref_id })),
            raw_response: Some(json!({ "mock": true })),
        }
    }
}