-- Extend verifications with full lifecycle, decisioning, risk, environment.
-- All additions are additive. Existing rows will get sensible defaults.
-- We preserve the append-only spirit: status changes will be recorded via new audit
-- mechanisms in later migrations, but we still allow controlled updates to the
-- verification header for status/decision (the core record remains the source of truth).

ALTER TABLE verifications
  ADD COLUMN IF NOT EXISTS decision text,                    -- approved | needs_review | rejected | failed
  ADD COLUMN IF NOT EXISTS decision_reason text,
  ADD COLUMN IF NOT EXISTS risk_score integer,
  ADD COLUMN IF NOT EXISTS risk_level text,                  -- low | medium | high
  ADD COLUMN IF NOT EXISTS completed_at timestamptz,
  ADD COLUMN IF NOT EXISTS expires_at timestamptz,
  ADD COLUMN IF NOT EXISTS failure_reason text,
  ADD COLUMN IF NOT EXISTS environment text NOT NULL DEFAULT 'test',  -- test | live
  ADD COLUMN IF NOT EXISTS updated_at timestamptz NOT NULL DEFAULT now();

-- Expand allowed status values (stored as text for audit clarity).
-- New statuses: created, processing, checks_completed, needs_review, approved, rejected, failed, expired
-- We keep old 'completed'/'pending' values working via mapping for now.

COMMENT ON COLUMN verifications.status IS 'created | processing | checks_completed | needs_review | approved | rejected | failed | expired | completed | pending';

-- Backfill existing data (for dev environments)
UPDATE verifications
SET environment = 'test',
    updated_at = now(),
    completed_at = COALESCE(completed_at, created_at)
WHERE environment IS NULL OR completed_at IS NULL;

-- Helpful indexes for new access patterns
CREATE INDEX IF NOT EXISTS idx_verifications_environment ON verifications(environment);
CREATE INDEX IF NOT EXISTS idx_verifications_status ON verifications(status);
CREATE INDEX IF NOT EXISTS idx_verifications_risk ON verifications(risk_level, risk_score);