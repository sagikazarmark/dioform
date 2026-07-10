# File Fields

File fields are field-like, but they are not ordinary typed `FieldPath<Model, Value>` fields.
Selected files are platform-owned values, so Dioform keeps them outside the `Form Draft` and
outside `FormStateSnapshot` serialization. Use a `FileFieldKey<Model>` to address a file selection
for metadata, validation, accessibility, reset, and file-aware submission.

## Basic Usage

```rust
use dioxus::prelude::*;
use dioform::prelude::*;

#[derive(Clone, Form)]
struct ProfileForm {
    display_name: String,
}

fn profile_form(form: FormHandle<ProfileForm, &'static str>) -> Element {
    let avatar_key = FileFieldKey::new("avatar");
    let display_name = form.text(ProfileForm::fields().display_name());

    use_hook({
        let form = form.clone();
        let avatar_key = avatar_key.clone();

        move || {
            form.file(avatar_key.clone())
                .validator("avatar_required")
                .on(ValidationTrigger::Submit)
                .check_optional({
                    let avatar_key = avatar_key.clone();

                    move |files| {
                        files
                            .selected_files(&avatar_key)
                            .is_empty()
                            .then_some("avatar_required")
                    }
                });
        }
    });

    let avatar = form.file(avatar_key.clone());

    let avatar_for_change = avatar.clone();
    let avatar_for_blur = avatar.clone();

    rsx! {
        form {
            input {
                name: display_name.name(),
                value: display_name.value(),
                oninput: move |event| display_name.on_input(event.value()),
            }
            input {
                r#type: "file",
                name: avatar.name(),
                onchange: move |event| avatar_for_change.on_change(event),
                onblur: move |_| avatar_for_blur.on_blur(),
            }
        }
    }
}
```

Register file validators during one-time form setup, such as the `use_hook` above. Calling
`.check(...)` or `.check_optional(...)` directly on every render registers a new validator each time
and can duplicate errors.

`FileFieldKey::new(...)` creates a single-file field. Use `FileFieldKey::multiple(...)` for a
multi-file field; selected files are then preserved in input order.

## Submission

File-aware submission passes a normal validated `SubmissionSnapshot<Model>` plus a
`FileSubmissionSnapshot<Model>`:

```rust
let attachments_key = FileFieldKey::multiple("attachments");

let result = form.submit_with_files(|submitted, files| {
    let draft = submitted.value();
    let attachments = files.selected_files(&attachments_key);

    // `draft` is the typed form model. `attachments` are adapter-owned selected files.
});
```

Managed async submit has matching file-aware APIs:

```rust
let attachments_key_for_submit = attachments_key.clone();

let result = form.managed_submit().on_submit_async_with_files(event, move |submitted, files| {
    let attachments_key = attachments_key_for_submit.clone();

    async move {
        save_profile(submitted.into_value(), files.selected_files(&attachments_key)).await
    }
});
```

## Validation

File-selection validators are scoped to the file field identity. They run when that file selection is
validated, blurred, changed under a validation mode that validates changes, or when submit validation
requires them. They do not rerun just because an unrelated typed field changes. Async validators
registered with `check_with_context(...)` are model-context-aware: if the form draft changes while
one is pending, its late result is treated as stale instead of applying to the file field.

Use synchronous validators for metadata checks:

```rust
let resume_key = FileFieldKey::new("resume");

form.file(resume_key.clone())
    .validator("resume_pdf")
    .on(ValidationTrigger::Submit)
    .check_optional(move |files| {
        let selected = files.selected_files(&resume_key);

        selected
            .first()
            .is_none_or(|file| file.media_type() != Some("application/pdf"))
            .then_some("resume_must_be_pdf")
    });
```

Use async validators for checks such as virus scanning or server-side upload policy. Submit-triggered
async file validators participate in `SubmitBlocker::PendingValidation` just like other submit
validators.

```rust
let resume_key_for_validator = resume_key.clone();

form.file(resume_key.clone())
    .async_validator("virus_scan")
    .on(ValidationTrigger::Submit)
    .check(move |files| {
        let resume_key = resume_key_for_validator.clone();

        async move {
            let selected = files.selected_files(&resume_key);

            if scan_files(selected).await {
                Vec::new()
            } else {
                vec!["file_rejected"]
            }
        }
    });
```

## State Boundaries

Reset, reinitialization, and Dioxus adapter `restore_state_snapshot(...)` clear selected files.
Form-state snapshots intentionally do not carry selected files, file-field metadata, or file-field
validator result state. This avoids restoring touched or valid file-field state without the actual
platform files.
