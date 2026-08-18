# Mint collection item identities from a never-rewinding counter

`reset()` and `reinitialize()` both call `FieldStore::clear()`, which drops every `CollectionState`
alongside field versions and metadata. The next `collection_items()` runs `ensure_collection_state`,
which builds a fresh `CollectionState` minting **Collection Item Identities** from zero in positional
order. Identities are therefore *reused*, and a retained binding for old identity `N` starts addressing
whatever logical item now occupies that identity — with `is_resolved()` reporting `true` while it does
so. Reproduced against a plain **Form Core** with no `VirtualDom`: after a `reinitialize`, a retained
leaf binding reads `""` and reports unresolved, then flips to `true` and reads a *different* invoice's
row as soon as anything calls `items()`, and a write through it lands on that row.

Dioform will mint every **Collection Item Identity** from a per-collection counter that never moves
backward. `reset()` restores `current_items` from the `baseline_items` it already stores;
`reinitialize()` allocates fresh identities above the counter; `restore_state_snapshot()` adopts the
snapshot's identities and advances the counter to `max(live, snapshot)`. `FieldStore::clear()` stops
wiping the collections map. No number is ever issued twice.

## One rule, not a rule per operation

The tempting shape is a split — reset preserves identities because it restores the same baseline,
reinitialize invalidates because it installs a new one, restore is left alone because it is deliberately
identity-preserving. It does not survive contact with itself.

`CollectionState::new` is the only re-mint path and always starts at zero, so "reset keeps positional
identities" re-mints `[0,1,2]` even when a prior `reinitialize` had already retired those numbers and
moved the baseline to `[12,13,14]` — the fix reproducing the defect it exists to close. The existing
integrity check does not catch it either: `into_collection_state` rejects a counter at or below the
maximum *live* identity, never one below a *retired* one. Carrying a counter forward is only meaningful
when paired with never minting below it, which is one rule rather than a branch of three.

The split also inverts the thing it claims to track. `reinitialize` is the save-then-adopt call — the
application POSTs, the server returns the persisted model, the application adopts it — where the rows
are the same logical items the user is still looking at. `reset()` is the call that discards the user's
work outright. A rule that invalidates identity on the first and preserves it on the second tracks
nothing a user would recognise. `reinitialize(baseline.clone())` makes this sharp: every observable is
unchanged, yet the whole collection would renumber, making identity observable through an operation with
no observable effect — the failure mode [ADR-0024](0024-compare-form-handles-by-observable-identity.md)
exists to prevent.

Nor could an application evaluate the split's predicate. It turns on "was this a baseline row", and
[ADR-0002](0002-use-library-owned-collection-item-identity.md) makes the identity opaque with `key()` as
its only accessor. [ADR-0022](0022-represent-an-absent-binding-target-in-the-return-type.md) rejected a
per-case answer on exactly this ground — "answering collections differently would give the library two
answers to one question" — and [ADR-0023](0023-resolve-the-rendered-collection-item-index-live.md)
states the invariant unqualified: a **Collection Item Identity** denotes one logical item for as long as
any binding holds it. This decision is that sentence implemented.

## The crate already made this choice for its other allocator

`restore_state_snapshot` calls `advance_next_validator_id_to_at_least`, which is a `max` and never a
rewind, so a restored snapshot can never cause a `ValidatorId` to be issued twice. That call sits in the
same function that installs collection identity state wholesale. The rule here is the one already in the
file, applied to the allocator that was missed.

`clear_collection_items_with_origin` is the second precedent: clearing a collection empties
`current_items` and deliberately leaves `next_item_identity` alone. Monotonic allocation is already the
contract for the collection-clear operation. `reset()` and `reinitialize()` are the inconsistency.

## Identity lifetime is not the Form Version's job

The alternative was to make the existing **Form Version** participate in resolution, so a binding minted
under an older version never resolves. `form_version` is a mutation counter, not a lifecycle epoch: it
increments in `replace_field_with_origin`, `replace_collection_item_field_with_origin`, and
`after_collection_mutation`, so a binding stamped with it would go unresolved on the next keystroke, and
appending a row would invalidate every other live binding in the collection.

