CREATE TABLE music_import_playlists (
    import_id UUID NOT NULL REFERENCES music_import_runs(id) ON DELETE CASCADE,
    spotify_playlist_id TEXT NOT NULL,
    tidal_playlist_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (import_id, spotify_playlist_id)
);
