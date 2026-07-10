use dioxus::prelude::*;
use dioform::prelude::*;

/// dioform supports three submit modes. **Managed** (`managed_submit`, shown
/// live) prevents the default and runs the typed lifecycle. **Native browser**
/// (`browser_submit(action)`) hands the browser real `method`/`action`
/// attributes and lets it POST rendered field names: the no-JS fallback.
/// **Progressive** (`progressive_submit`, in `progressive_form` below) runs a
/// client preflight and only blocks the browser POST when a known blocker
/// exists. Submit availability is a prediction for the browser-owned modes.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct SignupForm {
    email: String,
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

    let email = form.text(SignupForm::fields().email());
    let email_oninput = email.clone();
    let submit = form.managed_submit();
    let mut message = use_signal(String::new);

    // Attributes a native browser form would carry (no submit performed here).
    let browser = form.browser_submit("/signup");
    let can_submit = submit.can_submit();

    rsx! {
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
            button {
                class: "btn btn-primary btn-sm",
                r#type: "submit",
                disabled: !can_submit,
                "Managed submit"
            }
        }
        p { class: "mt-2 text-xs text-base-content/55", "submit.can_submit() → {can_submit}" }
        if !message.read().is_empty() {
            p { class: "mt-1 text-sm text-base-content/75", "{message}" }
        }
        div { class: "mt-4 border-t border-base-300 pt-4",
            p { class: "mb-1 text-xs font-semibold uppercase tracking-wider text-base-content/45", "Native browser fallback attributes" }
            p { class: "font-mono text-xs text-base-content/70",
                "method=\"{browser.method()}\" action=\"{browser.action()}\" · field name=\"{email.name()}\""
            }
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
