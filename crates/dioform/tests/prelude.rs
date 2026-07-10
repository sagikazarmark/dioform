use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use dioform::advanced::ValidatorId;
use dioform::prelude::*;

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct PreludeSignupForm {
    email: String,
    accepts_terms: bool,
    topics: Vec<String>,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct PreludeAttachmentForm {
    attachments: String,
}

struct PreludeSignupScope;

#[test]
fn prelude_covers_ordinary_form_setup() {
    let form: FormHandle<PreludeSignupForm, &'static str> =
        FormHandle::new_with_error_type(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        });
    let fields = PreludeSignupForm::fields();
    let email_path = fields.email();

    let email_validator = form
        .field(email_path.clone())
        .validator("email")
        .on(ValidationTrigger::Manual)
        .check_optional(|value, _context| value.is_empty().then_some("email_required"));
    let terms_validator = form
        .validator("terms")
        .on(ValidationTrigger::Manual)
        .check_optional(|context| {
            (!context.form().accepts_terms).then_some(FormValidationError::field(
                PreludeSignupForm::fields().accepts_terms(),
                "terms_required",
            ))
        });

    form.validate_all(ValidationTrigger::Manual);

    assert_eq!(
        form.field_validation_status(email_path, email_validator),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.form_validation_status_by_id(terms_validator),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| *error.error())
        .collect();
    assert_eq!(errors, vec!["email_required", "terms_required"]);
}

#[test]
fn prelude_covers_scoped_form_context_access() {
    let _context: Option<FormContext<PreludeSignupScope, PreludeSignupForm, &'static str>> = None;
    let _provide: fn(
        FormHandle<PreludeSignupForm, &'static str>,
    ) -> FormHandle<PreludeSignupForm, &'static str> =
        provide_form_context::<PreludeSignupScope, _, _>;
    let _try_use: fn() -> Option<FormHandle<PreludeSignupForm, &'static str>> =
        try_use_form_context::<PreludeSignupScope, PreludeSignupForm, &'static str>;
    let _use: fn() -> FormHandle<PreludeSignupForm, &'static str> =
        use_form_context::<PreludeSignupScope, PreludeSignupForm, &'static str>;
}

#[test]
fn bindings_expose_common_field_state_directly() {
    let form: FormHandle<PreludeSignupForm, &'static str> =
        FormHandle::new_with_error_type(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        });
    let fields = PreludeSignupForm::fields();
    let email_path = fields.email();
    let email_source = "email".to_owned();

    form.field(email_path.clone())
        .validator(email_source.as_str())
        .on(ValidationTrigger::Manual)
        .check_optional(|value, _context| value.is_empty().then_some("email_required"));
    form.validate_all(ValidationTrigger::Manual);

    let email = form.text(email_path.clone());
    assert!(!email.is_touched());
    assert_eq!(email.metadata(), form.field_metadata(email_path));
    assert_eq!(email.validation_errors()[0].error(), &"email_required");

    email.on_input("ada@example.com");
    email.on_blur();

    assert!(email.is_touched());
    assert!(email.is_blurred());
}

#[test]
fn collection_binding_exposes_collection_field_state() {
    let form = FormHandle::new(PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    });
    let topics = form.collection(PreludeSignupForm::fields().topics());
    let namespace = FormIdNamespace::from("signup".to_owned());

    assert_eq!(namespace.as_ref(), "signup");
    assert_eq!(namespace.to_string(), "signup");
    assert_eq!(topics.name(), "topics");
    assert!(!topics.is_dirty());

    topics.append("rust".to_owned());

    assert!(topics.is_touched());
    assert!(topics.is_dirty());
    assert_eq!(topics.value(), vec!["rust".to_owned()]);
    assert_eq!(topics.items().len(), 1);
}

