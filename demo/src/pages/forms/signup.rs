use dioxus::prelude::*;
use dioform::prelude::*;

use crate::ui::{PageHeader, StateGrid, field_checkbox, field_text};

#[derive(Clone, Debug, Default, PartialEq, Form)]
struct SignupForm {
    email: String,
    password: String,
    accepts_terms: bool,
}

fn build() -> FormHandle<SignupForm> {
    let form = FormHandle::<SignupForm>::from_config(
        FormConfig::new(SignupForm::default()).validation_mode(ValidationMode::on_blur()),
    );
    form.write_advanced(|core| {
        let fields = SignupForm::fields();
        core.register_sync_field_validator(fields.email(), "email", |value, _ctx| {
            let value = value.trim();
            if value.is_empty() {
                vec!["Enter an email address.".to_string()]
            } else if !(value.contains('@') && value.contains('.')) {
                vec!["Use an address with a local part and a domain.".to_string()]
            } else {
                Vec::new()
            }
        });
        core.register_sync_field_validator(
            fields.password(),
            "password",
            |value: &String, _ctx| {
                if value.chars().count() < 8 {
                    vec!["Use at least 8 characters.".to_string()]
                } else {
                    Vec::new()
                }
            },
        );
        core.register_sync_form_validator("signup-rules", |ctx| {
            let form = ctx.form();
            let fields = SignupForm::fields();
            let mut errors = Vec::new();
            if !form.accepts_terms {
                errors.push(FormValidationError::field(
                    fields.accepts_terms(),
                    "Accept the terms to continue.".to_string(),
                ));
            }
            let local = form.email.split('@').next().unwrap_or("").trim();
            if !local.is_empty() && form.password.to_lowercase().contains(&local.to_lowercase()) {
                errors.push(FormValidationError::field(
                    fields.password(),
                    "Do not include your email name in the password.".to_string(),
                ));
            }
            errors
        });
    });
    form
}

#[component]
pub fn Signup() -> Element {
    let form = use_form_handle(build);
    let fields = SignupForm::fields();
    let mut status = use_signal(|| "No submission yet.".to_string());

    let email = form.text(fields.email());
    let password = form.text(fields.password());
    let terms = form.checkbox(fields.accepts_terms());
    let submit = form.managed_submit();

    let submit_for_form = submit.clone();
    let form_for_reset = form.clone();
    let mut status_reset = status;

    let snapshot = form.snapshot();
    let can_submit = submit.can_submit();
    let terms_errors = form.visible_field_validation_errors(fields.accepts_terms());

    rsx! {
        PageHeader {
            eyebrow: "Realistic forms",
            title: "Signup",
            intro: "Required fields, a cross-field password rule, a submit error from the server-side check, and reset, combined on one page. Try taken@example.com once the form is otherwise valid.",
        }
        div { class: "mt-8 grid gap-6 lg:grid-cols-[1fr_18rem]",
            div { class: "rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm",
                form {
                    class: "space-y-4",
                    onsubmit: move |event| {
                        let result = submit_for_form.on_submit(event, |submission: SubmissionSnapshot<SignupForm>| {
                            if submission.value().email.trim().eq_ignore_ascii_case("taken@example.com") {
                                SubmitError::field(
                                    SignupForm::fields().email(),
                                    "That email is already registered.".to_string(),
                                )
                                .into()
                            } else {
                                SubmitErrors::none()
                            }
                        });
                        status.set(match result {
                            SubmitResult::Succeeded => "Account created.".to_string(),
                            SubmitResult::Rejected => "That email is already registered.".to_string(),
                            SubmitResult::Blocked(_) => "Fix the highlighted fields first.".to_string(),
                            SubmitResult::Started => "Working…".to_string(),
                        });
                    },
                    {field_text("Email", &email, "email", "ada@example.com")}
                    {field_text("Password", &password, "password", "at least 8 characters")}
                    div {
                        {field_checkbox("Accept the terms of service", &terms)}
                        div { class: "min-h-4",
                            for error in terms_errors {
                                p { class: "text-sm text-error", "{error.error()}" }
                            }
                        }
                    }
                    div { class: "flex gap-2 border-t border-base-300 pt-4",
                        button {
                            class: "btn btn-primary",
                            class: if !can_submit { "btn-disabled" },
                            r#type: "submit",
                            "Create account"
                        }
                        button {
                            class: "btn btn-ghost",
                            r#type: "button",
                            onclick: move |_| {
                                form_for_reset.reset();
                                status_reset.set("No submission yet.".to_string());
                            },
                            "Reset"
                        }
                    }
                }
            }
            aside { class: "space-y-3 rounded-2xl border border-base-300 bg-base-200/40 p-5",
                p { class: "text-xs font-semibold uppercase tracking-wider text-base-content/45", "Live state" }
                StateGrid {
                    rows: vec![
                        ("email", snapshot.email.clone()),
                        ("password_len", snapshot.password.chars().count().to_string()),
                        ("accepts_terms", snapshot.accepts_terms.to_string()),
                        ("can_submit", can_submit.to_string()),
                    ],
                }
                p { class: "text-sm text-base-content/70", "{status}" }
            }
        }
    }
}
