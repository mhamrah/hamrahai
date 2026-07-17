ALTER TABLE music_import_runs
    ADD COLUMN IF NOT EXISTS stage TEXT NOT NULL DEFAULT 'queued'
        CHECK (stage IN ('queued', 'preparing', 'reading_spotify', 'creating_playlists', 'matching_artists', 'following_artists', 'completed', 'failed')),
    ADD COLUMN IF NOT EXISTS playlist_total INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS playlists_imported INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS artist_total INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS artists_checked INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS artists_matched INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS artists_followed INTEGER NOT NULL DEFAULT 0;
