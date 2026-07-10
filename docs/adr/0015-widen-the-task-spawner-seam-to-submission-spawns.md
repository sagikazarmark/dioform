# Widen the task-spawner seam to submission spawns

Dioform will widen the `TaskSpawner` seam to carry **Submission** task spawning in addition to **Async
Validation**. This amends [ADR-0009](0009-add-task-spawner-seam-for-async-validation.md), which scoped the
seam to validation task spawning and left the heaviest async orchestration (the managed-submission wait loop
and the application submit future) on hard-wired `dioxus_core::spawn`, reachable in tests only through a
`VirtualDom`.

The seam gains a `spawn_detached(future)` method for fire-and-forget tasks that carry no `TaskId` and are
never cancelled. Managed-submission orchestration and the submit future self-terminate through `is_active()`
and submit-generation guards rather than spawner-side cancellation, so forcing them through the cancellable
`spawn(TaskId, future)` path would mint task ids they never use. `spawn(TaskId, future)` and `cancel(TaskId)`
remain for **Async Validation** tasks, which are cancelled on newer interaction. Routing the three submission
spawns (the managed wait loop, the application submit future, and the unmanaged async submit) through the
seam lets the inline spawner exercise submission start and completion on a bare **Form Handle**, the same
payoff ADR-0009 established for validation.

The seam does not absorb the debounced-listener dispatch, which additionally hard-wires Dioxus scope capture
(`Runtime::current()` and `in_scope`); that is a separate concern and a separate seam. Widening the spawner
is also necessary but not sufficient for testing the managed wait loop: that loop awaits validation settling
repeatedly, so the inline spawner must additionally become a re-pollable step executor before the loop can be
driven to completion without a `VirtualDom`.
