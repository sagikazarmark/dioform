# Async And Debounced Validation

This document describes the implemented **Async Validation** and **Debounced Validation** behavior using the project glossary from `CONTEXT.md`.

The **Form Core** remains renderer-agnostic. It owns validation state, source-level **Validation Status**, **Form Snapshots**, stale-result checks, source-aware result application, and submit blocking or flush decisions. It does not spawn tasks, store concrete timer futures, or own a pluggable runtime trait. Runtime work crosses the **Validation Runtime** boundary through core work-token APIs and the **Dioxus Adapter**, which is the first integration that supplies Dioxus task spawning, debounce futures, managed submit waiting, and **Form Cleanup**.

## Validation Lifecycle Boundary

The issue #77 implementation introduced a private `validation_lifecycle` module inside the **Form Core** as a source-level lifecycle slice. In line with [ADR-0001](adr/0001-renderer-agnostic-core-dioxus-adapter-and-derive-macro.md), it stays renderer-agnostic and owns only the state machine for one registered validator source: trigger eligibility, sync or async source kind, source-level **Validation Status** transitions, stored validator errors, pending async run freshness, skipped async-after-sync behavior, and debounced scheduling or flush decisions.

The surrounding **Form Core** keeps target-aware edges outside that slice. `FormCore` owns field and form validator registry ordering, sync **Validation Chain** traversal, field and form snapshot capture, field and form version checks, observer storage and emission, flattened validation-error views, **Submit Availability**, submission state, and structured **Submit Errors**. Submit errors stay outside lifecycle storage because they are produced by application submission behavior, use submission snapshots for stale-field protection, and are not results from a registered validator source that should advance a validator's lifecycle status.

The **Dioxus Adapter** keeps runtime execution outside the lifecycle slice. It registers ergonomic async validator builders, spawns Dioxus tasks, polls debounce futures, waits for managed async submit validation, tracks parse blockers, updates selector reactivity, and ignores late results after **Form Cleanup**. The lifecycle records that async work is pending or stale, but it does not execute futures or timers; that keeps the **Form Core** independent of Dioxus runtime choices.

## Concepts

**Async Validation** is validation whose result may arrive after newer interaction. Async results are applied only while they still match the validation run and field or form version they were created for.

**Debounced Validation** is async validation delayed until interaction settles. It is not delayed error display. A debounced validator is marked `Pending` while it waits, then starts from the latest owned snapshot if it is still current.

**Validation Runtime** is the adapter/runtime boundary for async tasks and timers. The current boundary is made of core work-token APIs plus adapter-owned execution, not a public pluggable runtime trait. The core exposes state-machine operations such as `begin_async_field_validation`, `complete_async_field_validation`, and `schedule_debounced_async_field_validation`; the Dioxus adapter exposes ergonomic builder APIs such as `form.field(email).async_validator("availability")` and `form.async_validator("account")`, a default `debounce_duration(...)` timer helper, plus lower-level runtime methods such as `validate_async_field_validator`, `validate_async_field_validator_with_debounce`, `validate_async_form_validator`, and `managed_submit().on_submit_async(...)`. `debounce_duration(...)` uses a runtime-neutral `futures_timer::Delay` future that is polled inside Dioxus-spawned validation tasks; applications can pass a Dioxus-specific, browser-specific, or test-controlled delay future factory when they need a different timer source. Dioxus async field and form validators accept non-`Send` futures by default, matching Dioxus UI ergonomics.

**Form Snapshot** is an owned copy of form values captured when async validation or submission starts. Async field validators receive both the owned field value and the owned form snapshot. They never borrow the live **Form Draft** across an await point.

**Stale Validation Result** is a result from an older async run. If the field value, form version, validator run, reset, or reinitialization makes a run no longer current, completion returns `None` and stored validation state is not overwritten.

**Form Cleanup** is the Dioxus adapter lifecycle step that deactivates a handle when its component instance is gone. Late async validation or submission results are ignored after cleanup; the library does not depend on hard cancellation of already-spawned futures.

**Validator Source** is the registered-validator identity used for source-aware replacement of validation errors and pending results. It is the field/form validator specialization of the broader **Validation Source** glossary term.

## Initialization Validation

Initialization validation is always explicit. Creating a form and registering validators never starts async work. Calling `validate_initialization()` runs synchronous initialization validators immediately and starts any registered async initialization validators through the Dioxus runtime. Its boolean return value reflects the immediate synchronous result; async validators can still be `Pending` after the call and later become `Valid` or `Invalid`.

## Field Validation

Register an async field validator through the Dioxus adapter builder. Explicit validation calls, validation enabled by the form's **Validation Mode**, blur validation, and managed submit can then start the validator through the Dioxus runtime. The start call records `ValidationStatus::Pending` immediately and spawns the future through Dioxus.

For immediate manual validation, register the validator for `ValidationTrigger::Manual` and call `validate_field`. No debounce delay is involved unless `.debounce(...)` is configured and the trigger is `ValidationTrigger::Change`.

