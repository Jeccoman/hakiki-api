-- Source registry: honest status of every connected (or planned) data source.
-- Used by GET /v1/sources and internally for routing/decisions.

CREATE TABLE verification_sources (
    id                 uuid        PRIMARY KEY,
    code               text        NOT NULL UNIQUE,   -- telco_identity, nida, brela, tra, bank_account
    name               text        NOT NULL,
    category           text        NOT NULL,          -- identity | business | tax | telco | banking
    status             text        NOT NULL,          -- live | sandbox | coming_soon | disabled
    supports_person    boolean     NOT NULL DEFAULT true,
    supports_business  boolean     NOT NULL DEFAULT false,
    description        text,
    created_at         timestamptz NOT NULL DEFAULT now(),
    updated_at         timestamptz NOT NULL DEFAULT now()
);

-- Seed known sources (sandbox unless real integration configured later)
INSERT INTO verification_sources (id, code, name, category, status, supports_person, supports_business, description) VALUES
  (gen_random_uuid(), 'telco_identity', 'Mobile / Telco Identity', 'telco', 'sandbox', true, false, 'SIM registration and mobile money identity (sandbox mock)'),
  (gen_random_uuid(), 'nida', 'National ID (NIDA)', 'identity', 'sandbox', true, false, 'Tanzania National Identification Authority (sandbox mock)'),
  (gen_random_uuid(), 'brela', 'Business Registry (BRELA)', 'business', 'sandbox', false, true, 'Business registration and ownership (sandbox mock)'),
  (gen_random_uuid(), 'tra', 'Tax Authority (TRA)', 'tax', 'sandbox', true, true, 'TIN and tax standing (sandbox mock)'),
  (gen_random_uuid(), 'bank_account', 'Bank Account Verification', 'banking', 'coming_soon', true, true, 'Bank account ownership / status (planned)');

CREATE INDEX idx_verification_sources_code ON verification_sources(code);
CREATE INDEX idx_verification_sources_status ON verification_sources(status);