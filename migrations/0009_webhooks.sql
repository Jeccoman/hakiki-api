-- Webhook foundation (endpoints + deliveries + attempts).
-- Deliveries are append-only records of what we attempted to send.

CREATE TABLE webhook_endpoints (
    id            uuid        PRIMARY KEY,
    api_key_id    uuid        NOT NULL REFERENCES api_keys(id),
    url           text        NOT NULL,
    secret        text        NOT NULL,   -- used for HMAC signature (store hashed? for simplicity we store raw for now - in real we'd rotate)
    events        text[]      NOT NULL DEFAULT '{}',  -- e.g. ['verification.approved', 'verification.needs_review']
    active        boolean     NOT NULL DEFAULT true,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE webhook_deliveries (
    id              uuid        PRIMARY KEY,
    webhook_endpoint_id uuid    NOT NULL REFERENCES webhook_endpoints(id),
    verification_id uuid,
    event_type      text        NOT NULL,   -- verification.created, verification.approved, ...
    payload         jsonb       NOT NULL,
    signature       text,
    status          text        NOT NULL DEFAULT 'pending',  -- pending | delivered | failed
    attempts        integer     NOT NULL DEFAULT 0,
    last_attempt_at timestamptz,
    delivered_at    timestamptz,
    response_code   integer,
    response_body   text,
    created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE webhook_attempts (
    id              uuid        PRIMARY KEY,
    delivery_id     uuid        NOT NULL REFERENCES webhook_deliveries(id),
    attempt_number  integer     NOT NULL,
    status_code     integer,
    response_body   text,
    error           text,
    attempted_at    timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_webhook_endpoints_api_key ON webhook_endpoints(api_key_id);
CREATE INDEX idx_webhook_deliveries_verification ON webhook_deliveries(verification_id);
CREATE INDEX idx_webhook_deliveries_status ON webhook_deliveries(status);