-- Track link mutations for delta sync and preserve delete tombstones for clients.

ALTER TABLE links
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

UPDATE links
SET updated_at = created_at
WHERE updated_at IS NULL;

ALTER TABLE links
    ALTER COLUMN updated_at SET DEFAULT NOW(),
    ALTER COLUMN updated_at SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_links_user_updated_at ON links(user_id, updated_at ASC);
