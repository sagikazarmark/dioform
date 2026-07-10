# dioform

Headless typed form state and Dioxus bindings for statically typed Rust form models.

Dioform is a Rust-first **headless form library**. The primary API is a
compile-time **form model** with typed `FieldPath<Model, Value>` values; rendered
field names exist for HTML interoperability, not as the main addressing mechanism.
It is not a dynamic schema form builder or a styled component kit.

This crate is the Dioxus-facing facade. It provides `FormConfig`, form hooks,
explicit `FormHandle` APIs, headless input bindings, parse blockers, managed
submission, async and debounced validation, and an `advanced` module for
low-level core/runtime/serialization types. The shared validation error type
defaults to `String`; applications that need structured errors can choose their
own `Error` type through `FormHandle<Model, Error>` or `FormConfig<Model, Error>`.

```rust
use dioform::Form;

#[derive(Form)]
#[form(rename_all = "camelCase")]
struct ProfileForm {
    first_name: String,
    #[form(name = "family-name")]
    last_name: String,
}

let fields = ProfileForm::fields();
assert_eq!(fields.first_name().identity().as_str(), "first_name");
assert_eq!(fields.first_name().field_name(), "firstName");
assert_eq!(fields.last_name().field_name(), "family-name");
```

## Related crates

- [`dioform-core`](https://crates.io/crates/dioform-core): renderer-agnostic form state core.
- [`dioform-derive`](https://crates.io/crates/dioform-derive): `#[derive(Form)]` and `#[derive(FieldGroup)]`.
- [`dioform-garde`](https://crates.io/crates/dioform-garde) / [`dioform-validator`](https://crates.io/crates/dioform-validator): optional validation adapters.
- [`dioform-fullstack`](https://crates.io/crates/dioform-fullstack): Dioxus Fullstack submit adapters.

See the [workspace README](https://github.com/sagikazarmark/dioform)
for the full documentation, design terminology, and live examples.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
