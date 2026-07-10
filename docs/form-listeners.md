# Form Listeners

Form Listeners are application-owned side-effect hooks for semantic form events. Use them for behavior such as autosave, analytics, dependent-field resets, or syncing ordinary application state. Do not put those side effects in validators: Field Validation and Form Validation should only decide whether typed form values are acceptable.

The first listener slices support value-replacement listeners, blur listeners, direct hook-owned field binding lifecycle listeners, submit lifecycle listeners, and debounced value-replacement listeners in the Dioxus adapter. Value-replacement listeners cover direct field replacements, direct collection structure mutations, direct collection item field replacements, and true multi-select changes backed by direct `Vec<Value>` fields.

## Field Listeners

Use `use_field_listener(form, path, listener)` when the listener should run for both user-originated and programmatic replacements of one typed field. For direct collection fields, collection insertions, removals, moves, and multi-select changes are value replacements for the collection field.

Use `use_field_listener_for_origin(form, path, FieldUpdateOrigin::User, listener)` when the listener should run only for user-originated field replacements. This is the safer default for dependent-field resets because listener-caused updates are ordinary programmatic replacements and should not usually re-enter the same listener.

```rust
let email = SignupForm::fields().email();
let accepts_terms = SignupForm::fields().accepts_terms();

use_field_listener_for_origin(
    form.clone(),
    email,
    FieldUpdateOrigin::User,
    move |context| {
        context.form().set_field(accepts_terms.clone(), false);
    },
);
```

The listener context exposes the `FormHandle`, the triggering `FieldIdentity`, and the `FieldUpdateOrigin`. It does not pass field values by default. If a side effect needs values, read them explicitly through the form handle with `field_value(...)` or `snapshot()`.

## Form-Level Listeners

Use `use_form_listener(form, listener)` when one listener should observe value replacements for every field in the form. Use `use_form_listener_for_origin(form, FieldUpdateOrigin::User, listener)` to observe only user-originated replacements.

Form-level listener context exposes the `FormHandle`, the triggering `FieldIdentity`, the rendered field name, the `FormListenerEvent`, and the `FieldUpdateOrigin`. The current event slice reports `FormListenerEvent::FieldReplaced` for value replacements. The context does not pass field values by default. This lets analytics and logging identify the triggering Field without accidentally receiving sensitive values.

## Blur Listeners

Use `use_field_blur_listener(form, path, listener)` when a side effect should run after one field is marked blurred. Use `use_form_blur_listener(form, listener)` when one listener should observe blur events for every field in the form, including direct collection item fields. Field blur listener context exposes the `FormHandle` and triggering `FieldIdentity`. Form blur listener context exposes the `FormHandle`, triggering `FieldIdentity`, and rendered field name. Blur listeners do not expose field values by default.

## Binding Lifecycle Listeners

Use `use_field_binding_listener(form, path, listener)` when a side effect should observe hook-owned binding mount and unmount events for one Field. The current lifecycle slice reports `FieldBindingLifecycle::Mounted` and `FieldBindingLifecycle::Unmounted` for direct field hooks such as `use_parsed_text(...)`, `use_parsed_text_with(...)`, `use_number(...)`, `use_number_with(...)`, `use_date(...)`, `use_date_with(...)`, `use_select(...)`, `use_select_with(...)`, `use_radio_group(...)`, and `use_multi_select(...)`. Binding lifecycle context exposes the `FormHandle`, triggering `FieldIdentity`, and lifecycle state, but no field values.

Binding lifecycle listeners are independent of hook order within a component. If a binding hook runs before its listener hook, the listener receives `Mounted` for currently active bindings when it registers. If listener cleanup runs before binding cleanup, the listener receives matching `Unmounted` events before it unregisters.

Collection item child binding lifecycle listeners are not part of this slice because the current listener registration API is scoped to typed `FieldPath<Model, Value>` paths on the root form model.

## Debounced Listeners

Use `use_debounced_field_listener(form, path, delay, listener)` or `use_debounced_field_listener_for_origin(form, path, origin, delay, listener)` when a field-scoped side effect should run only after value replacement settles. Use `use_debounced_form_listener(form, delay, listener)` or `use_debounced_form_listener_for_origin(form, origin, delay, listener)` for form-level value replacement events.

