# Deepening Plan: Form Core And Dioxus Adapter

This plan records six agreed deepening refactors surfaced by an architecture review of the two monoliths, `crates/dioform-core/src/lib.rs` (7,230 lines) and `crates/dioform/src/lib.rs` (10,888 lines). It uses the domain glossary in `CONTEXT.md` and the deep-module vocabulary (module, interface, seam, adapter, leverage, locality). Each refactor is behavior-preserving unless a linked ticket says otherwise.

The workspace already contains four genuinely deep modules: `AdapterRuntime`, `ManagedSubmission`, `ValidationChainRegistry`, and `validation_lifecycle::SourceState`. The friction is the residue left behind them in the two `lib.rs` files, each anchored by one god-struct: `FormCore` (~229 methods) and `FormHandle` (~106 public methods).

Decisions already recorded: [ADR-0009](adr/0009-add-task-spawner-seam-for-async-validation.md) (Task Spawner seam), [ADR-0010](adr/0010-carve-form-core-into-field-store-submission-state-and-chain-executor.md) (Form Core carve).

## Sequence And Dependencies

The six split into a **core trio** (2, 3, 6) that carve `FormCore` and share field-keyed state, and an **adapter three** (1, 4, 5) that are independent of each other and of the core work.

```text
2 FieldStore ──owns version──▶ 3 SubmissionState
     │                              (reads FieldStore::version)
     └──owns version──▶ 6 ChainExecutor
                              (reads FieldStore::version)

1 TaskSpawner   ┐
4 apply_field_mutation ├─ adapter track, independent
5 ListenerSet   ┘
```

The one cross-cutting invariant that makes the carve cohere: **every carved module reads `FieldStore::version`, none of them writes it.** Version gets exactly one owner, and three separate staleness comparisons collapse to "ask the store." That is why FieldStore (2) is the base and must land before SubmissionState (3) and ChainExecutor (6).

**Recommended order:** 2 → 3 → 6, then 1, 4, 5 in any order. Candidate 1 does not de-risk the core trio (the core is already runtime-free and tested through `form_core.rs` with no runtime), so it is not first.

## Behavior-Preserving Gates

Every step lands green against the existing suite before the next begins:

- `cargo test --workspace`: includes `form_core.rs` (131 tests, pure `FormCore`), `tracer_bullet.rs` (214 tests), the adapter contract tests, and `workspace_layering.rs` (asserts the crate dependency seam).
- No public API change and no new public type on the core or facade crate for steps 2, 3, 5, 6. Step 1 adds the `TaskSpawner` seam; step 4 is entirely private.
- `workspace_layering.rs` must stay green: the carve is internal to `dioform-core`; it must not add a renderer dependency.

Because the interface is the test surface, each carved module gets its own focused tests through its small interface, in addition to the existing suite passing unchanged.

---

## 1 · Task Spawner seam (adapter): DONE

**Files:** `crates/dioform/src/adapter_runtime.rs` (trait + both adapters + tests).

**Problem.** No runtime seam exists; adapter async-validation logic is only reachable through `dioxus_core::spawn`, so tests infer cancellation by drop-counting fake futures.

**Solution.** Extract a `TaskSpawner` interface (`spawn`, `cancel`) carried as `Rc<dyn TaskSpawner>` on `FormHandle`. Thread an explicit spawner-owned task id through `spawn` so the `Runtime::current().current_task()` lookup (adapter_runtime.rs:136) disappears.

- **Adapters (two → real seam):** `DioxusSpawner` (wraps `dioxus_core::spawn`/`remove_future`) in production; `InlineSpawner` (polls the future to completion synchronously, records `cancel` calls) in tests.
- **Out of scope:** timers: already injected as delay futures via `debounce_duration`; the `TaskSpawner` does not touch them.
- **Leverage:** async-validation start/cancel/stale-result logic becomes testable on a bare `FormHandle`; real cancellation becomes assertable.
- **Locality:** the three `dioxus_core` calls concentrate behind one seam.
- Supersede the `adapter_runtime.rs:720` comment with a pointer to ADR-0009.

---

## 2 · FieldStore (core, base of the trio)

**Files:** `crates/dioform-core/src/lib.rs` (new `field_store` module), migrate `FormCore` call sites.

**Problem.** `field_versions`, `field_metadata`, `collection_states` are parallel `BTreeMap`s keyed by **Field Identity**, kept aligned by convention across ~208 methods; readers use `.copied().unwrap_or_default()` because nothing guarantees a metadata entry has a matching version.

**Solution.** One `FieldStore` owning `FieldIdentity → FieldRecord { version, metadata, collection? }`.

