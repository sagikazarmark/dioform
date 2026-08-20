# Decline per-intent retention of submit-triggered validator results

Dioform will keep one current result per **Validator Source**, including for submit-triggered
validation, rather than retaining one result for every **Submit Intent**. A submit run replaces that
source's stored verdict from a different intent. Intent-scoped **Submit Availability** and visible
**Validation Errors** therefore describe the submit-triggered verdict for the most recently attempting
intent; once another intent runs the source, the earlier intent's verdict is no longer retained
([issue #59](https://github.com/sagikazarmark/dioform/issues/59)). The next submit attempt still runs
submit-triggered validation for its own intent before application submit behavior.

## One source has one result slot

The per-source validation lifecycle stores one **Validation Status**, the trigger that produced it, the
attempting **Submit Intent**, and one error list. Intent-scoped reads filter that slot by its stored
intent; they do not select among an intent-keyed history. A later submit run writes its own status,
trigger, intent, and errors into the same slot.

The asynchronous lifecycle has the same cardinality. Each source owns one current run identifier and
one pending-run state. Starting a newer run advances that identifier so an older completion is a
**Stale Validation Result**. Retaining results per intent would also require per-intent run identifiers
and pending states, permit several live runs for one validator source, and define how those runs jointly
produce source status, pending blockers, cancellation, and staleness. That multiplies the lifecycle by
an application-defined, potentially unbounded intent type to preserve verdicts that submission must
rerun before relying on them anyway.

## Submit-scoped state is deliberately ephemeral

`FormStateSnapshot` omits submit-triggered source results entirely. The stored intent is also skipped
during serialization because **Submit Intent** is an arbitrary application-defined type. Retaining an
intent-keyed result map would create a richer live-state model that the serialization boundary
deliberately cannot preserve, while making reset, reinitialization, restoration, and source removal
responsible for an otherwise unnecessary collection of old submit verdicts.

This decision keeps submit-scoped validation as the latest evidence from one source, not a cache of
authorization decisions. **Submit Availability** remains a read-only known-blocker signal. It may be
optimistic for an intent whose older verdict was replaced, but a fresh attempt self-corrects by running
that intent's submit-triggered validators before application submit behavior begins.

## An unknown-intent blocker is not a sound substitute

Treating "not yet validated for this intent" as a conservative blocker does not recover the discarded
verdict coherently. Applied consistently, every intent is blocked on a pristine form because none has
yet been attempted. The buttons that start validation would begin disabled.

Applied only after a different intent has run, the same absence of knowledge blocks in one form state
but not another. Remembering only that an intent was previously blocked, without retaining its errors,
reproduces the pathology addressed by
[issue #53](https://github.com/sagikazarmark/dioform/issues/53): **Submit Availability** reports a
**Submit Blocker**, no visible **Validation Error** explains it, and the user has no corrective action
that clears it. A new blocker variant would encode the same inconsistency rather than resolve it.

The conservatism documented for async submission does not promise cross-intent retention. It says that
stored errors from non-submit triggers can block availability even when a submit attempt might rerun
validation and proceed. Submit-triggered results are instead filtered by the intent in the source's
single current slot.

## ADR-0019 is unaffected

[ADR-0019](0019-decline-can-submit-when-invalid-opt-out.md) relies on a Publish-only requirement not
blocking Save Draft. That direction still holds: after a failed Publish attempt, Save Draft availability
does not inherit Publish's submit-scoped error, and a Save Draft attempt reruns the source for Save Draft.
After that run, a read of Publish availability no longer sees the older Publish verdict. ADR-0019 does
not depend on retaining that reverse reading.

## Consequences

**Per-intent reads have a latest-run boundary.** `form.intent(intent).availability()`,
`form.intent(intent).can_submit()`, `visible_validation_errors_for_intent`, and
`visible_field_validation_errors_for_intent` filter the currently stored submit-triggered verdict. They
do not reconstruct a verdict replaced by another intent's run.

**Availability is not authorization.** An available intent can still be refused when its submit attempt
runs current validation. Applications should use availability for current UI guidance and the submit
result for the attempt's outcome.

**Synchronous and asynchronous validators follow the same retention rule.** Async validation starts a
fresh run when the current source result belongs to another intent; it does not reuse that intent's
success or error.

## When to revisit

Reopen if a concrete application must present several intents' previously computed submit verdicts at
the same time, and rerunning validation on attempt cannot meet the interaction. A superseding decision
must define a bounded or explicitly managed intent key, concurrent async runs and their cancellation,
per-intent pending and stale status, reset and source-removal behavior, error visibility and clearing,
and whether the retained state can remain intentionally absent from `FormStateSnapshot`. It must also
provide an explainable user action for every blocker rather than reintroducing an errorless blocker.
