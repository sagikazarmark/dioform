# First-class Dioxus integration: signals-first dioform, `dioxus-field`, and custom registries

Status: accepted plan, rev 4 (2026-08-27). Tracking issue:
[#81](https://github.com/sagikazarmark/dioform/issues/81).
Focus Exit compatibility is recorded by
[ADR-0050](../adr/0050-map-field-convention-focus-exit-without-validation.md).

This document is the in-repo source of truth for the Dioxus-integration initiative. It condenses two
rounds of verified research (three parallel research passes each, plus an adversarial judge pass that
spot-checked every load-bearing claim against primary sources) into the plan the implementation
issues reference.

## Verdict

The reported integration pain is real, and the fix is two structural changes in dioform proper — not
a glue crate:

1. **Structural `PartialEq` for Field Paths.** `FieldPath::direct` already takes plain fn pointers
   and only erases them into fresh `Rc<dyn Fn>` allocations inside the constructor
   (`crates/dioform-core`). Keeping a `Direct { get: fn, get_mut: fn }` accessor variant makes
   derived-path equality structural and *true* — `Model::fields().street() ==
   Model::fields().street()` — the decision recorded by
   [ADR-0047](../adr/0047-make-field-paths-interchangeable.md), superseding
   [ADR-0030](../adr/0030-decline-partial-eq-for-field-paths-and-bindings.md). Composed paths
   (`join`, `.or`, and mounted group maps) compose through capturing closures and fall back to
   `Rc::ptr_eq` clone-of equality, documented. The comparison
   `identity == && name == && get_ptr == && get_mut_ptr ==` is never falsely equal (ICF can only
   merge behaviorally identical accessors; cross-CGU duplication causes only false *inequality* — a
   missed memoization, never staleness). Collection bindings stay excluded: the row-subscription
   hazard (ADR-0047 §collection-bindings-remain-excluded) is untouched by any equality semantics.
2. **Signals-first field views.** Dioxus's `Readable` trait is unsealed; `ReadSignal::new` boxes any
   implementation; first-party `dioxus-stores` demonstrates the exact pattern. Dioform field views
   can implement `Readable` + `SuperInto<ReadSignal>` so a binding is passable *directly* as a
   signal-typed prop — no per-field `use_memo`. Constraints verified in dioxus 0.7.10 source: reads
   must return a generational-box ref, so each exposed field needs a `CopyValue` cache slot in a
   form-owned `Owner`; the props memoizer calls `.peek()` on signal-typed props at diff time (peek
   must be side-effect-safe and never subscribe); recompute must be dirty-gated Memo-style. Slots
   are interned by `(Field Identity, fn-pointer pair)` — which is why the equality work lands first.

On top of those, a form-library-agnostic **Field Convention** crate (`dioxus-field`) and custom
**Widget Registries** built against it. dioxus-primitives is *not* the integration target (registries
are custom); its research value is design intelligence — see "Ecosystem findings".

## Ecosystem findings (research round 2)

The `dioxus-field` niche is **empty and essentially undiscussed**:

- Closest artifacts: dioxus-primitives' internal `use_controlled` idiom (two-tier, no context tier,
  no commit, no field meta, unpublished); the dormant `dioxus-forms` crate's per-field bind tuple
  (internal to one form library); the stalled cross-framework `ars-ui` (Ark-style, native-validity
  classification, quiet since 2026-06).
- Validated prior art elsewhere, none neutral: Thaw's `Model<T>` + crate-private `FieldInjection`
  (Leptos — the Rust template for the value port and precedence); Ark UI's meta-only Field context
  (mergeProps precedence, `aria-errormessage` + live region, MutationObserver hack for id
  presence); Base UI's rich field layer (per-flag prop-overridable meta, a commit-style validation
  entry point) kept deliberately closed — its docs literally name external-form-library-driven
  `invalid` as a use case. Radix Form died from coupling validity to native constraint validation.
  React Hook Form's `Controller` won because its contract costs widget authors zero.
