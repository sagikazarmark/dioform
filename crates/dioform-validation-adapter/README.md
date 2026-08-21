# dioform-validation-adapter

Shared support for renderer-agnostic [Dioform](https://github.com/sagikazarmark/dioform)
validation adapters.

A **validation adapter** maps an external validation library's diagnostics into a
form's shared **validation error** type. Every adapter needs the same routing
plumbing: an exact map from an external diagnostic path to a typed validation
target, adapter-neutral classification of exact and live collection candidates,
and a borrowed diagnostic view carrying ephemeral route provenance. This crate
owns those pieces so each first-party adapter
([`dioform-garde`](https://crates.io/crates/dioform-garde),
[`dioform-validator`](https://crates.io/crates/dioform-validator), and any
future adapter) does not re-derive them.

This is an infrastructure crate for building adapters. Application code depends on
a concrete adapter instead.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
