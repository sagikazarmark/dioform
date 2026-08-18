# Report a retired Submit Validation Token as its own Submit Blocker

A **Submission** refused because the **Submit Validation Token** presented to it no longer describes the
current **Form Draft** reports `SubmitBlocker::StaleSubmitValidation`, a fifth variant, rather than
borrowing `ValidationErrors`. The variant is chosen ahead of `PendingValidation` and behind
`ValidationErrors`. It is an outcome-only blocker: **Submit Availability** can never contain it.

## The category was not imprecise, it was false

`begin_intent_submission_after_validation` refuses on five disjoint conditions — two comparing the
token's `form_version` and `field_versions` against the live draft, three describing real validation
state — then asks `submit_validation_blocker_for_intent` to name the reason. That function knows only
two of the five: pending or unresolved async gives `PendingValidation`, and *everything else* falls to
`ValidationErrors`. Staleness is in the everything-else.

With zero validators registered the refusal reports `SubmitAttempt::Blocked(ValidationErrors)` while
`validation_errors()` is empty, every visible-error accessor is empty, and `submit_availability()`
reports no blockers at all. The two public signals contradict each other: the form says "your submit
was blocked because of errors" and "you may submit" at the same instant.

The difference from the other four blockers is what forces a new variant rather than a new accessor.
`ValidationErrors` has `validation_errors()`, `ParseErrors` has `parse_errors()`, `PendingValidation`
has `validation_statuses()`; each is a true category that may want more detail. A retired token has no
detail to withhold. The actual reason — the draft moved, the verdict no longer applies, submit again —
had no representation in the type at all.

## Why a blocker, when staleness is not a condition of the form

The honest objection to this decision is that the other four blockers are standing conditions of the
form, and a retired token is a relation between the form and a value the *caller* holds. That
objection is correct, and it is why the asymmetry below is documented rather than smoothed over. It
does not win, for two reasons that are structural rather than aesthetic.

`SubmitListenerEvent::SubmitBlocked` carries a `SubmitBlocker`. An outcome modelled outside the blocker
vocabulary cannot ride that event, so a single refusal would need a second **Submit Listener** event —
permanently splitting the listener surface for one case, on an enum applications match.

`SubmitBlocker` is `#[non_exhaustive]`; `SubmitStatus`, `SubmitAttempt`, and `SubmitResult` are not.
The cost of moving the outcome to any of them is not hypothetical: this repo's own demo stops
compiling on `SubmitResult`, which it matches with four arms and no wildcard. Adding a `SubmitBlocker`
variant costs no consumer a match arm.

Considered and rejected: **falling through to re-validate instead of refusing**, which
[ADR-0007](0007-use-browser-submit-preflight-without-async-waiting.md) already closed by declining to
"invent resubmit orchestration after async validation". One argument *for* it is false and is recorded
here so it is not re-raised as support — a fall-through would not ship a submission whose async submit
validators never saw the new value, because every path that bumps `form_version` also invalidates async
field validators for the model change, so the re-run would simply block on `PendingValidation`. It
fails for a different reason: it turns a terminal outcome into an implicit retry loop, so a user who
keeps typing never receives one. Whether the *adapter* should retry is a separate question with its own
issue; this decision is about what a refusal is called.

## Precedence: staleness outranks pending, and yields to real errors

The variant alone fixes only the narrowest case. Staleness co-occurs with unresolved async validation
in most refusals — an edit during in-flight validation both retires the token and re-opens the
validator — and in that cell `PendingValidation` wins today. The ordering matters more than the
variant.

Staleness is placed **ahead of** `PendingValidation` because in that cell the two are not independent
facts. The unresolved validator and the retired token are one event described twice, and only one
description is actionable: nothing is running — the adapter has already finished its managed async
submission — so reporting pending validation implies that waiting will help, when only submitting again
will.

