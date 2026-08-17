# Re-address a parse blocker to the item its mount renders

`use_collection_item_parsed_text_with` — the funnel behind `use_collection_item_number`,
`use_collection_item_date`, and `use_collection_item_parsed_text` — splits its state across two
lifetimes. The hook state is built once, inside `use_hook`, and derives the **Parse Blocker**'s
**Field Identity** from the **Collection Item Identity** available at that moment. The binding it
returns is rebuilt every render from the *current* item. So the registration and the binding it
belongs to can address different logical items, and every parse read and write takes both halves.

Dioform will keep one registration for a mounted scope's lifetime and re-address it. The hook pushes
the current field into its registration on every render; re-addressing clears the raw text and
**Parse Error** held for the old field and notifies it; every parse read and write carries its
caller's address and touches the mount only when that address matches; and a write additionally
requires the addressed item to resolve. A registration whose entry has been swept away is
re-registered rather than left mute.

Two reproductions, both verified against a `VirtualDom` whose scope renders `items()[0]`
positionally. Remove the row the hook registered against, and the binding stays healthy on every
visible surface — `is_resolved()` is `true`, `name()` is `Some("lines[0].quantity")` — while typing
unparsable text yields no **Parse Error**, `can_submit()` stays `true`, and the keystrokes vanish:
the removal swept the pinned id, so the blocker never engages at all. Reinitialize instead, and the
pinned registration survives; typing blocks submission with an error attached to a row that no
longer exists, while a freshly built binding for the row actually on screen reports nothing, so the
UI that would clear the blocker cannot see it.

## The mount owns the blocker; the caller only proves it still belongs

`CONTEXT.md` already says whose the blocker is: "A mounted input binding's unresolved **Parse
Error** that prevents adapter-mediated submission until the binding parses successfully or
unmounts." One mount is one input element, and `ParseState` gives it one error slot. That is the
right shape, and this decision keeps it — what was missing is that the mount's *address* was fixed
at first render while the thing it renders was not.

The caller's address is therefore a guard, never a second source of truth. `oninput()` and
`onblur()` clone the whole binding, and `ParseBindingRegistration` is `Rc`-backed, so a clone parked
in a handler shares the mount with the binding the scope renders now. Each clone holds its own fixed
item in its base, so without the guard two callers write one slot and the last one wins while the
other's read silently returns `None` — a clone would drive the mounted control's blocker after it
had stopped representing that control. Matching the caller's address against the mount's makes a
clone that no longer matches inert, which is the same answer [ADR-0022](0022-represent-an-absent-binding-target-in-the-return-type.md)
gives a retained binding whose item is gone: the write has nothing to do, so it does nothing.

The resolution check on writes is that ADR's rule reaching a path that never asked. Its "writes stay
silent" is already false for parse errors, because the error branch of the parsed input handler
calls `set_error` unconditionally. That defect is reachable today without any of this, and is fixed
under its own issue; it is named here only because this decision puts the address and its resolution
in the same place, and a fix that re-addressed without checking would rebuild the hole.

## The transition is the decision

Re-addressing clears the old field's raw text and error. Without that step the fix is worse than the
defect it replaces.

A blocker left behind by a scope that has moved on belongs to no mounted binding, which contradicts
the definition above, and nothing terminates it: the mounted binding reads `None` for its own field
and renders clean, while `parse_errors()` and `has_parse_blockers()` keep blocking submission. No UI
can see it, so no UI can clear it. It survives until the row it names is removed, a blanket clear
runs, or the last `Rc` drops. The status quo is louder and less correct, but a user can always retype
and clear it; a fix must not trade a visible wrong answer for an invisible dead end.

Clearing on the transition also keeps the sweep honest. `unregister_collection_item_parse_bindings`
matches the entry's address, so an entry that has been re-addressed to a live row is no longer swept
when the row it used to render is removed, and an entry still naming a removed row is swept with its
error. Both are what a mount-owned blocker should do.

## Rejected: call the situation misuse

`docs/collection-fields.md` requires a row that calls a collection-item hook to be a component keyed
by its **Collection Item Identity**, and under that rule a row's identity cannot change without a
remount. Declaring anything else unsupported would make this a documentation defect plus a debug
assertion, with no behavior to define.

