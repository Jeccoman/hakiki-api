//! BRELA sandbox mock adapter for businesses.

use async_trait::async_trait;
use serde_json::json;

use super::VerificationSource;
use crate::domain::{CheckKind, NormalizedCheckResult, Outcome, Subject};

#[derive(Default)]
pub struct BrelaMockSource;

#[async_trait]
impl VerificationSource for BrelaMockSource {
    fn name(&self) -> &'static str {
        "brela"
    }

    async fn verify(&self, subject: &Subject) -> NormalizedCheckResult {
        let ref_id = &subject.reference;
        // Sandbox: registration numbers that start with certain letters or contain numbers
        let looks_like_reg = ref_id.len() >= 6
            && (ref_id.to_uppercase().starts_with("1")
                || ref_id.to_uppercase().contains("TZ")
                || ref_id.chars().any(|c| c.is_ascii_digit()));

        let is_match = looks_like_reg && subject.kind == crate::domain::SubjectKind::Business;
        let outcome = if is_match { Outcome::Match } else { Outcome::NoMatch };

        NormalizedCheckResult {
            source: self.name().to_string(),
            check_kind: CheckKind::Brela,
            outcome,
            confidence: Some(if is_match { 0.79 } else { 0.35 }),
            message: Some(if is_match {
                "Business registration found in sandbox BRELA".into()
            } else {
                "Business not found or invalid registration (sandbox)".into()
            }),
            normalized_fields: Some(json!({ "registration_no": ref_id })),
            raw_response: Some(json!({ "mock": true, "brela_sandbox": true })),
        }
    }
}