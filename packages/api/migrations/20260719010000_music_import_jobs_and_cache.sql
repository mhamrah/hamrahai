ALTER TABLE music_import_runs
    ADD COLUMN IF NOT EXISTS task_attempts INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS worker_started_at TIMESTAMPTZ;

CREATE TABLE music_catalog_track_mappings (
    provider TEXT NOT NULL CHECK (provider IN ('spotify', 'tidal')),
    isrc TEXT NOT NULL,
    target_track_id TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (provider, isrc)
);

CREATE INDEX idx_music_catalog_track_mappings_expires
    ON music_catalog_track_mappings(expires_at);

CREATE TABLE music_playlist_sync_hashes (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    spotify_playlist_id TEXT NOT NULL,
    tidal_playlist_id TEXT NOT NULL,
    source_track_hash TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, spotify_playlist_id)
);
