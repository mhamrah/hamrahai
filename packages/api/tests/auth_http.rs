#![allow(unused)]

use axum::{
    Router,
    body::{self, Body},
    http::{Request, StatusCode},
};
use hamrah_server::{
    db::{DbPool, create_session, init_pool, run_migrations, upsert_user},
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

async fn setup_router() -> anyhow::Result<Option<(DbPool, Router)>> {
    if env::var("DATABASE_URL").is_err() {
        eprintln!("Skipping HTTP auth endpoint tests: DATABASE_URL not set");
        return Ok(None);
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
