//! Cloudflare Worker library entry.
//!
//! The docs-by-example UI (router, pages, live examples) lives in `main.rs` and
//! is compiled for the `web`/`server` targets. This `cdylib` half is only the
//! Cloudflare Worker backend: it renders no pages, so it pulls in none of the
//! Dioxus rendering stack, just the plain Axum `/api/*` routes.

#![cfg_attr(feature = "worker", allow(clippy::unused_async))]

#[cfg(feature = "worker")]
mod signup;
#[cfg(feature = "worker")]
mod worker;
