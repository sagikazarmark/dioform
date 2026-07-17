# dioform demo

A docs-by-example gallery for `dioform`.

## Build targets

The crate builds for four targets via Cargo features (mirroring the `dioxus-clerk` demo):

| Feature         | What it is                                                                           |
| --------------- | ------------------------------------------------------------------------------------ |
| `web`           | Cloudflare-SPA client; reaches the backend by fetching the Worker's `/api/*` routes. |
| `fullstack-web` | Native fullstack client; calls Dioxus server functions directly.                     |
| `server`        | Native fullstack server (SSR + server functions).                                    |
| `worker`        | Cloudflare Worker `cdylib`; static assets plus the `/api/*` routes.                  |

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

## Run locally

```sh
npm ci
npm run build                          # compile build/style.css (or: npm run watch)
dx serve --fullstack \
  @client --platform web --no-default-features --features fullstack-web \
  @server --platform server --no-default-features --features server
```

`--features fullstack-web` is required: the plain `web` feature is the Cloudflare-SPA client
(its backend calls target the Worker), while `fullstack-web` calls the local server functions.
`build/style.css` is generated from `src/style.css` (Tailwind + daisyUI) and is git-ignored, so run
`npm run build` before the first `dx serve` and after editing RSX classes (`npm run watch`
rebuilds on change).
