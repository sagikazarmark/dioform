# dioform-validator

[![crates.io](https://img.shields.io/crates/v/dioform-validator?style=flat-square)](https://crates.io/crates/dioform-validator)
[![docs.rs](https://img.shields.io/docsrs/dioform-validator?style=flat-square)](https://docs.rs/dioform-validator)

**Renderer-agnostic [`validator`](https://crates.io/crates/validator) validation adapter for [Dioform Core](https://crates.io/crates/dioform-core).**

This is an opt-in validation adapter: it depends on `dioform-core` and
`validator`, but not on the Dioxus facade crate. The adapter flattens nested
`validator` diagnostics into Dioform validation errors, mapped into the
application's shared validation error type.

See [`docs/validation-adapters.md`](https://github.com/sagikazarmark/dioform/blob/main/docs/validation-adapters.md)
in the workspace for usage patterns and dependency guidance.

## Collection rows

Use a `ValidatorCollectionTargetRule` for diagnostics below
`ValidationErrorsKind::List`. The adapter keeps the list index structural and
resolves it to the current Dioform Collection Item Identity on every validation
run:

```rust
use dioform_validator::{
    ValidatorCollectionPath, ValidatorCollectionTargetRule,
    ValidatorValidationExt,
};

let quantity_rule = ValidatorCollectionTargetRule::descendant(
    // Named validator fields before and after exactly one structural list index.
    ValidatorCollectionPath::new(["lines"], ["quantity"]),
    lines_path(),
    line_quantity_path(),
)?;

form.validator_validation()
    .collection_target_rule(quantity_rule)
    .register_string_errors();
# Ok::<(), dioform_core::CollectionValidationTargetRuleError>(())
```

For a collection inside named structs, include every external field component,
for example `ValidatorCollectionPath::new(["invoice", "lines"],
["details", "quantity"])`. This is not a wildcard string: a literal field key
such as `"lines[0]"` remains a named field component and does not match the
structural list index.

The API can also represent a rule targeting the row value itself:

```rust
let row_rule = ValidatorCollectionTargetRule::item(
    ValidatorCollectionPath::new(["lines"], ["quantity"]),
    lines_path(),
)?;
# Ok::<(), dioform_core::CollectionValidationTargetRuleError>(())
```

`validator::ValidationErrors` places terminal errors under a named row field.
The matcher therefore still names that external field, while the item-value
constructor attaches the resulting diagnostic to the logical row value.

Rules are durable dependencies of the registered form validator. Append,
insert, remove, move, swap, clear, explicit item replacement, collection or
containing-field replacement, reset, reinitialization, and valid state restore
therefore resolve against the identity order paired with the draft being
validated. Do not put collection-row targets in `ValidatorPathMap`: an exact
target that captures one Collection Item Identity is deliberately ineligible.

## Routing and reporting

Routing uses this order:

1. An eligible structurally static exact `ValidatorPathMap` entry wins.
2. A captured-item exact entry is ignored as unsafe.
3. Exactly one matching collection rule resolves the emitted row index live.
4. Ambiguous rules or one unresolved target preserve the diagnostic on the form.
5. A true miss also preserves the diagnostic on the form.

A custom mapper can inspect `ValidatorDiagnostic::route_provenance()` to retain
the ephemeral route classification in an application error. Reporting is
optional and has no routing side effects:

```rust
let builder = form
    .validator_validation()
    .path_map(path_map)
    .collection_target_rule(quantity_rule)
    .on_unmapped_path(|path| eprintln!("unmapped validator path: {path}"))
    .on_collection_resolution_failure(|path, failure| {
        eprintln!("could not resolve {path}: {failure:?}");
    });

for issue in builder.configuration_issues() {
    eprintln!("validator adapter configuration: {issue:?}");
}

builder.register(map_diagnostic);
```

`configuration_issues()` aggregates ineligible exact `PathMap` targets and
duplicate structural collection matchers. Registration stays infallible;
runtime ambiguity and unresolved targets fail closed to the form. Neither reporter
requires `Send`, and each runs once per applicable diagnostic.

Collection rules support one structural list index and a static item descendant.
Collections nested inside collection items are not representable by the current
core identity model. `FieldPath::direct` is a semantic trust boundary: Dioform
checks that its identity is structurally eligible but cannot prove that manually
supplied accessors describe that identity truthfully.

## Install

```toml
[dependencies]
dioform-validator = "0.1.1"
validator = { version = "0.20", features = ["derive"] }
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
