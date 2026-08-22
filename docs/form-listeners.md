# Form Listeners

Form Listeners are application-owned side-effect hooks for semantic form events. Use them for behavior such as autosave, analytics, dependent-field resets, or syncing ordinary application state. Do not put those side effects in validators: Field Validation and Form Validation should only decide whether typed form values are acceptable.

The first listener slices support value-replacement listeners, blur listeners, direct hook-owned field binding lifecycle listeners, submit lifecycle listeners, and debounced value-replacement listeners in the Dioxus adapter. Value-replacement listeners cover direct field replacements, direct collection structure mutations, direct collection item field replacements, and true multi-select changes backed by direct `Vec<Value>` fields.

## Field Listeners

Use `use_field_listener(form, path, listener)` when the listener should run for both user-originated and programmatic value replacements reaching one typed field. For direct collection fields, collection insertions, removals, moves, and multi-select changes are value replacements for the collection field.

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

### Listener Reach

A value-replacement listener registered on a Field runs when that Field is written, when a Field it contains is written, and when a Field containing it is written. Siblings never reach it, and the Identity Path Separator anchors that boundary: a write to `counterparty` does not reach a listener on `counterparty_account`.

Both directions are the same fact. Writing a Field is a whole-subtree assignment, so writing `invoice.customer` replaces the value of `invoice.customer.name`, and writing `invoice.customer.name` replaces the value of `invoice.customer`. Reach follows the write, not the registration, so the context reports the identity of the Field that was written, which is not always the one the listener registered on. A listener that wants one Field exactly compares `context.field_identity()` against its own `path.identity()`.

A listener registered on a Collection Field additionally hears value replacements of its own items' fields, so a listener on `invoice.lines` hears both a pushed row and an edited one. Those item events carry a collection item identity: read `collection_path()` and `collection_item_identity()` to identify the row, not `as_str()`, which is item-relative and returns only the child-field segment. That is also why the exact-identity comparison above does not hold for a listener registered on a Collection Field — it never receives its own static identity for an item write.

Reach belongs to the event rather than to the listener, so it is decided per surface. Form-level listeners are unchanged: they already run for every field. Blur listeners and binding lifecycle listeners report that something happened *inside* a Field rather than that a value was replaced, so both reach outward only.

## Form-Level Listeners

Use `use_form_listener(form, listener)` when one listener should observe value replacements for every field in the form. Use `use_form_listener_for_origin(form, FieldUpdateOrigin::User, listener)` to observe only user-originated replacements.

Form-level listener context exposes the `FormHandle`, the triggering `FieldIdentity`, the rendered field name, the `FormListenerEvent`, and the `FieldUpdateOrigin`. The current event slice reports `FormListenerEvent::FieldReplaced` for value replacements. The context does not pass field values by default. This lets analytics and logging identify the triggering Field without accidentally receiving sensitive values.

## Blur Listeners

Use `use_field_blur_listener(form, path, listener)` when a side effect should run after that Field or a Field it contains is marked blurred. Reach is outward only: a listener on `invoice.customer` hears a blur of `invoice.customer.name`, while a listener on `invoice.customer.name` does not hear a blur of `invoice.customer`. A Collection Field listener hears blurs inside its own items, but sibling collections and static descendants of the collection path do not. The Identity Path Separator anchors the boundary, so a blur of `counterparty_account.name` does not reach a listener on `counterparty`.

Each triggering blur produces one callback. Moving focus between two children of the same container therefore runs the container listener twice; the listener does not debounce or synthesize a single "left the container" event.

Field blur listener context exposes the `FormHandle`, the triggering `FieldIdentity`, the listener's registered `FieldIdentity`, and accessors that distinguish a direct blur from a contained Field's blur. `field_identity()` always returns the triggering identity. Blur and touched metadata stay exact: inside a container listener reached by a child blur, `is_field_blurred(container)` and `is_field_touched(container)` remain `false` unless the container itself was separately marked. Blur listeners do not expose field values by default.

Use `use_form_blur_listener(form, listener)` when one listener should observe blur events for every field in the form, including direct collection item fields. Form blur listener context exposes the `FormHandle`, triggering `FieldIdentity`, and rendered field name.

## Binding Lifecycle Listeners

