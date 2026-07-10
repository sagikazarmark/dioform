# dioform-fullstack

Dioxus Fullstack submit adapters for [Dioform](https://github.com/sagikazarmark/dioform).

This adapter keeps Dioform's submission lifecycle local to the form. Server
functions return application-defined transport payloads, and callers map those
payloads into structured `SubmitErrors`. Transport failures stay on a separate
explicit mapping path, so network, serialization, or server-function invocation
failures do not become submit errors by accident.

Depends on the [`dioform`](https://crates.io/crates/dioform) facade and
Dioxus Fullstack.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
