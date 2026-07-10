# Use a shared validation-adapter support crate

Dioform will extract the structure duplicated across `dioform-garde` and `dioform-validator`
into a new `dioform-validation-adapter` support crate. Both adapters today copy, nearly verbatim, a
`PathMap<Model>` (a `BTreeMap<String, ValidationTarget>` keyed by an **External Diagnostic Path**, with a
typed `with_field` registration) and a borrowed diagnostic view exposed to the mapper closure. The shared
crate will own both as `PathMap<Model>` and `DiagnosticView<'a, Path: ?Sized, Err: ?Sized>`. Each adapter
re-exposes them under its existing names as type aliases (`GardePathMap<Model> = PathMap<Model>`,
`GardeDiagnostic<'a> = DiagnosticView<'a, garde::Path, garde::Error>`, and the `validator` equivalents), so
no application-facing API changes.

The field-vs-form routing that both adapters hand-roll collapses into a new
`FormValidationError::for_target(ValidationTarget, error)` constructor added to the **Form Core**, because it
only ever touches two types the core already owns. The rest of the bridge does **not** live in the core: an
**External Diagnostic Path** is a **Validation Adapter** concern (see `CONTEXT.md`), separate from a typed
**Field Path** until an adapter maps it, so a `String`-keyed map of foreign library paths must not enter the
renderer-agnostic core. This extends [ADR-0003](0003-use-separate-validation-adapter-crates.md) with a shared
support layer rather than contradicting it; the layering test admits one new node.

The shared crate holds only the two data types. Each adapter keeps its own library-specific diagnostic
iteration (garde's flat `report.iter()` versus `validator`'s nested tree walk) and its own builder plus
extension trait. The builders are not shared: the `register` and `register_with_context` bounds are
library-specific (`garde::Validate<Context = ()>` versus
`validator::ValidateArgs<'args, Args = &'args Context>` with a higher-ranked lifetime), and a shared builder
would have to expose those bounds through a trait that leaks into each adapter's public `register` signature:
more indirection than the duplication it removes. Two adapters justify the seam; a third inherits the bridge.
