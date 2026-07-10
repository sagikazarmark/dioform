use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use dioform_core::{
    CollectionItemIdentity, ErrorVisibilityPolicy, FieldIdentity, FieldPath, FieldUpdateOrigin,
    FormCore, FormObserverEvent, FormObserverField, FormValidationError, SubmitAttempt,
    SubmitBlocker, SubmitError, SubmitErrors, SubmitResult, SubmitStatus, ValidationMode,
    ValidationStatus, ValidationTarget, ValidationTrigger, ValidationTriggers, ValidatorSource,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContactForm {
    name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContactSubmitIntent {
    SaveDraft,
    Publish,
}

fn name_path() -> FieldPath<ContactForm, String> {
    FieldPath::direct(
        FieldIdentity::new("name"),
        "name",
        |model: &ContactForm| &model.name,
        |model: &mut ContactForm| &mut model.name,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegistrationForm {
    email: String,
    password: String,
    confirm_password: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InvoiceForm {
    lines: Vec<InvoiceLine>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InvoicePage {
    invoice: InvoiceForm,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InvoiceLine {
    description: String,
    quantity: u32,
}

fn lines_path() -> FieldPath<InvoiceForm, Vec<InvoiceLine>> {
    FieldPath::direct(
        FieldIdentity::new("lines"),
        "lines",
        |model: &InvoiceForm| &model.lines,
        |model: &mut InvoiceForm| &mut model.lines,
    )
}

fn invoice_path() -> FieldPath<InvoicePage, InvoiceForm> {
    FieldPath::direct(
        FieldIdentity::new("invoice"),
        "invoice",
        |model: &InvoicePage| &model.invoice,
        |model: &mut InvoicePage| &mut model.invoice,
    )
}

fn nested_lines_path() -> FieldPath<InvoicePage, Vec<InvoiceLine>> {
    invoice_path().join(lines_path())
}

fn line_description_path() -> FieldPath<InvoiceLine, String> {
    FieldPath::direct(
        FieldIdentity::new("description"),
        "description",
        |line: &InvoiceLine| &line.description,
        |line: &mut InvoiceLine| &mut line.description,
    )
}

fn line_field_identity(item: CollectionItemIdentity, field: &'static str) -> FieldIdentity {
    FieldIdentity::collection_item("lines", item, field)
}

fn invoice_form() -> InvoiceForm {
    InvoiceForm {
        lines: vec![
            InvoiceLine {
                description: "Design".to_owned(),
                quantity: 2,
            },
            InvoiceLine {
                description: "Build".to_owned(),
                quantity: 1,
            },
        ],
    }
}

fn invoice_page() -> InvoicePage {
    InvoicePage {
        invoice: invoice_form(),
    }
}

fn email_path() -> FieldPath<RegistrationForm, String> {
    FieldPath::direct(
        FieldIdentity::new("email"),
        "email",
        |model: &RegistrationForm| &model.email,
        |model: &mut RegistrationForm| &mut model.email,
    )
}

fn password_path() -> FieldPath<RegistrationForm, String> {
    FieldPath::direct(
        FieldIdentity::new("password"),
        "password",
        |model: &RegistrationForm| &model.password,
        |model: &mut RegistrationForm| &mut model.password,
    )
}

fn confirm_password_path() -> FieldPath<RegistrationForm, String> {
    FieldPath::direct(
        FieldIdentity::new("confirm_password"),
        "confirm_password",
        |model: &RegistrationForm| &model.confirm_password,
        |model: &mut RegistrationForm| &mut model.confirm_password,
    )
}

#[test]
fn form_core_owns_a_draft_and_replaces_field_values() {
    let mut form = FormCore::new(ContactForm {
        name: "Grace".to_owned(),
    });

    assert_eq!(form.draft().baseline().name, "Grace");
    assert_eq!(form.field_value(name_path()), "Grace");

    form.set_field(name_path(), "Ada".to_owned());

    assert_eq!(form.draft().baseline().name, "Grace");
    assert_eq!(form.field_value(name_path()), "Ada");
    assert_eq!(
        form.snapshot(),
        ContactForm {
            name: "Ada".to_owned()
        }
    );
}

#[test]
fn public_api_supports_standard_rust_affordances() {
    let source_name = "email".to_owned();
    let source = ValidatorSource::from(source_name.as_str());
    assert_eq!(source.as_ref(), "email");
    assert_eq!(source.to_string(), "email");
    assert_eq!(String::from(source), "email");

    let path = name_path();
    assert!(format!("{path:?}").contains("FieldPath"));

    let triggers: ValidationTriggers = [ValidationTrigger::Blur, ValidationTrigger::Blur]
        .into_iter()
        .collect();
    assert!(triggers.contains(ValidationTrigger::Blur));
    assert!(!triggers.contains(ValidationTrigger::Change));

    let submit_errors: SubmitErrors<ContactForm, &'static str> =
        [SubmitError::form("server unavailable")]
            .into_iter()
            .collect();
    assert_eq!(submit_errors.errors().len(), 1);
    assert_eq!((&submit_errors).into_iter().count(), 1);
    assert_eq!(submit_errors.into_iter().count(), 1);

    let attempt: SubmitAttempt<ContactForm> = SubmitAttempt::Blocked(SubmitBlocker::ParseErrors);
    assert!(attempt.is_blocked());
    assert_eq!(attempt.blocker(), Some(SubmitBlocker::ParseErrors));

    let result = SubmitResult::Blocked(SubmitBlocker::ValidationErrors);
    assert!(result.is_blocked());
    assert_eq!(result.blocker(), Some(SubmitBlocker::ValidationErrors));

    let status = SubmitStatus::Rejected;
    assert!(status.is_rejected());
    assert_eq!(status.blocker(), None);
}

#[test]
fn validation_mode_names_match_blur_and_change_semantics() {
    assert_eq!(ValidationMode::default(), ValidationMode::on_blur());

    assert!(!ValidationMode::on_submit().validates_on_blur());
    assert!(!ValidationMode::on_submit().validates_on_change());

    assert!(ValidationMode::on_blur().validates_on_blur());
    assert!(!ValidationMode::on_blur().validates_on_change());
    assert_eq!(
        ValidationMode::on_blur_or_submit(),
        ValidationMode::on_blur()
    );

    assert!(ValidationMode::on_change().validates_on_blur());
    assert!(ValidationMode::on_change().validates_on_change());
    assert!(!ValidationMode::submit_then_revalidate().validates_on_blur());
    assert!(!ValidationMode::submit_then_revalidate().validates_on_change());
    assert!(!ValidationMode::submit_then_revalidate().should_validate_on_blur(0));
    assert!(!ValidationMode::submit_then_revalidate().should_validate_on_change(0));
    assert!(ValidationMode::submit_then_revalidate().should_validate_on_blur(1));
    assert!(ValidationMode::submit_then_revalidate().should_validate_on_change(1));

    assert!(
        ValidationMode::on_submit()
            .validate_on_blur()
            .validates_on_blur()
    );
    assert!(
        !ValidationMode::on_blur()
            .with_blur_validation(false)
            .validates_on_blur()
    );
}

#[test]
fn dirty_state_is_derived_from_current_values_and_baseline_values() {
    let mut form = FormCore::new(ContactForm {
        name: "Grace".to_owned(),
    });

    assert!(!form.is_dirty());
    assert!(!form.is_field_dirty(name_path()));

    form.set_field(name_path(), "Ada".to_owned());

    assert!(form.is_dirty());
    assert!(form.is_field_dirty(name_path()));
    assert!(!form.is_field_touched(name_path()));

    form.set_field(name_path(), "Grace".to_owned());

    assert!(!form.is_dirty());
    assert!(!form.is_field_dirty(name_path()));
}

#[test]
fn collection_item_identity_follows_reorder_with_metadata_and_errors() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let items = form.collection_items(lines_path());
    let first = items[0].identity();
    let second = items[1].identity();
    let second_description = line_field_identity(second, "description");

    assert_eq!(items[0].index(), 0);
    assert_eq!(items[1].index(), 1);

    form.set_user_collection_item_field(
        lines_path(),
        second,
        line_description_path(),
        "Build v2".to_owned(),
    );
    form.mark_collection_item_field_blurred(lines_path(), second, line_description_path());
    let validated_second_description = second_description.clone();
    form.register_sync_form_validator("line-errors", move |_context| {
        vec![FormValidationError::field_identity(
            validated_second_description.clone(),
            "describe line",
        )]
    });
    form.validate_form(ValidationTrigger::Manual);

    assert!(form.is_collection_dirty(lines_path()));
    assert!(form.is_collection_item_field_dirty(lines_path(), second, line_description_path()));
    assert!(form.is_field_identity_touched(&second_description));
    assert!(form.is_field_identity_blurred(&second_description));
    assert_eq!(
        form.field_validation_errors_by_identity(&second_description)[0].error(),
        &"describe line"
    );

    assert!(form.move_user_collection_item_to_index(lines_path(), second, 0));

    let items = form.collection_items(lines_path());
    assert_eq!(items[0].identity(), second);
    assert_eq!(items[1].identity(), first);
    assert_eq!(form.snapshot().lines[0].description, "Build v2");
    assert!(form.is_field_identity_touched(&second_description));
    assert!(form.is_field_identity_blurred(&second_description));
    assert_eq!(
        form.field_validation_errors_by_identity(&second_description)[0].error(),
        &"describe line"
    );
}

#[test]
fn nested_collection_paths_keep_static_path_names_and_logical_item_identity() {
    let mut form = FormCore::new(invoice_page());
    let lines = nested_lines_path();
    let description = line_description_path();
    let items = form.collection_items(lines.clone());
    let second = items[1].identity();
    let second_description = FieldIdentity::collection_item("invoice.lines", second, "description");

    assert_eq!(lines.identity().as_str(), "invoice.lines");
    assert_eq!(lines.field_name(), "invoice.lines");

    assert!(form.set_user_collection_item_field(
        lines.clone(),
        second,
        description,
        "Build v2".to_owned(),
    ));

    assert_eq!(form.snapshot().invoice.lines[1].description, "Build v2");
    assert!(form.is_field_identity_touched(&second_description));
    assert!(form.move_user_collection_item_to_index(lines, second, 0));
    assert_eq!(form.snapshot().invoice.lines[0].description, "Build v2");
    assert!(form.is_field_identity_touched(&second_description));
}

#[test]
fn field_path_try_join_rejects_collection_item_identities() {
    let mut form = FormCore::new(invoice_form());
    let item = form.collection_items(lines_path())[0].identity();
    let item_identity_lines_path = FieldPath::direct(
        FieldIdentity::collection_item("lines", item, ""),
        "lines[0]",
        |model: &InvoiceForm| &model.lines,
        |model: &mut InvoiceForm| &mut model.lines,
    );

    assert!(invoice_path().try_join(lines_path()).is_some());
    assert!(invoice_path().try_join(item_identity_lines_path).is_none());
}

#[test]
fn form_state_snapshot_round_trips_collection_item_identities_and_item_scoped_state() {
    let mut source: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let lines = lines_path();
    let description = line_description_path();
    let initial_items = source.collection_items(lines.clone());
    let removed = initial_items[0].identity();
    let kept = initial_items[1].identity();
    let inserted = source
        .insert_user_collection_item(
            lines.clone(),
            1,
            InvoiceLine {
                description: "Review".to_owned(),
                quantity: 3,
            },
        )
        .expect("insert index should be valid");

    assert!(inserted > kept);

    let removed_line = source
        .remove_user_collection_item(lines.clone(), removed)
        .expect("first item should be removable");
    assert_eq!(removed_line.description, "Design");
    assert!(source.move_user_collection_item_to_index(lines.clone(), kept, 0));

    let kept_quantity = FieldIdentity::collection_item("lines", kept, "quantity");
    let inserted_description = FieldIdentity::collection_item("lines", inserted, "description");

    assert_eq!(
        source.submit(|_submitted| {
            SubmitError::field_identity(kept_quantity.clone(), "server quantity")
        }),
        SubmitResult::Rejected,
    );
    source.register_sync_collection_item_field_validator(
        lines.clone(),
        description.clone(),
        "required",
        |value, _context| {
            if value.trim().is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );
    source.mark_collection_item_field_blurred(lines.clone(), inserted, description.clone());
    source.set_user_collection_item_field(
        lines.clone(),
        inserted,
        description.clone(),
        String::new(),
    );
    source.validate_all(ValidationTrigger::Manual);

    assert_eq!(source.snapshot().lines[0].description, "Build");
    assert_eq!(source.snapshot().lines[1].description, "");
    assert!(source.is_field_identity_blurred(&inserted_description));
    assert_eq!(
        source.field_validation_errors_by_identity(&inserted_description)[0].error(),
        &"required",
    );
    assert_eq!(
        source.field_validation_errors_by_identity(&kept_quantity)[0].error(),
        &"server quantity",
    );

    let snapshot = source.state_snapshot();
    let identity_state = snapshot.collection_identity_state();
    let lines_state = identity_state
        .collections()
        .iter()
        .find(|state| state.collection() == lines.identity())
        .expect("lines collection identity should be serialized");

    assert_eq!(identity_state.version(), 1);
    assert_eq!(lines_state.baseline_items(), &[removed, kept]);
    assert_eq!(lines_state.current_items(), &[kept, inserted]);
    assert_eq!(lines_state.next_item_identity(), 3);

    let mut restored: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(InvoiceForm { lines: Vec::new() });
    restored.register_sync_collection_item_field_validator(
        lines.clone(),
        description,
        "required",
        |value, _context| {
            if value.trim().is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );

    restored
        .restore_state_snapshot(snapshot)
        .expect("serialized form state should restore");

    let restored_items: Vec<_> = restored
        .collection_items(lines.clone())
        .into_iter()
        .map(|item| item.identity())
        .collect();

    assert_eq!(restored_items, vec![kept, inserted]);
    assert_eq!(restored.snapshot().lines[0].description, "Build");
    assert_eq!(restored.snapshot().lines[1].description, "");
    assert!(restored.is_field_identity_blurred(&inserted_description));
    assert_eq!(
        restored.field_validation_errors_by_identity(&inserted_description)[0].error(),
        &"required",
    );
    assert!(
        restored
            .field_validation_errors_by_identity(&kept_quantity)
            .is_empty()
    );
    assert_eq!(restored.last_submit_status(), None);

    let next = restored.push_user_collection_item(
        lines.clone(),
        InvoiceLine {
            description: "Ship".to_owned(),
            quantity: 1,
        },
    );

    assert!(next > inserted);
    assert_eq!(restored.collection_items(lines)[2].identity(), next);
}

#[test]
fn form_state_snapshot_restores_collection_item_validator_state_only_for_registered_validators() {
    let mut source: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let lines = lines_path();
    let description = line_description_path();
    let first = source.collection_items(lines.clone())[0].identity();

    source.register_sync_collection_item_field_validator(
        lines.clone(),
        description.clone(),
        "required",
        |value, _context| {
            if value.trim().is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );
    source.set_user_collection_item_field(lines.clone(), first, description, String::new());
    source.validate_all(ValidationTrigger::Manual);

    let field = FieldIdentity::collection_item("lines", first, "description");
    assert_eq!(
        source.field_validation_errors_by_identity(&field)[0].error(),
        &"required"
    );

    let snapshot = source.state_snapshot();
    let mut restored: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(InvoiceForm { lines: Vec::new() });

    restored
        .restore_state_snapshot(snapshot)
        .expect("snapshot should restore without collection-item validator registration");

    assert_eq!(restored.snapshot().lines[0].description, "");
    assert!(
        restored
            .field_validation_errors_by_identity(&field)
            .is_empty()
    );
    assert!(restored.validation_errors().is_empty());
}

#[test]
fn form_state_snapshot_drops_pending_async_validation_work() {
    let mut source: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = source.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );

    source
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Change)
        .expect("async validation should start");

    assert_eq!(
        source.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(
        source.submit_availability().blockers(),
        &[SubmitBlocker::PendingValidation]
    );

    let snapshot = source.state_snapshot();
    let mut restored: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "placeholder".to_owned(),
        });
    let restored_availability = restored.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );

    restored
        .restore_state_snapshot(snapshot)
        .expect("snapshot should restore");

    assert_eq!(restored.snapshot().name, "Ada");
    assert_eq!(
        restored.field_validation_status(name_path(), restored_availability),
        Some(ValidationStatus::Unknown)
    );
    assert!(restored.submit_availability().is_available());
    assert!(
        restored
            .begin_async_field_validation(
                name_path(),
                restored_availability,
                ValidationTrigger::Change,
            )
            .is_some()
    );
}

#[test]
fn form_state_snapshot_restores_validator_results_without_overwriting_registered_configuration() {
    let mut source: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    source.register_sync_field_validator_for_triggers(
        name_path(),
        "snapshot_source",
        ValidationTrigger::Manual,
        |_value, _context| vec!["snapshot_error"],
    );
    source.validate_field(name_path(), ValidationTrigger::Manual);

    let snapshot = source.state_snapshot();
    let mut restored: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "placeholder".to_owned(),
        });
    restored.register_sync_field_validator_for_triggers(
        name_path(),
        "target_source",
        ValidationTrigger::Blur,
        |_value, _context| vec!["target_error"],
    );

    restored
        .restore_state_snapshot(snapshot)
        .expect("snapshot should restore onto matching validator id");

    let restored_errors: Vec<_> = restored
        .field_validation_errors(name_path())
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(restored_errors, vec![("target_source", "snapshot_error")]);

    restored.validate_field(name_path(), ValidationTrigger::Blur);

    let rerun_errors: Vec<_> = restored
        .field_validation_errors(name_path())
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(rerun_errors, vec![("target_source", "target_error")]);
}

#[test]
fn form_state_snapshot_clears_submit_validation_runtime_state() {
    let mut target: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    target.register_sync_form_validator_for_triggers(
        "publish_only",
        ValidationTrigger::Submit,
        |context| {
            if context.submit_intent::<ContactSubmitIntent>() == Some(&ContactSubmitIntent::Publish)
            {
                vec![FormValidationError::form("publish intent leaked")]
            } else {
                Vec::new()
            }
        },
    );
    let stale_validation = target
        .intent(ContactSubmitIntent::Publish)
        .validation_snapshot();

    assert!(
        !target
            .intent(ContactSubmitIntent::Publish)
            .validate_for_submit()
    );
    assert_eq!(
        target.visible_form_validation_errors_for_intent(&ContactSubmitIntent::Publish)[0].error(),
        &"publish intent leaked"
    );

    let source: FormCore<ContactForm, &'static str> = FormCore::new_with_error_type(ContactForm {
        name: "Grace".to_owned(),
    });
    let snapshot = source.state_snapshot();

    target
        .restore_state_snapshot(snapshot)
        .expect("snapshot should restore");

    assert_eq!(target.snapshot().name, "Grace");
    assert!(matches!(
        target
            .intent(ContactSubmitIntent::Publish)
            .begin_submission_after_validation(&stale_validation),
        SubmitAttempt::Blocked(_)
    ));

    target.validate_all(ValidationTrigger::Submit);

    assert!(target.form_validation_errors().is_empty());
}

#[test]
fn form_state_snapshot_invalidates_submit_validation_snapshot_when_versions_would_collide() {
    let mut target: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });

    target.reset();
    target.reset();
    let stale_validation = target.submit_validation_snapshot();

    let mut source: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Grace".to_owned(),
        });
    source.reset();
    let snapshot = source.state_snapshot();

    target
        .restore_state_snapshot(snapshot)
        .expect("snapshot should restore");

    assert_eq!(target.snapshot().name, "Grace");
    assert!(matches!(
        target.begin_submission_after_validation(&stale_validation),
        SubmitAttempt::Blocked(_)
    ));
}

