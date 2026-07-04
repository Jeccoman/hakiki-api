-- Enhance webhook deliveries for real executable delivery with retries.

ALTER TABLE webhook_deliveries
  ADD COLUMN IF NOT EXISTS next_attempt_at timestamptz,
  ADD COLUMN IF NOT EXISTS max_attempts integer NOT NULL DEFAULT 5,
  ADD COLUMN IF NOT EXISTS last_error text;

-- Update status comment
COMMENT ON COLUMN webhook_deliveries.status IS 'pending | sending | delivered | failed | retrying';

-- Make sure we have indexes for worker
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_due ON webhook_deliveries(next_attempt_at) WHERE status IN ('pending', 'retrying');
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_status ON webhook_deliveries(status);