# dioform-fullstack

[![crates.io](https://img.shields.io/crates/v/dioform-fullstack?style=flat-square)](https://crates.io/crates/dioform-fullstack)
[![docs.rs](https://img.shields.io/docsrs/dioform-fullstack?style=flat-square)](https://docs.rs/dioform-fullstack)

**Dioxus Fullstack submit adapters for [Dioform](https://github.com/sagikazarmark/dioform).**

This adapter keeps Dioform's submission lifecycle local to the form. Server
functions return application-defined transport payloads, and callers map those
payloads into structured `SubmitErrors`. Transport failures stay on a separate
explicit mapping path, so network, serialization, or server-function invocation
failures do not become submit errors by accident.

Depends on the [`dioform`](https://crates.io/crates/dioform) facade and
Dioxus Fullstack.

## Install

```toml
[dependencies]
dioform = "0.1.1"
dioform-fullstack = "0.1.1"
```

## Feature Flags

- `server`: enables the server-side half (pulls in `dioxus-server` and `dioxus-fullstack/server`); enable it from the server build only.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
