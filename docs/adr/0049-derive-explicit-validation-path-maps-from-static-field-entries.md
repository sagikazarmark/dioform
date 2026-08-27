# Derive explicit validation path maps from static field entries

Dioform will let `#[derive(Form)]` expose the non-skipped direct fields of a **Named Form Struct**
as an enumerable set of **Static Field Entries**. The validation-adapter support crate will turn that
set into a **Derived Path Map**, and the garde and validator adapters will expose it through their
builders. This removes the hand-written O(fields) happy-path map without weakening **Explicit Path
Mapping** or reversing the dependency direction established by
[ADR-0003](0003-use-separate-validation-adapter-crates.md).

## Enumerate the generated direct fields in Form Core

`dioform-core` will define `StaticFieldEntry<Model>` and `EnumerableStaticFields`. The entry pairs one
Rust field identifier with the erased `ValidationTarget` created from a model-typed `FieldPath`; its
`Model` parameter keeps construction from mixing a target from another form model. `#[derive(Form)]`
will implement the separate trait rather than adding a required method to `Form`, so existing manual
`Form` implementations remain source-compatible.

The derive will emit one entry for every accessor it generates, using the same `FieldPath::direct`
construction as that accessor. `#[form(skip)]` therefore excludes both the accessor and the entry.
`#[form(name = "...")]` and `#[form(rename_all = "...")]` affect only rendered **Field Names**; the
entry key is always the Rust identifier that also supplies the direct **Field Identity**. Private
non-skipped fields are included because they have generated accessors too.

This is a flat enumeration, not recursive schema discovery. V1's "flat static scalar fields" boundary
means flat, one-segment path coverage rather than classification of arbitrary Rust value types: the
derive promises one exact entry per direct accessor and deliberately avoids type-syntax heuristics.
A direct aggregate-valued field can therefore be targeted only as a whole; the derive does not produce
entries for its descendants or collection rows.

## Build and select a derived map explicitly

`dioform-validation-adapter` will add `PathMap::derived()` when `Model: EnumerableStaticFields`. It
constructs an ordinary exact-path `PathMap` whose external keys are the entries' Rust identifiers.
Those are the paths garde's derive emits for ordinary static fields and the keys validator uses for
ordinary field errors.

`GardeValidationBuilder` and `ValidatorValidationBuilder` will each add `derived_path_map()`, exactly
equivalent to `path_map(PathMap::derived())`. The common path becomes:

```rust
core
    .garde_validation()
    .derived_path_map()
    .register_string_errors();
```

`PathMap::derived()` is a starting value, not a closed schema. Applications can compose it with
`with_field(...)` before passing it to `path_map(...)`: a registration for an existing external key
overwrites that target, while a renamed or additional external key extends the map. Adding a renamed
key need not remove the unused identifier key because runtime routing remains exact.

## This remains Explicit Path Mapping

A **Derived Path Map** is populated before validation runs from compile-time-generated entries and then
uses the same exact runtime lookup as a hand-written map. No diagnostic is matched at validation time
by rendered **Field Name**, serde name, or Rust name, and an unregistered path still resolves to the
form. "Path inference" remains prohibited: it means deriving an attachment from an unregistered
runtime path, not generating explicit registrations from the same declaration that generates typed
field accessors.

For ordinary direct fields, the generated registration cannot omit a newly added non-skipped accessor
the way a hand-written map can. It does not claim that every path an external library can emit is in
the static set. Garde dives into non-`Form` values, custom validators that emit inner paths, collection
rows, and external-library path renames can all remain outside it.

This amends one premise of
[ADR-0037](0037-report-unmapped-diagnostics-from-the-adapter.md): `#[derive(Form)]` now has an
enumerable direct field set. ADR-0037's decision still stands because registration cannot know the
complete external diagnostic path set. `on_unmapped_path` remains the validation-time net for anything
outside the derived static skeleton, while collection resolution failures keep their separate report.

## Decline derive-driven adapter configuration

Dioform will not add `#[form(garde)]`, `#[form(validator)]`, or a similar derive flag. The derive creates
types and trait implementations, while adapter registration is a runtime act on one `FormCore`
instance with application-chosen validation errors, triggers, context, mapper, and reporting. A derive
flag could only generate another method that runtime code must still call.

It would also make `dioform-derive` emit paths naming an optional adapter crate. That reverses the
boundary in ADR-0003: validation adapters depend on **Form Core**, while the core, derive, and ordinary
**Facade Crate** API remain independent of external validation libraries.

## Defer feature-gated facade sugar

A feature-gated facade helper such as `use_garde_form(initial)` has a sound dependency arrow from
`dioform` to `dioform-garde` and could provide true one-line setup for `Error = String` forms. We defer
it rather than decline it. Derived maps must first ship, and real call sites must show that the residual
roughly three-line setup is both common and stable enough to justify widening the facade's feature
surface.

Revisit only when that evidence exists and the helper can choose triggers, mapping, reporting, and
error semantics without hiding material policy. Adopting it would consciously amend ADR-0003's current
decision to keep external-validation features off the facade; it is not part of the derived-map work.

## Deferred extensions

Nested **Field Group** recursion needs an explicit opt-in analogous to `#[garde(dive)]`; it must not be
assumed from a field's Rust type. Derived collection-row rules need a sound type-level design and will
not use proc-macro syntax heuristics to guess `Vec` or other collection types. External-library renames
remain explicit additions to the derived starting map. These extensions do not block the direct-field
implementation in [issue #95](https://github.com/sagikazarmark/dioform/issues/95).