#[test]
fn file_selection_binding_stores_selected_file_metadata_outside_form_draft() {
    let initial = PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    };
    let form = FormHandle::new(initial.clone());
    let attachments = form.file(FileFieldKey::multiple("attachments"));

    assert_eq!(
        attachments.cardinality(),
        FileSelectionCardinality::Multiple
    );
    assert!(attachments.allows_multiple());

    attachments.select_files([
        SelectedFileMetadata::new("resume.pdf", 1_024).with_media_type("application/pdf"),
        SelectedFileMetadata::new("portfolio.zip", 4_096),
    ]);

    let selected = attachments.selected_files();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].name(), "resume.pdf");
    assert_eq!(selected[0].size_bytes(), 1_024);
    assert_eq!(selected[0].media_type(), Some("application/pdf"));
    assert_eq!(selected[1].name(), "portfolio.zip");
    assert_eq!(selected[1].size_bytes(), 4_096);
    assert_eq!(selected[1].media_type(), None);
    assert_eq!(form.snapshot(), initial);
}

#[test]
fn file_selection_binding_enforces_single_file_cardinality_by_default() {
    let initial = PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    };
    let form = FormHandle::new(initial.clone());
    let avatar = form.file(FileFieldKey::new("avatar"));

    assert_eq!(avatar.cardinality(), FileSelectionCardinality::Single);
    assert!(!avatar.allows_multiple());

    avatar.select_files([
        SelectedFileMetadata::new("avatar.png", 1_024),
        SelectedFileMetadata::new("alternate.png", 2_048),
    ]);

    let selected = avatar.selected_files();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].name(), "avatar.png");
    assert_eq!(form.snapshot(), initial);
}

#[test]
fn file_selection_reads_and_snapshots_apply_reader_cardinality() {
    let form = FormHandle::new(PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    });
    let multiple_key = FileFieldKey::multiple("attachments");
    let single_key = FileFieldKey::single("attachments");
    let multiple = form.file(multiple_key);
    let single = form.file(single_key.clone());
    let mut submitted_files = Vec::new();

    multiple.select_files([
        SelectedFileMetadata::new("resume.pdf", 1_024),
        SelectedFileMetadata::new("portfolio.zip", 4_096),
    ]);

    assert_eq!(multiple.selected_files().len(), 2);

    let selected_through_single_key = single.selected_files();
    assert_eq!(selected_through_single_key.len(), 1);
    assert_eq!(selected_through_single_key[0].name(), "resume.pdf");

    let result = form.submit_with_files(|_submitted, files| {
        submitted_files = files.selected_files(&single_key);
    });

    assert_eq!(result, SubmitResult::Succeeded);
    assert_eq!(submitted_files.len(), 1);
    assert_eq!(submitted_files[0].name(), "resume.pdf");
}

#[test]
fn file_selection_binding_stores_platform_file_handles_outside_form_draft() {
    let initial = PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    };
    let form = FormHandle::new(initial.clone());
    let attachments_key = FileFieldKey::new("attachments");
    let attachments = form.file(attachments_key.clone());
    let file_data = FileData::new(SerializedFileData {
        path: "resume.pdf".into(),
        size: 1_024,
        last_modified: 0,
        content_type: Some("application/pdf".to_owned()),
        contents: None,
    });
    let mut submitted_files: Vec<SelectedFile> = Vec::new();

    attachments.select_files([SelectedFile::from_file_data(file_data)]);

    let result = form.submit_with_files(|submitted, files| {
        assert_eq!(submitted.value(), &initial);
        submitted_files = files.selected_files(&attachments_key);
    });

    assert_eq!(result, SubmitResult::Succeeded);
    assert_eq!(submitted_files.len(), 1);
    assert_eq!(submitted_files[0].name(), "resume.pdf");
    assert_eq!(submitted_files[0].media_type(), Some("application/pdf"));
    assert_eq!(submitted_files[0].file_data().unwrap().name(), "resume.pdf");
    assert_eq!(form.snapshot(), initial);
}

