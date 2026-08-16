# Resolve the rendered collection item index live

A **Collection Field** binding captures its item's rendered index when it is constructed and never
re-resolves it. `CollectionItem` is a `{identity, index}` snapshot, the adapter's binding stores that
snapshot, and the rendered **Field Name** is formatted from the captured integer. Removing, inserting
or reordering a *sibling* therefore leaves any retained binding rendering a name that addresses a
different row than the one it is bound to.

Dioform will resolve the index **live** against the collection whenever it is asked for, and will
represent an unresolvable index in the return type: `index()` returns `Option<usize>` and `name()`
returns `Option<String>` on the adapter's collection bindings.

This supersedes the closing section of
[ADR-0022](0022-represent-an-absent-binding-target-in-the-return-type.md), "A known staleness is
deliberately left out", which permitted the captured-index name and deferred the question to issue #41.
The reasoning that changed is recorded below.

## The stale name is not merely stale; it collides with a live row

ADR-0022 permitted the captured name on the grounds that the rendered surface is allowed to be neutral
and the row is about to unmount. That reasoning does not survive the reproduction.

Verified against a plain **Form Core** with no `VirtualDom`, on a four-row collection with row 0
removed:

```
retained binding for old row 2:  name = lines[2].description   value = "c"
freshly built binding at index 2: name = lines[2].description  value = "d"
```

Both bindings are **resolved**. Neither is unmounting. They render one **Field Name** over two
different logical items. Under **Native Browser Submission** the browser serializes controls by their
**Field Name**, and `docs/browser-submission.md` commits to the server contract explicitly —
"submitted browser data uses HTML-compatible names such as `invoice.lines[0].product.name`". Two
controls posting under `lines[2].description` means the server silently keeps one and drops the other.

That is data loss on the documented no-JS path, not a cosmetic defect, and it is why this is decided
differently from what ADR-0022 permitted.

The severe case is `CollectionRadioGroupBinding`. A colliding `name` does not mislabel a control — it
*merges two radio groups into one*, so selecting an option in one row clears the selection in another.

## The name stays index-derived; only its resolution changes

Deriving the rendered name from the **Collection Item Identity** instead would be immune to staleness
by construction, the way `accessibility_name` already is. It is rejected outright: it would emit
`lines.item-1.description` and break every server that decodes the form. The rendered **Field Name** is
an HTML interoperability contract with a consumer outside the library, and identity is deliberately not
part of it.

`accessibility()` is already identity-derived and is therefore **not affected** by this defect. The
issue that prompted this decision claimed otherwise; the claim was wrong.

## Absence goes in the return type, for both accessors

There is no neutral `usize`. Returning `0` asserts "first row", which is the same lie
`Default::default()` would have told about a selection, and ADR-0022 already rejected that shape. A
live `index()` therefore cannot be total, and cannot remain `const fn`.

Returning `""` from `name()` was considered at length and rejected on three grounds.

It is not neutral. `value=""` is inert — it renders an empty control and nothing branches on it.
`name=""` is the exact value the HTML entry-set construction algorithm branches on to *exclude* a
control from submission. A value that changes browser behaviour is a load-bearing instruction, and the
library would be issuing it silently on the application's behalf.

It is not unambiguous. The derive accepts `#[form(name = "")]` with no emptiness check, and the crate
itself constructs a `FieldPath` with an empty rendered name in `collection_item_self_path`. `""` cannot
mean "unresolved" when it already means "a field named that".

It would recreate the disagreement ADR-0022 exists to remove. `name()` and `index()` are two views of
one fact. Making one total-and-neutral while the type system forces the other to be `Option` has a
single binding reporting the same absence in two shapes that do not agree.

ADR-0022's rule is not violated by this. That rule governs a **Field**'s *value* — `value()`,
`checked()`, `is_selected()`, `is_rendered_selected()` — and `name()` was explicitly carved out of it
rather than covered by it. The anti-`Option` argument recorded there ("every render site would write
`.unwrap_or_default()`, the neutral value with extra steps") was reasoned about values, where the
default is inert. For a name, `.unwrap_or_default()` is the application *choosing* `name=""` having
been shown the absence — an informed default rather than a library-imposed one.

This also aligns the public surface with the crate's own internals: `collection_item_field_name`
already resolves the index live and already returns `Option<String>`, and every **Form Listener**
receives that live name. The public accessor was the only thing still reporting the captured one.

## The fix stays out of the reactivity layer

Making `name()` register a reactive dependency was investigated and rejected as both broken and
unnecessary.

It does not work. **Field Ancestry** is deliberately *strict* between a collection and its own items —
[ADR-0020](0020-derive-field-ancestry-from-identity-paths.md) argues the case and
`a_collection_field_does_not_relate_to_its_own_items` guards it — so a structure change never
synthesizes a value notification for an item-child identity. Tracking that identity subscribes to
precisely the selector a structure change skips. Tracking the *collection* identity instead would fire,
but that is the fan-out ADR-0020 rejected, and
`collection_structure_selectors_rerender_without_rerendering_item_value_readers` exists to prevent it.

It is also not the mechanism. Waking a selector re-runs a component; it does not change what a function
returns. A retained binding that re-renders still formats over the snapshot it owns. Live resolution is
the fix; tracking was never going to be.

It is unnecessary in a rendered DOM. A parent that calls `items()` subscribes to the collection's value
selector, which *is* woken by a structure change, and rebuilds every binding with a fresh index. The
`use_collection_item_*` hooks rebuild each render for the same reason. The reachable damage is
therefore in retained bindings used outside render — event handlers, spawned futures, `use_effect`,
plain `FormHandle` use, and anything reading the form by rendered name — which is the same surface
ADR-0022 identified as the reachable one.

## Resolution must agree with the value, in lockstep

The live lookup is read-only and must not go through `collection_items`, which takes `&mut self` and
ensures validator state as a side effect. `dioxus_collection_is_resolved_reads_without_borrowing_the_core_mutably`
pins that guards of this class do not reach for a mutable borrow, and a render-path accessor that did
would panic inside `read_core`.

Resolving with `current_index` alone is insufficient. `set_field` on the collection path mutates the
draft `Vec` without touching `CollectionState`, so the two can desynchronize: the index resolves while
the item is gone from the draft. `collection_item_field_value` already guards this by bounds-checking
the resolved index against the draft, and the index lookup must apply the same check.

`name()`, `index()`, `value()` and `is_resolved()` therefore answer `Some`/`None` in exact lockstep.
An unresolved binding never renders a name, and a binding that renders a name always has a value.

## The core keeps its snapshot

`CollectionItem`'s captured index is *correct at the moment `collection_items()` returned it*, which is
what a snapshot is for. `CollectionItem::index()` stays a total `const fn` and **Form Core** keeps its
current behaviour.

The defect is that the adapter binding *stored* the snapshot and treated its index as durable truth. The
adapter retains the **Collection Item Identity** and resolves position on demand. Core gains one
additive read-only index lookup — the same read `collection_item_field_value` already performs
internally — and nothing else. As in ADR-0022, this is an adapter-surface fix.

## Identity reuse is a separate defect

Resolution assumes a **Collection Item Identity** denotes one logical item for as long as any binding
holds it. `reset()` and `reinitialize()` break that assumption by clearing the collection state, after
which identities are re-minted from zero and a retained binding can resurrect onto an unrelated item
with `is_resolved()` reporting `true`.

That is a stale *identity* rather than a stale *name*, it predates this decision, and it is tracked as
issue #42. Nothing here fixes it, and nothing here depends on it being fixed: live resolution is
correct for every identity that still denotes what it denoted when the binding was built.
