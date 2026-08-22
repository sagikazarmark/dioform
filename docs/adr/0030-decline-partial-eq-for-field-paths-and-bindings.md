# Decline `PartialEq` for field paths, field group maps, and bindings

> **Superseded by [ADR-0047](0047-make-field-paths-interchangeable.md).** ADR-0047 reverses the
> refusal for field paths, field-group maps, and scalar bindings while retaining the collection-binding
> exclusion.

Dioxus requires every `#[component]` prop to be `PartialEq`. `FieldPath` does not implement it, nor does
the derived `…FieldGroupMap`, nor `CollectionBinding` and `CollectionItemBinding`. So a reusable
field-group helper stays a plain `fn` rather than a component
([issue #43](https://github.com/sagikazarmark/dioform/issues/43)), and a keyed collection row cannot be
handed the item it renders ([issue #44](https://github.com/sagikazarmark/dioform/issues/44)).

Dioform will not add equality to any of them. [ADR-0024](0024-compare-form-handles-by-observable-identity.md)
made **Form Handle** comparable and excluded "`FieldPath`, the derived `…FieldGroupMap`, or any binding
type" pending this decision; that clause now stands as a decision with reasons rather than a deferral.
Helpers parameterised by mount site remain plain `fn`s, and rows keep taking the handle and a
**Collection Item Identity** as props.

## Identity equality is unsound, and the sound comparison means "is a clone of"

The obvious definition is equality by **Field Identity**, which is already `Eq + Hash` and is what the
whole field-state layer is keyed by. [ADR-0021](0021-traverse-optional-fields-with-a-named-path-combinator.md)
deliberately made identity non-unique, so it would be wrong. The optional-field combinator clones the
parent's identity and shares its rendered name, substituting only the two accessor closures, so
`path.or(&a)` and `path.or(&b)` carry equal identity, equal name, and different read-and-materialise
behaviour. `docs/optional-fields.md` states that aliasing as a commitment: the two are "two views of one
field". `FieldPath::direct` is public besides, and takes identity, rendered name, and both accessors as
independent arguments with nothing tying them together.

That matters more here than it would elsewhere, because the consumer is prop memoization. Dioxus's
generated `memoize` copies the new props over the old **only when they compare unequal**, so an equality
that lies does not merely skip a render — the child retains the wrong accessor permanently.

A sound alternative exists: compare identity, rendered name, and `Rc::ptr_eq` on both accessors, so equal
means genuinely interchangeable and `path.or(&a) != path.or(&b)` correctly. Its cost is the contract it
implies. `Clone` shares the accessor `Rc`s, and `#[derive(Form)]` builds a fresh path with fresh closures
on every `Model::fields().field()` call, so:

```rust
Model::fields().street() == Model::fields().street()   // false
```

Two independently derived paths to the same field compare unequal. Never stale, never wrong — but "equal"
would mean "is a clone of", not "addresses the same field".

## The contract costs more than the optimisation is worth

What equality buys is prop memoization, and only conditionally: a caller who hoists the **Field Group
Map** into a `use_hook` gets it, a caller who rebuilds it each render does not. Nothing is incorrect
without it. What it costs is a public `PartialEq` on the primary typed addressing mechanism whose meaning
is not the one a reader reaching for `PartialEq` assumes. That is a permanent trap traded for a
conditional optimisation nobody has reported needing.

Implementing it only on the derived `…FieldGroupMap`, delegating to a private comparison and leaving
`FieldPath` without a public `PartialEq`, narrows the exposure but not the contract. The map's fields
*are* field paths, so the clone-of semantics move behind a type that does not explain them, and any
component wanting a `FieldPath` prop directly is still blocked. The narrower option buys a smaller
surface for the same surprise.

## The helper is a plain `fn` because of what it is

The README's reusable field-group helper takes a **Field Group Map** because it is parameterised by
**Field Group Mount**, which is not handle-shaped data and which **Form Context** could not supply
either. Making it a `#[component]` would not remove that prop; it would only make the prop comparable.
`README.md` and ADR-0024's consequences already give this reason, and it is not a shortfall waiting on
equality to be repaired.

## Bindings fail for a second, independent reason

The sound comparison is not reachable from the adapter. `FieldPathAccessor` is private to
`dioform-core` and not re-exported, so `dioform` cannot compare two paths by accessor pointer until core
exposes path interchangeability as a capability — which is what this ADR declines. A binding equality
would have to be built on something weaker, and everything weaker is the unsound comparison above.

The order-sensitivity that motivated the original proposal is gone from the stored state:
[ADR-0023](0023-resolve-the-rendered-collection-item-index-live.md) resolves the rendered index live, so
a `CollectionItemBinding` now holds a handle, a collection path, and a **Collection Item Identity**, with
nothing captured to compare. Comparing a live index instead would make equality a function of mutable
form state, evaluated during a memoize diff.

What actually keeps a row correct across a structure change is its reactive subscription, not its props.
**Field Ancestry** is strict between a **Collection Field** and its own items
([ADR-0020](0020-derive-field-ancestry-from-identity-paths.md)), so a structure change never wakes an
item-child value reader; a row stays correct today only because its own `items()` lookup registers a read
on the collection's value. A row handed a binding as a prop and doing no lookup loses that subscription.
The failure is broader than a reorder: remove the **last** row, and the survivors keep both identity and
index, their props compare equal, the rows memoize, and a sibling count derived from `items()` goes stale
— leaving an enabled "move down" control that calls `move_to_index` out of range.

`CollectionBinding` and `CollectionItemBinding` are not symmetric — a collection path encodes no
position, so the collection half carries none of that — but the collection half alone buys nothing the
handle prop does not already give.

## When to revisit

Reopen on a reported case rather than an abstract one: a caller who genuinely cannot hoist the **Field
Group Map**, and who measures the re-render cost as material. An ADR superseding this one has to settle
three things it leaves open — whether "equal" may mean clone-of on a public type, whether core exposes
path interchangeability as a named capability rather than leaking `FieldPathAccessor`, and, for bindings,
how a row that receives one keeps a subscription to its collection's structure.
