use dioform::prelude::*;
use dioxus::prelude::*;

/// Application submit behavior returns structured `SubmitErrors`, targeted at a
/// field or the whole form, separate from validation errors. They surface
/// through the same visible-error reads, and they clear on their own when the
/// offending value changes (stale submit-error clearing). Submit `taken` to see
/// a field-targeted submit error, then edit the field to watch it clear.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct ReserveForm {
    username: String,
}

#[component]
pub fn SubmitErrorsExample() -> Element {
    let form = use_form(ReserveForm::default());
    let fields = ReserveForm::fields();

    let username = form.text(fields.username());
    let username_oninput = username.clone();
    let submit = form.managed_submit();
    let mut message = use_signal(String::new);

    let username_errors = form.visible_field_validation_errors(fields.username());

    rsx! {
        form {
            class: "space-y-3",
            onsubmit: move |event| {
                let result = submit
                    .on_submit(event, |submission: SubmissionSnapshot<ReserveForm>| {
                        if submission.value().username.trim().eq_ignore_ascii_case("taken") {
                            SubmitError::field(
                                ReserveForm::fields().username(),
                                "That username is already reserved.".to_string(),
                            )
                            .into()
                        } else {
                            SubmitErrors::none()
                        }
                    });
                message.set(match result {
                    SubmitResult::Succeeded => "Reserved!".into(),
                    SubmitResult::Rejected => "Rejected by the server-side check.".into(),
                    other => format!("{other:?}"),
                });
            },
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium", "Username (submit \"taken\")" }
                input {
                    class: "input input-bordered w-full",
                    name: username.name(),
                    value: username.value(),
                    oninput: move |e| username_oninput.on_input(e.value()),
                }
                for error in username_errors {
                    p { class: "mt-1 text-sm text-error", "{error.error()}" }
                }
            }
            button { class: "btn btn-primary btn-sm", r#type: "submit", "Reserve" }
        }
        if !message.read().is_empty() {
            p { class: "mt-3 text-sm text-base-content/75", "{message}" }
        }
    }
}
