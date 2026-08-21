# dioform-garde

[![crates.io](https://img.shields.io/crates/v/dioform-garde?style=flat-square)](https://crates.io/crates/dioform-garde)
[![docs.rs](https://img.shields.io/docsrs/dioform-garde?style=flat-square)](https://docs.rs/dioform-garde)

**Renderer-agnostic [`garde`](https://crates.io/crates/garde) validation adapter for [Dioform Core](https://crates.io/crates/dioform-core).**

This is an opt-in validation adapter: it depends on `dioform-core` and `garde`,
but not on the Dioxus facade crate. The adapter registers one synchronous
form-level validator and maps every `garde::Report` diagnostic into the
application's shared validation error type.

Simple forms whose validation error type is `String` can use
`GardeValidationBuilder::register_string_errors`. Richer applications should
provide an explicit mapper that preserves the external `garde` path, message, and
selected Dioform target in their own enum or struct error type. Context-aware
validation translates Dioform's `FormValidatorContext` into the external
`garde::Validate::Context` value.

## Collection Rows

Configure collection diagnostics with a structured matcher and typed collection
paths instead of exact entries such as `lines[0].description`:

```rust
use dioform_garde::{GardeCollectionRowMatcher, GardeValidationExt};

form.garde_validation()
    .collection_row_descendant(
        GardeCollectionRowMatcher::new(["lines"], ["description"]),
        lines_path(),
        line_description_path(),
    )
    .expect("the collection and descendant paths must be structurally static")
    .register_string_errors();
```

The matcher inserts exactly one numeric row component between its named
components. It reconstructs a candidate with public `garde::Path` constructors
and compares paths structurally. A numeric index is therefore different from a
string key such as `"0"` or `"[0]"`; there is no wildcard string grammar.

For a diagnostic attached to the item value itself, leave the named suffix
empty and use `collection_row_item`:

```rust
form.garde_validation()
    .collection_row_item(
        GardeCollectionRowMatcher::new(
            ["tags"],
            std::iter::empty::<&str>(),
        ),
        tags_path(),
    )
    .expect("the collection path must be structurally static")
    .register_string_errors();
```

The adapter registers durable `CollectionValidationTargetRule`s with Form Core.
Each validation run resolves Garde's current row index against the current
Collection Item Identity order. Append, insert, remove, move, swap, clear,
item replacement, reset, reinitialization, and collection-affecting replacement
therefore use the identities paired with the draft being validated.

Eligible static `GardePathMap` entries take precedence over collection rules.
An exact entry that captures a Collection Item Identity is ineligible and never
routes to that captured identity. Use `configuration_issues()` before a terminal
registration call to inspect those entries together with duplicate collection
matchers; registration itself remains infallible.

Custom mappers can inspect `GardeDiagnostic::route_provenance()`. True misses
invoke only `on_unmapped_path`. Ambiguous matching rules or a matched row with no
current identity fail closed to the form and optionally invoke
`on_collection_resolution_failure`; both reporters run once per diagnostic in
Garde report order and do not require `Send`.

Current collection rules are synchronous and support direct or named-struct-
composed collections with item-value or static-descendant targets. Collections
nested inside collection items are not supported. `FieldPath::direct` is a
semantic trust boundary: Dioform can reject a captured identity, but cannot
prove that manually supplied accessors agree with their static identity.

See [`docs/validation-adapters.md`](https://github.com/sagikazarmark/dioform/blob/main/docs/validation-adapters.md)
in the workspace for usage patterns and dependency guidance.

## Install

```toml
[dependencies]
dioform-garde = "0.1.1"
garde = { version = "0.23", features = ["derive"] }
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
