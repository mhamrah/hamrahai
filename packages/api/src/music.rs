use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, OsRng, rand_core::RngCore},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{auth::require_session_or_claims, db::DbPool};

mod import;

const SPOTIFY_SCOPES: &str =
    "user-read-private playlist-read-private playlist-read-collaborative user-follow-read";
const TIDAL_SCOPES: &str = "playlists.write collection.write search.read user.read";

#[derive(Deserialize)]
pub struct BeginConnectionRequest {
    pub redirect_path: Option<String>,
}
#[derive(Deserialize)]
pub struct MusicOAuthCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}
#[derive(Deserialize)]
struct ProviderTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    scope: Option<String>,
}
#[derive(Deserialize)]
struct SpotifyAccountResponse {
    id: String,
}
#[derive(Deserialize)]
struct TidalAccountResponse {
    data: TidalAccountData,
}
#[derive(Deserialize)]
struct TidalAccountData {
    id: String,
}
#[derive(sqlx::FromRow)]
struct OAuthState {
    user_id: Uuid,
    code_verifier: String,
    redirect_path: String,
}

#[derive(sqlx::FromRow)]
struct StoredMusicConnection {
    access_token_encrypted: Option<String>,
    refresh_token_encrypted: Option<String>,
    token_expires_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct BeginConnectionResponse {
    pub authorization_url: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MusicConnectionResponse {
    pub provider: String,
    pub provider_account_id: Option<String>,
    pub status: String,
    pub connected_at: Option<chrono::DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateImportRequest {
    pub include_owned_playlists: bool,
    #[serde(default)]
    pub include_saved_playlists: bool,
    #[serde(default = "default_true")]
    pub include_followed_artists: bool,
}
fn default_true() -> bool {
    true
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MusicImportResponse {
    pub id: Uuid,
    pub status: String,
    pub include_owned_playlists: bool,
    pub include_saved_playlists: bool,
    pub include_followed_artists: bool,
    pub stage: String,
    pub total_items: i32,
    pub imported_items: i32,
    pub unmatched_items: i32,
    pub playlist_total: i32,
    pub playlists_imported: i32,
    pub artist_total: i32,
    pub artists_checked: i32,
    pub artists_matched: i32,
    pub artists_followed: i32,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
}
fn error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ErrorResponse {
            success: false,
            error: message.to_string(),
        }),
    )
        .into_response()
}

fn valid_provider(provider: &str) -> bool {
    matches!(provider, "spotify" | "tidal")
}
fn redirect_path(value: Option<String>) -> Result<String, &'static str> {
    let value = value.unwrap_or_else(|| "/settings".to_string());
    if value.starts_with('/') && !value.starts_with("//") {
        Ok(value)
    } else {
        Err("redirect_path must be a local path")
    }
}
fn env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{name} is not configured"))
}
fn hash_state(state: &str) -> String {
    Sha256::digest(state.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn random_urlsafe(bytes: usize) -> String {
    let mut data = vec![0; bytes];
    OsRng.fill_bytes(&mut data);
    URL_SAFE_NO_PAD.encode(data)
}
fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
fn cipher(key: &[u8]) -> Result<Aes256Gcm, String> {
    Aes256Gcm::new_from_slice(key)
        .map_err(|_| "MUSIC_TOKEN_ENCRYPTION_KEY must decode to 32 bytes".to_string())
}
fn encrypt_token(token: &str) -> Result<String, String> {
    let value = env("MUSIC_TOKEN_ENCRYPTION_KEY")?;
    let key = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| "MUSIC_TOKEN_ENCRYPTION_KEY must be base64url".to_string())?;
    encrypt_token_with_key(token, &key)
}
fn encrypt_token_with_key(token: &str, key: &[u8]) -> Result<String, String> {
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher(key)?
        .encrypt(Nonce::from_slice(&nonce), token.as_bytes())
        .map_err(|_| "could not encrypt provider token".to_string())?;
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(nonce),
        URL_SAFE_NO_PAD.encode(ciphertext)
    ))
}
fn decrypt_token(token: &str) -> Result<String, String> {
    let value = env("MUSIC_TOKEN_ENCRYPTION_KEY")?;
    let key = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| "MUSIC_TOKEN_ENCRYPTION_KEY must be base64url".to_string())?;
    decrypt_token_with_key(token, &key)
}
fn decrypt_token_with_key(token: &str, key: &[u8]) -> Result<String, String> {
    let (nonce, ciphertext) = token
        .split_once('.')
        .ok_or_else(|| "stored provider token is invalid".to_string())?;
    let nonce = URL_SAFE_NO_PAD
        .decode(nonce)
        .map_err(|_| "stored provider token is invalid".to_string())?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(ciphertext)
        .map_err(|_| "stored provider token is invalid".to_string())?;
    let plaintext = cipher(key)?
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| "stored provider token could not be decrypted".to_string())?;
    String::from_utf8(plaintext).map_err(|_| "stored provider token is invalid".to_string())
}
fn query_value(value: &str) -> String {
    value.bytes().fold(String::new(), |mut encoded, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{byte:02X}");
        }
        encoded
    })
}

