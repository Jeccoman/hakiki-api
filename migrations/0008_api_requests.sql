-- API request audit log (for observability, abuse detection, and partner transparency).

CREATE TABLE api_requests (
    id            uuid        PRIMARY KEY,
    api_key_id    uuid,                         -- references api_keys.id when known
    partner_name  text,
    method        text        NOT NULL,
    path          text        NOT NULL,
    status_code   integer,
    latency_ms    integer,
    request_id    text,
    ip_address    text,
    user_agent    text,
    environment   text,
    created_at    timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_api_requests_api_key ON api_requests(api_key_id);
CREATE INDEX idx_api_requests_created ON api_requests(created_at);
CREATE INDEX idx_api_requests_request_id ON api_requests(request_id);