//! Risk scoring engine entry point.

use crate::domain::{NormalizedCheckResult, RiskAssessment, RiskLevel, Decision};
use crate::risk::rules;

/// Runs risk assessment over normalized results.
/// Returns a RiskAssessment ready to be persisted.
pub fn score_verification(
    normalized: &[NormalizedCheckResult],
    subject_full_name: Option<&str>,
    subject_phone: Option<&str>,
) -> RiskAssessment {
    let (flags, score) = rules::evaluate(normalized, subject_full_name, subject_phone);

    // Decision thresholds (as specified)
    let (level, decision) = if score >= 70 {
        (RiskLevel::High, Decision::NeedsReview)
    } else if score >= 31 {
        (RiskLevel::Medium, Decision::NeedsReview)
    } else {
        (RiskLevel::Low, Decision::Approved)
    };

    // Critical flags can force higher scrutiny
    let has_critical = flags.iter().any(|f| f.severity == "critical");
    let final_decision = if has_critical && decision == Decision::Approved {
        Decision::NeedsReview
    } else {
        decision
    };

    // If any hard no-match on primary identity sources, bump to review at minimum
    let primary_no_match = normalized.iter().any(|r| {
        matches!(r.check_kind, crate::domain::CheckKind::Nida | crate::domain::CheckKind::TelcoIdentity)
            && r.outcome == crate::domain::Outcome::NoMatch
    });
    let final_level = if primary_no_match && level == RiskLevel::Low {
        RiskLevel::Medium
    } else {
        level
    };

    RiskAssessment {
        score,
        level: final_level,
        decision: final_decision,
        flags,
    }
}