#[test]
fn collection_item_removal_clears_item_scoped_state() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let item = form.collection_items(lines_path())[1].identity();
    let description = line_field_identity(item, "description");

    form.mark_collection_item_field_blurred(lines_path(), item, line_description_path());
    let validated_description = description.clone();
    form.register_sync_form_validator("line-errors", move |_context| {
        vec![FormValidationError::field_identity(
            validated_description.clone(),
            "bad line",
        )]
    });
    form.validate_form(ValidationTrigger::Manual);

    assert!(form.is_field_identity_blurred(&description));
    assert_eq!(
        form.field_validation_errors_by_identity(&description).len(),
        1
    );

    let removed = form
        .remove_user_collection_item(lines_path(), item)
        .expect("item should be removed");

    assert_eq!(removed.description, "Build");
    assert!(!form.is_field_identity_touched(&description));
    assert!(
        form.field_validation_errors_by_identity(&description)
            .is_empty()
    );
    assert!(
        form.collection_item_field_value(lines_path(), item, line_description_path())
            .is_none()
    );
    assert_eq!(form.snapshot().lines.len(), 1);
}

#[test]
fn collection_item_submit_errors_are_cleared_by_item_change_or_removal() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let item = form.collection_items(lines_path())[0].identity();
    let quantity = line_field_identity(item, "quantity");

    assert_eq!(
        form.submit(|_submitted| SubmitError::field_identity(quantity.clone(), "server quantity")),
        SubmitResult::Rejected
    );
    assert_eq!(
        form.field_validation_errors_by_identity(&quantity)[0].error(),
        &"server quantity"
    );

    assert!(form.set_user_collection_item_field(lines_path(), item, line_quantity_path(), 4));

    assert!(
        form.field_validation_errors_by_identity(&quantity)
            .is_empty()
    );

    assert_eq!(
        form.submit(|_submitted| SubmitError::field_identity(quantity.clone(), "server quantity")),
        SubmitResult::Rejected
    );

    form.remove_user_collection_item(lines_path(), item);

    assert!(
        form.field_validation_errors_by_identity(&quantity)
            .is_empty()
    );
}

#[test]
fn collection_item_validator_templates_apply_to_current_and_inserted_items() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let first = form.collection_items(lines_path())[0].identity();

    form.register_sync_collection_item_field_validator(
        lines_path(),
        line_quantity_path(),
        "quantity",
        |value, _context| {
            if *value == 0 {
                vec!["quantity required"]
            } else {
                Vec::new()
            }
        },
    );

    form.set_user_collection_item_field(lines_path(), first, line_quantity_path(), 0);
    form.validate_all(ValidationTrigger::Manual);

    let first_quantity = line_field_identity(first, "quantity");
    assert_eq!(
        form.field_validation_errors_by_identity(&first_quantity)[0].error(),
        &"quantity required"
    );

    let inserted = form.push_user_collection_item(
        lines_path(),
        InvoiceLine {
            description: "Review".to_owned(),
            quantity: 0,
        },
    );

    form.validate_all(ValidationTrigger::Manual);

    let inserted_quantity = line_field_identity(inserted, "quantity");
    assert_eq!(
        form.field_validation_errors_by_identity(&inserted_quantity)[0].error(),
        &"quantity required"
    );
}

#[test]
fn collection_item_validator_errors_follow_reordered_items_and_clear_on_removal() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let items = form.collection_items(lines_path());
    let first = items[0].identity();
    let second = items[1].identity();
    let second_quantity = line_field_identity(second, "quantity");

    form.register_sync_collection_item_field_validator(
        lines_path(),
        line_quantity_path(),
        "quantity",
        |value, _context| {
            if *value == 0 {
                vec!["quantity required"]
            } else {
                Vec::new()
            }
        },
    );
    form.set_user_collection_item_field(lines_path(), second, line_quantity_path(), 0);
    form.validate_all(ValidationTrigger::Manual);

    assert_eq!(
        form.field_validation_errors_by_identity(&second_quantity)[0].error(),
        &"quantity required"
    );

    assert!(form.move_user_collection_item_to_index(lines_path(), second, 0));

    let items = form.collection_items(lines_path());
    assert_eq!(items[0].identity(), second);
    assert_eq!(items[1].identity(), first);
    assert_eq!(
        form.field_validation_errors_by_identity(&second_quantity)[0].error(),
        &"quantity required"
    );

    form.remove_user_collection_item(lines_path(), second);

    assert!(
        form.field_validation_errors_by_identity(&second_quantity)
            .is_empty()
    );
    assert!(
        form.field_validation_statuses_by_identity(&second_quantity)
            .is_empty()
    );
}

#[test]
fn collection_item_validator_templates_participate_in_submit_and_coexist_with_other_validators() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let item = form.collection_items(lines_path())[0].identity();

    form.register_sync_field_validator(lines_path(), "lines", |lines, _context| {
        if lines.is_empty() {
            vec!["line required"]
        } else {
            Vec::new()
        }
    });
    form.register_sync_collection_item_field_validator_for_triggers(
        lines_path(),
        line_quantity_path(),
        "quantity",
        ValidationTrigger::Submit,
        |value, _context| {
            if *value == 0 {
                vec!["quantity required"]
            } else {
                Vec::new()
            }
        },
    );
    form.register_sync_form_validator("invoice", |_context| Vec::new());

    form.set_user_collection_item_field(lines_path(), item, line_quantity_path(), 0);

    assert_eq!(
        form.submit(|_submitted| SubmitErrors::<InvoiceForm, &'static str>::none()),
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );

    let quantity = line_field_identity(item, "quantity");
    assert_eq!(
        form.field_validation_errors_by_identity(&quantity)[0].error(),
        &"quantity required"
    );
    assert_eq!(
        form.validation_statuses()
            .into_iter()
            .filter(|status| status.target().as_field() == Some(&quantity))
            .count(),
        1
    );
}

#[test]
fn collection_item_chain_views_preserve_duplicate_labels_and_flattened_order() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(InvoiceForm {
            lines: vec![InvoiceLine {
                description: "Design".to_owned(),
                quantity: 0,
            }],
        });
    let item = form.collection_items(lines_path())[0].identity();
    let quantity = line_field_identity(item, "quantity");

    let form_rule = form.register_sync_form_validator_for_triggers(
        "invoice_form",
        ValidationTrigger::Manual,
        {
            let quantity = quantity.clone();
            move |_context| {
                vec![
                    FormValidationError::field_identity(quantity.clone(), "form_quantity"),
                    FormValidationError::form("form_invoice"),
                ]
            }
        },
    );
    let quantity_first = form.register_sync_collection_item_field_validator_for_triggers(
        lines_path(),
        line_quantity_path(),
        "quantity",
        ValidationTrigger::Manual,
        |value, context| {
            assert_eq!(context.source().as_str(), "quantity");

            if *value == 0 {
                vec!["quantity_first"]
            } else {
                Vec::new()
            }
        },
    );
    let lines_rule = form.register_sync_field_validator_for_triggers(
        lines_path(),
        "lines",
        ValidationTrigger::Manual,
        |lines, _context| {
            if lines.len() == 1 {
                vec!["single_line"]
            } else {
                Vec::new()
            }
        },
    );
    let quantity_second = form.register_sync_collection_item_field_validator_for_triggers(
        lines_path(),
        line_quantity_path(),
        "quantity",
        ValidationTrigger::Manual,
        |value, context| {
            assert_eq!(context.source().as_str(), "quantity");

            if *value == 0 {
                vec!["quantity_second"]
            } else {
                Vec::new()
            }
        },
    );

    assert!(form_rule.as_u64() < quantity_first.as_u64());
    assert!(quantity_first.as_u64() < lines_rule.as_u64());
    assert!(lines_rule.as_u64() < quantity_second.as_u64());

    form.validate_all(ValidationTrigger::Manual);

    assert_eq!(
        form.submit(|_submitted| {
            SubmitErrors::with_source(
                "server",
                [
                    SubmitError::field_identity(quantity.clone(), "server_quantity"),
                    SubmitError::form("server_form"),
                ],
            )
        }),
        SubmitResult::Rejected,
    );

    let statuses: Vec<_> = form
        .validation_statuses()
        .into_iter()
        .map(|status| {
            (
                status.target(),
                status.validator_id(),
                status.source().as_str().to_owned(),
                status.status(),
            )
        })
        .collect();
    assert_eq!(
        statuses,
        vec![
            (
                ValidationTarget::Field(lines_path().identity()),
                lines_rule,
                "lines".to_owned(),
                ValidationStatus::Invalid,
            ),
            (
                ValidationTarget::Field(quantity.clone()),
                quantity_first,
                "quantity".to_owned(),
                ValidationStatus::Invalid,
            ),
            (
                ValidationTarget::Field(quantity.clone()),
                quantity_second,
                "quantity".to_owned(),
                ValidationStatus::Invalid,
            ),
            (
                ValidationTarget::Form,
                form_rule,
                "invoice_form".to_owned(),
                ValidationStatus::Invalid,
            ),
        ]
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.target(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                Some(lines_rule),
                ValidationTarget::Field(lines_path().identity()),
                "lines",
                "single_line",
            ),
            (
                Some(quantity_first),
                ValidationTarget::Field(quantity.clone()),
                "quantity",
                "quantity_first",
            ),
            (
                Some(quantity_second),
                ValidationTarget::Field(quantity.clone()),
                "quantity",
                "quantity_second",
            ),
            (
                Some(form_rule),
                ValidationTarget::Field(quantity.clone()),
                "invoice_form",
                "form_quantity",
            ),
            (
                Some(form_rule),
                ValidationTarget::Form,
                "invoice_form",
                "form_invoice",
            ),
            (
                None,
                ValidationTarget::Field(quantity.clone()),
                "server",
                "server_quantity",
            ),
            (None, ValidationTarget::Form, "server", "server_form"),
        ]
    );
}

fn line_quantity_path() -> FieldPath<InvoiceLine, u32> {
    FieldPath::direct(
        FieldIdentity::new("quantity"),
        "quantity",
        |line: &InvoiceLine| &line.quantity,
        |line: &mut InvoiceLine| &mut line.quantity,
    )
}

#[test]
fn collection_insertions_get_distinct_logical_identities() {
    let mut form = FormCore::new(invoice_form());
    let first_items = form.collection_items(lines_path());
    let inserted = form.insert_user_collection_item(
        lines_path(),
        1,
        InvoiceLine {
            description: "Review".to_owned(),
            quantity: 3,
        },
    );

    let items = form.collection_items(lines_path());

    assert_eq!(inserted, Some(items[1].identity()));
    assert_ne!(items[1].identity(), first_items[0].identity());
    assert_ne!(items[1].identity(), first_items[1].identity());
    assert_eq!(form.snapshot().lines[1].description, "Review");
    assert!(form.is_collection_dirty(lines_path()));
}

#[test]
fn user_interaction_tracks_touched_and_blurred_separately() {
    let mut form = FormCore::new(ContactForm {
        name: "Grace".to_owned(),
    });

    form.set_user_field(name_path(), "Ada".to_owned());

    assert!(form.is_field_touched(name_path()));
    assert!(!form.is_field_blurred(name_path()));

    form.mark_field_blurred(name_path());

    assert!(form.is_field_touched(name_path()));
    assert!(form.is_field_blurred(name_path()));
}

#[test]
fn observer_events_report_transitions_without_field_values_by_default() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&events);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));
    form.register_sync_field_validator(name_path(), "required", |value, _context| {
        if value.is_empty() {
            vec!["required"]
        } else {
            Vec::new()
        }
    });

    form.set_user_field(name_path(), "Ada".to_owned());
    form.validate_field(name_path(), ValidationTrigger::Manual);
    assert!(matches!(form.begin_submission(), SubmitAttempt::Started(_)));
    form.reset();
    form.reinitialize(ContactForm {
        name: "Lin".to_owned(),
    });

    let events = events.borrow();
    let debug_output = format!("{events:?}");

    assert_eq!(events.len(), 6);

    let FormObserverEvent::FieldUpdated {
        field,
        origin,
        value,
        ..
    } = &events[0]
    else {
        panic!("expected field update event, got {:?}", events[0]);
    };
    assert_eq!(*field, FormObserverField::from_path(&name_path()));
    assert_eq!(*origin, FieldUpdateOrigin::User);
    assert!(value.is_redacted());

    assert!(matches!(
        &events[1],
        FormObserverEvent::ValidationRan {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Valid,
            ..
        } if field.as_str() == "name" && source.as_str() == "required"
    ));

    let FormObserverEvent::SubmitAttempted { attempt, .. } = &events[2] else {
        panic!("expected submit attempt event, got {:?}", events[2]);
    };
    assert_eq!(*attempt, 1);

    assert!(matches!(
        &events[3],
        FormObserverEvent::ValidationRan {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Submit,
            status: ValidationStatus::Valid,
            ..
        } if field.as_str() == "name" && source.as_str() == "required"
    ));

    let FormObserverEvent::Reset { value, .. } = &events[4] else {
        panic!("expected reset event, got {:?}", events[4]);
    };
    assert!(value.is_redacted());

    let FormObserverEvent::Reinitialized { value, .. } = &events[5] else {
        panic!("expected reinitialization event, got {:?}", events[5]);
    };
    assert!(value.is_redacted());

    assert!(events.iter().any(|event| match event {
        FormObserverEvent::FieldUpdated { value, .. }
        | FormObserverEvent::Reset { value, .. }
        | FormObserverEvent::Reinitialized { value, .. } => value.is_redacted(),
        _ => false,
    }));
    assert!(!debug_output.contains("Ada"));
    assert!(!debug_output.contains("Lin"));
}

#[test]
fn form_state_snapshot_does_not_transfer_observers() {
    let source_events = Rc::new(Cell::new(0));
    let observed_source_events = Rc::clone(&source_events);
    let mut source: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });

    source.observe(move |_event| observed_source_events.set(observed_source_events.get() + 1));

    let snapshot = source.state_snapshot();
    let restored_events = Rc::new(Cell::new(0));
    let observed_restored_events = Rc::clone(&restored_events);
    let mut restored: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "placeholder".to_owned(),
        });

    restored
        .observe(move |_event| observed_restored_events.set(observed_restored_events.get() + 1));
    restored
        .restore_state_snapshot(snapshot)
        .expect("snapshot should restore");
    restored.set_field(name_path(), "Lin".to_owned());

    assert_eq!(source_events.get(), 0);
    assert_eq!(restored_events.get(), 1);
}

#[test]
fn observer_event_output_redacts_sensitive_values_by_default() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&events);
    let secret = "correct-horse-battery-staple";
    let mut form = FormCore::new(RegistrationForm {
        email: String::new(),
        password: String::new(),
        confirm_password: String::new(),
    });

    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));

    form.set_user_field(password_path(), secret.to_owned());
    form.reset();
    form.reinitialize(RegistrationForm {
        email: "ada@example.com".to_owned(),
        password: secret.to_owned(),
        confirm_password: secret.to_owned(),
    });

    let events = events.borrow();
    let debug_output = format!("{events:?}");

    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|event| match event {
        FormObserverEvent::FieldUpdated { value, .. }
        | FormObserverEvent::Reset { value, .. }
        | FormObserverEvent::Reinitialized { value, .. } => value.is_redacted(),
        _ => true,
    }));
    assert!(!debug_output.contains(secret));
}

#[test]
fn registering_sync_field_validators_does_not_run_validation() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let field = name_path();
    let expected_identity = field.identity();
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator(field, "required", move |value, context| {
        validator_runs.set(validator_runs.get() + 1);
        assert_eq!(value, "");
        assert_eq!(context.form().name.as_str(), "");
        assert_eq!(context.field_identity(), expected_identity);
        assert_eq!(context.source().as_str(), "required");
        assert_eq!(context.trigger(), ValidationTrigger::Manual);
        assert!(!context.field_metadata().is_blurred());
        vec!["required"]
    });

    assert_eq!(runs.get(), 0);
    assert_eq!(
        form.validation_status(name_path(), "required"),
        Some(ValidationStatus::Unknown)
    );
    assert!(form.validation_errors().is_empty());

    form.validate_field(name_path(), ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.validation_status(name_path(), "required"),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(form.validation_errors()[0].error(), &"required");
}

#[test]
fn form_initialization_does_not_validate_by_default() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "initial_required",
        ValidationTrigger::Initial,
        move |value, context| {
            validator_runs.set(validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Initial);

            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );

    assert_eq!(runs.get(), 0);
    assert_eq!(
        form.validation_status(name_path(), "initial_required"),
        Some(ValidationStatus::Unknown)
    );
    assert!(form.validation_errors().is_empty());
    assert!(form.visible_validation_errors().is_empty());
    assert!(form.can_submit());
}

