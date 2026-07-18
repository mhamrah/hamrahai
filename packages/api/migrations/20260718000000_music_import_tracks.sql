ALTER TABLE music_import_runs
    ADD COLUMN IF NOT EXISTS include_saved_tracks BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN IF NOT EXISTS playlist_track_total INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS playlist_tracks_imported INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS saved_track_total INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS saved_tracks_imported INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS tracks_matched INTEGER NOT NULL DEFAULT 0;

ALTER TABLE music_import_runs
    DROP CONSTRAINT IF EXISTS music_import_runs_stage_check,
    ADD CONSTRAINT music_import_runs_stage_check CHECK (
        stage IN (
            'queued',
            'preparing',
            'reading_spotify',
            'creating_playlists',
            'adding_playlist_tracks',
            'matching_artists',
            'following_artists',
            'saving_liked_tracks',
            'completed',
            'failed'
        )
    );
