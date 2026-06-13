use anyhow::Context;
use chrono::{Duration, Utc};
use serde::Serialize;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

pub type DbPool = Pool<Postgres>;

pub async fn init_pool() -> anyhow::Result<DbPool> {
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL environment variable must be set")?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("failed to connect to Postgres")?;
    Ok(pool)
}

pub async fn run_migrations(pool: &DbPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

#[derive(sqlx::FromRow, Clone, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub provider: Option<String>,
    pub provider_id: Option<String>,
    pub auth_method: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: Option<chrono::DateTime<Utc>>,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
    pub last_login_platform: Option<String>,
    pub email_verified_at: Option<chrono::DateTime<Utc>>,
}

pub async fn upsert_user(pool: &DbPool, email: &str, name: Option<&str>) -> anyhow::Result<User> {
    upsert_user_profile(pool, email, name, None, None, None, None, None, None).await
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_user_profile(
    pool: &DbPool,
    email: &str,
    name: Option<&str>,
    picture: Option<&str>,
    provider: Option<&str>,
    provider_id: Option<&str>,
    auth_method: Option<&str>,
    platform: Option<&str>,
    email_verified_at: Option<chrono::DateTime<Utc>>,
) -> anyhow::Result<User> {
    let id = Uuid::new_v4();
    let u = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (
            id, email, name, picture, provider, provider_id, auth_method,
            last_login_at, last_login_platform, email_verified_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), $8, $9)
        ON CONFLICT (email) DO UPDATE SET
            name = COALESCE(EXCLUDED.name, users.name),
            picture = COALESCE(EXCLUDED.picture, users.picture),
            provider = COALESCE(EXCLUDED.provider, users.provider),
            provider_id = COALESCE(EXCLUDED.provider_id, users.provider_id),
            auth_method = COALESCE(EXCLUDED.auth_method, users.auth_method),
            last_login_at = NOW(),
            last_login_platform = COALESCE(EXCLUDED.last_login_platform, users.last_login_platform),
            email_verified_at = COALESCE(EXCLUDED.email_verified_at, users.email_verified_at),
            updated_at = NOW()
        RETURNING id, email, name, picture, provider, provider_id, auth_method, created_at,
                  updated_at, last_login_at, last_login_platform, email_verified_at
        "#,
    )
    .bind(id)
    .bind(email)
    .bind(name)
    .bind(picture)
    .bind(provider)
    .bind(provider_id)
    .bind(auth_method)
    .bind(platform)
    .bind(email_verified_at)
    .fetch_one(pool)
    .await?;
    Ok(u)
}

pub async fn get_user_by_id(pool: &DbPool, id: Uuid) -> anyhow::Result<Option<User>> {
    let u = sqlx::query_as::<_, User>(
        r#"
        SELECT id, email, name, picture, provider, provider_id, auth_method, created_at,
               updated_at, last_login_at, last_login_platform, email_verified_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(u)
}

#[derive(sqlx::FromRow, Clone)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    // Stored as SHA-256 (URL-safe base64, no padding) of the raw refresh token
    pub refresh_token: String,
    pub created_at: chrono::DateTime<Utc>,
    pub expires_at: chrono::DateTime<Utc>,
}

pub async fn create_session(
    pool: &DbPool,
    user_id: Uuid,
    raw_refresh_token: &str,
    ttl_hours: i64,
) -> anyhow::Result<Session> {
    let id = Uuid::new_v4();
    let expires_at = Utc::now() + Duration::hours(ttl_hours);
    let hashed = hash_refresh_token(raw_refresh_token);
    let s = sqlx::query_as!(
        Session,
        r#"INSERT INTO sessions (id, user_id, refresh_token, expires_at)
           VALUES ($1, $2, $3, $4)
           RETURNING id, user_id, refresh_token, created_at, expires_at"#,
        id,
        user_id,
        hashed,
        expires_at
    )
    .fetch_one(pool)
    .await?;
    touch_session_for_lints(&s);
    Ok(s)
}

pub async fn get_session_by_token(
    pool: &DbPool,
    raw_token: &str,
) -> anyhow::Result<Option<Session>> {
    let hashed = hash_refresh_token(raw_token);
    let s = sqlx::query_as!(
        Session,
        r#"SELECT id, user_id, refresh_token, created_at, expires_at FROM sessions WHERE refresh_token = $1"#,
        hashed
    )
    .fetch_optional(pool)
    .await?;
    if let Some(ref sess) = s {
        touch_session_for_lints(sess);
    }
    Ok(s)
}

