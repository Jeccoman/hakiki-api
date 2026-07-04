-- Manual review foundation.
-- When risk engine or rules decide needs_review, we create a review_case.
-- Approvals/rejections here drive the final verification decision (append audit).

CREATE TABLE review_cases (
    id              uuid        PRIMARY KEY,
    verification_id uuid        NOT NULL UNIQUE REFERENCES verifications(id),
    status          text        NOT NULL DEFAULT 'open',  -- open | approved | rejected
    priority        text        NOT NULL DEFAULT 'medium', -- low | medium | high
    assigned_to     text,
    created_at      timestamptz NOT NULL DEFAULT now(),
    resolved_at     timestamptz,
    resolution      text
);

CREATE TABLE review_notes (
    id              uuid        PRIMARY KEY,
    review_case_id  uuid        NOT NULL REFERENCES review_cases(id),
    author          text        NOT NULL,   -- for now "api-key:<name>" or "system"
    note            text        NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_review_cases_verification ON review_cases(verification_id);
CREATE INDEX idx_review_cases_status ON review_cases(status);
CREATE INDEX idx_review_notes_case ON review_notes(review_case_id);