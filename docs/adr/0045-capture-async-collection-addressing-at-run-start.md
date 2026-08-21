# Capture async collection addressing at run start

An async form validator may emit collection-row diagnostics after the live collection has changed. Resolving those diagnostics against a later live identity order could attach an old diagnostic to a different logical row. Dioform will therefore make collection-aware async targeting an opt-in **Form Core** capability and pair the model being validated with the collection identity sequences needed by that validator.

## Capture only registered addressing dependencies

A rules-aware async form validator registers a finite set of typed **Collection Validation Target Rules**. Registration prepares those collections before validation and before a **Submit Validation Token** can rely on them. Rules-free validators retain their existing behavior and incur no collection-addressing work.

When an immediate run starts, a debounce delay expires, or a pending debounce is flushed for submit, one exclusive core operation captures the model-only **Form Snapshot** and the current identity sequence for each distinct collection referenced by that validator's rules. Multiple rules for one collection share one sequence, including an explicitly captured empty sequence. Baseline identities, allocation counters, metadata, errors, and unrelated collections are excluded. Scheduling and cancellation before actual run start capture nothing.

This private pairing is the **Async Validation Addressing Snapshot**. Run-start addressing capture is read-only: it does not prepare collections, mint identities, reconcile collection state, or advance submit-validation currency. The run never resolves against later live identities. Normal run and form-version checks still reject stale completion, while external effects already performed by validators, mappers, or reporters are not rolled back.

## Authorize registered nominal rule shapes

Async resolution is authorized by the nominal shape registered with the validator: one static collection **Field Identity** plus either the item root or one static descendant identity. An independently reconstructed equivalent rule is authorized; another descendant of the same captured collection is not. Resolution uses registration-owned captured data and never executes the query rule's accessor after the async boundary.

Each registered rule instance checks its model cardinality against the shared captured sequence. If instances with the same nominal shape disagree, that shape cannot resolve. Equal nominal shapes backed by dishonest accessors but equal cardinality remain indistinguishable under the existing `FieldPath::direct` semantic trust boundary; this decision does not introduce path equality or an opaque registration handle.

Resolution remains a narrow optional-target operation. A target is returned only when the shape is authorized, its captured addressing state is coherent, and the requested row exists. Otherwise resolution fails closed and the diagnostic remains at form scope. Because that failure is not necessarily proof that a row is absent, shared adapter provenance calls one matched but unresolved candidate `UnresolvedTarget`; multiple matching rules remain `AmbiguousMatchingRules`, and no matching rule remains an **Unmapped Diagnostic**. A detailed public failure taxonomy is rejected because every cause has the same routing consequence and applications have no distinct recovery action.

## Boundaries

The **Form Snapshot** remains model-only, and the raw addressing payload is neither public nor serialized. The Dioxus adapter transports registered rules through both configured and live async form-validator builders but owns no addressing logic. External diagnostic path grammar, matching, route provenance, mapping, and reporting remain adapter concerns under ADR-0012, ADR-0018, and ADR-0037.

This decision does not add async wrappers to the synchronous `garde` or `validator` integrations, suppress external side effects from stale runs, support collections nested inside collection items, expose raw collection identity sequences, or move adapter callbacks into a core freshness transaction.
