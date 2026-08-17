# Match listener reach to what each event asserts

[ADR-0020](0020-derive-field-ancestry-from-identity-paths.md) closes by naming four listener surfaces
— `field_listeners`, `debounced_field_callbacks`, `field_blur_callbacks`, `field_binding_listeners` —
as "the same class of defect on a different surface", deferred to follow-ups that the new predicate
would make cheap. They are the same defect. They do not take the same fix.

**Field Ancestry** is symmetric, and symmetry is correct for the relation. What is not correct is
applying it uniformly to every listener surface, because the surfaces do not deliver the same kind of
fact. Dioform will choose each surface's **Listener Reach** by asking what the event asserts about the
listener's **Field**:

- **Value replacement** asserts *the value at this path was replaced*. That is true in both
  directions, so these surfaces reach across the whole of **Field Ancestry**.
- **Blur** and **binding lifecycle** assert *something happened inside this Field*. That is true only
  of the **Fields** that contain the one the event names, so these surfaces reach only from a
  contained **Field** outward.

One criterion, applied to all four, rather than an exception carved for the awkward ones.

## Value replacement is symmetric because the write is a whole-subtree assignment

`replace_field_with_origin` ends in `*path.get_mut(self.draft.current_mut()) = value`. Writing
`invoice.customer` does not merely *affect* `invoice.customer.name` — it overwrites the storage that
holds it. The surfaces are named for this: `dispatch_value_replacement_listeners`, and
`FormListenerEvent::FieldReplaced`. The contract is replacement, not change.

The objection that an ancestor write may leave a descendant's value byte-identical, so firing its side
effect is unwarranted, does not survive contact with the existing contract. `set_field` carries no
`PartialEq` bound and `apply_field_mutation` dispatches unconditionally, so `set_field(email,
same_value)` already fires that listener today under exact identity. Firing on an unchanged value is
what these surfaces have always done; ancestry does not introduce it.

Symmetry here also keeps `apply_field_mutation` internally consistent. It calls
`start_runtime_async_field_validators` — which filters by the symmetric predicate — a dozen lines
above its listener dispatch. An asymmetric listener rule would put two adjacent statements in one
function body on opposite sides of the same question.

## Blur reaches outward only, because focus containment is not value containment

Under the symmetric predicate a blur of `invoice.customer` fires the listener registered on
`invoice.customer.name`. That registration — a leaf blur listener that trims or normalises the field
the user just left — is the common one, and it would run when focus never entered `name`. The library
would be reporting a focus event that did not happen, on the surface most people use.

Reversed, the same predicate is right: `invoice.customer` genuinely does contain the widget that
blurred, and `use_field_blur_listener` on a container path is currently a registration that compiles,
registers, and can never fire from any DOM-driven blur, because blur enters only through a leaf
binding's `on_blur` and a container has no element.

This matches the platform. `focusout` bubbles; `blur` does not. Moving focus between two inputs inside
one container fires the container's listener twice, and neither the DOM nor Dioform has a
"focus left the subtree" notion — you compute it from where focus went. That double fire is the
semantics, not a defect to engineer away, and it is why the event is named *a blur occurred at or
below this Field* rather than *this Field blurred*.

### Blur metadata stays exact

ADR-0020 left `FieldMetadata` and the touched/blurred flags unexpanded, because "expanding them would
invent a contract rather than restore one". That decision stands, and the consequence is a caveat
worth stating plainly: a container's blur listener fires while `is_field_blurred(container)` reports
`false`.

The alternative was measured and is worse. `should_show_validation_errors` reads the blurred flag
directly, and every visible-error selector and `FieldAccessibility` routes through it, so aggregating
blurred upward makes a child's stored error **visible on an input the user never focused** and starts
announcing `aria-invalid` for it. It would also fork the meaning of serialized metadata, which
`docs/form-state-serialization.md` exists to protect, and it would redefine **Blurred Field** —
"a **Field** that has lost focus at least once" — into "focus has been inside this subtree". That is a
domain change, and it does not belong in a fix to listener dispatch.

A related defect predates this decision and is not fixed here: ADR-0020 widened validator *selection*
without widening error *visibility*, so a child blur can store a container validator's error that
blocks submit while being invisible. That is a visibility-policy question and has its own issue.

## Binding lifecycle reaches outward only, and its replay has to carry identities

"A hook-owned binding mounted" is the same shape of fact as a blur: true of the **Fields** that
contain the bound one, not of the ones it contains. The reach follows blur.

