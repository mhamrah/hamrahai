use axum::{
    Json,
    extract::Request,
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{Duration as ChronoDuration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{
    DbPool, User, create_session, delete_session_by_token, get_session_by_token,
    get_user_by_session_token, rotate_session, upsert_user_profile,
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
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub provider: Option<String>,
    pub provider_id: Option<String>,
    pub auth_method: Option<String>,
    pub platform: Option<String>,
    pub email_verified_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct TokensResponse {
    pub success: bool,
    pub user: User,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub expires_at: chrono::DateTime<Utc>,
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

pub async fn auth_native(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(req): Json<NativeLoginRequest>,
) -> impl IntoResponse {
    let platform = req.platform.as_deref().unwrap_or("web");
    let provider = req.provider.as_deref();
    let auth_method = req.auth_method.as_deref().or(provider).unwrap_or("oauth");
    let user = match upsert_user_profile(
        &pool,
        &req.email,
        req.name.as_deref(),
        req.picture.as_deref(),
        provider,
        req.provider_id.as_deref(),
        Some(auth_method),
        Some(platform),
        req.email_verified_at,
    )
    .await
    {
        Ok(u) => u,
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
        user,
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
        user,
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

pub async fn session_logout(State(pool): State<DbPool>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(response) = validate_unsafe_cookie_request(&headers) {
        return response;
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

pub async fn csrf_cookie_guard(req: Request, next: Next) -> Response {
    if requires_csrf_check(&req) {
        if let Err(response) = validate_unsafe_cookie_request(req.headers()) {
            return response;
        }
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

fn validate_unsafe_cookie_request(headers: &HeaderMap) -> Result<(), Response> {
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        if !allowed_origins().iter().any(|allowed| allowed == origin) {
            return Err(StatusCode::FORBIDDEN.into_response());
        }

        let csrf_cookie = read_cookie(headers, CSRF_COOKIE);
        let csrf_header = headers
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);

        if csrf_cookie.is_none() || csrf_cookie != csrf_header {
            return Err(StatusCode::FORBIDDEN.into_response());
        }
    }
    Ok(())
}

pub fn allowed_origins() -> Vec<String> {
    std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| {
            [
                "https://hamrah.app",
                "https://www.hamrah.app",
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
