# Track submit-validation currency with a dedicated generation

A **Submit Validation Token** proves more than equality of the **Form Draft**. Submit validators can
read field metadata as well as the whole model, file selections are validated and submitted outside
the draft, collection logical identities determine which item a validator addresses, and the final
authorization depends on the validators and stored results that established the pass. Dioform will
therefore track this proof with a dedicated monotonic submit-validation generation captured by the
token and compared again when the **Submission** starts.

The generation is global to one form. Field validators can read the whole draft, form validators can
read any field's metadata, and a validator may decide whether an intent matters inside application
code, so the current registrations do not expose enough information to prove that a transition is
irrelevant to one field or intent. A retired token never becomes current again merely because values
later return to an earlier state.

The generation advances when validation-relevant inputs, obligations, or evidence change: every
successful field or collection-item replacement; an actual change to validator-visible field
metadata; file-selection or collection logical-identity changes; reset, reinitialization, and state
restore; registration or removal of a submit-applicable validator; and operations that discard or
supersede evidence outside the validation cycle the token represents. It does not advance for an
unresolved or rejected write, presentation-only state, notifications by themselves, or validation
result progression that belongs to the same validation cycle.

## Why not reuse the existing versions

Form and field versions also decide whether asynchronous validation results are stale. Expanding them
to every metadata, validator-registry, and evidence transition would invalidate unrelated async work
and continue conflating two different questions: whether one async result still describes its input,
and whether a submit attempt still holds complete proof. Reconstructing every validator obligation at
the final gate was also declined; absence of an error is not proof that every required validator ran.

Existing unbounded setters remain conservative successful replacements and retire submit-validation
proof even when an application considers the assigned value equal. Adding `PartialEq` cannot solve
that generally: custom field values need not implement it, and a fallback-equal write through an
optional materializing path can still change the whole draft. A future equality-aware assignment API
requires its own equivalence contract and must suppress every proof-invalidating effect if it is to
preserve a token.

The documented submit-listener ordering remains validation, `SubmitAttempted`, final authorization,
then `SubmissionStarted`. Listener mutations stay useful, and the final authorization detects any
validation-relevant transition they cause. Automatic revalidation or continuation after retirement is
a separate policy decision.

The generation is live proof state rather than persisted form state. Restoring a state snapshot
retires current proof instead of restoring its generation, and no serialized snapshot carries a
**Submit Validation Token**, so this decision does not change the form-state serialization format.
