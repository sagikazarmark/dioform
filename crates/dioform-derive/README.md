# dioform-derive

Derive macros for [Dioform](https://github.com/sagikazarmark/dioform)
typed field paths.

This crate provides `#[derive(Form)]` and `#[derive(FieldGroup)]` for non-generic
named form structs. `#[derive(Form)]` generates typed field accessors where each
`FieldPath` keeps Rust-based **field identity** separate from the rendered HTML
**field name**. `#[derive(FieldGroup)]` generates a typed field-group map for
reusable groups of fields that can be mounted under a nested path or explicitly
mapped into a differently shaped form.

These macros are re-exported by the [`dioform`](https://crates.io/crates/dioform)
facade: depend on that crate rather than this one directly.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
