# Expose exact item-root validation reads on collection item bindings

A `CollectionItemBinding` represents one logical collection item's whole-value **Field** while also
creating bindings for its descendants. Form Core can store an error against that item value, but the
ordinary row binding cannot read it except through a whole-form aggregate. Dioform will expose the
narrow validation presentation surface needed to render that correctly targeted error.

## The public surface is validation-specific

The binding exposes its item-root **Field Identity**, all stored validation errors attached there, all
currently visible validation errors attached there, and visible validation errors filtered for one
**Submit Intent**. Intent filtering is part of the minimum surface: an intentful form must not present a
Publish error as a Save Draft error merely because both target the same item value.

The binding does not gain a whole-item value read, rendered **Field Name**, accessibility helper,
metadata or dirty-state selectors, `on_blur`, setters, or event handlers. A collection row normally
renders several descendant controls and is not itself evidence that one control representing the whole
item was rendered, focused, blurred, or edited. Those broader reads require their own use cases and, for
item value and dirty state, correct identity-preserving replacement notifications.

## Item-root error reach is exact

The error selectors return errors attached exactly to the item's whole-value **Field Identity**. They do
not aggregate errors attached to descendant Fields. This matches every other field-scoped selector and
keeps row summaries an application presentation decision rather than creating a second aggregate-error
API.

Outward **Error Visibility** from ADR-0032 still applies: a descendant blur or touch may reveal an error
attached to the item root without changing the root's exact metadata. The Dioxus Adapter must therefore
wake an already-registered item-root visible-error selector when descendant metadata changes can alter
that selector's answer, including when the current **Validation Mode** does not run validation for the
interaction.

Error reads register lazily against the exact item-root identity and their existing selector kinds, in
line with ADR-0029. They do not register against the containing Collection Field or every descendant.
That exact registration topology does not promise exact notification fan-out: existing validation
transitions may still wake validation selectors form-wide. Reorder preserves the identity and its
errors; removal wakes retained item-root error readers and then returns empty results for the retired
identity.

## Rejected alternatives

Full read parity with `MultiSelectItem` was rejected because symmetry alone does not justify permanent
whole-item value, state, naming, or accessibility APIs. `MultiSelectItem` may itself represent a rendered
option control, while an ordinary collection row makes no such assertion. An accessibility helper was
rejected with that broader surface because its input-oriented contract would contradict the absence of a
whole-item control. Full interactive parity was rejected for the same reason. Aggregating descendant
errors was rejected because it would give `validation_errors()` different reach depending on which
binding type the application called it on.

The collection-field and validation-adapter documentation will connect item-value diagnostic routing to
the item-root validation reads. Focused tests carry the executable example; the existing
collection-validation demo remains about item-child validators rather than coupling two separate
concepts into one example.
