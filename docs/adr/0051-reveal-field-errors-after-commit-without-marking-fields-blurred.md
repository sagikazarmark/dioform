# Reveal Field errors after Commit without marking Fields blurred

Dioform records exact, sticky `committed` metadata when a Dioform-produced `dioxus-field` binding
reports **Commit**. The default **Error Visibility** policy is `CommitOrSubmit`: an error attached to
a **Field** becomes visible after that **Field** or one it contains has been committed, or after a
submit attempt. The existing `BlurOrSubmit` policy remains available when an application explicitly
wants focus-exit-driven presentation.

Commit feeds the Commit **Validation Trigger**, but it does not mark the **Field** touched
or blurred and does not dispatch blur listeners. **Focus Exit** remains the only Field Convention
report that Dioform maps to exact touched and **Blurred Field** metadata plus blur listeners. A switch
can therefore validate and present an error when toggled while `is_field_blurred()` remains false.

The committed flag belongs in **Field Metadata**, rather than in adapter-only presentation state, so
core visible-error selectors, Field Meta, accessibility state, summaries, and non-Dioxus adapters all
apply one rule. Like touched and blurred metadata, it stays exact while the visibility predicate uses
the outward reach from [ADR-0032](0032-widen-error-visibility-outward-from-the-field-an-error-is-attached-to.md):
committing a leaf may reveal a container error without marking the container committed. Form-scoped
errors remain hidden until submit.

Changing the default to `TouchedOrSubmit` was declined because a user write marks a **Field** touched
before the widget reports that its interaction unit has ended. Inferring presentation from a stored
Commit-triggered result was also declined because the Commit trigger can run without a Field Convention
Commit and does not preserve which event made the result presentation-relevant.

Committed metadata clears on **Reset**, field reset, and **Reinitialization**, and participates in
opt-in **Form State Serialization**. The snapshot format advances to version 6. Because validators can
read `FieldMetadata`, the first Commit also retires existing **Submit Validation Tokens** before
Commit-triggered validation runs; repeated Commit reports leave the sticky metadata unchanged.

This amends [ADR-0050](0050-map-field-convention-focus-exit-without-validation.md) only for the
default-visibility and serialized-metadata statements. Its separation of Commit from Focus Exit and
blur listeners, while Commit continues to feed validation, is unchanged.
