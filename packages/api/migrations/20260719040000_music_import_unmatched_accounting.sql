WITH unmatched_tracking AS (
    SELECT installed_on
    FROM _sqlx_migrations
    WHERE
        version = 20260718020000
        AND success
),
persisted_unmatched AS (
    SELECT
        run.id,
        COUNT(track.id)::INTEGER AS unmatched_items
    FROM music_import_runs AS run
    JOIN unmatched_tracking AS tracking
        ON run.created_at >= tracking.installed_on
    LEFT JOIN music_import_unmatched_tracks AS track
        ON track.import_id = run.id
    WHERE
        run.status IN ('completed', 'partial')
        AND run.completed_at IS NOT NULL
        AND run.error IS NULL
    GROUP BY run.id
)
UPDATE music_import_runs AS run
SET
    unmatched_items = persisted_unmatched.unmatched_items,
    status = CASE
        WHEN
            run.status = 'partial'
            AND persisted_unmatched.unmatched_items = 0
        THEN 'completed'
        ELSE run.status
    END
FROM persisted_unmatched
WHERE
    run.id = persisted_unmatched.id
    AND (
        run.unmatched_items <> persisted_unmatched.unmatched_items
        OR (
            run.status = 'partial'
            AND persisted_unmatched.unmatched_items = 0
        )
    );