#[test]
fn explicit_initialization_validation_records_source_status_and_visibility() {
    let initial_runs = Rc::new(Cell::new(0));
    let manual_runs = Rc::new(Cell::new(0));
    let initial_validator_runs = Rc::clone(&initial_runs);
    let manual_validator_runs = Rc::clone(&manual_runs);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "initial_required",
        ValidationTrigger::Initial,
        move |value, context| {
            initial_validator_runs.set(initial_validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Initial);

            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );
    form.register_sync_field_validator_for_triggers(
        name_path(),
        "manual_required",
        ValidationTrigger::Manual,
        move |_value, context| {
            manual_validator_runs.set(manual_validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Manual);
            vec!["manual_required"]
        },
    );
    form.register_sync_form_validator_for_triggers(
        "initial_form",
        ValidationTrigger::Initial,
        |context| {
            assert_eq!(context.trigger(), ValidationTrigger::Initial);
            vec![FormValidationError::form("initial_form_invalid")]
        },
    );

    assert!(!form.validate_initialization());

    assert_eq!(initial_runs.get(), 1);
    assert_eq!(manual_runs.get(), 0);
    assert_eq!(
        form.validation_status(name_path(), "initial_required"),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.validation_status(name_path(), "manual_required"),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.form_validation_status("initial_form"),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                ValidationTarget::Field(name_path().identity()),
                "initial_required",
                "required",
            ),
            (
                ValidationTarget::Form,
                "initial_form",
                "initial_form_invalid"
            ),
        ]
    );
    assert!(form.visible_validation_errors().is_empty());

    form.mark_field_blurred_without_validation(name_path());

    let visible_field_errors: Vec<_> = form
        .visible_field_validation_errors(name_path())
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(visible_field_errors, vec![("initial_required", "required")]);
    assert!(form.visible_form_validation_errors().is_empty());

    form.mark_submit_attempt();

    let visible_form_errors: Vec<_> = form
        .visible_form_validation_errors()
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        visible_form_errors,
        vec![("initial_form", "initial_form_invalid")]
    );
}

#[test]
fn sync_field_validation_flattens_multiple_validators_and_errors_deterministically() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator(name_path(), "required", |value, _context| {
        if value.is_empty() {
            vec!["required", "blank"]
        } else {
            Vec::new()
        }
    });
    form.register_sync_field_validator(name_path(), "length", |value, _context| {
        if value.len() < 3 {
            vec!["too_short"]
        } else {
            Vec::new()
        }
    });

    form.validate_field(name_path(), ValidationTrigger::Manual);

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.field().unwrap().as_str().to_owned(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            ("name".to_owned(), "required", "required"),
            ("name".to_owned(), "required", "blank"),
            ("name".to_owned(), "length", "too_short"),
        ]
    );

    let statuses: Vec<_> = form
        .field_validation_statuses(name_path())
        .into_iter()
        .map(|status| (status.source().as_str().to_owned(), status.status()))
        .collect();
    assert_eq!(
        statuses,
        vec![
            ("required".to_owned(), ValidationStatus::Invalid),
            ("length".to_owned(), ValidationStatus::Invalid),
        ]
    );
}

#[test]
fn field_validator_views_follow_registration_order_across_fields() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: String::new(),
            password: String::new(),
            confirm_password: String::new(),
        });

    let password_rule =
        form.register_sync_field_validator(password_path(), "password", |value, _context| {
            if value.is_empty() {
                vec!["password_required"]
            } else {
                Vec::new()
            }
        });
    let email_rule =
        form.register_sync_field_validator(email_path(), "email", |value, _context| {
            if value.is_empty() {
                vec!["email_required"]
            } else {
                Vec::new()
            }
        });

    assert!(password_rule.as_u64() < email_rule.as_u64());

    form.validate_all(ValidationTrigger::Manual);

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.field().unwrap().as_str().to_owned(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                Some(password_rule),
                "password".to_owned(),
                "password",
                "password_required",
            ),
            (
                Some(email_rule),
                "email".to_owned(),
                "email",
                "email_required"
            ),
        ]
    );

    let statuses: Vec<_> = form
        .validation_statuses()
        .into_iter()
        .map(|status| {
            (
                status.validator_id(),
                status.source().as_str().to_owned(),
                status.status(),
            )
        })
        .collect();
    assert_eq!(
        statuses,
        vec![
            (
                password_rule,
                "password".to_owned(),
                ValidationStatus::Invalid,
            ),
            (email_rule, "email".to_owned(), ValidationStatus::Invalid),
        ]
    );
}

#[test]
fn duplicate_validator_labels_coexist_with_stable_registration_ids() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    let first = form.register_sync_field_validator(name_path(), "name", |_value, context| {
        assert_eq!(context.source().as_str(), "name");
        vec!["first"]
    });
    let second = form.register_sync_field_validator(name_path(), "name", |_value, context| {
        assert_eq!(context.source().as_str(), "name");
        vec!["second"]
    });
    let form_rule = form.register_sync_form_validator("name", |context| {
        assert_eq!(context.source().as_str(), "name");
        vec![FormValidationError::form("form")]
    });

    assert_ne!(first, second);
    assert_ne!(first, form_rule);
    assert!(first.as_u64() < second.as_u64());
    assert!(second.as_u64() < form_rule.as_u64());

    form.validate_all(ValidationTrigger::Manual);

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (Some(first), "name", "first"),
            (Some(second), "name", "second"),
            (Some(form_rule), "name", "form"),
        ]
    );
    assert_eq!(
        form.field_validation_status(name_path(), first),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.field_validation_status(name_path(), second),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.validation_status(name_path(), "name"),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.form_validation_status_by_id(form_rule),
        Some(ValidationStatus::Invalid)
    );
}

#[test]
fn flattened_status_views_use_category_and_registration_order_with_duplicate_labels() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: String::new(),
            password: String::new(),
            confirm_password: String::new(),
        });

    let form_first = form.register_sync_form_validator("shared", |_context| Vec::new());
    let password =
        form.register_sync_field_validator(password_path(), "shared", |_value, _context| {
            Vec::new()
        });
    let email =
        form.register_sync_field_validator(email_path(), "shared", |_value, _context| Vec::new());
    let form_second = form.register_sync_form_validator("shared", |_context| Vec::new());

    assert!(form_first.as_u64() < password.as_u64());
    assert!(password.as_u64() < email.as_u64());
    assert!(email.as_u64() < form_second.as_u64());

    let statuses: Vec<_> = form
        .validation_statuses()
        .into_iter()
        .map(|status| {
            (
                status.target(),
                status.validator_id(),
                status.source().as_str().to_owned(),
                status.status(),
            )
        })
        .collect();
    assert_eq!(
        statuses,
        vec![
            (
                ValidationTarget::Field(password_path().identity()),
                password,
                "shared".to_owned(),
                ValidationStatus::Unknown,
            ),
            (
                ValidationTarget::Field(email_path().identity()),
                email,
                "shared".to_owned(),
                ValidationStatus::Unknown,
            ),
            (
                ValidationTarget::Form,
                form_first,
                "shared".to_owned(),
                ValidationStatus::Unknown,
            ),
            (
                ValidationTarget::Form,
                form_second,
                "shared".to_owned(),
                ValidationStatus::Unknown,
            ),
        ]
    );
}

#[test]
fn flattened_error_views_use_source_category_order_across_validation_and_submit() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: String::new(),
            password: String::new(),
            confirm_password: String::new(),
        });

    let form_rule = form.register_sync_form_validator_for_triggers(
        "account",
        ValidationTrigger::Manual,
        |_context| {
            vec![
                FormValidationError::field(confirm_password_path(), "password_mismatch"),
                FormValidationError::form("account_unavailable"),
            ]
        },
    );
    let password_rule = form.register_sync_field_validator_for_triggers(
        password_path(),
        "required",
        ValidationTrigger::Manual,
        |_value, _context| vec!["password_required", "password_blank"],
    );
    let email_required_rule = form.register_sync_field_validator_for_triggers(
        email_path(),
        "required",
        ValidationTrigger::Manual,
        |_value, _context| vec!["email_required"],
    );
    let email_format_rule = form.register_sync_field_validator_for_triggers(
        email_path(),
        "format",
        ValidationTrigger::Manual,
        |_value, _context| vec!["email_format"],
    );
    let policy_rule = form.register_sync_form_validator_for_triggers(
        "policy",
        ValidationTrigger::Manual,
        |_context| vec![FormValidationError::field(email_path(), "email_domain")],
    );

    assert!(form_rule.as_u64() < password_rule.as_u64());
    assert!(password_rule.as_u64() < email_required_rule.as_u64());
    assert!(email_required_rule.as_u64() < email_format_rule.as_u64());
    assert!(email_format_rule.as_u64() < policy_rule.as_u64());

    form.validate_all(ValidationTrigger::Manual);

    assert_eq!(
        form.submit(|_submitted| {
            SubmitErrors::with_source(
                "server",
                [
                    SubmitError::field(confirm_password_path(), "server_confirm"),
                    SubmitError::field(email_path(), "server_email"),
                    SubmitError::form("server_form"),
                ],
            )
        }),
        SubmitResult::Rejected
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.target(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                Some(password_rule),
                ValidationTarget::Field(password_path().identity()),
                "required",
                "password_required",
            ),
            (
                Some(password_rule),
                ValidationTarget::Field(password_path().identity()),
                "required",
                "password_blank",
            ),
            (
                Some(email_required_rule),
                ValidationTarget::Field(email_path().identity()),
                "required",
                "email_required",
            ),
            (
                Some(email_format_rule),
                ValidationTarget::Field(email_path().identity()),
                "format",
                "email_format",
            ),
            (
                Some(form_rule),
                ValidationTarget::Field(confirm_password_path().identity()),
                "account",
                "password_mismatch",
            ),
            (
                Some(form_rule),
                ValidationTarget::Form,
                "account",
                "account_unavailable",
            ),
            (
                Some(policy_rule),
                ValidationTarget::Field(email_path().identity()),
                "policy",
                "email_domain",
            ),
            (
                None,
                ValidationTarget::Field(confirm_password_path().identity()),
                "server",
                "server_confirm",
            ),
            (
                None,
                ValidationTarget::Field(email_path().identity()),
                "server",
                "server_email",
            ),
            (None, ValidationTarget::Form, "server", "server_form"),
        ]
    );

    let visible_errors: Vec<_> = form
        .visible_validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.target(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(visible_errors, errors);

    let email_errors: Vec<_> = form
        .field_validation_errors(email_path())
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        email_errors,
        vec![
            (Some(email_required_rule), "required", "email_required"),
            (Some(email_format_rule), "format", "email_format"),
            (Some(policy_rule), "policy", "email_domain"),
            (None, "server", "server_email"),
        ]
    );

    let visible_email_errors: Vec<_> = form
        .visible_field_validation_errors(email_path())
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(visible_email_errors, email_errors);

    let form_errors: Vec<_> = form
        .form_validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        form_errors,
        vec![
            (Some(form_rule), "account", "account_unavailable"),
            (None, "server", "server_form"),
        ]
    );

    let visible_form_errors: Vec<_> = form
        .visible_form_validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(visible_form_errors, form_errors);
}

#[test]
fn optional_validator_adapters_support_zero_or_one_error_rules() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "different".to_owned(),
        });

    let email_rule = form.register_sync_field_validator_optional(
        email_path(),
        "email_required",
        |value, context| {
            assert_eq!(context.source().as_str(), "email_required");

            value.is_empty().then_some("email_required")
        },
    );
    let passwords_rule = form.register_sync_form_validator_optional("passwords_match", |context| {
        assert_eq!(context.source().as_str(), "passwords_match");

        (context.form().password != context.form().confirm_password)
            .then(|| FormValidationError::field(confirm_password_path(), "password_mismatch"))
    });

    form.validate_all(ValidationTrigger::Manual);

    assert_eq!(
        form.field_validation_status(email_path(), email_rule),
        Some(ValidationStatus::Valid)
    );
    assert_eq!(
        form.form_validation_status_by_id(passwords_rule),
        Some(ValidationStatus::Invalid)
    );
    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.field().unwrap().as_str().to_owned(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![(
            Some(passwords_rule),
            "confirm_password".to_owned(),
            "password_mismatch"
        )]
    );
}

#[test]
fn rerunning_one_validator_source_replaces_only_that_sources_errors() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator(name_path(), "required", |value, _context| {
        if value.is_empty() {
            vec!["required"]
        } else {
            Vec::new()
        }
    });
    form.register_sync_field_validator(name_path(), "reserved", |_value, _context| {
        vec!["reserved"]
    });

    form.validate_field(name_path(), ValidationTrigger::Manual);
    form.set_user_field(name_path(), "Ada".to_owned());

    assert_eq!(
        form.validate_field_source(name_path(), "required", ValidationTrigger::Manual),
        Some(ValidationStatus::Valid)
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(errors, vec![("reserved", "reserved")]);
    assert_eq!(
        form.validation_status(name_path(), "required"),
        Some(ValidationStatus::Valid)
    );
    assert_eq!(
        form.validation_status(name_path(), "reserved"),
        Some(ValidationStatus::Invalid)
    );
}

#[test]
fn async_field_validation_moves_from_pending_to_valid_without_clearing_unrelated_errors() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Manual,
    );
    form.register_sync_field_validator_for_triggers(
        name_path(),
        "reserved",
        ValidationTrigger::Blur,
        |_value, _context| vec!["reserved"],
    );
    form.validate_field_source(name_path(), "reserved", ValidationTrigger::Blur);

    let run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("async validator should start");

    assert_eq!(run.form_snapshot().value().name, "Ada");
    assert_eq!(run.field_value(), "Ada");
    assert_eq!(run.source().as_str(), "availability");
    assert_eq!(run.trigger(), ValidationTrigger::Manual);
    assert_eq!(run.validator_id(), availability);
    assert_eq!(run.field_identity(), name_path().identity());
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(
        form.submit_availability().blockers(),
        &[SubmitBlocker::ValidationErrors]
    );

    assert_eq!(
        form.complete_async_field_validation(name_path(), availability, &run, Vec::<&str>::new()),
        Some(ValidationStatus::Valid)
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(errors, vec![("reserved", "reserved")]);
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Valid)
    );
}

#[test]
fn field_validation_chain_runs_sync_before_async_when_sync_passes() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&events);
    let sync_runs = Rc::new(Cell::new(0));
    let sync_validator_runs = Rc::clone(&sync_runs);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: String::new(),
            confirm_password: String::new(),
        });

    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));
    let availability = form.register_async_field_validator_for_triggers(
        email_path(),
        "availability",
        ValidationTrigger::Manual,
    );
    let format = form.register_sync_field_validator_for_triggers(
        email_path(),
        "format",
        ValidationTrigger::Manual,
        move |value, _context| {
            sync_validator_runs.set(sync_validator_runs.get() + 1);

            if value.contains('@') {
                Vec::new()
            } else {
                vec!["email_format"]
            }
        },
    );

    let run = form
        .begin_async_field_validation(email_path(), availability, ValidationTrigger::Manual)
        .expect("async validator should start after sync validators pass");

    assert_eq!(sync_runs.get(), 1);
    assert_eq!(run.validator_id(), availability);
    assert_eq!(
        form.field_validation_status(email_path(), format),
        Some(ValidationStatus::Valid)
    );
    assert_eq!(
        form.field_validation_status(email_path(), availability),
        Some(ValidationStatus::Pending)
    );

    let events = events.borrow();
    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[0],
        FormObserverEvent::ValidationRan {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Valid,
            ..
        } if field.as_str() == "email" && source.as_str() == "format"
    ));
    assert!(matches!(
        &events[1],
        FormObserverEvent::AsyncValidationScheduled {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Pending,
            ..
        } if field.as_str() == "email" && source.as_str() == "availability"
    ));

    let statuses: Vec<_> = form
        .field_validation_statuses(email_path())
        .into_iter()
        .map(|status| (status.validator_id(), status.status()))
        .collect();
    assert_eq!(
        statuses,
        vec![
            (availability, ValidationStatus::Pending),
            (format, ValidationStatus::Valid),
        ]
    );
}

#[test]
fn field_validation_chain_skips_async_when_sync_fails_and_clears_only_skipped_errors() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "taken@example.com".to_owned(),
            password: String::new(),
            confirm_password: String::new(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        email_path(),
        "availability",
        ValidationTrigger::Manual,
    );
    let password_required = form.register_sync_field_validator_for_triggers(
        password_path(),
        "password_required",
        ValidationTrigger::Manual,
        |value, _context| {
            if value.is_empty() {
                vec!["password_required"]
            } else {
                Vec::new()
            }
        },
    );
    let required = form.register_sync_field_validator_for_triggers(
        email_path(),
        "required",
        ValidationTrigger::Manual,
        |value, _context| {
            if value.is_empty() {
                vec!["email_required"]
            } else {
                Vec::new()
            }
        },
    );

    let run = form
        .begin_async_field_validation(email_path(), availability, ValidationTrigger::Manual)
        .expect("async validator should start while sync validators pass");
    assert_eq!(
        form.complete_async_field_validation(
            email_path(),
            availability,
            &run,
            ["email_unavailable"],
        ),
        Some(ValidationStatus::Invalid)
    );
    form.validate_field_validator(
        password_path(),
        password_required,
        ValidationTrigger::Manual,
    );
    form.set_field(email_path(), String::new());

    assert!(
        form.begin_async_field_validation(email_path(), availability, ValidationTrigger::Manual)
            .is_none()
    );

    assert_eq!(
        form.field_validation_status(email_path(), availability),
        Some(ValidationStatus::Skipped)
    );
    assert_eq!(
        form.field_validation_status(email_path(), required),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                Some(password_required),
                "password_required",
                "password_required"
            ),
            (Some(required), "required", "email_required"),
        ]
    );
}

