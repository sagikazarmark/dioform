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

## Install

```toml
[dependencies]
dioform-validator = "0.1.1"
validator = { version = "0.20", features = ["derive"] }
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
