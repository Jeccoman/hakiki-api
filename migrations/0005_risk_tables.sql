-- Risk scoring and flags (append-only audit friendly).
-- A verification can have one risk_score row (latest) and many risk_flags.

CREATE TABLE risk_scores (
    id              uuid        PRIMARY KEY,
    verification_id uuid        NOT NULL REFERENCES verifications(id),
    score           integer     NOT NULL CHECK (score >= 0 AND score <= 100),
    level           text        NOT NULL,   -- low | medium | high
    decision        text        NOT NULL,   -- approved | needs_review | rejected
    created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE risk_flags (
    id              uuid        PRIMARY KEY,
    verification_id uuid        NOT NULL REFERENCES verifications(id),
    code            text        NOT NULL,   -- e.g. PHONE_NAME_MISMATCH, NIDA_UNAVAILABLE
    severity        text        NOT NULL,   -- low | medium | high | critical
    message         text        NOT NULL,
    source          text,                   -- which source contributed the flag
    created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_risk_scores_verification ON risk_scores(verification_id);
CREATE INDEX idx_risk_flags_verification  ON risk_flags(verification_id);
CREATE INDEX idx_risk_flags_code          ON risk_flags(code);