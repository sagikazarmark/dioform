use dioform::prelude::*;
use dioform_garde::GardeValidationExt;
use dioxus::prelude::*;
use dioxus_daisyui::components::{
    button::{Button, ButtonColor},
    field::{Field as DaisyField, FieldDescription, FieldError, FieldLabel},
    input::TextField,
    radio_group::{RadioGroup, RadioItem, RadioItemColor},
    select::{Select, SelectList, SelectOption, SelectTrigger, SelectValue},
    switch::{SwitchColor, SwitchField},
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
    #[garde(custom(require_choice))]
    attendance: String,
    #[garde(range(min = 1, max = 5))]
    seats: u32,
    #[garde(skip)]
    track: Option<Track>,
    #[garde(length(max = 160))]
    notes: String,
    #[garde(skip)]
    preferences: Preferences,
    #[garde(custom(require_acceptance))]
    accepted_terms: bool,
}

fn require_choice(value: &str, _context: &()) -> Result<(), garde::Error> {
    if value.is_empty() {
        Err(garde::Error::new("Choose how you will attend."))
    } else {
        Ok(())
    }
}

fn require_acceptance(value: &bool, _context: &()) -> Result<(), garde::Error> {
    if *value {
        Ok(())
    } else {
        Err(garde::Error::new("Accept the code of conduct to continue."))
    }
}

fn build_form() -> FormHandle<WorkshopRegistration> {
    FormHandle::<WorkshopRegistration>::from_config(
        FormConfig::new(WorkshopRegistration {
            seats: 1,
            ..WorkshopRegistration::default()
        })
        .id_namespace("workshop-registration")
        .validation_mode(ValidationMode::on_commit())
        .register_core(|core| {
            core.garde_validation()
                .triggers([ValidationTrigger::Commit, ValidationTrigger::Submit])
                .derived_path_map()
                .register_string_errors();
        }),
    )
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
    let seats = use_number(&form, fields.seats());
    let seats_state = seats.clone();
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
                            TextField {
                                context: seats,
                                label: "Seats",
                                description: "Parsed into a u32; text that is not a number is a Parse Error, not a validation error.",
                                class: "min-w-0 w-full",
                                required: true,
                                inputmode: "numeric",
                                placeholder: "1",
                            }
                        }

                        DaisyField {
                            context: form.radio_group(fields.attendance()),
                            class: "min-w-0 grid-cols-1",
                            FieldLabel { class: "font-medium", required: true, "Attendance" }
                            RadioGroup { required: true, horizontal: true,
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
                                SwitchField {
                                    context: form.field(preferences.product_updates()),
                                    label: "Product updates",
                                    description: "Monthly release notes",
                                    color: SwitchColor::Primary,
                                }
                                SwitchField {
                                    context: form.field(preferences.event_reminders()),
                                    label: "Event reminders",
                                    description: "One reminder before the event",
                                    color: SwitchColor::Primary,
                                }
                            }
                        }

                        div { class: "rounded-xl bg-base-200/60 p-4",
                            SwitchField {
                                context: form.field(fields.accepted_terms()),
                                label: "I agree to the code of conduct",
                                description: "Required to register",
                                color: SwitchColor::Primary,
                                required: true,
                            }
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
                            ("seats.parse_blocked", seats_state.parse_error().is_some().to_string()),
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
                    p { class: "mt-2 text-sm leading-6 text-base-content/60",
                        "Seats binds through the same registry control as the text fields. Its binding is over the rendered text, so unparsable input keeps the last parsed value in the Form Draft, holds a Parse Blocker, and reports the parse message where validation errors appear."
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the generated id of the `<label>` whose text is `text`.
    fn label_id(html: &str, text: &str) -> String {
        let end = html
            .find(&format!(">{text}</label>"))
            .expect("labelled field");
        let tag = &html[html[..end].rfind("<label ").expect("label tag")..end];
        let id = tag.find("id=\"").expect("label id") + 4;

        tag[id..]
            .split('"')
            .next()
            .expect("label id value")
            .to_owned()
    }

    #[test]
    fn composite_controls_are_named_by_their_field_label() {
        let html = dioxus::ssr::render_element(rsx! { HappyPathExample {} });

        for label in ["Attendance", "I agree to the code of conduct"] {
            let id = label_id(&html, label);

            assert!(
                html.contains(&format!("aria-labelledby=\"{id}\"")),
                "the control for {label:?} does not reference its Field Label id {id:?}",
            );
        }
    }

    #[test]
    fn diagnostics_explain_commit_and_focus_exit_as_separate_facts() {
        let html = dioxus::ssr::render_element(rsx! { HappyPathExample {} });

        assert!(html.contains("name.visible_errors"));
        assert!(html.contains("name.blurred"));
        assert!(html.contains("Commit and Focus Exit separately"));
    }
}
