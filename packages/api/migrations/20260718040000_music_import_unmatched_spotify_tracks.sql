ALTER TABLE music_import_unmatched_tracks
    DROP CONSTRAINT music_import_unmatched_tracks_reason_check;

ALTER TABLE music_import_unmatched_tracks
    ADD CONSTRAINT music_import_unmatched_tracks_reason_check
    CHECK (reason IN ('missing_isrc', 'not_available_in_tidal', 'not_available_in_spotify'));
