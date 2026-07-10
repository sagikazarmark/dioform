# Dioform

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/sagikazarmark/dioform/dagger.yaml?style=flat-square)](https://github.com/sagikazarmark/dioform/actions/workflows/dagger.yaml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/sagikazarmark/dioform/badge?style=flat-square)](https://securityscorecards.dev/viewer/?uri=github.com/sagikazarmark/dioform)
[![Crates.io](https://img.shields.io/crates/v/dioform?style=flat-square)](https://crates.io/crates/dioform)
[![docs.rs](https://img.shields.io/docsrs/dioform?style=flat-square)](https://docs.rs/dioform)

Headless form state and Dioxus bindings for statically typed Rust form models.

Dioform is a Rust-first **Headless Form Library**. It is not a dynamic schema
form builder, styled component kit, or clone of every JavaScript form-library
feature. The primary API is a compile-time **Form Model** with typed
`FieldPath<Model, Value>` values; rendered field names exist for HTML
interoperability, not as the main addressing mechanism.

The workspace is split into:

- `dioform-core`: renderer-agnostic form draft, field path, validation, submission, reset, reinitialization, async validation, debounce, stale-result, and observer semantics.
- `dioform`: Dioxus-facing `FormConfig`, hooks, explicit `FormHandle` APIs, headless bindings, parse blockers, managed submission, async validation task spawning, debounced validation timers, cleanup guards, and an explicit `advanced` module for low-level core/runtime/serialization types.
- `dioform-derive`: `#[derive(Form)]` and `#[derive(FieldGroup)]` support for named form structs.
- `dioform-garde`: optional renderer-agnostic `garde` validation adapter for mapping external diagnostics into Dioform validation errors.
- `dioform-validator`: optional renderer-agnostic `validator` validation adapter that flattens nested `validator` diagnostics into Dioform validation errors.

Input helpers are documented in [`docs/input-helpers.md`](docs/input-helpers.md). File fields are documented in [`docs/file-fields.md`](docs/file-fields.md). Collection fields are documented in [`docs/collection-fields.md`](docs/collection-fields.md). Async and debounced validation are documented in [`docs/async-validation.md`](docs/async-validation.md). Validation adapters are documented in [`docs/validation-adapters.md`](docs/validation-adapters.md). Reusable field groups are introduced below. The [`demo/`](demo) gallery demonstrates typed choices, typed select conversion, parsed numeric and date inputs, nested field paths, field-name overrides, true multi-select fields, form-owned repeatable line items, form-state snapshot restore, observer diagnostics, debounced async field and form validators, plus managed async submit flushing.

The [`demo/`](demo) directory is a docs-by-example gallery (a fullstack `dx serve` app) with an extensive, feature-by-feature set of live examples, each mounted next to the exact source that runs it, plus a set of realistic product forms. Run it with `dx serve` from `demo/`; see [`demo/README.md`](demo/README.md).

The Dioxus facade defaults the shared validation error type to `String` for simple forms. Applications that need structured errors can choose their own `Error` type through `FormHandle<Model, Error>` or `FormConfig<Model, Error>`.

## Derive Field Names

`#[derive(Form)]` supports non-generic named structs with direct field accessors. By default, each rendered field name uses the Rust field identifier, while **Field Identity** remains Rust-based and separate from rendered names.

Use `#[form(name = "...")]` on a field to override one rendered **Field Name** segment. Use `#[form(rename_all = "camelCase")]` on the form model to derive all non-overridden rendered field-name segments in `camelCase`:

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

Supported `rename_all` values: `"camelCase"`. Field-level `#[form(name = "...")]` takes precedence over the form-level policy. Serde rename attributes are intentionally not used for form field names.

## Reusable Field Groups

`#[derive(FieldGroup)]` generates a typed field-group map for reusable groups of fields. A group can be mounted under a nested typed path, or explicitly mapped into a form with a different shape, while reusable rendering still receives an explicit `FormHandle<Model>`.

```rust
use dioxus::prelude::*;
use dioform::{FieldGroup, Form, FormHandle};

#[derive(Clone, Form, FieldGroup)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub zip: String,
}

#[derive(Clone, Form)]
pub struct CheckoutForm {
    billing: Address,
}

#[derive(Clone, Form)]
pub struct ProfileForm {
    street_line: String,
    locality: String,
    postcode: String,
}

fn address_fields<Model>(form: &FormHandle<Model>, fields: AddressFieldGroupMap<Model>) -> Element
where
    Model: 'static,
{
    let street = form.text(fields.street());
    let city = form.text(fields.city());
    let zip = form.text(fields.zip());

    rsx! {
        fieldset {
            input { name: street.name(), value: street.value() }
            input { name: city.name(), value: city.value() }
            input { name: zip.name(), value: zip.value() }
        }
    }
}

fn checkout_address(form: FormHandle<CheckoutForm>) -> Element {
    let billing = Address::mount(CheckoutForm::fields().billing());

    rsx! { {address_fields(&form, billing)} }
}

fn profile_address(form: FormHandle<ProfileForm>) -> Element {
    let address = AddressFieldGroupMap {
        street: ProfileForm::fields().street_line(),
        city: ProfileForm::fields().locality(),
        zip: ProfileForm::fields().postcode(),
    };

    rsx! { {address_fields(&form, address)} }
}
```

Field groups do not own validation or collection item identity. Register cross-field rules with existing form validators, and use collection bindings for collection item fields.

`ValidationMode::on_blur()` is the default validation mode: validators run on blur and submit, but not on every change. Use `ValidationMode::on_submit()` for true submit-only validation, `ValidationMode::on_change()` when value changes should also run validation, or `ValidationMode::submit_then_revalidate()` to validate on submit before the first submit attempt and then revalidate on change or blur. Validator registration still uses explicit `ValidationTriggers`; the validation mode only controls automatic execution, and `ErrorVisibilityPolicy` controls when stored errors are shown.

Design terminology lives in [`CONTEXT.md`](CONTEXT.md).

The first release intentionally keeps form drafts form-owned, reinitialization explicit, input parsing separate from validation, collection item identity library-owned and opaque, and Dioxus access explicit through `FormHandle` as the primary path. Optional typed context access is documented in [`docs/form-context.md`](docs/form-context.md). Advanced escape hatches such as `FormHandle::write_advanced` are intentionally named as low-level APIs.

Minimum verification:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features --workspace`
- `cargo test --all-targets --all-features --workspace`

Or run the same checks (fmt, clippy, test, doc, build) in a container with [Dagger](https://dagger.io), exactly as CI does:

- `dagger check`: from the repo root for the workspace, or from `demo/` for the demo app

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
