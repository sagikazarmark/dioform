use dioxus::prelude::*;
use dioform::prelude::*;

use crate::ui::StateGrid;

/// The form tracks per-field metadata (touched, blurred, dirty) and whole-form
/// rollups (`is_dirty`, `is_pristine`). Dirty is **non-sticky**: revert a field
/// to its baseline and it reads clean again. `reset` returns the whole form to
/// its baseline, `reset_field` resets a single field, and `reinitialize`
/// intentionally replaces the baseline with new data.
#[derive(Clone, Debug, PartialEq, Form)]
struct SettingsForm {
    display_name: String,
    email: String,
}

fn baseline() -> SettingsForm {
    SettingsForm {
        display_name: "Ada".into(),
        email: "ada@example.com".into(),
    }
}

#[component]
pub fn StateMetaExample() -> Element {
    let form = use_form(baseline());
    let fields = SettingsForm::fields();

    let name = form.text(fields.display_name());
    let email = form.text(fields.email());
    let name_oninput = name.clone();
    let email_oninput = email.clone();

    let reset_all = form.clone();
    let reset_name = form.clone();
    let reinit = form.clone();

    rsx! {
        div { class: "space-y-3",
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium", "Display name" }
                input {
                    class: "input input-bordered w-full",
                    name: name.name(),
                    value: name.value(),
                    oninput: move |e| name_oninput.on_input(e.value()),
                    onblur: move |_| name.on_blur(),
                }
            }
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium", "Email" }
                input {
                    class: "input input-bordered w-full",
                    name: email.name(),
                    value: email.value(),
                    oninput: move |e| email_oninput.on_input(e.value()),
                    onblur: move |_| email.on_blur(),
                }
            }
        }

        div { class: "mt-4 flex flex-wrap gap-2",
            button {
                class: "btn btn-sm btn-outline",
                onclick: move |_| reset_all.reset(),
                "reset (all)"
            }
            button {
                class: "btn btn-sm btn-outline",
                onclick: move |_| reset_name.reset_field(SettingsForm::fields().display_name()),
                "reset_field (name)"
            }
            button {
                class: "btn btn-sm btn-ghost",
                onclick: move |_| {
                    reinit.reinitialize(SettingsForm {
                        display_name: "Grace".into(),
                        email: "grace@example.com".into(),
                    });
                },
                "reinitialize"
            }
        }

        div { class: "mt-4 border-t border-base-300 pt-4",
            StateGrid {
                rows: vec![
                    ("form.is_pristine()", form.is_pristine().to_string()),
                    ("form.is_dirty()", form.is_dirty().to_string()),
                    ("name.is_field_dirty", form.is_field_dirty(fields.display_name()).to_string()),
                    ("name.is_field_touched", form.is_field_touched(fields.display_name()).to_string()),
                    ("name.is_field_blurred", form.is_field_blurred(fields.display_name()).to_string()),
                ],
            }
        }
    }
}
