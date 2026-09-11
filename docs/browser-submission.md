# Browser Submission Modes

Dioform supports three submit modes with different ownership boundaries.

**Dioxus-Managed Submission** uses `managed_submit()`. The Dioxus `onsubmit` handler calls `prevent_default()` and `stop_propagation()`, runs the typed submission lifecycle, and passes a **Submission Snapshot** to application submit behavior.

```rust
let submit = form.managed_submit();

form {
    onsubmit: move |event| {
        submit.on_submit(event, |snapshot| save(snapshot.into_value()));
    },
}
```

**Native Browser Submission** uses `browser_submit(action)`. It provides form attributes for browser-owned POST and does not attach a Dioxus submit handler. The browser serializes rendered controls by their **Field Names** and owns navigation and server response handling.

```rust
let submit = form.browser_submit("/signup");
let email = form.text(SignupForm::fields().email());

form {
    method: submit.method(),
    action: submit.action(),
    input { name: email.name(), value: email.value() }
}
```

**Progressive Submission** uses `progressive_submit()`. When hydrated, its `onsubmit` handler runs a **Browser Submit Preflight** and calls `prevent_default()` only when the current client state has a known blocker. If preflight allows the submit, the event is left alone and the browser POST continues.

```rust
let submit = form.progressive_submit();

form {
    method: "post",
    action: "/signup",
    onsubmit: move |event| {
        submit.on_submit(event);
    },
}
```

Progressive preflight checks mounted **Parse Blockers**, runs synchronous submit-triggered validation, and respects existing submit-relevant pending validation. It does not start or wait for submit-only async validators; use **Dioxus-Managed Submission** when submit correctness depends on client async validation completing before application submit behavior runs.

**Submit Availability** is only a prediction for browser-owned submit modes. Avoid disabling native fallback submit buttons solely from JS-only availability state unless intentionally requiring JavaScript; otherwise a no-JS user may lose the browser POST fallback even though the server remains the final authority.

For **Dioxus-Managed Submission**, a hard-disabled submit button causes a different dead end: it
suppresses the submit attempt that would reveal stored errors still withheld by the **Error
Visibility** policy. Keep the control actionable, use styling to communicate predicted availability
when useful, and render `submit_availability().blockers()` with details from the unfiltered
`validation_errors()`, `parse_errors()`, and `validation_statuses()` accessors when users need to know
why **Submit Availability** is currently false.

Intentful progressive forms still pass **Submit Intent** explicitly for client preflight. Do not infer typed intent from submit button `name` or `value`; those remain ordinary HTML data for the server.

```rust
let publish = form.progressive_submit().intent(ArticleSubmitIntent::Publish);

button { r#type: "submit", name: "intent", value: "publish", "Publish" }
```

Field name overrides and collection indexes affect rendered browser names only. **Field Identity** remains separate so validation state and collection metadata can follow logical fields while submitted browser data uses HTML-compatible names such as `invoice.lines[0].product.name`.

## Restoring a rejected browser POST

When the server rejects a POST, return the response values and application-defined diagnostics to
the re-rendered page. **Browser Rejection Restoration** initializes the page's form with those values
as both draft and baseline, typed rejection targets, and the attempted **Submit Intent**. It records
one prior attempt with `Rejected` status, without starting a managed submission. Rejections can
therefore be restored even when parsing or client validation would prevent submission.

The application owns the request/response payload and its mapping. For example, a server might
return this data after decoding `name`, `count`, and collection rows from POST fields:

```rust
use dioform::prelude::*;

#[derive(Clone, PartialEq, Form)]
struct Entry { count: u32 }

#[derive(Clone, PartialEq, Form)]
struct EditForm { name: String, count: u32, entries: Vec<Entry> }

#[derive(Clone, PartialEq)]
enum Intent { Save, Publish }

// Application transport DTO: serialization and server parsing are application-owned.
struct RejectedPost {
    values: EditForm,
    intent: Intent,
    name_error: Option<String>,
    form_error: Option<String>,
    raw_count: Option<String>,
    row_errors: Vec<(usize, String)>,
    row_count_errors: Vec<(usize, String)>,
    raw_row_counts: Vec<(usize, String)>,
}

fn rejected_config(response: RejectedPost) -> FormConfig<EditForm> {
    FormConfig::new(response.values)
        .id_namespace("edit-post")
        .browser_rejection(response.intent, move |targets| {
            let mut errors = SubmitErrors::none();
            if let Some(error) = &response.name_error {
                errors.push(SubmitError::field(EditForm::fields().name(), error.clone()));
            }
            if let Some(error) = &response.form_error {
                errors.push(SubmitError::form(error.clone()));
            }
            for (index, error) in &response.row_errors {
                // Whole-item targets resolve against THIS response's receiving form.
                if let Some(field) = targets.collection_item(EditForm::fields().entries(), *index) {
                    errors.push(SubmitError::field_identity(field, error.clone()));
                } else {
                    errors.push(SubmitError::form(error.clone()));
                }
            }
            for (index, error) in &response.row_count_errors {
                if let Some(field) = targets.collection_item_field(
                    EditForm::fields().entries(), *index, Entry::fields().count(),
                ) {
                    errors.push(SubmitError::field_identity(field, error.clone()));
                } else {
                    errors.push(SubmitError::form(error.clone()));
                }
            }
            let mut rejection = BrowserRejection::new(errors);
            if let Some(raw) = &response.raw_count {
                rejection = rejection.raw_field(EditForm::fields().count(), raw.clone());
            }
            for (index, raw) in &response.raw_row_counts {
                if let Some(field) = targets.collection_item_field(
                    EditForm::fields().entries(), *index, Entry::fields().count(),
                ) {
                    rejection = rejection.raw_field_identity(field, raw.clone());
                }
            }
            rejection
        })
}
```

