use axum::{
    Json,
    extract::Request,
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{Duration as ChronoDuration, Utc};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, decode_header, encode,
    jwk::JwkSet,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::{
    DbPool, User, create_session, delete_session_by_token, get_session_by_token,
    get_user_by_auth_provider, get_user_by_session_token, link_user_auth_provider,
    list_user_auth_provider_names, rotate_session, update_user_login_profile, upsert_user_profile,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub email: String,
    pub iat: usize,
    pub exp: usize,
}

fn jwt_secret() -> anyhow::Result<String> {
    std::env::var("JWT_SECRET").map_err(|_| anyhow::anyhow!("JWT_SECRET must be set"))
}

pub fn issue_access_token(user: &User) -> anyhow::Result<String> {
    let now = Utc::now();
    let iat = now.timestamp() as usize;
    let exp = (now + ChronoDuration::hours(1)).timestamp() as usize;
    let claims = Claims {
        sub: user.id,
        email: user.email.clone(),
        iat,
        exp,
    };
    let key = EncodingKey::from_secret(jwt_secret()?.as_bytes());
    let token = encode(&Header::default(), &claims, &key)?;
    Ok(token)
}

fn validate_token(token: &str) -> bool {
    let k = DecodingKey::from_secret(jwt_secret().ok().unwrap_or_default().as_bytes());
    let key = k;
    let validation = Validation::default();
    decode::<Claims>(token, &key, &validation).is_ok()
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer ").map(|t| t.to_string()))
}

#[derive(Deserialize)]
pub struct NativeLoginRequest {
    pub email: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub provider: Option<String>,
    pub provider_id: Option<String>,
    pub auth_method: Option<String>,
    pub platform: Option<String>,
    /// When true, this is an account-linking operation rather than a sign-in.
    /// Linking must be authenticated so an expired client session cannot create
    /// a second account and replace the user's active session.
    pub link_provider: Option<String>,
    pub email_verified_at: Option<chrono::DateTime<Utc>>,
    pub id_token: Option<String>,
    pub credential: Option<String>,
}

#[derive(Serialize)]
pub struct TokensResponse {
    pub success: bool,
    pub user: AuthUserResponse,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub expires_at: chrono::DateTime<Utc>,
}

#[derive(Serialize)]
pub struct AuthUserResponse {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub provider: Option<String>,
    pub provider_id: Option<String>,
    pub auth_method: Option<String>,
    pub auth_providers: Vec<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: Option<chrono::DateTime<Utc>>,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
    pub last_login_platform: Option<String>,
    pub email_verified_at: Option<chrono::DateTime<Utc>>,
}

impl AuthUserResponse {
    async fn from_user(pool: &DbPool, user: User) -> Self {
        let mut auth_providers = list_user_auth_provider_names(pool, user.id)
            .await
            .unwrap_or_default();
        if auth_providers.is_empty()
            && let Some(provider) = user.provider.as_ref()
        {
            auth_providers.push(provider.clone());
        }

        Self {
            id: user.id,
            email: user.email,
            name: user.name,
            picture: user.picture,
            provider: user.provider,
            provider_id: user.provider_id,
            auth_method: user.auth_method,
            auth_providers,
            created_at: user.created_at,
            updated_at: user.updated_at,
            last_login_at: user.last_login_at,
            last_login_platform: user.last_login_platform,
            email_verified_at: user.email_verified_at,
        }
    }
}

#[derive(Serialize)]
pub struct SessionValidationResponse {
    pub success: bool,
    pub user: Option<User>,
    pub expires_at: Option<chrono::DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct LogoutResponse {
    pub success: bool,
}

#[derive(Serialize)]
struct AuthErrorResponse {
    success: bool,
    error: String,
}

struct VerifiedIdentity {
    email: String,
    name: Option<String>,
    picture: Option<String>,
    provider_id: Option<String>,
    email_verified_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct AppleClaims {
    sub: String,
    email: Option<String>,
    email_verified: Option<Value>,
}

pub async fn auth_native(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(req): Json<NativeLoginRequest>,
) -> impl IntoResponse {
    let platform = req
        .platform
        .as_deref()
        .unwrap_or_else(|| {
            if headers
                .get("x-requested-with")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value == "hamrah-ios")
            {
                "ios"
            } else {
                "web"
            }
        })
        .to_string();
    let provider = req.provider.as_deref().unwrap_or("oauth").to_string();
    let identity = match verify_native_identity(&provider, &req).await {
        Ok(identity) => identity,
        Err(error) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(AuthErrorResponse {
                    success: false,
                    error,
                }),
            )
                .into_response();
        }
    };
    let auth_method = req
        .auth_method
        .as_deref()
        .or(Some(provider.as_str()))
        .unwrap_or("oauth");
    let current_user = if req.link_provider.as_deref() == Some("true") {
        current_session_or_bearer_user(&pool, &headers).await
    } else {
        None
    };
    if req.link_provider.as_deref() == Some("true") && current_user.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthErrorResponse {
                success: false,
                error: "Sign in again before linking an authentication provider".to_string(),
            }),
        )
            .into_response();
    }
    let user = match resolve_native_user(&pool, &provider, &identity, current_user).await {
        Ok(user) => user,
        Err(ResolveNativeUserError::ProviderAlreadyLinked) => {
            return (
                StatusCode::CONFLICT,
                Json(AuthErrorResponse {
                    success: false,
                    error: "Auth provider is already linked to another account".to_string(),
                }),
            )
                .into_response();
        }
        Err(ResolveNativeUserError::Database) => {
            return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let user = match update_user_login_profile(
        &pool,
        user.id,
        identity.name.as_deref(),
        identity.picture.as_deref(),
        Some(&provider),
        identity.provider_id.as_deref(),
        Some(auth_method),
        Some(&platform),
        identity.email_verified_at,
    )
    .await
    {
        Ok(user) => user,
        Err(_) => return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let refresh = Uuid::new_v4().to_string();
    let session = match create_session(&pool, user.id, &refresh, 24 * 30).await {
        Ok(s) => s,
        Err(_e) => return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let access = match issue_access_token(&user) {
        Ok(t) => t,
        Err(_e) => return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let body = TokensResponse {
        success: true,
        user: AuthUserResponse::from_user(&pool, user).await,
        access_token: access,
        refresh_token: refresh.clone(),
        expires_in: 3600,
        expires_at: session.expires_at,
    };
    let mut response = Json(body).into_response();
    if platform == "web" {
        attach_session_cookies(response.headers_mut(), &headers, &refresh);
    }
    response
}

enum ResolveNativeUserError {
    ProviderAlreadyLinked,
    Database,
}

async fn current_session_or_bearer_user(pool: &DbPool, headers: &HeaderMap) -> Option<User> {
    let claims = require_session_or_claims(pool, headers).await.ok()?;
    crate::db::get_user_by_id(pool, claims.sub)
        .await
        .ok()
        .flatten()
}

async fn resolve_native_user(
    pool: &DbPool,
    provider: &str,
    identity: &VerifiedIdentity,
    current_user: Option<User>,
) -> Result<User, ResolveNativeUserError> {
    if let Some(provider_id) = identity.provider_id.as_deref()
        && let Some(linked_user) = get_user_by_auth_provider(pool, provider, provider_id)
            .await
            .map_err(|_| ResolveNativeUserError::Database)?
    {
        if let Some(current_user) = current_user.as_ref()
            && linked_user.id != current_user.id
        {
            return Err(ResolveNativeUserError::ProviderAlreadyLinked);
        }
        link_identity(pool, linked_user.id, provider, identity).await?;
        return Ok(linked_user);
    }

    let user = if let Some(current_user) = current_user {
        current_user
    } else {
        upsert_user_profile(
            pool,
            &identity.email,
            identity.name.as_deref(),
            identity.picture.as_deref(),
            Some(provider),
            identity.provider_id.as_deref(),
            Some(provider),
            None,
            identity.email_verified_at,
        )
        .await
        .map_err(|_| ResolveNativeUserError::Database)?
    };

    link_identity(pool, user.id, provider, identity).await?;
    Ok(user)
}

async fn link_identity(
    pool: &DbPool,
    user_id: Uuid,
    provider: &str,
    identity: &VerifiedIdentity,
) -> Result<(), ResolveNativeUserError> {
    if let Some(provider_id) = identity.provider_id.as_deref() {
        link_user_auth_provider(
            pool,
            user_id,
            provider,
            provider_id,
            &identity.email,
            identity.name.as_deref(),
            identity.picture.as_deref(),
        )
        .await
        .map_err(|error| {
            if error
                .to_string()
                .contains("already linked to another account")
            {
                ResolveNativeUserError::ProviderAlreadyLinked
            } else {
                ResolveNativeUserError::Database
            }
        })?;
    }
    Ok(())
}

async fn verify_native_identity(
    provider: &str,
    req: &NativeLoginRequest,
) -> Result<VerifiedIdentity, String> {
    match provider {
        "google" => verify_google_identity(req).await,
        "apple" => verify_apple_identity(req).await,
        _ => verify_legacy_identity(req, "Email is required for native authentication"),
    }
}

fn native_identity_token<'a>(
    req: &'a NativeLoginRequest,
    missing_message: &str,
) -> Result<&'a str, String> {
    let id_token = req
        .id_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty());
    let credential = req
        .credential
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty());

    match (id_token, credential) {
        (Some(id_token), Some(credential)) if id_token != credential => {
            Err("Conflicting native auth tokens".to_string())
        }
        (Some(token), _) | (_, Some(token)) => Ok(token),
        (None, None) => Err(missing_message.to_string()),
    }
}

