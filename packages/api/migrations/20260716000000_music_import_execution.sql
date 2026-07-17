CREATE UNIQUE INDEX IF NOT EXISTS idx_music_import_runs_one_active_per_user
    ON music_import_runs (user_id)
    WHERE status IN ('queued', 'running');
