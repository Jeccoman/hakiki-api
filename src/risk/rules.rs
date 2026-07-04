//! Deterministic risk rules and flag codes.
//!
//! Keep rules simple, auditable, and easy to extend.
//! Critical flags can force rejection.

use crate::domain::{CheckKind, NormalizedCheckResult, Outcome, RiskFlag};

pub const FLAG_PHONE_NAME_MISMATCH: &str = "PHONE_NAME_MISMATCH";
pub const FLAG_LOW_CONFIDENCE: &str = "LOW_CONFIDENCE_MATCH";
pub const FLAG_SOURCE_UNSUPPORTED: &str = "SOURCE_UNSUPPORTED";
pub const FLAG_NIDA_UNAVAILABLE: &str = "NIDA_UNAVAILABLE";
pub const FLAG_BRELA_UNAVAILABLE: &str = "BRELA_UNAVAILABLE";
pub const FLAG_TRA_UNAVAILABLE: &str = "TRA_UNAVAILABLE";
pub const FLAG_BUSINESS_NOT_FOUND: &str = "BUSINESS_NOT_FOUND";
pub const FLAG_TIN_MISSING: &str = "TIN_MISSING";
pub const FLAG_DUPLICATE_ID: &str = "DUPLICATE_ID";
pub const FLAG_CONSENT_INVALID: &str = "CONSENT_INVALID";

pub fn evaluate(
    results: &[NormalizedCheckResult],
    subject_name: Option<&str>,
    subject_phone: Option<&str>,
) -> (Vec<RiskFlag>, i32) {
    let mut flags = Vec::new();
    let mut score: i32 = 0;

    let mut has_match = false;
    let mut has_strong_match = false;

    for r in results {
        match r.outcome {
            Outcome::Match => {
                has_match = true;
                if r.confidence.unwrap_or(0.8) >= 0.85 {
                    has_strong_match = true;
                } else {
                    flags.push(RiskFlag {
                        code: FLAG_LOW_CONFIDENCE.to_string(),
                        severity: "medium".to_string(),
                        message: format!("Low confidence from {}", r.source),
                        source: Some(r.source.clone()),
                    });
                    score += 18;
                }
            }
            Outcome::NoMatch => {
                flags.push(RiskFlag {
                    code: format!("{}_NO_MATCH", r.source.to_uppercase()),
                    severity: "high".to_string(),
                    message: format!("No match from {}", r.source),
                    source: Some(r.source.clone()),
                });
                score += 35;
            }
            Outcome::Unsupported | Outcome::Error => {
                let code = match r.check_kind {
                    CheckKind::Nida => FLAG_NIDA_UNAVAILABLE,
                    CheckKind::Brela => FLAG_BRELA_UNAVAILABLE,
                    CheckKind::Tra => FLAG_TRA_UNAVAILABLE,
                    _ => FLAG_SOURCE_UNSUPPORTED,
                };
                flags.push(RiskFlag {
                    code: code.to_string(),
                    severity: if matches!(r.check_kind, CheckKind::Nida | CheckKind::Brela) { "high" } else { "medium" }.to_string(),
                    message: r.message.clone().unwrap_or_else(|| "Source unavailable".to_string()),
                    source: Some(r.source.clone()),
                });
                if matches!(r.check_kind, CheckKind::Nida | CheckKind::Brela) {
                    score += 22;
                } else {
                    score += 8;
                }
            }
            Outcome::Pending => {
                score += 5;
            }
        }

        // Example specific rule: phone + name provided but poor alignment
        if let (Some(name), Some(_phone)) = (subject_name, subject_phone) {
            if name.len() < 3 && r.check_kind == CheckKind::TelcoIdentity {
                flags.push(RiskFlag {
                    code: FLAG_PHONE_NAME_MISMATCH.to_string(),
                    severity: "medium".to_string(),
                    message: "Name appears incomplete for phone identity check".to_string(),
                    source: Some("telco".into()),
                });
                score += 12;
            }
        }
    }

    if !has_match && !results.is_empty() {
        score += 25;
    }
    if has_strong_match {
        score = score.saturating_sub(15);
    }

    // Clamp
    score = score.clamp(0, 100);

    (flags, score)
}