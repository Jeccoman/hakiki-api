//! Telco / Mobile identity source (sandbox).
//!
//! Currently a deterministic mock. Real version would call operator KYC APIs
//! with proper consent proof.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

use super::VerificationSource;
use crate::domain::{CheckKind, NormalizedCheckResult, Outcome, Subject};

#[derive(Default)]
pub struct TelcoSource;

#[async_trait]
impl VerificationSource for TelcoSource {
    fn name(&self) -> &'static str {
        "telco_identity"
    }

    async fn verify(&self, subject: &Subject) -> NormalizedCheckResult {
        let phone = subject
            .phone
            .as_deref()
            .map(|p| p.replace([' ', '+', '-'], ""))
            .unwrap_or_default();

        let matched = (phone.starts_with("255") || phone.starts_with('0')) && phone.len() >= 10;

        let outcome = if matched { Outcome::Match } else { Outcome::NoMatch };
        let confidence = if matched { Some(0.92) } else { Some(0.65) };

        NormalizedCheckResult {
            source: self.name().to_string(),
            check_kind: CheckKind::TelcoIdentity,
            outcome,
            confidence,
            message: Some(if matched {
                "SIM registration matches subject identity (sandbox)".into()
            } else {
                "No registered SIM found for subject (sandbox)".into()
            }),
            normalized_fields: Some(json!({
                "phone": phone,
                "match_reason": if matched { "prefix_and_length" } else { "no_match" }
            })),
            raw_response: Some(json!({
                "mock": true,
                "phone_e164": if phone.starts_with("0") { format!("255{}", &phone[1..]) } else { phone.clone() },
                "ts": Utc::now().to_rfc3339(),
            })),
        }
    }
}