- **Interface:** `touch(id)` (materialize + bump), `record(id)`, `metadata(id)`, `version(id)`, `collection(id)`, `is_file(id)`.
- **Invariants encapsulated:** version bumps *with* the write (can't drift); lazy **Field Registration** preserved: reads never allocate, absent field reads as version `0`; the file-field exclusion rule (today re-written in ~5 places in `state_snapshot`) defined once.
- **Boundary:** field-keyed only. Per-validator state stays in `ValidationChainRegistry`.
- **Locality:** version/metadata drift becomes impossible; `unwrap_or_default()` collapses into `version()`.
- Compatible with ADR-0002: library-owned **Collection Item Identity** lives inside `FieldRecord.collection`.

---

## 3 · SubmissionState (core)

**Files:** `crates/dioform-core/src/lib.rs` (new `submission` module), migrate `FormCore`/`FormCoreIntent`.

**Problem.** The submit lifecycle is a real state machine stored as five loose fields mutated at ~77 sites; **Submit Availability** is reassembled from eight scattered predicates (lib.rs 6549-6760) on every read.

**Solution.** A `SubmissionState` owning the five fields plus `submit_errors`.

- **Interface:** transitions (`begin`, `block`, `finish`, `record_status`) + one `availability(intent, pending, versions)` read.
- **Accept dependencies, don't create them:** `availability` takes a pending-validation view and `FieldStore` versions as parameters; it does not reach into the chain, so it is testable with fakes.
- **Intent:** stored erased inside; typed **Submit Intent** applied only at the intent-scoped boundary (preserves ADR-0004).
- **Locality:** illegal submit states become unrepresentable; `FormCore` sheds five fields and ~77 touch-sites.

---

## 4 · apply_field_mutation template (adapter): DONE

**Files:** `crates/dioform/src/lib.rs`: `set_field` (8625), `set_user_field` (8650), `mark_field_blurred` (8688), `select_files` (9075), collection-item variants.

**Problem.** Each mutating method re-implements the write → notify-selectors → (gated) async kickoff → notify-validation → dispatch-listeners sequence; the reactivity invariant lives in copy-paste, not code.

**Solution.** One private `apply_field_mutation` that fixes the ordering; each public method supplies a `FieldMutation` descriptor (selector transitions, trigger, dispatch kind).

- **Strictly behavior-preserving:** reproduce each method's current steps exactly. (The one deliberate divergence, `notify_validation_changed`, was carried unchanged through the refactor and then decided separately; see Finding A. Do not normalize behavior inside a structural refactor.)
- **Locality:** notify/validate ordering and completeness fixed in one place; a forgotten selector notification becomes impossible.

---

## 5 · ListenerSet (adapter): TRACKED as #127

Deferred to a focused follow-up: [issue #127](https://github.com/sagikazarmark/dioform/issues/127). Lowest-value of the six (locality only), largest surface (~1,000 lines), and behavior-preserving: a good fit for a dedicated pass.

**Files:** `crates/dioform/src/lib.rs`: listener subsystem (2634-3700), `FormListeners` (2723), 8 RAII registration types (3225-3502).

**Problem.** Eight **Form Listener** kinds each carry a full parallel type family (Entry, RegistrationInner, Registration, Context) and a register/unregister/callbacks triple: ~1,000 lines of near-identical mechanism.

**Solution.** One generic `ListenerSet<Context>` owning the repeated mechanism (id allocation, `Vec<Entry>` storage, a single `Subscription` RAII with one `Drop`, dispatch/callbacks). The eight kinds become thin instantiations; debounce and field-binding lifecycle compose their extra state *around* a `ListenerSet`, not inside a mega-generic. No macro (macro-generated types are worse for navigability than one generic).

- **Framing:** this is a **locality/duplication** win, not a depth win: the listener interface is already small. A new immediate listener kind becomes one `ListenerSet<Ctx>` field plus two forwarding methods.
- Behavior-preserving.

---

## 6 · ChainExecutor (core, speculative): REJECTED on implementation

**Outcome:** not extracted. See [ADR-0011](adr/0011-do-not-extract-a-chain-executor-module.md).

On close inspection the chain orchestration does not carve cleanly: running one field validator
(`validate_field_validator_key`) needs the draft model, field-store metadata, submission intent, and
mutable registry access simultaneously. A `ChainExecutor` module would borrow four subsystems at once
and relocate the coupling rather than concentrate it; it fails the deletion test. The submit-side win
was already captured by candidate 3, which moved the submit-availability queries onto
`ValidationChainRegistry`. The remaining orchestration legitimately lives at the coordinating
`FormCore` layer.

---

## Separate Findings (not refactors)

These surfaced during design and must be decided on their own merits, each with its own test: do not fold into the structural refactors above.

**Finding A: `notify_validation_changed` divergence** (resolved in [issue #129](https://github.com/sagikazarmark/dioform/issues/129)). `set_field`/`set_user_field` called `notify_validation_changed()` unconditionally, but `mark_field_blurred` called it only when the blur ran validation. **Decision: blur was under-notifying.** A blur that runs no validation still flips the blurred/touched metadata that gates blur- and touch-scoped **Error Visibility** (`should_show_validation_errors`), so it can change what a validation subscriber should see, exactly like a value write clearing submit errors. The two paths now converge on one rule: **every field mutation notifies validation subscribers.** The `notify_validation_when_idle` flag on `FieldMutation` is removed; the pinning test is `field_mutations_notify_validation_subscribers_even_without_running_validation` in `crates/dioform/tests/tracer_bullet.rs`.

**Finding B: reverse the "no pluggable runtime trait" note.** Recorded as ADR-0009. The `adapter_runtime.rs:720` comment stating the runtime boundary is "not a pluggable runtime trait" is superseded; replace it with a pointer to the ADR when candidate 1 lands.
