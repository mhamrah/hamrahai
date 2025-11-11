#![allow(unused)]
// Basic WebAuthn "register begin" test skeleton.
//
// This is an integration-test scaffold that demonstrates how to call the
// register-begin endpoint. It is marked #[ignore] because it requires a
// running hamrah-api instance and environment configuration.
//
// To run manually:
// HAMRAH_API_BASE=http://localhost:8080 \
// TEST_USER_ID=<uuid> \
// TEST_USER_EMAIL=webauthn-test@example.com \
// cargo test -p hamrah-server --test webauthn_basic -- --ignored
//
// Notes:
// - In production, the API is at https://api.hamrah.app
// - This test does not validate cookies/CSRF flows; it only exercises the
//   "begin registration" endpoint returning WebAuthn PublicKeyCredentialCreationOptions.

use anyhow::Result;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires running API and environment configuration"]
async fn webauthn_register_begin_skeleton() -> Result<()> {
    // Base URL for the API under test
    let api_base =
        std::env::var("HAMRAH_API_BASE").unwrap_or_else(|_| "http://localhost:8080".to_string());

    // A user identifier to associate with the registration
    let user_id = std::env::var("TEST_USER_ID")
        .ok()
        .and_then(|s| Uuid::try_parse(&s).ok())
        .unwrap_or_else(|| Uuid::new_v4());

    let email = std::env::var("TEST_USER_EMAIL")
        .unwrap_or_else(|_| "webauthn-test@example.com".to_string());

    // Request body mirrors the expected wire contract (snake_case)
    // See packages/shared/src/dto.ts RegisterBeginRequest (label/flow_id removed)
    let body = json!({
        "user_id": user_id.to_string(),
        "email": email,
        "display_name": "WebAuthn Test",
    });

    let client = reqwest::Client::builder()
        .user_agent("hamrah-webauthn-test/0.1")
        .build()?;

    let url = format!("{}/api/webauthn/register/begin", api_base);

    // Optional: include Origin header to simulate browser CORS context
    // For localhost you can remove this; for production set to https://hamrah.app
    let origin =
        std::env::var("TEST_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".to_string());

    let resp = client
        .post(&url)
        .header("Origin", origin)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    // Expect 200 OK with JSON body
    assert!(
        resp.status().is_success(),
        "register begin HTTP status was not success: {}",
        resp.status()
    );

    let json: serde_json::Value = resp.json().await?;
    let success = json
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    assert!(success, "register begin did not return success: {}", json);

    // Optionally validate that options were returned
    assert!(
        json.get("options").is_some(),
        "register begin response missing 'options': {}",
        json
    );

    // Optionally validate presence of challenge_id if your API includes it
    // assert!(
    //     json.get("challenge_id").and_then(|v| v.as_str()).is_some(),
    //     "missing challenge_id in response: {}",
    //     json
    // );

    Ok(())
}
