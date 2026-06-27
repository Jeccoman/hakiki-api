-- API keys for partner (bank/fintech) access to the verification endpoints.
-- Only the SHA-256 hash of a key is stored — the raw key is shown once at
-- issue time and never persisted. Keys are revoked by setting revoked_at,
-- never deleted, so the audit trail of who-had-access stays intact.

create table api_keys (
    id            uuid        primary key,
    partner_name  text        not null,
    key_hash      text        not null unique,
    created_at    timestamptz not null default now(),
    last_used_at  timestamptz,
    revoked_at    timestamptz
);
