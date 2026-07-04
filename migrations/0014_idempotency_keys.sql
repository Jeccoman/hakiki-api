-- Idempotency key support for safe retries on POST operations (mainly verifications).

CREATE TABLE idempotency_keys (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations(id),
    key text NOT NULL,
    request_hash text NOT NULL,   -- hash of the request body for safety
    response_body jsonb,
    status_code integer,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL DEFAULT (now() + interval '24 hours'),
    UNIQUE (organization_id, key)
);

CREATE INDEX idx_idempotency_org_key ON idempotency_keys(organization_id, key);
CREATE INDEX idx_idempotency_expires ON idempotency_keys(expires_at);