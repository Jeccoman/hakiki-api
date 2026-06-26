-- Hakiki initial schema.
-- Design rule: verifications, consents and check_results are an APPEND-ONLY
-- audit trail. Nothing here is ever UPDATEd or DELETEd in normal operation —
-- a KYC broker must be able to prove what was checked, for whom, and with what
-- consent, after the fact.

create table waitlist_entries (
    id          uuid primary key,
    company     text        not null,
    email       text        not null,
    sector      text,
    created_at  timestamptz not null default now()
);

create table verifications (
    id                  uuid        primary key,
    subject_kind        text        not null,   -- person | business
    subject_reference   text        not null,   -- NIDA no, BRELA no, phone...
    subject_full_name   text,
    subject_phone       text,
    status              text        not null,   -- completed | pending
    created_at          timestamptz not null default now()
);

create table consents (
    id              uuid        primary key,
    verification_id uuid        not null references verifications(id),
    method          text        not null,       -- ussd_otp | signed_mandate...
    granted_at      timestamptz not null,
    recorded_at     timestamptz not null default now()
);

create table check_results (
    id              uuid        primary key,
    verification_id uuid        not null references verifications(id),
    check_kind      text        not null,       -- telco_identity | nida | brela | tra
    outcome         text        not null,       -- match | no_match | pending | unsupported
    detail          text        not null,
    created_at      timestamptz not null default now()
);

create index idx_consents_verification      on consents(verification_id);
create index idx_check_results_verification on check_results(verification_id);
create index idx_verifications_reference    on verifications(subject_reference);