fn verify_legacy_identity(
    req: &NativeLoginRequest,
    missing_message: &str,
) -> Result<VerifiedIdentity, String> {
    let email = req
        .email
        .clone()
        .filter(|email| !email.trim().is_empty())
        .ok_or_else(|| missing_message.to_string())?;

    Ok(VerifiedIdentity {
        email,
        name: req.name.clone(),
        picture: req.picture.clone(),
        provider_id: req.provider_id.clone(),
        email_verified_at: req.email_verified_at,
    })
}

async fn verify_google_identity(req: &NativeLoginRequest) -> Result<VerifiedIdentity, String> {
    let id_token = native_identity_token(req, "Google ID token is missing")?;

    if std::env::var("GOOGLE_AUTH_TEST_BYPASS").as_deref() == Ok("true")
        && let Some(email) = id_token.strip_prefix("test-google:")
    {
        return Ok(VerifiedIdentity {
            email: email.to_string(),
            name: req.name.clone(),
            picture: req.picture.clone(),
            provider_id: req
                .provider_id
                .clone()
                .or_else(|| Some(format!("test-google-user:{email}"))),
            email_verified_at: Some(Utc::now()),
        });
    }

    let token_info = reqwest::Client::new()
        .get(format!(
            "https://oauth2.googleapis.com/tokeninfo?id_token={id_token}"
        ))
        .send()
        .await
        .map_err(|error| format!("Google token verification failed: {error}"))?;

    if !token_info.status().is_success() {
        return Err("Google ID token was rejected".to_string());
    }

    let claims: Value = token_info
        .json()
        .await
        .map_err(|error| format!("Failed to parse Google token response: {error}"))?;

    let audience = claims
        .get("aud")
        .and_then(Value::as_str)
        .ok_or_else(|| "Google token is missing an audience".to_string())?;
    if !allowed_google_audiences()
        .iter()
        .any(|allowed| allowed == audience)
    {
        return Err("Google token audience is not allowed".to_string());
    }

    let email_verified = claims
        .get("email_verified")
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| value.as_str().map(|raw| raw.eq_ignore_ascii_case("true")))
        })
        .unwrap_or(false);
    if !email_verified {
        return Err("Google account email is not verified".to_string());
    }

    let email = claims
        .get("email")
        .and_then(Value::as_str)
        .filter(|email| !email.is_empty())
        .ok_or_else(|| "Google token is missing an email".to_string())?
        .to_string();

    Ok(VerifiedIdentity {
        email,
        name: claims
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| req.name.clone()),
        picture: claims
            .get("picture")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| req.picture.clone()),
        provider_id: claims
            .get("sub")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| req.provider_id.clone()),
        email_verified_at: Some(Utc::now()),
    })
}

