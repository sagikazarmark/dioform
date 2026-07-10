# Submit Intent

**Submit Intent** is an application-defined typed value that names why one **Dioxus-Managed Submission** was started. Use it when one **Form Draft** can be submitted for different purposes, such as saving a draft, publishing, saving and continuing, or deleting through the submission lifecycle.

Model intent as a small purpose value, usually an enum:

```rust
#[derive(Clone, Copy, Eq, PartialEq)]
enum ArticleSubmitIntent {
    SaveDraft,
    Publish,
    SaveAndContinue,
}
```

Submit triggers pass intent explicitly:

```rust
let result = form
    .managed_submit()
    .intent(ArticleSubmitIntent::Publish)
    .on_submit(event, |submission| {
        match submission.intent() {
            ArticleSubmitIntent::SaveDraft => save_draft(submission.value()),
            ArticleSubmitIntent::Publish => publish(submission.value()),
            ArticleSubmitIntent::SaveAndContinue => save_and_continue(submission.value()),
        }
    });
```

Submit-triggered validators can read the intent from their validation context:

```rust
form.write_advanced(|core| {
    core.register_sync_form_validator_for_triggers(
        "publish-title-required",
        ValidationTrigger::Submit,
        |context| {
            if context.submit_intent::<ArticleSubmitIntent>() == Some(&ArticleSubmitIntent::Publish)
                && context.form().title.is_empty()
            {
                vec![FormValidationError::field(ArticleForm::fields().title(), "required")]
            } else {
                Vec::new()
            }
        },
    );
});
```

The latest outcome is available globally as a `SubmitStatus`. Intentful forms scope status and availability to the button intent they render:

```rust
let publish = form.intent(ArticleSubmitIntent::Publish);

let publish_status = publish.last_status();

let publish_availability = publish.availability();

let latest = form
    .last_submit_status_as::<ArticleSubmitIntent>()
    .map(|status| (*status.intent(), status.status()));

let publish_errors = form.visible_validation_errors_for_intent(&ArticleSubmitIntent::Publish);

let publish_title_errors = form.visible_field_validation_errors_for_intent(
    ArticleForm::fields().title(),
    &ArticleSubmitIntent::Publish,
);
```

`last_submit_status()`, `submit_availability()`, and global visible-error selectors remain outcome-only/global conveniences. Intentful UIs that render per-button state should prefer `form.intent(intent).last_status()`, `form.intent(intent).availability()`, `visible_validation_errors_for_intent(&intent)`, and `visible_field_validation_errors_for_intent(path, &intent)` so a Save Draft result is not confused with a Publish result. UIs that need to react to whichever intent produced the latest outcome can use `last_submit_status_as::<Intent>()`. Availability is a read-only known-blocker signal; the submit attempt still performs submit-triggered validation with the provided intent before application submit behavior runs.

Progressive browser submit preflight also scopes typed intent explicitly with `progressive_submit().intent(intent)`. The typed **Submit Intent** is not inferred from HTML submit button `name` or `value`; those values are ordinary submitted browser data for the server.

Use **Submit Intent** only for submission purpose. Do not use it for secrets, large payloads, form field values, analytics data, mouse coordinates, or UI state that does not participate in validation, submit errors, or application submit behavior. The **Submission Snapshot** already carries the validated submitted value.

Observer events omit raw **Submit Intent** values by default. Intent remains available through typed application-facing APIs such as the submit handler payload, validation context, and intent-aware status, availability, and visible-error reads. `FormStateSnapshot` does not serialize submit-scoped validation state, stored submit errors, or the latest submit status because that state is associated with an arbitrary application-defined intent type.
