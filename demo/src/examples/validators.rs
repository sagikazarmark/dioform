use dioform::prelude::*;
use dioxus::prelude::*;

/// Two validator kinds. A **field validator** sees one field's value and guards
/// it in isolation (email format). A **form validator** reads the whole model,
/// so it can enforce cross-field rules and attach the resulting error to
/// whichever field should show it (here: the confirmation must match the
/// password). Both are registered once through `write_advanced`.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct SignupForm {
    email: String,
    password: String,
    confirm_password: String,
}

#[component]
pub fn ValidatorsExample() -> Element {
    let form = use_form_handle(|| {
        let handle = FormHandle::<SignupForm>::from_config(
            FormConfig::new(SignupForm::default()).validation_mode(ValidationMode::on_change()),
        );
        handle.write_advanced(|core| {
            let fields = SignupForm::fields();
            core.register_sync_field_validator(fields.email(), "email-format", |value, _ctx| {
                if value.contains('@') && value.contains('.') {
                    Vec::new()
                } else {
                    vec!["Enter a valid email address.".to_string()]
                }
            });
            core.register_sync_form_validator("passwords-match", |ctx| {
                let form = ctx.form();
                if form.password.is_empty() || form.password == form.confirm_password {
                    Vec::new()
                } else {
                    vec![FormValidationError::field(
                        SignupForm::fields().confirm_password(),
                        "Passwords must match.".to_string(),
                    )]
                }
            });
        });
        handle
    });

    let fields = SignupForm::fields();
    let email = form.text(fields.email());
    let password = form.text(fields.password());
    let confirm = form.text(fields.confirm_password());

    let email_oninput = email.clone();
    let password_oninput = password.clone();
    let confirm_oninput = confirm.clone();

    let email_errors = form.visible_field_validation_errors(fields.email());
    let confirm_errors = form.visible_field_validation_errors(fields.confirm_password());

    rsx! {
        div { class: "space-y-3",
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium", "Email (field validator)" }
                input {
                    class: "input input-bordered w-full",
                    r#type: "email",
                    name: email.name(),
                    value: email.value(),
                    oninput: move |e| email_oninput.on_input(e.value()),
                    onblur: move |_| email.on_blur(),
                }
                for error in email_errors {
                    p { class: "mt-1 text-sm text-error", "{error.error()}" }
                }
            }
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium", "Password" }
                input {
                    class: "input input-bordered w-full",
                    r#type: "password",
                    name: password.name(),
                    value: password.value(),
                    oninput: move |e| password_oninput.on_input(e.value()),
                    onblur: move |_| password.on_blur(),
                }
            }
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium", "Confirm password (form validator)" }
                input {
                    class: "input input-bordered w-full",
                    r#type: "password",
                    name: confirm.name(),
                    value: confirm.value(),
                    oninput: move |e| confirm_oninput.on_input(e.value()),
                    onblur: move |_| confirm.on_blur(),
                }
                for error in confirm_errors {
                    p { class: "mt-1 text-sm text-error", "{error.error()}" }
                }
            }
        }
    }
}
