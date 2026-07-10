//! Cloudflare Worker backend for the demo's Cloudflare-SPA deployment.
//!
//! Cloudflare serves the static Dioxus bundle directly and only invokes this
//! Worker for `/api/*` (see `wrangler.toml`'s `run_worker_first`). The one route
//! reimplements the "Server validation" backend in plain Axum, since standard
//! Dioxus fullstack (server functions) does not compile for Workers. It shares
//! [`crate::signup::evaluate_signup`] with the native server function, so the
//! two backends can never drift.

use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde::Serialize;
use tower_service::Service;
use worker::{Context, Env, HttpRequest, Result, event};

use crate::signup::{SignupRejection, SignupRequest, evaluate_signup};

/// Wire mirror of `dioform_fullstack::ServerSubmitOutcome<SignupRejection>`:
/// it serializes to the identical JSON the native `#[server]` function returns,
/// so the SPA client decodes both the same way, without pulling the fullstack
/// adapter (and its Tokio networking paths) into the Worker build.
#[derive(Serialize)]
enum SignupOutcome {
    Accepted,
    Rejected(SignupRejection),
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> Result<axum::http::Response<axum::body::Body>> {
    // Everything that isn't an API call is a static asset (SPA fallback).
    if !req.uri().path().starts_with("/api/") {
        return asset_response(req, &env).await;
    }

    let mut router = router();
    Ok(router.call(req).await?)
}

fn router() -> Router {
    Router::new().route("/api/check_signup", post(check_signup))
}

/// `/api/check_signup`: the SPA counterpart of the native server function.
async fn check_signup(Json(request): Json<SignupRequest>) -> impl IntoResponse {
    let outcome = match evaluate_signup(&request.email) {
        Some(rejection) => SignupOutcome::Rejected(rejection),
        None => SignupOutcome::Accepted,
    };
    Json(outcome)
}

/// Hand every non-API request to Cloudflare's static asset binding.
async fn asset_response(
    req: HttpRequest,
    env: &Env,
) -> Result<axum::http::Response<axum::body::Body>> {
    let response = env.assets("ASSETS")?.fetch_request(req).await?;
    let (parts, body) = response.into_parts();
    Ok(axum::http::Response::from_parts(
        parts,
        axum::body::Body::new(body),
    ))
}
