use dioform::prelude::*;
use dioform_garde::GardeValidationExt;
use dioxus::prelude::*;
use dioxus_daisyui::components::{
    button::{Button, ButtonColor},
    field::{Field as DaisyField, FieldDescription, FieldError, FieldLabel},
    input::TextField,
    radio_group::{RadioGroup, RadioItem, RadioItemColor},
    select::{Select, SelectList, SelectOption, SelectTrigger, SelectValue},
    switch::{Switch, SwitchColor},
    textarea::TextareaField,
};

use super::StateGrid;
use crate::components::{DemoPane, DemoSurface};

/// A complete form built from Dioform's standard integration seams: typed Field
/// Paths, garde-backed validation, dioxus-field components, a reusable Field
/// Group, Dioxus-Managed Submission, and field-scoped Form Selector reads.
#[derive(Clone, Debug, Default, PartialEq, Form, FieldGroup)]
struct Preferences {
    product_updates: bool,
    event_reminders: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Track {
    Frontend,
    Backend,
    Leadership,
}

impl Track {
    fn label(self) -> &'static str {
        match self {
            Self::Frontend => "Frontend systems",
            Self::Backend => "Backend systems",
            Self::Leadership => "Engineering leadership",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Form, garde::Validate)]
struct WorkshopRegistration {
    #[garde(length(min = 2, max = 80))]
    name: String,
    #[garde(email)]
    email: String,
    #[garde(length(min = 1))]
    attendance: String,
    #[garde(skip)]
    track: Option<Track>,
    #[garde(length(max = 160))]
    notes: String,
    #[garde(skip)]
    preferences: Preferences,
    #[garde(custom(require_acceptance))]
    accepted_terms: bool,
}

fn require_acceptance(value: &bool, _context: &()) -> Result<(), garde::Error> {
    if *value {
        Ok(())
    } else {
        Err(garde::Error::new("Accept the code of conduct to continue."))
    }
}

fn build_form() -> FormHandle<WorkshopRegistration> {
    let handle = FormHandle::<WorkshopRegistration>::from_config(
        FormConfig::new(WorkshopRegistration::default())
            .validation_mode(ValidationMode::on_commit()),
    )
    .with_id_namespace("workshop-registration");

    handle.write_advanced(|core| {
        core.garde_validation()
            .triggers([ValidationTrigger::Commit, ValidationTrigger::Submit])
            .derived_path_map()
            .register_string_errors();
    });

    handle
}

fn status_label(status: Option<SubmitStatus>) -> String {
    match status {
        None => "-".to_owned(),
        Some(SubmitStatus::Succeeded) => "succeeded".to_owned(),
        Some(SubmitStatus::Rejected) => "rejected".to_owned(),
        Some(SubmitStatus::Blocked(_)) => "blocked".to_owned(),
    }
}

#[component]
pub fn HappyPathExample() -> Element {
    let form = use_form_handle(build_form);
    let fields = WorkshopRegistration::fields();
    let preferences = Preferences::mount(fields.preferences());
    let submit = form.managed_submit();
    let mut success_message = use_signal(String::new);

    let attendance_options = [("in-person", "In person"), ("remote", "Remote")];
    let track_options = [Track::Frontend, Track::Backend, Track::Leadership];

    rsx! {
        DemoSurface {
            primary: rsx! {
                DemoPane { label: "Workshop registration",
                    form {
                        class: "space-y-6",
                        novalidate: true,
                        onsubmit: move |event| {
                            let _ = submit.on_submit(event, |submitted| {
                                let value = submitted.value();
                                success_message.set(format!(
                                    "{} registered for {} attendance.",
                                    value.name, value.attendance
                                ));
                                SubmitErrors::none()
                            });
                        },

                        div { class: "grid gap-5 sm:grid-cols-2",
                            TextField {
                                context: form.text(fields.name()),
                                label: "Name",
                                description: "Required; validated by garde on commit and submit.",
                                class: "min-w-0 w-full",
                                required: true,
                                autocomplete: "name",
                                placeholder: "Ada Lovelace",
                            }
                            TextField {
                                context: form.text(fields.email()),
                                label: "Email",
                                description: "We will only use this for workshop logistics.",
                                class: "min-w-0 w-full",
                                required: true,
                                r#type: "email",
                                autocomplete: "email",
                                placeholder: "ada@example.com",
                            }
                        }

                        DaisyField {
                            context: form.radio_group(fields.attendance()),
                            class: "min-w-0 grid-cols-1",
                            FieldLabel {
                                id: "happy-attendance-label",
                                class: "font-medium",
                                required: true,
                                "Attendance"
                            }
                            RadioGroup {
                                required: true,
                                horizontal: true,
                                aria_labelledby: "happy-attendance-label",
                                for (index, (value, label)) in attendance_options.into_iter().enumerate() {
                                    label { class: "flex cursor-pointer items-center gap-2 text-sm",
                                        RadioItem {
                                            color: RadioItemColor::Primary,
                                            value: value.to_owned(),
                                            index,
                                            aria_label: label,
                                        }
                                        "{label}"
                                    }
                                }
                            }
                            FieldError {}
                        }

                        DaisyField {
                            context: form.select(fields.track()),
                            class: "min-w-0 grid-cols-1",
                            FieldLabel { class: "font-medium", "Track (optional)" }
                            Select::<Track> { class: "w-full",
                                SelectTrigger { class: "w-full",
                                    SelectValue { placeholder: "Choose a track" }
                                }
                                SelectList { class: "z-20 w-full min-w-56",
                                    for (index, track) in track_options.into_iter().enumerate() {
                                        SelectOption::<Track> {
                                            key: "{track:?}",
                                            value: track,
                                            index,
                                            text_value: Some(track.label().to_owned()),
                                            "{track.label()}"
                                        }
                                    }
                                }
                            }
                            FieldDescription { "Leave this blank if you are still deciding." }
                            FieldError {}
                        }

                        TextareaField {
                            context: form.textarea(fields.notes()),
                            label: "Notes (optional)",
                            description: "Accessibility or dietary details, up to 160 characters.",
                            class: "min-w-0 w-full",
                            rows: "3",
                            placeholder: "Anything the organizers should know?",
                        }

                        fieldset { class: "rounded-xl border border-base-300 p-4",
                            legend { class: "px-1 text-sm font-semibold", "Communication preferences" }
                            div { class: "grid gap-4 sm:grid-cols-2",
                                DaisyField {
                                    context: form.checkbox(preferences.product_updates()),
                                    class: "min-w-0 grid-cols-1",
                                    FieldLabel {
                                        id: "happy-product-updates-label",
                                        class: "font-medium",
                                        "Product updates"
                                    }
                                    div { class: "flex items-center gap-3",
                                        Switch {
                                            color: SwitchColor::Primary,
                                            aria_labelledby: "happy-product-updates-label",
                                        }
                                        span { class: "text-sm text-base-content/65", "Monthly release notes" }
                                    }
                                }
                                DaisyField {
                                    context: form.checkbox(preferences.event_reminders()),
                                    class: "min-w-0 grid-cols-1",
                                    FieldLabel {
                                        id: "happy-event-reminders-label",
                                        class: "font-medium",
                                        "Event reminders"
                                    }
                                    div { class: "flex items-center gap-3",
                                        Switch {
                                            color: SwitchColor::Primary,
                                            aria_labelledby: "happy-event-reminders-label",
                                        }
                                        span { class: "text-sm text-base-content/65", "One reminder before the event" }
                                    }
                                }
                            }
                        }

                        DaisyField {
                            context: form.checkbox(fields.accepted_terms()),
                            class: "min-w-0 grid-cols-1 rounded-xl bg-base-200/60 p-4",
                            FieldLabel {
                                id: "happy-terms-label",
                                class: "font-medium",
                                required: true,
                                "I agree to the code of conduct"
                            }
                            div { class: "flex items-center gap-3",
                                Switch {
                                    color: SwitchColor::Primary,
                                    required: true,
                                    aria_labelledby: "happy-terms-label",
                                }
                                span { class: "text-sm text-base-content/65", "Required to register" }
                            }
                            FieldError {}
                        }

                        Button { color: ButtonColor::Primary, r#type: "submit", "Register" }
                    }
                }
            },
            secondary: rsx! {
                DemoPane { label: "Focused form state",
                    StateGrid {
                        rows: vec![
                            ("form.dirty", form.is_dirty().to_string()),
                            ("name.touched", form.is_field_touched(fields.name()).to_string()),
                            ("name.blurred", form.is_field_blurred(fields.name()).to_string()),
                            (
                                "name.visible_errors",
                                form.visible_field_validation_errors(fields.name())
                                    .len()
                                    .to_string(),
                            ),
                            ("terms.touched", form.is_field_touched(fields.accepted_terms()).to_string()),
                            ("submit.attempts", form.submit_attempt_count().to_string()),
                            ("submit.status", status_label(form.last_submit_status())),
                            ("can_submit", form.can_submit().to_string()),
                            (
                                "track",
                                form.field_value(fields.track())
                                    .map(|track| track.label().to_owned())
                                    .unwrap_or_else(|| "-".to_owned()),
                            ),
                            (
                                "product_updates",
                                form.field_value(preferences.product_updates()).to_string(),
                            ),
                        ],
                    }
                    if !success_message.read().is_empty() {
                        div { class: "mt-5 rounded-xl border border-success/30 bg-success/10 p-4 text-sm",
                            p { class: "font-medium text-success", "Last successful submission" }
                            p { class: "mt-1", "{success_message}" }
                        }
                    }
                    p { class: "mt-5 text-sm leading-6 text-base-content/60",
                        "The panel uses field-scoped Form Selectors rather than subscribing each control to the whole form. Optional fields stay optional; every Dioxus-Managed Submission still reruns garde before producing a Submission Snapshot."
                    }
                    p { class: "mt-2 text-sm leading-6 text-base-content/60",
                        "The Field Convention reports Commit and Focus Exit separately. Dioform uses Commit for validation and committed Error Visibility, then Focus Exit for exact touched and Blurred Field state without validating twice."
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_explain_commit_and_focus_exit_as_separate_facts() {
        let html = dioxus::ssr::render_element(rsx! { HappyPathExample {} });

        assert!(html.contains("name.visible_errors"));
        assert!(html.contains("name.blurred"));
        assert!(html.contains("Commit and Focus Exit separately"));
    }
}
