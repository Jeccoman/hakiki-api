-- Multi-tenant organization model for Hakiki.
-- All major resources will be scoped to an organization.

CREATE TABLE organizations (
    id uuid PRIMARY KEY,
    name text NOT NULL,
    slug text NOT NULL UNIQUE,
    status text NOT NULL DEFAULT 'active',  -- active | suspended | deleted
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE organization_members (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    email text NOT NULL,
    role text NOT NULL,  -- owner | admin | developer | compliance_reviewer | viewer
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (organization_id, email)
);

-- Add organization scoping to api_keys
ALTER TABLE api_keys
  ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);

CREATE INDEX IF NOT EXISTS idx_api_keys_organization ON api_keys(organization_id);

-- Scope major verification and audit tables
ALTER TABLE verifications ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);
ALTER TABLE consents ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);
ALTER TABLE check_results ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);
ALTER TABLE risk_scores ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);
ALTER TABLE risk_flags ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);
ALTER TABLE review_cases ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);
ALTER TABLE review_notes ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);
ALTER TABLE audit_events ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);
ALTER TABLE webhook_endpoints ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);
ALTER TABLE webhook_deliveries ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);
ALTER TABLE api_requests ADD COLUMN IF NOT EXISTS organization_id uuid REFERENCES organizations(id);

-- Indexes for scoping queries (very important for multi-tenant isolation)
CREATE INDEX IF NOT EXISTS idx_verifications_organization ON verifications(organization_id);
CREATE INDEX IF NOT EXISTS idx_api_requests_organization ON api_requests(organization_id);
CREATE INDEX IF NOT EXISTS idx_webhook_endpoints_organization ON webhook_endpoints(organization_id);
CREATE INDEX IF NOT EXISTS idx_audit_events_organization ON audit_events(organization_id);
CREATE INDEX IF NOT EXISTS idx_review_cases_organization ON review_cases(organization_id);

-- Backfill for demo: will be handled in Rust startup code to create demo org and attach existing demo key.