#[test]
fn file_field_identity_does_not_collide_with_ordinary_model_field_identity() {
    let form = FormHandle::new(PreludeAttachmentForm {
        attachments: String::new(),
    });
    let ordinary_attachments = form.text(PreludeAttachmentForm::fields().attachments());
    let file_attachments = form.file(FileFieldKey::new("attachments"));

    file_attachments.on_blur();

    assert!(file_attachments.is_blurred());
    assert!(!ordinary_attachments.is_blurred());
}

#[test]
fn file_selection_binding_tracks_user_interaction_metadata_when_files_are_selected() {
    let form = FormHandle::new(PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    });
    let attachments = form.file(FileFieldKey::new("attachments"));

    assert!(!attachments.is_touched());
    assert_eq!(attachments.metadata(), FieldMetadata::default());

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

    assert!(attachments.is_touched());
    assert!(attachments.metadata().is_touched());
}

#[test]
fn file_selection_binding_tracks_blur_metadata() {
    let form = FormHandle::new(PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    });
    let attachments = form.file(FileFieldKey::new("attachments"));

    assert!(!attachments.is_touched());
    assert!(!attachments.is_blurred());

    attachments.on_blur();

    assert!(attachments.is_touched());
    assert!(attachments.is_blurred());
    assert!(attachments.metadata().is_blurred());
}

#[test]
fn file_selection_binding_exposes_accessibility_helpers() {
    let form = FormHandle::new(PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    })
    .with_id_namespace("signup");
    let attachments = form.file(FileFieldKey::new("attachments"));

    let accessibility = attachments.accessibility();

    assert_eq!(accessibility.input_id(), "signup-attachments-input");
    assert_eq!(accessibility.help_id(), "signup-attachments-help");
    assert_eq!(accessibility.error_id(), "signup-attachments-error");
    assert!(!accessibility.aria_invalid());
    assert_eq!(
        accessibility.aria_describedby().as_deref(),
        Some("signup-attachments-help")
    );
}

#[test]
fn file_selection_binding_exposes_validation_errors_attached_to_its_identity() {
    let form: FormHandle<PreludeSignupForm, &'static str> =
        FormHandle::new_with_error_type(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        });
    let attachments = form.file(FileFieldKey::new("attachments"));
    let attachment_identity = attachments.identity();

    form.validator("attachments")
        .on(ValidationTrigger::Manual)
        .check_optional(move |_context| {
            Some(FormValidationError::field_identity(
                attachment_identity.clone(),
                "file_required",
            ))
        });
    form.validate_all(ValidationTrigger::Manual);

    let errors: Vec<_> = attachments
        .validation_errors()
        .into_iter()
        .map(|error| *error.error())
        .collect();

    assert_eq!(errors, vec!["file_required"]);
}

#[test]
fn file_selection_binding_exposes_visible_validation_errors_after_blur() {
    let form: FormHandle<PreludeSignupForm, &'static str> =
        FormHandle::new_with_error_type(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        });
    let attachments = form.file(FileFieldKey::new("attachments"));
    let attachment_identity = attachments.identity();

    form.validator("attachments")
        .on(ValidationTrigger::Manual)
        .check_optional(move |_context| {
            Some(FormValidationError::field_identity(
                attachment_identity.clone(),
                "file_required",
            ))
        });
    form.validate_all(ValidationTrigger::Manual);

    assert!(attachments.visible_validation_errors().is_empty());

    attachments.on_blur();

    let visible_errors: Vec<_> = attachments
        .visible_validation_errors()
        .into_iter()
        .map(|error| *error.error())
        .collect();

    assert_eq!(visible_errors, vec!["file_required"]);
}

#[test]
fn submit_with_files_receives_a_frozen_file_selection_snapshot() {
    let initial = PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    };
    let form = FormHandle::new(initial.clone());
    let attachments_key = FileFieldKey::new("attachments");
    let attachments = form.file(attachments_key.clone());
    let mut submitted_files: Vec<SelectedFile> = Vec::new();

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

    let result = form.submit_with_files(|submitted, files| {
        assert_eq!(submitted.value(), &initial);

        attachments.select_files([SelectedFileMetadata::new("portfolio.zip", 4_096)]);
        submitted_files = files.selected_files(&attachments_key);
    });

    assert_eq!(result, SubmitResult::Succeeded);
    assert_eq!(submitted_files.len(), 1);
    assert_eq!(submitted_files[0].name(), "resume.pdf");
    assert_eq!(attachments.selected_files()[0].name(), "portfolio.zip");
    assert_eq!(form.snapshot(), initial);
}