async fn verify_apple_identity(req: &NativeLoginRequest) -> Result<VerifiedIdentity, String> {
    let id_token = native_identity_token(req, "Apple identity token is missing")?;

    if std::env::var("APPLE_AUTH_TEST_BYPASS").as_deref() == Ok("true")
        && let Some(email) = id_token.strip_prefix("test-apple:")
    {
        return Ok(VerifiedIdentity {
            email: email.to_string(),
            name: req.name.clone(),
            picture: req.picture.clone(),
            provider_id: req
                .provider_id
                .clone()
                .or_else(|| Some("test-apple-user".to_string())),
            email_verified_at: Some(Utc::now()),
        });
    }

    let header =
        decode_header(id_token).map_err(|error| format!("Invalid Apple token header: {error}"))?;
    if header.alg != Algorithm::RS256 {
        return Err("Apple token algorithm is not allowed".to_string());
    }
    let kid = header
        .kid
        .as_deref()
        .ok_or_else(|| "Apple token is missing a key id".to_string())?;

    let jwks: JwkSet = reqwest::Client::new()
        .get("https://appleid.apple.com/auth/keys")
        .send()
        .await
        .map_err(|error| format!("Apple key lookup failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Apple key lookup failed: {error}"))?
        .json()
        .await
        .map_err(|error| format!("Failed to parse Apple keys: {error}"))?;

    let jwk = jwks
        .find(kid)
        .ok_or_else(|| "No Apple signing key matched the token".to_string())?;
    let decoding_key = DecodingKey::from_jwk(jwk)
        .map_err(|error| format!("Invalid Apple signing key: {error}"))?;

    let audiences = allowed_apple_audiences();
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["https://appleid.apple.com"]);
    validation.set_audience(&audiences);

    let claims = decode::<AppleClaims>(id_token, &decoding_key, &validation)
        .map_err(|error| format!("Apple identity token was rejected: {error}"))?
        .claims;

    let email = claims
        .email
        .or_else(|| req.email.clone())
        .filter(|email| !email.trim().is_empty())
        .ok_or_else(|| "Apple token is missing an email".to_string())?;
    let provider_id = (!claims.sub.trim().is_empty())
        .then_some(claims.sub)
        .or_else(|| req.provider_id.clone());

    Ok(VerifiedIdentity {
        email,
        name: req.name.clone().filter(|name| !name.trim().is_empty()),
        picture: req.picture.clone(),
        provider_id,
        email_verified_at: apple_email_is_verified(claims.email_verified).then(Utc::now),
    })
}

