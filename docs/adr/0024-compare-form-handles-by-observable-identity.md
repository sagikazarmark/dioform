# Compare form handles by observable identity

Dioxus requires every `#[component]` prop to be `PartialEq`, and `FormHandle` does not implement it, so
the handle cannot cross a component boundary through the primary API. Splitting one form into a parent
and a child — the ordinary way to keep a Dioxus form readable — fails with an `E0369` pointed at the
`#[component]` attribute rather than at the prop that caused it.

Dioform will implement `PartialEq` for `FormHandle<Model, Error>` as `Rc::ptr_eq` on the form instance
conjoined with `FormIdNamespace` equality, adding no bound to `Model` or `Error`. It will **not**
implement `Eq` or `Hash`, and it will **not** extend equality to `FieldPath`, the derived
`…FieldGroupMap`, or any binding type.

## The form instance is a sufficient identity anchor

`FormHandle` holds seven `Rc`-shaped fields, and comparing one of them is enough because the seven can
never diverge. `FormHandle::from_core` is the only site in the workspace that allocates them, and it
allocates all seven in a single expression; `impl Clone for FormHandle` is the only site that propagates
them, and it shares all seven together. Every public constructor — `new`, `new_with_error_type`,
`from_config`, `from_core_with_id_namespace`, and the `use_form*` hooks — funnels into one of those two.
No API accepts an already-shared `Rc<RefCell<FormCore<..>>>`; every constructor takes `FormCore` by
value and wraps it itself.

That invariant is load-bearing but unenforced by the type system, so it is asserted by test rather than
assumed. A future constructor that shares a `core` across independently built handles would make two
handles with different runtimes, listeners, and adapter state compare equal; the test is what makes that
mistake loud.

## The ID namespace is part of the identity, not an accessory

`with_id_namespace` returns a handle sharing all seven pointers and differing only in the rendered-ID
prefix, so pointer equality alone would call two such handles the same. That is not merely a missed
re-render. Dioxus's generated `memoize` copies the new props over the old **only when they compare
unequal**, so an over-permissive equality leaves the child holding its original handle permanently — a
child that received one namespace-variant would render the other's element IDs for the rest of its life.

This also matches the shape Dioxus uses for its own `Rc`-backed handles. `Callback` compares as
`ptr_eq(&callback) && origin == origin`; `Signal`, `Memo`, `Resource`, and `ReactiveContext` are all
pointer identity. `CONTEXT.md` already defines **Form Handle** observable identity as the underlying
form instance together with its **Form ID Namespace**; this makes the code agree with the glossary.

## Memoizing on a never-changing handle is safe because reactivity does not consult props

The concern this design has to answer is staleness: if the handle never changes, Dioxus skips the child,
and a child that renders form state would go stale. It does not, because the two mechanisms are
independent. Props memoization is a diff-time early return; reactive dirtying pushes the scope onto a
flat height-ordered `dirty_scopes` set that the scheduler drains without consulting any parent or any
props. Every dioform read routes through `ReactiveSubscribers::track_read`, which subscribes the current
scope's `ReactiveContext`, and `notify_changed` marks those subscribers dirty.

This was verified against Dioxus 0.7.10 with a probe, not only by reading: with an always-equal handle,
a signal change re-rendered the child while the parent never re-ran. A memoized skip does not remove the
scope, so hooks, listener registrations, and **Parse Blockers** registered by that child stay live.

## `FieldPath` was deliberately excluded, and identity equality would be wrong

> **Superseded for field paths, field-group maps, and scalar bindings by
> [ADR-0047](0047-make-field-paths-interchangeable.md).** The **Form Handle** equality decision in
> this ADR remains in force, as does the requirement that path equality never rely on identity alone.

The obvious next step — equality on `FieldPath` by **Field Identity**, which is already `Eq + Hash` and
is what the whole field-state layer is keyed by — is unsound, because
[ADR-0021](0021-traverse-optional-fields-with-a-named-path-combinator.md) deliberately made identity
non-unique. The combinator's derived path reuses the parent's identity and rendered name, so
`path.or(&a)` and `path.or(&b)` carry equal identity, equal name, and different read-and-materialise
behaviour. That aliasing is intended and documented: the two are "two views of one field".
`FieldPath::direct` is public besides, and takes identity, rendered name, and both accessors as
independent arguments with nothing tying them together.

Under a memoizing comparison, an equality that lies does not merely skip a render — per the copy rule
above, the child retains the wrong accessor permanently. A conservative alternative exists and is sound:
compare identity, rendered name, and `Rc::ptr_eq` on both accessors, so equal means genuinely
interchangeable. It is not adopted here because `#[derive(Form)]` builds a fresh `FieldPath::direct`
with fresh closures on every `Model::fields().field()` call, so two independently derived paths to one
field would compare unequal — correct, never stale, but a contract meaning "equal" only for clones, which
deserves its own decision rather than arriving as a side effect of this one. That decision was
[ADR-0030](0030-decline-partial-eq-for-field-paths-and-bindings.md), which declined it. ADR-0047
revisits the representation, retains direct function pointers for structural comparison, and limits
clone-of equality to composed paths.

The derived `…FieldGroupMap` follows `FieldPath` and is excluded with it: its fields are field paths and
it has no other way to compare them.

## Consequences

The reusable field-group helper in the README stays a plain `fn` and does not become a `#[component]`.
That is not a shortfall of this decision. The helper takes a **Field Group Map** because it is
parameterised by mount site, which is not handle-shaped data and which **Form Context** could not supply
either.

Prop-passing is made to work; it is not promoted. **Form Context** remains the answer when a handle
would otherwise be threaded through a subtree of renderless helpers, and `docs/collection-fields.md`
continues to require row components to take an identity as a prop and read the handle from context,
because a hook's state lives in the scope that calls it.

> **Overtaken by [#35](https://github.com/sagikazarmark/dioform/issues/35)** on the collection-rows
> half. A keyed row now takes the handle as an ordinary prop alongside its identity prop, and a page
> that renders rows no longer declares a **Form Context Scope** for them. The reason given above
> still holds and is why the row stays a component keyed by **Collection Item Identity** — a hook's
> state lives in the scope that calls it — but that never required context, only a component
> boundary. Everything else in this paragraph, and the decision, stand: **Form Context** is still the
> answer for a subtree of renderless helpers.

Making the handle prop-able widens exposure to an existing hazard: a child that adopts a passed handle
with `use_form_handle(move || props.form.clone())` runs **Form Cleanup** on the shared form at its own
unmount, and `deactivate` is one-way — no code path sets the adapter active again. This is reachable
today through context and is not created here, but it becomes easier to reach and is documented and
`debug_assert`ed alongside this change.
