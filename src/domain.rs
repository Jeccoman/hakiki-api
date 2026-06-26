//! Core domain types shared across routes and verification sources.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Person,
    Business,
}

/// The thing being verified. `reference` is the source-relevant identifier:
/// a NIDA number for a person, a BRELA registration number for a business, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub kind: SubjectKind,
    pub reference: String,
    pub full_name: Option<String>,
    pub phone: Option<String>,
}

/// Consent is mandatory on every request — this is the "consent-first" promise
/// the product is built on, not an afterthought.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Consent {
    /// How consent was captured, e.g. "ussd_otp" or "signed_mandate".
    pub method: String,
    pub granted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    TelcoIdentity,
    Nida,
    Brela,
    Tra,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Match,
    NoMatch,
    Pending,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub check: CheckKind,
    pub outcome: Outcome,
    pub detail: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateVerification {
    pub subject: Subject,
    pub checks: Vec<CheckKind>,
    pub consent: Consent,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Completed,
    Pending,
}

#[derive(Debug, Clone, Serialize)]
pub struct Verification {
    pub id: Uuid,
    pub status: VerificationStatus,
    pub subject: Subject,
    pub results: Vec<CheckResult>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JoinWaitlist {
    pub company: String,
    pub email: String,
    pub sector: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaitlistEntry {
    pub id: Uuid,
    pub company: String,
    pub email: String,
    pub sector: Option<String>,
    pub created_at: DateTime<Utc>,
}

// --- enum <-> database text mappings -------------------------------------
// Enums are stored as text columns. Keeping the conversions explicit (rather
// than relying on a derive) makes the on-disk audit values obvious and stable.

impl SubjectKind {
    pub fn as_db(&self) -> &'static str {
        match self {
            SubjectKind::Person => "person",
            SubjectKind::Business => "business",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "business" => SubjectKind::Business,
            _ => SubjectKind::Person,
        }
    }
}

impl CheckKind {
    pub fn as_db(&self) -> &'static str {
        match self {
            CheckKind::TelcoIdentity => "telco_identity",
            CheckKind::Nida => "nida",
            CheckKind::Brela => "brela",
            CheckKind::Tra => "tra",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "nida" => CheckKind::Nida,
            "brela" => CheckKind::Brela,
            "tra" => CheckKind::Tra,
            _ => CheckKind::TelcoIdentity,
        }
    }
}

impl Outcome {
    pub fn as_db(&self) -> &'static str {
        match self {
            Outcome::Match => "match",
            Outcome::NoMatch => "no_match",
            Outcome::Pending => "pending",
            Outcome::Unsupported => "unsupported",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "no_match" => Outcome::NoMatch,
            "pending" => Outcome::Pending,
            "unsupported" => Outcome::Unsupported,
            _ => Outcome::Match,
        }
    }
}

impl VerificationStatus {
    pub fn as_db(&self) -> &'static str {
        match self {
            VerificationStatus::Completed => "completed",
            VerificationStatus::Pending => "pending",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "pending" => VerificationStatus::Pending,
            _ => VerificationStatus::Completed,
        }
    }
}
