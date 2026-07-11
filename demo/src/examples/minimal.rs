use dioform::prelude::*;
use dioxus::prelude::*;

/// The smallest useful form: one text field, managed submission, and a status
/// line. `use_form` builds a `FormHandle` from an initial model; `form.text`
/// binds a typed field path to an `<input>`; `managed_submit` owns the submit
/// lifecycle so the closure only sees the validated, owned snapshot.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct GreetForm {
    name: String,
}

#[component]
pub fn MinimalExample() -> Element {
    let form = use_form(GreetForm::default());
    let mut greeting = use_signal(String::new);

    let name = form.text(GreetForm::fields().name());
    let submit = form.managed_submit();

    let name_for_input = name.clone();
    let name_for_blur = name.clone();

    rsx! {
        form {
            class: "space-y-3",
            onsubmit: move |event| {
                let result = submit
                    .on_submit(event, |_submitted: SubmissionSnapshot<GreetForm>| {
                        SubmitErrors::none()
                    });
                if result.is_succeeded() {
                    greeting.set(format!("Hello, {}!", form.snapshot().name));
                }
            },
            input {
                class: "input input-bordered w-full",
                r#type: "text",
                name: name.name(),
                value: name.value(),
                placeholder: "Your name",
                oninput: move |event| name_for_input.on_input(event.value()),
                onblur: move |_| name_for_blur.on_blur(),
            }
            button { class: "btn btn-primary", r#type: "submit", "Greet" }
        }
        if !greeting.read().is_empty() {
            p { class: "mt-3 text-sm font-medium", "{greeting}" }
        }
    }
}
