# Clear sync validator results on write across Field Ancestry

An ordinary write clears the stored results of synchronous validators whose values it replaced —
the reset **Field** and every **Field** in **Field Ancestry** with it, plus the written **Field**'s
own collection rows. This generalizes [ADR-0034](0034-clear-reset-validator-results-across-field-ancestry.md)
from `reset_field` to every write path, and it covers sync form validators' field-targeted errors,
which ADR-0034 left outside its scope. No validator runs, no metadata is written, and the clearing
is unconditional on trigger set and on **Validation Mode**.

## A stored verdict was invalidated only by a trigger re-running

A stored synchronous validator result was invalidated only when a **Validation Trigger** the
validator is registered for ran again. Nothing invalidated it when the value it was computed over
changed. A Commit-narrowed validator driven invalid by a Commit kept its **Validation Error** through a
write that corrected the value — at exact **Field Identity**, with no containment involved — and
the error stayed *visible*, because the written **Field**'s committed flag survives a write.

The consequence is a contradiction inside one core. **Submit Availability** counts stored errors
without a trigger gate, so `can_submit()` reported a blocker, while `submit()` on the same state
ran the application's submit behavior and succeeded. The library reported a form as unsubmittable
and then submitted it.

This is the general class behind [#55](https://github.com/sagikazarmark/dioform/issues/55), whose
`reset_field` instance ADR-0034 fixed by making one method's reach internally consistent. The
remedy here is the same one, applied where the values are actually replaced.

## Clearing, not re-deriving and not disbelieving

Three mechanisms were implemented or evaluated.

**Re-running related validators on write** is what **Validation Mode** already does when it is
configured to, and widening it unconditionally was declined for the reasons ADR-0034 records:
chain selection filters on ancestry and trigger, never on status, so a widened pass takes validators
from unknown to invalid and manufactures errors on values the user never touched. A write is not a
mode change.

**Read-time staleness** — stamping each verdict and disbelieving it when a version in
**Field Ancestry** has moved — is what the issue originally proposed, and it is sound. It is
declined on cost. The precedent it would follow, `submit_error_applies_to_current`, is affordable
because one **Submission** clones the field-version map once at `begin_submission`. A sync verdict
has no such carrier: the stamp would be cloned per validator run, on every keystroke under a
change-validating mode, and compared by an ancestry walk on every error read. It would also have to
persist, forcing a **Form State Snapshot** format change and a reconciliation rule against a
restored version map that is replaced wholesale.

**Clearing at write** needs no new state, no new status, no read-path walk, and no format change.
Its cost is that it discards a verdict nothing recomputes — a field validator can read the whole
**Form Model** through its **Validator Context**, so a cleared result may have depended on data
outside the values the write replaced. That loss is accepted here on the same grounds ADR-0034
accepted it: exact-identity clearing already discarded such verdicts on the reset path, and a
verdict describing a replaced value is not knowledge.

## The reach is symmetric, with both collection orientations

The predicate is symmetric `FieldAncestry::relates` widened by `FieldAncestry::contains` in **both**
argument orders. Both are needed, and the reset path needed only one.

`reset_field` takes a **Field Path**, so its actor identity is always static and a **Collection
Field** can only ever be on the actor's side. A write's actor can be a collection-item identity, and
then the collection sits on the *verdict*'s side instead. With only the reset orientation, a
keystroke in a row reaches a verdict on a static ancestor of the collection but not one on the
collection itself, giving the nearer registration strictly less reach than the further one — the
inversion [ADR-0028](0028-match-listener-reach-to-what-each-event-asserts.md) declined to ship. The
mirror orientation is the one the value-replacement listener filter already uses.

This does not relax the shared predicate. `FieldAncestry` keeps its clauses, its strictness, and its
unit tests; selectors and validator selection depend on a collection staying unrelated to its own
items.

A symmetric clear on a write and the outward-only selection
[ADR-0035](0035-select-commit-validators-outward-from-the-field-that-committed.md) chose for a Commit are
one criterion applied to two events, not two rules. A value change asserts that the value at a path
was replaced, which is true of the **Fields** it contains and the **Fields** that contain it. A Commit
asserts only that an interaction completed at one **Field**.

## Every write path, identified by what it already clears

The clearing belongs wherever stale **Submit Errors** are already cleared, and for the same reason:
a **Stale Submit Error** and a stale validator verdict are both verdicts about a value the form no
longer holds. Pairing them makes the site list mechanical rather than enumerated, which matters
because this defect exists in the first place from fixing one write path in isolation.

A collection reorder is included. Moving a row mutates the draft, so the **Collection Field**'s own
value changes and a verdict on it is stale; the strict collection clause is what keeps surviving
rows' item-scoped verdicts untouched, and no per-operation carve-out is required or expressible —
the six collection mutators share one tail that cannot distinguish them.

Clearing is guarded to sources holding a verdict, matching the guard the adjacent async
invalidation already applies. A validator that never ran has nothing to clear, and clearing pending
state is the async invalidation's job.

## Form validators are in scope, and their status must collapse to unknown

Both first-party **Validation Adapters** register whole-model validation as a *sync form validator*
that emits field-targeted errors, and there is no per-field mode. Leaving form validators out would
leave the defect intact for every application using an adapter, which is the library's primary
validation story.

Their stored errors are cleared per error by target, the way item-scoped cleanup already clears
them. ADR-0034 stated that a form validator's source state "cannot be partially cleared without
falsely turning an un-run validator valid"; the capability claim was wrong — that partial clearing
already ships — but the hazard it names is real, because the partial-clear path promotes a source to
valid when its last error is removed. On this path the status must collapse to unknown instead: the
validator did not pass, its verdict was discarded.

A form validator's error can be *caused* by a **Field** outside the ancestry of the **Field** it is
*targeted* at, so a write to that cause clears nothing. This under-reach is accepted and is the same
loss accepted above for field validators. It fails safe: the stale error stays visible and blocking
until the next Commit or submit attempt, both of which re-run the whole form chain unconditionally.

## Consequences

Under a Commit-scoped **Validation Mode**, an error stored by a Commit stops rendering on the next
keystroke and returns on the following Commit if the value is still invalid. This is a user-visible
change and it applies under every **Error Visibility** policy, because clearing removes the error
from the store rather than filtering it from a view.

`can_submit()` stops reporting a blocker for a verdict whose value has been replaced. For a
validator registered for neither Commit nor submit, nothing re-runs it, so its verdict is gone until
an explicit `validate_field` call. This is deliberate: an unrefuted verdict about a value that no
longer exists is not evidence, and the alternative — blocking submission on a verdict the user is no
longer shown — is the invisible submit blocker ADR-0035 removed.

Two tests changed rather than being preserved. Both asserted that a stored verdict survives a write,
which is the behavior this decision reverses.

**Submit Availability** stays conservative about pending work and submit-scoped verdicts. It is no
longer conservative about verdicts describing values the **Form Draft** has replaced.
