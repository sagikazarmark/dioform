use dioform::prelude::*;
use dioxus::prelude::*;
use dioxus_daisyui::components::{
    field::{Field as DaisyField, FieldDescription, FieldError, FieldLabel},
    input::Input,
    radio_group::{RadioGroup, RadioItem, RadioItemColor},
    switch::{Switch, SwitchColor},
    textarea::Textarea,
};

use super::StateGrid;
use crate::components::{DemoPane, DemoSurface};

/// Dioform produces the `dioxus-field` Binding and Field Meta consumed by the
/// registry. The registry components know nothing about Dioform: each control
/// resolves its value, writes, commit signal, metadata, and errors from the
/// Field Context provided by its surrounding `Field`.
#[derive(Clone, Debug, PartialEq, Form)]
struct RegistryProfile {
    display_name: String,
    bio: String,
    visibility: String,
    analytics: bool,
}

impl Default for RegistryProfile {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            bio: String::new(),
            visibility: "team".to_owned(),
            analytics: false,
        }
    }
}

#[component]
pub fn DioxusFieldRegistryExample() -> Element {
    let form = use_form_handle(|| {
        let handle =
            FormHandle::<RegistryProfile>::from_config(FormConfig::new(RegistryProfile::default()))
                .with_id_namespace("registry-profile");
        handle.write_advanced(|core| {
            let fields = RegistryProfile::fields();
            core.register_sync_field_validator(
                fields.display_name(),
                "display-name",
                |value, _context| {
                    (value.trim().chars().count() < 3)
                        .then(|| "Use at least three characters.".to_owned())
                        .into_iter()
                        .collect()
                },
            );
            core.register_sync_field_validator(fields.bio(), "bio-length", |value, _context| {
                (value.chars().count() > 120)
                    .then(|| "Keep the bio to 120 characters.".to_owned())
                    .into_iter()
                    .collect()
            });
        });
        handle
    });
    let fields = RegistryProfile::fields();
    let snapshot = form.snapshot();
    let visibility_options = [
        ("public", "Public"),
        ("team", "Team only"),
        ("private", "Private"),
    ];

    rsx! {
        DemoSurface {
            primary: rsx! {
                DemoPane { label: "Git registry components",
                    div { class: "space-y-5",
                        DaisyField {
                            context: form.text(fields.display_name()),
                            class: "min-w-0 grid-cols-1",
                            FieldLabel { class: "font-medium", "Display name" }
                            Input {
                                class: "min-w-0 w-full",
                                required: true,
                                placeholder: "Ada Lovelace",
                            }
                            FieldDescription {
                                id: "registry-display-name-help",
                                class: "min-w-0 whitespace-normal text-base-content/60",
                                "This label, description, and error are associated through Field Meta."
                            }
                            FieldError {
                                id: "registry-display-name-error",
                                class: "text-sm",
                            }
                        }

                        DaisyField {
                            context: form.textarea(fields.bio()),
                            class: "min-w-0 grid-cols-1",
                            FieldLabel { class: "font-medium", "Short bio" }
                            Textarea {
                                class: "min-w-0 w-full",
                                rows: "3",
                                placeholder: "What are you working on?",
                            }
                            FieldDescription {
                                id: "registry-bio-help",
                                class: "min-w-0 whitespace-normal text-base-content/60",
                                "The registry commits this field when the native change event ends the edit."
                            }
                            FieldError {
                                id: "registry-bio-error",
                                class: "text-sm",
                            }
                        }

                        DaisyField {
                            context: form.radio_group(fields.visibility()),
                            class: "min-w-0 grid-cols-1",
                            FieldLabel {
                                id: "registry-visibility-label",
                                class: "font-medium",
                                "Profile visibility"
                            }
                            RadioGroup {
                                class: "min-w-0",
                                aria_labelledby: "registry-visibility-label",
                                for (index, (value, label)) in visibility_options.into_iter().enumerate() {
                                    div { class: "flex items-center gap-2",
                                        RadioItem {
                                            color: RadioItemColor::Primary,
                                            value: value.to_owned(),
                                            index,
                                            aria_label: label,
                                        }
                                        span { class: "text-sm", "{label}" }
                                    }
                                }
                            }
                        }

                        DaisyField {
                            context: form.checkbox(fields.analytics()),
                            class: "min-w-0 grid-cols-1",
                            FieldLabel {
                                id: "registry-analytics-label",
                                class: "font-medium",
                                "Anonymous analytics"
                            }
                            div { class: "flex items-center gap-3",
                                Switch {
                                    color: SwitchColor::Primary,
                                    aria_labelledby: "registry-analytics-label",
                                }
                                span { class: "text-sm text-base-content/65",
                                    "The switch writes and commits through the same bool binding."
                                }
                            }
                        }
                    }
                }
            },
            secondary: rsx! {
                DemoPane { label: "Dioform snapshot",
                    StateGrid {
                        rows: vec![
                            ("display_name", snapshot.display_name.clone()),
                            ("bio", snapshot.bio.clone()),
                            ("visibility", snapshot.visibility.clone()),
                            ("analytics", snapshot.analytics.to_string()),
                        ],
                    }
                    p { class: "mt-4 text-sm leading-6 text-base-content/60",
                        "No registry-specific event handlers or value props are wired here. Field Context carries the binding and presentation metadata."
                    }
                }
            },
        }
    }
}
