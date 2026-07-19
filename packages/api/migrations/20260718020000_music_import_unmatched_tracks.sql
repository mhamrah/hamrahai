CREATE TABLE music_import_unmatched_tracks (
    id UUID PRIMARY KEY,
    import_id UUID NOT NULL REFERENCES music_import_runs(id) ON DELETE CASCADE,
    source_collection TEXT NOT NULL,
    track_name TEXT NOT NULL,
    artist_name TEXT,
    album_name TEXT,
    isrc TEXT,
    reason TEXT NOT NULL CHECK (reason IN ('missing_isrc', 'not_available_in_tidal')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_music_import_unmatched_tracks_import
    ON music_import_unmatched_tracks(import_id, created_at, id);
