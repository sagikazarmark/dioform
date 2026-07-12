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

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