It does not hold, because the rule is about *rows*. A scope that is not a row — a page-level control
bound to the first line, a detail panel bound to the selected row, a virtualized slot — is
unaddressed by it, and the root scope of a `VirtualDom` cannot carry a key at all, so the rule is not
merely silent there but unstatable. The library's own `collection_listener_parse_blocker_probe`
binds `items()[0]` positionally. A rule that a supported scope cannot satisfy is not a rule, and an
assertion derived from it would fire on code the library never told anyone not to write.

The keyed-row requirement is untouched by this. A row still keys by identity, and this decision is
not licence to key a row positionally; it decides what happens in the scopes that requirement was
never about.

## Rejected: unregister and re-register on every change

Dropping the registration and taking a fresh one has the right shape — new item, new mount — and it
terminates by construction. It cannot be relied on to run. `ParseBindingRegistration` is
`Rc`-backed, so replacing the hook slot does not drop the inner while a handler clone or a retained
binding still holds it, and `Drop` is where unregistration lives. The old blocker would survive the
change it was supposed to end, and a stale handler could still write through it, which is this
defect one identity later. Rebuilding it correctly means an explicit deactivation step — at which
point the registration is being re-addressed anyway, with id churn and a second notification as the
only difference.

## Rejected: let the caller address the mount

The mount could carry only lifetime and an id, with every read and write supplying its own field and
the stored error's stamp deciding what a read returns. It reads well — `ParseError` already carries
the field it was stamped with, so form-level reporting needs no address at all — and it makes a
retained clone's parse writes land where its value writes land.

It has no transition, and so it has exactly the dead end described above. It also drifts the entry's
address on write rather than on render, which leaves `field_parse_errors`, `has_field_parse_errors`,
the item sweep, and `Drop` reading an address set by whichever caller wrote last: removing a row can
then delete the entry backing a scope still mounted on another row, after which `set_parse_error`
finds no entry and silently discards every keystroke. And it redefines the entry's field from "what
this mount addresses" to "what last invoked it", which is invocation-owned state wearing
mount-owned vocabulary — it would need a new term in `CONTEXT.md` to describe honestly, where the
decision taken here needs none.

## Rejected: drop the entry's address and keep only the error's

Deleting `ParseBindingState::field` and deriving the field queries, the sweep, and `Drop` from the
stored `ParseError`'s own stamp removes the staleness rather than redefining it, and needs no new
address on the binding at all. It is a genuine simplification of the reporting half and could be
taken later on its own merits.

It is not this decision, because it answers nothing about the transition: an error stamped for the
old field is exactly as stranded when the only thing that knows the scope moved is the render that
did not happen to write. An address that is only ever a by-product of the last write cannot be
cleared when no write occurs.

## This is not ADR-0023's live resolution

[ADR-0023](0023-resolve-the-rendered-collection-item-index-live.md) retains a **Collection Item
Identity** and resolves its rendered index on demand: one durable identity, one derived projection.
Nothing here resolves on demand. The identity a mount addresses is *replaced* when the scope renders
a different item, and the replacement is an event with consequences — state cleared, a selector
notified — not a read. Calling it live resolution would suggest the address could be recomputed
from something the registration already holds, and it cannot: only the render knows.

## Consequences

The hook changes, because only the hook sees the transition. The parse helpers gain the caller's
address, which the two binding cores can supply without resolving anything, since a
collection item child **Field Identity** is derivable from the item and the child path alone. Both
the trait carrying it and the registration are crate-private, so the public surface does not change
and neither serialization version moves.

A scope that re-addresses loses its in-flight raw text, and the input renders the new item's
formatted value. That is the intended reading: text typed for one logical item is not input for
another, and a **Collection Item Identity** denotes one logical item for as long as any binding
holds it ([ADR-0025](0025-mint-collection-item-identities-from-a-never-rewinding-counter.md)).
Reordering is unaffected, because reordering does not change an item's identity.

Registrations built outside a hook are untouched. `CollectionItemBinding::parsed_text_with` mints a
fresh registration per call and drops it with the binding, so a per-render caller loses its raw text
on the next render and never holds a blocker. That is a mount-lifetime hazard rather than an
addressing one, it fails loudly and identically for direct fields, and "use the hook" already
answers it — though the warning currently sits only on the hooks and belongs on the direct
constructors too.