#[test]
fn async_field_validation_moves_from_pending_to_invalid_with_deterministic_errors() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Grace".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Manual,
    );

    let run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("async validator should start");

    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );

    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &run,
            ["unavailable", "reserved"],
        ),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .field_validation_errors(name_path())
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (Some(availability), "availability", "unavailable"),
            (Some(availability), "availability", "reserved"),
        ]
    );
    assert_eq!(
        form.submit_availability().blockers(),
        &[SubmitBlocker::ValidationErrors]
    );

    form.mark_field_blurred(name_path());

    let visible_errors: Vec<_> = form
        .visible_field_validation_errors(name_path())
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        visible_errors,
        vec![
            ("availability", "unavailable"),
            ("availability", "reserved"),
        ]
    );
}

#[test]
fn direct_async_field_validation_also_runs_same_trigger_form_validators() {
    let form_runs = Rc::new(Cell::new(0));
    let form_validator_runs = Rc::clone(&form_runs);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "different".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        email_path(),
        "availability",
        ValidationTrigger::Manual,
    );
    let passwords = form.register_sync_form_validator_for_triggers(
        "passwords_match",
        ValidationTrigger::Manual,
        move |context| {
            form_validator_runs.set(form_validator_runs.get() + 1);

            if context.form().password == context.form().confirm_password {
                Vec::new()
            } else {
                vec![FormValidationError::field(
                    confirm_password_path(),
                    "password_mismatch",
                )]
            }
        },
    );

    let run = form
        .begin_async_field_validation(email_path(), availability, ValidationTrigger::Manual)
        .expect("async field validator should start");

    assert_eq!(run.validator_id(), availability);
    assert_eq!(form_runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(passwords),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.field_validation_status(email_path(), availability),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(
        form.field_validation_errors(confirm_password_path())[0].error(),
        &"password_mismatch"
    );
}

#[test]
fn direct_debounced_async_field_validation_also_runs_same_trigger_form_validators() {
    let form_runs = Rc::new(Cell::new(0));
    let form_validator_runs = Rc::clone(&form_runs);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "different".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        email_path(),
        "availability",
        ValidationTrigger::Change,
    );
    let passwords = form.register_sync_form_validator_for_triggers(
        "passwords_match",
        ValidationTrigger::Change,
        move |context| {
            form_validator_runs.set(form_validator_runs.get() + 1);

            if context.form().password == context.form().confirm_password {
                Vec::new()
            } else {
                vec![FormValidationError::field(
                    confirm_password_path(),
                    "password_mismatch",
                )]
            }
        },
    );

    let scheduled = form
        .schedule_debounced_async_field_validation(
            email_path(),
            availability,
            ValidationTrigger::Change,
        )
        .expect("debounced async field validator should schedule");

    assert_eq!(scheduled.validator_id(), availability);
    assert_eq!(form_runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(passwords),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.field_validation_status(email_path(), availability),
        Some(ValidationStatus::Pending)
    );
}

#[test]
fn direct_async_field_validation_runs_same_trigger_form_validators_when_sync_fails() {
    let form_runs = Rc::new(Cell::new(0));
    let form_validator_runs = Rc::clone(&form_runs);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: String::new(),
            password: "secret".to_owned(),
            confirm_password: "different".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        email_path(),
        "availability",
        ValidationTrigger::Manual,
    );
    let required = form.register_sync_field_validator_for_triggers(
        email_path(),
        "required",
        ValidationTrigger::Manual,
        |value, _context| {
            if value.is_empty() {
                vec!["email_required"]
            } else {
                Vec::new()
            }
        },
    );
    let passwords = form.register_sync_form_validator_for_triggers(
        "passwords_match",
        ValidationTrigger::Manual,
        move |context| {
            form_validator_runs.set(form_validator_runs.get() + 1);

            if context.form().password == context.form().confirm_password {
                Vec::new()
            } else {
                vec![FormValidationError::field(
                    confirm_password_path(),
                    "password_mismatch",
                )]
            }
        },
    );

    assert!(
        form.begin_async_field_validation(email_path(), availability, ValidationTrigger::Manual)
            .is_none()
    );

    assert_eq!(form_runs.get(), 1);
    assert_eq!(
        form.field_validation_status(email_path(), availability),
        Some(ValidationStatus::Skipped)
    );
    assert_eq!(
        form.field_validation_status(email_path(), required),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.form_validation_status_by_id(passwords),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.target(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                Some(required),
                ValidationTarget::Field(email_path().identity()),
                "required",
                "email_required",
            ),
            (
                Some(passwords),
                ValidationTarget::Field(confirm_password_path().identity()),
                "passwords_match",
                "password_mismatch",
            ),
        ]
    );
}

#[test]
fn direct_debounced_async_field_validation_runs_same_trigger_form_validators_when_sync_fails() {
    let form_runs = Rc::new(Cell::new(0));
    let form_validator_runs = Rc::clone(&form_runs);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: String::new(),
            password: "secret".to_owned(),
            confirm_password: "different".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        email_path(),
        "availability",
        ValidationTrigger::Change,
    );
    let required = form.register_sync_field_validator_for_triggers(
        email_path(),
        "required",
        ValidationTrigger::Change,
        |value, _context| {
            if value.is_empty() {
                vec!["email_required"]
            } else {
                Vec::new()
            }
        },
    );
    let passwords = form.register_sync_form_validator_for_triggers(
        "passwords_match",
        ValidationTrigger::Change,
        move |context| {
            form_validator_runs.set(form_validator_runs.get() + 1);

            if context.form().password == context.form().confirm_password {
                Vec::new()
            } else {
                vec![FormValidationError::field(
                    confirm_password_path(),
                    "password_mismatch",
                )]
            }
        },
    );

    assert!(
        form.schedule_debounced_async_field_validation(
            email_path(),
            availability,
            ValidationTrigger::Change,
        )
        .is_none()
    );

    assert_eq!(form_runs.get(), 1);
    assert_eq!(
        form.field_validation_status(email_path(), availability),
        Some(ValidationStatus::Skipped)
    );
    assert_eq!(
        form.field_validation_status(email_path(), required),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.form_validation_status_by_id(passwords),
        Some(ValidationStatus::Invalid)
    );
}

#[test]
fn duplicate_async_field_sources_keep_independent_runs_and_deterministic_order() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let first = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Manual,
    );
    let second = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Manual,
    );

    let first_run = form
        .begin_async_field_validation(name_path(), first, ValidationTrigger::Manual)
        .expect("first async validator should start");
    let second_run = form
        .begin_async_field_validation(name_path(), second, ValidationTrigger::Manual)
        .expect("second async validator should start");

    assert_eq!(
        form.complete_async_field_validation(name_path(), second, &second_run, ["second"]),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.complete_async_field_validation(name_path(), first, &first_run, ["first"]),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .field_validation_errors(name_path())
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (Some(first), "availability", "first"),
            (Some(second), "availability", "second"),
        ]
    );
}

#[test]
fn stale_async_field_validation_completion_after_edit_does_not_replace_newer_result() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "first".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Manual,
    );

    let first_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("first async validator should start");

    form.set_user_field(name_path(), "second".to_owned());

    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Stale)
    );
    assert!(form.field_validation_errors(name_path()).is_empty());
    assert!(form.visible_field_validation_errors(name_path()).is_empty());

    let second_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("second async validator should start");

    assert_eq!(second_run.field_value(), "second");
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );

    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &second_run,
            ["second_unavailable"],
        ),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .field_validation_errors(name_path())
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(errors, vec![("availability", "second_unavailable")]);

    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &first_run,
            ["first_unavailable"],
        ),
        None
    );

    let errors: Vec<_> = form
        .field_validation_errors(name_path())
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(errors, vec![("availability", "second_unavailable")]);
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Invalid)
    );
}

#[test]
fn duplicate_async_field_completion_is_ignored_after_first_result_applies() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Manual,
    );

    let run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("async validator should start");

    assert_eq!(
        form.complete_async_field_validation(name_path(), availability, &run, Vec::<&str>::new()),
        Some(ValidationStatus::Valid)
    );
    assert_eq!(
        form.complete_async_field_validation(name_path(), availability, &run, ["duplicate"]),
        None
    );
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Valid)
    );
    assert!(form.field_validation_errors(name_path()).is_empty());
}

#[test]
fn duplicate_async_form_completion_is_ignored_after_first_result_applies() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Manual);

    let run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start");

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &run,
            Vec::<FormValidationError<&str>>::new(),
        ),
        Some(ValidationStatus::Valid)
    );
    assert_eq!(
        form.complete_async_form_validation(
            account,
            &run,
            [FormValidationError::form("duplicate")],
        ),
        None
    );
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Valid)
    );
    assert!(form.form_validation_errors().is_empty());
}

#[test]
fn stale_async_field_validation_completion_after_context_edit_does_not_apply() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "first@example.com".to_owned(),
            password: "old-secret".to_owned(),
            confirm_password: "old-secret".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        email_path(),
        "availability",
        ValidationTrigger::Manual,
    );

    let run = form
        .begin_async_field_validation(email_path(), availability, ValidationTrigger::Manual)
        .expect("async validator should start");

    assert_eq!(run.field_value(), "first@example.com");
    assert_eq!(run.form_snapshot().value().password, "old-secret");

    form.set_field(password_path(), "new-secret".to_owned());

    assert_eq!(
        form.field_validation_status(email_path(), availability),
        Some(ValidationStatus::Stale)
    );
    assert!(form.field_validation_errors(email_path()).is_empty());

    assert_eq!(
        form.complete_async_field_validation(
            email_path(),
            availability,
            &run,
            ["first_unavailable"],
        ),
        None
    );

    assert!(form.field_validation_errors(email_path()).is_empty());
    assert_eq!(
        form.field_validation_status(email_path(), availability),
        Some(ValidationStatus::Stale)
    );
}

#[test]
fn observer_events_cover_async_lifecycle_without_values() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&events);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "start".to_owned(),
        });

    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Change]),
    );

    let run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("async validator should start");
    assert_eq!(
        form.complete_async_field_validation(name_path(), availability, &run, Vec::<&str>::new()),
        Some(ValidationStatus::Valid)
    );

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTrigger::Manual,
        |value, _context| {
            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );
    form.set_field(name_path(), String::new());
    assert!(
        form.begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
            .is_none()
    );

    form.set_field(name_path(), "old".to_owned());
    let stale_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("async validator should start after sync validators pass");
    form.set_field(name_path(), "new".to_owned());
    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &stale_run,
            ["old_unavailable"],
        ),
        None
    );

    let scheduled = form
        .schedule_debounced_async_field_validation(
            name_path(),
            availability,
            ValidationTrigger::Change,
        )
        .expect("debounced async validator should schedule");
    let debounced_run = form
        .begin_debounced_async_field_validation(name_path(), availability, &scheduled)
        .expect("debounced async validator should flush");
    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &debounced_run,
            Vec::<&str>::new(),
        ),
        Some(ValidationStatus::Valid)
    );

    let events = events.borrow();
    let debug_output = format!("{events:?}");

    assert!(events.iter().any(|event| matches!(
        event,
        FormObserverEvent::AsyncValidationCompleted {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Valid,
            ..
        } if field.as_str() == "name" && source.as_str() == "availability"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        FormObserverEvent::AsyncValidationSkipped {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Skipped,
            ..
        } if field.as_str() == "name" && source.as_str() == "availability"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        FormObserverEvent::AsyncValidationStaleIgnored {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Stale,
            ..
        } if field.as_str() == "name" && source.as_str() == "availability"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        FormObserverEvent::DebouncedAsyncValidationScheduled {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Change,
            status: ValidationStatus::Pending,
            ..
        } if field.as_str() == "name" && source.as_str() == "availability"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        FormObserverEvent::DebouncedAsyncValidationFlushed {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Change,
            status: ValidationStatus::Pending,
            ..
        } if field.as_str() == "name" && source.as_str() == "availability"
    )));
    assert!(!debug_output.contains("old"));
    assert!(!debug_output.contains("new"));
    assert!(!debug_output.contains("start"));
}

#[test]
fn observer_events_cover_async_form_lifecycle_without_values() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&events);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "start@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });

    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));
    let account = form.register_async_form_validator_for_triggers(
        "account",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Change]),
    );

    let run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start");
    assert_eq!(
        form.complete_async_form_validation(
            account,
            &run,
            Vec::<FormValidationError<&str>>::new(),
        ),
        Some(ValidationStatus::Valid)
    );

    form.register_sync_form_validator_for_triggers(
        "required",
        ValidationTrigger::Manual,
        |context| {
            if context.form().email.is_empty() {
                vec![FormValidationError::form("required")]
            } else {
                Vec::new()
            }
        },
    );
    form.set_field(email_path(), String::new());
    assert!(
        form.begin_async_form_validation(account, ValidationTrigger::Manual)
            .is_none()
    );

    form.set_field(email_path(), "old@example.com".to_owned());
    let stale_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start after sync validators pass");
    form.set_field(email_path(), "new@example.com".to_owned());
    assert_eq!(
        form.complete_async_form_validation(
            account,
            &stale_run,
            [FormValidationError::form("old_unavailable")],
        ),
        None
    );

    let scheduled = form
        .schedule_debounced_async_form_validation(account, ValidationTrigger::Change)
        .expect("debounced async form validator should schedule");
    let debounced_run = form
        .begin_debounced_async_form_validation(account, &scheduled)
        .expect("debounced async form validator should flush");
    assert_eq!(
        form.complete_async_form_validation(
            account,
            &debounced_run,
            Vec::<FormValidationError<&str>>::new(),
        ),
        Some(ValidationStatus::Valid)
    );

    let events = events.borrow();
    let debug_output = format!("{events:?}");

    assert!(events.iter().any(|event| matches!(
        event,
        FormObserverEvent::AsyncValidationCompleted {
            target: ValidationTarget::Form,
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Valid,
            ..
        } if source.as_str() == "account"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        FormObserverEvent::AsyncValidationSkipped {
            target: ValidationTarget::Form,
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Skipped,
            ..
        } if source.as_str() == "account"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        FormObserverEvent::AsyncValidationStaleIgnored {
            target: ValidationTarget::Form,
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Stale,
            ..
        } if source.as_str() == "account"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        FormObserverEvent::DebouncedAsyncValidationScheduled {
            target: ValidationTarget::Form,
            source,
            trigger: ValidationTrigger::Change,
            status: ValidationStatus::Pending,
            ..
        } if source.as_str() == "account"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        FormObserverEvent::DebouncedAsyncValidationFlushed {
            target: ValidationTarget::Form,
            source,
            trigger: ValidationTrigger::Change,
            status: ValidationStatus::Pending,
            ..
        } if source.as_str() == "account"
    )));
    assert!(!debug_output.contains("old@example.com"));
    assert!(!debug_output.contains("new@example.com"));
    assert!(!debug_output.contains("start@example.com"));
}

#[test]
fn debounced_async_field_validation_marks_pending_until_latest_value_starts() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Change,
    );

    let first = form
        .schedule_debounced_async_field_validation(
            name_path(),
            availability,
            ValidationTrigger::Change,
        )
        .expect("debounced validator should schedule");

    assert_eq!(first.field_identity(), name_path().identity());
    assert_eq!(first.validator_id(), availability);
    assert_eq!(first.source().as_str(), "availability");
    assert_eq!(first.trigger(), ValidationTrigger::Change);
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(form.submit_availability().blockers(), &[]);

    form.set_user_field(name_path(), "Grace".to_owned());
    let second = form
        .schedule_debounced_async_field_validation(
            name_path(),
            availability,
            ValidationTrigger::Change,
        )
        .expect("latest debounced validator should schedule");

    assert!(
        form.begin_debounced_async_field_validation(name_path(), availability, &first)
            .is_none()
    );

    let run = form
        .begin_debounced_async_field_validation(name_path(), availability, &second)
        .expect("latest debounced validator should start after delay");

    assert!(
        form.begin_debounced_async_field_validation(name_path(), availability, &second)
            .is_none()
    );

    assert_eq!(run.field_value(), "Grace");
    assert_eq!(run.form_snapshot().value().name, "Grace");
    assert_eq!(run.trigger(), ValidationTrigger::Change);

    assert_eq!(
        form.complete_async_field_validation(name_path(), availability, &run, ["unavailable"]),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.field_validation_errors(name_path())[0].error(),
        &"unavailable"
    );
    assert!(form.visible_field_validation_errors(name_path()).is_empty());
}

#[test]
fn debounced_async_form_validation_marks_pending_until_latest_snapshot_starts() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "first@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Change);

    let first = form
        .schedule_debounced_async_form_validation(account, ValidationTrigger::Change)
        .expect("debounced form validator should schedule");

    assert_eq!(first.validator_id(), account);
    assert_eq!(first.source().as_str(), "account");
    assert_eq!(first.trigger(), ValidationTrigger::Change);
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(form.submit_availability().blockers(), &[]);

    form.set_user_field(email_path(), "second@example.com".to_owned());
    let second = form
        .schedule_debounced_async_form_validation(account, ValidationTrigger::Change)
        .expect("latest debounced form validator should schedule");

    assert!(
        form.begin_debounced_async_form_validation(account, &first)
            .is_none()
    );

    let run = form
        .begin_debounced_async_form_validation(account, &second)
        .expect("latest debounced form validator should start after delay");

    assert!(
        form.begin_debounced_async_form_validation(account, &second)
            .is_none()
    );

    assert_eq!(run.form_snapshot().value().email, "second@example.com");
    assert_eq!(run.trigger(), ValidationTrigger::Change);

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &run,
            [
                FormValidationError::field(email_path(), "email_unavailable"),
                FormValidationError::form("account_unavailable"),
            ],
        ),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                ValidationTarget::Field(email_path().identity()),
                "account",
                "email_unavailable",
            ),
            (ValidationTarget::Form, "account", "account_unavailable"),
        ]
    );
    assert!(form.visible_validation_errors().is_empty());
}

