# Create selector registrations only from reactive reads

[ADR-0020](0020-derive-field-ancestry-from-identity-paths.md) closes by deferring two things to a
follow-up: replacing the per-write scan of `FormReactivity::fields` with range queries, and the fact
that the map "is only ever inserted into, never removed". It names the growth as the reason the scan
degrades, and warns that computing ancestors by splitting a written path "would create entries for
untracked identities, accelerating the unbounded growth of that map".

The warning is aimed at a fix that was never written. The growth it describes is already happening,
from two ordinary paths, and the larger of them is the notification path itself. Dioform will decide
*when a selector registration comes into existence* rather than *when one is destroyed*: an entry
exists because a **Form Selector** was read inside a reactive scope, and for no other reason.

## Notification creates most of the entries, and creating them buys nothing

`FormReactivity::field` resolves an entry with `entry().or_insert_with()`, and `notify_selector`
resolves through it on every field-scoped notification. So writing a field nothing has ever read
creates a permanent entry holding five empty subscriber lists. On a probe form driven through
binding edits, a blur, a numeric parse, a collection append, per-item edits, `validate_all`,
`submit`, and `reset`, five of the seven resulting entries were created this way, and every entry
created by the write-only drivers was.

The insert cannot be load-bearing, because the only consumer of `tracked_field_identities()` is
`notify_selector_transition`. Across every arm that reads the tracked list — `UnknownMutation`,
`ValidationChanged`, `SubmitAttempted`, the composite legs, and the ancestry expansion — an absent
identity and a present one with zero subscribers are indistinguishable, since notifying an entry
nobody subscribes to wakes nobody. The ancestry expansion is the case worth stating explicitly: it
filters *tracked* identities against the *written* set, and the written set arrives on the transition,
never from the map. A blind write to `lines[0].description` followed by a reader mounting and a write
to the containing `lines` reaches that reader identically with and without the insert.

What the insert does instead is convert every write into permanent map growth, on the same map whose
size sets the cost of the fan-out.

## Reads outside a reactive scope create entries that can never have a subscriber

`ReactiveSubscribers::track_read` subscribes only when `ReactiveContext::current()` is `Some`, but the
five `track_field_*` helpers call `field` unconditionally first. A read with no current context
therefore registers an entry and subscribes nothing to it, permanently.

This channel is much smaller than the notification one and worth fixing for its own sake rather than
for its size. It is also not where it was assumed to be: **Validator Context** holds an owned **Form
Snapshot** and lives in **Form Core**, which has no reactivity at all, so validators — sync or async —
never touch this map. The reads that leak are binding accessors and **Form Listener** callbacks
invoked outside a render, where being inside the Dioxus runtime is not the same as being inside a
`ReactiveContext`.

## The invariant, and what it pins

Both halves rest on one invariant: **a subscriber exists only if its entry exists**. Subscribers are
added exclusively by `ReactiveSubscribers::track_read`, reachable for a field only through
`FormReactivity::field`, which inserts. A notification that finds no entry therefore provably had no
subscriber to miss, and a reader arriving afterwards subscribes to a freshly created entry and reads
current state.

This pins the read-side gate to exactly `ReactiveContext::current().is_some()` — the same predicate
`track_read` uses to decide whether to subscribe. A gate keyed on anything else, such as an identity
filter or a notion of "is mounted", would let a subscriber exist without an entry and break the
non-inserting notification path. The two changes are independent to land and must not drift apart in
what they test.

## This bounds the write path, not the map

Entries created by genuine reactive reads still live for the form's lifetime, and one **Collection
Item Identity** per item ever rendered still accumulates, because identities never rewind
([ADR-0025](0025-mint-collection-item-identities-from-a-never-rewinding-counter.md)). This decision
closes the growth that writes cause; it does not prune. That is deliberate — the pruning designs
available today are each worse than the growth they remove:

**Pruning at collection-item removal**, mirroring `clear_collection_item_state`, is unsafe. The
removal sites prune before they notify, so readers are unlinked without their last wake. A
`ReactiveContext` records the specific `Subscribers` `Arc` it joined, so a re-created entry carries a
different list and the orphaned reader never wakes again. `restore_baseline_items` copies the baseline
back on **Reset**, so a removed baseline row returns — permanently unreactive. And
`ParseBindingRegistrationInner::drop` fires at row unmount, after the prune, re-inserting what was
just removed. The population this would orphan is precisely the one
[ADR-0022](0022-represent-an-absent-binding-target-in-the-return-type.md) exists to protect: code
holding a binding across a mutation, in an event handler or a spawned future.

**A post-notify emptiness sweep** is safe — `Drop for Inner`, `clear_subscribers`, and `run_in` keep
list membership and a context's recorded set in lockstep, so an empty entry is provably unreachable —
but it reclaims nothing where it runs. Scope teardown drops hooks before it drops the context, so at
every notification a dying row is still subscribed; entries go empty only after the flush, when no
notification follows. It also costs several times the per-transition key clone it was compared
against, and `ParseBindingRegistrationInner::drop` notifying once per unmounted row makes clearing a
collection run one sweep per row over the whole map. If it is ever adopted it belongs behind a growth
guard rather than on every transition, and its emptiness test must fail closed: `Subscribers::visit`
invokes its closure zero times on a poisoned lock, which is indistinguishable from an empty list and
would evict a live entry.

**Binding-lifecycle bookkeeping** cannot serve as the signal. `mounted_field_bindings` is fed only by
`use_field_binding_hook` and `use_parsed_text_with`; every collection-item hook bypasses it, and
`FieldIdentityKind::CollectionItem` entries are exactly the ones that accumulate. It also counts
bindings rather than readers, so evicting on binding unmount would drop entries that plain
`field_value` readers and error-summary components still hold.

## Range queries stay declined

ADR-0020 anticipated replacing the per-write scan with `BTreeMap` range queries, on the ground that
`Ord` is derived and descendants of `p` are the range from `p` plus the **Identity Path Separator**.
The premise is sound — related sets are contiguous in every case, including the collection ones — but
the direction is declined rather than deferred again.

The scan is not the cost. The `ValidationChanged` leg inside `composite_notifications` is fed through
`extend_unique`, a linear `contains` per item over a list that grows with the tracked count, which is
quadratic and dominates a collection-row write by a wide margin. Replacing that dedup with an
order-preserving membership index leaves the emitted `Vec` byte-identical, so the existing
exact-equality tests verify it, and removes the cost that range queries were reached for.

What range queries would add is a second contract on top of the **Identity Path Separator**: that the
derived `Ord` keeps every related set contiguous. That couples notification correctness to the
declaration order of `FieldIdentityKind`'s variants and the field order inside `CollectionItem`, where
reordering — an ordinary refactor — breaks notification with no compile error and no failing predicate
test. It also needs bounds that are not expressible as identities: the upper bound of one item's block
requires arithmetic on an opaque **Collection Item Identity**, and `FieldIdentity::new` rejects the
malformed lower bound by `debug_assert`. ADR-0020 kept **Field Ancestry** a bare predicate so its
representation would stay swappable; a bounds companion narrows that guarantee instead of preserving
it, and buys a share of a cost that is no longer there.