#[test]
fn intentful_submit_with_files_preserves_submit_intent_and_file_snapshot() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Intent {
        Publish,
    }

    let form = FormHandle::new(PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    });
    let attachments_key = FileFieldKey::new("attachments");
    let attachments = form.file(attachments_key.clone());
    let mut submitted_intent = None;
    let mut submitted_files = Vec::new();

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

    let result = form
        .intent(Intent::Publish)
        .submit_with_files(|submitted, files| {
            submitted_intent = Some(*submitted.intent());
            submitted_files = files.selected_files(&attachments_key);
        });

    assert_eq!(result, SubmitResult::Succeeded);
    assert_eq!(submitted_intent, Some(Intent::Publish));
    assert_eq!(submitted_files[0].name(), "resume.pdf");
}

#[test]
fn changing_file_selection_clears_stale_file_submit_errors() {
    let form: FormHandle<PreludeSignupForm, &'static str> =
        FormHandle::new_with_error_type(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        });
    let attachments_key = FileFieldKey::new("attachments");
    let attachments = form.file(attachments_key.clone());
    let attachment_identity = attachments_key.identity();

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

    let result = form.submit_with_files(move |_submitted, _files| {
        SubmitError::field_identity(attachment_identity, "try_again")
    });

    assert_eq!(result, SubmitResult::Rejected);
    assert_eq!(attachments.validation_errors()[0].error(), &"try_again");

    attachments.select_files([SelectedFileMetadata::new("portfolio.zip", 4_096)]);

    assert!(attachments.validation_errors().is_empty());
}

#[test]
fn file_selection_binding_clears_selected_files_without_mutating_form_draft() {
    let initial = PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    };
    let form = FormHandle::new(initial.clone());
    let attachments = form.file(FileFieldKey::new("attachments"));

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);
    attachments.clear();

    assert!(attachments.selected_files().is_empty());
    assert_eq!(form.snapshot(), initial);
}

#[test]
fn reset_clears_selected_files_and_restores_form_draft() {
    let initial = PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    };
    let form = FormHandle::new(initial.clone());
    let attachments = form.file(FileFieldKey::new("attachments"));

    form.text(PreludeSignupForm::fields().email())
        .on_input("ada@example.com");
    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);
    form.reset();

    assert!(attachments.selected_files().is_empty());
    assert_eq!(form.snapshot(), initial);
}

#[test]
fn reinitialize_clears_selected_files_and_replaces_form_draft() {
    let initial = PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    };
    let reinitialized = PreludeSignupForm {
        email: "ada@example.com".to_owned(),
        accepts_terms: true,
        topics: vec!["rust".to_owned()],
    };
    let form = FormHandle::new(initial);
    let attachments = form.file(FileFieldKey::new("attachments"));

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);
    form.reinitialize(reinitialized.clone());

    assert!(attachments.selected_files().is_empty());
    assert_eq!(form.snapshot(), reinitialized);
}

#[test]
fn restore_state_snapshot_clears_selected_files() {
    let initial = PreludeSignupForm {
        email: String::new(),
        accepts_terms: false,
        topics: Vec::new(),
    };
    let restored = PreludeSignupForm {
        email: "ada@example.com".to_owned(),
        accepts_terms: true,
        topics: vec!["rust".to_owned()],
    };
    let form: FormHandle<PreludeSignupForm, &'static str> =
        FormHandle::new_with_error_type(initial);
    let snapshot_source: FormHandle<PreludeSignupForm, &'static str> =
        FormHandle::new_with_error_type(restored.clone());
    let snapshot = snapshot_source.state_snapshot();
    let attachments = form.file(FileFieldKey::new("attachments"));

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);
    form.restore_state_snapshot(snapshot)
        .expect("compatible snapshot should restore");

    assert!(attachments.selected_files().is_empty());
    assert_eq!(form.snapshot(), restored);
}