The mechanism does not. `mounted_field_bindings` is keyed by the mounted binding's identity and counts
active bindings, and both replay paths — a listener registering while bindings are already mounted,
and a listener dropping while they still are — look that count up by the listener's own identity and
replay events built from it. Widening dispatch without widening those two lookups leaves a listener on
a container receiving nothing at registration and a bare `Unmounted` later, breaking the balance
`docs/form-listeners.md` promises "regardless of listener hook order".

The map stays identity-to-count. A set would be the obvious simplification and would silently drop
multiplicity when two components bind the same **Field**; the count is load-bearing, and
`record_field_binding_lifecycle` decrements it unchecked. What changes is that both replays scan for
contained identities and emit each event with the *mounted binding's* identity, matching what live
dispatch already passes.

## Collections: ancestor-or-equal on the collection component, for listeners only

The strict collection clause keeps a **Collection Field** from relating to its own items, which is
what stops a structure change from re-rendering item value readers and what stops appending a row from
re-running every existing row's validators. It stays strict.

For listeners it produces an inversion that cannot be shipped: item-field writes carry a
`CollectionItem` identity and structure mutations carry the collection's static identity, so a
listener on `invoice.lines` hears pushes but not row edits, while a listener on `invoice` hears both.
The nearer registration receives strictly less than the further one.

The fix is a listener-side widening to ancestor-**or-equal** on the collection component, in the
listener filters. Co-dispatching the collection identity alongside the item identity at each dispatch
site was the alternative and is worse on two counts. It has to enumerate call sites, and the
enumeration is easy to get wrong — `mark_collection_item_field_blurred_without_validation` is a
distinct site reached whenever a binding holds a parse error, so an incomplete list yields a listener
that hears a row blur when the text parses and silently does not when it fails. And the identity it
co-dispatches is a `Static` one, which the symmetric static clause then relates *downward*: every
static descendant of the collection path starts receiving every item-field edit, which is exactly the
relation `a_static_descendant_of_a_collection_path_does_not_relate_to_its_items` forbids. Co-dispatch
avoids changing the predicate by changing the effective relation instead, invisibly, where no test can
see it.

The residual asymmetry is deliberate: listeners register through `FieldPath<Model, Value>` on the root
model, so a listener identity is always `Static` and item-scoped registration is not expressible.
Widening the shared predicate would therefore buy nothing a listener can use and would cost the
selector and validator contracts the strict clause protects.

## Reentry stays a panic, and the debounced reschedule loop is fixed here

Widening reach widens reentry. A listener on a container that writes one of its own fields is a
documented dependent-field reset, and under ancestry it re-enters itself. It keeps panicking: that is
the invariant every listener dispatch site already holds, the message already names the origin-filtered
hook that fixes it, and a silently dropped side effect on surfaces whose job is autosave and audit is
worse than a loud failure at development time. Applications that need the pattern use
`use_field_listener_for_origin`, because listener-caused writes are **Programmatic Updates**.

The debounced surface cannot rely on that. Its callback is borrowed only after the delay resolves, so
the reentry panic is unreachable there, and a debounced listener that writes a related **Field**
reschedules itself indefinitely with no diagnostic. The loop is not new — an unfiltered
`use_debounced_form_listener` reaches it today — but until now it required a listener that was
self-evidently self-triggering. Ancestry makes it reachable from a callback that names no field it
listens to, and from a two-listener cycle in which neither is self-referential. Enlarging a defect's
blast radius and deferring it is not a defensible split, so a bounded consecutive-reschedule guard
lands with the widening.

## Ordering is registration order

Ancestry makes the ancestor-plus-descendant pair the normal shape rather than a rarity, so the order
in which two matching listeners run becomes observable: whether a section-level autosave sees a
field-level normaliser's output is exactly this question. Listener entries are pushed on registration
and `retain`ed on removal, so registration order is already what happens and already deterministic;
it is documented rather than disclaimed.

Depth-based ordering — nearest-first or outermost-first — is declined. **Field Ancestry** is a
predicate with no `parent()`, `segments()`, or `depth()`, deliberately, so that its representation
stays swappable. Ordering by depth would force that seam open to serve a preference nobody has asked
for. The caveat that registration order tracks component mount order, so a remounted subtree's
listener moves to the end, is documented with it.

## What this narrows

ADR-0020's closing paragraph stands as a description of the defect and is narrowed on the fix: the four
surfaces are the same defect class, and three distinct reaches serve them. The predicate itself is
untouched — `FieldAncestry::relates` keeps its clauses, its strictness, and its unit tests — and gains
a directional companion for the surfaces that assert containment rather than replacement.
