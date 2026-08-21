# Use a shared validation-adapter support crate

Dioform extracts the structure shared by `dioform-garde` and `dioform-validator` into the
`dioform-validation-adapter` support crate. It owns `PathMap<Model>` for structurally static exact
**Explicit Path Mappings**, `DiagnosticView<'a, Path: ?Sized, Err: ?Sized>` for mapper calls, and the
adapter-neutral route outcome, provenance, resolution-failure, routing function, and configuration issue
types. Each adapter re-exposes the application-facing aliases
(`GardePathMap<Model> = PathMap<Model>`,
`GardeDiagnostic<'a> = DiagnosticView<'a, garde::Path, garde::Error>`, and the `validator` equivalents)
while retaining its external-library path grammar.

The field-vs-form attachment shared by both adapters uses
`FormValidationError::for_target(ValidationTarget, error)` in **Form Core**, because it only touches two
types the core already owns. The external-path half of the bridge does **not** live in the core: an
**External Diagnostic Path** is a **Validation Adapter** concern (see `CONTEXT.md`), separate from a typed
**Field Path** until an adapter maps it, so a `String`-keyed map of foreign library paths must not enter the
renderer-agnostic core. This extends [ADR-0003](0003-use-separate-validation-adapter-crates.md) with a shared
support layer rather than contradicting it; the layering test admits one new node.

The live collection extension in [ADR-0043](0043-resolve-collection-diagnostic-targets-against-current-identities.md)
widens this seam without moving external syntax into the core. **Form Core** owns
`CollectionValidationTargetRule<Model>`, `CollectionValidationTargetRule::item`,
`CollectionValidationTargetRule::descendant`, `CollectionValidationTargetRule::resolve`, and
`register_sync_form_validator_for_triggers_with_collection_target_rules` and
`register_sync_form_validator_with_collection_target_rules`. The support crate owns `ExactPathLookup`, `DiagnosticRoute`,
`DiagnosticRouteProvenance`, `CollectionValidationTargetResolutionFailure`, and `route_diagnostic` so
first- and third-party adapters share exact-static precedence, live-rule classification, and form-scoped
fallbacks. Each adapter still owns how its **External Diagnostic Path** structurally matches one rule.

Each adapter also keeps its library-specific diagnostic iteration (`garde`'s flat `report.iter()` versus
`validator`'s nested tree walk) and its own builder plus extension trait. The builders are not shared: the
`register` and `register_with_context` bounds are
library-specific (`garde::Validate<Context = ()>` versus
`validator::ValidateArgs<'args, Args = &'args Context>` with a higher-ranked lifetime), and a shared builder
would have to expose those bounds through a trait that leaks into each adapter's public `register` signature:
more indirection than the duplication it removes. Two adapters justify the seam; a third inherits the bridge.