#[test]
fn restore_state_snapshot_clears_file_field_lifecycle_and_validation_state() {
    fn form_with_file_validator() -> FormHandle<PreludeSignupForm, &'static str> {
        let form = FormHandle::new_with_error_type(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        });
        let attachments_key = FileFieldKey::new("attachments");

        form.file(attachments_key.clone())
            .validator("attachments")
            .on(ValidationTrigger::Manual)
            .check_optional(move |files| {
                files
                    .selected_files(&attachments_key)
                    .is_empty()
                    .then_some("file_required")
            });

        form
    }

    let source = form_with_file_validator();
    let source_attachments = source.file(FileFieldKey::new("attachments"));

    source_attachments.select_files(Vec::<SelectedFile>::new());
    source.validate_all(ValidationTrigger::Manual);

    assert!(source_attachments.is_touched());
    assert_eq!(
        source_attachments.validation_errors()[0].error(),
        &"file_required"
    );

    let snapshot = source.state_snapshot();
    let target = form_with_file_validator();
    let target_attachments = target.file(FileFieldKey::new("attachments"));

    target
        .restore_state_snapshot(snapshot)
        .expect("compatible snapshot should restore");

    assert!(target_attachments.selected_files().is_empty());
    assert_eq!(target_attachments.metadata(), FieldMetadata::default());
    assert!(target_attachments.validation_errors().is_empty());
}

#[test]
fn restore_state_snapshot_strips_file_targeted_form_validator_errors() {
    fn form_with_file_targeted_form_validator()
    -> (FormHandle<PreludeSignupForm, &'static str>, ValidatorId) {
        let form = FormHandle::new_with_error_type(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        });
        let attachment_identity = FileFieldKey::<PreludeSignupForm>::new("attachments").identity();

        let validator_id = form
            .validator("attachments")
            .on(ValidationTrigger::Manual)
            .check_optional(move |_context| {
                Some(FormValidationError::field_identity(
                    attachment_identity.clone(),
                    "file_required",
                ))
            });

        (form, validator_id)
    }

    let (source, source_validator_id) = form_with_file_targeted_form_validator();
    let source_attachments = source.file(FileFieldKey::new("attachments"));

    source.validate_all(ValidationTrigger::Manual);

    assert_eq!(
        source.form_validation_status_by_id(source_validator_id),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        source_attachments.validation_errors()[0].error(),
        &"file_required"
    );

    let snapshot = source.state_snapshot();
    let (target, target_validator_id) = form_with_file_targeted_form_validator();
    let target_attachments = target.file(FileFieldKey::new("attachments"));

    target
        .restore_state_snapshot(snapshot)
        .expect("compatible snapshot should restore");

    assert_eq!(
        target.form_validation_status_by_id(target_validator_id),
        Some(ValidationStatus::Unknown)
    );
    assert!(target_attachments.validation_errors().is_empty());
}

#[test]
fn file_selection_change_runs_configured_form_change_validation() {
    let form: FormHandle<PreludeSignupForm, &'static str> = FormHandle::from_config(
        FormConfig::new(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        })
        .validation_mode(ValidationMode::on_change()),
    );
    let attachments = form.file(FileFieldKey::new("attachments"));
    let attachment_identity = attachments.identity();

    form.validator("attachments")
        .on(ValidationTrigger::Change)
        .check_optional(move |_context| {
            Some(FormValidationError::field_identity(
                attachment_identity.clone(),
                "file_required",
            ))
        });

    assert!(attachments.validation_errors().is_empty());

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

    assert_eq!(attachments.validation_errors()[0].error(), &"file_required");
}

