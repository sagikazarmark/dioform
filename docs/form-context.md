# Form Context Access

Explicit **Form Handles** remain the primary Dioxus integration style. Use context access when a form handle would otherwise be threaded through many renderless helper components in one Dioxus subtree.

Context access provides an existing handle; it does not create or configure a form. Form initialization still belongs in `use_form_config`, `use_form_handle`, or the simpler form hooks.

```rust
use dioxus::prelude::*;
use dioform::prelude::*;

#[derive(Clone, Form)]
struct SignupForm {
    email: String,
}

struct SignupScope;

fn signup_page() -> Element {
    let form = use_form(SignupForm {
        email: String::new(),
    });
    let form = provide_form_context::<SignupScope, _, _>(form);
    let email = form.text(SignupForm::fields().email());

    rsx! {
        form {
            input { name: email.name(), value: email.value() }
            signup_summary {}
        }
    }
}

fn signup_summary() -> Element {
    let form = use_form_context::<SignupScope, SignupForm, String>();
    let email = form.field_value(SignupForm::fields().email());

    rsx! { p { "Current email: {email}" } }
}
```

Use distinct scope marker types when the same **Form Model** appears more than once in a subtree:

```rust
struct ShippingAddressScope;
struct BillingAddressScope;
```

`try_use_form_context::<Scope, Model, Error>()` returns `None` when the provider is missing. `use_form_context::<Scope, Model, Error>()` is the convenience form for components that require the provider and should fail clearly when it is absent.

Context access is still **Renderless Form Access**. It returns a **Form Handle**; applications still own markup, styling, layout, accessibility copy, and visual components.
