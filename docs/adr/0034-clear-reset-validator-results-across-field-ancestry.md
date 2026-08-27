# Clear reset validator results across Field Ancestry

`reset_field` clears stored field-validator results for the reset **Field** and every **Field** in
**Field Ancestry** with it. The reach is symmetric, with one reset-local widening: resetting a
**Collection Field** also clears item-scoped results for the baseline rows it keeps. No validator
runs, no relative value is read, and only the reset **Field**'s own interaction metadata is cleared.

## A reset replaces more than the value at its exact identity

Resetting a containing **Field** assigns its **Baseline Value** over the whole subtree. Every
descendant value is therefore replaced. Resetting a leaf likewise changes the value of every
containing **Field**. A stored field-validator result on either side describes a value the form no
longer holds, so exact-identity clearing leaves stale **Validation Errors** and stale pending state.

This is the same symmetric **Field Ancestry** reach already used to clear **Submit Errors** after a
field replacement. It also follows the domain rule in `CONTEXT.md`: a write reaches the values,
**Validation Errors**, **Stale Submit Errors**, and value-replacement **Field Listeners** of the
fields it contains and the fields that contain it.

The shared `FieldAncestry::relates` predicate deliberately keeps a **Collection Field** unrelated to
its own item identities so collection structure changes do not wake every item value selector. A
collection reset is different: assigning the baseline collection replaces the values of the rows it
keeps as well as dropping added rows. The reset path therefore combines symmetric `relates` with
directional `contains(reset_field, validator_field)`. This is local to reset and does not change the
shared predicate, selector reach, or validator selection.

## Clearing remains deliberately conservative

A field validator can inspect the whole **Form Model** through its **Validator Context**. Clearing a
relative's result can therefore discard a verdict that depended on data outside the reset subtree,
and reset runs nothing that recomputes it. This loss is accepted.

It is not a new hazard introduced by ancestry reach. `reset_field` already cleared every validator
attached to its exact identity, regardless of which model data that validator read or which triggers
it registered. Widening the clear extends that existing conservative contract to every value the
reset assignment replaces. It also preserves the documented behavior that reset clears stored
errors rather than immediately manufacturing new ones.

Clearing is unconditional on trigger set. Restricting it to submit-triggered validators was declined:

- Commit-only and change-only results are exactly the stored verdicts that **Submit Availability** may
  consult even though submit authority does not.
- **Native Browser Submission** may run without client preflight, so submit-time regeneration is not
  universal.
- Correctness would depend on submit validation never becoming status-aware.
- Trigger filtering would give reset a third reach rule alongside exact identity and **Field
  Ancestry**.

Re-running related validators was also declined. A reset has no validation trigger, running every
related chain would bypass **Validation Mode**, validators that had never run could create errors on
baseline values, and the strict collection boundary would still omit the item validators most
directly affected by a collection reset.

## One predicate controls the guard and the clear

The validation-state no-op guard and the mutating branch use the same reset reach predicate over both
the ordinary field-validator table and the instantiated collection-item validator-state table. A
related result must force the mutating branch even when the reset field's exact value and metadata are
already at baseline.

That branch increments the form and field versions before clearing results. This ordering preserves
the two-phase submission invariant: a **Submit Validation Token** captured before the reset is retired
and cannot start a **Submission** after evidence it relied on was cleared. Clearing in the early-return
branch would remove stored verdicts without changing either version and could authorize the old token.

## Consequences

Resetting a container clears descendant validator results; resetting a leaf clears container results;
siblings remain untouched. Resetting a **Collection Field** clears kept rows' item-scoped results in
place while existing collection cleanup removes dropped rows' state.

The adapter's existing validation-changed transition wakes mounted validation-error readers after the
core clear. Descendant touched, blurred, and committed metadata survives a container reset: metadata
is validator input, and mutating it without recomputing verdicts was separately declined in issue #63.

Sync form validators that emit field-targeted errors are outside this decision. Their source state
cannot be partially cleared without falsely turning an un-run validator valid.