```rust
let email = SignupForm::fields().email();
let availability = form
    .field(email)
    .async_validator("availability")
    .on(ValidationTrigger::Manual)
    .check(|value, snapshot| async move {
        // `value` and `snapshot` are owned values captured for this run.
        // They are not live borrows from the Form Draft.
        if value == "taken@example.com" && snapshot.value().email == value {
            vec!["email_unavailable"]
        } else {
            Vec::new()
        }
    });

let status = form.field_validation_status(email, availability);

assert_eq!(status, Some(ValidationStatus::Unknown));
form.validate_field(email, ValidationTrigger::Manual);
assert_eq!(form.field_validation_status(email, availability), Some(ValidationStatus::Pending));
```

When the future resolves with no errors, the source becomes `Valid`. When it resolves with one or more errors, the source becomes `Invalid`, errors are stored under that validator source, and selectors such as `field_validation_errors`, `visible_field_validation_errors`, `field_validation_status`, `can_submit`, and `field_accessibility` update reactively.

The tracer-bullet test `dioxus_adapter_async_field_validation_updates_reactive_selectors` is the executable example for immediate async field validation. It demonstrates `Pending`, a later `Invalid` result, visible errors after blur, `can_submit` changing to `false`, and ARIA invalid state. The same API path returns `Valid` when the validator future resolves with an empty error list.

## Debounced Value-Change Validation

Use a debounced validator for value-change checks that should wait for user input to settle. The Dioxus adapter provides `debounce_duration(Duration::from_millis(...))` as the default timer helper. It is a runtime-neutral delay future, not a hard dependency on a Dioxus-owned timer type. The lower-level `.debounce(...)` API still accepts any future factory, so applications can choose a custom timer implementation when needed.

```rust
use std::time::Duration;

use dioform::debounce_duration;

let email = SignupForm::fields().email();
form.async_validator("account")
    .on(ValidationTriggers::new([
        ValidationTrigger::Change,
        ValidationTrigger::Submit,
    ]))
    .debounce(debounce_duration(Duration::from_millis(350)))
    .check(move |snapshot| async move {
        if snapshot.value().email == "taken@example.com" {
            vec![FormValidationError::field(email, "email_unavailable")]
        } else {
            Vec::new()
        }
    });
```

Only value-change validation is debounced. Submit-triggered validation starts immediate async validation instead.

The form's **Validation Mode** decides when value-change validation runs automatically. `ValidationMode::on_change()` runs change validation from the first value change. `ValidationMode::submit_then_revalidate()` waits until a submit attempt has happened, then runs change validation and blur validation automatically. Explicit calls such as `validate_field` and submit-triggered validation do not depend on the mode.

If a second value change schedules a newer debounced run before the first delay completes, the first delayed run is stale and never starts validation. The latest run captures a fresh **Form Snapshot** after its delay finishes. The core tests `debounced_async_field_validation_marks_pending_until_latest_value_starts` and `debounced_async_form_validation_marks_pending_until_latest_snapshot_starts` cover the field and form state-machine behavior. The Dioxus tests `dioxus_adapter_debounced_value_change_async_validation_updates_reactive_selectors` and `dioxus_adapter_debounced_value_change_async_form_validation_updates_reactive_selectors` cover the adapter/runtime behavior.

The [`demo/`](../demo) Async validation page demonstrates the ergonomic builder APIs with a debounced async field validator for username availability and a debounced async form validator for invite-code availability.

## Submit Behavior

**Submit Availability** is UI-oriented. It reports current known blockers such as stored validation errors, parse blockers, required pending validation, and in-flight submission. It is not a guarantee that a future submit can skip validation. It is intentionally conservative: stored errors from non-submit triggers can make `can_submit()` false even though an actual submit attempt may rerun submit-triggered validation and proceed.

Pending async validation blocks submit availability only when that validator is relevant to `ValidationTrigger::Submit`. A value-change-only pending validator does not block submission. A validator registered for both value change and submit can block submission while pending.

Async validators remember the trigger that produced their current result. A prior `Change` or `Manual` async success does not satisfy submit correctness for a validator that also participates in `ValidationTrigger::Submit`; submit records a pending blocker and starts submit-scoped async validation for the current snapshot.

Synchronous submit APIs cannot await async validation. `managed_submit().on_submit(...)` runs synchronous submit validation and returns `SubmitResult::Blocked(SubmitBlocker::ValidationErrors)` when submit-relevant async validation is still pending, stale, unknown, or must run before submission can be trusted. `submit_async_unmanaged(...)` and `intent(intent).submit_async(...)` make the same fire-and-return behavior explicit for callers that intentionally do not want managed waiting.

Dioxus-managed async submit can wait. `managed_submit().on_submit_async(...)` uses `submit_async_managed`, starts or flushes submit-relevant async validation immediately, waits for pending submit-relevant async validation to settle, then starts the application submit future only if validation still applies to the same field-version snapshot and has no errors.

Progressive browser submit preflight does not wait. `progressive_submit().on_submit(event)` runs synchronous submit validation and blocks existing known blockers, but it does not start submit-only async validators or wait for them before allowing browser POST. Use managed async submit when client async validation must finish before submit behavior runs.

