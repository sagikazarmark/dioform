# Keep the per-target async validation entry points

Dioform will keep the adapter's async-validation entry points split by target (typed field,
field identity, and form) rather than collapsing them into one parameterized surface. The
architecture review counted eighteen `validate_async_*` methods and proposed folding them to a few;
on inspection the count is thin aliases over genuinely distinct operations, not accidental
duplication.

The typed field path and the identity path cannot be unified by erasure. The typed path snapshots the
typed field **value** and hands it to the validator (`FnOnce(Value, AsyncValidatorContext) -> Fut`,
reading `run.field_value()`); the identity and form paths have no `Value` type and pass only the
context (`FnOnce(AsyncValidatorContext) -> Fut`). Erasing field into field-identity would drop the
value from the validator contract, so the two are different operations that happen to share shape.

The debounce axis is likewise not worth folding into an `Option<Delay>` parameter. The plain path
begins validation immediately; the debounce path carries the whole `DebounceWake` state machine
(schedule, register the delay, then begin-or-flush on wake inside the spawned task). Merging them
behind `if let Some(delay)` produces one branchy method that is less clear than the two focused ones.

Of the eighteen methods, twelve are one-to-three-line aliases (`_after_sync` and the plain entries)
over six real bodies parameterized by a `sync_already_ran` flag, and every one is called only by the
three internal start matrices in `register_runtime_async_*`. Threading the flag through to delete the
aliases is possible and safe (the async bodies would not change), but it buys only wrapper deletion
with no gain in depth, so it is deliberately not pursued. The caller-facing async surface is already
small (four `pub` methods); this width is internal and distinct from the per-validator execution
coupling recorded in [ADR-0011](0011-do-not-extract-a-chain-executor-module.md). Future reviews should
not re-propose erasing field into field-identity or folding the debounce path.
