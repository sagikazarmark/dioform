# Represent an absent binding target in the return type

A **Field Binding** whose addressed **Field** has no value in the **Form Draft** — an **Unresolved
Binding** — currently gets one of two unrelated answers depending on which accessor the application
reached for. Four accessors panic; the rest return a neutral value. The split follows what the type
system happened to permit rather than a decision about what absence means, and in two cases the same
binding disagrees with itself: `CollectionSelectBinding::value()` panics while `is_selected()` on the
identical base returns `false`, and `CollectionRenderedSelectBinding::value()` returns `""` while
`typed_value()` on the same binding panics.

Dioform will answer absence by **surface**: the rendered surface is total and neutral, the typed surface
makes absence representable in the return type, and no accessor panics. This holds library-wide, not
only for collections.

## The panics are reachable from documented usage

All four were reached against a plain **Form Core** with no `VirtualDom`: create a binding, remove the
item, call the accessor. Render-time is mostly safe because Dioxus flushes ancestors first, so a parent
re-render unmounts removed rows before they render. The reachable path is ordinary code that holds a
binding clone across a mutation — event handlers, spawned futures, `use_effect` / `use_memo` — which the
library actively encourages: `oninput()` / `onchange()` are documented as handing the handler its own
clone precisely so the binding stays usable for `value()`.

So the panic fires on correct code, not on a mistake. That is what distinguishes it from the crate's
existing panicking accessors, and it is why the fix is not a `try_*` twin.

## The core is already total; only the adapter's public surface lies

`FormCore::collection_item_field_value` returns `Option`, `collection_item_exists` already exists, and
every core write returns whether it landed. The adapter's private wrappers thread that answer correctly.
The defect is confined to the public binding methods, which discard it — `.expect(...)` on the typed
accessors, `.unwrap_or_default()` on the rendered ones, and `()` on every setter.

The decision therefore costs no new core capability. It removes a lossy translation at one boundary.

## The rendered surface stays neutral; the typed surface gains `Option`

`Value` in the panicking accessors is generic with no `Default` bound, so unlike `String` and `bool`
there is genuinely no neutral value to return. The split is type-forced rather than arbitrary — but
`is_selected()` proves a non-panicking answer was available on the same data, and `Default::default()`
would be worse than either option, because it asserts a selection the model does not hold.

Accessors on one binding are therefore required to agree in **meaning**, not in return type. Absence
reads as `""`, as `false`, and as `None`, and none of those is a lie. Requiring identical return types
instead — `Option<String>` from a text binding — was rejected: every render site would write
`.unwrap_or_default()`, which is the neutral value with extra steps, and the churn buys nothing a
documented policy does not already give.

This is the shape [ADR-0021](0021-traverse-optional-fields-with-a-named-path-combinator.md) settled for
**Optional Fields**: a total, neutral editing surface, with an honest accessor alongside that recovers
the absent-versus-present distinction. Answering collections differently would give the library two
answers to one question, which is the divergence this decision exists to prevent.

## The rule is library-wide

Collections are today the only surface where a binding's target can vanish: plain **Field Path**
accessors are infallible, and ADR-0021's combinator keeps derived optional paths total. The
collection-scoped rule and the library-wide rule therefore have identical implementations right now.

Stating it library-wide costs nothing today and binds the presence work ADR-0021 defers — *presence as a
first-class, metadata-carrying concept*. When that lands, it inherits this answer instead of inventing a
second one.

## Reads gain honesty; writes stay silent

A write to an **Unresolved Binding** is a no-op, and stays one. This asymmetry is deliberate.

A read must return something, so it either lies or crashes; that is the defect. A write to a removed row
has genuinely nothing to do. Surfacing the no-op through setter return types was rejected on a hard
constraint: the ergonomic handlers are `impl FnMut(Event<FormData>)` by signature and physically cannot
propagate a result. `set_value` would report the no-op while `oninput()` — the documented path, the one
that hands the handler its own clone — could not. That reintroduces exactly the intra-binding
disagreement being removed here.

Metadata and validation errors are likewise unchanged: an **Unresolved Binding** reports no metadata and
no errors. Removal releases the item's scoped state, so "not touched, no errors" is a true statement
about state that no longer exists, not a fabricated default. `Option` there would express "absent" for
something that is correctly empty.

## `is_resolved()` belongs on the leaf bindings, not only the row

The natural placement is the item binding, so callers guard once instead of per-accessor. That alone is
insufficient: the values retained across a mutation are the leaf bindings, and a handler holding a
`CollectionTextBinding` has no route back to the `CollectionItemBinding` it came from. The guard is
therefore available on the item binding, on every leaf collection binding, and on the multi-select item.

It is an inherent method on each, not a public trait. Nothing needs to be generic over "things that can
dangle", and a trait would add a nameable concept to the public vocabulary for no gain.

## Break the signature rather than add a `try_*` twin

The crate has a `try_*` convention — `try_use_form_context`, `FieldPath::try_join`, `try_on_change` — and
it does not apply here. In every existing case the panicking partner fires on a **programmer error**: no
context provider mounted, a malformed join. Keeping a panicking `value()` alongside a `try_value()`
would preserve a footgun that only ever goes off when the application did what the documentation said.

The typed accessors therefore return `Option<Value>` outright in the next breaking release, with no
deprecation tail. The crate is pre-1.0 with two releases published, so this is as cheap as it will ever
be, and the compiler finds every call site.

## A known staleness is deliberately left out

> **Superseded by [ADR-0023](0023-resolve-the-rendered-collection-item-index-live.md).** This section
> permitted the captured-index name on the grounds that the row is about to unmount. That does not
> hold: the stale name collides with a *live* row's name, and both bindings are resolved. `index()`
> and `name()` now resolve live and return `Option`. The rest of this ADR stands.

`CollectionItem` captures its index at construction, and `index()` reads the captured copy. Removing a
sibling therefore makes `index()` and the rendered **Field Name** wrong for a **resolved** binding — a
retained binding for row 1 still renders `lines[1].description` after row 0 is removed, while a freshly
built binding for the same item renders `lines[0].description`.

That is the same root cause — a retained binding observing a mutated collection — but it is wrong output
from a correct binding rather than a crash, and resolving the index live has its own design questions.
It is tracked separately as issue #41. Until then `name()` keeps returning the captured-index name for resolved and
unresolved bindings alike, which this decision permits: the rendered surface is allowed to be neutral,
and the row is about to unmount.
