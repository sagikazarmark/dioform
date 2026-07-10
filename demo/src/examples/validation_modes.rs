use dioxus::prelude::*;
use dioform::prelude::*;

/// `ValidationMode` decides *when* registered validators run automatically. The
/// same "at least 3 characters" rule feels different under each mode: type into
/// both fields and watch when the error appears. On-change validates every
/// keystroke; on-blur (the default) waits until you leave the field.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct AccountForm {
    username: String,
}

#[component]
fn ModeCard(mode: ValidationMode, title: String, hint: String) -> Element {
    let form = use_form_handle(move || {
        let handle = FormHandle::<AccountForm>::from_config(
            FormConfig::new(AccountForm::default()).validation_mode(mode),
        );
        handle.write_advanced(|core| {
            core.register_sync_field_validator(
                AccountForm::fields().username(),
                "min-length",
                |value, _ctx| {
                    if value.chars().count() < 3 {
                        vec!["Use at least 3 characters.".to_string()]
                    } else {
                        Vec::new()
                    }
                },
            );
        });
        handle
    });

    let username = form.text(AccountForm::fields().username());
    let username_oninput = username.clone();
    let errors = form.visible_field_validation_errors(AccountForm::fields().username());

    rsx! {
        div { class: "rounded-xl border border-base-300 bg-base-100 p-4",
            p { class: "text-sm font-semibold", "{title}" }
            p { class: "mb-2 text-xs text-base-content/55", "{hint}" }
            input {
                class: "input input-bordered input-sm w-full",
                name: username.name(),
                value: username.value(),
                oninput: move |e| username_oninput.on_input(e.value()),
                onblur: move |_| username.on_blur(),
            }
            div { class: "mt-1 min-h-5",
                for error in errors {
                    p { class: "text-xs text-error", "{error.error()}" }
                }
            }
        }
    }
}

#[component]
pub fn ValidationModesExample() -> Element {
    rsx! {
        div { class: "grid gap-3 sm:grid-cols-2",
            ModeCard {
                mode: ValidationMode::on_change(),
                title: "ValidationMode::on_change()",
                hint: "Validates on every keystroke.",
            }
            ModeCard {
                mode: ValidationMode::on_blur(),
                title: "ValidationMode::on_blur() (default)",
                hint: "Validates when the field loses focus.",
            }
        }
    }
}
