#![allow(unused)]

use axum::{
    Router,
    body::{self, Body},
    http::{Request, StatusCode, header::SET_COOKIE},
};
use hamrah_server::{
    db::{DbPool, create_session, get_session_by_token, init_pool, run_migrations, upsert_user},
    routes::create_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use tower::util::ServiceExt; // for .oneshot()
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
struct ValidateResponse {
    valid: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct TokensResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

#[derive(Debug, Deserialize, Serialize)]
struct ModelsResponse {
    models: Vec<ModelResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ModelResponse {
    id: String,
    display_name: String,
    provider: String,
    status: String,
    replacement_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct UserPrefsResponse {
    default_model: String,
    preferred_models: Vec<String>,
    last_updated_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct SessionValidateResponse {
    success: bool,
    user: Option<serde_json::Value>,
    expires_at: Option<String>,
    error: Option<String>,
}

async fn setup_router() -> anyhow::Result<Option<(DbPool, Router)>> {
    if env::var("DATABASE_URL").is_err() {
        eprintln!("Skipping HTTP auth endpoint tests: DATABASE_URL not set");
        return Ok(None);
    }
    if env::var("JWT_SECRET").is_err() {
        unsafe {
            env::set_var("JWT_SECRET", "test-jwt-secret-for-http-auth-tests");
        }
    }
    let pool = init_pool().await?;
    run_migrations(&pool).await?;
    let router = create_router(pool.clone());
    Ok(Some((pool, router)))
}

#[tokio::test]
async fn http_validate_without_token_returns_false() -> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };

    let req = Request::builder()
        .method("GET")
        .uri("/api/auth/tokens/validate")
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let parsed: ValidateResponse = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(parsed.valid, false);

    Ok(())
}

#[tokio::test]
async fn http_refresh_with_valid_session_issues_new_tokens() -> anyhow::Result<()> {
    let Some((pool, router)) = setup_router().await? else {
        return Ok(());
    };

    // Arrange: ensure a user and a valid session exist
    let user_email = format!("http-refresh-{}@example.com", Uuid::new_v4());
    let user = upsert_user(&pool, &user_email, Some("HTTP Refresh Test")).await?;

    let original_raw_refresh = Uuid::new_v4().to_string();
    let _session = create_session(&pool, user.id, &original_raw_refresh, 6).await?;

    // Act: POST /api/auth/tokens/refresh with the raw refresh token
    let payload = json!({ "refresh_token": original_raw_refresh });
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/tokens/refresh")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();

    // Assert: success and proper shape
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "refresh endpoint should return 200 OK"
    );

    let body_bytes = body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let parsed: TokensResponse = serde_json::from_slice(&body_bytes).unwrap();

    assert!(
        !parsed.access_token.is_empty(),
        "access_token must be present and non-empty"
    );
    assert!(
        !parsed.refresh_token.is_empty(),
        "refresh_token must be present and non-empty"
    );
    assert!(
        parsed.expires_in > 0,
        "expires_in should be a positive duration (seconds)"
    );

    Ok(())
}

#[tokio::test]
async fn http_session_validate_with_valid_cookie_returns_user() -> anyhow::Result<()> {
    let Some((pool, router)) = setup_router().await? else {
        return Ok(());
    };

    let user_email = format!("http-session-{}@example.com", Uuid::new_v4());
    let user = upsert_user(&pool, &user_email, Some("HTTP Session Test")).await?;
    let raw_session = Uuid::new_v4().to_string();
    let _session = create_session(&pool, user.id, &raw_session, 6).await?;

    let req = Request::builder()
        .method("GET")
        .uri("/api/auth/sessions/validate")
        .header("cookie", format!("session={raw_session}"))
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let parsed: SessionValidateResponse = serde_json::from_slice(&body_bytes).unwrap();

    assert!(parsed.success);
    assert_eq!(
        parsed
            .user
            .as_ref()
            .and_then(|user| user.get("email"))
            .and_then(|email| email.as_str()),
        Some(user_email.as_str())
    );
    assert!(parsed.expires_at.is_some());

    Ok(())
}

#[tokio::test]
async fn http_session_validate_without_cookie_returns_unauthorized() -> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };

    let req = Request::builder()
        .method("GET")
        .uri("/api/auth/sessions/validate")
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let body_bytes = body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let parsed: SessionValidateResponse = serde_json::from_slice(&body_bytes).unwrap();
    assert!(!parsed.success);
    assert_eq!(parsed.error.as_deref(), Some("missing_session"));

    Ok(())
}

#[tokio::test]
async fn http_session_logout_deletes_session_and_clears_cookies() -> anyhow::Result<()> {
    let Some((pool, router)) = setup_router().await? else {
        return Ok(());
    };

    let user_email = format!("http-logout-{}@example.com", Uuid::new_v4());
    let user = upsert_user(&pool, &user_email, Some("HTTP Logout Test")).await?;
    let raw_session = Uuid::new_v4().to_string();
    let _session = create_session(&pool, user.id, &raw_session, 6).await?;
    let csrf_token = Uuid::new_v4().to_string();

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/sessions/logout")
        .header("host", "localhost:8080")
        .header("origin", "http://localhost:5173")
        .header(
            "cookie",
            format!("session={raw_session}; csrf_token={csrf_token}"),
        )
        .header("x-csrf-token", csrf_token)
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let set_cookies: Vec<_> = resp.headers().get_all(SET_COOKIE).iter().collect();
    assert!(
        set_cookies
            .iter()
            .any(|cookie| cookie.to_str().unwrap_or_default().starts_with("session=;"))
    );
    assert!(set_cookies.iter().any(|cookie| {
        cookie
            .to_str()
            .unwrap_or_default()
            .starts_with("csrf_token=;")
    }));
    assert!(get_session_by_token(&pool, &raw_session).await?.is_none());

    Ok(())
}