Staleness is placed **behind** `ValidationErrors` because real submit-blocking errors are true
independently of the edit that retired the token and remain actionable on their own. Telling a user to
submit again when the form genuinely has errors sends them through a round trip to arrive at the errors
anyway.

The alternative ordering — name staleness only when nothing else is true — was measured and costs zero
test changes, against eight for this one. It was declined because the zero-churn result is achieved by
leaving the common case reporting the wrong thing. Six of those eight tests are *named* for staleness
while asserting `PendingValidation`; the changed assertions are those tests finally able to say what
their names always meant.

The funnel cannot express this ordering, because it never sees the token. Staleness is decided at the
refusal site, which has both, and the funnel keeps its own last-resort arm as a defensive default.

## An outcome-only blocker is a deliberate asymmetry

**Submit Availability** computes purely from live form state and takes no token, so it can never
produce this variant. It is the first `SubmitBlocker` that `SubmitAvailability::blockers()` cannot
contain, and the first that does not round-trip between the two signals. `contains` will accept it and
answer `false` forever.

This is inherent, not an omission, and it is the precise reason the objection above was overruled
rather than dismissed. **Submit Availability** stays an unconditional "no known blockers" signal in the
sense [ADR-0019](0019-decline-can-submit-when-invalid-opt-out.md) defends — it answers about the form,
not about a submission someone attempted. What changes is not availability's meaning but the scope of
the blocker vocabulary, which is now larger than what availability reports.

The asymmetry must be documented rather than discovered, because the contradiction it produces is
exactly the symptom that motivated this decision: at the instant of a stale refusal `can_submit()` is
`true` and `blockers()` is empty while **Last Submit Status** reports blocked. That reading is now
correct — the form *is* submittable, and the attempt that failed used a token that no longer applies —
but it reads as a bug unless the invariant is stated.

## Consequences

The adapter's blocker match arms group by which selector notification a refusal emits. The new variant
is named explicitly rather than left to a catch-all, and notifies the submit transition only: a stale
refusal writes the submit status and nothing else, so a whole-form notification would claim that
values, parse errors, and validation errors had all changed. The synchronous catch-all it would
otherwise have fallen into emits no notification at all — a pre-existing inconsistency with its
sibling, fixed alongside.

`docs/async-validation.md` documents `SubmitBlocker::ValidationErrors` as the blocker for
submit-relevant async validation that is "still pending, stale, unknown, or must run". That sentence
encodes the defect and is superseded.

`SubmitBlocker` derives serde under a feature, but no `FormStateSnapshot` field carries it and
`docs/form-state-serialization.md` excludes submit status by design, so
`FORM_STATE_SERIALIZATION_VERSION` is not bumped. Applications that persist **Last Submit Status**
themselves are on an unversioned format and will see an unknown variant; that exposure is inherent to
serializing an outcome type and is not created by this decision, but it is real.

`CONTEXT.md` gains **Submit Validation Token** as the name for the held proof. The type is still called
`SubmitValidationSnapshot`, which collides with **Submission Snapshot** and **Form Snapshot** — three
"snapshots" for three different things, only one of which carries form values. Renaming the type to
match the domain term is deliberately **not** part of this change and needs its own decision; until
then the mismatch is known rather than accidental.

## What this does not fix

`SubmitBlocker::InFlightSubmission` remains un-scoped to the **Submit Intent** actually in flight. That
is deliberate — `CONTEXT.md` and ADR-0019 both hold that an **In-Flight Submission** blocks every
intent — but the intent is stored without a public reader, so a Save-Draft button still cannot report
that Publish is the submission running. That is a missing accessor, not a blocker-category question.

A refusal can also fire when nothing semantically changed, because a value-preserving write still bumps
the version counters this decision compares. That makes the refusal spurious; it does not make the
category wrong. The variant is therefore documented against the verdict's currency rather than against
a semantic change, so it stays true whether or not the spurious refusals are fixed.
