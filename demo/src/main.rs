//! dioform demo: binary entry point (native fullstack server + wasm SPA
//! client).
//!
//! Every page mounts a real feature *and* renders that feature's own source
//! (via the compile-time `code!` macro), so the snippet you read is exactly the
//! code that runs. The UI lives in [`app`] (router + shell), [`pages`] (one
//! route each), and [`examples`] (the small, pure components the pages both
//! mount and quote).
//!
//! `dioxus::launch` runs both halves: on the wasm client it hydrates the app,
//! on the Axum server it serves the app plus every registered `#[server]`
//! function. The Cloudflare Worker backend is a separate `cdylib`, see
//! `lib.rs`/`worker.rs`, and none of these page modules are compiled for it.

#[cfg(any(feature = "web", feature = "server"))]
mod app;
#[cfg(any(feature = "web", feature = "server"))]
mod examples;
#[cfg(any(feature = "web", feature = "server"))]
mod pages;
#[cfg(any(feature = "web", feature = "server"))]
mod server_api;
#[cfg(any(feature = "web", feature = "server"))]
mod signup;
#[cfg(any(feature = "web", feature = "server"))]
mod ui;

#[cfg(any(feature = "web", feature = "server"))]
fn main() {
    dioxus::launch(app::App);
}

// The Worker build (`--no-default-features --features worker`) compiles the
// `cdylib` in `lib.rs`; the binary is an empty stub so `cargo` still has a
// `main` to check.
#[cfg(not(any(feature = "web", feature = "server")))]
fn main() {}