#[test]
fn core_debounced_async_validation_only_schedules_value_change_triggers() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Change]),
    );
    let account = form.register_async_form_validator_for_triggers(
        "account",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Change]),
    );

    assert!(
        form.schedule_debounced_async_field_validation(
            name_path(),
            availability,
            ValidationTrigger::Manual,
        )
        .is_none()
    );
    assert!(
        form.schedule_debounced_async_form_validation(account, ValidationTrigger::Manual)
            .is_none()
    );
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Unknown)
    );

    assert!(
        form.schedule_debounced_async_field_validation(
            name_path(),
            availability,
            ValidationTrigger::Change,
        )
        .is_some()
    );
    assert!(
        form.schedule_debounced_async_form_validation(account, ValidationTrigger::Change)
            .is_some()
    );
}

#[test]
fn core_flushes_only_submit_relevant_debounced_field_validation() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let advisory = form.register_async_field_validator_for_triggers(
        name_path(),
        "advisory",
        ValidationTrigger::Change,
    );
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );

    form.schedule_debounced_async_field_validation(
        name_path(),
        advisory,
        ValidationTrigger::Change,
    )
    .expect("advisory debounced validation should schedule");
    form.schedule_debounced_async_field_validation(
        name_path(),
        availability,
        ValidationTrigger::Change,
    )
    .expect("submit-relevant debounced validation should schedule");

    let target = ValidationTarget::Field(name_path().identity());

    assert!(!form.should_flush_debounced_validation_for_submit(&target, advisory));
    assert!(form.should_flush_debounced_validation_for_submit(&target, availability));
    assert_eq!(
        form.submit_availability().blockers(),
        &[SubmitBlocker::PendingValidation]
    );
}

#[test]
fn core_flushes_debounced_field_validation_as_submit_triggered_work() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );
    let scheduled = form
        .schedule_debounced_async_field_validation(
            name_path(),
            availability,
            ValidationTrigger::Change,
        )
        .expect("debounced validation should schedule");

    let run = form
        .flush_debounced_async_field_validation_for_trigger(
            name_path(),
            availability,
            &scheduled,
            ValidationTrigger::Submit,
        )
        .expect("submit flush should start submit-scoped validation");

    assert_eq!(run.trigger(), ValidationTrigger::Submit);
    assert!(
        form.begin_debounced_async_field_validation(name_path(), availability, &scheduled)
            .is_none()
    );
}

#[test]
fn core_submit_flush_skips_debounced_field_async_when_submit_sync_fails() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );
    let required = form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTrigger::Submit,
        |value, _context| {
            if value.is_empty() {
                vec!["name_required"]
            } else {
                Vec::new()
            }
        },
    );
    let scheduled = form
        .schedule_debounced_async_field_validation(
            name_path(),
            availability,
            ValidationTrigger::Change,
        )
        .expect("value-change debounce should schedule");

    assert!(
        form.flush_debounced_async_field_validation_for_trigger(
            name_path(),
            availability,
            &scheduled,
            ValidationTrigger::Submit,
        )
        .is_none()
    );

    assert_eq!(
        form.field_validation_status(name_path(), required),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Skipped)
    );
    assert_eq!(form.field_validation_errors(name_path()).len(), 1);
    assert_eq!(
        form.field_validation_errors(name_path())[0]
            .source()
            .as_str(),
        "required"
    );
}

#[test]
fn core_flushes_only_submit_relevant_debounced_form_validation() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let advisory =
        form.register_async_form_validator_for_triggers("advisory", ValidationTrigger::Change);
    let account = form.register_async_form_validator_for_triggers(
        "account",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );

    form.schedule_debounced_async_form_validation(advisory, ValidationTrigger::Change)
        .expect("advisory debounced form validation should schedule");
    form.schedule_debounced_async_form_validation(account, ValidationTrigger::Change)
        .expect("submit-relevant debounced form validation should schedule");

    assert!(!form.should_flush_debounced_validation_for_submit(&ValidationTarget::Form, advisory));
    assert!(form.should_flush_debounced_validation_for_submit(&ValidationTarget::Form, account));
    assert_eq!(
        form.submit_availability().blockers(),
        &[SubmitBlocker::PendingValidation]
    );
}

#[test]
fn core_flushes_debounced_form_validation_as_submit_triggered_work() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account = form.register_async_form_validator_for_triggers(
        "account",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );
    let scheduled = form
        .schedule_debounced_async_form_validation(account, ValidationTrigger::Change)
        .expect("debounced form validation should schedule");

    let run = form
        .flush_debounced_async_form_validation_for_trigger(
            account,
            &scheduled,
            ValidationTrigger::Submit,
        )
        .expect("submit flush should start submit-scoped form validation");

    assert_eq!(run.trigger(), ValidationTrigger::Submit);
    assert!(
        form.begin_debounced_async_form_validation(account, &scheduled)
            .is_none()
    );
}

#[test]
fn core_submit_flush_skips_debounced_form_async_when_submit_sync_fails() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "different".to_owned(),
        });
    let account = form.register_async_form_validator_for_triggers(
        "account",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );
    let passwords = form.register_sync_form_validator_for_triggers(
        "passwords_match",
        ValidationTrigger::Submit,
        |context| {
            if context.form().password == context.form().confirm_password {
                Vec::new()
            } else {
                vec![FormValidationError::field(
                    confirm_password_path(),
                    "password_mismatch",
                )]
            }
        },
    );
    let scheduled = form
        .schedule_debounced_async_form_validation(account, ValidationTrigger::Change)
        .expect("value-change debounce should schedule");

    assert!(
        form.flush_debounced_async_form_validation_for_trigger(
            account,
            &scheduled,
            ValidationTrigger::Submit,
        )
        .is_none()
    );

    assert_eq!(
        form.form_validation_status_by_id(passwords),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Skipped)
    );
    assert_eq!(
        form.field_validation_errors(confirm_password_path()).len(),
        1
    );
    assert_eq!(
        form.field_validation_errors(confirm_password_path())[0]
            .source()
            .as_str(),
        "passwords_match"
    );
}

#[test]
fn reset_invalidates_pending_async_field_validation_and_debounced_field_run() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "first".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Change]),
    );

    let stale_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("async validator should start");
    let stale_debounce = form
        .schedule_debounced_async_field_validation(
            name_path(),
            availability,
            ValidationTrigger::Change,
        )
        .expect("debounced validator should schedule");

    form.reset();

    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Unknown)
    );
    assert!(
        form.begin_debounced_async_field_validation(name_path(), availability, &stale_debounce)
            .is_none()
    );

    let fresh_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("fresh async validator should start after reset");

    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &stale_run,
            ["stale_unavailable"],
        ),
        None
    );
    assert!(form.field_validation_errors(name_path()).is_empty());
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );

    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &fresh_run,
            ["fresh_unavailable"],
        ),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.field_validation_errors(name_path())[0].error(),
        &"fresh_unavailable"
    );
}

#[test]
fn reinitialize_invalidates_pending_async_field_validation_and_debounced_field_run() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "first".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Change]),
    );

    let stale_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("async validator should start");
    let stale_debounce = form
        .schedule_debounced_async_field_validation(
            name_path(),
            availability,
            ValidationTrigger::Change,
        )
        .expect("debounced validator should schedule");

    form.reinitialize(ContactForm {
        name: "fresh".to_owned(),
    });

    assert_eq!(form.field_value(name_path()), "fresh");
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Unknown)
    );
    assert!(
        form.begin_debounced_async_field_validation(name_path(), availability, &stale_debounce)
            .is_none()
    );

    let fresh_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("fresh async validator should start after reinitialize");

    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &stale_run,
            ["stale_unavailable"],
        ),
        None
    );
    assert!(form.field_validation_errors(name_path()).is_empty());
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );

    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &fresh_run,
            Vec::<&str>::new()
        ),
        Some(ValidationStatus::Valid)
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn reset_invalidates_pending_async_form_validation_and_debounced_form_run() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "first@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account = form.register_async_form_validator_for_triggers(
        "account",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Change]),
    );

    let stale_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start");
    let stale_debounce = form
        .schedule_debounced_async_form_validation(account, ValidationTrigger::Change)
        .expect("debounced form validator should schedule");

    form.reset();

    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Unknown)
    );
    assert!(
        form.begin_debounced_async_form_validation(account, &stale_debounce)
            .is_none()
    );

    let fresh_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("fresh async form validator should start after reset");

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &stale_run,
            [FormValidationError::form("stale_unavailable")],
        ),
        None
    );
    assert!(form.validation_errors().is_empty());
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Pending)
    );

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &fresh_run,
            [FormValidationError::form("fresh_unavailable")],
        ),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.form_validation_errors()[0].error(),
        &"fresh_unavailable"
    );
}

#[test]
fn reinitialize_invalidates_pending_async_form_validation_and_debounced_form_run() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "first@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account = form.register_async_form_validator_for_triggers(
        "account",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Change]),
    );

    let stale_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start");
    let stale_debounce = form
        .schedule_debounced_async_form_validation(account, ValidationTrigger::Change)
        .expect("debounced form validator should schedule");

    form.reinitialize(RegistrationForm {
        email: "fresh@example.com".to_owned(),
        password: "fresh".to_owned(),
        confirm_password: "fresh".to_owned(),
    });

    assert_eq!(form.field_value(email_path()), "fresh@example.com");
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Unknown)
    );
    assert!(
        form.begin_debounced_async_form_validation(account, &stale_debounce)
            .is_none()
    );

    let fresh_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("fresh async form validator should start after reinitialize");

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &stale_run,
            [FormValidationError::form("stale_unavailable")],
        ),
        None
    );
    assert!(form.validation_errors().is_empty());
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Pending)
    );

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &fresh_run,
            Vec::<FormValidationError<&str>>::new(),
        ),
        Some(ValidationStatus::Valid)
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn reset_and_reinitialize_clear_registered_validator_lifecycle_without_extra_observer_events() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&events);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let required = form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTrigger::Manual,
        |value, _context| {
            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Manual,
    );

    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));

    let pending_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("async validation should start after sync validator passes");

    assert_eq!(
        form.field_validation_status(name_path(), required),
        Some(ValidationStatus::Valid)
    );
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );

    form.reset();

    assert_eq!(
        form.field_validation_status(name_path(), required),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Unknown)
    );
    assert!(form.validation_errors().is_empty());
    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &pending_run,
            ["stale_unavailable"],
        ),
        None
    );

    form.set_user_field(name_path(), String::new());
    form.validate_field(name_path(), ValidationTrigger::Manual);

    assert_eq!(
        form.field_validation_status(name_path(), required),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Skipped)
    );

    form.reinitialize(ContactForm {
        name: "Lin".to_owned(),
    });

    assert_eq!(
        form.field_validation_status(name_path(), required),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Unknown)
    );
    assert!(form.validation_errors().is_empty());

    let events = events.borrow();
    assert_eq!(events.len(), 8);
    assert!(matches!(
        &events[0],
        FormObserverEvent::ValidationRan {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Valid,
            ..
        } if field.as_str() == "name" && source.as_str() == "required"
    ));
    assert!(matches!(
        &events[1],
        FormObserverEvent::AsyncValidationScheduled {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Pending,
            ..
        } if field.as_str() == "name" && source.as_str() == "availability"
    ));
    assert!(matches!(&events[2], FormObserverEvent::Reset { .. }));
    assert!(matches!(
        &events[3],
        FormObserverEvent::AsyncValidationStaleIgnored {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Stale,
            ..
        } if field.as_str() == "name" && source.as_str() == "availability"
    ));
    assert!(matches!(&events[4], FormObserverEvent::FieldUpdated { .. }));
    assert!(matches!(
        &events[5],
        FormObserverEvent::ValidationRan {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Invalid,
            ..
        } if field.as_str() == "name" && source.as_str() == "required"
    ));
    assert!(matches!(
        &events[6],
        FormObserverEvent::AsyncValidationSkipped {
            target: ValidationTarget::Field(field),
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Skipped,
            ..
        } if field.as_str() == "name" && source.as_str() == "availability"
    ));
    assert!(matches!(
        &events[7],
        FormObserverEvent::Reinitialized { .. }
    ));
}

#[test]
fn async_form_validation_moves_from_pending_to_valid_from_owned_snapshot() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Manual);

    let run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start");

    assert_eq!(run.form_snapshot().value().email, "ada@example.com");
    assert_eq!(run.source().as_str(), "account");
    assert_eq!(run.trigger(), ValidationTrigger::Manual);
    assert_eq!(run.validator_id(), account);
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Pending)
    );

    assert_eq!(
        form.complete_async_form_validation(account, &run, Vec::<FormValidationError<&str>>::new()),
        Some(ValidationStatus::Valid)
    );
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Valid)
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn form_validation_chain_runs_sync_before_async_when_sync_passes() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&events);
    let sync_runs = Rc::new(Cell::new(0));
    let sync_validator_runs = Rc::clone(&sync_runs);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });

    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));
    let account =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Manual);
    let passwords = form.register_sync_form_validator_for_triggers(
        "passwords_match",
        ValidationTrigger::Manual,
        move |context| {
            sync_validator_runs.set(sync_validator_runs.get() + 1);

            if context.form().password == context.form().confirm_password {
                Vec::new()
            } else {
                vec![FormValidationError::field(
                    confirm_password_path(),
                    "password_mismatch",
                )]
            }
        },
    );

    let run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start after sync validators pass");

    assert_eq!(sync_runs.get(), 1);
    assert_eq!(run.validator_id(), account);
    assert_eq!(
        form.form_validation_status_by_id(passwords),
        Some(ValidationStatus::Valid)
    );
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Pending)
    );

    let events = events.borrow();
    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[0],
        FormObserverEvent::ValidationRan {
            target: ValidationTarget::Form,
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Valid,
            ..
        } if source.as_str() == "passwords_match"
    ));
    assert!(matches!(
        &events[1],
        FormObserverEvent::AsyncValidationScheduled {
            target: ValidationTarget::Form,
            source,
            trigger: ValidationTrigger::Manual,
            status: ValidationStatus::Pending,
            ..
        } if source.as_str() == "account"
    ));

    let statuses: Vec<_> = form
        .form_validation_statuses()
        .into_iter()
        .map(|status| (status.validator_id(), status.status()))
        .collect();
    assert_eq!(
        statuses,
        vec![
            (account, ValidationStatus::Pending),
            (passwords, ValidationStatus::Valid),
        ]
    );
}

#[test]
fn form_validation_chain_skips_async_when_sync_fails_and_clears_only_skipped_errors() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: String::new(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Manual);
    let email_required = form.register_sync_field_validator_for_triggers(
        email_path(),
        "email_required",
        ValidationTrigger::Manual,
        |value, _context| {
            if value.is_empty() {
                vec!["email_required"]
            } else {
                Vec::new()
            }
        },
    );

    let run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start while sync validators pass");
    assert_eq!(
        form.complete_async_form_validation(
            account,
            &run,
            [FormValidationError::form("account_unavailable")],
        ),
        Some(ValidationStatus::Invalid)
    );
    form.validate_field_validator(email_path(), email_required, ValidationTrigger::Manual);
    form.set_field(confirm_password_path(), "different".to_owned());
    let passwords = form.register_sync_form_validator_for_triggers(
        "passwords_match",
        ValidationTrigger::Manual,
        |context| {
            if context.form().password == context.form().confirm_password {
                Vec::new()
            } else {
                vec![FormValidationError::field(
                    confirm_password_path(),
                    "password_mismatch",
                )]
            }
        },
    );

    assert!(
        form.begin_async_form_validation(account, ValidationTrigger::Manual)
            .is_none()
    );

    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Skipped)
    );
    assert_eq!(
        form.form_validation_status_by_id(passwords),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.target(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                Some(email_required),
                ValidationTarget::Field(email_path().identity()),
                "email_required",
                "email_required",
            ),
            (
                Some(passwords),
                ValidationTarget::Field(confirm_password_path().identity()),
                "passwords_match",
                "password_mismatch",
            ),
        ]
    );
}

#[test]
fn async_form_validation_records_form_level_errors() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "taken@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Manual);

    let run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start");

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &run,
            [FormValidationError::form("account_unavailable")],
        ),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.target(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![(
            Some(account),
            ValidationTarget::Form,
            "account",
            "account_unavailable"
        ),]
    );
}

#[test]
fn async_form_validation_records_field_attached_errors_in_flattened_views() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "different".to_owned(),
        });
    let account =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Manual);

    let run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start");

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &run,
            [FormValidationError::field(
                confirm_password_path(),
                "password_mismatch",
            )],
        ),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .field_validation_errors(confirm_password_path())
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.field().unwrap().as_str().to_owned(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![(
            Some(account),
            "confirm_password".to_owned(),
            "account",
            "password_mismatch"
        )]
    );
}

#[test]
fn stale_async_form_validation_completion_after_edit_does_not_replace_newer_result() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "first@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Manual);

    let first_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("first async form validator should start");

    form.set_user_field(email_path(), "second@example.com".to_owned());

    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Stale)
    );
    assert!(form.validation_errors().is_empty());

    let second_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("second async form validator should start");

    assert_eq!(
        second_run.form_snapshot().value().email,
        "second@example.com"
    );
    assert_eq!(
        form.complete_async_form_validation(
            account,
            &second_run,
            [FormValidationError::form("second_unavailable")],
        ),
        Some(ValidationStatus::Invalid)
    );

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &first_run,
            [FormValidationError::form("first_unavailable")],
        ),
        None
    );

    let errors: Vec<_> = form
        .form_validation_errors()
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(errors, vec![("account", "second_unavailable")]);
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Invalid)
    );
}

#[test]
fn reset_invalidates_pending_async_form_validation() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "first@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Manual);

    let stale_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start");

    form.reset();

    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Unknown)
    );

    let fresh_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("fresh async form validator should start after reset");

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &stale_run,
            [FormValidationError::form("stale_unavailable")],
        ),
        None
    );
    assert!(form.validation_errors().is_empty());
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Pending)
    );

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &fresh_run,
            [FormValidationError::form("fresh_unavailable")],
        ),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.form_validation_errors()[0].error(),
        &"fresh_unavailable"
    );
}

