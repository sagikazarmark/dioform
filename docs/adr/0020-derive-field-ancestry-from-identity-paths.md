# Derive Field Ancestry from identity paths

Writing a **Field** must reach the **Fields** it contains and the **Fields** that contain it. Today no
such relation exists anywhere in the workspace: `FieldBindingCore::read_value` tracks
`path.identity()` verbatim, `FormReactivity::field` is an exact `BTreeMap` lookup,
`SelectorTransition::FieldValueChanged` carries one identity, and in **Form Core**
`replace_field_with_origin` bumps only the written identity's version. Dioform will add
**Field Ancestry** as a relation *derived* from **Field Identity** paths, and will apply it at the
notification and staleness sites rather than by fanning out stored state.

This records two decisions taken together, because the second is only defensible given the first:
how the relation is represented, and where it is applied.

## Derive the relation; do not store a parent chain

**Field Ancestry** is decided by comparing identity paths with a separator-anchored segment test, so
`counterparty` does not match the sibling `counterparty_account`. The rejected alternative was
structural parent links on `FieldIdentity` — an `Option<Rc<FieldIdentity>>` parent, or a normalized
segment vector — recorded at `FieldPath::try_join`, which has both identities in hand.

The structural alternative is not rejected on cost. Measured, all three layouts are 48 bytes:
`FieldIdentityKind::CollectionItem` already dominates, and `Option<Rc<_>>` niche-packs into the
`Static` variant with room to spare. Two arguments rule it out instead.

**Derived equality.** `FieldIdentity` derives `Eq`, `Ord`, and `Hash`, and is the key of
`FieldStore::{versions, metadata, collections}` and of `FormReactivity::fields`. A stored parent field
joins those derives, so `FieldIdentity::new("invoice.customer")` would compare unequal to the joined
identity for the same field, silently splitting the version and subscriber maps. Excluding the field
from the derives means hand-writing `Eq`/`Ord`/`Hash` to ignore a member of a type used as a map key
throughout the workspace — a permanent footgun.

**Totality.** Identities arrive from more construction sites than `try_join`: `FieldIdentity::new`,
`CollectionItemFieldAddress::identity_from_static_segments`, and serde `Deserialize`, which funnels
arbitrary strings into `Static { path }` and does not round-trip a parent chain. A stored chain is
*partial* over these paths; a derived rule is *total*. The paths where a stored chain would silently
lose the relation are hydration and state restore — the hardest to test and the ones
`docs/form-state-serialization.md` exists to protect.

The cost is a contract: the dot becomes the **Identity Path Separator**, reserved. `FieldIdentity::new`
is public and unconstrained today, and `crates/dioform-core/tests/form_core.rs` already hand-builds
`FieldIdentity::new("counterparty.name")` as a single *flattened* field for an `Option<Party>`, which
under this decision gains a relation to `counterparty`. The rule therefore belongs in
`FieldIdentity::new`'s rustdoc with a `debug_assert`, not only in a design note. The `try_join` rustdoc
claiming joined paths "are interned for the lifetime of the process" is incorrect — `owned_segment` is
a bare `.into()` and `join_static_path` allocates on every call — and is corrected as part of this work.

The relation ships as one **symmetric** predicate in `dioform_core::__private`, doc-hidden. The
precedent it follows from `CollectionItemFieldAddress` is the export — a doc-hidden `__private`
re-export of a type two first-party crates share — not the type's shape: that one carries derived
addressing data, while this one is a fieldless namespace holding a single associated function. Both
**Form Core** and the **Dioxus Adapter** need it and Rust has no cross-crate `pub(crate)`, but the
payoff today is internal: two first-party crates agreeing on one rule.
[ADR-0018](0018-decline-public-validation-adapter-trait.md) declined publishing a seam whose entire
payoff was internal dedup; the same test applies here. Promote it to documented public when a second
renderer adapter or a real application need appears.

