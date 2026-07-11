use std::time::Duration;

use dioform::prelude::*;
use dioxus::prelude::*;

/// Async validators run on Dioxus-spawned tasks from an owned snapshot, so they
/// can call a server without blocking input. `.debounce(...)` coalesces
/// keystrokes (the check only fires after typing pauses) and stale results
/// from superseded edits are discarded automatically. `debounce_duration` is the
/// runtime-neutral delay; swap in a browser timer for tighter control.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct HandleForm {
    username: String,
}

#[component]
pub fn AsyncValidationExample() -> Element {
    let form = use_form_handle(|| {
        let handle = FormHandle::<HandleForm>::from_config(
            FormConfig::new(HandleForm::default()).validation_mode(ValidationMode::on_change()),
        );
        handle
            .field(HandleForm::fields().username())
            .async_validator("username-availability")
            .on(ValidationTriggers::new([
                ValidationTrigger::Change,
                ValidationTrigger::Submit,
            ]))
            .debounce(debounce_duration(Duration::from_millis(500)))
            .check(|value, _snapshot| async move {
                // A real validator would await a server call here. The reserved
                // set stands in for "already taken".
                let taken = ["admin", "root", "ada", "taken"];
                if taken.contains(&value.trim().to_ascii_lowercase().as_str()) {
                    vec!["That username is already taken.".to_string()]
                } else {
                    Vec::new()
                }
            });
        handle
    });

    let username = form.text(HandleForm::fields().username());
    let username_oninput = username.clone();

    let checking = form.is_field_validating(HandleForm::fields().username());
    let errors = form.visible_field_validation_errors(HandleForm::fields().username());
    let snapshot = form.snapshot();
    let settled = !checking && errors.is_empty() && !snapshot.username.trim().is_empty();

    rsx! {
        label { class: "block",
            span { class: "mb-1 block text-sm font-medium", "Username (async availability check)" }
            input {
                class: "input input-bordered w-full",
                name: username.name(),
                value: username.value(),
                placeholder: "try: ada, admin, or your own",
                oninput: move |e| username_oninput.on_input(e.value()),
                onblur: move |_| username.on_blur(),
            }
        }
        div { class: "mt-2 min-h-6 text-sm",
            if checking {
                span { class: "inline-flex items-center gap-2 text-base-content/60",
                    span { class: "loading loading-spinner loading-xs" }
                    "Checking availability…"
                }
            } else if !errors.is_empty() {
                for error in errors {
                    span { class: "text-error", "{error.error()}" }
                }
            } else if settled {
                span { class: "text-success", "Username is available." }
            }
        }
    }
}
