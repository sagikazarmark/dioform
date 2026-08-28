use dioform::prelude::*;
use dioform_garde::GardeValidationExt;
use dioxus::prelude::*;

/// An existing `garde` schema can drive form validation without rewriting rules
/// as native validators. The `dioform-garde` adapter registers a form
/// validator that runs `garde`, then maps each diagnostic onto a typed field
/// path through a compile-time-derived explicit path map (rendered names are
/// never treated as validation addresses). Here the shared error type is
/// `String`, so `register_string_errors()` needs no mapper.
#[derive(Clone, Debug, Default, PartialEq, Form, garde::Validate)]
struct SignupForm {
    #[garde(email)]
    email: String,
    #[garde(length(min = 8))]
    password: String,
}

#[component]
pub fn AdapterValidationExample() -> Element {
    let form = use_form_config(
        FormConfig::new(SignupForm::default())
            .validation_mode(ValidationMode::on_change())
            .register_core(|core| {
                core.garde_validation()
                    .triggers([ValidationTrigger::Change, ValidationTrigger::Submit])
                    .derived_path_map()
                    .register_string_errors();
            }),
    );

    let fields = SignupForm::fields();
    let email = form.text(fields.email());
    let password = form.text(fields.password());

    let email_oninput = email.clone();
    let password_oninput = password.clone();

    let email_errors = form.visible_field_validation_errors(fields.email());
    let password_errors = form.visible_field_validation_errors(fields.password());

    rsx! {
        div { class: "space-y-3",
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium", "Email (garde: #[garde(email)])" }
                input {
                    class: "input input-bordered w-full",
                    name: email.name(),
                    value: email.value(),
                    oninput: move |e| email_oninput.on_input(e.value()),
                    onblur: email.onblur(),
                }
                for error in email_errors {
                    p { class: "mt-1 text-sm text-error", "{error.error()}" }
                }
            }
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium",
                    "Password (garde: #[garde(length(min = 8))])"
                }
                input {
                    class: "input input-bordered w-full",
                    r#type: "password",
                    name: password.name(),
                    value: password.value(),
                    oninput: move |e| password_oninput.on_input(e.value()),
                    onblur: password.onblur(),
                }
                for error in password_errors {
                    p { class: "mt-1 text-sm text-error", "{error.error()}" }
                }
            }
        }
    }
}
