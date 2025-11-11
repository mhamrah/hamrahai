use chrono::{Duration, Utc};
use std::env;
use uuid::Uuid;

use hamrah_server::db::{
    create_session, get_session_by_token, init_pool, purge_expired_sessions, rotate_session,
    run_migrations, DbPool,
};

/// Re-implement the hashing logic from db.rs for verification.
/// (Keep in sync with hash_refresh_token; if that changes, update here.)
fn expected_hash(raw: &str) -> String {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Helper: Acquire a database pool if DATABASE_URL is present; otherwise skip tests.
async fn setup_db() -> anyhow::Result<Option<DbPool>> {
    if env::var("DATABASE_URL").is_err() {
        eprintln!("Skipping session tests: DATABASE_URL not set");
        return Ok(None);
    }
    let pool = init_pool().await?;
    // Run migrations to ensure schema
    run_migrations(&pool).await?;
    Ok(Some(pool))
}

#[tokio::test]
async fn test_create_session_hashes_token() -> anyhow::Result<()> {
    let maybe_pool = setup_db().await?;
    let Some(pool) = maybe_pool else {
        return Ok(());
    };

    let user_id = Uuid::new_v4();
    // Minimal user insert for foreign key constraints if required
    sqlx::query("INSERT INTO users (id, email, name) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind("hash-test@example.com")
        .bind("Hash Test User")
        .execute(&pool)
        .await?;

    let raw_refresh = "raw_refresh_token_value";
    let session = create_session(&pool, user_id, raw_refresh, 6).await?;
    let expected = expected_hash(raw_refresh);
    assert_eq!(
        session.refresh_token, expected,
        "Stored refresh_token must be the SHA-256 base64url hash"
    );
    Ok(())
}

#[tokio::test]
async fn test_get_session_by_token_returns_session() -> anyhow::Result<()> {
    let maybe_pool = setup_db().await?;
    let Some(pool) = maybe_pool else {
        return Ok(());
    };

    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, name) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind("lookup-test@example.com")
        .bind("Lookup Test User")
        .execute(&pool)
        .await?;

    let raw_refresh = "another_raw_refresh_token";
    let created = create_session(&pool, user_id, raw_refresh, 1).await?;

    // Fetch by raw token (function hashes internally)
    let fetched = get_session_by_token(&pool, raw_refresh).await?;
    assert!(
        fetched.is_some(),
        "Session should be retrievable by original raw token"
    );
    let fetched_session = fetched.unwrap();
    assert_eq!(
        fetched_session.id, created.id,
        "Fetched session ID must match created session ID"
    );
    assert_eq!(
        fetched_session.refresh_token,
        expected_hash(raw_refresh),
        "Fetched hashed refresh token must match expected hash"
    );
    Ok(())
}

#[tokio::test]
async fn test_rotate_session_updates_hash_and_expiry() -> anyhow::Result<()> {
    let maybe_pool = setup_db().await?;
    let Some(pool) = maybe_pool else {
        return Ok(());
    };

    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, name) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind("rotate-test@example.com")
        .bind("Rotate Test User")
        .execute(&pool)
        .await?;

    let original_raw = "original_refresh_token";
    let session = create_session(&pool, user_id, original_raw, 1).await?;

    let new_raw = "rotated_refresh_token_value";
    let rotated = rotate_session(&pool, session.id, new_raw, 2).await?;

    assert_ne!(
        rotated.refresh_token, session.refresh_token,
        "Rotated hashed token must differ from original"
    );
    assert_eq!(
        rotated.refresh_token,
        expected_hash(new_raw),
        "Rotated token must equal expected hash"
    );
    assert!(
        rotated.expires_at > session.expires_at,
        "Rotated session expiry must be extended"
    );
    Ok(())
}

#[tokio::test]
async fn test_purge_expired_sessions_removes_old() -> anyhow::Result<()> {
    let maybe_pool = setup_db().await?;
    let Some(pool) = maybe_pool else {
        return Ok(());
    };

    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, name) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind("purge-test@example.com")
        .bind("Purge Test User")
        .execute(&pool)
        .await?;

    // Create one expired (expires_at in the past) and one valid session
    let expired_raw = "expired_refresh";
    let valid_raw = "valid_refresh";
    let now = Utc::now();

    // Insert directly to control timestamps
    sqlx::query(
        "INSERT INTO sessions (id, user_id, refresh_token, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(expected_hash(expired_raw))
    .bind(now - Duration::days(40))
    .bind(now - Duration::hours(1)) // already expired
    .execute(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO sessions (id, user_id, refresh_token, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(expected_hash(valid_raw))
    .bind(now - Duration::days(5))
    .bind(now + Duration::hours(6)) // still valid
    .execute(&pool)
    .await?;

    let affected = purge_expired_sessions(&pool).await?;
    assert!(
        affected >= 1,
        "At least one expired / old session should have been purged"
    );

    // Ensure valid session remains
    let valid = get_session_by_token(&pool, valid_raw).await?;
    assert!(
        valid.is_some(),
        "Valid non-expired session must remain after purge"
    );

    Ok(())
}
