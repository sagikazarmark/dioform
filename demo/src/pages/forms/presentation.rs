//! Styled form controls used by the realistic form pages.

use dioform::{CheckboxBinding, SelectBinding, TextBinding};
use dioxus::prelude::*;

fn field_errors(errors: Vec<dioform::ValidationErrorSnapshot<String>>) -> Element {
    rsx! {
        div { class: "min-h-4",
            for error in errors {
                p { class: "text-sm text-error", "{error.error()}" }
            }
        }
    }
}

/// A labeled text input bound to a `TextBinding`, with inline validation errors.
pub fn field_text<Model: 'static>(
    label: &str,
    binding: &TextBinding<Model, String>,
    input_type: &'static str,
    placeholder: &str,
) -> Element {
    let accessibility = binding.accessibility();
    rsx! {
        label { class: "block space-y-1",
            span { class: "text-sm font-medium", "{label}" }
            input {
                class: "input input-bordered w-full",
                class: if accessibility.aria_invalid() { "input-error" },
                r#type: "{input_type}",
                name: binding.name(),
                value: binding.value(),
                placeholder: "{placeholder}",
                oninput: binding.oninput(),
                onblur: binding.onblur(),
            }
            {field_errors(binding.visible_validation_errors())}
        }
    }
}

/// A labeled checkbox bound to a `CheckboxBinding`.
pub fn field_checkbox<Model: 'static>(
    label: &str,
    binding: &CheckboxBinding<Model, String>,
) -> Element {
    rsx! {
        label { class: "flex items-center gap-2",
            input {
                class: "checkbox checkbox-primary",
                r#type: "checkbox",
                name: binding.name(),
                checked: binding.checked(),
                oninput: binding.onchange(),
                onblur: binding.onblur(),
            }
            span { class: "text-sm font-medium", "{label}" }
        }
    }
}

/// A labeled string `<select>` bound to a `SelectBinding`, with inline errors.
pub fn field_select<Model: 'static, const N: usize>(
    label: &str,
    binding: &SelectBinding<Model, String, String>,
    options: [(&'static str, &'static str); N],
) -> Element {
    let accessibility = binding.accessibility();
    rsx! {
        label { class: "block space-y-1",
            span { class: "text-sm font-medium", "{label}" }
            select {
                class: "select select-bordered w-full",
                class: if accessibility.aria_invalid() { "select-error" },
                name: binding.name(),
                value: binding.value(),
                onchange: binding.onchange(),
                onblur: binding.onblur(),
                for (value , option_label) in options {
                    option {
                        value: "{value}",
                        selected: binding.is_selected(&value.to_string()),
                        "{option_label}"
                    }
                }
            }
            {field_errors(binding.visible_validation_errors())}
        }
    }
}