A dedicated epoch fares no better, because resolution is not the only reader. `is_resolved()` and
`value()` route through the binding's `read_value`, which consults the collection, but `metadata()` and
`validation_errors()` are keyed on **Field Identity** with no existence check at all. Absence-by-removal
is safe today only because `clear_collection_item_state` retains out that item's metadata, validators,
form errors, and submit errors — the neutral read is neutral because the entry is gone. An epoch does
not remove anything, so a same-numbered identity is live and *has* state: the binding would report
`is_resolved() == false` while `metadata()` returned another row's real touched and blurred flags. That
is the intra-binding disagreement ADR-0022 was written to remove, re-created by the fix.

Never reusing a number avoids all of it. A retired identity is genuinely absent from `current_items`, so
it is an **Unresolved Binding** in the sense `CONTEXT.md` already defines, every identity-keyed read is
neutral for the reason ADR-0022 already gives, and no **Field Identity** is ever reused — so nothing can
collide in the field store, the validator keys, the submission errors, the observer events, or the
adapter's reactivity map. The public surface does not change and neither serialization version moves.

## Reinitialize remounts every row, and that is accepted

Identities feed `CollectionItemBinding::key()`, which `docs/collection-fields.md` requires as the Dioxus
row `key:` so a row's hook state moves with its logical item. Fresh identities on `reinitialize`
therefore remount every row, costing focus, scroll position, and the parse state held in each row's
scope.

No correspondence heuristic is adopted to avoid it. Matching new rows to old ones needs an
application-supplied key, which ADR-0002 rejected outright, and guessing by position would resurrect the
positional identity this decision removes. If the remount proves costly in practice, the answer is an
explicit opt-in API for adopting a model while preserving identities — a decision on its own terms,
not a heuristic smuggled in here.

## A foreign snapshot can still alias, and is documented rather than guarded

Advancing the counter on restore prevents future collisions; it cannot disambiguate identities that
already collide. A snapshot minted by a *different* allocator history — another form instance, another
process — can carry `[0,1]` denoting different rows than the live form's `[0,1]`, and a retained binding
will alias onto the restored row.

Same-form round-trips are safe under this decision, because `reinitialize` now allocates above the
counter and a binding minted after a snapshot holds an identity that snapshot cannot contain. The
residual case is left as a documented limitation rather than a guard: the primary use — hydration and
cross-process restore — targets a form that has just mounted and holds no retained bindings, and a guard
strict enough to catch the aliasing case would reject that one too.

## Consequences

`FieldStore::clear()` no longer wipes collections, so a `CollectionState` outlives a reset. This is safe
for dirty derivation because `is_collection_dirty` already disjoins the identity comparison with a value
comparison against the baseline, so a stale state can only over-report dirtiness, never under-report it.
The `Model` type is fixed across `reinitialize`, so a static collection path cannot orphan; a derived
path under an absent optional parent can leave an entry behind, bounded by the number of distinct
collection paths ever reached rather than growing per reset.

At the time of this decision, `reset_field` was generic over its value type, so `Value = Vec<Item>`
compiled without touching `CollectionState`; a single monotonic counter still covered that path.
The later collection-reset support now reconciles `CollectionState`: baseline identities survive and
identities added after the baseline are retired.

`state_snapshot` builds collection identity state from whatever the field store holds, and
`ensure_collection_state` is lazy, so a snapshot captured before a collection was ever read carries an
empty identity map and restoring it wipes live identity state. That is the same counter rewind reached
by a third path and is closed by the same rule.

Two parse-binding defects surfaced while settling this and are tracked separately, because both are
reachable today and neither depends on identity reuse: reset, reinitialize, and restore clear parse
errors without unregistering the parse binding, so a write through an unresolved binding still lands —
already contradicting ADR-0022's "writes stay silent" — and `use_collection_item_parsed_text_with` pins
its registration's identity at first render while rebuilding the binding every render.
