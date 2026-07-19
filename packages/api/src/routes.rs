use crate::attestation;
use crate::auth;
use crate::db::DbPool;
use crate::links;
use crate::models;
use crate::music;
use crate::preferences;
use crate::summaries;
use crate::tags;
use crate::users;
use crate::webauthn;
use axum::response::{IntoResponse, Response};
use axum::{
    Router,
    http::{
        HeaderName, Method,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    middleware,
    routing::{delete, get, patch, post},
};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};

pub type AppState = (DbPool, Arc<webauthn::WebAuthnConfig>);

// Wrapper handlers that extract pool from tuple state for existing handlers
async fn auth_native_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    json: axum::Json<auth::NativeLoginRequest>,
) -> Response {
    auth::auth_native(axum::extract::State(pool), headers, json)
        .await
        .into_response()
}

async fn auth_refresh_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    json: axum::Json<auth::RefreshRequest>,
) -> impl IntoResponse {
    auth::auth_refresh(axum::extract::State(pool), json).await
}

async fn auth_validate_wrapper(headers: axum::http::HeaderMap) -> impl IntoResponse {
    auth::auth_validate(headers).await
}

async fn session_validate_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    auth::session_validate(axum::extract::State(pool), headers).await
}

async fn session_logout_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    auth::session_logout(axum::extract::State(pool), headers).await
}

async fn attestation_challenge_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    json: axum::Json<attestation::AttestationChallengeRequest>,
) -> impl IntoResponse {
    attestation::challenge(axum::extract::State(pool), headers, json).await
}

async fn attestation_verify_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    json: axum::Json<attestation::AttestationVerifyRequest>,
) -> impl IntoResponse {
    attestation::verify_attestation(axum::extract::State(pool), headers, json).await
}

async fn attestation_assert_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    json: axum::Json<attestation::AssertionRequest>,
) -> impl IntoResponse {
    attestation::verify_assertion(axum::extract::State(pool), headers, json).await
}

async fn list_links_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    query: axum::extract::Query<links::ListLinksQuery>,
) -> impl IntoResponse {
    if let Some(response) = attestation::reject_invalid_request_headers(&pool, &headers).await {
        return response;
    }
    links::list_links_with_query(axum::extract::State(pool), headers, query)
        .await
        .into_response()
}

async fn create_link_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    json: axum::Json<links::CreateLinkRequest>,
) -> Response {
    if let Some(response) = attestation::reject_invalid_request_headers(&pool, &headers).await {
        return response;
    }
    links::create_link(axum::extract::State(pool), headers, json)
        .await
        .into_response()
}

async fn update_link_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: axum::extract::Path<uuid::Uuid>,
    json: axum::Json<links::UpdateLinkRequest>,
) -> Response {
    if let Some(response) = attestation::reject_invalid_request_headers(&pool, &headers).await {
        return response;
    }
    links::update_link(axum::extract::State(pool), headers, path, json)
        .await
        .into_response()
}

async fn delete_link_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: axum::extract::Path<uuid::Uuid>,
) -> Response {
    if let Some(response) = attestation::reject_invalid_request_headers(&pool, &headers).await {
        return response;
    }
    links::delete_link(axum::extract::State(pool), headers, path)
        .await
        .into_response()
}

async fn me_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> Response {
    if let Some(response) = attestation::reject_invalid_request_headers(&pool, &headers).await {
        return response;
    }
    users::me(axum::extract::State(pool), headers)
        .await
        .into_response()
}

async fn list_tags_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> Response {
    if let Some(response) = attestation::reject_invalid_request_headers(&pool, &headers).await {
        return response;
    }
    tags::list_tags(axum::extract::State(pool), headers)
        .await
        .into_response()
}

async fn latest_summary_for_link_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: axum::extract::Path<uuid::Uuid>,
) -> Response {
    if let Some(response) = attestation::reject_invalid_request_headers(&pool, &headers).await {
        return response;
    }
    summaries::latest_summary_for_link(axum::extract::State(pool), headers, path)
        .await
        .into_response()
}

async fn set_tags_for_link_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: axum::extract::Path<uuid::Uuid>,
    json: axum::Json<tags::SetTagsRequest>,
) -> Response {
    if let Some(response) = attestation::reject_invalid_request_headers(&pool, &headers).await {
        return response;
    }
    tags::set_tags_for_link(axum::extract::State(pool), headers, path, json)
        .await
        .into_response()
}