Mounted parse blockers are checked before async validation or application submit behavior starts. Parsed text bindings own mounted parse-error state, so Dioxus components should create them with `use_parsed_text(form.clone(), path)` rather than constructing a fresh `parsed_text` binding on every render.

```rust
use std::time::Duration;

use dioform::debounce_duration;

let email = SignupForm::fields().email();

form.field(email)
    .async_validator("availability")
    .on(ValidationTriggers::new([
        ValidationTrigger::Change,
        ValidationTrigger::Submit,
    ]))
    .debounce(debounce_duration(Duration::from_millis(350)))
    .check(|value, _snapshot| async move {
        if value == "taken@example.com" {
            vec!["email_unavailable"]
        } else {
            Vec::new()
        }
    });

let result = form
    .managed_submit()
    .on_submit_async(event, |submitted| async move {
        // The submit handler runs only after submit-relevant validation settles.
        save_account(submitted.into_value()).await
    });
```

The tracer-bullet tests `dioxus_managed_async_submit_flushes_debounced_validation_before_submit_handler`, `dioxus_plain_submit_flushes_debounced_validation_before_blocking`, `dioxus_managed_async_submit_flushes_debounced_form_validation_before_submit_handler`, `dioxus_plain_submit_flushes_debounced_form_validation_before_blocking`, `dioxus_managed_async_submit_blocks_when_flushed_validation_returns_errors`, `dioxus_managed_async_submit_blocks_when_flushed_form_validation_returns_errors`, and `dioxus_managed_async_submit_does_not_submit_stale_flushed_validation_after_draft_edit` are the executable examples for submit flushing and submit blocking.

## Stale Results And Cleanup

Async validators are protected by run IDs and field or form versions. Editing a field after an async field run starts marks that source `Stale`, clears errors for that source, and prevents the old result from replacing newer state. Because async field validators receive the whole **Form Snapshot**, any draft edit can stale pending or completed async field validation; this is intentionally conservative and avoids a validation dependency graph. Reset and reinitialization invalidate pending async runs and debounced delayed runs in the same way.

```rust
let availability = core.register_async_field_validator_for_triggers(
    email,
    "availability",
    ValidationTrigger::Manual,
);
let old_run = core
    .begin_async_field_validation(email, availability, ValidationTrigger::Manual)
    .expect("validator should start");

core.set_field(email, "fresh@example.com".to_owned());

assert_eq!(
    core.field_validation_status(email, availability),
    Some(ValidationStatus::Stale),
);
assert_eq!(
    core.complete_async_field_validation(
        email,
        availability,
        &old_run,
        ["old_value_unavailable"],
    ),
    None,
);
assert!(core.field_validation_errors(email).is_empty());
```

The Dioxus adapter also checks whether the form handle is still active before applying late async results. Dropping the component runs cleanup, deactivates the adapter, and cancels scheduled or tracked validation work on a best-effort basis without changing live validation status as a correctness signal. Superseded debounced timers and ordinary validation tasks are also cancelled where the Dioxus runtime can remove them. If a future completes after cleanup, the result is ignored instead of mutating a destroyed form instance.

## Observer Events

Async validation lifecycle transitions are reported through value-redacted [`FormObserverEvent`] values. The core emits `AsyncValidationScheduled` when an async validator becomes `Pending`, `AsyncValidationCompleted` when a current result stores `Valid` or `Invalid`, `AsyncValidationSkipped` when sync-before-async short-circuiting skips an async validator, `AsyncValidationStaleIgnored` when an old completion is rejected, and debounced scheduling/flush events for delayed value-change validation.

Observer events intentionally expose validator source, target, trigger, and status rather than field values or form snapshots. `FormObserverValue::Redacted` remains the default value marker for form transitions that could otherwise include sensitive data, and `FormObserverEvent` is `#[non_exhaustive]` so downstream observers can continue compiling as the lifecycle grows.

Executable coverage:

- `stale_async_field_validation_completion_after_edit_does_not_replace_newer_result`
- `stale_async_form_validation_completion_after_edit_does_not_replace_newer_result`
- `reset_invalidates_pending_async_field_validation_and_debounced_field_run`
- `reset_invalidates_pending_async_form_validation_and_debounced_form_run`
- `reinitialize_invalidates_pending_async_field_validation_and_debounced_field_run`
- `reinitialize_invalidates_pending_async_form_validation_and_debounced_form_run`
- `dioxus_adapter_ignores_stale_async_field_validation_after_edit`
- `dioxus_adapter_ignores_stale_async_form_validation_after_edit`
- `dioxus_adapter_ignores_late_async_field_validation_success_after_form_cleanup`
- `dioxus_adapter_ignores_late_async_field_validation_errors_after_form_cleanup`
- `dioxus_adapter_ignores_late_async_form_validation_success_after_form_cleanup`
- `dioxus_adapter_ignores_late_async_form_validation_errors_after_form_cleanup`
