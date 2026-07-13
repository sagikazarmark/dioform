use dioform::prelude::*;
use dioxus::prelude::*;

use super::StateGrid;
use crate::components::{DemoPane, DemoSurface};

/// One form handle drives every input kind through a typed field path: text and
/// textarea (`form.text` / `form.textarea`), a boolean checkbox
/// (`form.checkbox`), a string `<select>` (`form.select`), and a typed radio
/// group (`form.radio_group`). Each binding gives you `name`/`value` (or
/// `checked`) for the element and `on_input`/`on_change`/`on_blur` handlers.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct ProfileForm {
    full_name: String,
    bio: String,
    subscribe: bool,
    role: String,
    plan: String,
}

#[component]
pub fn FieldBindingsExample() -> Element {
    let form = use_form(ProfileForm {
        role: "engineer".into(),
        plan: "team".into(),
        ..Default::default()
    });
    let fields = ProfileForm::fields();

    let name = form.text(fields.full_name());
    let bio = form.textarea(fields.bio());
    let subscribe = form.checkbox(fields.subscribe());
    let role = form.select(fields.role());
    let plan = form.radio_group(fields.plan());

    let snapshot = form.snapshot();

    rsx! {
        DemoSurface {
            primary: rsx! {
                DemoPane { label: "Bindings",
                    div { class: "space-y-4",
                        label { class: "block",
                            span { class: "mb-1 block text-sm font-medium", "Full name (text)" }
                            input {
                                class: "input input-bordered w-full",
                                name: name.name(),
                                value: name.value(),
                                oninput: name.oninput(),
                                onblur: name.onblur(),
                            }
                        }
                        label { class: "block",
                            span { class: "mb-1 block text-sm font-medium", "Bio (textarea)" }
                            textarea {
                                class: "textarea textarea-bordered w-full",
                                name: bio.name(),
                                value: bio.value(),
                                oninput: bio.oninput(),
                            }
                        }
                        label { class: "block",
                            span { class: "mb-1 block text-sm font-medium", "Role (select)" }
                            select {
                                class: "select select-bordered w-full",
                                name: role.name(),
                                value: role.value(),
                                onchange: role.onchange(),
                                option { value: "engineer", selected: role.is_selected(&"engineer".to_string()), "Engineer" }
                                option { value: "designer", selected: role.is_selected(&"designer".to_string()), "Designer" }
                                option { value: "founder", selected: role.is_selected(&"founder".to_string()), "Founder" }
                            }
                        }
                        fieldset {
                            span { class: "mb-1 block text-sm font-medium", "Plan (radio group)" }
                            div { class: "flex gap-2",
                                for (value , label) in [("free", "Free"), ("team", "Team"), ("scale", "Scale")] {
                                    {
                                        let checked = plan.is_selected(&value.to_string());
                                        rsx! {
                                            label {
                                                class: "btn btn-sm btn-outline",
                                                class: if checked { "btn-primary" },
                                                input {
                                                    class: "sr-only",
                                                    r#type: "radio",
                                                    name: plan.name(),
                                                    checked,
                                                    onclick: plan.onselect(value.to_string()),
                                                }
                                                "{label}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        label { class: "flex items-center gap-2",
                            input {
                                class: "checkbox checkbox-primary",
                                r#type: "checkbox",
                                name: subscribe.name(),
                                checked: subscribe.checked(),
                                oninput: subscribe.onchange(),
                            }
                            span { class: "text-sm font-medium", "Subscribe to the newsletter (checkbox)" }
                        }
                    }
                }
            },
            secondary: rsx! {
                DemoPane { label: "Form snapshot",
                    StateGrid {
                        rows: vec![
                            ("full_name", snapshot.full_name.clone()),
                            ("bio", snapshot.bio.clone()),
                            ("role", snapshot.role.clone()),
                            ("plan", snapshot.plan.clone()),
                            ("subscribe", snapshot.subscribe.to_string()),
                        ],
                    }
                }
            },
        }
    }
}
