# Resolve collection diagnostic targets against current identities

An external validation library addresses collection rows by index, while Dioform stores errors against
the **Collection Item Identity** of one logical row. A fixed external-path-to-identity map becomes wrong
after insertion, removal, or reordering: it can miss a new row, target the wrong surviving row, or attach
an error to a retired identity. Dioform will instead register a durable typed **Collection Validation
Target Rule** and resolve each emitted external row index against the identity order paired with the exact
**Form Draft** being validated.

## The rule is typed and the matcher is adapter-owned

**Form Core** owns the typed rule, its association with the registered form validator, preparation of the
collection identity order, and resolution of one current row index to a `ValidationTarget`. A rule may
target either the collection item value or one static descendant. The core does not receive an **External
Diagnostic Path** and does not parse `garde` or `validator` syntax.

Each **Validation Adapter** owns how an external path matches a rule. The `validator` adapter retains the
structural index exposed by `ValidationErrorsKind::List`. The `garde` adapter uses a structured matcher
with exactly one row-index position, rebuilds a candidate with public `garde::Path` constructors, and
checks path equality. Dioform does not add a shared wildcard-string language, parse `garde::Path` through
its hidden iterator, or accept an opaque matcher that prevents overlap checks.

The exact `PathMap` remains the source-compatible surface for structurally static targets. Collection
rules are configured separately on each adapter builder, while the typed core rule and shared routing
outcome remain public for third-party adapters. This extends the data-and-registration seam from
[ADR-0012](0012-use-a-shared-validation-adapter-support-crate.md) and
[ADR-0018](0018-decline-public-validation-adapter-trait.md) without introducing a public adapter trait.

## Routing is classified before it becomes a target

For one external diagnostic, an eligible exact mapping to a structurally static field wins. An exact
mapping that captures a **Collection Item Identity** is ineligible and never attaches an error to that
captured identity. Otherwise, exactly one matching collection rule resolves its row index live. Multiple
matching collection rules or a matched rule whose row has no current identity are a **Collection
Validation Target Resolution Failure**. No eligible exact mapping and no collection-rule match is an
**Unmapped Diagnostic**.

Every fallback preserves the diagnostic at form scope. Dioform never drops it, guesses correspondence,
panics during validation, or attaches it to an old identity. The shared adapter support exposes ephemeral
**Diagnostic Route Provenance** to the mapper. `on_unmapped_path` remains specific to a true routing miss,
while an optional structured reporter observes collection target resolution failures. Applications may
copy provenance into their own **Validation Error**, but neither `ValidationTarget` nor stored core error
state gains adapter metadata. This corrects ADR-0037's narrower assumption that every adapter-selected
form target necessarily denotes an unmapped path while preserving its decision not to store provenance
in the core.

Adapter builders also expose all statically detectable routing-configuration issues without making
registration fallible. Runtime-only failures still use the classified fail-closed path. A manually
constructed `FieldPath::direct` remains a semantic trust boundary: Dioform can reject a structurally
dynamic exact identity, but cannot prove that application-supplied accessors truthfully describe a
structurally static identity.

## Validation uses one paired draft and identity order

A registered rule makes its collection identity state a durable validator dependency. The state is
prepared before the validation run and before a **Submit Validation Token** is captured, then read without
mutation while diagnostics are mapped. Coordinated insert, append, remove, move, swap, item replacement,
clear, reset, containing-field reset, reinitialization, and full `FormStateSnapshot` restoration that
satisfies every registered rule's cardinality checks all produce the identity order for the corresponding
draft transition. Item replacement preserves the identity of that logical item; reinitialization mints
fresh identities; reset restores baseline identities.

A **Collection-Affecting Field Replacement** replaces every current logical item in each tracked
collection it reaches. Dioform clears state belonging to the displaced current identities and mints a
fresh current sequence from the never-rewinding counter, without positional or value matching. Baseline
identities remain reserved for reset. Exact collection replacement and replacement of a containing field
follow the same rule; the explicit collection-item replacement operation remains identity-preserving.
Live collection diagnostic routing cannot ship before this reconciliation exists, because an equal-length
generic replacement otherwise leaves old identities apparently resolvable over new rows.

Collection identity preparation and validator-rule registration are validation-proof inputs. The
dedicated submit-validation generation from [ADR-0041](0041-track-submit-validation-currency-with-a-dedicated-generation.md)
should land first so those transitions retire held proof through one established mechanism, although it
is not required to compute a synchronous target correctly.

## Boundaries

This decision covers the synchronous first-party adapters and collection shapes the current identity
model can represent: direct or named-struct-composed collections with an item-value or static-descendant
target. Collections nested inside collection items remain deferred. Persisted collection identities are
adopted only through full `FormStateSnapshot` restoration, where the **Form Draft** and identity state
move together. Identity-only restoration is not a supported lifecycle operation: cardinality can
prove shape but not that equal-length rows retain the same logical correspondence. A future partial
collection-state restoration interface would require a separate decision and must carry collection
values and identities together. Future async collection targeting must capture the identity sequences
atomically with its owned **Form Snapshot** rather than resolve a later live order. Item-root binding
selectors are a separate presentation convenience; their absence does not change where a row-level
diagnostic belongs.

## Rejected alternatives

A shared mutable `PathMap` was rejected because application-controlled routing can change outside form
lifecycle coordination and submit-proof currency. Re-registering a validator destroys its results and
does not revalidate. Static literal row entries were rejected because they cache the exact index-to-
identity relationship this decision must keep live. Collection-first routing was rejected because it
silently defeats durable exact overrides; blanket overlap rejection was rejected because future indices
can create overlaps that did not exist at registration. Documentation-only treatment was rejected
because the current behavior creates unreachable, submission-blocking field errors rather than merely
offering limited presentation. Checked identity-only restoration was rejected because structural and
cardinality validation cannot prove logical correspondence: adopting an equal-length sequence over
different rows would make retained bindings address different logical items. Fully paired partial
collection-state restoration remains a possible future enhancement only if a concrete use case justifies
a second restoration surface beside full `FormStateSnapshot` restoration.
