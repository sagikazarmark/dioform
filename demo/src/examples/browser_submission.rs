use dioform::prelude::*;
use dioxus::prelude::*;

use crate::components::{DemoPane, DemoSurface};

/// dioform supports three submit modes. **Managed** (`managed_submit`, shown
/// live) prevents the default and runs the typed lifecycle. **Native browser**
/// (`browser_submit(action)`) hands the browser real `method`/`action`
/// attributes and lets it POST rendered field names: the no-JS fallback.
/// **Progressive** (`progressive_submit`, in `progressive_form` below) runs a
/// client preflight and only blocks the browser POST when a known blocker
/// exists. Submit availability is a prediction for the browser-owned modes.
/// The managed submit button intentionally stays enabled when availability is
/// false: hard-disabling it would suppress the submit attempt that reveals
/// errors still withheld by the configured Error Visibility policy.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct SignupForm {
    email: String,
}

fn blocked_reason(
    blocker: SubmitBlocker,
    validation_errors: &[ValidationErrorSnapshot<String>],
    parse_errors: &[ParseError],
    pending_validation: &[String],
) -> (&'static str, Vec<String>) {
    match blocker {
        SubmitBlocker::ValidationErrors => (
            "Validation errors",
            validation_errors
                .iter()
                .map(|snapshot| snapshot.error().clone())
                .collect(),
        ),
        SubmitBlocker::ParseErrors => (
            "Parse errors",
            parse_errors
                .iter()
                .map(|error| error.message().to_owned())
                .collect(),
        ),
        SubmitBlocker::PendingValidation => ("Pending validation", pending_validation.to_vec()),
        SubmitBlocker::InFlightSubmission => (
            "Submission in progress",
            vec!["Wait for the current submission to finish.".to_string()],
        ),
        _ => (
            "Another submit blocker",
            vec!["Review the form and try submitting again.".to_string()],
        ),
    }
}

#[component]
pub fn BrowserSubmissionExample() -> Element {
    let form = use_form_handle(|| {
        let handle = FormHandle::<SignupForm>::from_config(
            FormConfig::new(SignupForm::default()).validation_mode(ValidationMode::on_change()),
        );
        handle.write_advanced(|core| {
            core.register_sync_field_validator(
                SignupForm::fields().email(),
                "required",
                |value, _ctx| {
                    if value.trim().is_empty() {
                        vec!["Email is required.".to_string()]
                    } else {
                        Vec::new()
                    }
                },
            );
        });
        handle
    });

    let fields = SignupForm::fields();
    let email = form.text(fields.email());
    let email_oninput = email.clone();
    let submit = form.managed_submit();
    let mut message = use_signal(String::new);

    // Attributes a native browser form would carry (no submit performed here).
    let browser = form.browser_submit("/signup");
    let availability = form.submit_availability();
    let can_submit = availability.is_available();
    let email_errors = form.visible_field_validation_errors(fields.email());
    let validation_errors = form.validation_errors();
    let parse_errors = form.parse_errors();
    let pending_validation = form
        .validation_statuses()
        .into_iter()
        .filter(|status| status.status() == ValidationStatus::Pending)
        .map(|status| format!("{} validator is still running.", status.source()))
        .collect::<Vec<_>>();
    let blocked_reasons = availability
        .blockers()
        .iter()
        .copied()
        .map(|blocker| {
            blocked_reason(
                blocker,
                &validation_errors,
                &parse_errors,
                &pending_validation,
            )
        })
        .collect::<Vec<_>>();

    rsx! {
        DemoSurface {
            primary: rsx! {
                DemoPane { label: "Managed submit",
                    form {
                        class: "space-y-3",
                        onsubmit: move |event| {
                            let result = submit.on_submit(event, |_s| SubmitErrors::<SignupForm, String>::none());
                            message.set(format!("managed submit → {result:?}"));
                        },
                        input {
                            class: "input input-bordered w-full",
                            r#type: "email",
                            placeholder: "Email (required)",
                            name: email.name(),
                            value: email.value(),
                            oninput: move |e| email_oninput.on_input(e.value()),
                        }
                        for error in email_errors {
                            p { class: "text-sm text-error", "{error.error()}" }
                        }
                        button {
                            class: "btn btn-primary btn-sm",
                            r#type: "submit",
                            "Managed submit"
                        }
                    }
                    p { class: "mt-2 text-xs text-base-content/55", "submit.can_submit() → {can_submit}" }
                    if !blocked_reasons.is_empty() {
                        div { class: "mt-3 rounded-xl border border-warning/40 bg-warning/5 p-3",
                            p { class: "text-sm font-semibold", "Current submit availability blockers" }
                            for (category, details) in blocked_reasons {
                                div { class: "mt-2",
                                    p { class: "text-xs font-semibold uppercase tracking-wide text-warning", "{category}" }
                                    ul { class: "mt-1 list-inside list-disc text-sm text-base-content/75",
                                        for detail in details {
                                            li { "{detail}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !message.read().is_empty() {
                        p { class: "mt-1 text-sm text-base-content/75", "{message}" }
                    }
                }
            },
            secondary: rsx! {
                DemoPane { label: "Native browser fallback attributes",
                    p { class: "font-mono text-xs text-base-content/70",
                        "method=\"{browser.method()}\" action=\"{browser.action()}\" · field name=\"{email.name()}\""
                    }
                }
            },
        }
    }
}

/// Progressive submission: hydrated preflight that only blocks a real browser
/// POST when the current client state has a known blocker. Shown for reference;
/// it is not mounted live because it navigates on success.
#[allow(dead_code)]
fn progressive_form(form: FormHandle<SignupForm>) -> Element {
    let submit = form.progressive_submit();
    let email = form.text(SignupForm::fields().email());
    rsx! {
        form {
            method: "post",
            action: "/signup",
            onsubmit: move |event| {
                submit.on_submit(event);
            },
            input { name: email.name(), value: email.value() }
            button { r#type: "submit", "Sign up" }
        }
    }
}
