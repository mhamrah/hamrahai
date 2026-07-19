ALTER TABLE music_import_runs
    ADD COLUMN IF NOT EXISTS worker_task_name TEXT,
    ADD COLUMN IF NOT EXISTS next_attempt_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_music_import_runs_next_attempt
    ON music_import_runs(next_attempt_at)
    WHERE status = 'queued' AND next_attempt_at IS NOT NULL;
