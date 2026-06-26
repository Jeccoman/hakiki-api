//! Telco connector — the first integration ("step 1: the telco route").
//!
//! In production this brokers a consented lookup against a mobile operator's
//! SIM-registration / mobile-money KYC records. Right now it's a deterministic
//! mock so the API, the frontend demo, and partner conversations have something
//! real to call before any operator contract is signed.

use async_trait::async_trait;

use super::VerificationSource;
use crate::domain::{CheckKind, CheckResult, Outcome, Subject};

#[derive(Default)]
pub struct TelcoSource;

#[async_trait]
impl VerificationSource for TelcoSource {
    fn name(&self) -> &'static str {
        "telco"
    }

    async fn verify(&self, subject: &Subject) -> CheckResult {
        // Mock rule: treat anything shaped like a Tanzanian MSISDN as a match.
        let matched = subject
            .phone
            .as_deref()
            .map(|p| p.replace([' ', '+', '-'], ""))
            .map(|p| (p.starts_with("255") || p.starts_with('0')) && p.len() >= 10)
            .unwrap_or(false);

        if matched {
            CheckResult {
                check: CheckKind::TelcoIdentity,
                outcome: Outcome::Match,
                detail: "SIM registration matches subject identity".into(),
            }
        } else {
            CheckResult {
                check: CheckKind::TelcoIdentity,
                outcome: Outcome::NoMatch,
                detail: "No registered SIM found for subject".into(),
            }
        }
    }
}