The delay argument is a factory that returns a fresh `Future<Output = ()>` for each matching event. When a newer matching event arrives before an older delay completes, the older scheduled callback is ignored. Debounced listener callbacks receive the same listener contexts as immediate value-replacement listeners and still do not receive field values by default; read values explicitly through the `FormHandle` when needed.

Debounced listeners are application side effects, not validation work. They do not block submission, do not affect `SubmitAvailability`, do not change validation status, and are not flushed when a submit starts.

## Submit Listeners

Use `use_submit_listener(form, listener)` when a side effect should observe submit lifecycle events without replacing the submit handler itself. The listener receives `SubmitListenerContext`, which exposes the `FormHandle`, a `SubmitListenerEvent`, and typed submit intent access through `submit_intent::<Intent>()`. It does not pass the submitted value by default.

The current submit event slice reports:

- `SubmitListenerEvent::SubmitAttempted` after a submit attempt is recorded.
- `SubmitListenerEvent::SubmissionStarted` after submit validation passes and application submit behavior starts.
- `SubmitListenerEvent::SubmitBlocked(blocker)` when submission does not start because of a known `SubmitBlocker`.
- `SubmitListenerEvent::SubmissionRejected` when application submit behavior returns structured submit errors.
- `SubmitListenerEvent::SubmissionSucceeded` when application submit behavior completes successfully.

For managed async submission that waits on submit-relevant async validation, `SubmitAttempted` is emitted when the attempt is recorded, and `SubmissionStarted` is emitted later only if validation settles successfully and the application submit behavior starts.

For intentful forms, call `context.submit_intent::<MySubmitIntent>()` to read the typed **Submit Intent** that produced the listener event. The method returns `None` when the requested type does not match the event's intent type.

## Ordering

For direct field value replacements and direct collection-backed value replacements, Dioform applies listener ordering as follows:

1. Replace the typed field value in the Form Draft.
2. Update field and form versions, dirty state inputs, and stale submit-error state.
3. Run configured synchronous value-change validation and emit Form Observer diagnostics from the core.
4. Notify Dioxus selectors and schedule runtime async validation when configured.
5. Dispatch matching form-level listeners.
6. Dispatch matching field-scoped listeners.

For direct field and direct collection item field blur events, Dioform applies listener ordering as follows:

1. Mark the Field touched and blurred.
2. Notify Dioxus metadata selectors.
3. Run configured blur validation and notify validation selectors when configured.
4. Dispatch matching form-level blur listeners.
5. Dispatch matching field-scoped blur listeners.

For direct hook-owned field binding lifecycle events, Dioform records active binding counts per `FieldIdentity`, dispatches mount listeners after the binding is created, and dispatches unmount listeners during hook cleanup before the binding is dropped. Newly registered listeners receive `Mounted` for active bindings of the same field so mount/unmount events remain balanced regardless of listener hook order.

For debounced value-replacement listeners, Dioform schedules matching form-level debounced listeners before matching field-scoped debounced listeners, mirroring immediate listener scope ordering. Callback execution happens later when the listener's own delay future completes; stale scheduled callbacks are ignored.

For submit events, Dioform records the submit attempt and runs submit-triggered validation before dispatching `SubmitAttempted`. It dispatches `SubmissionStarted` only after submission actually starts, then dispatches `SubmissionSucceeded` or `SubmissionRejected` from the successful or structured-error finish transition. Blocked attempts dispatch `SubmitBlocked` after the blocker is recorded.

Listener-caused field replacements are ordinary new Programmatic Updates. They preserve the same metadata, validation, observer, selector, and listener invariants as any other `set_field(...)` call.

Field Listeners do not participate in Submit Availability and Submit Listeners do not replace submit handlers. The submit lifecycle still performs submit-triggered validation before application submit behavior.

## Reentry

Listeners can create cycles if they write fields that trigger the same listener again. Dioform detects same-callback reentry and panics with a listener-specific message rather than exposing an internal borrow failure. Prefer origin-filtered listeners for user-driven side effects, especially when a listener writes back to the same field or to a field with another listener.
