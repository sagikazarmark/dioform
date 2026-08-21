# Report Unmapped Diagnostics from the adapter, not from the Validation Target

A **Validation Adapter** gains an opt-in configuration step that reports the **External Diagnostic
Paths** it could not route, alongside `source`, `triggers`, and `path_map`. **Form Core** gains
nothing: `ValidationTarget::Form` stays a unit variant, so a stored **Validation Error** still cannot
say whether it is form-scoped because the diagnostic is genuinely whole-model or because it is an
**Unmapped Diagnostic**. The empty `PathMap` default is unchanged, and registration neither asserts
nor requires a non-empty map.

## Provenance does not belong on the Validation Target

`ValidationTarget` derives `PartialEq` and `Eq` unconditionally and is stored inside every
`FormValidationError`, which is serialized inside `FormStateSnapshot` behind
`FORM_STATE_SERIALIZATION_VERSION`. Any payload on the `Form` variant — even a bare marker — makes two
form-scoped targets that describe the same attachment point compare unequal, and rejects every snapshot
written by the previous version. Both costs are permanent and are paid by every form, including the
forms that register no **Validation Adapter** at all and every native validator that mints a
form-scoped target directly.

The variant that would carry an **External Diagnostic Path** is already declined:
[ADR-0012](0012-use-a-shared-validation-adapter-support-crate.md) records that a foreign library's
paths are a **Validation Adapter** concern and must not enter the renderer-agnostic core. What that
leaves genuinely open is a bare marker, and a marker buys nothing the adapter cannot record on its own
side of the seam.

The distinction is recoverable where it is actually decided, but a form target is not itself the
classification. At the time of this decision, an adapter selected a form target either for a genuinely
whole-model diagnostic or because `PathMap` missed, and the adapter knew which external path produced it.
Live collection rules later added a third form-scoped outcome: a **Collection Validation Target Resolution
Failure**. [ADR-0043](0043-resolve-collection-diagnostic-targets-against-current-identities.md) therefore
classifies the route before attachment as an exact static target, a live rule target, a collection
resolution failure, or an **Unmapped Diagnostic**. This corrects the narrower equation of “adapter-selected
form target” with “unmapped” without changing this ADR's decision: nothing needs to travel in
`ValidationTarget` or stored core state when the adapter can expose ephemeral **Diagnostic Route
Provenance** while mapping.

## A registration-time guardrail cannot see the case that bites

Asserting a non-empty `PathMap` at registration, or requiring `path_map` before `register`, addresses
only the map that maps nothing. A map that registers `email` on a model with a second validated field
routes that second field's diagnostic to the form by exactly the same fallback, and every documented
example maps fewer paths than its model can emit. A registration-time check reports the case a reader
is least likely to reach and stays silent on the case the documentation itself produces.

It also has no true predicate to check. `PathMap` registers the paths an application chose to address;
it does not declare the set an external library can emit, and neither `#[derive(Form)]` nor
`#[derive(FieldGroup)]` yields an enumerable field set to compare against. Because `insert_field` is the
only inserter, a form-scoped outcome cannot distinguish "deliberately whole-model" from "not yet
mapped" — `garde` emits genuinely whole-model diagnostics at the empty path, and `validator` keys
schema-level rules by an arbitrary string. A mandatory `path_map` would therefore force applications
whose diagnostics are all whole-model to register an empty map to say so, and would make the library's
own tests for **Explicit Path Mapping**'s fallback inexpressible without a second opt-out method.

Reporting at validation time has the predicate the guardrail lacks: the paths the library actually
emitted and the classified route selected for this run on this **Form Draft**. `on_unmapped_path` reports
only a true miss. `on_collection_resolution_failure` separately reports ambiguity or a matched rule whose
row is missing. Both outcomes preserve the diagnostic at form scope.

## Consequences

**The default stays silent unless an application asks.** This decision makes an **Unmapped Diagnostic**
observable; it does not make it loud. An application that configures no reporter still gets the
behaviour [ADR-0018](0018-decline-public-validation-adapter-trait.md) chose — the diagnostic attaches to
the form rather than being dropped or matched by name — and still gets a **Validation Error** that is
invisible before a submit attempt under
[ADR-0032](0032-widen-error-visibility-outward-from-the-field-an-error-is-attached-to.md) while blocking
**Submit Availability** under
[ADR-0019](0019-decline-can-submit-when-invalid-opt-out.md). What changes is that the composite is now
diagnosable on demand instead of only by reading stored errors and inferring.

**It reaches the applications that have no mapper.** The reporter is a configuration step, not a
mapper argument and not a change to a terminal method, so it is available to `register_string_errors`
and `register_string_errors_with_context`. Those take no mapper at all, and an application that reaches
for the string convenience is the one most likely to have skipped `path_map` — the mapper-side remedy
was unavailable exactly where it was needed.

**It is additive.** The reporting handle is owned by the adapter closure and never crosses into **Form
Core**, which registers whole-model validation through a validator bound that requires no `Send`. No
existing call site changes, no terminal method signature changes, and the **Facade Crate** is
untouched.

**Route provenance remains ephemeral.** `DiagnosticView::route_provenance()` is available only while an
application mapper is converting the external diagnostic. An application may copy it into its own
**Validation Error**, but the adapter reporters are optional and side-effect-free with respect to
validation and routing. Builder `configuration_issues()` separately aggregates statically detectable
ineligible captured-item exact targets and duplicate collection matchers; it neither validates nor makes
registration fallible.

**The gap this closes is developer diagnosability, not the rendered form.** Error visibility reads the
**Validation Target** alone, so an application that records every unmapped path still cannot render
those errors before a submit attempt except by widening **Error Visibility** for the whole form.
Anything that changes what the user sees is a change to ADR-0032's reveal, not to this seam.