async fn get_user_prefs_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> Response {
    if let Some(response) = attestation::reject_invalid_request_headers(&pool, &headers).await {
        return response;
    }
    preferences::get_user_prefs(axum::extract::State(pool), headers).await
}

async fn update_user_prefs_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    json: axum::Json<preferences::UpdateUserPrefsRequest>,
) -> Response {
    if let Some(response) = attestation::reject_invalid_request_headers(&pool, &headers).await {
        return response;
    }
    preferences::update_user_prefs(axum::extract::State(pool), headers, json).await
}

async fn list_music_connections_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> Response {
    music::list_connections(axum::extract::State(pool), headers).await
}

async fn begin_music_connection_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: axum::extract::Path<String>,
    json: axum::Json<music::BeginConnectionRequest>,
) -> Response {
    music::begin_connection(axum::extract::State(pool), headers, path, json).await
}

async fn music_connection_callback_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    path: axum::extract::Path<String>,
    query: axum::extract::Query<music::MusicOAuthCallbackQuery>,
) -> Response {
    music::complete_connection(axum::extract::State(pool), path, query).await
}

async fn disconnect_music_connection_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: axum::extract::Path<String>,
) -> Response {
    music::disconnect_connection(axum::extract::State(pool), headers, path).await
}

async fn create_music_import_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    json: axum::Json<music::CreateImportRequest>,
) -> Response {
    music::create_import(axum::extract::State(pool), headers, json).await
}

async fn restart_music_import_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: axum::extract::Path<uuid::Uuid>,
    json: Option<axum::Json<music::CreateImportRequest>>,
) -> Response {
    music::restart_import(axum::extract::State(pool), headers, path, json).await
}

async fn list_music_imports_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> Response {
    music::list_imports(axum::extract::State(pool), headers).await
}

async fn list_music_unmatched_tracks_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: axum::extract::Path<uuid::Uuid>,
) -> Response {
    music::list_unmatched_tracks(axum::extract::State(pool), headers, path).await
}

async fn list_music_import_activity_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: axum::extract::Path<uuid::Uuid>,
) -> Response {
    music::list_import_activity(axum::extract::State(pool), headers, path).await
}

async fn delete_music_import_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: axum::extract::Path<uuid::Uuid>,
) -> Response {
    music::delete_import(axum::extract::State(pool), headers, path).await
}

async fn execute_music_import_task_wrapper(
    axum::extract::State((pool, _)): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    path: axum::extract::Path<uuid::Uuid>,
) -> Response {
    music::execute_import_task(axum::extract::State(pool), headers, path).await
}

