# Do not extract a chain executor module

Dioform will keep the sync-before-async validation-chain orchestration inside **Form Core**
rather than extracting a separate `ChainExecutor` module. This reverses the speculative third part
of [ADR-0010](0010-carve-form-core-into-field-store-submission-state-and-chain-executor.md), which
proposed the extraction only if its outcome interface stayed clean. On implementation it did not.

Running one field validator requires the current **Form Draft** model, the field's metadata from
the **Field Store**, the submit intent from the **Submission State**, and mutable access to the
`ValidationChainRegistry` to store the result, all at once, in `validate_field_validator_key`. A
`ChainExecutor` module would have to borrow four **Form Core** subsystems simultaneously, so it would
relocate the coupling into another file without concentrating it: deleting the module would not
collapse complexity, which means it is not a real seam. The one available in-place win (returning
outcomes instead of emitting `Form Observer` events inline) buys little, because the test suite
already asserts chain results through direct **Validation Status** readers rather than observer
capture.

The genuinely deep modules for validation already exist: `validation_lifecycle::SourceState` owns the
per-source state machine, `ValidationChainRegistry` owns validator storage and the chain-iterating
submit-availability queries, and `SubmissionState` owns submit-scoped state and its stored submit
errors (see ADR-0010). The orchestration that remains in **Form Core** (sequencing sync
before async, choosing skip-versus-clear for async validators, and iterating field and collection-item
keys) coordinates those deep modules and legitimately lives at the coordinating layer. Future
architecture reviews should not re-suggest extracting it unless the per-validator execution stops
needing draft, field-store, submission, and registry access together.
