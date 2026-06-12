#![forbid(unsafe_code)]
// Temporarily relax missing_docs while the module surface is in flux during migration/tests.
// Once documentation is added, re‑enable with #![deny(missing_docs)]
//! Library entry point for the `hamrah-server` crate.
//!
//! This file exposes internal modules and selected public APIs so that
//! integration and unit tests (and potential future workspace crates)
//! can import functionality via `use hamrah_server::...` rather than
//! relying on `main.rs`.
//!
//! The crate name is derived from the package name `hamrah-server`
//! (hyphens become underscores), hence tests should use
//! `use hamrah_server::db::create_session;` etc.
//!
//! Keep re‑exports intentionally curated—avoid exporting everything
//! blindly to preserve encapsulation. Add new items when tests or
//! external consumers legitimately need them.

// ---------------------------------------------------------------------------
// Module Declarations
// ---------------------------------------------------------------------------

pub mod attestation;
pub mod auth;
pub mod db;
pub mod links;
pub mod routes;
pub mod summaries;
pub mod tags;
pub mod users;
pub mod webauthn;

// ---------------------------------------------------------------------------
// Re-exports: Database Layer
// ---------------------------------------------------------------------------

pub use db::{
    DbPool, Session, Summary, Tag, User, create_session, get_session_by_token,
    get_summary_for_link, get_user_by_id, init_pool, list_tags_for_user, purge_expired_sessions,
    rotate_session, run_migrations, set_link_tags, upsert_tag, upsert_user,
};

// ---------------------------------------------------------------------------
// Re-exports: WebAuthn Types & Flows
// (Label / flow_id removed as per recent contract simplification.)
// ---------------------------------------------------------------------------

pub use webauthn::{
    AuthenticateBeginRequest, AuthenticateBeginResponse, AuthenticateVerifyRequest,
    AuthenticateVerifyResponse, RegisterBeginRequest, RegisterBeginResponse, RegisterVerifyRequest,
    RegisterVerifyResponse, WebAuthnChallenge, WebAuthnConfig, WebAuthnCredential,
    authenticate_begin, authenticate_verify, create_challenge, delete_challenge, get_challenge,
    register_begin, register_verify,
};

// ---------------------------------------------------------------------------
// Re-exports: Auth (token/session abstractions) & Attestation (if needed)
// Add more when tests require them.
// ---------------------------------------------------------------------------

pub use attestation::*;
pub use auth::*;

// ---------------------------------------------------------------------------
// Test Utilities (Optional Future Section)
// If you introduce internal helpers purely for testing, expose them
// under a cfg(test) gated module.
// ---------------------------------------------------------------------------

#[cfg(test)]
pub mod test_support {
    //! Helpers available only in test builds.
    use super::*;

    /// Initialize a pooled database and run migrations; returns `None`
    /// if `DATABASE_URL` is not set (allows graceful skipping).
    pub async fn init_db_for_tests() -> anyhow::Result<Option<DbPool>> {
        if std::env::var("DATABASE_URL").is_err() {
            eprintln!("Skipping DB-dependent test: DATABASE_URL not set");
            return Ok(None);
        }
        let pool = init_pool().await?;
        run_migrations(&pool).await?;
        Ok(Some(pool))
    }
}
