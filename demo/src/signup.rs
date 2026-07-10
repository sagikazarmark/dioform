//! Pure signup-availability logic, shared by every build target.
//!
//! This module deliberately depends on nothing but `serde` so it compiles for
//! the page-rendering builds (`web`/`server`) *and* the Cloudflare Worker
//! (`worker`). The native server function ([`crate::server_api`]) and the Worker
//! route ([`crate::worker`]) both delegate here, so the "is this email taken?"
//! rule lives in exactly one place regardless of where it runs.

use serde::{Deserialize, Serialize};

/// Request body for the `/api/check_signup` route (the SPA → Worker call).
/// Unused by the native `server` build, which takes the email as a plain server
/// function argument.
#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignupRequest {
    pub email: String,
}

/// Application rejection returned by the server for an otherwise well-formed
/// submission. Mapped into a structured field submit error on the client.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SignupRejection {
    EmailTaken,
}

/// Server-side availability check. `taken@example.com` stands in for an address
/// that already exists in a real database; every other value is accepted.
/// Returns `Some(rejection)` when the email is taken, `None` when it is free.
/// Only the backends run it (`server` function body, Worker route); the SPA
/// client reaches those over the network instead.
#[allow(dead_code)]
pub fn evaluate_signup(email: &str) -> Option<SignupRejection> {
    if email.trim().eq_ignore_ascii_case("taken@example.com") {
        Some(SignupRejection::EmailTaken)
    } else {
        None
    }
}
