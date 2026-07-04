-- Enhance check_results for normalized + raw data while preserving backward compatibility.
-- Old simple rows continue to work. New fields are nullable.

ALTER TABLE check_results
  ADD COLUMN IF NOT EXISTS confidence real,
  ADD COLUMN IF NOT EXISTS message text,
  ADD COLUMN IF NOT EXISTS normalized_fields jsonb,
  ADD COLUMN IF NOT EXISTS raw_response jsonb;

-- Optional: we keep the original "detail" column for simple human message.