- Live upstream thread: [components#199](https://github.com/DioxusLabs/components/issues/199)
  (2026-02) — the maintainer endorses React-Aria-style public hooks with an explicit
  state-level/rendering-level split and root-provider/child-consumer contexts: the same architecture
  one level down. Forms umbrella: [components#5](https://github.com/DioxusLabs/components/issues/5).
  Fullstack angle: [dioxus#1996](https://github.com/DioxusLabs/dioxus/issues/1996).
- Cautionary pattern: ownerless Rust convention crates die (RustForWeb radix archived 2026-02;
  ars-ui stalled). Survival modes: single-owner substrate (our registries) or upstream adoption —
  the design keeps both open by being donatable.

## The layering

Four layers, separated by what each is allowed to know. `dioxus-field` is **not** a form library:
no model, no submission, no "all the fields". `Field` must work wrapping a bare `use_signal` with no
dioform in sight — that is the test that the layering holds.

| Layer             | Knows Dioxus | Knows forms | Knows styling |
| ----------------- | ------------ | ----------- | ------------- |
| `dioform-core`    | no           | yes         | no            |
| `dioform`         | yes          | yes         | no            |
| `dioxus-field`    | yes          | no          | no            |
| custom registries | yes          | no          | yes           |

## Settled design points

- **Commit is not a renamed blur.** A **Commit** is the widget-defined end of one interaction unit,
  with three origins: focus leaving the widget's focus scope, widget-state transitions (popup close,
  slider drag-end, combobox Enter), and form submit. On the dioform side `on_commit()` feeds the
  Commit **Validation Trigger** with outward-only validator reach; **Focus Exit** separately feeds
  exact touched and blurred metadata plus blur listeners without validation.
- **Change Origin**: `User | Programmatic`. `User` implies touched-marking and user-event validation
  semantics; `Programmatic` maps to dioform's **Programmatic Update** (dirty possible, not touched).
  Prop-trio-only widgets imply `User`.
- **Error text crosses the boundary as pre-rendered display strings.** `FieldMeta` carries
  signal-backed `errors: Vec<Rc<str>>` alongside the `invalid` flag (`invalid` defaults to
  `!errors.is_empty()` but is independently overridable). The producer formats: dioform's `meta()`
  uses `Error: Display` by default with a formatter override. dioxus-field never sees a typed error.
- **Explicit exclusions for dioxus-field**: no initial-value tracking (baseline is form knowledge —
  dioform's **Baseline Value**), no validity classification (`invalid` is a dumb producer-set flag;
  native-ValidityState coupling killed Radix Form), flag semantics are producer-defined (`touched`
  sticky, `dirty` value-equality).
- **Dioxus version reality**: latest stable is 0.7.10 (2026-07-30). The
  [dioxus#2467](https://github.com/DioxusLabs/dioxus/issues/2467) listener-passthrough fix (#5554)
  landed 2026-08-20 and is in **no released version**. Field parts and registry widgets therefore
  use the 0.7.10-compatible pattern — listener attributes (`onblur(cb)` etc.) inside an explicit
  `attributes:` vec, or explicit `Option<EventHandler>` props — with bare handler props through
  `extends` noted as the upgrade path. Watch for silent first-listener-wins collisions (dioxus-core
  dispatches only the first listener of a name per element; blur is non-bubbling).

## Phase 1 — Supersede ADR-0030, scoped to scalars (small–medium)

[ADR-0047](../adr/0047-make-field-paths-interchangeable.md) records this decision.

- `Direct { get: fn, get_mut: fn }` variant in `FieldPathAccessor`; `Rc<dyn Fn>` stays for composed
  paths.
- `impl PartialEq for FieldPath` in core (structural for direct accessors, `Rc::ptr_eq` fallback for
  composed) — this *is* the named interchangeability capability; `FieldPathAccessor` stays private.
- Scalar binding equality as `handle == && path ==`. Collection bindings excluded; rows keep
  handle + **Collection Item Identity** props.
- Tests: `fields().x() == fields().x()`; `.or(&a) != .or(&b)`; mounted-map clone-equality; a loud
  failure if codegen changes degrade fn-pointer comparison. Clippy
  `unpredictable_function_pointer_comparisons` gets a reasoned `#[allow]`.
- Superseding ADR answers ADR-0030's three open questions: clone-of survives only for composed paths
  and is documented; the capability is core's own `PartialEq`; collection-row subscription is
  deliberately out of scope. Unblocks the scalar half of issues #43/#44.

## Phase 2 — Signals-first field views (medium–large)

- Per-field view types implementing `Readable<Storage = UnsyncStorage>`: dirty-gated Memo-style
  recompute into a `CopyValue` cache slot, subscription through dioform's existing `Subscribers`,
  side-effect-safe peek.
- Slots interned by `(Field Identity, fn-pointer pair)` in a form-owned `Owner` held by the
  `FormHandle` (exact form lifetime, no scope coupling, no per-render leaks). Composed paths get
  per-instance slots; hoist to share.
- `SuperInto<ReadSignal<T>>` with a dioform marker so a view is passable literally as a signal prop.
- Tests: nested reads (no borrow panic), peek-at-diff-time, slot-count stability across renders,
  child memoization (value-unchanged signal props must not re-render the child).

## Phase 3 — `dioxus-field` (medium)

Hosting, recorded by
[ADR-0048](../adr/0048-incubate-dioxus-field-in-the-workspace-with-an-extraction-trigger.md): incubate
as a dioform workspace member (`crates/dioxus-field`) with hard guardrails — zero
dioform dependencies including dev-deps (compiles and tests standalone; cross-crate tests live in
`dioform-integration-tests`), own version/README/changelog, no dioform vocabulary in its API or
docs. **Extract to a standalone repository before going public-facing** (before the crates.io 0.1
and before the formal upstream proposal), via subtree split with history.
Donatability to dioxus-primitives is a standing design constraint.

- **Two-level contract.** Lower: the documented **Binding Prop Trio** — `value: ReadSignal<T>`,
  `on_change: Callback<T>`, `on_commit: Callback<()>`, names fixed by the conformance kit — which
  any registry satisfies with zero dependency (matches the vocabulary upstream standardized in
  components PR #40: read-only signals + callbacks). Upper: **Value Binding** (`Binding<T>`) as the
  ergonomic carrier — `read` as a `ReadSignal<T>` (from Phase 2 views), `write(T, ChangeOrigin)`,
  `commit()`, optional `focus_exit()`, and identity from Phase 1's sound identity. Binding decomposes
  into the trio, which intentionally carries no Focus Exit capability. Rich `From`
  conversions on the Thaw `Model<T>` template: `Signal<T>`, `(ReadSignal, Callback)`, plain `T` →
  uncontrolled.
- **Field Context** carries signal-backed **Field Meta** (ids, name, required/disabled, invalid,
  errors, touched, dirty) — Dioxus context is not reactive, so a plain struct would leave
  `FieldError` stale. **Binding Resolution**: explicit prop > Field Context > internal uncontrolled
  signal; applies to meta too — at minimum `invalid`/`disabled` per-flag prop-overridable. Meta and
  value contracts are independently adoptable; every part works with no `Field` ancestor.
- A minimal **Focus Request** slot in Field Context (widget registers a focus callback; *which*
  field to focus stays the form library's decision). In v1 — adding a slot to a published contract
  later is a breaking change.
- Headless `Field` / `Label` / `FieldDescription` / `FieldError`; `FieldError`/`FieldDescription`
  register their ids into the signal-backed meta on mount/drop (the Dioxus-native replacement for
  Ark's MutationObserver hack). Field Meta → attribute helper: `aria-invalid`,
  `aria-errormessage` + polite live region when invalid, `aria-describedby` chaining, `data-*`.
- **Conformance kit** with named tests: commit is synchronously observable before submit handling
  runs; write carries origin; optional Focus Exit is exact and ordered; resolution precedence holds;
  focus round-trip; error/description ids appear in meta on mount and vanish on drop.
- dioform producer surface (in dioform, not dioxus-field): `From<CheckboxBinding<M, E>> for
  Binding<bool>` and friends; `binding.meta()` (Display-based error formatting, override
  available); `on_commit()` feeding the Commit trigger; Focus Exit feeding exact blurred/touched
  metadata and blur listeners without validation. `FieldAccessibility` demoted to producing Field
  Meta.

## Phase 4 — First custom registry + demo gallery (medium–large)

- Registry components authored to the convention — the same opening everywhere: `use_binding`,
  `use_field_meta`, attribute spread. Registry authors implement interaction and ARIA; they never
  think about forms. Registry lives in its own repository (name/location: owner's call).
- Proposed v1 widget list (owner to confirm): checkbox, switch, text input, textarea, select
  (single), radio group, slider. v1.1: combobox, multi-select, date picker, tag group.
- Commit implemented at widget-defined interaction boundaries; Focus Exit implemented from the
  widget's complete logical focus scope, including owned popup content.
- Real hidden native inputs driven by `FieldMeta.name` — restoring **Progressive Submission** /
  `BrowserSubmitBinding` (dioxus-primitives cannot offer this today; only its Checkbox/Switch have a
  functional `name`).
- Tri-state checkbox binding in dioform (`Option<bool>` path; cycle policy stays app-side).
- Demo gallery page in `demo/` rendering a full form from the registry — the conformance kit's
  living surface and the artifact for ecosystem advocacy.
- Primitives research applied as design intelligence: no trio-shaped "`None` means uncontrolled"
  trap; controlled-empty always expressible; explicit event-handler props where collisions lurk.

## Phase 5 — Ecosystem proof + upstream (small, parallel, optional)

- Compatibility demo: Phase 2 views driving *stock* dioxus-primitives directly — proof the
  signals-first layer works for registries that never adopt the convention.
- Seed components #199 first (pitch: the field-scoped instance of the state/rendering split the
  maintainer already endorsed), then #5; open a formal proposal issue only once those threads carry
  the context. Upstream reimplementing the contract is a win — dioform's moat is form state.
- Publish `dioxus-field` 0.1 to crates.io early (a real 0.1, not a placeholder) to hold the name.
  The Slider `on_value_commit` PR upstream stays cheap and worthwhile.

## Risks

| Risk | Containment |
| --- | --- |
| `Readable` impl subtleties: diff-time peek, dirty-gating, re-entrancy | Clone the dioxus-stores architecture; dedicated tests for nested reads and the memoize path. |
| Signal-slot leaks from bad interning | Phase ordering: fn-pointer identity (Phase 1) is the intern key. Slot-count stability test. |
| fn-pointer comparison unpredictability across codegen | Perf-only failure mode (false inequality). Equality tests fail loudly on regression. |
| `dioxus-field` scope creep (a widget or `Form` lands in it) | Hard content rule; the conformance kit owns the contract, not new components. |
| Stale field UI from non-reactive context | Signal-backed Field Context; test `invalid` flipping propagates to `FieldError` without a parent re-render. |
| Ownerless-convention death spiral | Single-owner registries as substrate; donatability as design constraint; publish a real 0.1. |
