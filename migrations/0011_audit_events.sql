-- Lightweight append-only audit events for major verification actions.
-- Complements the existing verifications + consents + check_results.

CREATE TABLE audit_events (
    id              uuid        PRIMARY KEY,
    verification_id uuid        REFERENCES verifications(id),
    event_type      text        NOT NULL,   -- verification.created, checks.completed, risk.scored, decision.made, review.approved, webhook.queued, etc.
    actor           text,                   -- "api-key:public-demo" or "system:risk-engine" or "reviewer:..."
    payload         jsonb,
    created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_events_verification ON audit_events(verification_id);
CREATE INDEX idx_audit_events_type ON audit_events(event_type);
CREATE INDEX idx_audit_events_created ON audit_events(created_at);