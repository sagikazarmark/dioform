# dioform-derive

Derive macros for [Dioform](https://github.com/sagikazarmark/dioform)
typed field paths.

This crate provides `#[derive(Form)]` and `#[derive(FieldGroup)]` for non-generic
named form structs. `#[derive(Form)]` generates typed field accessors where each
`FieldPath` keeps Rust-based **field identity** separate from the rendered HTML
**field name**. `#[derive(FieldGroup)]` generates a typed field-group map for
reusable groups of fields that can be mounted under a nested path or explicitly
mapped into a differently shaped form.

For Dioxus applications, these macros are re-exported by the
[`dioform`](https://crates.io/crates/dioform) facade.

## Core-only Models

For server validation or another renderer, depend on core and the macros directly:

```toml
[dependencies]
dioform-core = "0.6"
dioform-derive = "0.6"
```

```rust
use dioform_core::Form;   // Trait providing fields().
use dioform_derive::Form; // Derive macro, in a separate namespace.

#[derive(Clone, Form)]
#[form(crate = "::dioform_core")]
struct SignupForm {
    email: String,
}

assert_eq!(SignupForm::fields().email().field_name(), "email");
```

The model-level `#[form(crate = "…")]` attribute selects the crate path used by
generated code. The default is `::dioform`, so the attribute is required when the
facade is absent. Both `Form` and `FieldGroup` support it, including paths to
renamed dependencies. It does not change field identity or rendered field names.
Models derived against core can also be used by the facade in client code.

See the [core server-validation walkthrough](https://github.com/sagikazarmark/dioform/blob/main/crates/dioform-core/README.md#validating-on-the-server-with-dioform-core)
for validation execution and owned diagnostics.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
