use dioxus::prelude::*;
use dioform::prelude::*;

/// Selected files are platform-owned, so dioform keeps them *outside* the
/// typed draft and addresses them with a `FileFieldKey`. `form.file(key)` gives
/// a binding for the `<input type="file">`; file validators are registered once
/// (in a `use_hook`) and file-aware submission hands the closure both the typed
/// `SubmissionSnapshot` and the selected files.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct UploadForm {
    caption: String,
}

#[component]
pub fn FileFieldsExample() -> Element {
    let form = use_form(UploadForm::default());
    let avatar_key = FileFieldKey::new("avatar");
    let mut status = use_signal(String::new);

    // Register the file validator exactly once, not on every render.
    use_hook({
        let form = form.clone();
        let key = avatar_key.clone();
        move || {
            form.file(key.clone())
                .validator("avatar-required")
                .on(ValidationTrigger::Submit)
                .check_optional({
                    let key = key.clone();
                    move |files| {
                        files
                            .selected_files(&key)
                            .is_empty()
                            .then_some("Choose an image before submitting.".to_string())
                    }
                });
        }
    });

    let caption = form.text(UploadForm::fields().caption());
    let caption_oninput = caption.clone();
    let avatar = form.file(avatar_key.clone());
    let avatar_change = avatar.clone();
    let avatar_blur = avatar.clone();
    let selected = avatar.selected_files();

    let submit = form.managed_submit();

    rsx! {
        form {
            class: "space-y-3",
            onsubmit: move |event| {
                let result = submit
                    .on_submit_with_files(event, |_submitted, _files| {
                        SubmitErrors::<UploadForm, String>::none()
                    });
                status.set(match result {
                    SubmitResult::Succeeded => "Submitted with the selected file.".into(),
                    SubmitResult::Blocked(_) => "Blocked: the file field is required.".into(),
                    other => format!("{other:?}"),
                });
            },
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium", "Caption (typed field)" }
                input {
                    class: "input input-bordered w-full",
                    name: caption.name(),
                    value: caption.value(),
                    oninput: move |e| caption_oninput.on_input(e.value()),
                }
            }
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium", "Avatar (file field)" }
                input {
                    class: "file-input file-input-bordered w-full",
                    r#type: "file",
                    accept: "image/*",
                    name: avatar.name(),
                    onchange: move |e| avatar_change.on_change(e),
                    onblur: move |_| avatar_blur.on_blur(),
                }
            }
            if selected.is_empty() {
                p { class: "text-sm text-base-content/55", "No file selected." }
            } else {
                ul { class: "space-y-1 text-sm",
                    for (i , file) in selected.iter().enumerate() {
                        li { key: "{i}", class: "font-mono text-xs",
                            "{file.name()}"
                            span { class: "text-base-content/50", " · {file.media_type().unwrap_or(\"unknown\")}" }
                        }
                    }
                }
            }
            button { class: "btn btn-primary btn-sm", r#type: "submit", "Submit" }
        }
        if !status.read().is_empty() {
            p { class: "mt-3 text-sm text-base-content/75", "{status}" }
        }
    }
}
