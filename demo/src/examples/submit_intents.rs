use dioform::prelude::*;
use dioxus::prelude::*;

use crate::components::{DemoPane, DemoSurface};

/// One draft, two submit purposes. A typed **submit intent** names why a
/// submission started, so a submit-triggered validator can require a title only
/// when publishing, and each button reads its own `last_status`. The intent is
/// passed explicitly through `managed_submit().intent(...)`, never inferred
/// from the button.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Intent {
    SaveDraft,
    Publish,
}

#[derive(Clone, Debug, Default, PartialEq, Form)]
struct Article {
    title: String,
    body: String,
}

fn status_label(status: Option<SubmitStatus>) -> String {
    match status {
        None => "-".into(),
        Some(SubmitStatus::Succeeded) => "succeeded".into(),
        Some(SubmitStatus::Rejected) => "rejected".into(),
        Some(SubmitStatus::Blocked(_)) => "blocked".into(),
    }
}

#[component]
pub fn SubmitIntentsExample() -> Element {
    let form = use_form_handle(|| {
        let handle = FormHandle::<Article>::new(Article::default());
        handle.write_advanced(|core| {
            core.register_sync_form_validator_for_triggers(
                "publish-title-required",
                ValidationTrigger::Submit,
                |ctx| {
                    if ctx.submit_intent::<Intent>() == Some(&Intent::Publish)
                        && ctx.form().title.trim().is_empty()
                    {
                        vec![FormValidationError::field(
                            Article::fields().title(),
                            "A title is required to publish.".to_string(),
                        )]
                    } else {
                        Vec::new()
                    }
                },
            );
        });
        handle
    });

    let fields = Article::fields();
    let title = form.text(fields.title());
    let body = form.textarea(fields.body());
    let title_oninput = title.clone();
    let body_oninput = body.clone();

    let submit = form.managed_submit();
    let mut pending = use_signal(|| Intent::SaveDraft);
    let mut message = use_signal(String::new);

    let draft_status = status_label(form.intent(Intent::SaveDraft).last_status());
    let publish_status = status_label(form.intent(Intent::Publish).last_status());
    let title_errors = form.visible_field_validation_errors(fields.title());

    rsx! {
        DemoSurface {
            primary: rsx! {
                DemoPane { label: "Article",
                    form {
                        class: "space-y-3",
                        onsubmit: move |event| {
                            let intent = pending();
                            let result = submit
                                .intent(intent)
                                .on_submit(event, |submission| {
                                    let _ = submission.value();
                                    SubmitErrors::none()
                                });
                            message.set(match (intent, result) {
                                (Intent::SaveDraft, SubmitResult::Succeeded) => "Draft saved.".into(),
                                (Intent::Publish, SubmitResult::Succeeded) => "Published!".into(),
                                (_, SubmitResult::Blocked(_)) => "Publish blocked: add a title.".into(),
                                (_, other) => format!("{other:?}"),
                            });
                        },
                        input {
                            class: "input input-bordered w-full",
                            placeholder: "Title",
                            name: title.name(),
                            value: title.value(),
                            oninput: move |e| title_oninput.on_input(e.value()),
                        }
                        for error in title_errors {
                            p { class: "text-sm text-error", "{error.error()}" }
                        }
                        textarea {
                            class: "textarea textarea-bordered w-full",
                            placeholder: "Body",
                            name: body.name(),
                            value: body.value(),
                            oninput: move |e| body_oninput.on_input(e.value()),
                        }
                        div { class: "flex gap-2",
                            button {
                                class: "btn btn-sm btn-outline",
                                r#type: "submit",
                                onclick: move |_| pending.set(Intent::SaveDraft),
                                "Save draft"
                            }
                            button {
                                class: "btn btn-sm btn-primary",
                                r#type: "submit",
                                onclick: move |_| pending.set(Intent::Publish),
                                "Publish"
                            }
                        }
                    }
                }
            },
            secondary: rsx! {
                DemoPane { label: "Submit status",
                    div { class: "grid grid-cols-2 gap-3 font-mono text-xs",
                        div { "SaveDraft.last_status: " span { class: "font-semibold", "{draft_status}" } }
                        div { "Publish.last_status: " span { class: "font-semibold", "{publish_status}" } }
                    }
                    if !message.read().is_empty() {
                        p { class: "mt-2 text-sm text-base-content/75", "{message}" }
                    }
                }
            },
        }
    }
}
