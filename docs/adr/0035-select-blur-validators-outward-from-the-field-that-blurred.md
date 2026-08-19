# Select blur validators outward from the Field that blurred

A **Validation Chain** entered on **Blur** selects only the validators registered on the **Field** that
blurred and on the **Fields** that contain it. Every other **Validation Trigger** keeps the symmetric
**Field Ancestry** selection [ADR-0020](0020-derive-field-ancestry-from-identity-paths.md) established.
The reach is read from the **Validation Trigger** already threaded through validator selection, and one
predicate carries it to all three selection sites — the field validator table, the collection-item
validator table, and the adapter's async starts. `FieldAncestry::relates` is not modified.

## ADR-0020 widened a write filter, and a blur is not a write

The widening ADR-0020 committed to is justified by value staleness: writing a **Field** changes the
value every validator in ancestry with it reads, so their verdicts are stale until they re-run. That
justification is recorded as a *write* justification, in the ADR and in the doc comments on both
selectors. A blur writes nothing. No **Validation Error** anywhere in the subtree became stale because
focus left a containing **Field**, so nothing in the recorded reasoning covers selecting a descendant's
validators on **Blur**.

What that unjustified reach produces is the defect this decision fixes. Blurring a container ran every
descendant validator and stored their verdicts on leaves the user never focused. Those errors are
invisible under blur- and touch-scoped **Error Visibility**, because
[ADR-0032](0032-widen-error-visibility-outward-from-the-field-an-error-is-attached-to.md) widens
visibility outward from the **Field** an error is attached to and a leaf contains nothing — and they
still block submit, because **Submit Availability** reads stored errors. The form was unsubmittable
with nothing displayed to explain why.

ADR-0020's own closing paragraph, as narrowed by
[ADR-0028](0028-match-listener-reach-to-what-each-event-asserts.md), already states the rule this
decision applies: value replacement uses the predicate as written, while blur and binding lifecycle
assert containment rather than replacement and reach outward only. That sentence was written about
listener surfaces. Validator selection is a third surface, unowned by either reach decision —
[ADR-0034](0034-clear-reset-validator-results-across-field-ancestry.md) names it as a sibling of
selector reach when scoping itself away from it — so the criterion is available here, and it is applied
here on this surface's own evidence rather than inherited as already decided.

## This is not the door ADR-0032 closed

ADR-0032 closed *narrowing selection back to exact identity*, on two grounds. The load-bearing one is a
consequence: blur enters through leaf bindings, and a leaf-input UI never writes the containing
object's path, so under exact-identity selection a validator registered on a container would never run
from leaf-driven interaction at all. Outward-only selection preserves that ground exactly. A leaf blur
still selects every containing **Field**'s validators, and the test pinning that reveal stays green.

ADR-0032 then named the mirror direction in its own closing section — a container blur storing
descendant errors that are invisible and submit-blocking — declined to fix it by making those errors
visible, and classified it: a selection defect with its own issue. This decision is that follow-up, not
a reversal.

One test shipped with ADR-0032 asserts the storing half of the defect while pinning the invisibility
half. It blurs a container and asserts that the descendant validator's error *is* stored and *is not*
visible. The stored assertion was scaffolding for the visibility question ADR-0032 was deciding, not a
contract; it is rewritten here to assert the descendant error is never created. It is the only test in
the workspace that changes behavior.

## The trigger is where the reach belongs

`CONTEXT.md` defines a **Validation Trigger** as a semantic form event, and a **Validation Chain** as
the validators for a field or form *and trigger*. Chain membership is already trigger-dependent, and
the **Dioxus Adapter** already maps `onblur` onto **Blur**. So the trigger is the event whose assertion
the reach criterion asks about, and keying reach off it needs no new parameter threaded through the
chain entry points. An application calling the field-and-trigger validation entry point with **Blur**
is asking for blur semantics; **Manual** remains the symmetric "validate this subtree" call.

ADR-0034 declined trigger-filtered reach for reset. That decision does not extend here: a reset has no
trigger at all, which is why filtering would have given it a third reach rule invented for the
occasion. Blur-triggered selection is reading a trigger that is already present.

