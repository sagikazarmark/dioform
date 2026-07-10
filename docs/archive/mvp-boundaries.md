# MVP Boundaries And Extension Points

This document records the intentional Dioform MVP boundaries from [PRD issue #1](https://github.com/sagikazarmark/dioform/issues/1). It uses the project glossary in `CONTEXT.md` and follows [ADR-0001](adr/0001-renderer-agnostic-core-dioxus-adapter-and-derive-macro.md), which keeps the **Form Core**, **Dioxus Adapter**, and derive macro in separate crates.

The MVP is a small, complete vertical slice of a **Headless Form Library**, not a clone of every TanStack Form feature. Contributors should preserve these boundaries unless a follow-up issue explicitly moves a post-MVP feature into scope.

## Comparison Notes For TanStack Form Readers

TanStack Form is a useful reference point for framework-neutral form state, selector-based subscriptions, validation causes, schema adapters, listeners, field groups, and devtools. Dioform deliberately keeps the same high-level separation between a renderer-agnostic core and renderer integration, but chooses Rust-native interfaces where they are deeper than string-path or component-driven APIs.

A capability-by-capability parity map (every TanStack Form feature, where it lands in Dioform, the owning issue where work is tracked, and each deliberate divergence) lives in [`tanstack-parity.md`](tanstack-parity.md).

The first release should preserve these differences:

- Typed **Field Paths** are the primary field-addressing API. Rendered **Field Names** exist for HTML interoperability and future server/native form interop.
- **Field Identity** is separate from rendered names so metadata, validation errors, selectors, observers, and collection item state do not depend on string field names.
- **Collection Item Identity** is library-owned so draft rows and duplicate-valued rows keep metadata through insertion, removal, and reordering without requiring application IDs.
- **Raw Input State** and **Parse Errors** belong to the **Dioxus Adapter**, while the **Form Core** stores typed values only. Mounted **Parse Blockers** prevent Dioxus-managed submission from submitting stale typed values while visible input is unparsable.
- **Validation Sources** stay source-aware rather than flattening all errors for one field into one replaceable slot.
- **Form Handles** are explicit in the MVP. Typed context providers are an optional post-MVP ergonomic layer over existing handles; renderless field components and app-specific field group composition remain separate concerns.
- Validation logic is configured through explicit **Validation Triggers** and a higher-level **Validation Mode**. The mode can choose submit-only, blur, change, or submit-first-then-revalidate behavior without changing the core trigger model.

## MVP Scope

The MVP centers on statically known Rust **Form Models** with form-owned **Form Drafts**. Applications supply initial values through **Form Configuration**, then the form owns the editable **Form Draft** until explicit **Reset**, **Reinitialization**, or **Submission** behavior runs. The MVP does not expose **Application-Owned Draft** APIs. The derive macro produces typed **Static Field Paths** for direct fields on a **Named Form Struct**, while **Field Identity** remains separate from rendered **Field Names**. The **Form Core** owns state-machine behavior for values, metadata, sync validation, submission, reset, reinitialization, and observer events without depending on Dioxus.

The **Dioxus Adapter** is the facade for Dioxus users. It connects the core to Dioxus hooks, events, hydration-safe initialization, controlled input bindings, adapter-owned **Raw Input State**, **Parse Errors**, **Parse Blockers**, **File Selection**, **Accessibility Helpers**, async validation tasks, debounced validation timers, and explicit **Form ID Namespaces**. Submission in the MVP is **Dioxus-Managed Submission**, not native browser posting.

The demoable proof started as the three-field signup example from issue #13 and now lives in the [`demo/`](../demo) gallery. The demo demonstrates direct typed field paths, controlled text and checkbox bindings, sync **Field Validation**, sync **Form Validation**, async submit behavior, default **Error Visibility**, **Submit Availability**, structured **Submit Errors**, **Reset**, and headless rendering.

## Approved Dioxus Adapter Surface

`use_form_config(FormConfig::new(...))` is the canonical Dioxus hook for configured forms that need explicit validation modes, error visibility policies, custom validation error types, or explicit **Form ID Namespaces**. `use_form_handle(|| ...)` remains the lower-level hook for callers that need to build a `FormHandle` directly. Documentation and examples should prefer explicit **Form ID Namespaces** whenever derived IDs are shown, especially for reusable forms or pages that can render more than one form instance. Convenience hooks such as `use_form(initial)`, `use_form_with_id_namespace(initial, namespace)`, and `use_form_from_core(core)` remain thin wrappers for simple cases and do not create implicit context access.

Optional post-MVP context access is documented in [Form Context Access](form-context.md). It provides an existing **Form Handle** through a typed **Form Context Scope** for descendants that intentionally choose renderless context lookup. It does not replace explicit handles as the primary path, does not create global singleton behavior, and does not reuse **Form ID Namespaces** as context keys.

Hook initialization is not prop synchronization. Initial values are captured when the hook initializes the form for the component instance; later parent data changes do not overwrite the draft automatically. Applications call `reinitialize(new_initial)` when they intentionally replace the baseline and draft. Creating a form does not run validation automatically; initialization validation runs only when the application explicitly requests it.

`FormHandle::managed_submit()` exposes the primary Dioxus-managed submit surface. `SubmitBinding::on_submit(event, submit)` and `SubmitBinding::on_submit_async(event, submit)` accept the Dioxus event supplied to a form `onsubmit` handler, call `prevent_default()` to prevent native browser submission, call `stop_propagation()`, and then run the existing synchronous or asynchronous submission lifecycle. Intentful forms scope the binding first, such as `managed_submit().intent(intent).on_submit(event, submit)`; see [Submit Intent](submit-intent.md).

Stopping propagation is intentional for **Dioxus-Managed Submission**. `FormHandle::browser_submit(action)` exposes **Native Browser Submission** attributes for browser-owned POST without a Dioxus submit handler. `FormHandle::progressive_submit()` exposes **Progressive Submission**: hydrated client code can block known parse, validation, pending-validation, or in-flight blockers, but allowed submits fall through to browser POST and do not create a **Submission Snapshot**. See [Browser Submission Modes](browser-submission.md).

**Hydration Support** means deterministic **Form Initialization** from the same **Form Configuration** across server and client renders, including explicit **Form ID Namespaces** for stable, collision-resistant derived IDs. It does not serialize or transfer the **Form Draft**, field metadata, **Validation Errors**, parse state, or submit state across the SSR/client boundary by default. Opt-in form-state snapshots are a separate explicit transfer path documented in [Form State Serialization](form-state-serialization.md).

## Preserved Extension Points

Several features are designed-for but intentionally post-MVP. Public names and state concepts should leave room for them, but the first implementation should not add their full behavior.

**Collection Fields** started post-MVP. The first issue #36 slice supports direct `Vec<Item>` fields, and issue #92 extends that slice to nested direct collection paths composed through named struct traversal. Broader array and list helpers remain deferred. **Field Identity** and metadata design remain compatible with future **Collection Fields** where item metadata follows items across insertions, removals, and reordering.

The collection slice supports direct `Vec<Item>` fields on a **Named Form Struct** plus nested direct `Vec<Item>` paths reached with `FieldPath::join`. It still defers maps, sets, arrays, collection traversal through collection item hierarchies, enum-variant collections, and **Dynamic Forms**. Collection item metadata follows library-owned **Collection Item Identity** rather than rendered index or application-provided row keys; see [ADR-0002](adr/0002-use-library-owned-collection-item-identity.md) and [Collection Fields](collection-fields.md).

Opt-in **Form State Serialization** can now include runtime **Collection Item Identity** sequences for tracked collections so collection item metadata and validation errors can round-trip across explicit state transfer. This does not change deterministic **Hydration Support** defaults and does not serialize submit-scoped state, observer registrations, validator closures, Dioxus tasks, or adapter parse bindings.

True multi-select **Fields** are supported for direct `Vec<Value>` **Fields** through headless helpers built on the first **Collection Field** slice. Multiple independent boolean checkbox **Fields** remain the right model when each checkbox has its own durable **Field** value in the **Form Draft**. A single **Field** containing many selected values uses **Collection Field** semantics for selected-item identity, per-item metadata, dirty tracking, **Field Validation** attachment, **Reset** behavior, and future reordering behavior.

**Async Validation**, **Debounced Validation**, and **Validation Runtime** integration are implemented through a renderer-agnostic **Form Core** state machine and the first Dioxus runtime integration. The core owns snapshots, statuses, stale-result protection, and submit blockers without depending on Dioxus task spawning or timers. See [Async And Debounced Validation](async-validation.md).

**Validation Adapters** remain outside the **Form Core** and **Facade Crate** dependency surface. The first first-party adapter, `dioform-garde`, lives in a separate renderer-agnostic adapter crate. Additional external validation libraries can map their diagnostics into the form's shared **Validation Error** type through their own adapter crates.

**File Fields** are implemented as adapter-owned **File Selection**, not ordinary cloneable values in the **Form Draft**. Selected files and platform file handles stay outside form-state snapshots, while Dioxus-managed file-aware submissions and file-selection validators receive explicit file snapshots. Native or progressive browser file-upload POST integration remains separate from this Dioxus-managed slice.

Post-MVP composition and tooling extension points remain open but intentionally narrow: reusable typed field groups, broader user-facing side-effect listener APIs beyond value-replacement, blur, direct hook-owned field binding lifecycle, submit lifecycle, and debounced value-replacement listeners, devtools UI built on **Form Observer** events, broader Dioxus Fullstack and server-response integration, dynamic form models, application-owned drafts, native/progressive browser file-upload integration, broader collection traversal, and validation adapters for external schema libraries.

## Out Of Scope For The MVP

The following are explicit MVP exclusions:

- Maps, sets, arrays, collection traversal through collection item hierarchies, enum-variant collections, and collection shapes beyond direct or named-struct-composed `Vec<Item>` **Collection Field** paths.
- Multi-select shapes beyond direct `Vec<Value>` **Fields**; independent boolean checkbox **Fields** remain supported.
- Additional **Validation Adapter Crates** beyond the first-party `garde` adapter.
- Dioxus Fullstack-specific submit helpers, server functions, or transport APIs.
- Browser submission support beyond POST-oriented **Native Browser Submission** and **Progressive Submission**, including file-aware multipart helpers and automatic server rejection mapping.
- Treating user-selected files as ordinary cloneable **Form Draft** values, or serializing and restoring platform file handles through **Form State Serialization**.
- **Dynamic Forms**, because the primary API is a compile-time **Form Model** with typed **Field Paths**.
- Whole-model schema coercion (a `parseValuesWithSchema` analog). Coercion stays a per-field **Input Parsing** concern owned by the **Dioxus Adapter**; the statically typed **Form Model** has no raw-versus-output type to reconcile, and normalizing an already-typed value is an application step or value-replacement **Form Listener**. See [ADR-0017](adr/0017-decline-whole-model-schema-coercion.md).
- **Application-Owned Drafts**, because the MVP form owns its **Form Draft** and **Baseline Value**.
- A can-submit-when-invalid opt-out (a `canSubmitWhenInvalid` analog). **Submit Availability** stays an unconditional "no known blockers" signal; save-draft flows use intent-scoped submit validation and server-authoritative flows use **Submit Errors**, so no validation-bypass path is added. See [ADR-0019](adr/0019-decline-can-submit-when-invalid-opt-out.md).
- Renderless field components or implicit global form access. Optional typed context access exists only as scoped Dioxus subtree lookup over existing **Form Handles**.
- Automatic form-state transfer for SSR or hydration; **Hydration Support** means deterministic initialization unless the application explicitly captures and restores a `FormStateSnapshot`.
- Styled input components and collection-aware input helpers. Post-MVP headless select, radio, numeric, and date-oriented helpers are documented in [`input-helpers.md`](input-helpers.md).
- `no_std` support.
- Multi-version Dioxus compatibility; the MVP **Dioxus Adapter** targets Dioxus `0.7`, while the **Form Core** remains insulated from Dioxus version churn.
- Devtools UI beyond lightweight **Form Observer** events.
- Listener APIs beyond the value-replacement, blur, direct hook-owned field binding lifecycle, submit lifecycle, and debounced value-replacement slices documented in [`form-listeners.md`](form-listeners.md), including collection item child binding lifecycle coverage. Validators should remain pure validation behavior; applications can still use normal Dioxus event handlers for purely local UI behavior.
- Reusable typed field group composition APIs. Applications can extract ordinary Dioxus components that receive explicit **Form Handles** and **Field Paths** in the MVP.

## Why Headless Stays Headless

Dioform must remain a **Headless Form Library**. It provides behavior and state through the **Form Core** and **Dioxus Adapter**, but applications own markup, styling, layout, visual components, and UX copy.

The library may provide **Accessibility Helpers** for IDs and ARIA relationships because those are behavior-adjacent and help users build correct forms without forcing a design system. It should not provide styled text inputs, buttons, field wrappers, error callouts, or a UI kit. Styled components would couple the crate to visual conventions, CSS choices, accessibility tradeoffs, and app-specific layout needs that are outside the library's purpose.

Renderless field components remain outside the MVP. Explicit **Form Handles** keep dependencies visible, make multiple forms straightforward, and match ADR-0001's separation between the renderer-agnostic core and the Dioxus-specific adapter. The post-MVP context API preserves that shape by providing existing handles through typed **Form Context Scopes** instead of global or type-only lookup.

## Pre-Stable Public API Decisions

Before a stable release, the facade keeps ordinary user-facing imports at the crate root and in `dioform::prelude`, while low-level core/runtime/serialization types are exposed through `dioform::advanced` or the `dioform-core` crate. Advanced mutation through the Dioxus adapter is named `FormHandle::write_advanced` to distinguish it from narrower field update APIs that preserve adapter invariants more locally.

`FieldIdentity` and `CollectionItemIdentity` are opaque public values. Applications can compare them, use their accessor methods, and use `CollectionItemIdentity::key()` for rendering keys or diagnostics, but should not depend on enum variants or numeric reconstruction.

`#[derive(Form)]` remains intentionally scoped to non-generic named structs with direct field accessors, field-level `#[form(name = "...")]`, field-level `#[form(skip)]`, and form-level `#[form(rename_all = "camelCase")]` for rendered **Field Name** segments. The `rename_all` support is only the **Field Name Policy** slice of issue #39; richer traversal such as automatic nested accessors, tuple traversal, and variant-inner paths remains future work and should be additive or explicitly pre-stable before a stable release.