pub async fn list_connections(State(pool): State<DbPool>, headers: HeaderMap) -> Response {
    let claims = match require_session_or_claims(&pool, &headers).await {
        Ok(value) => value,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    match sqlx::query_as::<_, MusicConnectionResponse>("SELECT provider, provider_account_id, status, connected_at, last_error FROM music_connections WHERE user_id = $1 ORDER BY provider")
        .bind(claims.sub).fetch_all(&pool).await {
        Ok(rows) => Json(rows).into_response(), Err(err) => { tracing::error!(%err, "list music connections"); error(StatusCode::INTERNAL_SERVER_ERROR, "could not load music connections") }
    }
}

pub async fn begin_connection(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(provider): Path<String>,
    Json(request): Json<BeginConnectionRequest>,
) -> Response {
    let claims = match require_session_or_claims(&pool, &headers).await {
        Ok(value) => value,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    if !valid_provider(&provider) {
        return error(StatusCode::NOT_FOUND, "unsupported music provider");
    }
    let redirect_path = match redirect_path(request.redirect_path) {
        Ok(value) => value,
        Err(message) => return error(StatusCode::BAD_REQUEST, message),
    };
    let state = random_urlsafe(32);
    let verifier = random_urlsafe(64);
    let state_hash = hash_state(&state);
    if let Err(err) = sqlx::query("INSERT INTO music_oauth_states (state_hash, user_id, provider, code_verifier, redirect_path, expires_at) VALUES ($1,$2,$3,$4,$5,$6)")
        .bind(&state_hash).bind(claims.sub).bind(&provider).bind(&verifier).bind(&redirect_path).bind(Utc::now() + Duration::minutes(10)).execute(&pool).await {
        tracing::error!(%err, "create music oauth state"); return error(StatusCode::INTERNAL_SERVER_ERROR, "could not begin connection");
    }
    let url = match provider.as_str() {
        "spotify" => match (env("SPOTIFY_CLIENT_ID"), env("SPOTIFY_REDIRECT_URI")) {
            (Ok(client_id), Ok(callback)) => format!(
                "https://accounts.spotify.com/authorize?response_type=code&client_id={}&redirect_uri={}&state={state}&code_challenge={}&code_challenge_method=S256&scope={}",
                query_value(&client_id),
                query_value(&callback),
                code_challenge(&verifier),
                query_value(SPOTIFY_SCOPES)
            ),
            (Err(message), _) | (_, Err(message)) => {
                return error(StatusCode::SERVICE_UNAVAILABLE, &message);
            }
        },
        "tidal" => match (env("TIDAL_CLIENT_ID"), env("TIDAL_REDIRECT_URI")) {
            (Ok(client_id), Ok(callback)) => format!(
                "https://login.tidal.com/authorize?response_type=code&client_id={}&redirect_uri={}&state={state}&code_challenge={}&code_challenge_method=S256&scope={}",
                query_value(&client_id),
                query_value(&callback),
                code_challenge(&verifier),
                query_value(TIDAL_SCOPES)
            ),
            (Err(message), _) | (_, Err(message)) => {
                return error(StatusCode::SERVICE_UNAVAILABLE, &message);
            }
        },
        _ => unreachable!(),
    };
    Json(BeginConnectionResponse {
        authorization_url: url,
    })
    .into_response()
}

pub async fn disconnect_connection(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(provider): Path<String>,
) -> Response {
    let claims = match require_session_or_claims(&pool, &headers).await {
        Ok(value) => value,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    if !valid_provider(&provider) {
        return error(StatusCode::NOT_FOUND, "unsupported music provider");
    }
    match sqlx::query("DELETE FROM music_connections WHERE user_id = $1 AND provider = $2")
        .bind(claims.sub)
        .bind(provider)
        .execute(&pool)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            tracing::error!(%err, "disconnect music connection");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not disconnect music provider",
            )
        }
    }
}

