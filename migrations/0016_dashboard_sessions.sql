-- Dashboard auth foundation: simple session tokens for MVP.

CREATE TABLE dashboard_sessions (
    id uuid PRIMARY KEY,
    member_id uuid NOT NULL REFERENCES organization_members(id),
    organization_id uuid NOT NULL REFERENCES organizations(id),
    token text NOT NULL UNIQUE,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_dashboard_sessions_token ON dashboard_sessions(token);
CREATE INDEX idx_dashboard_sessions_expires ON dashboard_sessions(expires_at);