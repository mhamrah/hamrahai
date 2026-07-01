#![allow(unused)]

use axum::{
    Router,
    body::{self, Body},
    http::{Request, StatusCode},
};
use base64::Engine;
use hamrah_server::{
    db::{DbPool, init_pool, run_migrations},
    routes::create_router,
};
use serde::Deserialize;
use serde_json::json;
use std::env;
use tower::util::ServiceExt;
use uuid::Uuid;

#[derive(Deserialize)]
struct AttestationChallengeResponse {
    success: bool,
    challenge: Option<String>,
    challenge_id: String,
    error: Option<String>,
}

#[derive(Deserialize)]
struct ErrorResponse {
    success: bool,
    error: Option<String>,
}

#[derive(Deserialize)]
struct TokensResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct CreateLinkResponse {
    id: String,
    canonical_url: String,
}

#[derive(Deserialize)]
struct LinkMutationResponse {
    success: bool,
    link: ServerLink,
}

#[derive(Deserialize)]
struct DeleteLinkResponse {
    success: bool,
}

#[derive(Deserialize)]
struct LinkDeltaResponse {
    links: Vec<ServerLink>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct ServerLink {
    id: String,
    original_url: String,
    canonical_url: String,
    title: Option<String>,
    status: String,
}

async fn setup_router() -> anyhow::Result<Option<(DbPool, Router)>> {
    if env::var("DATABASE_URL").is_err() {
        eprintln!("Skipping attestation/link tests: DATABASE_URL not set");
        return Ok(None);
    }
    if env::var("JWT_SECRET").is_err() {
        unsafe {
            env::set_var("JWT_SECRET", "test-jwt-secret-for-attestation-link-tests");
        }
    }
    let pool = init_pool().await?;
    run_migrations(&pool).await?;
    let router = create_router(pool.clone());
    Ok(Some((pool, router)))
}

async fn login(router: &Router) -> String {
    unsafe {
        env::set_var("GOOGLE_AUTH_TEST_BYPASS", "true");
    }

    let email = format!("links-{}@example.com", Uuid::new_v4());
    let payload = json!({
        "provider": "google",
        "platform": "ios",
        "id_token": format!("test-google:{email}")
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
    let tokens: TokensResponse = serde_json::from_slice(&body_bytes).unwrap();
    tokens.access_token
}

#[tokio::test]
async fn attestation_verify_fetches_uuid_challenge_without_database_type_error()
-> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };
    unsafe {
        env::set_var("APPLE_TEAM_ID", "TEAMID1234");
    }

    let challenge_payload = json!({
        "platform": "ios",
        "bundle_id": "app.hamrah.ios"
    });
    let challenge_req = Request::builder()
        .method("POST")
        .uri("/api/attestation/challenge")
        .header("content-type", "application/json")
        .body(Body::from(challenge_payload.to_string()))
        .unwrap();
    let challenge_resp = router.clone().oneshot(challenge_req).await.unwrap();
    assert_eq!(challenge_resp.status(), StatusCode::OK);
    let challenge_body = body::to_bytes(challenge_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let challenge: AttestationChallengeResponse = serde_json::from_slice(&challenge_body).unwrap();
    assert!(challenge.success, "{:?}", challenge.error);

    let verify_payload = json!({
        "challenge_id": challenge.challenge_id,
        "key_id": "test-key-id",
        "attestation_object": "not-valid-base64",
        "bundle_id": "app.hamrah.ios"
    });
    let verify_req = Request::builder()
        .method("POST")
        .uri("/api/attestation/verify")
        .header("content-type", "application/json")
        .body(Body::from(verify_payload.to_string()))
        .unwrap();
    let verify_resp = router.clone().oneshot(verify_req).await.unwrap();
    assert_eq!(verify_resp.status(), StatusCode::BAD_REQUEST);
    let verify_body = body::to_bytes(verify_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let parsed: ErrorResponse = serde_json::from_slice(&verify_body).unwrap();
    assert_eq!(parsed.error.as_deref(), Some("Invalid attestation object"));

    Ok(())
}

#[tokio::test]
async fn attestation_challenge_response_decodes_to_stored_verifier_challenge() -> anyhow::Result<()>
{
    let Some((pool, router)) = setup_router().await? else {
        return Ok(());
    };

    let challenge_payload = json!({
        "platform": "ios",
        "bundle_id": "app.hamrah.ios"
    });
    let challenge_req = Request::builder()
        .method("POST")
        .uri("/api/attestation/challenge")
        .header("content-type", "application/json")
        .body(Body::from(challenge_payload.to_string()))
        .unwrap();
    let challenge_resp = router.clone().oneshot(challenge_req).await.unwrap();
    assert_eq!(challenge_resp.status(), StatusCode::OK);
    let challenge_body = body::to_bytes(challenge_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let challenge: AttestationChallengeResponse = serde_json::from_slice(&challenge_body).unwrap();
    assert!(challenge.success, "{:?}", challenge.error);

    let stored_challenge: String =
        sqlx::query_scalar("SELECT challenge FROM app_attest_challenges WHERE id = $1")
            .bind(Uuid::parse_str(&challenge.challenge_id)?)
            .fetch_one(&pool)
            .await?;
    let response_client_data = base64::engine::general_purpose::STANDARD
        .decode(challenge.challenge.expect("challenge should be present"))
        .expect("challenge should be base64-encoded client data");

    assert_eq!(response_client_data, stored_challenge.as_bytes());
    assert!(stored_challenge.is_ascii());

    Ok(())
}

#[tokio::test]
async fn attestation_verify_rejects_malformed_challenge_id_without_database_error()
-> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };

    let verify_payload = json!({
        "challenge_id": "not-a-uuid",
        "key_id": "test-key-id",
        "attestation_object": "not-valid-base64",
        "bundle_id": "app.hamrah.ios"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/attestation/verify")
        .header("content-type", "application/json")
        .body(Body::from(verify_payload.to_string()))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body_bytes = body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let parsed: ErrorResponse = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(parsed.error.as_deref(), Some("Invalid challenge"));

    Ok(())
}

#[tokio::test]
async fn protected_native_link_routes_reject_missing_attestation_headers() -> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };
    let access_token = login(&router).await;

    let req = Request::builder()
        .method("GET")
        .uri("/v1/links?since=&limit=100")
        .header("authorization", format!("Bearer {access_token}"))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body_bytes = body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let parsed: ErrorResponse = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(parsed.error.as_deref(), Some("Missing App Attest key"));

    Ok(())
}

#[tokio::test]
async fn native_link_create_and_delta_list_round_trip_with_explicit_test_attestation_bypass()
-> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };
    unsafe {
        env::set_var("ALLOW_UNATTESTED_IOS_REQUESTS", "true");
    }
    let access_token = login(&router).await;

