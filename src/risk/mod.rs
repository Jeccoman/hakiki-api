//! Risk scoring engine.
//!
//! Takes normalized check results and produces a deterministic risk assessment
//! with score, level, decision and structured flags.
//!
//! This is intentionally rule-based and transparent for auditability.

pub mod engine;
pub mod model;
pub mod rules;

pub use engine::score_verification;