pub async fn complete_connection(
    State(pool): State<DbPool>,
    Path(provider): Path<String>,
    axum::extract::Query(query): axum::extract::Query<MusicOAuthCallbackQuery>,
) -> Response {
    if !valid_provider(&provider) {
        return error(StatusCode::NOT_FOUND, "unsupported music provider");
    }
    let Some(state) = query.state else {
        return error(StatusCode::BAD_REQUEST, "missing oauth state");
    };
    let row = match sqlx::query_as::<_, OAuthState>("DELETE FROM music_oauth_states WHERE state_hash = $1 AND provider = $2 AND expires_at > NOW() RETURNING user_id, code_verifier, redirect_path")
        .bind(hash_state(&state)).bind(&provider).fetch_optional(&pool).await {
        Ok(Some(value)) => value, Ok(None) => return error(StatusCode::BAD_REQUEST, "expired or invalid oauth state"), Err(err) => { tracing::error!(%err, "consume music oauth state"); return error(StatusCode::INTERNAL_SERVER_ERROR, "could not complete connection"); }
    };
    if query.error.is_some() {
        return axum::response::Redirect::temporary(&format!(
            "{}{}?music_connection_error=declined",
            std::env::var("WEB_APP_URL").unwrap_or_else(|_| "https://hamrah.app".to_string()),
            row.redirect_path
        ))
        .into_response();
    }
    let Some(code) = query.code else {
        return error(StatusCode::BAD_REQUEST, "missing authorization code");
    };
    let token = match exchange_code(&provider, &code, &row.code_verifier).await {
        Ok(value) => value,
        Err(message) => {
            tracing::warn!(provider, %message, "music oauth exchange failed");
            return error(StatusCode::BAD_GATEWAY, "provider authorization failed");
        }
    };
    let provider_account_id = provider_account_id(&provider, &token.access_token).await;
    let access = match encrypt_token(&token.access_token) {
        Ok(value) => value,
        Err(message) => return error(StatusCode::SERVICE_UNAVAILABLE, &message),
    };
    let refresh = match token
        .refresh_token
        .as_deref()
        .map(encrypt_token)
        .transpose()
    {
        Ok(value) => value,
        Err(message) => return error(StatusCode::SERVICE_UNAVAILABLE, &message),
    };
    let scopes = token
        .scope
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let expiry = token
        .expires_in
        .map(|seconds| Utc::now() + Duration::seconds(seconds));
    if let Err(err) = sqlx::query("INSERT INTO music_connections (id,user_id,provider,provider_account_id,access_token_encrypted,refresh_token_encrypted,token_expires_at,granted_scopes,status,connected_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'connected',NOW()) ON CONFLICT (user_id,provider) DO UPDATE SET provider_account_id=COALESCE(EXCLUDED.provider_account_id, music_connections.provider_account_id), access_token_encrypted=EXCLUDED.access_token_encrypted, refresh_token_encrypted=EXCLUDED.refresh_token_encrypted, token_expires_at=EXCLUDED.token_expires_at, granted_scopes=EXCLUDED.granted_scopes, status='connected', last_error=NULL, connected_at=NOW(), updated_at=NOW()")
        .bind(Uuid::new_v4()).bind(row.user_id).bind(&provider).bind(provider_account_id).bind(access).bind(refresh).bind(expiry).bind(scopes).execute(&pool).await { tracing::error!(%err, "store music connection"); return error(StatusCode::INTERNAL_SERVER_ERROR, "could not save music connection"); }
    axum::response::Redirect::temporary(&format!(
        "{}{}?music_connection={provider}",
        std::env::var("WEB_APP_URL").unwrap_or_else(|_| "https://hamrah.app".to_string()),
        row.redirect_path
    ))
    .into_response()
}