Use `use_field_binding_listener(form, path, listener)` when a side effect should observe hook-owned binding mount and unmount events for a Field or any Field it contains. Reach is outward only: a listener on `invoice.customer` hears a binding on `invoice.customer.name`, while a listener on `invoice.customer.name` does not hear a binding on `invoice.customer`. The current lifecycle slice reports `FieldBindingLifecycle::Mounted` and `FieldBindingLifecycle::Unmounted` for direct field hooks such as `use_parsed_text(...)`, `use_parsed_text_with(...)`, `use_number(...)`, `use_number_with(...)`, `use_optional_number(...)`, `use_date(...)`, `use_date_with(...)`, `use_optional_date(...)`, `use_optional_text(...)`, `use_select(...)`, `use_select_with(...)`, `use_radio_group(...)`, and `use_multi_select(...)`. Binding lifecycle context exposes the `FormHandle`, the bound Field's `FieldIdentity`, and lifecycle state, but no field values.

Binding lifecycle listeners are independent of hook order within a component. If one or more binding hooks in the listener's reach run before its listener hook, the listener receives one `Mounted` event for each currently active binding when it registers. If listener cleanup runs before binding cleanup, the listener receives matching `Unmounted` events before it unregisters. Live and replayed events always report the bound Field's identity rather than the listener's registered identity.

Collection item child binding lifecycle events are not part of this slice. Collection item binding hooks currently do not dispatch lifecycle events, while the listener registration API is scoped to typed `FieldPath<Model, Value>` paths on the root form model.

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

For direct hook-owned field binding lifecycle events, Dioform records active binding counts per `FieldIdentity`, dispatches mount listeners after the binding is created, and dispatches unmount listeners during hook cleanup before the binding is dropped. Newly registered listeners receive `Mounted` for each active binding at or below their registered Field so mount/unmount events remain balanced regardless of listener hook order.

For debounced value-replacement listeners, Dioform schedules matching form-level debounced listeners before matching field-scoped debounced listeners, mirroring immediate listener scope ordering. Callback execution happens later when the listener's own delay future completes; stale scheduled callbacks are ignored.

For submit events, Dioform records the submit attempt and runs submit-triggered validation before dispatching `SubmitAttempted`. It dispatches `SubmissionStarted` only after submission actually starts, then dispatches `SubmissionSucceeded` or `SubmissionRejected` from the successful or structured-error finish transition. Blocked attempts dispatch `SubmitBlocked` after the blocker is recorded.

Several field-scoped listeners can match one write, because Listener Reach admits a Field's containers and the Fields it contains. They run in registration order, which is the order their hooks ran: a listener on a container registered before a listener on one of its leaves observes the write first. Dioform does not order them by depth. Registration order tracks component mount order, so a remounted subtree's listener moves to the end.

Each matching listener runs once per dispatched value replacement. One write dispatches one event, including a collection item field write, which dispatches once with the item's identity.

Listener-caused field replacements are ordinary new Programmatic Updates. They preserve the same metadata, validation, observer, selector, and listener invariants as any other `set_field(...)` call.

Field Listeners do not participate in Submit Availability and Submit Listeners do not replace submit handlers. The submit lifecycle still performs submit-triggered validation before application submit behavior.

## Reentry

Listeners can create cycles if they write fields that trigger the same listener again. Dioform detects same-callback reentry and panics with a listener-specific message rather than exposing an internal borrow failure. Prefer origin-filtered listeners for user-driven side effects, especially when a listener writes back to the same field or to a field with another listener.

Field blur listeners use the same reentry protection. A blur callback that causes another blur at or below its registered Field panics; there is no origin-filtered blur-listener variant that can opt out of listener-caused events.

Listener Reach widens what counts as a cycle. A listener on a container that resets one of its own fields writes inside its own reach and so re-enters itself, which is the common dependent-field reset written against a container instead of a leaf. Use `use_field_listener_for_origin(..., FieldUpdateOrigin::User, ...)`: listener-caused writes are Programmatic Updates, so the origin filter resolves it. Dioform does not silently drop the second dispatch — a dropped side effect on a surface whose job is autosave or audit is worse than a development-time panic.

A debounced callback runs after its delay resolves, so same-callback reentry cannot catch its cycles. Dioform instead bounds how many times in a row a debounced listener runs on a listener-caused write: past that bound it panics rather than rescheduling forever. The bound catches a debounced listener that writes a Field in its own reach as well as a cycle between two listeners neither of which writes into its own reach.

The count is of runs, not of schedules, so a callback that writes several Fields in one listener's reach is not a cycle — those writes supersede one another and produce one run. Any write made from outside a listener callback, whether the user's or the application's, ends the run and clears the counts, so an ordinary chain — a debounced normalizer whose write wakes a debounced autosave, once per user edit — never reaches the bound.