pub async fn get_user_by_session_token(
    pool: &DbPool,
    raw_token: &str,
) -> anyhow::Result<Option<(Session, User)>> {
    let Some(session) = get_session_by_token(pool, raw_token).await? else {
        return Ok(None);
    };

    if session.expires_at < Utc::now() {
        return Ok(None);
    }

    let Some(user) = get_user_by_id(pool, session.user_id).await? else {
        return Ok(None);
    };

    Ok(Some((session, user)))
}

pub async fn delete_session_by_token(pool: &DbPool, raw_token: &str) -> anyhow::Result<u64> {
    let hashed = hash_refresh_token(raw_token);
    let result = sqlx::query("DELETE FROM sessions WHERE refresh_token = $1")
        .bind(hashed)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn rotate_session(
    pool: &DbPool,
    session_id: Uuid,
    raw_new_token: &str,
    ttl_hours: i64,
) -> anyhow::Result<Session> {
    let expires_at = Utc::now() + Duration::hours(ttl_hours);
    let hashed = hash_refresh_token(raw_new_token);
    let s = sqlx::query_as!(
        Session,
        r#"UPDATE sessions SET refresh_token = $1, expires_at = $2 WHERE id = $3
           RETURNING id, user_id, refresh_token, created_at, expires_at"#,
        hashed,
        expires_at,
        session_id
    )
    .fetch_one(pool)
    .await?;
    touch_session_for_lints(&s);
    Ok(s)
}

pub async fn purge_expired_sessions(pool: &DbPool) -> anyhow::Result<u64> {
    // TODO: schedule this to run periodically (e.g., via a cron/worker)
    // Current policy: remove sessions that are expired OR very old by creation time.
    // Adjust the retention window as needed.
    let now = Utc::now();
    let threshold = now - Duration::days(30);
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at < $1 OR created_at < $2")
        .bind(now)
        .bind(threshold)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

fn hash_refresh_token(raw: &str) -> String {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

#[inline]
fn touch_session_for_lints(s: &Session) {
    let _ = std::hint::black_box(&s.refresh_token);
    let _ = std::hint::black_box(&s.created_at);
}
#[derive(sqlx::FromRow, Clone, Serialize)]
pub struct Tag {
    pub id: Uuid,
    pub name: String,
}

pub async fn upsert_tag(pool: &DbPool, name: &str) -> anyhow::Result<Tag> {
    if let Some(t) = sqlx::query_as!(Tag, r#"SELECT id, name FROM tags WHERE name = $1"#, name)
        .fetch_optional(pool)
        .await?
    {
        return Ok(t);
    }
    let id = Uuid::new_v4();
    let t = sqlx::query_as!(
        Tag,
        r#"INSERT INTO tags (id, name) VALUES ($1, $2) RETURNING id, name"#,
        id,
        name
    )
    .fetch_one(pool)
    .await?;
    Ok(t)
}

#[derive(sqlx::FromRow, Clone, Serialize)]
pub struct Summary {
    pub id: Uuid,
    pub link_id: Uuid,
    pub content: Option<String>,
    pub model: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

pub async fn set_link_tags(
    pool: &DbPool,
    user_id: Uuid,
    link_id: Uuid,
    tag_names: &[String],
) -> anyhow::Result<()> {
    // Ensure link belongs to user
    let owner = sqlx::query_scalar!(
        r#"SELECT user_id as "user_id: Uuid" FROM links WHERE id = $1"#,
        link_id
    )
    .fetch_optional(pool)
    .await?;
    if owner != Some(user_id) {
        return Err(anyhow::anyhow!("link not found or access denied"));
    }
    // Upsert tags and attach
    for name in tag_names {
        let tag = upsert_tag(pool, name).await?;
        sqlx::query!(
            r#"INSERT INTO link_tags (link_id, tag_id) VALUES ($1, $2)
               ON CONFLICT DO NOTHING"#,
            link_id,
            tag.id
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn list_tags_for_user(pool: &DbPool, user_id: Uuid) -> anyhow::Result<Vec<Tag>> {
    let tags = sqlx::query_as!(
        Tag,
        r#"SELECT DISTINCT t.id, t.name
           FROM tags t
           JOIN link_tags lt ON lt.tag_id = t.id
           JOIN links l ON l.id = lt.link_id
           WHERE l.user_id = $1
           ORDER BY t.name"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(tags)
}

pub async fn get_summary_for_link(
    pool: &DbPool,
    user_id: Uuid,
    link_id: Uuid,
) -> anyhow::Result<Option<Summary>> {
    // Ensure ownership and select summary
    let s = sqlx::query_as!(
        Summary,
        r#"SELECT s.id, s.link_id, s.content, s.model, s.created_at
           FROM summaries s
           JOIN links l ON l.id = s.link_id
           WHERE s.link_id = $1 AND l.user_id = $2
           ORDER BY s.created_at DESC
           LIMIT 1"#,
        link_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(s)
}
