# Read the in-flight Submit Intent from the window that owns it

**Form Core** and the **Dioxus Adapter** each gain a typed, non-consuming reader for the **Submit
Intent** of an **In-Flight Submission**, and an intent-scoped predicate so a submit button can ask
whether its own intent is the one running. Each reader answers for the same window in which its layer
reports `SubmitBlocker::InFlightSubmission`: the core's window is the accepted submission, and the
adapter's is the accepted submission together with the managed async-validation wait. The waiting
intent is stored in the waiting window itself rather than mirrored into a second store, and
`SubmitIntentSnapshot` stays crate-private.

## The reader's window is the blocker's window

The alternative — reading only the accepted submission at both layers — leaves the adapter reporting
`InFlightSubmission` for a window it cannot name. A form waiting on submit-relevant **Async
Validation** already answers `true` to `is_submitting()` and already blocks every intent, so a Save
Draft button correctly told it is blocked would still be unable to say that Publish is the reason.
That is the deficit [ADR-0033](0033-report-a-retired-submit-validation-token-as-its-own-submit-blocker.md)
named and deferred, and a reader narrower than the blocker it explains reproduces it one lifecycle
phase earlier.

Tying each layer's reader to that layer's blocker keeps the two windows honest without unifying them.
The core has no knowledge of the adapter's wait and keeps the narrower window; the adapter composes the
two exactly as `is_submitting()` already composes the two flags. **In-Flight Submission** is defined by
acceptance and names both phases inside itself, so neither reading needs a glossary change.

## The window is the intent

The waiting window was a bare flag with the intent held only in the detached wait task's captured
**Submit Validation Token**. It becomes an optional erased intent: opening the window and naming it are
one assignment, and a window that is open without an intent is unrepresentable.

Mirroring the waiting intent into the adapter's existing `active_submit_intent` was rejected. That
store is released at every site that closes the submission it belongs to, and on the primary managed
path the handoff writes the intent and closes the waiting window on consecutive lines — a store shared
by both windows is cleared one statement after it is written, for the whole duration of the
application's submit future. Making the release conditional on the core still being in flight repairs
that case but leaves the general one: a wait loop that closes a window it does not own would clear an
intent a live submission owns, and the same store feeds the listener dispatch behind
`finish_submission_success`, so an ordinary success would begin dispatching **Submission Succeeded**
with a unit intent and typed listeners would silently stop firing. A read-only addition must not put a
failure mode into an existing write path.

With one store per window, each released by the window that owns it, both cases are structurally
unreachable rather than avoided by discipline at eleven call sites. The unowned-window defect is real
and is recorded separately; under this shape a stale close loses exactly what it loses today.

## The adapter's core-window store stays

`active_submit_intent` cannot be deleted in favour of a core reader. `finish_submission_success` is
public and non-generic and feeds a listener dispatch that takes the intent already erased, because
listeners downcast it themselves. A typed core reader cannot be instantiated there, and the crate
boundary rules out naming `SubmitIntentSnapshot` from the adapter. Publishing that type, or exporting an
erased accessor through the core's doc-hidden private module, would each buy the deletion at the cost of
widening a seam [ADR-0010](0010-carve-form-core-into-field-store-submission-state-and-chain-executor.md)
keeps closed — and neither is needed once the waiting intent has its own store, since the core-layer
reader lives inside the core where the type is nameable.

## What the invariant can claim

The reader is generic and answers `None` both when nothing is in flight and when the caller asks for an
intent type other than the stored one, matching every other typed intent read in the workspace. A
biconditional with `is_submitting()` is therefore false for every type but the stored one, and cannot be
written as a general test. The invariant is the implication — a reader that answers `Some` is a form
that is submitting — with the biconditional pinned at the intent type actually submitted.

No liveness qualifier is attached. `is_active()` is a fence against late asynchronous completions
mutating a form after its UI is gone; no public read in the adapter consults it, dead-form reads
answering from stored state are pinned by tests, and the predicate is private, so a contract holding
"on live forms only" would name a condition callers cannot evaluate. A liveness gate would also
manufacture the violation this decision rules out: starting a submission through a handle whose form has
been cleaned up stores an intent and reports `is_submitting()`, and only a gated reader would answer
`None`.

## The scoped predicate keeps the scope's vocabulary

The intent-scoped read is `is_in_flight`, not `is_submitting`. Scoped submit types drop the qualifier
their scope already supplies — `submit_availability` becomes `availability`, `last_submit_status`
becomes `last_status`, across every scoped type in both crates — and `is_submitting` is the case where
the qualifier is the verb, so reusing it would put one identifier on two different meanings a single
`intent()` call apart.

The predicate narrows only the question it asks. `SubmitBlocker::InFlightSubmission` remains
un-narrowed, as [ADR-0019](0019-decline-can-submit-when-invalid-opt-out.md) requires: an intent that is
not the one running is still blocked by the one that is. "Publish is in flight" and "Save Draft is
blocked" are both true, and a button that wants to distinguish its own spinner from someone else's now
has a read for each.

## Every adapter submission entry point respects the waiting window

The managed async-validation wait is an **In-Flight Submission** at the adapter boundary, so it blocks
submission through every `FormHandle` entry point, not only managed and progressive submission
bindings. Direct intentless and intent-scoped submission therefore report
`SubmitBlocker::InFlightSubmission` through their ordinary attempted-and-blocked lifecycle while a
managed wait owns the window. They do not run validation, capture a payload, invoke application submit
behavior, or replace the waiting request.

Allowing a direct submission to overlap once the waiting window has request identity was declined.
Request identity would prevent the old waiter from clearing the newer submission's ownership, but the
overlap would still contradict the adapter's `is_submitting` and availability answers and could make
the waiter publish a late terminal block while another submission is already accepted. `FormCore`
remains unaware of the adapter-owned waiting window; this guard belongs to the Dioxus adapter entry
points that compose both windows.

## Consequences

**The reader can be `None` while the form is submitting when the caller asks for the wrong intent
type.** A managed wait carries request identity, so a waiter that no longer owns the window cannot close
it or erase the current intent. The implication remains the general typed-reader invariant because the
caller may still request a type other than the stored one.

**Availability and the reader may disagree in direction, and that is correct.** With Publish running, a
Save Draft scope reports no in-flight intent of its own and no availability, because the blocker is
un-narrowed by design. Applications that render per-button spinners should read the scoped predicate;
applications that render "why can't I submit" should read availability.

**The doc comment on the adapter's `is_submitting` is corrected here.** It claims a submission has
started, while the body has always also covered the managed wait — the same conflation this decision
resolves.

**`FORM_STATE_SERIALIZATION_VERSION` is not bumped**, and the glossary is unchanged. No serialized field
carries a **Submit Intent**, and **In-Flight Submission** already spans both phases.
