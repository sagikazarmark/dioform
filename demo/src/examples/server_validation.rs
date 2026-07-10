use dioxus::prelude::*;
use dioform::prelude::*;
use dioform_fullstack::SubmitBindingFullstackExt;

use crate::server_api::check_signup_call;
use crate::signup::SignupRejection;

/// The `dioform-fullstack` adapter routes managed submission through a
/// `#[server]` function. `on_submit_server_fn` takes the async call, a
/// `map_rejection` for application rejections (mapped to a structured field
/// submit error), and a `map_failure` for transport errors, so a "server says
/// no" outcome and a "network broke" outcome stay on separate paths. Submit
/// `taken@example.com` to get a server rejection.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct SignupForm {
    email: String,
}

#[component]
pub fn ServerValidationExample() -> Element {
    let form = use_form(SignupForm::default());
    let fields = SignupForm::fields();

    let email = form.text(fields.email());
    let email_oninput = email.clone();
    let submit = form.managed_submit();
    let mut message = use_signal(String::new);

    let email_errors = form.visible_field_validation_errors(fields.email());
    let is_submitting = form.is_submitting();

    rsx! {
        form {
            class: "space-y-3",
            onsubmit: move |event| {
                let result = submit.on_submit_server_fn(
                    event,
                    |submitted: SubmissionSnapshot<SignupForm>| async move {
                        check_signup_call(submitted.value().email.clone()).await
                    },
                    |rejection| match rejection {
                        SignupRejection::EmailTaken => SubmitError::field(
                            SignupForm::fields().email(),
                            "That email is already registered.".to_string(),
                        )
                        .into(),
                    },
                    |_failure| SubmitError::form(
                        "Could not reach the server. Try again.".to_string(),
                    )
                    .into(),
                );
                message.set(format!("submit started → {result:?}"));
            },
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium", "Email (try taken@example.com)" }
                input {
                    class: "input input-bordered w-full",
                    r#type: "email",
                    name: email.name(),
                    value: email.value(),
                    oninput: move |e| email_oninput.on_input(e.value()),
                }
                for error in email_errors {
                    p { class: "mt-1 text-sm text-error", "{error.error()}" }
                }
            }
            button {
                class: "btn btn-primary btn-sm",
                r#type: "submit",
                disabled: is_submitting,
                "Check on the server"
            }
        }
        if !message.read().is_empty() {
            p { class: "mt-3 text-sm text-base-content/75", "{message}" }
        }
    }
}