#[test]
fn reinitialize_invalidates_pending_async_form_validation() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "first@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Manual);

    let stale_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("async form validator should start");

    form.reinitialize(RegistrationForm {
        email: "fresh@example.com".to_owned(),
        password: "fresh".to_owned(),
        confirm_password: "fresh".to_owned(),
    });

    assert_eq!(form.field_value(email_path()), "fresh@example.com");
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Unknown)
    );

    let fresh_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("fresh async form validator should start after reinitialize");

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &stale_run,
            [FormValidationError::form("stale_unavailable")],
        ),
        None
    );
    assert!(form.validation_errors().is_empty());
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Pending)
    );

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &fresh_run,
            Vec::<FormValidationError<&str>>::new(),
        ),
        Some(ValidationStatus::Valid)
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn validators_can_be_marked_skipped_by_source() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator(name_path(), "optional", |_value, _context| {
        vec!["not_used"]
    });
    form.validate_field(name_path(), ValidationTrigger::Manual);

    assert!(form.skip_field_validator(name_path(), "optional"));
    assert_eq!(
        form.validation_status(name_path(), "optional"),
        Some(ValidationStatus::Skipped)
    );
    assert!(form.validation_errors().is_empty());
    assert!(!form.skip_field_validator(name_path(), "missing"));
}

#[test]
fn sync_field_validators_run_only_for_registered_triggers() {
    let value_change_runs = Rc::new(Cell::new(0));
    let submit_runs = Rc::new(Cell::new(0));
    let value_change_validator_runs = Rc::clone(&value_change_runs);
    let submit_validator_runs = Rc::clone(&submit_runs);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "change_required",
        ValidationTrigger::Change,
        move |value, context| {
            value_change_validator_runs.set(value_change_validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);

            if value.is_empty() {
                vec!["change_required"]
            } else {
                Vec::new()
            }
        },
    );
    form.register_sync_field_validator_for_triggers(
        name_path(),
        "submit_required",
        ValidationTrigger::Submit,
        move |value, context| {
            submit_validator_runs.set(submit_validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Submit);

            if value.is_empty() {
                vec!["submit_required"]
            } else {
                Vec::new()
            }
        },
    );

    form.validate_field(name_path(), ValidationTrigger::Manual);

    assert_eq!(value_change_runs.get(), 0);
    assert_eq!(submit_runs.get(), 0);
    assert!(form.validation_errors().is_empty());
    assert_eq!(
        form.validation_status(name_path(), "change_required"),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.validation_status(name_path(), "submit_required"),
        Some(ValidationStatus::Unknown)
    );

    form.validate_field(name_path(), ValidationTrigger::Change);

    assert_eq!(value_change_runs.get(), 1);
    assert_eq!(submit_runs.get(), 0);
    assert_eq!(
        form.validation_status(name_path(), "change_required"),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.validation_status(name_path(), "submit_required"),
        Some(ValidationStatus::Unknown)
    );

    form.validate_field(name_path(), ValidationTrigger::Submit);

    assert_eq!(value_change_runs.get(), 1);
    assert_eq!(submit_runs.get(), 1);

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![
            ("change_required", "change_required"),
            ("submit_required", "submit_required"),
        ]
    );
}

#[test]
fn sync_form_validators_run_only_for_registered_triggers() {
    let account_runs = Rc::new(Cell::new(0));
    let blur_runs = Rc::new(Cell::new(0));
    let account_validator_runs = Rc::clone(&account_runs);
    let blur_validator_runs = Rc::clone(&blur_runs);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "taken@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "different".to_owned(),
        });

    form.register_sync_form_validator_for_triggers(
        "account",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Submit]),
        move |context| {
            account_validator_runs.set(account_validator_runs.get() + 1);

            match context.trigger() {
                ValidationTrigger::Manual => vec![FormValidationError::form("manual_account")],
                ValidationTrigger::Submit => vec![FormValidationError::form("submit_account")],
                other => panic!("account validator ran for unexpected trigger: {other:?}"),
            }
        },
    );
    form.register_sync_form_validator_for_triggers(
        "blur_passwords_match",
        ValidationTrigger::Blur,
        move |_context| {
            blur_validator_runs.set(blur_validator_runs.get() + 1);
            vec![FormValidationError::field(
                confirm_password_path(),
                "blur_password_mismatch",
            )]
        },
    );

    form.validate_form(ValidationTrigger::Blur);

    assert_eq!(account_runs.get(), 0);
    assert_eq!(blur_runs.get(), 1);
    assert_eq!(
        form.form_validation_status("account"),
        Some(ValidationStatus::Unknown)
    );

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(account_runs.get(), 1);
    assert_eq!(blur_runs.get(), 1);

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![
            (ValidationTarget::Form, "account", "manual_account"),
            (
                ValidationTarget::Field(confirm_password_path().identity()),
                "blur_passwords_match",
                "blur_password_mismatch",
            ),
        ]
    );

    form.validate_form(ValidationTrigger::Submit);

    assert_eq!(account_runs.get(), 2);
    assert_eq!(blur_runs.get(), 1);

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![
            (ValidationTarget::Form, "account", "submit_account"),
            (
                ValidationTarget::Field(confirm_password_path().identity()),
                "blur_passwords_match",
                "blur_password_mismatch",
            ),
        ]
    );
}

#[test]
fn value_change_validation_runs_only_when_policy_is_configured() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Grace".to_owned(),
        });

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTrigger::Change,
        move |value, context| {
            validator_runs.set(validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);

            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );

    form.set_user_field(name_path(), String::new());

    assert_eq!(runs.get(), 0);
    assert_eq!(
        form.validation_status(name_path(), "required"),
        Some(ValidationStatus::Unknown)
    );
    assert!(form.validation_errors().is_empty());

    form.set_validation_mode(ValidationMode::on_change());
    form.set_user_field(name_path(), "Ada".to_owned());

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.validation_status(name_path(), "required"),
        Some(ValidationStatus::Valid)
    );
}

#[test]
fn submit_then_revalidate_mode_runs_change_validation_after_submit_attempt() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Grace".to_owned(),
        })
        .with_validation_mode(ValidationMode::submit_then_revalidate());

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTrigger::Change,
        move |value, context| {
            validator_runs.set(validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);

            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );

    form.set_user_field(name_path(), String::new());

    assert_eq!(runs.get(), 0);
    assert_eq!(
        form.validation_status(name_path(), "required"),
        Some(ValidationStatus::Unknown)
    );

    assert!(form.validate_for_submit());
    form.set_user_field(name_path(), "Ada".to_owned());

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.validation_status(name_path(), "required"),
        Some(ValidationStatus::Valid)
    );
}

#[test]
fn submit_then_revalidate_mode_runs_blur_validation_after_submit_attempt() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        })
        .with_validation_mode(ValidationMode::submit_then_revalidate());

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTrigger::Blur,
        move |value, context| {
            validator_runs.set(validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Blur);

            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );

    form.mark_field_blurred(name_path());

    assert_eq!(runs.get(), 0);
    assert_eq!(
        form.validation_status(name_path(), "required"),
        Some(ValidationStatus::Unknown)
    );

    assert!(form.validate_for_submit());
    form.mark_field_blurred(name_path());

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.validation_status(name_path(), "required"),
        Some(ValidationStatus::Invalid)
    );
}

#[test]
fn submit_then_revalidate_mode_preserves_submit_validation_correctness() {
    let called = Cell::new(false);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        })
        .with_validation_mode(ValidationMode::submit_then_revalidate());

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "submit_required",
        ValidationTrigger::Submit,
        |value, context| {
            assert_eq!(context.trigger(), ValidationTrigger::Submit);

            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );

    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(!called.get());
    assert_eq!(form.submit_attempt_count(), 1);
    assert_eq!(
        form.validation_status(name_path(), "submit_required"),
        Some(ValidationStatus::Invalid)
    );
}

#[test]
fn submit_then_revalidate_mode_runs_form_validation_after_submit_attempt() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Grace".to_owned(),
        })
        .with_validation_mode(ValidationMode::submit_then_revalidate());

    form.register_sync_form_validator_for_triggers(
        "name_present",
        ValidationTrigger::Change,
        move |context| {
            validator_runs.set(validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);

            if context.form().name.is_empty() {
                vec![FormValidationError::field(name_path(), "required")]
            } else {
                Vec::new()
            }
        },
    );

    form.set_user_field(name_path(), String::new());

    assert_eq!(runs.get(), 0);
    assert_eq!(
        form.form_validation_status("name_present"),
        Some(ValidationStatus::Unknown)
    );

    assert!(form.validate_for_submit());
    form.set_user_field(name_path(), "Ada".to_owned());

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status("name_present"),
        Some(ValidationStatus::Valid)
    );
}

#[test]
fn configured_value_change_runs_changed_field_and_form_validators() {
    let email_runs = Rc::new(Cell::new(0));
    let password_runs = Rc::new(Cell::new(0));
    let form_runs = Rc::new(Cell::new(0));
    let email_validator_runs = Rc::clone(&email_runs);
    let password_validator_runs = Rc::clone(&password_runs);
    let form_validator_runs = Rc::clone(&form_runs);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "different".to_owned(),
        })
        .with_validation_mode(ValidationMode::on_submit().validate_on_change());

    form.register_sync_field_validator_for_triggers(
        email_path(),
        "email_required",
        ValidationTrigger::Change,
        move |value, context| {
            email_validator_runs.set(email_validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);
            assert!(context.field_metadata().is_touched());

            if value.is_empty() {
                vec!["email_required"]
            } else {
                Vec::new()
            }
        },
    );
    form.register_sync_field_validator_for_triggers(
        password_path(),
        "password_required",
        ValidationTrigger::Change,
        move |value, _context| {
            password_validator_runs.set(password_validator_runs.get() + 1);

            if value.is_empty() {
                vec!["password_required"]
            } else {
                Vec::new()
            }
        },
    );
    form.register_sync_form_validator_for_triggers(
        "passwords_match",
        ValidationTrigger::Change,
        move |context| {
            form_validator_runs.set(form_validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);

            if context.form().password == context.form().confirm_password {
                Vec::new()
            } else {
                vec![FormValidationError::field(
                    confirm_password_path(),
                    "password_mismatch",
                )]
            }
        },
    );

    form.set_user_field(email_path(), String::new());

    assert_eq!(email_runs.get(), 1);
    assert_eq!(password_runs.get(), 0);
    assert_eq!(form_runs.get(), 1);

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                ValidationTarget::Field(email_path().identity()),
                "email_required",
                "email_required",
            ),
            (
                ValidationTarget::Field(confirm_password_path().identity()),
                "passwords_match",
                "password_mismatch",
            ),
        ]
    );
    assert!(
        form.visible_field_validation_errors(email_path())
            .is_empty()
    );

    form.mark_field_blurred(email_path());

    assert_eq!(
        form.visible_field_validation_errors(email_path())[0].error(),
        &"email_required"
    );

    form.set_user_field(password_path(), "different".to_owned());

    assert_eq!(email_runs.get(), 1);
    assert_eq!(password_runs.get(), 1);
    assert_eq!(form_runs.get(), 2);

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(errors, vec![("email_required", "email_required")]);
}

#[test]
fn default_visible_errors_wait_for_blur_or_submit_attempt() {
    let mut blur_form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });
    blur_form.register_sync_field_validator(name_path(), "required", |_value, _context| {
        vec!["required"]
    });
    blur_form.validate_field(name_path(), ValidationTrigger::Manual);

    assert_eq!(blur_form.validation_errors().len(), 1);
    assert!(blur_form.visible_validation_errors().is_empty());

    blur_form.mark_field_blurred(name_path());

    assert_eq!(
        blur_form.visible_validation_errors()[0].error(),
        &"required"
    );

    let mut submit_form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });
    submit_form.register_sync_field_validator(name_path(), "required", |_value, context| {
        assert_eq!(context.trigger(), ValidationTrigger::Submit);
        vec!["required"]
    });

    assert!(!submit_form.validate_for_submit());
    assert_eq!(submit_form.submit_attempt_count(), 1);
    assert_eq!(
        submit_form.visible_field_validation_errors(name_path())[0].error(),
        &"required"
    );
}

#[test]
fn error_visibility_policy_controls_visible_error_selectors() {
    let mut touched_form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        })
        .with_error_visibility_policy(ErrorVisibilityPolicy::TouchedOrSubmit);
    touched_form.register_sync_field_validator(name_path(), "required", |_value, _context| {
        vec!["required"]
    });
    touched_form.validate_field(name_path(), ValidationTrigger::Manual);

    assert!(
        touched_form
            .visible_field_validation_errors(name_path())
            .is_empty()
    );

    touched_form.mark_field_touched(name_path());

    assert_eq!(
        touched_form.visible_field_validation_errors(name_path())[0].error(),
        &"required"
    );

    let mut submit_only_form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        })
        .with_error_visibility_policy(ErrorVisibilityPolicy::SubmitOnly);
    submit_only_form.register_sync_field_validator(name_path(), "required", |_value, _context| {
        vec!["required"]
    });
    submit_only_form.validate_field(name_path(), ValidationTrigger::Manual);
    submit_only_form.mark_field_blurred(name_path());

    assert!(
        submit_only_form
            .visible_field_validation_errors(name_path())
            .is_empty()
    );

    submit_only_form.mark_submit_attempt();

    assert_eq!(
        submit_only_form.visible_field_validation_errors(name_path())[0].error(),
        &"required"
    );
}

#[test]
fn trigger_scoped_value_change_errors_follow_default_visibility_policy() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "change_required",
        ValidationTrigger::Change,
        |_value, context| {
            assert_eq!(context.trigger(), ValidationTrigger::Change);
            vec!["required"]
        },
    );
    form.register_sync_field_validator_for_triggers(
        name_path(),
        "blur_hint",
        ValidationTrigger::Blur,
        |_value, context| {
            assert_eq!(context.trigger(), ValidationTrigger::Blur);
            vec!["blur_hint"]
        },
    );

    form.validate_field(name_path(), ValidationTrigger::Change);

    assert_eq!(form.field_validation_errors(name_path()).len(), 1);
    assert!(form.visible_field_validation_errors(name_path()).is_empty());

    form.mark_field_blurred_without_validation(name_path());

    let visible_errors: Vec<_> = form
        .visible_field_validation_errors(name_path())
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(visible_errors, vec![("change_required", "required")]);

    form.validate_field(name_path(), ValidationTrigger::Blur);

    let visible_errors: Vec<_> = form
        .visible_field_validation_errors(name_path())
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        visible_errors,
        vec![("change_required", "required"), ("blur_hint", "blur_hint")]
    );
}

#[test]
fn submit_handler_receives_an_owned_validated_snapshot() {
    let submitted = Rc::new(RefCell::new(None));
    let submitted_snapshot = Rc::clone(&submitted);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });

    form.register_sync_field_validator(name_path(), "required", |value, context| {
        assert_eq!(context.trigger(), ValidationTrigger::Submit);

        if value.is_empty() {
            vec!["required"]
        } else {
            Vec::new()
        }
    });

    let result = form.submit(move |submitted| {
        assert_eq!(submitted.value().name, "Ada");
        submitted_snapshot
            .borrow_mut()
            .replace(submitted.into_value());
    });

    assert_eq!(result, SubmitResult::Succeeded);
    assert_eq!(
        submitted.borrow().as_ref(),
        Some(&ContactForm {
            name: "Ada".to_owned()
        })
    );
    assert_eq!(form.submit_attempt_count(), 1);
    assert!(!form.is_submitting());
}

#[test]
fn submit_intent_reaches_submit_validation_and_handler_snapshot() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_form_validator_for_triggers(
        "publish_name_required",
        ValidationTrigger::Submit,
        |context| {
            assert!(context.submit_intent::<()>().is_none());

            if context.submit_intent::<ContactSubmitIntent>() == Some(&ContactSubmitIntent::Publish)
                && context.form().name.is_empty()
            {
                vec![FormValidationError::field(
                    name_path(),
                    "publish_name_required",
                )]
            } else {
                Vec::new()
            }
        },
    );

    let draft_result = form
        .intent(ContactSubmitIntent::SaveDraft)
        .submit(|submitted| {
            assert_eq!(submitted.intent(), &ContactSubmitIntent::SaveDraft);
            assert_eq!(submitted.value().name, "");
        });

    assert_eq!(draft_result, SubmitResult::Succeeded);
    assert_eq!(
        form.intent(ContactSubmitIntent::SaveDraft).last_status(),
        Some(SubmitStatus::Succeeded)
    );

    let publish_called = Cell::new(false);
    let publish_result = form
        .intent(ContactSubmitIntent::Publish)
        .submit(|_submitted| publish_called.set(true));

    assert_eq!(
        publish_result,
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(!publish_called.get());
    assert_eq!(
        form.field_validation_errors(name_path())[0].error(),
        &"publish_name_required"
    );
    assert_eq!(
        form.intent(ContactSubmitIntent::Publish).last_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );
    assert_eq!(
        form.intent(ContactSubmitIntent::SaveDraft).last_status(),
        None
    );
    let latest = form
        .last_submit_status_as::<ContactSubmitIntent>()
        .expect("latest submit status should carry typed intent");
    assert_eq!(latest.intent(), &ContactSubmitIntent::Publish);
    assert_eq!(
        latest.status(),
        SubmitStatus::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(form.intent(ContactSubmitIntent::SaveDraft).can_submit());
    assert!(!form.intent(ContactSubmitIntent::Publish).can_submit());
}

#[test]
fn submit_intent_scope_rejects_mismatched_validation_snapshot() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let validation = form
        .intent(ContactSubmitIntent::SaveDraft)
        .validation_snapshot();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        form.intent(ContactSubmitIntent::Publish)
            .begin_submission_after_validation(&validation);
    }));

    assert!(result.is_err());
    assert!(!form.is_submitting());
    assert_eq!(form.last_submit_status(), None);
}

