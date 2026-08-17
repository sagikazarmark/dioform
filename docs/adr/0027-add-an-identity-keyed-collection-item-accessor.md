# Add an identity-keyed collection item accessor

`CollectionBinding` gains `item(identity) -> Option<CollectionItemBinding>`, the read half of the
identity-keyed family its mutations already speak. `MultiSelectBinding` does **not** gain the mirror
accessor: it is keyed by value, not by **Collection Item Identity**, so it has no equivalent gap.

## The two halves of the API spoke different languages

Every identity-keyed operation on a collection takes a **Collection Item Identity** — `remove(item)`,
`move_to_index(item, index)` — and `append` / `insert` hand one back. Reading had exactly one entry
point, `items()`, which takes nothing and returns everything. A caller holding an identity therefore
had to scan the collection to get back to the item it already named:

```rust
let item = lines
    .items()
    .into_iter()
    .find(|candidate| candidate.identity() == identity)
    .expect("...");
```

That is the shape a keyed row opened with, and it recurred wherever an identity outlives the binding
it came from: the identity `append` just returned, one captured in an event handler or in
application state, one delivered by a **Form Observer** transition.

## It removes a panic path from leaf rendering code

The scan's `expect` is what makes this more than convenience. A row cannot ask "am I still here?"
without deciding what an unresolvable identity means, and until now the least-bad answer was a panic
site in code whose only job is to paint one row.

With the absence in the return type, a row writes `let Some(item) = lines.item(identity) else {
return rsx! {} };` instead. The early return is legal as long as it precedes the row's first hook:
returning ahead of every `use_` call leaves the scope's hook slots untouched, while skipping one hook
and claiming a later one mis-indexes them and panics inside `dioxus-core`. In a rendered DOM the
branch is a guard rather than a reachable path — Dioxus flushes the parent first, so a removed row
unmounts before it renders again — but a guard is what the row needed, and it is cheaper than an
`expect` that has to be argued about.

`Option` rather than a binding that resolves to neutral values is what
[ADR-0022](0022-represent-an-absent-binding-target-in-the-return-type.md) asks for, and answering at
the lookup keeps the accessors on the returned binding out of the question entirely: a binding handed
back by `item()` is resolved at that moment, and reports absence afterwards exactly as any other
retained binding does.

## The lookup carries the row's subscription

**Field Ancestry** is strict between a collection and its own items
([ADR-0020](0020-derive-field-ancestry-from-identity-paths.md)), so a structure change never wakes an
item-child value reader. A row survives a sibling removal or a reorder only because its own lookup
registers a read on the *collection's* value — which is what `items()` does, and what nothing else
would do on the row's behalf.

`item()` therefore registers the same collection-value read. Resolving through an item-child selector
would subscribe to precisely the selector a structure change skips by design, and a row built on it
would silently stop re-rendering. The regression test covers removal of a *later* sibling, whose
index change is invisible to the row: the assertion is that the row re-rendered at all.

The lookup itself stays read-only, unlike `items()`, which takes a mutable core borrow to ensure item
validator state. Nothing about resolving an identity needs that borrow, and staying read-only means
the accessor answers inside `read_core` — the same property
[ADR-0023](0023-resolve-the-rendered-collection-item-index-live.md) pinned for the live index
lookups. Item validator state is ensured by the mutations that create items and by validator
registration, so no state depends on the read.

## Multi-select is keyed by value, and gets nothing

`MultiSelectBinding` has the same surface gap in shape — `items()` and `selected_identity(value)`,
but no `item(identity)` — and the argument above does not transfer. Its entire surface is keyed by
value: `select`, `deselect`, `toggle`, `is_selected`, `selected_item`, `selected_identity`. There is
no read-versus-write language split to close, because identity is not the language on either side.

The only ways to hold one of its identities are `select`'s return value, `selected_identity(value)`,
and `items()` — and in each case the caller has, or just had, the value that produced it, which
`selected_item(value)` takes directly. Adding an identity-keyed twin would put a second lookup key on
a surface that deliberately has one, which is the kind of surface this repo has declined before on
weaker grounds than these ([ADR-0017](0017-decline-whole-model-schema-coercion.md),
[ADR-0018](0018-decline-public-validation-adapter-trait.md),
[ADR-0019](0019-decline-can-submit-when-invalid-opt-out.md)).

Symmetry between the two bindings is not itself a reason: they are symmetrical in the mechanism (a
multi-select's selected value *is* a logical collection item) but not in what the application names
them by, and it is the naming that decides which accessors exist.

## When to revisit

Reopen the multi-select half if an application genuinely holds a bare identity with no route back to
its value — a selection tracked across a `reinitialize`, say, or an identity arriving through a
**Form Observer** transition that the application must resolve without knowing what was selected. The
observer case is the plausible one; it has not been demonstrated, and until it is, `items()` plus
`selected_item(value)` covers the reachable paths.
