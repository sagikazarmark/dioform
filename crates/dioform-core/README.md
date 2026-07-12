# dioform-core

[![crates.io](https://img.shields.io/crates/v/dioform-core?style=flat-square)](https://crates.io/crates/dioform-core)
[![docs.rs](https://img.shields.io/docsrs/dioform-core?style=flat-square)](https://docs.rs/dioform-core)

**Renderer-agnostic typed form state core for [Dioform](https://github.com/sagikazarmark/dioform).**

The core owns form drafts, typed field paths, validation state, submission state,
and reset and reinitialization semantics, plus value-redacted observer events,
without depending on Dioxus or a concrete async runtime.

Async and debounced validation cross the runtime boundary through explicit
work-token APIs. The core decides when a validator is pending, skipped, stale,
valid, or invalid; adapters execute the returned work from owned `FormSnapshot`
values and complete it back into the core.

Most applications should depend on the [`dioform`](https://crates.io/crates/dioform)
facade instead of using this crate directly. Use `dioform-core` when building
a renderer other than Dioxus, or a validation adapter.

## Install

```toml
[dependencies]
dioform-core = "0.1.1"
```

## Feature Flags

- `serde`: enables serialization support for form-state snapshots.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
