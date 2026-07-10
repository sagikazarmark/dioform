# Dioform: Demo

A docs-by-example gallery for [`dioform`](https://github.com/sagikazarmark/dioform).
Every page mounts a real feature next to the **exact source that runs it** (rendered with
the compile-time `code!` macro from [`dioxus-code`](https://crates.io/crates/dioxus-code)),
so the snippet you read is guaranteed to be the code you see running.

## Structure

The app mirrors the structure of the [`dioxus-clerk`](https://github.com/sagikazarmark/dioxus-clerk)
demo:

- `src/app.rs`: the `Route` enum (dioxus-router) and the shared shell (header, grouped
  sidebar, mobile nav, footer).
- `src/pages/`: one route component per page: prose, a docs link, and the example's source
  (via `code!`). Grouped by nav section (`basics`, `validation`, `fields`, `submission`,
  `server`, `forms`).
- `src/examples/`: small, pure feature components. Each keeps the `dioform` API front and
  center; layout chrome lives in `src/ui.rs`.
- `src/server_api.rs`: the target-aware backend for the "Server validation" page, a Dioxus
  `#[server]` function on the native build, or a `fetch` to the Worker's `/api/*` route on the
  Cloudflare-SPA build.
- `src/signup.rs`: the pure "is this email taken?" rule, shared by the server function and the
  Worker route (serde-only, so it compiles for every target).
- `src/worker.rs` / `src/lib.rs`: the Cloudflare Worker entry (`cdylib`); serves the static
  bundle and the `/api/*` routes.
- `src/ui.rs`: presentation-only helpers (page headers, the live/source example card, shared
  field inputs for the realistic forms).

## Build targets

The crate builds for four targets via Cargo features (mirroring the `dioxus-clerk` demo):

| Feature | What it is |
| --- | --- |
| `web` | Cloudflare-SPA client; reaches the backend by fetching the Worker's `/api/*` routes. |
| `fullstack-web` | Native fullstack client; calls Dioxus server functions directly. |
| `server` | Native fullstack server (SSR + server functions). |
| `worker` | Cloudflare Worker `cdylib`; static assets plus the `/api/*` routes. |

Cloudflare Workers can't run Dioxus fullstack (server functions pull in Tokio networking paths
that don't compile for Workers), so the Worker reimplements the one server-backed example as a
plain Axum route. `src/signup.rs` keeps the actual rule in one place.

## What it covers

**Basics**: minimal form, all field bindings (text/textarea/checkbox/select/radio), parsed
inputs (number/money).

**Validation**: validation modes & triggers, field & form (cross-field) validators, the
whole-form error summary, async & debounced validation, and the `garde` validation adapter.

**Fields & state**: collections (append/insert/remove/move/swap/replace/clear), collection item
validation, file fields, nested structs & composed paths with field-name overrides, reusable
field groups, dirty/touched/blurred/pristine meta with reset/reset_field/reinitialize, listeners
& dependent-field resets, and full form-state snapshot capture/restore.

**Submission**: submit intents (save draft vs publish), managed/browser/progressive submission,
and structured submit errors with stale-error clearing.

**Server**: fullstack server-function validation via `dioform-fullstack`.

**Realistic forms**: signup, checkout (conditional shipping), invoice (repeatable line items),
and a nested project planner, combining several features per page.

## Run it (native fullstack)

The app runs with the [Dioxus CLI](https://dioxuslabs.com/learn/0.7/getting_started/):

```sh
cargo install dioxus-cli               # if needed
npm install                            # once, for the Tailwind toolchain
npm run build                          # compile assets/style.css (or: npm run watch)
dx serve --fullstack --features fullstack-web
```

`--features fullstack-web` is required: the plain `web` feature is the Cloudflare-SPA client
(its backend calls target the Worker), while `fullstack-web` calls the local server functions.
`assets/style.css` is generated from `style.css` (Tailwind + daisyUI) and is git-ignored, so run
`npm run build` before the first `dx serve` and after editing RSX classes (`npm run watch`
rebuilds on change).

## Run it with Dagger

[Dagger](https://dagger.io) builds and runs everything in containers: no local Node, `dx`, or
Wrangler needed:

```sh
dagger check                # release builds of BOTH the native fullstack app and the Worker
dagger call serve up        # native fullstack, tunnelled to a local port
dagger call worker dev up   # Cloudflare Worker via `wrangler dev`
```

To deploy the Worker, pass the Cloudflare credentials explicitly:

```sh
dagger call worker deploy \
  --account-id "$CLOUDFLARE_ACCOUNT_ID" \
  --api-token env://CLOUDFLARE_API_TOKEN
```

CI deploys automatically ([`deploy.yaml`](../.github/workflows/deploy.yaml)): pushes to `main`
roll out to production, and pull requests upload a preview version (its URLs posted as a PR
comment). Both jobs need `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_API_TOKEN` repository secrets;
preview only runs for same-repo PRs, since fork PRs can't read the secrets.

## Verify

```sh
# Cloudflare-SPA client.
cargo check --no-default-features --features web --target wasm32-unknown-unknown
# Native fullstack client.
cargo check --no-default-features --features fullstack-web --target wasm32-unknown-unknown
# Native server.
cargo check --features server
# Cloudflare Worker.
cargo check --no-default-features --features worker --target wasm32-unknown-unknown
```
