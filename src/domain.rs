//! Core domain types shared across routes and verification sources.
//!
//! Design principles:
//! - Explicit DB string mappings for stable, auditable text values in Postgres.
//! - Additive evolution: old clients continue to work; new fields are optional in JSON.
//! - Consent-first and append-only philosophy preserved.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Person,
    Business,
}

/// The thing being verified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub kind: SubjectKind,
    pub reference: String,
    pub full_name: Option<String>,
    pub phone: Option<String>,
}

/// Explicit consent captured for every verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Consent {
    pub method: String,
    pub granted_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    TelcoIdentity,
    Nida,
    Brela,
    Tra,
    BankAccount,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Match,
    NoMatch,
    Pending,
    Unsupported,
    Error,
}

/// Simple result kept for backward compatibility (demo UI + old clients).
#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub check: CheckKind,
    pub outcome: Outcome,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Normalized + raw result for internal use and richer storage.
#[derive(Debug, Clone, Serialize)]
pub struct NormalizedCheckResult {
    pub source: String,
    pub check_kind: CheckKind,
    pub outcome: Outcome,
    pub confidence: Option<f32>,
    pub message: Option<String>,
    pub normalized_fields: Option<serde_json::Value>,
    pub raw_response: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateVerification {
    pub subject: Subject,
    pub checks: Vec<CheckKind>,
    pub consent: Consent,
}

// ---------------------------------------------------------------------------
// Verification lifecycle & decisioning
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Created,
    Processing,
    ChecksCompleted,
    NeedsReview,
    Approved,
    Rejected,
    Failed,
    Expired,
    // legacy fallbacks
    Completed,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approved,
    NeedsReview,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Test,
    Live,
}

impl Environment {
    pub fn as_db(&self) -> &'static str {
        match self { Environment::Test => "test", Environment::Live => "live" }
    }
    pub fn from_db(s: &str) -> Self {
        match s { "live" => Environment::Live, _ => Environment::Test }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Verification {
    pub id: Uuid,
    pub status: VerificationStatus,
    pub subject: Subject,
    pub results: Vec<CheckResult>,
    pub created_at: DateTime<Utc>,

    // Extended fields (new engine)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<Decision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_score: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<RiskLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<Environment>,
}

// ---------------------------------------------------------------------------
// Risk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct RiskAssessment {
    pub score: i32,
    pub level: RiskLevel,
    pub decision: Decision,
    pub flags: Vec<RiskFlag>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RiskFlag {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub source: Option<String>,
}

// ---------------------------------------------------------------------------
// Source registry view
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct VerificationSourceInfo {
    pub code: String,
    pub name: String,
    pub category: String,
    pub status: String,
    pub supports_person: bool,
    pub supports_business: bool,
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Webhooks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWebhookEndpoint {
    pub url: String,
    pub secret: String,
    #[serde(default)]
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebhookEndpoint {
    pub id: Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Reviews
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ReviewCase {
    pub id: Uuid,
    pub verification_id: Uuid,
    pub status: String,
    pub priority: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReviewDecisionBody {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateReviewNote {
    pub note: String,
}

// ---------------------------------------------------------------------------
// API logs (response model)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ApiRequestLog {
    pub id: Uuid,
    pub method: String,
    pub path: String,
    pub status_code: Option<i32>,
    pub latency_ms: Option<i32>,
    pub request_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Waitlist & Contact (unchanged for compatibility)
// ---------------------------------------------------------------------------

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

#[derive(Debug, Clone, Deserialize)]
pub struct CreateContact {
    pub name: String,
    pub email: String,
    pub organisation: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContactMessage {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub organisation: Option<String>,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// DB string mappings (explicit and auditable)
// ---------------------------------------------------------------------------

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
            CheckKind::BankAccount => "bank_account",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "nida" => CheckKind::Nida,
            "brela" => CheckKind::Brela,
            "tra" => CheckKind::Tra,
            "bank_account" => CheckKind::BankAccount,
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
            Outcome::Error => "error",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "no_match" => Outcome::NoMatch,
            "pending" => Outcome::Pending,
            "unsupported" => Outcome::Unsupported,
            "error" => Outcome::Error,
            _ => Outcome::Match,
        }
    }
}

impl VerificationStatus {
    pub fn as_db(&self) -> &'static str {
        match self {
            VerificationStatus::Created => "created",
            VerificationStatus::Processing => "processing",
            VerificationStatus::ChecksCompleted => "checks_completed",
            VerificationStatus::NeedsReview => "needs_review",
            VerificationStatus::Approved => "approved",
            VerificationStatus::Rejected => "rejected",
            VerificationStatus::Failed => "failed",
            VerificationStatus::Expired => "expired",
            VerificationStatus::Completed => "completed",
            VerificationStatus::Pending => "pending",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "created" => VerificationStatus::Created,
            "processing" => VerificationStatus::Processing,
            "checks_completed" => VerificationStatus::ChecksCompleted,
            "needs_review" => VerificationStatus::NeedsReview,
            "approved" => VerificationStatus::Approved,
            "rejected" => VerificationStatus::Rejected,
            "failed" => VerificationStatus::Failed,
            "expired" => VerificationStatus::Expired,
            "pending" => VerificationStatus::Pending,
            _ => VerificationStatus::Completed,
        }
    }
}

impl Decision {
    pub fn as_db(&self) -> &'static str {
        match self {
            Decision::Approved => "approved",
            Decision::NeedsReview => "needs_review",
            Decision::Rejected => "rejected",
            Decision::Failed => "failed",
        }
    }
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "approved" => Some(Decision::Approved),
            "needs_review" => Some(Decision::NeedsReview),
            "rejected" => Some(Decision::Rejected),
            "failed" => Some(Decision::Failed),
            _ => None,
        }
    }
}

impl RiskLevel {
    pub fn as_db(&self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
        }
    }
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "low" => Some(RiskLevel::Low),
            "medium" => Some(RiskLevel::Medium),
            "high" => Some(RiskLevel::High),
            _ => None,
        }
    }
}