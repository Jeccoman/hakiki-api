-- Add environment + display prefix to API keys (test vs live separation).

ALTER TABLE api_keys
  ADD COLUMN IF NOT EXISTS environment text NOT NULL DEFAULT 'test',
  ADD COLUMN IF NOT EXISTS display_prefix text;

-- Backfill
UPDATE api_keys SET environment = 'test' WHERE environment IS NULL;

CREATE INDEX IF NOT EXISTS idx_api_keys_environment ON api_keys(environment);