A namespace rather than a `FieldIdentity` method, because `FieldIdentity` is public API: a method
would have to be a `#[doc(hidden)] pub fn` on a type applications hold, which advertises the seam on
the very surface this decision is keeping it off. Swappability does not decide this — a method would
be equally swappable — so it is the export surface that does. It stays a predicate either way — no
`parent()`, `segments()`, or `depth()` accessors — so the representation remains swappable if
[ADR-0002](0002-use-library-owned-collection-item-identity.md)'s deferred map and array traversal ever
make segments stop being `.`-splittable. One symmetric predicate rather than a directional pair,
because every call site needs ancestor-or-descendant-or-equal and asking callers to reason about
direction is asking them to reason about the grammar.

### The collection clause is strict, and the empty segment is the item root

For `FieldIdentityKind::CollectionItem`, `static_path()` is `None`, so the relation needs explicit
clauses:

- same collection and same item, with segment-ancestry on the `field` component
- a static path that is a **strict** ancestor of the `collection` component
- `File` relates to nothing but itself

Within-item ancestry is not hypothetical: `CollectionItemFieldAddress::identity_for` accepts any
`FieldPath<Item, Value>`, including a joined one, and the derive-contract test
`nested_collection_paths.rs` already asserts `FieldIdentity::collection_item("invoice.lines", item,
"product.name")` through both **Form Core** and a Dioxus binding. Writing `lines[i].customer` must
reach `lines[i].customer.name`.

The collection clause must be strict ancestry, not ancestor-or-equal. With equality,
`CollectionStructureUserChanged` synthesizes `FieldValue` for the collection identity, that reaches
every item-child reader, and appending a row re-renders every existing row's value reader — the
contract `collection_structure_selectors_rerender_without_rerendering_item_value_readers` exists to
prevent. Equality is already handled imperatively where it is wanted, in
`replace_collection_item_field_with_origin`. That test is the guard for this decision and must stay
green.

An empty `field` segment is the item root and an ancestor of every non-empty sibling segment, which
subsumes `is_collection_item_value` without a separate clause. This is established semantics, not new:
`replace_collection_item_with_origin` already clears submit errors for every child of a replaced item
via `CollectionItemFieldAddress::matches_item`.

This stays inside ADR-0002's slice. It adds no addressing capability — every shape it covers is already
constructible and already asserted. Nested collections inside items, maps, sets, arrays, and
enum-variant paths remain deferred and remain blocked by the existing derive-contract failure test.

## Apply the relation to notification and staleness, not to stored versions

**Selector expansion is `FieldValue` only.** `apply_field_mutation` already calls
`notify_validation_changed()` on every field mutation, and the `ValidationChanged` arm fans out
`FieldValidationErrors` and `VisibleFieldValidationErrors` for every tracked identity. Expanding those
would add a redundant second wake on the hot path. `FieldMetadata` and `FieldParseErrors` stay
unexpanded: `set_user_field` marks only the written path touched, and an ordinary write does not clear
parse blockers for the written field either, so expanding them would invent a contract rather than
restore one. The four composite transitions in the adapter must pass their real tracked list rather
than an empty one; the structure contract is preserved by the strict collection clause above, not by
an empty list that also suppresses legitimate ancestor wakes.

**Field versions are not fanned out.** [ADR-0010](0010-carve-form-core-into-field-store-submission-state-and-chain-executor.md)
established that version has exactly one owner and that every carved module is a reader of
`FieldStore::version`, never a writer. Bumping relatives' versions would redefine `version(f)` from
"f was written" to "f or something near f changed", and it would not even work: **Field Registration**
is lazy, so the versions map holds only identities that were actually incremented and an absent read
returns `0`. A write-time fan-out cannot materialize a never-touched descendant, so a submit error
targeting a field the user never touched still compares `0 == 0` and is wrongly stored. Instead
`submit_error_applies_to_current` walks the target's ancestry at store time, which is total regardless
of registration order and leaves ADR-0010's invariant intact. This governs `SubmitError::field_identity`
only; `SubmitError::field` carries a whole-value comparison closure and is already ancestry-correct.

**Write-time submit-error clearing widens symmetrically.** `CONTEXT.md` defines a **Stale Submit Error**
as one that "refers to a field value before the field changed", and a write to `invoice.customer.name`
changes the value of `invoice.customer`. Symmetric clearing is that definition applied consistently.
It is also what the library already ships through the canonical `SubmitError::field` constructor, whose
store-time closure drops a parent error when any child differs — so the current behavior is not a
semantic worth preserving but an inconsistency decided by network timing: editing a sibling field just
before the server response drops the parent error, just after keeps it forever. Directional asymmetry
(clear descendants, keep ancestors) was evaluated and rejected: it is coarser than the existing value
comparison where it acts, absent where it does not, and it leaves the worse symptom unfixed.

