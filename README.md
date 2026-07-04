# hakiki-api

Consent-first KYC / KYB verification infrastructure for Tanzania.

## Current status

- Fully working Axum + SQLx Postgres backend.
- Sandbox/mock adapters only for telco, NIDA, BRELA, TRA.
- Bank account source is marked `coming_soon`.
- Full verification lifecycle, risk scoring, review cases, webhooks foundation, request logging.
- Append-only audit design is preserved.

**Important**: This is **not** connected to live NIDA / BRELA / TRA / telco production systems. All adapters are deterministic mocks for development, demos, and early partner testing.

## Multi-tenant & Customer Ready Features

- Organizations + Members (owner/admin/developer/compliance_reviewer/viewer)
- All resources (verifications, keys, webhooks, reviews, logs, audit) scoped to organization_id. Full audit performed.
- API keys linked to organizations
- Idempotency-Key header support on verification creation (replay + conflict)
- Working webhook delivery worker with retries, backoff, Hakiki-Signature (HMAC)
- Real rate limiting per API key (env HAKIKI_RATE_LIMIT_PER_MINUTE, 429 + headers)
- Dashboard auth: POST /dashboard/auth/login (email), use token in X-Dashboard-Token
- Role/permission enforcement on dashboard
- API key management from dashboard
- Dashboard verification detail, review, developer endpoints

Demo org auto created. Use demo-public-key for /v1.

Dashboard login example: POST /dashboard/auth/login with {"email": "demo@hakiki.test"}

Protected dashboard use X-Dashboard-Token: <token from login>

Role checks active (e.g. for api-keys).

## Run locally

```bash
# Terminal 1 - database
docker compose up db

# Terminal 2 - api (after migrations run automatically)
cargo run
```

Default demo key: `demo-public-key`

## Key endpoints

### Public
- `GET /health`
- `POST /v1/waitlist`
- `POST /v1/contact`

### Protected (X-API-Key header required)
- `POST /v1/verifications`
- `GET  /v1/verifications/{id}`
- `GET  /v1/sources`
- `GET  /v1/api-requests`
- Webhook management + test
- Review case management

## Example: Run a verification (demo key)

```bash
curl -X POST http://localhost:8080/v1/verifications \
  -H "Content-Type: application/json" \
  -H "X-API-Key: demo-public-key" \
  -d '{
    "subject": {
      "kind": "person",
      "reference": "19900101123456789",
      "full_name": "Amina Juma",
      "phone": "+255712345678"
    },
    "checks": ["telco_identity", "nida"],
    "consent": {
      "method": "demo_consent",
      "granted_at": "2026-07-04T12:00:00Z"
    }
  }'
```

## Environment

- Demo key and early development use `environment = "test"`.
- Production keys should be issued with `hk_live_` style prefixes (future).

## Architecture notes

- Modular monolith.
- `VerificationSource` trait for easy addition of real adapters later.
- Risk engine is transparent rule-based (see `src/risk/`).
- All major actions produce rows in `audit_events`, `risk_*`, `review_*` etc.

See the code and migrations for the full picture.