The reach lives in one predicate beside **Field Ancestry** in the core, called from all three selection
sites, because ADR-0032 has already ruled that a directional clause belongs inside the companion
predicate rather than at the call sites, where deciding it per-caller would make three spellings of one
relation. Placing it in the core also makes the trigger match exhaustive: **Validation Trigger** is
`#[non_exhaustive]`, so the same match written in the adapter would need a catch-all arm that silently
absorbs any trigger added later, while the core match fails to compile until the new trigger's reach is
decided.

`Manual`, `Initial`, and `Submit` stay symmetric. The honest reason is not that their assertions demand
symmetry — **Manual** asserts only that the application asked about a **Field** — but that no
interaction defect sits behind them. **Initial** and **Submit** reach validator selection only through
the whole-form pass, which enumerates every registered validator's own **Field** and is therefore
indifferent to reach, and **Manual** has no caller inside the library at all.

## The collection component takes ancestor-or-equal

The directional predicate treats a static **Collection Field** identity as containing its own
**Collection Item** identities, where the symmetric relation stays strict at that boundary. Blur
selection inherits that clause, and it is the one place where this decision selects *more* than before:
a field validator registered on the collection path now runs when a **Field** inside one of its items
blurs.

That is a correction, not scope creep. Under the symmetric predicate a validator on a static ancestor
of the collection ran on a row-leaf blur while the nearer validator on the collection path itself did
not — the inversion where a nearer registration receives strictly less than a further one, which
ADR-0028 called unshippable and ADR-0032 fixed for visibility on the same grounds. Collection-level
verdicts such as "at least one line" or "no duplicates" are the idiomatic shape for that registration,
and a repeater's collection field can never be blurred directly, so keeping the boundary strict here
would leave those verdicts unreachable from user interaction. The widening is upward-only: no item's
validators are selected by a blur elsewhere in the collection, and a static descendant of a collection
path still does not relate to its items.

## Consequences

Blurring a container no longer stores **Validation Errors** on the **Fields** it contains, so the
invisible submit blocker is gone and **Submit Availability** is no longer decided by verdicts the user
was never shown. Blurring a leaf is unchanged in every respect. A blurred **Field** still runs its own
validators, because the directional predicate is reflexive.

Two reaches are deliberately unaffected. Form validators still run on any blur and may still attach
errors to any **Field**, because form-validator selection applies no field filter. A **File Selection**
blur is unchanged, because a file identity relates to nothing but itself under either predicate.

Applications that render errors under the unconditional **Error Visibility** policy lose a display they
have today: a container blur showed its descendants' errors, and now produces none until those
**Fields** are written or a submit is attempted. This is accepted. Under that policy the errors were
being shown against inputs the user had not focused, which is the presentation ADR-0028 rejected when it
declined to aggregate blurred metadata upward.

Composite bindings — a select, radio group, or parsed-text control bound to a struct-valued path — write
and blur the container path. Validators registered strictly inside such a path no longer run from that
control's blur, and under blur-scoped visibility their verdicts were never displayable from it anyway.

## What this does not fix

**A stale verdict inside a composite binding now survives longer.** Under
`ValidationMode::on_blur()`, a write neither re-runs nor clears a related **Field**'s stored sync
verdict, so before this decision the inward blur reach was incidentally refreshing descendant verdicts
for a UI whose every write names the container. After it, a descendant verdict that has already been
stored — by a submit attempt, or by that leaf being blurred once through a separate binding — stays
stored and visible until the next submit attempt, though the value it describes has been corrected. A
submit attempt recovers it, and pre-submit the change is strictly an improvement: no error is stored at
all where one was previously stored and invisible.

The defect underneath is that a stored sync verdict is invalidated only by a trigger re-running and
never by the value changing, which is
[#64](https://github.com/sagikazarmark/dioform/issues/64) — the same class at exact identity, with no
ancestry in it. Fixing it there requires deciding how a write stales a verdict when field versions are
deliberately not fanned out, and whether errors should stop rendering as soon as the user types. Neither
belongs in a selection fix.

**The stored-but-invisible class is narrowed, not closed.** A **Blur**-triggered form validator can
still attach an error to a **Field** the user never focused, and one shipped test relies on exactly
that. Selection reach cannot reach it, because form-validator selection has no field filter.