#[test]
fn submit_intent_filters_visible_validation_errors() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        })
        .with_error_visibility_policy(ErrorVisibilityPolicy::SubmitOnly);

    form.register_sync_form_validator_for_triggers(
        "publish_name_required",
        ValidationTrigger::Submit,
        |context| {
            if context.submit_intent::<ContactSubmitIntent>() == Some(&ContactSubmitIntent::Publish)
                && context.form().name.is_empty()
            {
                vec![FormValidationError::field(
                    name_path(),
                    "publish_name_required",
                )]
            } else {
                Vec::new()
            }
        },
    );

    assert_eq!(
        form.intent(ContactSubmitIntent::Publish)
            .submit(|_submitted| ()),
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );

    assert_eq!(
        form.visible_field_validation_errors_for_intent(name_path(), &ContactSubmitIntent::Publish)
            [0]
        .error(),
        &"publish_name_required"
    );
    assert!(
        form.visible_field_validation_errors_for_intent(
            name_path(),
            &ContactSubmitIntent::SaveDraft
        )
        .is_empty()
    );
    assert_eq!(
        form.visible_validation_errors_for_intent(&ContactSubmitIntent::Publish)[0].error(),
        &"publish_name_required"
    );
    assert!(
        form.visible_validation_errors_for_intent(&ContactSubmitIntent::SaveDraft)
            .is_empty()
    );
}

#[test]
fn submit_intent_scopes_async_validation_results_and_pending_work() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let id =
        form.register_async_form_validator_for_triggers("publish_async", ValidationTrigger::Submit);

    assert!(
        !form
            .intent(ContactSubmitIntent::Publish)
            .validate_for_submit()
    );
    let publish_run = form
        .begin_async_form_validation_after_sync(id, ValidationTrigger::Submit)
        .expect("publish submit async validation should start");

    assert_eq!(
        publish_run
            .validator_context()
            .submit_intent::<ContactSubmitIntent>(),
        Some(&ContactSubmitIntent::Publish)
    );

    form.complete_async_form_validation(
        id,
        &publish_run,
        [FormValidationError::field(name_path(), "publish_error")],
    );

    assert!(!form.intent(ContactSubmitIntent::Publish).can_submit());
    assert!(form.intent(ContactSubmitIntent::SaveDraft).can_submit());

    assert!(
        !form
            .intent(ContactSubmitIntent::SaveDraft)
            .validate_for_submit()
    );
    let save_run = form
        .begin_async_form_validation_after_sync(id, ValidationTrigger::Submit)
        .expect("save draft submit async validation should start independently");

    assert_eq!(
        save_run
            .validator_context()
            .submit_intent::<ContactSubmitIntent>(),
        Some(&ContactSubmitIntent::SaveDraft)
    );
}

#[test]
fn submit_intent_availability_includes_non_submit_errors_for_all_intents() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "name_required_on_blur",
        ValidationTrigger::Blur,
        |value, _context| {
            if value.is_empty() {
                vec!["name_required"]
            } else {
                Vec::new()
            }
        },
    );

    form.validate_field(name_path(), ValidationTrigger::Blur);

    assert!(!form.intent(ContactSubmitIntent::Publish).can_submit());
    assert!(!form.intent(ContactSubmitIntent::SaveDraft).can_submit());
    assert_eq!(
        form.intent(ContactSubmitIntent::SaveDraft)
            .submit(|_submitted| ()),
        SubmitResult::Succeeded
    );
}

#[test]
fn form_state_snapshot_omits_submit_scoped_errors_and_validation_state() {
    let mut source: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        })
        .with_error_visibility_policy(ErrorVisibilityPolicy::Always);

    source.register_sync_form_validator_for_triggers(
        "publish_name_required",
        ValidationTrigger::Submit,
        |context| {
            if context.submit_intent::<ContactSubmitIntent>() == Some(&ContactSubmitIntent::Publish)
                && context.form().name.is_empty()
            {
                vec![FormValidationError::field(
                    name_path(),
                    "publish_name_required",
                )]
            } else {
                Vec::new()
            }
        },
    );

    assert_eq!(
        source
            .intent(ContactSubmitIntent::Publish)
            .submit(|_submitted| { SubmitError::field(name_path(), "server_rejected_publish") }),
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert_eq!(
        source
            .visible_field_validation_errors_for_intent(name_path(), &ContactSubmitIntent::Publish)
            [0]
        .error(),
        &"publish_name_required"
    );

    let snapshot = source.state_snapshot();
    let mut restored: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "stale target state".to_owned(),
        })
        .with_error_visibility_policy(ErrorVisibilityPolicy::Always);
    restored.register_sync_form_validator_for_triggers(
        "publish_name_required",
        ValidationTrigger::Submit,
        |_context| vec![FormValidationError::field(name_path(), "target_old_error")],
    );
    restored
        .intent(ContactSubmitIntent::Publish)
        .validate_for_submit();

    restored
        .restore_state_snapshot(snapshot)
        .expect("snapshot should restore");

    assert!(restored.validation_errors().is_empty());
    assert!(
        restored
            .visible_validation_errors_for_intent(&ContactSubmitIntent::Publish)
            .is_empty()
    );
    assert_eq!(
        restored.snapshot(),
        ContactForm {
            name: String::new()
        }
    );
}

#[test]
fn submit_intent_reaches_async_validation_context() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let id =
        form.register_async_form_validator_for_triggers("publish_async", ValidationTrigger::Submit);

    assert!(
        !form
            .intent(ContactSubmitIntent::Publish)
            .validate_for_submit()
    );

    let run = form
        .begin_async_form_validation_after_sync(id, ValidationTrigger::Submit)
        .expect("submit async validation should start");
    let context = run.validator_context();

    assert_eq!(
        context.submit_intent::<ContactSubmitIntent>(),
        Some(&ContactSubmitIntent::Publish)
    );
    assert_eq!(context.value().name, "Ada");
}

#[test]
fn submit_validates_before_handler_and_blocks_invalid_submissions() {
    let called = Cell::new(false);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator(name_path(), "required", |value, context| {
        assert_eq!(context.trigger(), ValidationTrigger::Submit);

        if value.is_empty() {
            vec!["required"]
        } else {
            Vec::new()
        }
    });

    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(!called.get());
    assert_eq!(form.submit_attempt_count(), 1);
    assert!(!form.is_submitting());
    assert_eq!(
        form.visible_field_validation_errors(name_path())[0].error(),
        &"required"
    );
}

#[test]
fn submit_runs_submit_triggered_validators_before_handler() {
    let called = Cell::new(false);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: String::new(),
            password: "secret".to_owned(),
            confirm_password: "different".to_owned(),
        });

    form.register_sync_field_validator_for_triggers(
        email_path(),
        "submit_email_required",
        ValidationTrigger::Submit,
        |value, context| {
            assert_eq!(context.trigger(), ValidationTrigger::Submit);

            if value.is_empty() {
                vec!["email_required"]
            } else {
                Vec::new()
            }
        },
    );
    form.register_sync_form_validator_for_triggers(
        "submit_passwords_match",
        ValidationTrigger::Submit,
        |context| {
            assert_eq!(context.trigger(), ValidationTrigger::Submit);

            if context.form().password == context.form().confirm_password {
                Vec::new()
            } else {
                vec![FormValidationError::field(
                    confirm_password_path(),
                    "password_mismatch",
                )]
            }
        },
    );

    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(!called.get());

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.field().unwrap().as_str().to_owned(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                "email".to_owned(),
                "submit_email_required",
                "email_required",
            ),
            (
                "confirm_password".to_owned(),
                "submit_passwords_match",
                "password_mismatch",
            ),
        ]
    );
}

#[test]
fn non_submit_triggered_validator_errors_do_not_block_submit_validation_authority() {
    let called = Cell::new(false);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "manual_hint",
        ValidationTrigger::Manual,
        |_value, _context| vec!["manual_hint"],
    );
    form.register_sync_field_validator_for_triggers(
        name_path(),
        "submit_required",
        ValidationTrigger::Submit,
        |value, _context| {
            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );

    form.validate_field(name_path(), ValidationTrigger::Manual);
    form.set_user_field(name_path(), "Ada".to_owned());

    assert_eq!(
        form.validation_status(name_path(), "manual_hint"),
        Some(ValidationStatus::Invalid)
    );

    let result = form.submit(|submitted| {
        assert_eq!(submitted.value().name, "Ada");
        called.set(true);
    });

    assert_eq!(result, SubmitResult::Succeeded);
    assert!(called.get());
    assert_eq!(
        form.validation_status(name_path(), "manual_hint"),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.validation_status(name_path(), "submit_required"),
        Some(ValidationStatus::Valid)
    );
}

#[test]
fn submit_availability_reflects_validation_errors_and_in_flight_submission() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    assert!(form.can_submit());
    assert!(form.submit_availability().is_available());

    form.register_sync_field_validator(name_path(), "required", |value, _context| {
        if value.is_empty() {
            vec!["required"]
        } else {
            Vec::new()
        }
    });

    assert!(form.can_submit());

    form.validate_all(ValidationTrigger::Manual);

    assert!(!form.can_submit());
    assert_eq!(
        form.submit_availability().blockers(),
        &[SubmitBlocker::ValidationErrors]
    );

    form.set_field(name_path(), "Ada".to_owned());
    form.validate_all(ValidationTrigger::Manual);

    assert!(form.can_submit());
    assert!(matches!(form.begin_submission(), SubmitAttempt::Started(_)));
    assert!(!form.can_submit());
    assert_eq!(
        form.submit_availability().blockers(),
        &[SubmitBlocker::InFlightSubmission]
    );
}

#[test]
fn submit_availability_reflects_only_submit_relevant_pending_validation() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let advisory = form.register_async_field_validator_for_triggers(
        name_path(),
        "advisory",
        ValidationTrigger::Manual,
    );
    let submit_required = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Submit,
    );

    form.begin_async_field_validation(name_path(), advisory, ValidationTrigger::Manual)
        .expect("advisory async validation should start");

    assert!(form.can_submit());
    assert!(form.submit_availability().is_available());

    form.begin_async_field_validation(name_path(), submit_required, ValidationTrigger::Submit)
        .expect("submit async validation should start");

    assert!(!form.can_submit());
    assert_eq!(
        form.submit_availability().blockers(),
        &[SubmitBlocker::PendingValidation]
    );
}

#[test]
fn synchronous_submit_blocks_pending_submit_validation_without_calling_handler() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Submit,
    );
    let called = Cell::new(false);

    form.begin_async_field_validation(name_path(), availability, ValidationTrigger::Submit)
        .expect("submit async validation should start");

    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert!(!called.get());
    assert_eq!(form.submit_attempt_count(), 1);
    assert_eq!(
        form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
    );
    assert!(!form.is_submitting());
    assert_eq!(
        form.submit_availability().blockers(),
        &[SubmitBlocker::PendingValidation]
    );
}

#[test]
fn duplicate_submit_validation_does_not_restart_same_pending_async_field_run() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );

    let value_change_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Change)
        .expect("value-change validation should start");

    let submit_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Submit)
        .expect("submit validation should replace value-change validation");

    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &value_change_run,
            ["value_change_unavailable"],
        ),
        None
    );
    assert!(
        form.begin_async_field_validation(name_path(), availability, ValidationTrigger::Submit)
            .is_none()
    );
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );

    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &submit_run,
            Vec::<&str>::new(),
        ),
        Some(ValidationStatus::Valid)
    );
}

#[test]
fn duplicate_submit_validation_does_not_restart_same_pending_async_form_run() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account = form.register_async_form_validator_for_triggers(
        "account",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );

    let value_change_run = form
        .begin_async_form_validation(account, ValidationTrigger::Change)
        .expect("value-change form validation should start");

    let submit_run = form
        .begin_async_form_validation(account, ValidationTrigger::Submit)
        .expect("submit form validation should replace value-change validation");

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &value_change_run,
            [FormValidationError::form("value_change_unavailable")],
        ),
        None
    );
    assert!(
        form.begin_async_form_validation(account, ValidationTrigger::Submit)
            .is_none()
    );
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Pending)
    );

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &submit_run,
            Vec::<FormValidationError<&str>>::new(),
        ),
        Some(ValidationStatus::Valid)
    );
}

#[test]
fn submit_requires_submit_triggered_async_field_validation_after_value_change_success() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );
    let value_change_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Change)
        .expect("value-change validation should start");

    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &value_change_run,
            Vec::<&str>::new(),
        ),
        Some(ValidationStatus::Valid)
    );

    let called = Cell::new(false);
    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert!(!called.get());
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );

    let submit_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Submit)
        .expect("submit validation should replace value-change result");

    assert_eq!(submit_run.trigger(), ValidationTrigger::Submit);
}

#[test]
fn submit_requires_submit_triggered_async_field_validation_after_manual_success() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Submit]),
    );
    let manual_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("manual validation should start");

    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &manual_run,
            Vec::<&str>::new(),
        ),
        Some(ValidationStatus::Valid)
    );

    let called = Cell::new(false);
    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert!(!called.get());
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );

    let submit_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Submit)
        .expect("submit validation should replace manual result");

    assert_eq!(submit_run.trigger(), ValidationTrigger::Submit);
}

#[test]
fn submit_requires_submit_triggered_async_form_validation_after_value_change_success() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account = form.register_async_form_validator_for_triggers(
        "account",
        ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
    );
    let value_change_run = form
        .begin_async_form_validation(account, ValidationTrigger::Change)
        .expect("value-change form validation should start");

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &value_change_run,
            Vec::<FormValidationError<&str>>::new(),
        ),
        Some(ValidationStatus::Valid)
    );

    let called = Cell::new(false);
    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert!(!called.get());
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Pending)
    );

    let submit_run = form
        .begin_async_form_validation(account, ValidationTrigger::Submit)
        .expect("submit form validation should replace value-change result");

    assert_eq!(submit_run.trigger(), ValidationTrigger::Submit);
}

#[test]
fn submit_requires_submit_triggered_async_form_validation_after_manual_success() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account = form.register_async_form_validator_for_triggers(
        "account",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Submit]),
    );
    let manual_run = form
        .begin_async_form_validation(account, ValidationTrigger::Manual)
        .expect("manual form validation should start");

    assert_eq!(
        form.complete_async_form_validation(
            account,
            &manual_run,
            Vec::<FormValidationError<&str>>::new(),
        ),
        Some(ValidationStatus::Valid)
    );

    let called = Cell::new(false);
    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert!(!called.get());
    assert_eq!(
        form.form_validation_status_by_id(account),
        Some(ValidationStatus::Pending)
    );

    let submit_run = form
        .begin_async_form_validation(account, ValidationTrigger::Submit)
        .expect("submit form validation should replace manual result");

    assert_eq!(submit_run.trigger(), ValidationTrigger::Submit);
}

#[test]
fn begin_submission_after_validation_rejects_reset_and_reinitialize_lifecycles() {
    let mut reset_form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    reset_form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Submit,
    );
    let reset_validation = reset_form.submit_validation_snapshot();

    assert!(!reset_form.validate_for_submit());
    assert_eq!(
        reset_form.submit_availability().blockers(),
        &[SubmitBlocker::PendingValidation]
    );

    reset_form.reset();

    assert_eq!(
        reset_form.begin_submission_after_validation(&reset_validation),
        SubmitAttempt::Blocked(SubmitBlocker::PendingValidation)
    );
    assert_eq!(
        reset_form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
    );

    let mut reinitialized_form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    reinitialized_form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Submit,
    );
    let reinitialize_validation = reinitialized_form.submit_validation_snapshot();

    assert!(!reinitialized_form.validate_for_submit());
    reinitialized_form.reinitialize(ContactForm {
        name: "Grace".to_owned(),
    });

    assert_eq!(
        reinitialized_form.begin_submission_after_validation(&reinitialize_validation),
        SubmitAttempt::Blocked(SubmitBlocker::PendingValidation)
    );
    assert_eq!(
        reinitialized_form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
    );
}

#[test]
fn begin_submission_after_validation_rejects_unresolved_submit_async_field_validation() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Submit,
    );
    let validation = form.submit_validation_snapshot();

    assert_eq!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::PendingValidation)
    );
    assert_eq!(
        form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
    );

    let run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Submit)
        .expect("submit async field validation should start");
    assert_eq!(
        form.complete_async_field_validation(name_path(), availability, &run, Vec::<&str>::new()),
        Some(ValidationStatus::Valid)
    );

    assert!(matches!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Started(_)
    ));
}

#[test]
fn begin_submission_after_validation_rejects_unresolved_submit_async_form_validation() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });
    let account =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Submit);
    let validation = form.submit_validation_snapshot();

    assert_eq!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::PendingValidation)
    );
    assert_eq!(
        form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
    );

    let run = form
        .begin_async_form_validation(account, ValidationTrigger::Submit)
        .expect("submit async form validation should start");
    assert_eq!(
        form.complete_async_form_validation(
            account,
            &run,
            Vec::<FormValidationError<&str>>::new(),
        ),
        Some(ValidationStatus::Valid)
    );

    assert!(matches!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Started(_)
    ));
}

#[test]
fn synchronous_submit_blocks_unresolved_submit_async_validation_without_calling_handler() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Submit,
    );
    let called = Cell::new(false);

    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert!(!called.get());
    assert_eq!(form.submit_attempt_count(), 1);
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(
        form.submit_availability().blockers(),
        &[SubmitBlocker::PendingValidation]
    );
}

#[test]
fn field_edit_invalidates_completed_submit_async_validation() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Submit,
    );
    let run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Submit)
        .expect("submit async validation should start");

    assert_eq!(
        form.complete_async_field_validation(name_path(), availability, &run, Vec::<&str>::new()),
        Some(ValidationStatus::Valid)
    );

    form.set_user_field(name_path(), "Grace".to_owned());

    let called = Cell::new(false);
    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert!(!called.get());
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );
}

#[test]
fn context_edit_invalidates_completed_submit_async_field_validation() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "old-secret".to_owned(),
            confirm_password: "old-secret".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        email_path(),
        "availability",
        ValidationTrigger::Submit,
    );
    let run = form
        .begin_async_field_validation(email_path(), availability, ValidationTrigger::Submit)
        .expect("submit async validation should start");

    assert_eq!(run.form_snapshot().value().password, "old-secret");
    assert_eq!(
        form.complete_async_field_validation(email_path(), availability, &run, Vec::<&str>::new()),
        Some(ValidationStatus::Valid)
    );

    form.set_user_field(password_path(), "new-secret".to_owned());

    assert_eq!(
        form.field_validation_status(email_path(), availability),
        Some(ValidationStatus::Stale)
    );

    let called = Cell::new(false);
    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert!(!called.get());
    assert_eq!(
        form.field_validation_status(email_path(), availability),
        Some(ValidationStatus::Pending)
    );
}

