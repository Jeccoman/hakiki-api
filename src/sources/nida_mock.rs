//! NIDA sandbox mock adapter.
//!
//! Returns Match for plausible Tanzanian NIDA / NIN patterns.
//! Never claims to be a real integration.

use async_trait::async_trait;
use serde_json::json;

use super::VerificationSource;
use crate::domain::{CheckKind, NormalizedCheckResult, Outcome, Subject};

#[derive(Default)]
pub struct NidaMockSource;

#[async_trait]
impl VerificationSource for NidaMockSource {
    fn name(&self) -> &'static str {
        "nida"
    }

    async fn verify(&self, subject: &Subject) -> NormalizedCheckResult {
        let ref_id = &subject.reference;

        // Very rough sandbox rule: long numeric strings that look like NINs
        let looks_valid = ref_id.len() >= 14 && ref_id.chars().all(|c| c.is_ascii_digit() || c.is_ascii_uppercase());

        let outcome = if looks_valid { Outcome::Match } else { Outcome::NoMatch };
        let confidence = if looks_valid { Some(0.88) } else { Some(0.4) };

        NormalizedCheckResult {
            source: self.name().to_string(),
            check_kind: CheckKind::Nida,
            outcome,
            confidence,
            message: Some(if looks_valid {
                "NIDA record found (sandbox)".to_string()
            } else {
                "No matching NIDA record (sandbox)".to_string()
            }),
            normalized_fields: Some(json!({
                "national_id": ref_id,
                "full_name_match": subject.full_name.is_some() && looks_valid
            })),
            raw_response: Some(json!({ "mock": true, "source": "nida_sandbox" })),
        }
    }
}