    let url = format!("https://example.com/article/{}", Uuid::new_v4());
    let create_payload = json!({
        "client_id": Uuid::new_v4().to_string(),
        "url": url,
        "title": "A saved article",
        "source_app": "com.apple.mobilesafari",
        "shared_text": "Interesting excerpt",
        "shared_at": "2026-06-28T15:29:00Z"
    });
    let create_req = Request::builder()
        .method("POST")
        .uri("/v1/links")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-app-attestation-mode", "none")
        .body(Body::from(create_payload.to_string()))
        .unwrap();
    let create_resp = router.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let create_body = body::to_bytes(create_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let created: CreateLinkResponse = serde_json::from_slice(&create_body).unwrap();
    assert_eq!(created.canonical_url, url);

    let list_req = Request::builder()
        .method("GET")
        .uri("/v1/links?since=&limit=100")
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-app-attestation-mode", "none")
        .body(Body::empty())
        .unwrap();
    let list_resp = router.clone().oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = body::to_bytes(list_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let delta: LinkDeltaResponse = serde_json::from_slice(&list_body).unwrap();
    let saved = delta
        .links
        .iter()
        .find(|link| link.id == created.id)
        .expect("created link should be present in delta response");
    assert_eq!(saved.original_url, url);
    assert_eq!(saved.canonical_url, url);
    assert_eq!(saved.title.as_deref(), Some("A saved article"));
    assert_eq!(saved.status, "synced");

    Ok(())
}

#[tokio::test]
async fn native_link_archive_and_delete_are_persisted() -> anyhow::Result<()> {
    let Some((_pool, router)) = setup_router().await? else {
        return Ok(());
    };
    unsafe {
        env::set_var("ALLOW_UNATTESTED_IOS_REQUESTS", "true");
    }
    let access_token = login(&router).await;

    let url = format!("https://example.com/archive/{}", Uuid::new_v4());
    let create_payload = json!({
        "client_id": Uuid::new_v4().to_string(),
        "url": url
    });
    let create_req = Request::builder()
        .method("POST")
        .uri("/v1/links")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-app-attestation-mode", "none")
        .body(Body::from(create_payload.to_string()))
        .unwrap();
    let create_resp = router.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let create_body = body::to_bytes(create_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let created: CreateLinkResponse = serde_json::from_slice(&create_body).unwrap();

    let archive_payload = json!({ "status": "archived" });
    let archive_req = Request::builder()
        .method("PATCH")
        .uri(format!("/v1/links/{}", created.id))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-app-attestation-mode", "none")
        .body(Body::from(archive_payload.to_string()))
        .unwrap();
    let archive_resp = router.clone().oneshot(archive_req).await.unwrap();
    assert_eq!(archive_resp.status(), StatusCode::OK);
    let archive_body = body::to_bytes(archive_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let archived: LinkMutationResponse = serde_json::from_slice(&archive_body).unwrap();
    assert!(archived.success);
    assert_eq!(archived.link.id, created.id);
    assert_eq!(archived.link.status, "archived");

    let list_req = Request::builder()
        .method("GET")
        .uri("/v1/links?since=&limit=100")
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-app-attestation-mode", "none")
        .body(Body::empty())
        .unwrap();
    let list_resp = router.clone().oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = body::to_bytes(list_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let delta: LinkDeltaResponse = serde_json::from_slice(&list_body).unwrap();
    let listed = delta
        .links
        .iter()
        .find(|link| link.id == created.id)
        .expect("archived link should remain in delta");
    assert_eq!(listed.status, "archived");

    let delete_req = Request::builder()
        .method("DELETE")
        .uri(format!("/v1/links/{}", created.id))
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-app-attestation-mode", "none")
        .body(Body::empty())
        .unwrap();
    let delete_resp = router.clone().oneshot(delete_req).await.unwrap();
    assert_eq!(delete_resp.status(), StatusCode::OK);
    let delete_body = body::to_bytes(delete_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let deleted: DeleteLinkResponse = serde_json::from_slice(&delete_body).unwrap();
    assert!(deleted.success);

    let list_after_delete_req = Request::builder()
        .method("GET")
        .uri("/v1/links?since=&limit=100")
        .header("authorization", format!("Bearer {access_token}"))
        .header("x-app-attestation-mode", "none")
        .body(Body::empty())
        .unwrap();
    let list_after_delete_resp = router.clone().oneshot(list_after_delete_req).await.unwrap();
    assert_eq!(list_after_delete_resp.status(), StatusCode::OK);
    let list_after_delete_body = body::to_bytes(list_after_delete_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let delta_after_delete: LinkDeltaResponse =
        serde_json::from_slice(&list_after_delete_body).unwrap();
    let deleted_link = delta_after_delete
        .links
        .iter()
        .find(|link| link.id == created.id)
        .expect("deleted link tombstone should remain in delta");
    assert_eq!(deleted_link.status, "deleted");

    Ok(())
}