fn apple_email_is_verified(value: Option<Value>) -> bool {
    value
        .and_then(|value| {
            value
                .as_bool()
                .or_else(|| value.as_str().map(|raw| raw.eq_ignore_ascii_case("true")))
        })
        .unwrap_or(false)
}

fn allowed_google_audiences() -> Vec<String> {
    std::env::var("GOOGLE_ALLOWED_CLIENT_IDS")
        .unwrap_or_else(|_| {
            "66020219411-bs8v3cvpah62q616uopgk0iasebnh4jh.apps.googleusercontent.com".to_string()
        })
        .split(',')
        .map(str::trim)
        .filter(|audience| !audience.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn allowed_apple_audiences() -> Vec<String> {
    std::env::var("APPLE_ALLOWED_CLIENT_IDS")
        .or_else(|_| std::env::var("APPLE_CLIENT_ID"))
        .unwrap_or_else(|_| "app.hamrah.ios,app.hamrah.web".to_string())
        .split(',')
        .map(str::trim)
        .filter(|audience| !audience.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

pub async fn auth_refresh(
    State(pool): State<DbPool>,
    Json(req): Json<RefreshRequest>,
) -> impl IntoResponse {
    let session = match get_session_by_token(&pool, &req.refresh_token).await {
        Ok(Some(s)) => s,
        Ok(None) => return axum::http::StatusCode::UNAUTHORIZED.into_response(),
        Err(_e) => return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if session.expires_at < Utc::now() {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }
    let new_refresh = Uuid::new_v4().to_string();
    let rotated = match rotate_session(&pool, session.id, &new_refresh, 24 * 30).await {
        Ok(s) => s,
        Err(_e) => return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let user = match crate::db::get_user_by_id(&pool, rotated.user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => return axum::http::StatusCode::UNAUTHORIZED.into_response(),
        Err(_e) => return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let access = match issue_access_token(&user) {
        Ok(t) => t,
        Err(_e) => return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let resp = TokensResponse {
        success: true,
        user: AuthUserResponse::from_user(&pool, user).await,
        access_token: access,
        refresh_token: new_refresh,
        expires_in: 3600,
        expires_at: rotated.expires_at,
    };
    Json(resp).into_response()
}

pub async fn session_validate(State(pool): State<DbPool>, headers: HeaderMap) -> impl IntoResponse {
    let Some(session_token) = read_cookie(&headers, SESSION_COOKIE) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(SessionValidationResponse {
                success: false,
                user: None,
                expires_at: None,
                error: Some("missing_session".to_string()),
            }),
        )
            .into_response();
    };

    match get_user_by_session_token(&pool, &session_token).await {
        Ok(Some((session, user))) => (
            StatusCode::OK,
            Json(SessionValidationResponse {
                success: true,
                user: Some(user),
                expires_at: Some(session.expires_at),
                error: None,
            }),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::UNAUTHORIZED,
            Json(SessionValidationResponse {
                success: false,
                user: None,
                expires_at: None,
                error: Some("invalid_session".to_string()),
            }),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn session_logout(State(pool): State<DbPool>, headers: HeaderMap) -> Response {
    if let Err(status) = validate_unsafe_cookie_request(&headers) {
        return status.into_response();
    }

    if let Some(session_token) = read_cookie(&headers, SESSION_COOKIE) {
        let _ = delete_session_by_token(&pool, &session_token).await;
    }

    let mut response = Json(LogoutResponse { success: true }).into_response();
    clear_session_cookies(response.headers_mut(), &headers);
    response
}

pub async fn auth_validate(headers: HeaderMap) -> impl IntoResponse {
    let valid = bearer_token(&headers)
        .map(|t| validate_token(&t))
        .unwrap_or(false);
    Json(serde_json::json!({"valid": valid}))
}

pub fn require_claims(headers: &HeaderMap) -> anyhow::Result<Claims> {
    let token = bearer_token(headers).ok_or_else(|| anyhow::anyhow!("missing bearer token"))?;
    let key = DecodingKey::from_secret(jwt_secret()?.as_bytes());
    let validation = Validation::default();
    let data = decode::<Claims>(&token, &key, &validation)?;
    Ok(data.claims)
}

/// Resolves an authenticated API caller from either a native bearer token or the
/// shared web session cookie. Cookie-authenticated unsafe requests remain
/// protected by `csrf_cookie_guard`.
pub async fn require_session_or_claims(
    pool: &DbPool,
    headers: &HeaderMap,
) -> anyhow::Result<Claims> {
    if let Ok(claims) = require_claims(headers) {
        return Ok(claims);
    }

    let session_token = read_cookie(headers, SESSION_COOKIE)
        .ok_or_else(|| anyhow::anyhow!("missing bearer token or session cookie"))?;
    let (session, user) = get_user_by_session_token(pool, &session_token)
        .await?
        .ok_or_else(|| anyhow::anyhow!("invalid session"))?;

    Ok(Claims {
        sub: user.id,
        email: user.email,
        iat: session.created_at.timestamp().max(0) as usize,
        exp: session.expires_at.timestamp().max(0) as usize,
    })
}

pub async fn csrf_cookie_guard(req: Request, next: Next) -> Response {
    if requires_csrf_check(&req)
        && let Err(status) = validate_unsafe_cookie_request(req.headers())
    {
        return status.into_response();
    }

    next.run(req).await
}

pub const SESSION_COOKIE: &str = "session";
pub const CSRF_COOKIE: &str = "csrf_token";
const SESSION_MAX_AGE_SECONDS: i64 = 60 * 60 * 24 * 30;

pub fn attach_session_cookies(headers: &mut HeaderMap, request_headers: &HeaderMap, token: &str) {
    let csrf_token = Uuid::new_v4().to_string();
    append_set_cookie(
        headers,
        build_cookie(
            request_headers,
            SESSION_COOKIE,
            token,
            true,
            SESSION_MAX_AGE_SECONDS,
        ),
    );
    append_set_cookie(
        headers,
        build_cookie(
            request_headers,
            CSRF_COOKIE,
            &csrf_token,
            false,
            SESSION_MAX_AGE_SECONDS,
        ),
    );
}

pub fn clear_session_cookies(headers: &mut HeaderMap, request_headers: &HeaderMap) {
    append_set_cookie(
        headers,
        build_cookie(request_headers, SESSION_COOKIE, "", true, 0),
    );
    append_set_cookie(
        headers,
        build_cookie(request_headers, CSRF_COOKIE, "", false, 0),
    );
}

pub fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|part| {
        let trimmed = part.trim();
        let (key, value) = trimmed.split_once('=')?;
        (key == name).then(|| value.to_string())
    })
}

fn append_set_cookie(headers: &mut HeaderMap, value: String) {
    if let Ok(header_value) = HeaderValue::from_str(&value) {
        headers.append(header::SET_COOKIE, header_value);
    }
}

fn build_cookie(
    request_headers: &HeaderMap,
    name: &str,
    value: &str,
    http_only: bool,
    max_age: i64,
) -> String {
    let mut cookie = format!(
        "{}={}; Path=/; Max-Age={}; SameSite=Lax",
        name, value, max_age
    );
    if should_use_secure_cookie(request_headers) {
        cookie.push_str("; Secure");
    }
    if !is_local_request(request_headers) {
        cookie.push_str("; Domain=.hamrah.app");
    }
    if http_only {
        cookie.push_str("; HttpOnly");
    }
    cookie
}

fn should_use_secure_cookie(headers: &HeaderMap) -> bool {
    !is_local_request(headers)
        || headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|proto| proto.eq_ignore_ascii_case("https"))
}

fn is_local_request(headers: &HeaderMap) -> bool {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    host.starts_with("localhost")
        || host.starts_with("127.0.0.1")
        || origin.starts_with("http://localhost")
        || origin.starts_with("https://localhost")
        || origin.starts_with("http://127.0.0.1")
        || origin.starts_with("https://127.0.0.1")
}

fn requires_csrf_check(req: &Request) -> bool {
    let method = req.method();
    let is_unsafe = method == Method::POST
        || method == Method::PUT
        || method == Method::PATCH
        || method == Method::DELETE;
    if !is_unsafe {
        return false;
    }

    let path = req.uri().path();
    let creates_session = matches!(
        path,
        "/api/auth/native"
            | "/api/webauthn/authenticate/discoverable"
            | "/api/webauthn/authenticate/discoverable/verify"
    );

    !creates_session && read_cookie(req.headers(), SESSION_COOKIE).is_some()
}

fn validate_unsafe_cookie_request(headers: &HeaderMap) -> Result<(), StatusCode> {
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        if !allowed_origins().iter().any(|allowed| allowed == origin) {
            return Err(StatusCode::FORBIDDEN);
        }

        let csrf_cookie = read_cookie(headers, CSRF_COOKIE);
        let csrf_header = headers
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);

        if csrf_cookie.is_none() || csrf_cookie != csrf_header {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(())
}

pub fn allowed_origins() -> Vec<String> {
    std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| {
            [
                "https://hamrah.app",
                "http://localhost:5173",
                "https://localhost:5173",
                "http://127.0.0.1:5173",
                "https://127.0.0.1:5173",
            ]
            .join(",")
        })
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
