//! Client-facing backend surface for the "Server validation" page.
//!
//! The example component ([`crate::examples::server_validation`]) calls one
//! function, [`check_signup_call`], and stays identical across every target.
//! The per-target wiring lives here:
//!
//! - **native fullstack** (`fullstack-web` client / `server`): the call routes
//!   through the Dioxus `#[server]` function [`check_signup`], which runs the
//!   body on the Axum server.
//! - **Cloudflare-SPA** (`web` without `fullstack-web`): the call `fetch`es the
//!   Worker's plain `/api/check_signup` route (see [`crate::worker`]).
//!
//! Both paths share [`crate::signup::evaluate_signup`], so the rule is defined
//! once.

// The `#[server]` macro expands to `dioxus_fullstack::` paths, which dioxus
// re-exports through its prelude under the `fullstack` feature, so the glob
// import must be in scope wherever the macro is used.
use dioform_fullstack::ServerSubmitOutcome;
#[cfg(any(feature = "server", feature = "fullstack-web"))]
use dioxus::prelude::*;

use crate::signup::SignupRejection;

/// Dioxus server function backing the native fullstack build. On the client the
/// macro generates a network call; on the server it runs the body. Compiled
/// only for the fullstack targets; the Cloudflare-SPA build reaches the Worker
/// `/api` route instead.
#[cfg(any(feature = "server", feature = "fullstack-web"))]
#[server(endpoint = "check_signup")]
pub async fn check_signup(email: String) -> ServerFnResult<ServerSubmitOutcome<SignupRejection>> {
    Ok(match crate::signup::evaluate_signup(&email) {
        Some(rejection) => ServerSubmitOutcome::rejected(rejection),
        None => ServerSubmitOutcome::accepted(),
    })
}

/// Target-aware client call used by the example's `on_submit_server_fn`. Returns
/// the server outcome, or a transport-error string that the example maps onto a
/// form-level submit error.
#[cfg(any(feature = "server", feature = "fullstack-web"))]
pub async fn check_signup_call(
    email: String,
) -> Result<ServerSubmitOutcome<SignupRejection>, String> {
    check_signup(email).await.map_err(|error| error.to_string())
}

/// Cloudflare-SPA counterpart: POST the email to the Worker's `/api/check_signup`
/// route and decode the same `ServerSubmitOutcome` payload.
#[cfg(all(feature = "web", not(feature = "fullstack-web")))]
pub async fn check_signup_call(
    email: String,
) -> Result<ServerSubmitOutcome<SignupRejection>, String> {
    use crate::signup::SignupRequest;

    let response = gloo_net::http::Request::post("/api/check_signup")
        .json(&SignupRequest { email })
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.ok() {
        return Err(format!("server returned HTTP {}", response.status()));
    }
    response
        .json::<ServerSubmitOutcome<SignupRejection>>()
        .await
        .map_err(|error| error.to_string())
}