#[test]
fn file_selection_validator_receives_file_submission_snapshot() {
    let form: FormHandle<PreludeSignupForm, &'static str> = FormHandle::from_config(
        FormConfig::new(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        })
        .validation_mode(ValidationMode::on_change()),
    );
    let attachments_key = FileFieldKey::new("attachments");
    let attachments = form.file(attachments_key.clone());

    attachments
        .clone()
        .validator("attachments")
        .on(ValidationTrigger::Change)
        .check_optional(move |files| {
            files
                .selected_files(&attachments_key)
                .is_empty()
                .then_some("file_required")
        });

    attachments.select_files(Vec::<SelectedFile>::new());

    assert_eq!(attachments.validation_errors()[0].error(), &"file_required");

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

    assert!(attachments.validation_errors().is_empty());
}

#[test]
fn sync_file_selection_validator_receives_validator_context() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Intent {
        Publish,
    }

    let form: FormHandle<PreludeSignupForm, &'static str> =
        FormHandle::new_with_error_type(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        });
    let attachments_key = FileFieldKey::new("attachments");
    let attachments = form.file(attachments_key.clone());
    let observed_trigger = Rc::new(Cell::new(None));
    let observed_intent = Rc::new(Cell::new(None));
    let observed_touched = Rc::new(Cell::new(false));
    let observed_file_name = Rc::new(RefCell::new(None));

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);
    attachments
        .validator("attachments")
        .on(ValidationTrigger::Submit)
        .check_with_context({
            let observed_trigger = Rc::clone(&observed_trigger);
            let observed_intent = Rc::clone(&observed_intent);
            let observed_touched = Rc::clone(&observed_touched);
            let observed_file_name = Rc::clone(&observed_file_name);

            move |files, context| {
                observed_trigger.set(Some(context.trigger()));
                observed_intent.set(context.submit_intent::<Intent>().copied());
                observed_touched.set(context.field_metadata().is_touched());
                observed_file_name.borrow_mut().replace(
                    files
                        .selected_files(&attachments_key)
                        .first()
                        .expect("selected file should be present")
                        .name()
                        .to_owned(),
                );

                Vec::new()
            }
        });

    let result = form
        .intent(Intent::Publish)
        .submit_with_files(|_submitted, _files| ());

    assert_eq!(result, SubmitResult::Succeeded);
    assert_eq!(observed_trigger.get(), Some(ValidationTrigger::Submit));
    assert_eq!(observed_intent.get(), Some(Intent::Publish));
    assert!(observed_touched.get());
    assert_eq!(observed_file_name.borrow().as_deref(), Some("resume.pdf"));
}

#[test]
fn file_selection_validator_does_not_run_on_unrelated_field_change() {
    let form: FormHandle<PreludeSignupForm, &'static str> = FormHandle::from_config(
        FormConfig::new(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        })
        .validation_mode(ValidationMode::on_change()),
    );
    let attachments_key = FileFieldKey::new("attachments");
    let attachments = form.file(attachments_key.clone());
    let runs = Rc::new(Cell::new(0));

    attachments
        .clone()
        .validator("attachments")
        .on(ValidationTrigger::Change)
        .check_optional({
            let runs = Rc::clone(&runs);

            move |files| {
                runs.set(runs.get() + 1);
                files
                    .selected_files(&attachments_key)
                    .is_empty()
                    .then_some("file_required")
            }
        });

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

    assert_eq!(runs.get(), 1);

    form.text(PreludeSignupForm::fields().email())
        .on_input("ada@example.com");

    assert_eq!(runs.get(), 1);
    assert!(attachments.validation_errors().is_empty());
}