#[test]
fn synchronous_submit_blocks_invalid_async_validation_without_submit_errors() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Submit,
    );
    let run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Submit)
        .expect("submit async validation should start");
    let called = Cell::new(false);

    assert_eq!(
        form.complete_async_field_validation(name_path(), availability, &run, ["unavailable"],),
        Some(ValidationStatus::Invalid)
    );

    let result = form.submit(|_submitted| {
        called.set(true);
        SubmitError::form("submit_error")
    });

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(!called.get());
    assert_eq!(form.submit_attempt_count(), 1);
    assert_eq!(
        form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );

    let errors: Vec<_> = form
        .visible_field_validation_errors(name_path())
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![(Some(availability), "availability", "unavailable")]
    );
}

#[test]
fn concurrent_submission_is_blocked_without_counting_duplicate_attempts() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });

    assert!(matches!(form.begin_submission(), SubmitAttempt::Started(_)));
    assert!(form.is_submitting());
    assert_eq!(form.submit_attempt_count(), 1);

    assert!(matches!(
        form.begin_submission(),
        SubmitAttempt::Blocked(SubmitBlocker::InFlightSubmission)
    ));
    assert_eq!(form.submit_attempt_count(), 1);

    assert!(form.finish_submission_success());
    assert!(!form.is_submitting());
}

#[test]
fn last_submit_status_tracks_submission_outcomes() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });

    assert_eq!(form.last_submit_status(), None);

    let submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    assert_eq!(form.last_submit_status(), None);
    assert_eq!(
        form.begin_submission(),
        SubmitAttempt::Blocked(SubmitBlocker::InFlightSubmission)
    );
    assert_eq!(
        form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission))
    );

    assert!(form.finish_submission_with_errors(submitted, SubmitError::form("try_later")));
    assert_eq!(form.last_submit_status(), Some(SubmitStatus::Rejected));

    let _submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };
    assert_eq!(form.last_submit_status(), Some(SubmitStatus::Rejected));
    assert!(form.finish_submission_success());
    assert_eq!(form.last_submit_status(), Some(SubmitStatus::Succeeded));

    let _submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };
    assert_eq!(form.last_submit_status(), Some(SubmitStatus::Succeeded));
    assert_eq!(
        form.begin_submission(),
        SubmitAttempt::Blocked(SubmitBlocker::InFlightSubmission)
    );
    assert_eq!(
        form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission))
    );
    assert!(form.finish_submission_success());
    assert_eq!(form.last_submit_status(), Some(SubmitStatus::Succeeded));

    form.register_sync_field_validator(name_path(), "required", |value, _context| {
        if value.is_empty() {
            vec!["required"]
        } else {
            Vec::new()
        }
    });
    form.set_field(name_path(), String::new());

    assert_eq!(
        form.submit(|_submitted| ()),
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert_eq!(
        form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );

    form.reset();
    assert_eq!(form.last_submit_status(), None);
    assert_eq!(
        form.block_submission_with_parse_errors(),
        SubmitAttempt::Blocked(SubmitBlocker::ParseErrors)
    );
    assert_eq!(
        form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ParseErrors))
    );
}

#[test]
fn valid_submit_validation_preserves_previous_submit_status_until_submission_outcome() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });

    assert_eq!(
        form.submit(|_submitted| SubmitError::form("try_later")),
        SubmitResult::Rejected
    );
    assert_eq!(form.last_submit_status(), Some(SubmitStatus::Rejected));

    assert!(form.validate_for_submit());

    assert_eq!(form.last_submit_status(), Some(SubmitStatus::Rejected));
}

#[test]
fn successful_submission_does_not_reset_values_or_update_baseline() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Grace".to_owned(),
        });

    form.set_user_field(name_path(), "Ada".to_owned());

    let result = form.submit(|submitted| {
        assert_eq!(submitted.value().name, "Ada");
    });

    assert_eq!(result, SubmitResult::Succeeded);
    assert_eq!(form.draft().baseline().name, "Grace");
    assert_eq!(form.field_value(name_path()), "Ada");
    assert!(form.is_dirty());
}

#[test]
fn draft_edits_during_in_flight_submission_do_not_change_the_submitted_snapshot() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });

    let submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    form.set_user_field(name_path(), "Lin".to_owned());

    assert_eq!(submitted.value().name, "Ada");
    assert_eq!(form.field_value(name_path()), "Lin");
    assert!(matches!(
        form.begin_submission(),
        SubmitAttempt::Blocked(SubmitBlocker::InFlightSubmission)
    ));
    assert_eq!(form.submit_attempt_count(), 1);

    assert!(form.finish_submission_success());

    let next_submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    assert_eq!(next_submitted.value().name, "Lin");
    assert_eq!(form.submit_attempt_count(), 2);
}

#[test]
fn submit_handler_can_return_form_level_submit_errors() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });

    let result = form.submit(|submitted| {
        assert_eq!(submitted.value().name, "Ada");
        SubmitError::form("try_later")
    });

    assert_eq!(result, SubmitResult::Rejected);
    assert!(!form.is_submitting());
    assert_eq!(form.submit_attempt_count(), 1);

    let errors: Vec<_> = form
        .form_validation_errors()
        .into_iter()
        .map(|error| (error.target(), error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![(ValidationTarget::Form, "submit", "try_later")]
    );
    assert_eq!(
        form.visible_form_validation_errors()[0].error(),
        &"try_later"
    );
    assert!(!form.can_submit());
}

#[test]
fn field_level_submit_errors_render_through_field_error_views() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "taken@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });

    let result = form.submit(|_submitted| {
        SubmitErrors::with_source(
            "server",
            [SubmitError::field(email_path(), "email_unavailable")],
        )
    });

    assert_eq!(result, SubmitResult::Rejected);

    let field_errors: Vec<_> = form
        .field_validation_errors(email_path())
        .into_iter()
        .map(|error| {
            (
                error.field().unwrap().as_str().to_owned(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        field_errors,
        vec![("email".to_owned(), "server", "email_unavailable")]
    );
    assert_eq!(form.validation_errors().len(), 1);
    assert_eq!(
        form.visible_field_validation_errors(email_path())[0].error(),
        &"email_unavailable"
    );
}

#[test]
fn form_state_snapshot_keeps_submit_attempt_count_but_drops_rejected_submit_state() {
    let mut source: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });

    assert_eq!(
        source.submit(|submitted| {
            assert_eq!(submitted.value().name, "Ada");
            SubmitError::form("try_later")
        }),
        SubmitResult::Rejected
    );
    assert_eq!(source.submit_attempt_count(), 1);
    assert_eq!(source.last_submit_status(), Some(SubmitStatus::Rejected));
    assert_eq!(source.form_validation_errors()[0].error(), &"try_later");

    let snapshot = source.state_snapshot();
    let mut restored: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "placeholder".to_owned(),
        });

    restored
        .restore_state_snapshot(snapshot)
        .expect("snapshot should restore");

    assert_eq!(restored.snapshot().name, "Ada");
    assert_eq!(restored.submit_attempt_count(), 1);
    assert_eq!(restored.last_submit_status(), None);
    assert!(restored.form_validation_errors().is_empty());
    assert!(restored.validation_errors().is_empty());
}

#[test]
fn changing_a_field_clears_stale_submit_errors_for_that_field() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "taken@example.com".to_owned(),
            password: "short".to_owned(),
            confirm_password: "short".to_owned(),
        });

    assert_eq!(
        form.submit(|_submitted| {
            vec![
                SubmitError::field(email_path(), "email_unavailable"),
                SubmitError::field(password_path(), "password_weak"),
            ]
        }),
        SubmitResult::Rejected
    );

    form.set_user_field(email_path(), "new@example.com".to_owned());

    assert!(form.field_validation_errors(email_path()).is_empty());
    assert_eq!(
        form.field_validation_errors(password_path())[0].error(),
        &"password_weak"
    );
}

#[test]
fn stale_in_flight_field_submit_errors_are_discarded_when_field_value_changed() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "taken@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });

    let submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    form.set_user_field(email_path(), "new@example.com".to_owned());

    assert!(form.finish_submission_with_errors(
        submitted,
        SubmitError::field(email_path(), "email_unavailable"),
    ));

    assert!(!form.is_submitting());
    assert!(form.field_validation_errors(email_path()).is_empty());
    assert!(form.validation_errors().is_empty());
}

#[test]
fn comparable_field_submit_errors_use_value_comparison_against_the_submitted_snapshot() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "taken@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });

    let submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    form.set_user_field(email_path(), "new@example.com".to_owned());
    form.set_user_field(email_path(), "taken@example.com".to_owned());

    assert!(form.finish_submission_with_errors(
        submitted,
        SubmitError::field(email_path(), "email_unavailable"),
    ));

    assert_eq!(
        form.field_validation_errors(email_path())[0].error(),
        &"email_unavailable"
    );
}

#[test]
fn form_level_submit_errors_survive_field_specific_stale_checks() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "taken@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });

    let submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    form.set_user_field(email_path(), "new@example.com".to_owned());

    assert!(form.finish_submission_with_errors(
        submitted,
        vec![
            SubmitError::field(email_path(), "email_unavailable"),
            SubmitError::form("try_later"),
        ],
    ));

    assert!(form.field_validation_errors(email_path()).is_empty());
    assert_eq!(form.form_validation_errors()[0].error(), &"try_later");
}

#[derive(Clone, Debug)]
struct UploadToken {
    token: String,
}

#[derive(Clone, Debug)]
struct UploadForm {
    token: UploadToken,
}

fn upload_token_path() -> FieldPath<UploadForm, UploadToken> {
    FieldPath::direct(
        FieldIdentity::new("token"),
        "token",
        |model: &UploadForm| &model.token,
        |model: &mut UploadForm| &mut model.token,
    )
}

#[test]
fn field_identity_submit_errors_for_non_comparable_fields_drop_after_field_changes() {
    let mut form: FormCore<UploadForm, &'static str> = FormCore::new_with_error_type(UploadForm {
        token: UploadToken {
            token: "initial".to_owned(),
        },
    });

    let submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    form.set_user_field(
        upload_token_path(),
        UploadToken {
            token: "changed".to_owned(),
        },
    );

    assert_eq!(form.field_value(upload_token_path()).token, "changed");

    assert!(form.finish_submission_with_errors(
        submitted,
        SubmitError::field_identity(upload_token_path().identity(), "upload_failed"),
    ));
    assert!(form.field_validation_errors(upload_token_path()).is_empty());

    let submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    assert!(form.finish_submission_with_errors(
        submitted,
        SubmitError::field_identity(upload_token_path().identity(), "upload_failed"),
    ));
    assert_eq!(
        form.field_validation_errors(upload_token_path())[0].error(),
        &"upload_failed"
    );
}

#[test]
fn successful_submission_clears_previous_submit_sourced_errors() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });

    assert_eq!(
        form.submit(|_submitted| SubmitError::form("try_later")),
        SubmitResult::Rejected
    );
    assert_eq!(form.validation_errors().len(), 1);

    assert_eq!(form.submit(|_submitted| ()), SubmitResult::Succeeded);

    assert!(form.validation_errors().is_empty());
    assert!(!form.is_submitting());
}

#[test]
fn sync_form_validation_reads_the_whole_draft_and_records_form_level_errors() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "taken@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });

    form.register_sync_form_validator("account", move |context| {
        validator_runs.set(validator_runs.get() + 1);
        assert_eq!(context.form().email, "taken@example.com");
        assert_eq!(context.source().as_str(), "account");
        assert_eq!(context.trigger(), ValidationTrigger::Manual);
        assert!(!context.field_metadata(email_path()).is_blurred());
        vec![FormValidationError::form("account_unavailable")]
    });

    assert_eq!(runs.get(), 0);
    assert_eq!(
        form.form_validation_status("account"),
        Some(ValidationStatus::Unknown)
    );
    assert!(form.validation_errors().is_empty());

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status("account"),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![(ValidationTarget::Form, "account", "account_unavailable"),]
    );
    assert_eq!(form.form_validation_errors().len(), 1);
    assert!(form.visible_form_validation_errors().is_empty());

    form.mark_submit_attempt();

    assert_eq!(
        form.visible_form_validation_errors()[0].target(),
        ValidationTarget::Form
    );
}

#[test]
fn form_validation_attaches_cross_field_errors_to_fields() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: String::new(),
        });

    form.register_sync_field_validator(
        confirm_password_path(),
        "confirm_required",
        |value, _context| {
            if value.is_empty() {
                vec!["confirm_required"]
            } else {
                Vec::new()
            }
        },
    );
    form.register_sync_form_validator("passwords_match", |context| {
        if context.form().password == context.form().confirm_password {
            Vec::new()
        } else {
            vec![FormValidationError::field(
                confirm_password_path(),
                "password_mismatch",
            )]
        }
    });

    form.validate_all(ValidationTrigger::Manual);

    let field_errors: Vec<_> = form
        .field_validation_errors(confirm_password_path())
        .into_iter()
        .map(|error| {
            (
                error.field().unwrap().as_str().to_owned(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        field_errors,
        vec![
            (
                "confirm_password".to_owned(),
                "confirm_required",
                "confirm_required",
            ),
            (
                "confirm_password".to_owned(),
                "passwords_match",
                "password_mismatch",
            ),
        ]
    );
    let all_errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.field().unwrap().as_str().to_owned(),
                error.source().as_str(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(all_errors, field_errors);
    assert!(form.form_validation_errors().is_empty());

    let statuses: Vec<_> = form
        .validation_statuses()
        .into_iter()
        .map(|status| {
            (
                status.target(),
                status.source().as_str().to_owned(),
                status.status(),
            )
        })
        .collect();
    assert_eq!(
        statuses,
        vec![
            (
                ValidationTarget::Field(confirm_password_path().identity()),
                "confirm_required".to_owned(),
                ValidationStatus::Invalid,
            ),
            (
                ValidationTarget::Form,
                "passwords_match".to_owned(),
                ValidationStatus::Invalid,
            ),
        ]
    );
}

#[test]
fn field_validators_run_before_form_validators_for_the_same_trigger() {
    let order = Rc::new(RefCell::new(Vec::new()));
    let field_order = Rc::clone(&order);
    let form_order = Rc::clone(&order);
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: "secret".to_owned(),
        });

    form.register_sync_field_validator(
        password_path(),
        "password_strength",
        move |_value, context| {
            assert_eq!(context.trigger(), ValidationTrigger::Blur);
            field_order.borrow_mut().push("field");
            Vec::new()
        },
    );
    form.register_sync_form_validator("passwords_match", move |context| {
        assert_eq!(context.trigger(), ValidationTrigger::Blur);
        assert_eq!(form_order.borrow().as_slice(), &["field"]);
        form_order.borrow_mut().push("form");
        Vec::new()
    });

    form.validate_field(password_path(), ValidationTrigger::Blur);

    assert_eq!(order.borrow().as_slice(), &["field", "form"]);
}

#[test]
fn rerunning_one_form_validator_source_replaces_only_that_sources_errors() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "taken@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: String::new(),
        });

    form.register_sync_form_validator("account", |context| {
        if context.form().email == "taken@example.com" {
            vec![FormValidationError::form("account_unavailable")]
        } else {
            Vec::new()
        }
    });
    form.register_sync_form_validator("passwords_match", |context| {
        if context.form().password == context.form().confirm_password {
            Vec::new()
        } else {
            vec![FormValidationError::field(
                confirm_password_path(),
                "password_mismatch",
            )]
        }
    });

    form.validate_form(ValidationTrigger::Manual);
    form.set_user_field(confirm_password_path(), "secret".to_owned());

    assert_eq!(
        form.validate_form_source("passwords_match", ValidationTrigger::Manual),
        Some(ValidationStatus::Valid)
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![(ValidationTarget::Form, "account", "account_unavailable")]
    );
    assert_eq!(
        form.form_validation_status("account"),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.form_validation_status("passwords_match"),
        Some(ValidationStatus::Valid)
    );
}

#[test]
fn reset_restores_baseline_values_and_clears_interaction_metadata() {
    let mut form = FormCore::new(ContactForm {
        name: "Grace".to_owned(),
    });

    form.set_user_field(name_path(), "Ada".to_owned());
    form.mark_field_blurred(name_path());

    form.reset();

    assert_eq!(form.field_value(name_path()), "Grace");
    assert!(!form.is_dirty());
    assert!(!form.is_field_touched(name_path()));
    assert!(!form.is_field_blurred(name_path()));
}

#[test]
fn reinitialize_explicitly_replaces_baseline_and_current_values() {
    let mut form = FormCore::new(ContactForm {
        name: "Grace".to_owned(),
    });

    form.set_user_field(name_path(), "Ada".to_owned());
    form.mark_field_blurred(name_path());

    form.reinitialize(ContactForm {
        name: "Lin".to_owned(),
    });

    assert_eq!(form.draft().baseline().name, "Lin");
    assert_eq!(form.field_value(name_path()), "Lin");
    assert!(!form.is_dirty());
    assert!(!form.is_field_touched(name_path()));
    assert!(!form.is_field_blurred(name_path()));
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConditionalForm {
    show_details: bool,
    details: String,
}

fn show_details_path() -> FieldPath<ConditionalForm, bool> {
    FieldPath::direct(
        FieldIdentity::new("show_details"),
        "show_details",
        |model: &ConditionalForm| &model.show_details,
        |model: &mut ConditionalForm| &mut model.show_details,
    )
}

fn details_path() -> FieldPath<ConditionalForm, String> {
    FieldPath::direct(
        FieldIdentity::new("details"),
        "details",
        |model: &ConditionalForm| &model.details,
        |model: &mut ConditionalForm| &mut model.details,
    )
}

#[test]
fn conditional_hidden_fields_keep_their_draft_values() {
    let mut form = FormCore::new(ConditionalForm {
        show_details: true,
        details: "Keep this".to_owned(),
    });

    form.set_user_field(show_details_path(), false);

    assert!(!*form.field_value(show_details_path()));
    assert_eq!(form.field_value(details_path()), "Keep this");
    assert_eq!(form.snapshot().details, "Keep this");
}
