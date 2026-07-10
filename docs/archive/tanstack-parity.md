# TanStack Form Parity Index

This is the durable index for how Dioform maps to [TanStack Form](https://tanstack.com/form).
It exists so the comparison lives in one place instead of being re-derived, and so each
open parity issue has a canonical home.

TanStack Form is a useful reference point because it shares Dioform's high-level shape:
a renderer-agnostic core plus a thin renderer adapter, headless and controlled, with
selector-based subscriptions. Dioform deliberately diverges wherever a Rust-native,
compile-time-typed interface is deeper than a string-path or component-driven API. Those
divergences are recorded here as first-class rows, not omissions.

For the vocabulary used below (Field Path, Validation Source, Submit Intent, Parse Blocker,
Collection Item Identity, and so on) see [`CONTEXT.md`](../CONTEXT.md). For the intentional
scope boundaries see [`mvp-boundaries.md`](mvp-boundaries.md).

Status legend:

- **Implemented**: shipped; the row records the deliberate divergence, if any.
- **Tracked (#N)**: an open issue owns the decision or the work.
- **Divergent by design**: Dioform intentionally does something different; the row says what.

> Issue references use reference-style links (defined at the bottom) so they render as
> clickable links in the GitHub file view. Keep the reference block in sync when issues close.

## Validation triggers and modes

| TanStack capability | Dioform | Status |
| --- | --- | --- |
| `onChange` / `onBlur` / `onSubmit` triggers | `ValidationTrigger` + higher-level `ValidationMode` (on-blur default, on-change, on-submit, submit-then-revalidate) | Implemented |
| `onMount` eager validation | Explicit `validate_initialization()` today; an opt-in eager `ValidationMode` is under evaluation | Tracked [#149] |
| `dynamic` / programmatic validation trigger | `ValidationTrigger::Manual` plus `validate_field` / `validate_form` / `validate_all` / `validate_*_validator` | Implemented |

## Validation chain and async

| TanStack capability | Dioform | Status |
| --- | --- | --- |
| Sync-before-async chain, async skipped on sync failure | Same default in the Validation Chain | Implemented |
| `asyncAlways` opt-out | Per-validator opt-out of chain skipping | Tracked [#140] |
| Per-validator async debounce | `.debounce(delay_factory)` on async validator builders | Implemented |
| Form-wide `asyncDebounceMs` default | Form-wide default with per-validator override | Tracked [#146] |
| `AbortSignal` hard cancellation | Correctness comes from Stale Validation Result guards + Form Cleanup, not cancellation; a *cooperative* signal for the app's own work is tracked | Divergent by design · Tracked [#142] |
| `onChangeListenTo` / `onBlurListenTo` linked fields | Cross-field rules today are Form Validation (Central Validator); a cheaper linked single-field re-run is tracked | Tracked [#132] |

## Error model and accessors

| TanStack capability | Dioform | Status |
| --- | --- | --- |
| Standard Schema validators (Zod / Valibot / ArkType) | Per-library adapter crates (`dioform-garde`, `dioform-validator`) sharing the `dioform-validation-adapter` data types. No Rust "Standard Schema" exists and per-library `register` bounds are irreducible, so a public adapter *trait* is declined; the shared data types are the extension point | Divergent by design (declined, [ADR-0018]) |
| Schema paths auto-map to fields | Derive the External-Path → Field Path map from `#[derive(Form)]` metadata | Tracked [#138] |
| `parseValuesWithSchema` (coerce/transform output) | The model is already statically typed; coercion is a per-field parsed-binding concern (Raw Input State / Parse Error), so there is no raw-vs-output type to reconcile | Divergent by design (declined, [ADR-0017]) |
| `setErrorMap` (manual errors) | Manual/application Validation Source injection | Tracked [#139] |
| `getAllErrors` aggregate | `validation_errors()` / `visible_validation_errors()`: whole-form aggregate over source-aware storage (fields + collection-item children + form + submit errors), each entry keeping target + source; see [`error-summary.md`](error-summary.md) | Implemented |
| `errorMap` keyed by cause | Per-field errors grouped by trigger/source, without flattening | Tracked [#136] |
| `errorSourceMap` (field vs form) | Validation Source already carries strictly more (field / form / submit / server / per-validator identity) | Divergent by design (richer) |

## Fields and collections

| TanStack capability | Dioform | Status |
| --- | --- | --- |
| `pushValue` / `insertValue` / `removeValue` / `moveValue` | `append` / `insert` / `remove` / `move_to_index` on `CollectionBinding` (+ programmatic variants) | Implemented |
| `swapValues` / `replaceValue` / `clearValues` | `swap` / `replace` (in-place, identity-preserving) / `clear` on `CollectionBinding`, with `_programmatic` variants; item-scoped state follows Collection Item Identity | Implemented |
| Nested arrays (`items[i].sub[j]`) | Nested Collection Field traversal with nested Collection Item Identity | Tracked [#147] |
| Developer-managed array item keys | Library-owned Collection Item Identity; metadata follows items through insert/remove/reorder with **no app keys** | Divergent by design (deeper) |
| `resetField(name)` | Single-field `reset_field(path)`: restores baseline, clears the field's metadata + field-scoped validation + parse state (direct fields; collections use whole-form reset) | Implemented |
| (no file model) | First-class File Fields with cardinality + file-aware submit snapshots, stored outside the Form Draft | Divergent by design (beyond TanStack) |

## State and meta

| TanStack capability | Dioform | Status |
| --- | --- | --- |
| `isTouched` / `isBlurred` / `isDirty` | Touched / Blurred / Dirty tracking | Implemented: **non-sticky dirty** (revert ⇒ clean), unlike TanStack's sticky dirty + `isDefaultValue` |
| `isValidating` / `isPristine` / `isDefaultValue` | Thin convenience readers over existing state (`is_validating` / `is_field_validating`, `is_pristine`, `is_default_value`); sticky-dirty semantics intentionally *not* adopted | Implemented |
| `submissionAttempts` / `isSubmitSuccessful` | `submission_attempts()` + `is_submit_successful()` (global, and per-intent success) over Last Submit Status; the per-**intent** attempt *count* is tracked separately | Implemented · Tracked [#155] |
| `canSubmit` | Submit Availability: evaluated **per Submit Intent** in intentful forms | Implemented (per-intent) |
| `canSubmitWhenInvalid` opt-out | No validation-bypass path; save-draft uses intent-scoped submit validation, server-authoritative uses Submit Errors. Availability stays an unconditional known-blockers signal | Divergent by design (declined, [ADR-0019]) |

## Submission

| TanStack capability | Dioform | Status |
| --- | --- | --- |
| `handleSubmit` | Dioxus-Managed Submission with owned Submission Snapshot | Implemented |
| `onSubmitMeta` / submit meta | Typed Submit Intent (Save Draft vs Publish) with per-intent availability, last-status, and validation | Implemented (richer) |
| `onSubmitInvalid` | A submit-invalid Form Listener event fired on blocked/invalid attempts | Tracked [#135] |
| Server errors block submit | Structured Submit Errors, stale-submit-error clearing, fullstack rejection mapping | Implemented |
| (controlled-only) | Native Browser Submission + Progressive Submission with hydrated preflight | Divergent by design (beyond TanStack) |

## Composition and reuse

| TanStack capability | Dioform | Status |
| --- | --- | --- |
| `createFormHook` bound components | Optional bound reusable-field-component layer under design evaluation | Tracked [#137] |
| `withFieldGroup` | Typed Field Groups + Field Group Maps | Implemented |
| `withForm` (typed sub-component split) | Covered by Field Groups + the [#137] exploration | Tracked [#137] |
| `formOptions` shared config | Shared adapter-based Form Configuration across client/server | Tracked [#143] |

## Fullstack and SSR

| TanStack capability | Dioform | Status |
| --- | --- | --- |
| `createServerValidate` + `mergeForm` | Isomorphic adapter-based validation shared client/server, merged via manual Validation Source | Tracked [#143] |
| Server submit rejection | `dioform-fullstack` maps rejections into structured Submit Errors | Implemented |
| SSR hydration | Deterministic Form Initialization; **no** automatic form-state serialization by default | Divergent by design |
| (n/a) | Opt-in Form State Serialization, including Collection Item Identity round-trip | Implemented (beyond TanStack) |

## Tooling and reactivity

| TanStack capability | Dioform | Status |
| --- | --- | --- |
| Selector-based subscriptions (`useStore` / `Subscribe`) | Form Selector reads with selective notifications | Implemented |
| Form Devtools UI | Value-redacted Form Observer event stream exists; a devtools UI on top of it is post-MVP | Tracked [#48] |

## The one unfiled concept: multiple renderer adapters

TanStack ships one framework-agnostic core with adapters for React, Vue, Solid, Svelte,
Angular, Lit, and Preact. Dioform's core (`dioform-core`) is likewise
renderer-agnostic, but the project is deliberately **Dioxus-only**. Shipping an adapter for
another Rust UI framework is a product-identity and scope decision, not a feature to adopt
quietly, so there is intentionally no issue for it. This row exists so the omission is a
recorded decision rather than an oversight.

## Where Dioform leads

The rows marked *Divergent by design (deeper/richer/beyond)* above are not gaps to close;
they are places the Rust, compile-time-typed model reaches further than TanStack:

- **Typed `FieldPath<Model, Value>`** is the primary addressing API; rendered Field Names
  exist only for HTML interop.
- **Non-sticky Dirty** matches user intuition (revert ⇒ clean) without a separate escape hatch.
- **Source-aware Validation Errors** never flatten multiple sources into one replaceable slot.
- **Submit Intent** models submit purpose as a typed value, richer than untyped submit meta.
- **Parse Blockers / Raw Input State** keep the last valid *typed* value while raw input is
  unparsable: a concern a string-valued JS form does not even represent.
- **Library-owned Collection Item Identity** lets draft-only and duplicate-valued rows keep
  metadata through reordering with no application keys.
- **Cooperative stale-result protection** over hard cancellation keeps the core free of an
  async-runtime cancellation dependency.
- **File Fields, native/progressive submission, and opt-in form-state serialization** all go
  beyond TanStack's controlled-only surface.

[#48]: https://github.com/sagikazarmark/dioform/issues/48
[#132]: https://github.com/sagikazarmark/dioform/issues/132
[#135]: https://github.com/sagikazarmark/dioform/issues/135
[#136]: https://github.com/sagikazarmark/dioform/issues/136
[#137]: https://github.com/sagikazarmark/dioform/issues/137
[#138]: https://github.com/sagikazarmark/dioform/issues/138
[#139]: https://github.com/sagikazarmark/dioform/issues/139
[#140]: https://github.com/sagikazarmark/dioform/issues/140
[#142]: https://github.com/sagikazarmark/dioform/issues/142
[#143]: https://github.com/sagikazarmark/dioform/issues/143
[#146]: https://github.com/sagikazarmark/dioform/issues/146
[#147]: https://github.com/sagikazarmark/dioform/issues/147
[ADR-0018]: adr/0018-decline-public-validation-adapter-trait.md
[ADR-0019]: adr/0019-decline-can-submit-when-invalid-opt-out.md
[#149]: https://github.com/sagikazarmark/dioform/issues/149
[ADR-0017]: adr/0017-decline-whole-model-schema-coercion.md
[#155]: https://github.com/sagikazarmark/dioform/issues/155
