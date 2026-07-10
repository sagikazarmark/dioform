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

Intentful progressive forms still pass **Submit Intent** explicitly for client preflight. Do not infer typed intent from submit button `name` or `value`; those remain ordinary HTML data for the server.

```rust
let publish = form.progressive_submit().intent(ArticleSubmitIntent::Publish);

button { r#type: "submit", name: "intent", value: "publish", "Publish" }
```

Field name overrides and collection indexes affect rendered browser names only. **Field Identity** remains separate so validation state and collection metadata can follow logical fields while submitted browser data uses HTML-compatible names such as `invoice.lines[0].product.name`.