#[test]
fn invalidate_pending_async_validations_marks_file_async_validators_stale() {
    let form: FormHandle<PreludeSignupForm, &'static str> =
        FormHandle::new_with_error_type(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        });
    let attachments = form.file(FileFieldKey::new("attachments"));
    let field = attachments.identity();
    let validator_id = form.write_advanced(|core| {
        core.register_async_field_identity_validator_for_triggers(
            field.clone(),
            "virus_scan",
            ValidationTrigger::Submit,
        )
    });
    let run = form.write_advanced(|core| {
        core.begin_async_field_identity_validation(
            field.clone(),
            validator_id,
            ValidationTrigger::Submit,
        )
        .expect("file async validation should start")
    });
    let status = || {
        form.read_core(|core| {
            core.field_validation_statuses_by_identity(&field)
                .into_iter()
                .find(|view| view.validator_id() == validator_id)
                .map(|view| view.status())
        })
    };

    assert_eq!(status(), Some(ValidationStatus::Pending));

    form.write_advanced(|core| core.invalidate_pending_async_validations());

    assert_eq!(status(), Some(ValidationStatus::Stale));
    assert_eq!(
        form.write_advanced(|core| core.complete_async_field_identity_validation(
            field,
            validator_id,
            &run,
            Vec::<&'static str>::new(),
        )),
        None
    );
}

#[test]
fn collection_mutation_preserves_context_free_file_async_validation_state() {
    let form: FormHandle<PreludeSignupForm, &'static str> =
        FormHandle::new_with_error_type(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        });
    let attachments = form.file(FileFieldKey::new("attachments"));
    let field = attachments.identity();
    let validator_id = form.write_advanced(|core| {
        core.register_async_field_identity_validator_for_triggers(
            field.clone(),
            "virus_scan",
            ValidationTrigger::Change,
        )
    });
    let run = form.write_advanced(|core| {
        core.begin_async_field_identity_validation(
            field.clone(),
            validator_id,
            ValidationTrigger::Change,
        )
        .expect("file async validation should start")
    });

    assert_eq!(
        form.write_advanced(|core| core.complete_async_field_identity_validation(
            field.clone(),
            validator_id,
            &run,
            ["file_rejected"],
        )),
        Some(ValidationStatus::Invalid)
    );

    form.collection(PreludeSignupForm::fields().topics())
        .append("rust".to_owned());

    assert_eq!(attachments.validation_errors()[0].error(), &"file_rejected");
    assert_eq!(
        form.read_core(|core| {
            core.field_validation_statuses_by_identity(&field)
                .into_iter()
                .find(|view| view.validator_id() == validator_id)
                .map(|view| view.status())
        }),
        Some(ValidationStatus::Invalid)
    );
}

#[test]
fn file_selection_blur_runs_configured_form_blur_validation() {
    let form: FormHandle<PreludeSignupForm, &'static str> = FormHandle::from_config(
        FormConfig::new(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        })
        .validation_mode(ValidationMode::on_blur()),
    );
    let attachments = form.file(FileFieldKey::new("attachments"));
    let attachment_identity = attachments.identity();

    form.validator("attachments")
        .on(ValidationTrigger::Blur)
        .check_optional(move |_context| {
            Some(FormValidationError::field_identity(
                attachment_identity.clone(),
                "file_required",
            ))
        });

    assert!(attachments.validation_errors().is_empty());

    attachments.on_blur();

    assert_eq!(attachments.validation_errors()[0].error(), &"file_required");
}

#[test]
fn submit_only_validation_mode_does_not_run_blur_validation() {
    let form: FormHandle<PreludeSignupForm, &'static str> = FormHandle::from_config(
        FormConfig::new(PreludeSignupForm {
            email: String::new(),
            accepts_terms: false,
            topics: Vec::new(),
        })
        .validation_mode(ValidationMode::on_submit()),
    );
    let email_path = PreludeSignupForm::fields().email();
    let email_validator = form
        .field(email_path.clone())
        .validator("email")
        .on(ValidationTrigger::Blur)
        .check_optional(|value, _context| value.is_empty().then_some("email_required"));
    let email = form.text(email_path.clone());

    email.on_blur();

    assert!(email.is_blurred());
    assert_eq!(email.validation_errors().len(), 0);
    assert_eq!(
        form.field_validation_status(email_path, email_validator),
        Some(ValidationStatus::Unknown)
    );
}