#[tokio::test]
async fn http_native_google_accepts_id_token_and_legacy_credential_alias() -> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };

    unsafe {
        env::set_var("GOOGLE_AUTH_TEST_BYPASS", "true");
    }

    for token_field in ["id_token", "credential"] {
        let email = format!("native-google-{token_field}-{}@example.com", Uuid::new_v4());
        let payload = json!({
            "provider": "google",
            "platform": "ios",
            token_field: format!("test-google:{email}")
        });
        let req = Request::builder()
            .method("POST")
            .uri("/api/auth/native")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(parsed["success"], true);
        assert!(
            parsed["access_token"]
                .as_str()
                .is_some_and(|token| !token.is_empty())
        );
        assert!(
            parsed["refresh_token"]
                .as_str()
                .is_some_and(|token| !token.is_empty())
        );
        assert_eq!(parsed["user"]["email"], email);
        assert_eq!(parsed["user"]["provider"], "google");
    }

    Ok(())
}

#[tokio::test]
async fn http_native_google_rejects_missing_id_token() -> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };

    let payload = json!({
        "provider": "google",
        "platform": "ios"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/native")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let body_bytes = body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(parsed["success"], false);
    assert_eq!(parsed["error"], "Google ID token is missing");

    Ok(())
}

#[tokio::test]
async fn http_native_apple_accepts_identity_token_email_without_request_email() -> anyhow::Result<()>
{
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };

    unsafe {
        env::set_var("APPLE_AUTH_TEST_BYPASS", "true");
    }

    let email = format!("native-apple-{}@example.com", Uuid::new_v4());
    let payload = json!({
        "provider": "apple",
        "platform": "ios",
        "id_token": format!("test-apple:{email}"),
        "provider_id": "apple-user-123"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/native")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(parsed["success"], true);
    assert!(
        parsed["access_token"]
            .as_str()
            .is_some_and(|token| !token.is_empty())
    );
    assert_eq!(parsed["user"]["email"], email);
    assert_eq!(parsed["user"]["provider"], "apple");
    assert_eq!(parsed["user"]["provider_id"], "apple-user-123");

    Ok(())
}

#[tokio::test]
async fn http_native_apple_rejects_missing_identity_token() -> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };

    let payload = json!({
        "provider": "apple",
        "platform": "ios"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/native")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let body_bytes = body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(parsed["success"], false);
    assert_eq!(parsed["error"], "Apple identity token is missing");

    Ok(())
}

#[tokio::test]
async fn http_models_returns_backend_catalog_with_deprecated_replacements() -> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };

    let req = Request::builder()
        .method("GET")
        .uri("/v1/models")
        .body(Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let parsed: ModelsResponse = serde_json::from_slice(&body_bytes).unwrap();

    assert!(
        parsed.models.iter().any(|model| {
            model.id == "@cf/zai-org/glm-4.7-flash" && model.status == "available"
        })
    );
    assert!(
        parsed
            .models
            .iter()
            .any(|model| model.status == "deprecated" && model.replacement_id.is_some())
    );

    Ok(())
}

#[tokio::test]
async fn http_user_prefs_round_trips_available_models_and_prunes_unavailable_preferred()
-> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };

    unsafe {
        env::set_var("GOOGLE_AUTH_TEST_BYPASS", "true");
    }

    let email = format!("prefs-{}@example.com", Uuid::new_v4());
    let login_payload = json!({
        "provider": "google",
        "platform": "ios",
        "id_token": format!("test-google:{email}")
    });
    let login_req = Request::builder()
        .method("POST")
        .uri("/api/auth/native")
        .header("content-type", "application/json")
        .body(Body::from(login_payload.to_string()))
        .unwrap();
    let login_resp = router.clone().oneshot(login_req).await.unwrap();
    assert_eq!(login_resp.status(), StatusCode::OK);
    let login_body = body::to_bytes(login_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let tokens: TokensResponse = serde_json::from_slice(&login_body).unwrap();

    let get_req = Request::builder()
        .method("GET")
        .uri("/v1/user/prefs")
        .header("authorization", format!("Bearer {}", tokens.access_token))
        .body(Body::empty())
        .unwrap();
    let get_resp = router.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_body = body::to_bytes(get_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let defaults: UserPrefsResponse = serde_json::from_slice(&get_body).unwrap();
    assert_eq!(defaults.default_model, "@cf/zai-org/glm-4.7-flash");
    assert!(defaults.preferred_models.is_empty());

    let update_payload = json!({
        "default_model": "@cf/google/gemma-4-26b-a4b-it",
        "preferred_models": [
            "@cf/zai-org/glm-4.7-flash",
            "gpt-4o-mini",
            "unknown-model"
        ]
    });
    let put_req = Request::builder()
        .method("PUT")
        .uri("/v1/user/prefs")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", tokens.access_token))
        .body(Body::from(update_payload.to_string()))
        .unwrap();
    let put_resp = router.clone().oneshot(put_req).await.unwrap();
    assert_eq!(put_resp.status(), StatusCode::OK);
    let put_body = body::to_bytes(put_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let updated: UserPrefsResponse = serde_json::from_slice(&put_body).unwrap();

    assert_eq!(updated.default_model, "@cf/google/gemma-4-26b-a4b-it");
    assert_eq!(updated.preferred_models, vec!["@cf/zai-org/glm-4.7-flash"]);

    Ok(())
}