async fn exchange_code(
    provider: &str,
    code: &str,
    verifier: &str,
) -> Result<ProviderTokenResponse, String> {
    let client = reqwest::Client::new();
    match provider {
        "spotify" => {
            let client_id = env("SPOTIFY_CLIENT_ID")?;
            let client_secret = env("SPOTIFY_CLIENT_SECRET")?;
            let redirect_uri = env("SPOTIFY_REDIRECT_URI")?;
            client
                .post("https://accounts.spotify.com/api/token")
                .basic_auth(client_id, Some(client_secret))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(format!(
                    "grant_type=authorization_code&code={}&redirect_uri={}&code_verifier={}",
                    query_value(code),
                    query_value(&redirect_uri),
                    query_value(verifier)
                ))
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json()
                .await
                .map_err(|e| e.to_string())
        }
        "tidal" => {
            let client_id = env("TIDAL_CLIENT_ID")?;
            let redirect_uri = env("TIDAL_REDIRECT_URI")?;
            client
                .post("https://auth.tidal.com/v1/oauth2/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(format!("grant_type=authorization_code&client_id={}&code={}&redirect_uri={}&code_verifier={}", query_value(&client_id), query_value(code), query_value(&redirect_uri), query_value(verifier)))
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json()
                .await
                .map_err(|e| e.to_string())
        }
        _ => Err("unsupported provider".to_string()),
    }
}

async fn provider_account_id(provider: &str, access_token: &str) -> Option<String> {
    let client = reqwest::Client::new();
    match provider {
        "spotify" => client
            .get("https://api.spotify.com/v1/me")
            .bearer_auth(access_token)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json::<SpotifyAccountResponse>()
            .await
            .ok()
            .map(|account| account.id),
        "tidal" => client
            .get("https://openapi.tidal.com/v2/users/me")
            .bearer_auth(access_token)
            .header("accept", "application/vnd.api+json")
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json::<TidalAccountResponse>()
            .await
            .ok()
            .map(|account| account.data.id),
        _ => None,
    }
}

async fn refresh_provider_token(
    provider: &str,
    refresh_token: &str,
) -> Result<ProviderTokenResponse, String> {
    let client = reqwest::Client::new();
    match provider {
        "spotify" => {
            let client_id = env("SPOTIFY_CLIENT_ID")?;
            let client_secret = env("SPOTIFY_CLIENT_SECRET")?;
            client
                .post("https://accounts.spotify.com/api/token")
                .basic_auth(client_id, Some(client_secret))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(format!(
                    "grant_type=refresh_token&refresh_token={}",
                    query_value(refresh_token)
                ))
                .send()
                .await
                .map_err(|error| error.to_string())?
                .error_for_status()
                .map_err(|error| error.to_string())?
                .json()
                .await
                .map_err(|error| error.to_string())
        }
        "tidal" => {
            let client_id = env("TIDAL_CLIENT_ID")?;
            client
                .post("https://auth.tidal.com/v1/oauth2/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(format!(
                    "grant_type=refresh_token&client_id={}&refresh_token={}",
                    query_value(&client_id),
                    query_value(refresh_token)
                ))
                .send()
                .await
                .map_err(|error| error.to_string())?
                .error_for_status()
                .map_err(|error| error.to_string())?
                .json()
                .await
                .map_err(|error| error.to_string())
        }
        _ => Err("unsupported provider".to_string()),
    }
}

