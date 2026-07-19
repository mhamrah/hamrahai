ALTER TABLE music_import_runs
    DROP CONSTRAINT IF EXISTS music_import_runs_stage_check,
    ADD CONSTRAINT music_import_runs_stage_check CHECK (
        stage IN (
            'queued',
            'preparing',
            'reading_spotify',
            'creating_playlists',
            'adding_playlist_tracks',
            'reconciling_tidal_playlists',
            'matching_artists',
            'following_artists',
            'saving_liked_tracks',
            'completed',
            'failed'
        )
    );