pub fn create_router(pool: DbPool) -> Router {
    // Initialize WebAuthn config
    let rp_id = std::env::var("WEBAUTHN_RP_ID").unwrap_or_else(|_| "localhost".to_string());
    let rp_origin = std::env::var("WEBAUTHN_RP_ORIGIN")
        .unwrap_or_else(|_| "https://localhost:5173".to_string());

    let webauthn_config = Arc::new(
        webauthn::WebAuthnConfig::new(&rp_id, &rp_origin)
            .expect("Failed to create WebAuthn config"),
    );

    let state: AppState = (pool, webauthn_config);

    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/api/auth/native", post(auth_native_wrapper))
        .route("/api/auth/tokens/refresh", post(auth_refresh_wrapper))
        .route("/api/auth/tokens/validate", get(auth_validate_wrapper))
        .route("/api/auth/sessions/validate", get(session_validate_wrapper))
        .route("/api/auth/sessions/logout", post(session_logout_wrapper))
        .route(
            "/api/attestation/challenge",
            post(attestation_challenge_wrapper),
        )
        .route("/api/attestation/verify", post(attestation_verify_wrapper))
        .route("/api/attestation/assert", post(attestation_assert_wrapper))
        // WebAuthn routes
        .route(
            "/api/webauthn/register/begin",
            post(webauthn::register_begin),
        )
        .route(
            "/api/webauthn/register/verify",
            post(webauthn::register_verify),
        )
        .route(
            "/api/webauthn/authenticate/discoverable",
            post(webauthn::authenticate_begin),
        )
        .route(
            "/api/webauthn/authenticate/discoverable/verify",
            post(webauthn::authenticate_verify),
        )
        // WebAuthn challenge management
        .route(
            "/api/webauthn/challenges",
            post(webauthn::create_challenge_handler),
        )
        .route(
            "/api/webauthn/challenges/{id}",
            get(webauthn::get_challenge_handler),
        )
        .route(
            "/api/webauthn/challenges/{id}",
            delete(webauthn::delete_challenge_handler),
        )
        // WebAuthn credential management
        .route(
            "/api/webauthn/credentials",
            post(webauthn::create_credential_handler),
        )
        .route(
            "/api/webauthn/credentials/{id}",
            get(webauthn::get_credential_handler),
        )
        .route(
            "/api/webauthn/credentials/{id}",
            delete(webauthn::delete_credential_handler),
        )
        .route(
            "/api/webauthn/credentials/{id}/counter",
            patch(webauthn::update_credential_counter_handler),
        )
        .route(
            "/api/webauthn/credentials/{id}/name",
            patch(webauthn::rename_credential_handler),
        )
        .route(
            "/api/webauthn/users/{user_id}/credentials",
            get(webauthn::get_user_credentials_handler),
        )
        // Existing routes
        .route(
            "/v1/links",
            get(list_links_wrapper).post(create_link_wrapper),
        )
        .route(
            "/v1/links/{id}",
            patch(update_link_wrapper).delete(delete_link_wrapper),
        )
        .route("/v1/users/me", get(me_wrapper))
        .route(
            "/v1/user/prefs",
            get(get_user_prefs_wrapper).put(update_user_prefs_wrapper),
        )
        .route("/v1/models", get(models::list_models))
        .route("/v1/music/connections", get(list_music_connections_wrapper))
        .route(
            "/v1/music/connections/{provider}/authorize",
            post(begin_music_connection_wrapper),
        )
        .route(
            "/v1/music/connections/{provider}/callback",
            get(music_connection_callback_wrapper),
        )
        .route(
            "/v1/music/connections/{provider}",
            delete(disconnect_music_connection_wrapper),
        )
        .route(
            "/v1/music/imports",
            get(list_music_imports_wrapper).post(create_music_import_wrapper),
        )
        .route(
            "/v1/music/imports/{id}/restart",
            post(restart_music_import_wrapper),
        )
        .route(
            "/v1/music/imports/{id}/unmatched-tracks",
            get(list_music_unmatched_tracks_wrapper),
        )
        .route(
            "/v1/music/imports/{id}/activity",
            get(list_music_import_activity_wrapper),
        )
        .route(
            "/v1/music/imports/{id}",
            delete(delete_music_import_wrapper),
        )
        .route(
            "/internal/music-imports/{id}/execute",
            post(execute_music_import_task_wrapper),
        )
        .route("/v1/tags", get(list_tags_wrapper))
        .route(
            "/v1/links/{id}/summary",
            get(latest_summary_for_link_wrapper),
        )
        .route("/v1/links/{id}/tags", post(set_tags_for_link_wrapper))
        .layer(middleware::from_fn(auth::csrf_cookie_guard))
        .layer(cors_layer())
        .with_state(state)
}

fn cors_layer() -> CorsLayer {
    let allowed = auth::allowed_origins();
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin, _| {
            origin
                .to_str()
                .is_ok_and(|origin| allowed.iter().any(|allowed| allowed == origin))
        }))
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            CONTENT_TYPE,
            AUTHORIZATION,
            HeaderName::from_static("x-csrf-token"),
            HeaderName::from_static("x-trace-id"),
            HeaderName::from_static("x-user-id"),
            HeaderName::from_static("x-request-challenge"),
            HeaderName::from_static("x-ios-development"),
            HeaderName::from_static("x-ios-bundle-id"),
            HeaderName::from_static("x-ios-simulator-id"),
            HeaderName::from_static("x-ios-app-version"),
            HeaderName::from_static("x-ios-app-attest-key"),
            HeaderName::from_static("x-ios-app-attest-assertion"),
            HeaderName::from_static("x-ios-app-bundle-id"),
            HeaderName::from_static("x-app-attestation-mode"),
        ])
}

async fn health() -> impl IntoResponse {
    "ok"
}

async fn ready() -> impl IntoResponse {
    "ready"
}

// Auth handlers moved to src/auth.rs