async fn access_token_for_import(
    pool: &DbPool,
    user_id: Uuid,
    provider: &str,
) -> Result<String, String> {
    let connection = sqlx::query_as::<_, StoredMusicConnection>(
        "SELECT access_token_encrypted, refresh_token_encrypted, token_expires_at FROM music_connections WHERE user_id = $1 AND provider = $2 AND status = 'connected'",
    )
    .bind(user_id)
    .bind(provider)
    .fetch_optional(pool)
    .await
    .map_err(|error| error.to_string())?
    .ok_or_else(|| format!("{provider} is not connected"))?;
    let access_token = connection
        .access_token_encrypted
        .as_deref()
        .ok_or_else(|| format!("{provider} authorization is incomplete"))?;
    let expires_soon = connection
        .token_expires_at
        .is_some_and(|expires_at| expires_at <= Utc::now() + Duration::seconds(30));
    if !expires_soon {
        return decrypt_token(access_token);
    }
    let refresh_token = connection
        .refresh_token_encrypted
        .as_deref()
        .ok_or_else(|| format!("{provider} authorization expired; reconnect to continue"))?;
    let refresh_token = decrypt_token(refresh_token)?;
    let refreshed = refresh_provider_token(provider, &refresh_token).await?;
    let access_token_encrypted = encrypt_token(&refreshed.access_token)?;
    let refresh_token_encrypted = refreshed
        .refresh_token
        .as_deref()
        .map(encrypt_token)
        .transpose()?;
    let expires_at = refreshed
        .expires_in
        .map(|seconds| Utc::now() + Duration::seconds(seconds));
    sqlx::query("UPDATE music_connections SET access_token_encrypted = $1, refresh_token_encrypted = COALESCE($2, refresh_token_encrypted), token_expires_at = $3, status = 'connected', last_error = NULL, updated_at = NOW() WHERE user_id = $4 AND provider = $5")
        .bind(access_token_encrypted)
        .bind(refresh_token_encrypted)
        .bind(expires_at)
        .bind(user_id)
        .bind(provider)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;
    Ok(refreshed.access_token)
}

async fn persist_import_progress(
    pool: &DbPool,
    import_id: Uuid,
    progress: &import::ImportProgress,
) {
    if let Err(err) = sqlx::query("UPDATE music_import_runs SET stage = $1, total_items = $2, imported_items = $3, unmatched_items = $4, playlist_total = $5, playlists_imported = $6, artist_total = $7, artists_checked = $8, artists_matched = $9, artists_followed = $10 WHERE id = $11")
        .bind(progress.stage)
        .bind(progress.playlist_total + progress.artist_total)
        .bind(progress.playlists_imported + progress.artists_followed)
        .bind(progress.artists_unmatched)
        .bind(progress.playlist_total)
        .bind(progress.playlists_imported)
        .bind(progress.artist_total)
        .bind(progress.artists_checked)
        .bind(progress.artists_matched)
        .bind(progress.artists_followed)
        .bind(import_id)
        .execute(pool)
        .await
    {
        tracing::warn!(%err, %import_id, "persist music import progress");
    }
}

