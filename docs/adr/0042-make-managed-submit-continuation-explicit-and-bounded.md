# Make managed submit continuation explicit and bounded

A **Dioxus-Managed Submission** terminates by default when its **Submit Validation Token** is retired.
One managed submit request may instead opt into **Managed Submit Continuation**, which permits one
additional full submit-validation cycle for the same **Submit Intent** after an ordinary **Form Draft**
replacement retires the first token. The additional cycle captures fresh proof and never refreshes or
reuses the retired token ([issue #66](https://github.com/sagikazarmark/dioform/issues/66)).

## Continuation is request-scoped

Continuation is selected on one managed submit request rather than stored in **Form Configuration**.
Strict behavior therefore remains visible at every call site unless that request opts in, and two
submit triggers for the same form may choose different policies. This matters for intentful forms: a
Save Draft request and a Publish request need not accept the same post-request changes.

The policy applies only to managed async submission. Synchronous managed submission cannot wait for
another async validation cycle, and **Progressive Submission** remains browser-owned under
[ADR-0007](0007-use-browser-submit-preflight-without-async-waiting.md).

## One additional cycle, not a retry loop

The first eligible retirement schedules the additional cycle on the managed continuation task. On its
next poll, the task captures the then-current validation-relevant state and immediately starts full
submit validation. Eligible replacements completed before that capture naturally coalesce into the
new cycle; Dioform does not add a debounce or quiescence delay.

The cycle is spent when its fresh token is captured and its synchronous submit-validation pass begins.
Any proof-retiring transition after that capture is terminal. There is no third cycle and no
configurable retry count. This bound prevents one request from owning the in-flight slot indefinitely
while a user, listener, or application keeps changing the form.

## Eligibility follows the operation that retired proof

Successful ordinary draft operations are eligible regardless of whether their origin is user,
listener, binding, or programmatic behavior. They include direct field replacement, collection-item
and item-field replacement, direct collection value replacement, and collection insertion, removal,
move, swap, or clear. A successful same-value replacement remains eligible because Dioform's ordinary
replacement APIs do not promise equality-aware assignment.

The state-clearing semantics of `reset_field` make it ineligible even when it also changes a value.
Full reset, reinitialization, state restoration, **File Selection** changes, standalone
validator-visible metadata changes, submit-applicable validator registration or removal, and
independent validation-evidence changes are also ineligible. A window containing both an eligible and
an ineligible retirement does not continue.

Update origin cannot decide eligibility. A programmatic write may be a benign normalizer or an
unrelated background replacement, while a listener write may be a direct consequence of user input.
Applications opt into the policy with that ambiguity visible instead of Dioform guessing which origin
expresses consent.

File changes remain terminal because a newly selected platform file is a distinct submission payload
and consent boundary outside the **Form Draft**. A fresh user request is required before application
submit behavior may receive it.

## Proof currency and continuation eligibility are separate

[ADR-0041](0041-track-submit-validation-currency-with-a-dedicated-generation.md) gives every
validation-relevant transition one monotonic proof generation. Continuation adds a second monotonic
barrier generation for ineligible retirement roots. A changed proof generation with an unchanged
barrier means every retirement since capture was eligible; a changed barrier makes the attempt
terminal. The generations advance atomically with the operation they classify, so a later eligible
write cannot hide an earlier ineligible transition.

The proof generation remains the only authorization check. The barrier can permit another validation
cycle, but it can never make an old token current or authorize submission. Implementation of
[issue #71](https://github.com/sagikazarmark/dioform/issues/71) is therefore a prerequisite for this
decision's implementation.

## Continuation is one observable submit attempt

The additional validation cycle does not increment the submit-attempt count, dispatch another
`SubmitAttempted`, write an intermediate `SubmitBlocked`, or record an intermediate **Last Submit
Status**. It retains the original **Submit Intent**, payload factory, application submit behavior, and
managed in-flight ownership. Validator and validation diagnostics may still describe work performed
by the additional cycle.

The request eventually dispatches one `SubmissionStarted` or one terminal `SubmitBlocked`. Existing
blocker precedence remains unchanged: current `ValidationErrors` outrank `StaleSubmitValidation`,
which outranks `PendingValidation`. Exhausting the continuation through another retirement therefore
reports `StaleSubmitValidation` only when no current validation errors outrank it. The initial managed
wait keeps its existing recorded `PendingValidation` status; this decision adds no continuation-time
status transition.

A later submit press remains a distinct blocked request. It reports `InFlightSubmission` without
replacing the waiting request, changing its intent, consuming its continuation, or supplying a new
handler or payload.

## Lifecycle cancellation is owned separately

Reset, reinitialization, successful state restoration, and cleanup invalidate a managed-validation
window rather than continue it. A cancelled waiter must not later mutate fresh submission state or
clear a newer request's ownership. That pre-existing request-identity defect is tracked by
[issue #72](https://github.com/sagikazarmark/dioform/issues/72), whose implementation is also a
prerequisite for managed submit continuation.

## Alternatives declined

Automatic continuation by default was declined because one click could submit field values or files
chosen after the click without renewed assent. Unbounded latest-state convergence was declined because
continuous writes could prevent a terminal outcome and repeatedly invoke external validators.
Origin-sensitive continuation was declined because origin does not describe whether a replacement is
safe to submit. Form-wide configuration was declined because it hides the departure from strict token
handling and cannot express different choices for different submit triggers.

[ADR-0033](0033-report-a-retired-submit-validation-token-as-its-own-submit-blocker.md) still governs
strict managed submission, direct core callers, ineligible retirement, and continuation exhaustion.
Its retired-token blocker remains an outcome of an attempt; this decision only gives one explicitly
configured adapter request a bounded opportunity to obtain fresh proof before that outcome becomes
terminal.
