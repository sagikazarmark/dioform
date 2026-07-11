use dioform::prelude::*;
use dioxus::prelude::*;

/// `visible_validation_errors()` returns every stored error across the whole
/// form (direct fields, collection children, form-level validators, and submit
/// errors) as one flat, source-aware list. That is exactly what an accessible
/// "N errors: jump to field" summary needs. Each entry keeps its `target`,
/// `field_identity`, `source`, and typed `error`, so you map identity back to a
/// field name using your own typed paths.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct ContactForm {
    email: String,
    phone: String,
    age: u32,
}

fn field_label(snapshot: &ValidationErrorSnapshot<String>) -> String {
    let fields = ContactForm::fields();
    match snapshot.field_identity() {
        Some(id) if id == fields.email().identity() => "email".into(),
        Some(id) if id == fields.phone().identity() => "phone".into(),
        Some(id) if id == fields.age().identity() => "age".into(),
        Some(_) => "field".into(),
        None => "form".into(),
    }
}

#[component]
pub fn ErrorSummaryExample() -> Element {
    let form = use_form_handle(|| {
        let handle = FormHandle::<ContactForm>::from_config(
            FormConfig::new(ContactForm::default()).validation_mode(ValidationMode::on_change()),
        );
        handle.write_advanced(|core| {
            let fields = ContactForm::fields();
            core.register_sync_field_validator(fields.email(), "email", |value, _ctx| {
                if value.is_empty() || value.contains('@') {
                    Vec::new()
                } else {
                    vec!["Email looks malformed.".to_string()]
                }
            });
            core.register_sync_field_validator(fields.age(), "min-age", |value: &u32, _ctx| {
                if *value >= 18 {
                    Vec::new()
                } else {
                    vec!["Must be 18 or older.".to_string()]
                }
            });
            core.register_sync_form_validator("contact-required", |ctx| {
                let form = ctx.form();
                if form.email.is_empty() && form.phone.is_empty() {
                    vec![FormValidationError::form(
                        "Provide at least an email or a phone number.".to_string(),
                    )]
                } else {
                    Vec::new()
                }
            });
        });
        handle
    });

    let fields = ContactForm::fields();
    let email = form.text(fields.email());
    let phone = form.text(fields.phone());
    let age = use_number(form.clone(), fields.age());

    let email_oninput = email.clone();
    let phone_oninput = phone.clone();
    let age_oninput = age.clone();

    let summary = form.visible_validation_errors();

    rsx! {
        div { class: "space-y-3",
            input {
                class: "input input-bordered input-sm w-full",
                placeholder: "Email",
                name: email.name(),
                value: email.value(),
                oninput: move |e| email_oninput.on_input(e.value()),
                onblur: move |_| email.on_blur(),
            }
            input {
                class: "input input-bordered input-sm w-full",
                placeholder: "Phone",
                name: phone.name(),
                value: phone.value(),
                oninput: move |e| phone_oninput.on_input(e.value()),
                onblur: move |_| phone.on_blur(),
            }
            input {
                class: "input input-bordered input-sm w-full",
                r#type: "number",
                placeholder: "Age",
                name: age.name(),
                value: age.value(),
                oninput: move |e| age_oninput.on_input(e.value()),
                onblur: move |_| age.on_blur(),
            }
        }
        if summary.is_empty() {
            p { class: "mt-4 text-sm text-success", "No errors: the form is valid." }
        } else {
            div { class: "mt-4 rounded-xl border border-error/40 bg-error/5 p-4",
                p { class: "text-sm font-semibold text-error", "{summary.len()} error(s)" }
                ul { class: "mt-2 space-y-1",
                    for snapshot in summary.iter() {
                        li { class: "text-sm",
                            span { class: "font-mono text-xs text-error/80", "{field_label(snapshot)}" }
                            span { class: "text-base-content/75", ": {snapshot.error()}" }
                        }
                    }
                }
            }
        }
    }
}