Use `use_form_config(rejected_config(response))` when the page mounts, then stable hooks such as
`use_number(&form, EditForm::fields().count())` and `use_collection_item_number(...)` for parsed
fields. Keep the browser `method`, `action`, names, and progressive handler as above. Render the
binding's `value()`, `parse_error()`, and intent-filtered validation errors; a Publish rejection is
visible and blocks availability for Publish, not Save. The Field Convention also presents parsed
binding errors alongside validation errors through its usual rendered-text metadata.

For a later rejected response on an existing handle, call
`form.restore_browser_rejection(values, intent, |targets| BrowserRejection::new(errors))`, building
collection targets inside that callback just as above. This is explicit **Reinitialization**: it
replaces draft and baseline, clears interaction, retires old pending work and submit authorization,
and replaces the rejection batch with one prior attempt. It does not merge errors into newer edits.
Ordinary `reset()` and `reinitialize(values)` clear the rejection and history; hook rerenders do not
reapply the configuration's response. Restoration emits the normal value-redacted reinitialization
observer event, not submission events or submission listeners.

Configured validators remain available. Explicit initialization validation (for example,
`form.validate_initialization()`, or an explicit `register_core` initialization-validation call)
can run alongside restored errors without overwriting server-only messages. Validation modes that
depend on a prior submit attempt are active immediately.

### Raw text and retry behavior

For a numeric POST value such as `"abc"`, the server supplies an application-chosen typed fallback
in `values.count` and preserves the raw text separately. Restoration never parses this text into
the typed draft or marks it touched, blurred, or committed. The first matching parsed binding
consumes it once and runs its parser; failure displays the raw text and creates a mounted
**Parse Blocker**. Existing mounted bindings can consume a new response immediately. An unmounted
field's pending text is not a blocker. Unmount/remount does not replay consumed text.

A related value replacement, item removal, reset, or reinitialization retires deferred text before
consumption; unrelated edits preserve it. Collection reordering follows the receiving logical item,
not its former index. Do not import item identities from a different form instance. Server parsing
diagnostics stay application-owned: **Parse Errors** and **Raw Input State** are not core
`validation_errors()`. A binding uses its own parser message, so exact server/client message parity
and preservation of valid noncanonical text (such as `"007"`) are not promised.

Restored errors affect current **Submit Availability**, but retry buttons should remain actionable.
A new attempt reaching core submit preflight clears the old rejection before checking client
validators; an unchanged draft with a server-only form rejection can POST again. Failed client
checks still block. The adapter checks mounted parse blockers **before** running submit validators:
a parse-blocked attempt retains the old server rejection. This ordering also applies to the
adapter's managed submission entry points. Correcting raw text or unmounting its binding removes
that blocker and permits a fresh preflight.

Core-only servers can call
`core.restore_browser_rejection(values, intent, |targets| SubmitErrors::new(...))` with the same typed
mapping boundary, using `()` for non-intentful forms. No `SubmissionSnapshot` is required, and no
validated **Submitted Value** is claimed.

### Initial-render parity

Use the same response, mapping, parser configuration, mounted bindings, and explicit ID namespace
for server and client initialization. Executable regression coverage renders a restored page with
raw `"abc"` and a server-only error through two independent Dioxus trees and compares their HTML:

```sh
cargo test -p dioform-integration-tests browser_rejection
cargo test -p dioform --test browser_rejection
```

These tests cover initial HTML/state parity and hook rerender/reset behavior. They do not simulate
browser navigation or DOM hydration; the application still transports the response to its island
or fullstack page and owns server parsing, rendering, and native POST handling.
