ALTER TABLE music_import_runs
    ADD COLUMN IF NOT EXISTS activity TEXT NOT NULL DEFAULT 'Waiting to start';

CREATE TABLE music_import_activity (
    id UUID PRIMARY KEY,
    import_id UUID NOT NULL REFERENCES music_import_runs(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_music_import_activity_import
    ON music_import_activity(import_id, created_at, id);