pub async fn create_import(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(request): Json<CreateImportRequest>,
) -> Response {
    let claims = match require_session_or_claims(&pool, &headers).await {
        Ok(value) => value,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    if !request.include_owned_playlists
        && !request.include_saved_playlists
        && !request.include_followed_artists
    {
        return error(
            StatusCode::BAD_REQUEST,
            "select at least one music collection",
        );
    }
    let connected: i64 = match sqlx::query_scalar("SELECT COUNT(*) FROM music_connections WHERE user_id = $1 AND provider IN ('spotify','tidal') AND status = 'connected'").bind(claims.sub).fetch_one(&pool).await { Ok(value) => value, Err(err) => { tracing::error!(%err, "check music connections"); return error(StatusCode::INTERNAL_SERVER_ERROR, "could not create music import") } };
    if connected != 2 {
        return error(
            StatusCode::CONFLICT,
            "connect spotify and tidal before importing",
        );
    }
    let id = Uuid::new_v4();
    if let Err(err) = sqlx::query("INSERT INTO music_import_runs (id,user_id,include_owned_playlists,include_saved_playlists,include_followed_artists,status,stage,started_at) VALUES ($1,$2,$3,$4,$5,'running','preparing',NOW())")
        .bind(id).bind(claims.sub).bind(request.include_owned_playlists).bind(request.include_saved_playlists).bind(request.include_followed_artists).execute(&pool).await {
        tracing::error!(%err, "create music import");
        return error(StatusCode::CONFLICT, "another music import is already running");
    }

    let initial_progress = import::ImportProgress::default();
    persist_import_progress(&pool, id, &initial_progress).await;
    let result = async {
        let spotify_access_token = access_token_for_import(&pool, claims.sub, "spotify")
            .await
            .map_err(|message| import::ImportFailure {
                message,
                outcome: import::ImportOutcome::default(),
                progress: initial_progress.clone(),
            })?;
        let tidal_access_token = access_token_for_import(&pool, claims.sub, "tidal")
            .await
            .map_err(|message| import::ImportFailure {
                message,
                outcome: import::ImportOutcome::default(),
                progress: initial_progress.clone(),
            })?;
        let provider =
            import::HttpMusicImportProvider::new(spotify_access_token, tidal_access_token);
        import::execute_import_with_progress(
            &provider,
            id,
            import::ImportOptions {
                include_owned_playlists: request.include_owned_playlists,
                include_saved_playlists: request.include_saved_playlists,
                include_followed_artists: request.include_followed_artists,
            },
            |progress| {
                let pool = pool.clone();
                async move { persist_import_progress(&pool, id, &progress).await }
            },
        )
        .await
    }
    .await;

    let (status, outcome, mut progress, import_error) = match result {
        Ok((outcome, progress)) if outcome.unmatched_items == 0 => {
            ("completed", outcome, progress, None)
        }
        Ok((outcome, progress)) => ("partial", outcome, progress, None),
        Err(failure) => {
            tracing::warn!(error = %failure.message, "music import failed");
            (
                "failed",
                failure.outcome,
                failure.progress,
                Some(
                    "could not complete music import; reconnect both providers and try again"
                        .to_string(),
                ),
            )
        }
    };
    progress.stage = if status == "failed" {
        "failed"
    } else {
        "completed"
    };
    match sqlx::query_as::<_, MusicImportResponse>("UPDATE music_import_runs SET status = $1, stage = $2, total_items = $3, imported_items = $4, unmatched_items = $5, playlist_total = $6, playlists_imported = $7, artist_total = $8, artists_checked = $9, artists_matched = $10, artists_followed = $11, error = $12, completed_at = NOW() WHERE id = $13 RETURNING id,status,include_owned_playlists,include_saved_playlists,include_followed_artists,stage,total_items,imported_items,unmatched_items,playlist_total,playlists_imported,artist_total,artists_checked,artists_matched,artists_followed,error,created_at")
        .bind(status).bind(progress.stage).bind(outcome.total_items).bind(outcome.imported_items).bind(outcome.unmatched_items).bind(progress.playlist_total).bind(progress.playlists_imported).bind(progress.artist_total).bind(progress.artists_checked).bind(progress.artists_matched).bind(progress.artists_followed).bind(import_error).bind(id).fetch_one(&pool).await {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(err) => { tracing::error!(%err, "complete music import"); error(StatusCode::INTERNAL_SERVER_ERROR, "could not complete music import") }
    }
}

pub async fn list_imports(State(pool): State<DbPool>, headers: HeaderMap) -> Response {
    let claims = match require_session_or_claims(&pool, &headers).await {
        Ok(value) => value,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    match sqlx::query_as::<_, MusicImportResponse>("SELECT id,status,include_owned_playlists,include_saved_playlists,include_followed_artists,stage,total_items,imported_items,unmatched_items,playlist_total,playlists_imported,artist_total,artists_checked,artists_matched,artists_followed,error,created_at FROM music_import_runs WHERE user_id = $1 ORDER BY created_at DESC LIMIT 20").bind(claims.sub).fetch_all(&pool).await {
        Ok(rows) => Json(rows).into_response(), Err(err) => { tracing::error!(%err, "list music imports"); error(StatusCode::INTERNAL_SERVER_ERROR, "could not load music imports") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_redirects_only() {
        assert!(redirect_path(Some("/settings".into())).is_ok());
        assert!(redirect_path(Some("https://bad.example".into())).is_err());
    }
    #[test]
    fn pkce_challenge_is_stable() {
        assert_eq!(
            code_challenge("abc"),
            "ungWv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0"
        );
    }
    #[test]
    fn state_hashes_without_retaining_value() {
        assert_ne!(hash_state("secret"), "secret");
    }
    #[test]
    fn encrypted_token_does_not_contain_plaintext() {
        assert!(
            !encrypt_token_with_key("provider-secret", &[7u8; 32])
                .unwrap()
                .contains("provider-secret")
        );
    }
    #[test]
    fn encrypted_token_round_trips() {
        let encrypted = encrypt_token_with_key("provider-secret", &[7u8; 32]).unwrap();
        assert_eq!(
            decrypt_token_with_key(&encrypted, &[7u8; 32]).unwrap(),
            "provider-secret"
        );
    }
}
