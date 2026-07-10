# Form State Serialization

Dioform normally relies on deterministic **Form Initialization** for SSR and hydration: server and client create the same form from the same configuration, and no runtime form state crosses the boundary.

Opt-in **Form State Serialization** is different. Applications that intentionally need selected state transfer can capture a `FormStateSnapshot` with `FormCore::state_snapshot()` or `FormHandle::state_snapshot()` and restore it with `restore_state_snapshot()`. This transfers value-bearing state explicitly instead of changing the default hydration behavior.

The current snapshot format is versioned with `FORM_STATE_SERIALIZATION_VERSION`. The embedded collection identity format is versioned separately with `COLLECTION_IDENTITY_SERIALIZATION_VERSION` so future collection identity changes can reject incompatible snapshots clearly. Treat snapshots as same-deployment transfer, not durable long-term persistence: restoring assumes a compatible crate version, compatible serialized model and error shapes, and the same normal validator and submit configuration.

Included in the current core snapshot:

- **Form Draft** baseline and current values.
- Field versions used for stale submit-error protection.
- Field metadata such as touched and blurred state.
- **Validation Mode** and **Error Visibility** policy.
- Stored non-submit validator result state for matching validators after the application registers its normal validators.
- Runtime **Collection Item Identity** state for tracked collection fields, including baseline and current item identity sequences plus the next identity counter.

Not included in the current core snapshot:

- Validator closures, submit handlers, observers, Dioxus tasks, debounced timers, or in-flight submissions.
- Pending async or debounced validation work; pending validator state is restored as unknown because no task or timer is restored with it.
- Submit-scoped validator result state, stored submit errors, the latest submit status, and raw **Submit Intent** values. Submit-scoped state is associated with an arbitrary application-defined **Submit Intent** type, so restoring it without that typed intent would make intent-scoped errors look global.
- Adapter-owned parse bindings and raw input state; restoring through the Dioxus adapter clears existing target parse state rather than transferring it. When an adapter parse-state transfer layer is added, collection item parse errors can use the same serialized **Field Identity** values restored by this snapshot.
- File selections, file-field metadata, and file-field validator result state. File selections are platform-owned adapter state; restoring touched or validated file-field state without the selected files would be misleading.
- Any automatic redaction or filtering of form values.

Stored validator result state is restored onto validators registered by the application's normal form configuration. Matching currently follows validator IDs allocated by registration order, so applications should register the same validators in the same order before restoring a snapshot. The target configuration keeps its source labels, trigger set, sync/async kind, and validator closures; snapshots do not promise migration across arbitrary validator configuration changes.

Because snapshots include the **Form Draft** and typed errors, they may contain passwords, tokens, raw user input, or other sensitive data. Serialization is intentionally explicit and opt-in. Applications must choose whether to serialize, encrypt, redact, or avoid transferring particular forms. Observer events remain value-redacted by default and are not serialized.

For collection fields, snapshotting differs from deterministic initialization in one important way: deterministic initialization can recreate the same vector values, but it cannot know that a current row was originally `item-2` after an insertion, removal, or reorder. `FormStateSnapshot` includes collection identity sequences so item-scoped metadata, non-submit validation errors, and future parse-state snapshots continue to refer to the same logical item after restore.