That symptom is not cosmetic. Stored submit errors feed `has_validation_errors()` and therefore
`SubmitBlocker::ValidationErrors`. A leaf-input UI never writes the containing object's path, so a
submit error targeting that object cannot be cleared by any user action and the form stays permanently
unsubmittable. A verdict that must survive edits to the values it was about belongs at form scope,
which no field write clears.

Write-time clearing is necessarily identity-only: `StoredSubmitError` drops the `applies_to_current`
closure at storage, so value comparison is unavailable there. Consequently a parent write that happens
to preserve a descendant's value will still clear that descendant's submit error, where store-time
would have kept it. This over-clears in the safe direction and matches the stance already recorded in
`docs/async-validation.md` — any draft edit stales pending async field validation, "intentionally
conservative and avoids a validation dependency graph".

**Validator re-runs widen the filter, not the pass count.** `validate_field_chain` is expensive per
call: it runs `ensure_all_collection_item_validator_states()` and clones-and-sorts the validator table
several times. Invoking it once per descendant multiplies all of that. The selection inside
`sync_field_keys_for_chain` and `sync_collection_item_keys_for_chain` widens instead, so the cost
stays one pass. Both widen: the item-child cases below are unreachable through the collection-item
table alone.

Widening the selection is what puts relatives in one chain, so the chain's *verdict* has to be
narrowed to compensate. `validate_field_sync_chain` folds every key it ran into a single `valid`
flag, and that flag gates async skip-versus-clear for the written field — so once relatives are in
the chain, a child's failing sync validator would skip its parent's async validators, and through
`with_async_start_sync_gate` would stop them starting at all. The flag therefore reports only the
written field's own validators; relatives run for their errors and notifications, not for their vote.
Widening the filter and keeping the async step per-field are in tension by construction, and this is
where the tension is resolved.

**Async validator restart propagates.** `replace_field_with_origin` invalidates async field validators
model-wide and `mark_stale` clears their errors, while the adapter restarts only the written identity.
Without propagation a parent write silently blanks a descendant's async result permanently — a worse
outcome than the stale renders that prompted this work.

## What is deliberately not in this decision

Expansion iterates the identities already present in the tracked map and filters them by the predicate.
It must never compute ancestors by splitting the written path and notifying them, because
`FormReactivity::field` inserts on lookup and would create entries for untracked identities,
accelerating the unbounded growth of that map.

Replacing the per-write scan with `BTreeMap` range queries is the right end state — `Ord` is derived, so
descendants of `p` are the range from `p` plus the separator — but it is deferred to the issue that
also addresses `FormReactivity::fields` never being pruned, where it belongs. The per-write O(tracked)
cost is already paid today: `tracked_field_identities()` clones every key on every transition and the
`FieldValueChanged` arm discards it.

Field listeners (`field_listeners`, `debounced_field_callbacks`, `field_blur_callbacks`,
`field_binding_listeners`) resolve callbacks by exact identity and are the same class of defect on a
different surface. `reset_field` emits no `ValidationChanged` at all. Both are deferred to follow-up
issues; the predicate this ADR introduces is what makes them cheap to fix.

[ADR-0028](0028-match-listener-reach-to-what-each-event-asserts.md) settles the listener half and
narrows this paragraph on the fix rather than the diagnosis: the four surfaces are one defect class,
but they do not take one reach. Value replacement uses this predicate as written; blur and binding
lifecycle assert containment rather than replacement and reach outward only.

The `reset_field` half is settled without this predicate, and narrows the paragraph above on the
diagnosis rather than the fix: `FormHandle::reset_field` now emits `ValidationChanged` as its last
transition, hand-emitted the way `apply_field_mutation` emits it for every other mutating field
method. That transition already fans validation-error selectors out over every tracked identity, so
the reset needs no ancestry expansion of its own to reach a contained field's, a sibling's, or the
form's validation subscribers.
