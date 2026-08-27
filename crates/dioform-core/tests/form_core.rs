use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use dioform_core::{
    CollectionIdentitySequence, CollectionItemIdentity, CollectionValidationTargetRule,
    CollectionValidationTargetRuleError, ErrorVisibilityPolicy, FieldIdentity, FieldPath,
    FieldUpdateOrigin, FormCore, FormObserverEvent, FormObserverField, FormStateRestoreError,
    FormValidationError, SubmitAttempt, SubmitBlocker, SubmitError, SubmitErrors, SubmitResult,
    SubmitStatus, ValidationMode, ValidationStatus, ValidationTarget, ValidationTrigger,
    ValidationTriggers, ValidatorSource,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Party {
    name: String,
    address: Option<PostalAddress>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PostalAddress {
    city: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Transaction {
    counterparty: Option<Party>,
}

static ABSENT_PARTY: Party = Party {
    name: String::new(),
    address: None,
};

static ALTERNATE_ABSENT_PARTY: Party = Party {
    name: String::new(),
    address: Some(PostalAddress {
        city: String::new(),
    }),
};

static ABSENT_POSTAL_ADDRESS: PostalAddress = PostalAddress {
    city: String::new(),
};

fn counterparty_path() -> FieldPath<Transaction, Option<Party>> {
    FieldPath::direct(
        FieldIdentity::new("counterparty"),
        "counterparty",
        |model: &Transaction| &model.counterparty,
        |model: &mut Transaction| &mut model.counterparty,
    )
}

fn party_name_path() -> FieldPath<Party, String> {
    FieldPath::direct(
        FieldIdentity::new("name"),
        "name",
        |party: &Party| &party.name,
        |party: &mut Party| &mut party.name,
    )
}

fn party_address_path() -> FieldPath<Party, Option<PostalAddress>> {
    FieldPath::direct(
        FieldIdentity::new("address"),
        "address",
        |party: &Party| &party.address,
        |party: &mut Party| &mut party.address,
    )
}

fn postal_address_city_path() -> FieldPath<PostalAddress, String> {
    FieldPath::direct(
        FieldIdentity::new("city"),
        "city",
        |address: &PostalAddress| &address.city,
        |address: &mut PostalAddress| &mut address.city,
    )
}

fn counterparty_name_path() -> FieldPath<Transaction, String> {
    counterparty_path()
        .or(&ABSENT_PARTY)
        .join(party_name_path())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Settlement {
    nominee: Option<Option<Party>>,
}

static ABSENT_NOMINEE: Option<Party> = None;

fn nominee_path() -> FieldPath<Settlement, Option<Option<Party>>> {
    FieldPath::direct(
        FieldIdentity::new("nominee"),
        "nominee",
        |model: &Settlement| &model.nominee,
        |model: &mut Settlement| &mut model.nominee,
    )
}

fn nominee_name_path() -> FieldPath<Settlement, String> {
    nominee_path()
        .or(&ABSENT_NOMINEE)
        .or(&ABSENT_PARTY)
        .join(party_name_path())
}

fn counterparty_city_path() -> FieldPath<Transaction, String> {
    counterparty_path()
        .or(&ABSENT_PARTY)
        .join(party_address_path())
        .or(&ABSENT_POSTAL_ADDRESS)
        .join(postal_address_city_path())
}

#[test]
fn direct_field_paths_with_the_same_structural_accessors_are_interchangeable() {
    assert_eq!(name_path(), name_path());
}

#[test]
fn composed_optional_field_paths_are_interchangeable_only_with_their_clones() {
    let first = counterparty_path().or(&ABSENT_PARTY);
    let first_clone = first.clone();
    let other_fallback = counterparty_path().or(&ALTERNATE_ABSENT_PARTY);

    assert_eq!(first, first_clone);
    assert_ne!(first, other_fallback);
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
struct CollectionPage {
    invoice: InvoiceForm,
    archived: Vec<InvoiceLine>,
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

fn collection_page_invoice_path() -> FieldPath<CollectionPage, InvoiceForm> {
    FieldPath::direct(
        FieldIdentity::new("invoice"),
        "invoice",
        |model: &CollectionPage| &model.invoice,
        |model: &mut CollectionPage| &mut model.invoice,
    )
}

fn collection_page_lines_path() -> FieldPath<CollectionPage, Vec<InvoiceLine>> {
    collection_page_invoice_path().join(lines_path())
}

fn archived_lines_path() -> FieldPath<CollectionPage, Vec<InvoiceLine>> {
    FieldPath::direct(
        FieldIdentity::new("archived"),
        "archived",
        |model: &CollectionPage| &model.archived,
        |model: &mut CollectionPage| &mut model.archived,
    )
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

fn line(description: &str) -> InvoiceLine {
    InvoiceLine {
        description: description.to_owned(),
        quantity: 1,
    }
}

fn line_identities(form: &mut FormCore<InvoiceForm>) -> Vec<CollectionItemIdentity> {
    form.collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect()
}

fn assert_async_validation_addressing_snapshot_survives_live_change<State>(
    prepare: impl FnOnce(&mut FormCore<InvoiceForm, &'static str>) -> State,
    change: impl FnOnce(&mut FormCore<InvoiceForm, &'static str>, &[CollectionItemIdentity], State),
) {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let rule = CollectionValidationTargetRule::descendant(lines_path(), line_description_path())
        .expect("a static item descendant should be supported");
    let validator = form.register_async_form_validator_for_triggers_with_collection_target_rules(
        "external rows",
        ValidationTrigger::Manual,
        [rule.clone()],
    );
    let state = prepare(&mut form);
    let validated_items: Vec<_> = form
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    let run = form
        .begin_async_form_validation(validator, ValidationTrigger::Manual)
        .expect("the async validator should start");

    change(&mut form, &validated_items, state);

    let target = run
        .validator_context()
        .resolve_collection_target(&rule, 0)
        .expect("the validated first row should remain addressable by the run");
    assert_eq!(
        target,
        ValidationTarget::field_identity(FieldIdentity::collection_item(
            "lines",
            validated_items[0],
            "description",
        ))
    );
    assert_eq!(
        form.complete_async_form_validation(
            validator,
            &run,
            [FormValidationError::for_target(target, "old diagnostic")],
        ),
        None
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn collection_validation_target_rules_prepare_and_resolve_item_and_descendant_targets() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let item_rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");
    let description_rule =
        CollectionValidationTargetRule::descendant(lines_path(), line_description_path())
            .expect("a static item descendant should be supported");
    let item_rule_for_validation = item_rule.clone();
    let description_rule_for_validation = description_rule.clone();

    form.register_sync_form_validator_with_collection_target_rules(
        "external rows",
        [item_rule, description_rule],
        move |context| {
            vec![
                FormValidationError::for_target(
                    item_rule_for_validation
                        .resolve(&context, 0)
                        .expect("the first row should resolve"),
                    "row",
                ),
                FormValidationError::for_target(
                    description_rule_for_validation
                        .resolve(&context, 1)
                        .expect("the second row should resolve"),
                    "description",
                ),
            ]
        },
    );

    form.validate_form(ValidationTrigger::Manual);

    let items = form.collection_items(lines_path());
    let targets: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| error.target())
        .collect();
    assert_eq!(
        targets,
        vec![
            ValidationTarget::field_identity(FieldIdentity::collection_item_value(
                "lines",
                items[0].identity(),
            )),
            ValidationTarget::field_identity(FieldIdentity::collection_item(
                "lines",
                items[1].identity(),
                "description",
            )),
        ]
    );
}

#[test]
fn collection_validation_target_rule_resolves_the_current_order_after_row_lifecycle_changes() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let rule = CollectionValidationTargetRule::descendant(lines_path(), line_description_path())
        .expect("a static item descendant should be supported");
    let rule_for_validation = rule.clone();
    form.register_sync_form_validator_with_collection_target_rules(
        "external rows",
        [rule],
        move |context| {
            (0..context.form().lines.len())
                .filter_map(|row| rule_for_validation.resolve(&context, row))
                .map(|target| FormValidationError::for_target(target, "description"))
                .collect()
        },
    );
    let initial = form.collection_items(lines_path());
    let removed = initial[0].identity();
    let survivor = initial[1].identity();

    form.remove_collection_item(lines_path(), removed)
        .expect("the first row should be removed");
    let appended = form.push_collection_item(lines_path(), line("Review"));
    assert!(form.move_collection_item_to_index(lines_path(), appended, 0));

    form.validate_form(ValidationTrigger::Manual);

    let targets: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| error.expect_field())
        .collect();
    assert_eq!(
        targets,
        vec![
            FieldIdentity::collection_item("lines", appended, "description"),
            FieldIdentity::collection_item("lines", survivor, "description"),
        ]
    );
    assert!(
        targets
            .iter()
            .all(|target| { target.collection_item_identity() != Some(removed) })
    );
}

#[test]
fn collection_validation_target_rule_follows_replacement_and_containing_field_reset() {
    let mut form: FormCore<CollectionPage, &'static str> =
        FormCore::new_with_error_type(CollectionPage {
            invoice: invoice_form(),
            archived: vec![line("Archived")],
        });
    let rule = CollectionValidationTargetRule::descendant(
        collection_page_lines_path(),
        line_description_path(),
    )
    .expect("a composed static collection path should be supported");
    let rule_for_validation = rule.clone();
    form.register_sync_form_validator_with_collection_target_rules(
        "external rows",
        [rule],
        move |context| {
            rule_for_validation
                .resolve(&context, 0)
                .map(|target| FormValidationError::for_target(target, "description"))
                .into_iter()
                .collect()
        },
    );
    let baseline = form.collection_items(collection_page_lines_path());

    assert!(
        form.replace_collection_item(collection_page_lines_path(), 0, line("Edited in place"),)
    );
    form.validate_form(ValidationTrigger::Manual);
    assert_eq!(
        form.validation_errors()[0]
            .expect_field()
            .collection_item_identity(),
        Some(baseline[0].identity())
    );

    form.set_field(
        collection_page_lines_path(),
        vec![line("Exact replacement")],
    );
    let exact_replacement = form.collection_items(collection_page_lines_path())[0].identity();
    assert!(
        !baseline
            .iter()
            .any(|item| item.identity() == exact_replacement)
    );

    form.set_field(
        collection_page_invoice_path(),
        InvoiceForm {
            lines: vec![line("Ancestor replacement")],
        },
    );
    let ancestor_replacement = form.collection_items(collection_page_lines_path())[0].identity();
    assert_ne!(ancestor_replacement, exact_replacement);

    form.reset_field(collection_page_invoice_path());
    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(
        form.collection_items(collection_page_lines_path()),
        baseline
    );
    assert_eq!(
        form.validation_errors()[0]
            .expect_field()
            .collection_item_identity(),
        Some(baseline[0].identity())
    );
}

#[test]
fn collection_validation_target_rule_reprepares_fresh_identities_on_reinitialize() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");
    let rule_for_validation = rule.clone();
    form.register_sync_form_validator_with_collection_target_rules(
        "external rows",
        [rule],
        move |context| {
            rule_for_validation
                .resolve(&context, 0)
                .map(|target| FormValidationError::for_target(target, "row"))
                .into_iter()
                .collect()
        },
    );
    let before = form.collection_identity_state();
    let before = before.collections()[0].current_items().to_vec();

    form.reinitialize(InvoiceForm {
        lines: vec![line("Reinitialized")],
    });

    let after = form.collection_identity_state();
    let after = after.collections()[0].current_items();
    assert_eq!(after.len(), 1);
    assert!(!before.contains(&after[0]));

    form.validate_form(ValidationTrigger::Manual);
    assert_eq!(
        form.validation_errors()[0]
            .expect_field()
            .collection_item_identity(),
        Some(after[0])
    );
}

#[test]
fn collection_validation_target_rule_resolves_a_valid_restored_identity_order() {
    let mut source: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let initial = source.collection_items(lines_path());
    source
        .remove_collection_item(lines_path(), initial[0].identity())
        .expect("the first row should be removed");
    let appended = source.push_collection_item(lines_path(), line("Review"));
    assert!(source.move_collection_item_to_index(lines_path(), appended, 0));
    let expected: Vec<_> = source
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    let snapshot = source.state_snapshot();

    let mut restored: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let rule = CollectionValidationTargetRule::descendant(lines_path(), line_description_path())
        .expect("a static item descendant should be supported");
    let rule_for_validation = rule.clone();
    restored.register_sync_form_validator_with_collection_target_rules(
        "external rows",
        [rule],
        move |context| {
            (0..context.form().lines.len())
                .filter_map(|row| rule_for_validation.resolve(&context, row))
                .map(|target| FormValidationError::for_target(target, "description"))
                .collect()
        },
    );

    restored
        .restore_state_snapshot(snapshot)
        .expect("matching draft and identity cardinalities should restore");
    restored.validate_form(ValidationTrigger::Manual);

    let resolved: Vec<_> = restored
        .validation_errors()
        .into_iter()
        .map(|error| {
            error
                .expect_field()
                .collection_item_identity()
                .expect("the rule should target a collection item")
        })
        .collect();
    assert_eq!(resolved, expected);
    assert_eq!(restored.snapshot().lines[0].description, "Review");
}

#[test]
fn collection_validation_target_rule_rejects_restore_cardinality_mismatch_atomically() {
    let source: FormCore<InvoiceForm, &'static str> = FormCore::new_with_error_type(invoice_form());
    let snapshot_without_identities = source.state_snapshot();

    let mut target: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(InvoiceForm {
            lines: vec![line("Target stays current")],
        });
    let rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");
    target.register_sync_form_validator_with_collection_target_rules(
        "external rows",
        [rule],
        |_context| Vec::new(),
    );
    let draft_before = target.snapshot();
    let identities_before = target.collection_identity_state();

    let error = target
        .restore_state_snapshot(snapshot_without_identities)
        .expect_err("registered collection cardinality must match the restored draft");

    assert_eq!(
        error,
        FormStateRestoreError::CollectionIdentityCardinalityMismatch {
            collection: FieldIdentity::new("lines"),
            sequence: CollectionIdentitySequence::Baseline,
            model_items: 2,
            identity_items: 0,
        }
    );
    assert_eq!(target.snapshot(), draft_before);
    assert_eq!(target.collection_identity_state(), identities_before);
}

#[test]
fn collection_validation_target_rule_rejects_current_restore_cardinality_mismatch() {
    let mut snapshot_source: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(InvoiceForm { lines: Vec::new() });
    snapshot_source.set_field(lines_path(), vec![line("Only current row")]);
    let mismatched_snapshot = snapshot_source.state_snapshot();

    let mut target: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");
    target.register_sync_form_validator_with_collection_target_rules(
        "external rows",
        [rule],
        |_context| Vec::new(),
    );

    let error = target
        .restore_state_snapshot(mismatched_snapshot)
        .expect_err("current identity cardinality must match the current restored draft");

    assert_eq!(
        error,
        FormStateRestoreError::CollectionIdentityCardinalityMismatch {
            collection: FieldIdentity::new("lines"),
            sequence: CollectionIdentitySequence::Current,
            model_items: 1,
            identity_items: 0,
        }
    );
}

#[test]
fn unregistering_collection_validation_target_rule_removes_obligation_without_rewinding() {
    let mut target: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");
    let validator = target.register_sync_form_validator_with_collection_target_rules(
        "external rows",
        [rule],
        |_context| Vec::new(),
    );
    let issued = target.collection_identity_state().collections()[0]
        .current_items()
        .to_vec();
    assert!(target.unregister_form_validator_by_id(validator));

    let source: FormCore<InvoiceForm, &'static str> = FormCore::new_with_error_type(invoice_form());
    target
        .restore_state_snapshot(source.state_snapshot())
        .expect("an unregistered rule should impose no restore obligation");

    let reminted: Vec<_> = target
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    assert!(reminted.iter().all(|item| !issued.contains(item)));
    assert!(reminted.iter().all(|item| *item > issued[1]));
}

#[test]
fn collection_validation_target_rule_rejects_captured_collection_and_descendant_identities() {
    let mut form = FormCore::new(invoice_form());
    let captured = form.collection_items(lines_path())[0].identity();
    let captured_collection = FieldPath::direct(
        FieldIdentity::collection_item("outer", captured, "lines"),
        "lines",
        |model: &InvoiceForm| &model.lines,
        |model: &mut InvoiceForm| &mut model.lines,
    );
    let captured_descendant = FieldPath::direct(
        FieldIdentity::collection_item("nested", captured, "description"),
        "description",
        |line: &InvoiceLine| &line.description,
        |line: &mut InvoiceLine| &mut line.description,
    );

    assert!(matches!(
        CollectionValidationTargetRule::item(captured_collection),
        Err(CollectionValidationTargetRuleError::UnsupportedCollectionIdentity { .. })
    ));
    assert!(matches!(
        CollectionValidationTargetRule::descendant(lines_path(), captured_descendant),
        Err(CollectionValidationTargetRuleError::UnsupportedDescendantIdentity { .. })
    ));
}

#[test]
fn collection_validation_target_rule_preparation_retires_submit_validation_proof() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let validation = form.submit_validation_snapshot();
    let rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");

    form.register_sync_form_validator_for_triggers_with_collection_target_rules(
        "manual external rows",
        ValidationTrigger::Manual,
        [rule],
        |context| {
            let _same_run = context.clone();
            Vec::new()
        },
    );

    assert_eq!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );
}

#[test]
fn deferred_collection_replacement_preparation_retires_submit_validation_proof() {
    let mut source: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    source.collection_items(lines_path());
    let snapshot = source.state_snapshot();

    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    form.restore_state_snapshot(snapshot)
        .expect("collection identity state should restore");
    form.set_field(lines_path(), vec![line("Replacement")]);
    let validation = form.submit_validation_snapshot();
    let rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");

    form.register_sync_form_validator_for_triggers_with_collection_target_rules(
        "manual external rows",
        ValidationTrigger::Manual,
        [rule],
        |_context| Vec::new(),
    );

    assert_eq!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );
}

#[test]
fn async_collection_target_rules_prepare_and_resolve_against_the_async_validation_addressing_snapshot()
 {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let registered_rule =
        CollectionValidationTargetRule::descendant(lines_path(), line_description_path())
            .expect("a static item descendant should be supported");
    let validator = form.register_async_form_validator_for_triggers_with_collection_target_rules(
        "external rows",
        ValidationTrigger::Manual,
        [registered_rule],
    );
    let initial = form.collection_items(lines_path());
    let run = form
        .begin_async_form_validation(validator, ValidationTrigger::Manual)
        .expect("the async validator should start");

    assert!(form.move_collection_item_to_index(lines_path(), initial[1].identity(), 0));

    let equivalent_rule =
        CollectionValidationTargetRule::descendant(lines_path(), line_description_path())
            .expect("an equivalent rule should be supported");
    let target = run
        .validator_context()
        .resolve_collection_target(&equivalent_rule, 0)
        .expect("the first validated row should resolve from the run snapshot");
    assert_eq!(
        target,
        ValidationTarget::field_identity(FieldIdentity::collection_item(
            "lines",
            initial[0].identity(),
            "description",
        ))
    );
    let unauthorized_rule =
        CollectionValidationTargetRule::descendant(lines_path(), line_quantity_path())
            .expect("another static descendant should be supported");
    assert!(
        run.validator_context()
            .resolve_collection_target(&unauthorized_rule, 0)
            .is_none()
    );
    assert_eq!(
        form.complete_async_form_validation(
            validator,
            &run,
            [FormValidationError::for_target(target, "old diagnostic")],
        ),
        None
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn async_validation_addressing_snapshot_survives_every_live_collection_identity_transition() {
    assert_async_validation_addressing_snapshot_survives_live_change(
        |_| (),
        |form, _, ()| {
            form.insert_collection_item(lines_path(), 0, line("Inserted"))
                .expect("the insertion index should be valid");
        },
    );
    assert_async_validation_addressing_snapshot_survives_live_change(
        |_| (),
        |form, items, ()| {
            form.remove_collection_item(lines_path(), items[0])
                .expect("the validated row should still be live before removal");
        },
    );
    assert_async_validation_addressing_snapshot_survives_live_change(
        |_| (),
        |form, items, ()| {
            assert!(form.move_collection_item_to_index(lines_path(), items[1], 0));
        },
    );
    assert_async_validation_addressing_snapshot_survives_live_change(
        |_| (),
        |form, _, ()| {
            assert!(form.swap_collection_items(lines_path(), 0, 1));
        },
    );
    assert_async_validation_addressing_snapshot_survives_live_change(
        |_| (),
        |form, _, ()| {
            assert!(form.replace_collection_item(lines_path(), 0, line("Replacement")));
        },
    );
    assert_async_validation_addressing_snapshot_survives_live_change(
        |_| (),
        |form, _, ()| {
            form.set_field(lines_path(), vec![line("Generic replacement")]);
        },
    );
    assert_async_validation_addressing_snapshot_survives_live_change(
        |_| (),
        |form, _, ()| {
            form.reinitialize(InvoiceForm {
                lines: vec![line("Reinitialized")],
            });
        },
    );
    assert_async_validation_addressing_snapshot_survives_live_change(
        |form| {
            form.insert_collection_item(lines_path(), 0, line("Reset away"))
                .expect("the insertion index should be valid");
        },
        |form, _, _| form.reset(),
    );
    assert_async_validation_addressing_snapshot_survives_live_change(
        |form| {
            let restore = form.state_snapshot();
            form.set_field(lines_path(), vec![line("Restored away")]);
            restore
        },
        |form, _, restore| {
            form.restore_state_snapshot(restore)
                .expect("the paired draft and identities should restore");
        },
    );
    assert_async_validation_addressing_snapshot_survives_live_change(
        |_| (),
        |form, items, ()| {
            assert!(form.move_collection_item_to_index(lines_path(), items[1], 0));
            assert!(form.move_collection_item_to_index(lines_path(), items[0], 0));
        },
    );
}

#[test]
fn rules_free_async_form_validation_captures_no_collection_addressing() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let validator =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Manual);
    assert!(form.collection_identity_state().collections().is_empty());

    let run = form
        .begin_async_form_validation(validator, ValidationTrigger::Manual)
        .expect("the async validator should start");
    let rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");

    assert!(
        run.validator_context()
            .resolve_collection_target(&rule, 0)
            .is_none()
    );
    assert!(form.collection_identity_state().collections().is_empty());
}

#[test]
fn async_collection_target_resolution_uses_registered_shapes_without_query_accessors() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let registered_rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");
    let validator = form.register_async_form_validator_for_triggers_with_collection_target_rules(
        "external rows",
        ValidationTrigger::Manual,
        [registered_rule],
    );
    let expected = form.collection_items(lines_path())[0].identity();
    let run = form
        .begin_async_form_validation(validator, ValidationTrigger::Manual)
        .expect("the async validator should start");
    let query_path: FieldPath<InvoiceForm, Vec<InvoiceLine>> = FieldPath::direct(
        FieldIdentity::new("lines"),
        "lines",
        |_: &InvoiceForm| panic!("async resolution must not read the query accessor"),
        |_: &mut InvoiceForm| panic!("async resolution must not read the query accessor"),
    );
    let query_rule = CollectionValidationTargetRule::item(query_path)
        .expect("an equivalent nominal shape should be supported");

    assert_eq!(
        run.validator_context()
            .resolve_collection_target(&query_rule, 0),
        Some(ValidationTarget::field_identity(
            FieldIdentity::collection_item_value("lines", expected),
        ))
    );
}

#[test]
fn async_collection_target_resolution_fails_closed_on_registered_cardinality_disagreement() {
    let mut form: FormCore<CollectionPage, &'static str> =
        FormCore::new_with_error_type(CollectionPage {
            invoice: invoice_form(),
            archived: vec![line("Archived")],
        });
    let collection = collection_page_lines_path();
    let dishonest_collection = FieldPath::direct(
        collection.identity(),
        "invoice.lines",
        |model: &CollectionPage| &model.archived,
        |model: &mut CollectionPage| &mut model.archived,
    );
    let registered_rule = CollectionValidationTargetRule::item(collection.clone())
        .expect("a composed static collection path should be supported");
    let disagreeing_rule = CollectionValidationTargetRule::item(dishonest_collection)
        .expect("the nominal collection identity is static");
    let validator = form.register_async_form_validator_for_triggers_with_collection_target_rules(
        "external rows",
        ValidationTrigger::Manual,
        [registered_rule, disagreeing_rule],
    );
    let run = form
        .begin_async_form_validation(validator, ValidationTrigger::Manual)
        .expect("the async validator should start");
    let query_rule = CollectionValidationTargetRule::item(collection)
        .expect("the query shape should be supported");

    assert!(
        run.validator_context()
            .resolve_collection_target(&query_rule, 0)
            .is_none()
    );
}

#[test]
fn async_collection_target_rule_preparation_retires_submit_validation_proof() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let validation = form.submit_validation_snapshot();
    let rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");

    form.register_async_form_validator_for_triggers_with_collection_target_rules(
        "manual external rows",
        ValidationTrigger::Manual,
        [rule],
    );

    assert_eq!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );
}

#[test]
fn async_collection_target_rules_capture_at_debounce_wake_and_submit_flush() {
    let rule = CollectionValidationTargetRule::descendant(lines_path(), line_description_path())
        .expect("a static item descendant should be supported");
    let triggers = ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]);

    let mut wake_form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let wake_validator = wake_form
        .register_async_form_validator_for_triggers_with_collection_target_rules(
            "wake rows",
            triggers.clone(),
            [rule.clone()],
        );
    let wake_items = wake_form.collection_items(lines_path());
    assert!(wake_form.move_collection_item_to_index(lines_path(), wake_items[1].identity(), 0));
    let scheduled = wake_form
        .schedule_debounced_async_form_validation(wake_validator, ValidationTrigger::Change)
        .expect("the debounce should schedule without starting a run");
    let wake_run = wake_form
        .begin_debounced_async_form_validation(wake_validator, &scheduled)
        .expect("the delayed run should start at timer wake");
    assert_eq!(
        wake_run
            .validator_context()
            .resolve_collection_target(&rule, 0),
        Some(ValidationTarget::field_identity(
            FieldIdentity::collection_item("lines", wake_items[1].identity(), "description"),
        ))
    );

    let mut flush_form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let flush_validator = flush_form
        .register_async_form_validator_for_triggers_with_collection_target_rules(
            "flush rows",
            triggers,
            [rule.clone()],
        );
    let flush_items = flush_form.collection_items(lines_path());
    assert!(flush_form.move_collection_item_to_index(lines_path(), flush_items[1].identity(), 0,));
    let scheduled = flush_form
        .schedule_debounced_async_form_validation(flush_validator, ValidationTrigger::Change)
        .expect("the value-change debounce should schedule");
    let flush_run = flush_form
        .flush_debounced_async_form_validation_for_trigger(
            flush_validator,
            &scheduled,
            ValidationTrigger::Submit,
        )
        .expect("submit should flush the delayed run");
    assert_eq!(flush_run.trigger(), ValidationTrigger::Submit);
    assert_eq!(
        flush_run
            .validator_context()
            .resolve_collection_target(&rule, 0),
        Some(ValidationTarget::field_identity(
            FieldIdentity::collection_item("lines", flush_items[1].identity(), "description"),
        ))
    );
}

#[test]
fn async_collection_target_capture_distinguishes_empty_from_unregistered_collections() {
    let mut form: FormCore<CollectionPage, &'static str> =
        FormCore::new_with_error_type(CollectionPage {
            invoice: InvoiceForm { lines: Vec::new() },
            archived: vec![line("Archived")],
        });
    let empty_rule = CollectionValidationTargetRule::item(collection_page_lines_path())
        .expect("a composed static collection path should be supported");
    let unrelated_rule = CollectionValidationTargetRule::item(archived_lines_path())
        .expect("an unrelated static collection path should be supported");
    let validator = form.register_async_form_validator_for_triggers_with_collection_target_rules(
        "empty rows",
        ValidationTrigger::Manual,
        [empty_rule.clone()],
    );
    let identity_state = form.collection_identity_state();
    assert_eq!(identity_state.collections().len(), 1);
    assert_eq!(
        identity_state.collections()[0].collection(),
        collection_page_lines_path().identity()
    );
    assert!(identity_state.collections()[0].current_items().is_empty());
    let run = form
        .begin_async_form_validation(validator, ValidationTrigger::Manual)
        .expect("the async validator should start");
    let context = run.validator_context();

    assert!(context.resolve_collection_target(&empty_rule, 0).is_none());
    assert!(
        context
            .resolve_collection_target(&unrelated_rule, 0)
            .is_none()
    );
}

#[test]
fn async_collection_addressing_capture_is_read_only_and_not_observable_on_the_run_token() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");
    let validator = form.register_async_form_validator_for_triggers_with_collection_target_rules(
        "manual rows",
        ValidationTrigger::Manual,
        [rule],
    );
    let submit_validation = form.submit_validation_snapshot();
    let identities_before = form.collection_identity_state();
    let run = form
        .begin_async_form_validation(validator, ValidationTrigger::Manual)
        .expect("the async validator should start");

    assert_eq!(form.collection_identity_state(), identities_before);
    assert_eq!(run, run.clone());
    assert!(!format!("{run:?}").contains("collection_addressing"));
    assert_eq!(
        form.complete_async_form_validation(
            validator,
            &run,
            Vec::<FormValidationError<&str>>::new(),
        ),
        Some(ValidationStatus::Valid)
    );
    assert!(matches!(
        form.begin_submission_after_validation(&submit_validation),
        SubmitAttempt::Started(_)
    ));
}

#[test]
fn async_validation_addressing_snapshot_is_isolated_from_later_validator_topology_changes() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let rule = CollectionValidationTargetRule::item(lines_path())
        .expect("a direct collection path should be supported");
    let first = form.register_async_form_validator_for_triggers_with_collection_target_rules(
        "first rows",
        ValidationTrigger::Manual,
        [rule.clone()],
    );
    let expected = form.collection_items(lines_path())[0].identity();
    let run = form
        .begin_async_form_validation(first, ValidationTrigger::Manual)
        .expect("the first async validator should start");

    form.register_async_form_validator_for_triggers_with_collection_target_rules(
        "second rows",
        ValidationTrigger::Manual,
        [rule.clone()],
    );
    assert_eq!(
        run.validator_context().resolve_collection_target(&rule, 0),
        Some(ValidationTarget::field_identity(
            FieldIdentity::collection_item_value("lines", expected),
        ))
    );

    assert!(form.unregister_form_validator_by_id(first));
    assert_eq!(
        run.validator_context().resolve_collection_target(&rule, 0),
        Some(ValidationTarget::field_identity(
            FieldIdentity::collection_item_value("lines", expected),
        ))
    );
    assert_eq!(
        form.complete_async_form_validation(
            first,
            &run,
            [FormValidationError::form("must not apply")],
        ),
        None
    );
    assert!(form.validation_errors().is_empty());
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Customer {
    name: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct NestedLine {
    customer: Customer,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct NestedInvoice {
    customer: Customer,
    customer_account: Customer,
    lines: Vec<NestedLine>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct NestedPage {
    invoice: NestedInvoice,
}

fn nested_invoice_path() -> FieldPath<NestedPage, NestedInvoice> {
    FieldPath::direct(
        FieldIdentity::new("invoice"),
        "invoice",
        |model: &NestedPage| &model.invoice,
        |model: &mut NestedPage| &mut model.invoice,
    )
}

fn invoice_customer_path() -> FieldPath<NestedInvoice, Customer> {
    FieldPath::direct(
        FieldIdentity::new("customer"),
        "customer",
        |invoice: &NestedInvoice| &invoice.customer,
        |invoice: &mut NestedInvoice| &mut invoice.customer,
    )
}

fn invoice_customer_account_path() -> FieldPath<NestedInvoice, Customer> {
    FieldPath::direct(
        FieldIdentity::new("customer_account"),
        "customer_account",
        |invoice: &NestedInvoice| &invoice.customer_account,
        |invoice: &mut NestedInvoice| &mut invoice.customer_account,
    )
}

fn customer_name_path() -> FieldPath<Customer, String> {
    FieldPath::direct(
        FieldIdentity::new("name"),
        "name",
        |customer: &Customer| &customer.name,
        |customer: &mut Customer| &mut customer.name,
    )
}

fn nested_customer_path() -> FieldPath<NestedPage, Customer> {
    nested_invoice_path().join(invoice_customer_path())
}

fn nested_customer_name_path() -> FieldPath<NestedPage, String> {
    nested_customer_path().join(customer_name_path())
}

fn nested_customer_account_path() -> FieldPath<NestedPage, Customer> {
    nested_invoice_path().join(invoice_customer_account_path())
}

fn nested_customer_account_name_path() -> FieldPath<NestedPage, String> {
    nested_customer_account_path().join(customer_name_path())
}

fn invoice_lines_path() -> FieldPath<NestedInvoice, Vec<NestedLine>> {
    FieldPath::direct(
        FieldIdentity::new("lines"),
        "lines",
        |invoice: &NestedInvoice| &invoice.lines,
        |invoice: &mut NestedInvoice| &mut invoice.lines,
    )
}

fn nested_invoice_lines_path() -> FieldPath<NestedPage, Vec<NestedLine>> {
    nested_invoice_path().join(invoice_lines_path())
}

fn line_customer_path() -> FieldPath<NestedLine, Customer> {
    FieldPath::direct(
        FieldIdentity::new("customer"),
        "customer",
        |line: &NestedLine| &line.customer,
        |line: &mut NestedLine| &mut line.customer,
    )
}

fn line_customer_name_path() -> FieldPath<NestedLine, String> {
    line_customer_path().join(customer_name_path())
}

fn line_field_identity_for(item: CollectionItemIdentity, field: &'static str) -> FieldIdentity {
    FieldIdentity::collection_item("invoice.lines", item, field)
}

fn nested_customer(name: &str) -> Customer {
    Customer {
        name: name.to_owned(),
    }
}

fn nested_page_with_one_line() -> NestedPage {
    NestedPage {
        invoice: NestedInvoice {
            lines: vec![NestedLine::default()],
            ..NestedInvoice::default()
        },
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

    let triggers: ValidationTriggers = [ValidationTrigger::Commit, ValidationTrigger::Commit]
        .into_iter()
        .collect();
    assert!(triggers.contains(ValidationTrigger::Commit));
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
fn validation_mode_names_match_commit_and_change_semantics() {
    assert_eq!(ValidationMode::default(), ValidationMode::on_commit());

    assert!(!ValidationMode::on_submit().validates_on_commit());
    assert!(!ValidationMode::on_submit().validates_on_change());

    assert!(ValidationMode::on_commit().validates_on_commit());
    assert!(!ValidationMode::on_commit().validates_on_change());
    assert_eq!(
        ValidationMode::on_commit_or_submit(),
        ValidationMode::on_commit()
    );

    assert!(ValidationMode::on_change().validates_on_commit());
    assert!(ValidationMode::on_change().validates_on_change());
    assert!(!ValidationMode::submit_then_revalidate().validates_on_commit());
    assert!(!ValidationMode::submit_then_revalidate().validates_on_change());
    assert!(!ValidationMode::submit_then_revalidate().should_validate_on_commit(0));
    assert!(!ValidationMode::submit_then_revalidate().should_validate_on_change(0));
    assert!(ValidationMode::submit_then_revalidate().should_validate_on_commit(1));
    assert!(ValidationMode::submit_then_revalidate().should_validate_on_change(1));

    assert!(
        ValidationMode::on_submit()
            .validate_on_commit()
            .validates_on_commit()
    );
    assert!(
        !ValidationMode::on_commit()
            .with_commit_validation(false)
            .validates_on_commit()
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
fn dirty_state_stays_a_value_comparison_across_field_ancestry() {
    let mut form = FormCore::new(NestedPage::default());

    form.set_field(nested_customer_path(), nested_customer("Ada"));

    assert!(form.is_field_dirty(nested_customer_path()));
    assert!(form.is_field_dirty(nested_customer_name_path()));
    assert!(!form.is_field_dirty(nested_customer_account_name_path()));

    form.set_field(nested_customer_name_path(), String::new());

    assert!(!form.is_field_dirty(nested_customer_path()));
    assert!(!form.is_field_dirty(nested_customer_name_path()));
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
fn reordering_collection_items_by_identity_keeps_item_state_with_the_items() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let review = form.push_collection_item(lines_path(), line("Review"));
    let items: Vec<_> = form
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    let (design, build) = (items[0], items[1]);
    let build_description = line_field_identity(build, "description");

    form.set_user_collection_item_field(
        lines_path(),
        build,
        line_description_path(),
        "Build v2".to_owned(),
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&events);
    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));

    assert!(form.reorder_user_collection_items(lines_path(), &[review, build, design]));

    let reordered: Vec<_> = form
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    assert_eq!(reordered, vec![review, build, design]);
    assert_eq!(form.snapshot().lines[0].description, "Review");
    assert_eq!(form.snapshot().lines[1].description, "Build v2");
    assert_eq!(form.snapshot().lines[2].description, "Design");
    assert!(form.is_field_identity_touched(&build_description));
    assert!(matches!(
        events.borrow().as_slice(),
        [FormObserverEvent::CollectionItemsReordered {
            order,
            origin: FieldUpdateOrigin::User,
            ..
        }] if *order == vec![review, build, design]
    ));
}

#[test]
fn replacing_a_whole_collection_through_set_field_reports_the_retired_identities() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let retired_items: Vec<_> = form
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&events);
    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));

    form.set_user_field(lines_path(), vec![line("Replacement")]);

    assert!(matches!(
        events.borrow().as_slice(),
        [
            FormObserverEvent::FieldUpdated { .. },
            FormObserverEvent::CollectionReplaced {
                retired,
                origin: FieldUpdateOrigin::User,
                ..
            },
        ] if *retired == retired_items
    ));
    // The replacement rows are fresh logical items; no retired identity is reissued.
    assert!(
        form.collection_items(lines_path())
            .iter()
            .all(|item| !retired_items.contains(&item.identity()))
    );
}

#[test]
fn reordering_collection_items_refuses_a_non_permutation_without_mutating() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let items: Vec<_> = form
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    let (design, build) = (items[0], items[1]);
    let retired = form.push_collection_item(lines_path(), line("Retired"));
    form.remove_collection_item(lines_path(), retired)
        .expect("the appended row should be removable");

    assert!(!form.reorder_collection_items(lines_path(), &[build]));
    assert!(!form.reorder_collection_items(lines_path(), &[build, design, build]));
    assert!(!form.reorder_collection_items(lines_path(), &[build, build]));
    assert!(!form.reorder_collection_items(lines_path(), &[build, retired]));
    assert!(form.reorder_collection_items(lines_path(), &[design, build]));

    let unchanged: Vec<_> = form
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    assert_eq!(unchanged, vec![design, build]);
    assert_eq!(form.snapshot().lines[0].description, "Design");
    assert!(!form.is_dirty());
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
    source.mark_field_identity_committed(&inserted_description);
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
    assert!(source.is_field_identity_committed(&inserted_description));
    assert_eq!(
        source.field_validation_errors_by_identity(&inserted_description)[0].error(),
        &"required",
    );
    assert_eq!(
        source.field_validation_errors_by_identity(&kept_quantity)[0].error(),
        &"server quantity",
    );

    let snapshot = source.state_snapshot();
    assert_eq!(snapshot.version(), 6);
    let identity_state = snapshot.collection_identity_state();
    let lines_state = identity_state
        .collections()
        .iter()
        .find(|state| state.collection() == lines.identity())
        .expect("lines collection identity should be serialized");

    assert_eq!(identity_state.version(), 2);
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
    assert!(restored.is_field_identity_committed(&inserted_description));
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
        ValidationTrigger::Commit,
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

    restored.validate_field(name_path(), ValidationTrigger::Commit);

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
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
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
    assert_eq!(
        target.begin_submission_after_validation(&stale_validation),
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );
}

#[test]
fn collection_item_removal_clears_item_scoped_state() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let item = form.collection_items(lines_path())[1].identity();
    let description = line_field_identity(item, "description");

    form.mark_collection_item_field_blurred(lines_path(), item, line_description_path());
    let validated_description = description.clone();
    let line_errors = form.register_sync_form_validator("line-errors", move |_context| {
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
    assert_eq!(
        form.form_validation_status_by_id(line_errors),
        Some(ValidationStatus::Unknown)
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
fn collection_item_field_write_clears_collection_and_written_row_verdicts_only() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let items = form.collection_items(lines_path());
    let first = items[0].identity();
    let second = items[1].identity();
    let first_quantity = line_field_identity(first, "quantity");
    let second_quantity = line_field_identity(second, "quantity");
    let lines_rule = form.register_sync_field_validator_for_triggers(
        lines_path(),
        "lines",
        ValidationTrigger::Manual,
        |_value, _context| vec!["lines"],
    );
    form.register_sync_collection_item_field_validator_for_triggers(
        lines_path(),
        line_quantity_path(),
        "quantity",
        ValidationTrigger::Manual,
        |_value, _context| vec!["quantity"],
    );
    form.validate_all(ValidationTrigger::Manual);

    assert!(form.set_user_collection_item_field(lines_path(), first, line_quantity_path(), 3,));

    assert_eq!(
        form.field_validation_status(lines_path(), lines_rule),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.field_validation_statuses_by_identity(&first_quantity)[0].status(),
        ValidationStatus::Unknown
    );
    assert_eq!(
        form.field_validation_statuses_by_identity(&second_quantity)[0].status(),
        ValidationStatus::Invalid
    );
}

#[test]
fn collection_reorder_clears_collection_verdict_but_preserves_row_verdicts() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let items = form.collection_items(lines_path());
    let first = items[0].identity();
    let second = items[1].identity();
    let first_quantity = line_field_identity(first, "quantity");
    let lines_rule = form.register_sync_field_validator_for_triggers(
        lines_path(),
        "lines",
        ValidationTrigger::Manual,
        |_value, _context| vec!["lines"],
    );
    form.register_sync_collection_item_field_validator_for_triggers(
        lines_path(),
        line_quantity_path(),
        "quantity",
        ValidationTrigger::Manual,
        |_value, _context| vec!["quantity"],
    );
    form.validate_all(ValidationTrigger::Manual);

    assert!(form.move_user_collection_item_to_index(lines_path(), second, 0));

    assert_eq!(
        form.field_validation_status(lines_path(), lines_rule),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.field_validation_statuses_by_identity(&first_quantity)[0].status(),
        ValidationStatus::Invalid
    );
}

#[test]
fn replacing_a_collection_item_clears_that_rows_verdicts_only() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let items = form.collection_items(lines_path());
    let first = items[0].identity();
    let second = items[1].identity();
    let first_quantity = line_field_identity(first, "quantity");
    let second_quantity = line_field_identity(second, "quantity");
    form.register_sync_collection_item_field_validator_for_triggers(
        lines_path(),
        line_quantity_path(),
        "quantity",
        ValidationTrigger::Manual,
        |_value, _context| vec!["quantity"],
    );
    form.validate_all(ValidationTrigger::Manual);

    assert!(form.replace_user_collection_item(lines_path(), 0, line("Review")));

    assert_eq!(
        form.field_validation_statuses_by_identity(&first_quantity)[0].status(),
        ValidationStatus::Unknown
    );
    assert_eq!(
        form.field_validation_statuses_by_identity(&second_quantity)[0].status(),
        ValidationStatus::Invalid
    );
}

#[test]
fn replacing_a_collection_field_mints_fresh_rows_and_clears_displaced_state() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let original = form.collection_items(lines_path());
    let first = original[0].identity();
    let first_quantity = line_field_identity(first, "quantity");
    form.register_sync_collection_item_field_validator_for_triggers(
        lines_path(),
        line_quantity_path(),
        "quantity",
        ValidationTrigger::Manual,
        |_value, _context| vec!["quantity"],
    );
    form.validate_all(ValidationTrigger::Manual);

    form.set_field(lines_path(), invoice_form().lines);

    let replacement = form.collection_items(lines_path());
    assert_eq!(replacement.len(), original.len());
    assert!(
        replacement
            .iter()
            .all(|item| original.iter().all(|old| old.identity() != item.identity()))
    );
    assert!(
        form.field_validation_statuses_by_identity(&first_quantity)
            .is_empty()
    );
    let replacement_quantity = line_field_identity(replacement[0].identity(), "quantity");
    assert_eq!(
        form.field_validation_statuses_by_identity(&replacement_quantity)[0].status(),
        ValidationStatus::Unknown
    );
}

#[test]
fn replacing_a_restored_collection_before_its_first_read_mints_fresh_rows() {
    let mut source: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let original: Vec<_> = source
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    let snapshot = source.state_snapshot();
    let mut restored: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    restored
        .restore_state_snapshot(snapshot)
        .expect("snapshot should restore");

    restored.set_field(lines_path(), invoice_form().lines);

    assert!(original.iter().all(|item| {
        restored
            .collection_item_index(lines_path(), *item)
            .is_none()
    }));
    let replacement_snapshot = restored.state_snapshot();
    let mut round_tripped: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    round_tripped
        .restore_state_snapshot(replacement_snapshot)
        .expect("replacement snapshot should restore");
    assert!(original.iter().all(|item| {
        round_tripped
            .collection_item_index(lines_path(), *item)
            .is_none()
    }));
    let round_tripped_replacement: Vec<_> = round_tripped
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    assert_eq!(round_tripped_replacement.len(), original.len());
    assert!(
        round_tripped_replacement
            .iter()
            .all(|item| !original.contains(item))
    );

    let replacement: Vec<_> = restored
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    assert_eq!(replacement.len(), original.len());
    assert!(replacement.iter().all(|item| !original.contains(item)));
}

#[test]
fn generic_collection_replacement_preserves_counter_and_baseline_contracts() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let baseline: Vec<_> = form
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    let transient = form.push_collection_item(lines_path(), line("Transient"));
    assert!(
        form.remove_collection_item(lines_path(), transient)
            .is_some()
    );
    let high_water = *baseline
        .iter()
        .chain(std::iter::once(&transient))
        .max()
        .expect("the collection has issued identities");
    let validation = form.submit_validation_snapshot();

    form.set_field(lines_path(), invoice_form().lines);

    let equal_length: Vec<_> = form
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    assert!(equal_length.iter().all(|item| *item > high_water));
    assert_eq!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );

    form.set_field(lines_path(), vec![line("Short")]);
    let shorter: Vec<_> = form
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    assert_eq!(shorter.len(), 1);
    assert!(shorter.iter().all(|item| !equal_length.contains(item)));

    form.set_field(lines_path(), vec![line("One"), line("Two"), line("Three")]);
    let longer: Vec<_> = form
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    assert_eq!(longer.len(), 3);
    assert!(longer.iter().all(|item| !shorter.contains(item)));
    let replacement_high_water = *longer.iter().max().expect("replacement rows exist");

    form.reset();

    let reset: Vec<_> = form
        .collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    assert_eq!(reset, baseline);
    assert!(form.push_collection_item(lines_path(), line("After reset")) > replacement_high_water);
}

#[test]
fn containing_field_replacement_reconciles_descendant_collections_only() {
    let mut form: FormCore<CollectionPage, &'static str> =
        FormCore::new_with_error_type(CollectionPage {
            invoice: invoice_form(),
            archived: vec![line("Archived")],
        });
    let current: Vec<_> = form
        .collection_items(collection_page_lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    let archived: Vec<_> = form
        .collection_items(archived_lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();

    form.set_field(
        collection_page_invoice_path(),
        InvoiceForm {
            lines: vec![line("Replacement")],
        },
    );

    let replacement: Vec<_> = form
        .collection_items(collection_page_lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    let unchanged: Vec<_> = form
        .collection_items(archived_lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect();
    assert_eq!(replacement.len(), 1);
    assert!(replacement.iter().all(|item| !current.contains(item)));
    assert_eq!(unchanged, archived);
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
fn collection_item_index_resolves_live_against_the_collection() {
    let mut form = FormCore::new(invoice_form());
    let items = form.collection_items(lines_path());
    let design = items[0].identity();
    let build = items[1].identity();

    assert_eq!(form.collection_item_index(lines_path(), design), Some(0));
    assert_eq!(form.collection_item_index(lines_path(), build), Some(1));

    form.remove_user_collection_item(lines_path(), design)
        .expect("the first line should be removed");

    assert_eq!(form.collection_item_index(lines_path(), design), None);
    assert_eq!(form.collection_item_index(lines_path(), build), Some(0));
}

#[test]
fn collection_item_index_bounds_checks_the_resolved_index_against_the_draft() {
    let mut form = FormCore::new(invoice_form());
    let build = form.collection_items(lines_path())[1].identity();

    // Writing the collection path directly mutates the draft `Vec` without touching collection
    // state, so a resolved index can outlive the item it addressed.
    form.set_field(lines_path(), Vec::new());

    assert_eq!(form.collection_item_index(lines_path(), build), None);
    assert!(
        form.collection_item_field_value(lines_path(), build, line_description_path())
            .is_none()
    );
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

    form.mark_field_committed(name_path());

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
fn field_scoped_error_views_preserve_source_category_and_registration_order() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let lines = lines_path();
    let description = line_description_path();
    let first = form.collection_items(lines.clone())[0].identity();
    let field = FieldIdentity::collection_item("lines", first, "description");

    let direct = form.register_sync_field_identity_validator_for_triggers(
        field.clone(),
        "direct",
        ValidationTrigger::Manual,
        |_model, _context| vec!["direct_first", "direct_second"],
    );
    let collection = form.register_sync_collection_item_field_validator_for_triggers(
        lines,
        description,
        "collection",
        ValidationTrigger::Manual,
        |_value, _context| vec!["collection"],
    );
    let form_rule =
        form.register_sync_form_validator_for_triggers("form", ValidationTrigger::Manual, {
            let field = field.clone();
            move |_context| vec![FormValidationError::field_identity(field.clone(), "form")]
        });

    assert_eq!(
        form.submit(|_submitted| SubmitError::field_identity(field.clone(), "submit")),
        SubmitResult::Rejected,
    );
    form.validate_all(ValidationTrigger::Manual);

    let errors: Vec<_> = form
        .field_validation_errors_by_identity(&field)
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
                Some(direct),
                ValidationTarget::Field(field.clone()),
                "direct",
                "direct_first",
            ),
            (
                Some(direct),
                ValidationTarget::Field(field.clone()),
                "direct",
                "direct_second",
            ),
            (
                Some(collection),
                ValidationTarget::Field(field.clone()),
                "collection",
                "collection",
            ),
            (
                Some(form_rule),
                ValidationTarget::Field(field.clone()),
                "form",
                "form",
            ),
            (
                None,
                ValidationTarget::Field(field.clone()),
                "submit",
                "submit",
            ),
        ]
    );
    assert!(form.has_visible_field_validation_errors_by_identity(&field));
    assert_eq!(
        form.visible_field_validation_errors_by_identity(&field)
            .into_iter()
            .map(|error| (
                error.validator_id(),
                error.target(),
                error.source().as_str(),
                *error.error(),
            ))
            .collect::<Vec<_>>(),
        errors,
    );
}

#[test]
fn visible_field_error_boolean_matches_visibility_and_submit_intent() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });

    form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Submit]),
        |_value, _context| vec!["required"],
    );
    form.validate_field(name_path(), ValidationTrigger::Manual);

    assert!(!form.has_visible_field_validation_errors(name_path()));

    form.mark_field_committed(name_path());

    assert!(form.has_visible_field_validation_errors(name_path()));
    assert!(form.has_visible_field_validation_errors_by_identity(&name_path().identity()));

    assert_eq!(
        form.intent(ContactSubmitIntent::Publish)
            .submit(|_submitted| ()),
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors),
    );
    assert!(form.has_visible_field_validation_errors_for_intent(
        name_path(),
        &ContactSubmitIntent::Publish,
    ));
    assert!(!form.has_visible_field_validation_errors_for_intent(
        name_path(),
        &ContactSubmitIntent::SaveDraft,
    ));
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
    form.validate_field_source(name_path(), "reserved", ValidationTrigger::Manual);

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
fn field_writes_clear_sync_verdicts_across_ancestry_but_not_for_siblings() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    let customer = form.register_sync_field_validator_for_triggers(
        nested_customer_path(),
        "customer",
        ValidationTrigger::Manual,
        |_value, _context| vec!["customer"],
    );
    let name = form.register_sync_field_validator_for_triggers(
        nested_customer_name_path(),
        "name",
        ValidationTrigger::Manual,
        |_value, _context| vec!["name"],
    );
    let account_name = form.register_sync_field_validator_for_triggers(
        nested_customer_account_name_path(),
        "account_name",
        ValidationTrigger::Manual,
        |_value, _context| vec!["account_name"],
    );
    form.validate_all(ValidationTrigger::Manual);

    form.set_field(nested_customer_name_path(), "Ada".to_owned());

    assert_eq!(
        form.field_validation_status(nested_customer_path(), customer),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.field_validation_status(nested_customer_name_path(), name),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.field_validation_status(nested_customer_account_name_path(), account_name),
        Some(ValidationStatus::Invalid)
    );

    form.validate_field_source(
        nested_customer_name_path(),
        "name",
        ValidationTrigger::Manual,
    );
    form.set_field(nested_customer_path(), nested_customer("Grace"));

    assert_eq!(
        form.field_validation_status(nested_customer_name_path(), name),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.field_validation_status(nested_customer_account_name_path(), account_name),
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
        ValidationTrigger::Commit,
        |_value, _context| vec!["reserved"],
    );
    form.validate_field_source(name_path(), "reserved", ValidationTrigger::Commit);

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

    form.mark_field_committed(name_path());

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
fn field_write_clears_only_completed_sync_verdicts_and_leaves_async_invalidation_in_charge() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "first".to_owned(),
        });
    let pending_async = form.register_async_field_validator_for_triggers(
        name_path(),
        "pending_async",
        ValidationTrigger::Manual,
    );
    let run = form
        .begin_async_field_validation(name_path(), pending_async, ValidationTrigger::Manual)
        .expect("async validator should start");
    let completed_sync = form.register_sync_field_validator_for_triggers(
        name_path(),
        "completed_sync",
        ValidationTrigger::Manual,
        |_value, _context| vec!["completed_sync"],
    );
    let never_run_sync = form.register_sync_field_validator_for_triggers(
        name_path(),
        "never_run_sync",
        ValidationTrigger::Commit,
        |_value, _context| vec!["never_run_sync"],
    );
    form.validate_field_source(name_path(), "completed_sync", ValidationTrigger::Manual);

    form.set_field(name_path(), "second".to_owned());

    assert_eq!(
        form.field_validation_status(name_path(), completed_sync),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.field_validation_status(name_path(), never_run_sync),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.field_validation_status(name_path(), pending_async),
        Some(ValidationStatus::Stale)
    );
    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            pending_async,
            &run,
            ["first_unavailable"],
        ),
        None
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
    let commit_runs = Rc::new(Cell::new(0));
    let account_validator_runs = Rc::clone(&account_runs);
    let commit_validator_runs = Rc::clone(&commit_runs);
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
        "commit_passwords_match",
        ValidationTrigger::Commit,
        move |_context| {
            commit_validator_runs.set(commit_validator_runs.get() + 1);
            vec![FormValidationError::field(
                confirm_password_path(),
                "commit_password_mismatch",
            )]
        },
    );

    form.validate_form(ValidationTrigger::Commit);

    assert_eq!(account_runs.get(), 0);
    assert_eq!(commit_runs.get(), 1);
    assert_eq!(
        form.form_validation_status("account"),
        Some(ValidationStatus::Unknown)
    );

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(account_runs.get(), 1);
    assert_eq!(commit_runs.get(), 1);

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
                "commit_passwords_match",
                "commit_password_mismatch",
            ),
        ]
    );

    form.validate_form(ValidationTrigger::Submit);

    assert_eq!(account_runs.get(), 2);
    assert_eq!(commit_runs.get(), 1);

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
                "commit_passwords_match",
                "commit_password_mismatch",
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
fn change_validation_on_a_contained_field_reruns_when_its_container_is_written() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default())
            .with_validation_mode(ValidationMode::on_change());

    form.register_sync_field_validator_for_triggers(
        nested_customer_name_path(),
        "required",
        ValidationTrigger::Change,
        |value, _context| {
            if value.is_empty() {
                vec!["name_required"]
            } else {
                Vec::new()
            }
        },
    );

    form.set_user_field(nested_customer_path(), Customer::default());

    assert_eq!(
        form.field_validation_errors(nested_customer_name_path())[0].error(),
        &"name_required"
    );

    form.set_user_field(nested_customer_path(), nested_customer("Ada"));

    assert!(
        form.field_validation_errors(nested_customer_name_path())
            .is_empty()
    );
}

#[test]
fn change_validation_on_a_container_reruns_when_a_field_it_contains_is_written() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default())
            .with_validation_mode(ValidationMode::on_change());

    form.register_sync_field_validator_for_triggers(
        nested_customer_path(),
        "named",
        ValidationTrigger::Change,
        |customer, _context| {
            if customer.name.is_empty() {
                vec!["customer_unnamed"]
            } else {
                Vec::new()
            }
        },
    );

    form.set_user_field(nested_customer_name_path(), String::new());

    assert_eq!(
        form.field_validation_errors(nested_customer_path())[0].error(),
        &"customer_unnamed"
    );

    form.set_user_field(nested_customer_name_path(), "Ada".to_owned());

    assert!(
        form.field_validation_errors(nested_customer_path())
            .is_empty()
    );
}

#[test]
fn change_validation_on_an_item_child_field_reruns_when_a_containing_field_is_written() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(nested_page_with_one_line())
            .with_validation_mode(ValidationMode::on_change());
    let original_item = form.collection_items(nested_invoice_lines_path())[0].identity();

    form.register_sync_collection_item_field_validator_for_triggers(
        nested_invoice_lines_path(),
        line_customer_name_path(),
        "required",
        ValidationTrigger::Change,
        |value, _context| {
            if value.is_empty() {
                vec!["name_required"]
            } else {
                Vec::new()
            }
        },
    );

    form.set_user_field(
        nested_invoice_path(),
        NestedInvoice {
            lines: vec![NestedLine::default()],
            ..NestedInvoice::default()
        },
    );

    let empty_item = form.collection_items(nested_invoice_lines_path())[0].identity();
    assert_ne!(empty_item, original_item);
    assert!(
        form.field_validation_errors_by_identity(&line_field_identity_for(
            original_item,
            "customer.name"
        ))
        .is_empty()
    );
    assert_eq!(
        form.field_validation_errors_by_identity(&line_field_identity_for(
            empty_item,
            "customer.name"
        ))[0]
            .error(),
        &"name_required"
    );

    form.set_user_field(
        nested_invoice_path(),
        NestedInvoice {
            lines: vec![NestedLine {
                customer: nested_customer("Ada"),
            }],
            ..NestedInvoice::default()
        },
    );

    let named_item = form.collection_items(nested_invoice_lines_path())[0].identity();
    assert_ne!(named_item, empty_item);
    assert!(
        form.field_validation_errors_by_identity(&line_field_identity_for(
            empty_item,
            "customer.name"
        ))
        .is_empty()
    );
    assert!(
        form.field_validation_errors_by_identity(&line_field_identity_for(
            named_item,
            "customer.name"
        ))
        .is_empty()
    );
}

#[test]
fn a_sync_failure_in_field_ancestry_does_not_skip_the_written_fields_async_validators() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());

    let availability = form.register_async_field_validator_for_triggers(
        nested_customer_path(),
        "availability",
        ValidationTrigger::Manual,
    );
    form.register_sync_field_validator_for_triggers(
        nested_customer_name_path(),
        "required",
        ValidationTrigger::Manual,
        |_value, _context| vec!["name_required"],
    );

    assert!(
        form.begin_async_field_validation(
            nested_customer_path(),
            availability,
            ValidationTrigger::Manual,
        )
        .is_some()
    );
    assert_eq!(
        form.field_validation_status(nested_customer_path(), availability),
        Some(ValidationStatus::Pending)
    );
}

#[test]
fn change_validation_on_a_sibling_outside_field_ancestry_does_not_rerun() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default())
            .with_validation_mode(ValidationMode::on_change());

    form.register_sync_field_validator_for_triggers(
        nested_customer_account_name_path(),
        "required",
        ValidationTrigger::Change,
        move |_value, _context| {
            validator_runs.set(validator_runs.get() + 1);
            Vec::new()
        },
    );

    form.set_user_field(nested_customer_path(), nested_customer("Ada"));

    assert_eq!(runs.get(), 0);
}

#[test]
fn change_validation_on_an_item_child_field_reruns_when_its_container_is_written() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(nested_page_with_one_line())
            .with_validation_mode(ValidationMode::on_change());
    let item = form.collection_items(nested_invoice_lines_path())[0].identity();

    form.register_sync_collection_item_field_validator_for_triggers(
        nested_invoice_lines_path(),
        line_customer_name_path(),
        "required",
        ValidationTrigger::Change,
        |value, _context| {
            if value.is_empty() {
                vec!["name_required"]
            } else {
                Vec::new()
            }
        },
    );

    assert!(form.set_user_collection_item_field(
        nested_invoice_lines_path(),
        item,
        line_customer_path(),
        Customer::default(),
    ));

    assert_eq!(
        form.field_validation_errors_by_identity(&line_field_identity_for(item, "customer.name"))
            [0]
        .error(),
        &"name_required"
    );

    assert!(form.set_user_collection_item_field(
        nested_invoice_lines_path(),
        item,
        line_customer_path(),
        nested_customer("Ada"),
    ));

    assert!(
        form.field_validation_errors_by_identity(&line_field_identity_for(item, "customer.name"))
            .is_empty()
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
fn submit_then_revalidate_mode_runs_commit_validation_after_submit_attempt() {
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
        ValidationTrigger::Commit,
        move |value, context| {
            validator_runs.set(validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Commit);

            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        },
    );

    form.commit_field(name_path());

    assert_eq!(runs.get(), 0);
    assert_eq!(
        form.validation_status(name_path(), "required"),
        Some(ValidationStatus::Unknown)
    );

    assert!(form.validate_for_submit());
    form.commit_field(name_path());

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

    form.mark_field_committed(email_path());

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
fn default_visible_errors_wait_for_commit_or_submit_attempt() {
    let mut commit_form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });
    assert_eq!(
        commit_form.error_visibility_policy(),
        ErrorVisibilityPolicy::CommitOrSubmit
    );
    commit_form.register_sync_field_validator(name_path(), "required", |_value, _context| {
        vec!["required"]
    });
    commit_form.validate_field(name_path(), ValidationTrigger::Manual);

    assert_eq!(commit_form.validation_errors().len(), 1);
    assert!(commit_form.visible_validation_errors().is_empty());

    commit_form.mark_field_committed(name_path());

    assert_eq!(
        commit_form.visible_validation_errors()[0].error(),
        &"required"
    );
    assert!(commit_form.is_field_committed(name_path()));
    assert!(!commit_form.is_field_touched(name_path()));
    assert!(!commit_form.is_field_blurred(name_path()));

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
fn blurring_a_contained_field_reveals_its_container_error_without_blurring_the_container() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default())
            .with_error_visibility_policy(ErrorVisibilityPolicy::BlurOrSubmit);
    form.register_sync_field_validator(
        nested_customer_path(),
        "customer_invalid",
        |_customer, _context| vec!["customer_invalid"],
    );
    form.validate_field(nested_customer_path(), ValidationTrigger::Manual);

    form.mark_field_blurred(nested_customer_name_path());

    assert_eq!(
        form.visible_field_validation_errors(nested_customer_path())[0].error(),
        &"customer_invalid"
    );
    assert!(!form.is_field_blurred(nested_customer_path()));
}

#[test]
fn committing_a_contained_field_reveals_its_container_error_without_committing_the_container() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    form.register_sync_field_validator(
        nested_customer_path(),
        "customer_invalid",
        |_customer, _context| vec!["customer_invalid"],
    );
    form.validate_field(nested_customer_path(), ValidationTrigger::Manual);

    form.mark_field_committed(nested_customer_name_path());

    assert_eq!(
        form.visible_field_validation_errors(nested_customer_path())[0].error(),
        &"customer_invalid"
    );
    assert!(!form.is_field_committed(nested_customer_path()));
}

#[test]
fn committing_a_field_runs_its_own_validator_without_blurring_it() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });
    form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTrigger::Commit,
        |_name, _context| vec!["required"],
    );

    form.commit_field(name_path());

    assert_eq!(
        form.field_validation_errors(name_path())[0].error(),
        &"required"
    );
    assert!(form.is_field_committed(name_path()));
    assert!(!form.is_field_blurred(name_path()));
}

#[test]
fn committing_a_container_does_not_run_validators_on_fields_it_contains() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    form.register_sync_field_validator(
        nested_customer_name_path(),
        "name_invalid",
        |_name, _context| vec!["name_invalid"],
    );

    form.commit_field(nested_customer_path());

    assert!(
        form.field_validation_errors(nested_customer_name_path())
            .is_empty()
    );
    assert!(form.can_submit());
}

#[test]
fn manual_validation_on_a_container_runs_validators_on_fields_it_contains() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    form.register_sync_field_validator_for_triggers(
        nested_customer_name_path(),
        "name_invalid",
        ValidationTrigger::Manual,
        |_name, _context| vec!["name_invalid"],
    );

    form.validate_field(nested_customer_path(), ValidationTrigger::Manual);

    assert_eq!(
        form.field_validation_errors(nested_customer_name_path())[0].error(),
        &"name_invalid"
    );
}

#[test]
fn committing_a_collection_item_container_does_not_run_its_descendant_validators() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(nested_page_with_one_line());
    let item = form.collection_items(nested_invoice_lines_path())[0].identity();
    form.register_sync_collection_item_field_validator_for_triggers(
        nested_invoice_lines_path(),
        line_customer_name_path(),
        "name_invalid",
        ValidationTrigger::Commit,
        |_name, _context| vec!["name_invalid"],
    );

    assert!(form.commit_collection_item_field(
        nested_invoice_lines_path(),
        item,
        line_customer_path(),
    ));

    assert!(
        form.field_validation_errors_by_identity(&line_field_identity_for(item, "customer.name"))
            .is_empty()
    );
}

#[test]
fn committing_a_collection_field_does_not_run_its_item_validators() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    form.register_sync_collection_item_field_validator_for_triggers(
        lines_path(),
        line_description_path(),
        "description_invalid",
        ValidationTrigger::Commit,
        move |_description, _context| {
            validator_runs.set(validator_runs.get() + 1);
            vec!["description_invalid"]
        },
    );

    form.commit_field(lines_path());

    assert_eq!(runs.get(), 0);
}

#[test]
fn committing_a_collection_item_field_runs_its_collection_validator() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let item = form.collection_items(lines_path())[0].identity();
    form.register_sync_field_validator_for_triggers(
        lines_path(),
        "lines_invalid",
        ValidationTrigger::Commit,
        |_lines, _context| vec!["lines_invalid"],
    );

    assert!(form.commit_collection_item_field(lines_path(), item, line_description_path(),));

    assert_eq!(
        form.field_validation_errors(lines_path())[0].error(),
        &"lines_invalid"
    );
}

#[test]
fn touched_visibility_reaches_containers_but_not_contained_fields() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default())
            .with_error_visibility_policy(ErrorVisibilityPolicy::TouchedOrSubmit);
    form.register_sync_field_validator(
        nested_customer_path(),
        "customer_invalid",
        |_customer, _context| vec!["customer_invalid"],
    );
    form.register_sync_field_validator(
        nested_customer_account_name_path(),
        "account_name_invalid",
        |_name, _context| vec!["account_name_invalid"],
    );
    form.validate_all(ValidationTrigger::Manual);

    form.mark_field_touched(nested_customer_name_path());
    form.mark_field_touched(nested_customer_account_path());

    assert_eq!(
        form.visible_field_validation_errors(nested_customer_path())[0].error(),
        &"customer_invalid"
    );
    assert!(
        form.visible_field_validation_errors(nested_customer_account_name_path())
            .is_empty()
    );
    assert!(!form.is_field_touched(nested_customer_path()));
}

#[test]
fn committing_a_collection_item_field_reveals_the_collection_error_but_not_sibling_item_errors() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let items = form.collection_items(lines_path());
    let first = items[0].identity();
    let second = items[1].identity();
    form.register_sync_collection_item_field_validator(
        lines_path(),
        line_description_path(),
        "description_invalid",
        |_description, _context| vec!["description_invalid"],
    );
    form.register_sync_form_validator_for_triggers(
        "lines_invalid",
        ValidationTrigger::Commit,
        |_context| vec![FormValidationError::field(lines_path(), "lines_invalid")],
    );
    form.validate_all(ValidationTrigger::Manual);

    assert!(form.commit_collection_item_field(lines_path(), first, line_description_path(),));

    assert_eq!(
        form.visible_field_validation_errors(lines_path())[0].error(),
        &"lines_invalid"
    );
    assert_eq!(
        form.visible_field_validation_errors_by_identity(&line_field_identity(
            first,
            "description"
        ))[0]
            .error(),
        &"description_invalid"
    );
    assert!(
        form.visible_field_validation_errors_by_identity(&line_field_identity(
            second,
            "description"
        ))
        .is_empty()
    );
}

#[test]
fn field_interaction_does_not_reveal_form_errors_before_submit() {
    for policy in [
        ErrorVisibilityPolicy::CommitOrSubmit,
        ErrorVisibilityPolicy::BlurOrSubmit,
        ErrorVisibilityPolicy::TouchedOrSubmit,
    ] {
        let mut form: FormCore<ContactForm, &'static str> =
            FormCore::new_with_error_type(ContactForm {
                name: String::new(),
            })
            .with_error_visibility_policy(policy);
        form.register_sync_form_validator_for_triggers(
            "form_invalid",
            ValidationTrigger::Manual,
            |_context| vec![FormValidationError::form("form_invalid")],
        );
        form.validate_all(ValidationTrigger::Manual);

        match policy {
            ErrorVisibilityPolicy::CommitOrSubmit => {
                form.mark_field_committed(name_path());
            }
            ErrorVisibilityPolicy::BlurOrSubmit => {
                form.mark_field_blurred(name_path());
            }
            ErrorVisibilityPolicy::TouchedOrSubmit => form.mark_field_touched(name_path()),
            _ => unreachable!(),
        }

        assert!(form.visible_form_validation_errors().is_empty());
    }
}

#[test]
fn error_visibility_policy_controls_visible_error_selectors() {
    let mut blur_only_form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        })
        .with_error_visibility_policy(ErrorVisibilityPolicy::BlurOrSubmit);
    blur_only_form.register_sync_field_validator(name_path(), "required", |_value, _context| {
        vec!["required"]
    });
    blur_only_form.validate_field(name_path(), ValidationTrigger::Manual);
    blur_only_form.mark_field_committed(name_path());

    assert!(
        blur_only_form
            .visible_field_validation_errors(name_path())
            .is_empty()
    );

    blur_only_form.mark_field_blurred(name_path());

    assert_eq!(
        blur_only_form.visible_field_validation_errors(name_path())[0].error(),
        &"required"
    );

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
        "commit_hint",
        ValidationTrigger::Commit,
        |_value, context| {
            assert_eq!(context.trigger(), ValidationTrigger::Commit);
            vec!["commit_hint"]
        },
    );

    form.validate_field(name_path(), ValidationTrigger::Change);

    assert_eq!(form.field_validation_errors(name_path()).len(), 1);
    assert!(form.visible_field_validation_errors(name_path()).is_empty());

    form.commit_field(name_path());

    let visible_errors: Vec<_> = form
        .visible_field_validation_errors(name_path())
        .into_iter()
        .map(|error| (error.source().as_str(), *error.error()))
        .collect();
    assert_eq!(
        visible_errors,
        vec![
            ("change_required", "required"),
            ("commit_hint", "commit_hint")
        ]
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
fn accepted_submission_exposes_its_typed_in_flight_intent() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });

    assert_eq!(form.in_flight_submit_intent::<ContactSubmitIntent>(), None);
    assert!(matches!(
        form.intent(ContactSubmitIntent::Publish).begin_submission(),
        SubmitAttempt::Started(_)
    ));

    assert_eq!(
        form.in_flight_submit_intent::<ContactSubmitIntent>(),
        Some(ContactSubmitIntent::Publish)
    );
    assert_eq!(form.in_flight_submit_intent::<String>(), None);
    assert!(form.is_submitting());
    assert!(form.intent(ContactSubmitIntent::Publish).is_in_flight());
    assert!(!form.intent(ContactSubmitIntent::SaveDraft).is_in_flight());
    assert_eq!(
        form.intent(ContactSubmitIntent::SaveDraft).availability(),
        dioform_core::SubmitAvailability::blocked_by([SubmitBlocker::InFlightSubmission,])
    );

    assert!(form.finish_submission());
    assert_eq!(form.in_flight_submit_intent::<ContactSubmitIntent>(), None);
}

#[test]
fn core_state_replacement_clears_the_in_flight_submit_intent() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let snapshot = form.state_snapshot();

    assert_eq!(form.in_flight_submit_intent::<ContactSubmitIntent>(), None);

    assert!(matches!(
        form.intent(ContactSubmitIntent::Publish).begin_submission(),
        SubmitAttempt::Started(_)
    ));
    form.reset();
    assert_eq!(form.in_flight_submit_intent::<ContactSubmitIntent>(), None);

    assert!(matches!(
        form.intent(ContactSubmitIntent::Publish).begin_submission(),
        SubmitAttempt::Started(_)
    ));
    form.reinitialize(ContactForm {
        name: "Grace".to_owned(),
    });
    assert_eq!(form.in_flight_submit_intent::<ContactSubmitIntent>(), None);

    assert!(matches!(
        form.intent(ContactSubmitIntent::Publish).begin_submission(),
        SubmitAttempt::Started(_)
    ));
    form.restore_state_snapshot(snapshot)
        .expect("fresh state snapshot should restore");
    assert_eq!(form.in_flight_submit_intent::<ContactSubmitIntent>(), None);
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
fn second_submit_intent_starts_an_independent_async_validation_run() {
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
fn latest_sync_submit_intent_run_replaces_the_previous_intents_verdict() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });
    form.register_sync_form_validator_for_triggers(
        "publish_name_required",
        ValidationTrigger::Submit,
        |context| {
            if context.submit_intent::<ContactSubmitIntent>() == Some(&ContactSubmitIntent::Publish)
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

    // Issue #59 and ADR-0039 deliberately pin last-run-wins across submit intents.
    assert_eq!(
        form.intent(ContactSubmitIntent::Publish)
            .submit(|_submitted| ()),
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert_eq!(
        form.intent(ContactSubmitIntent::Publish)
            .availability()
            .blockers(),
        &[SubmitBlocker::ValidationErrors]
    );

    assert_eq!(
        form.intent(ContactSubmitIntent::SaveDraft)
            .submit(|_submitted| ()),
        SubmitResult::Succeeded
    );
    assert!(
        form.intent(ContactSubmitIntent::SaveDraft)
            .availability()
            .is_available()
    );
    assert!(
        form.intent(ContactSubmitIntent::Publish)
            .availability()
            .is_available()
    );

    assert_eq!(
        form.intent(ContactSubmitIntent::Publish)
            .submit(|_submitted| ()),
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
}

#[test]
fn latest_async_submit_intent_run_replaces_the_previous_intents_verdict() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });
    let id =
        form.register_async_form_validator_for_triggers("publish_async", ValidationTrigger::Submit);

    // Issue #59 and ADR-0039 deliberately pin last-run-wins across submit intents.
    assert!(
        !form
            .intent(ContactSubmitIntent::Publish)
            .validate_for_submit()
    );
    let publish_run = form
        .begin_async_form_validation_after_sync(id, ValidationTrigger::Submit)
        .expect("publish submit async validation should start");
    form.complete_async_form_validation(
        id,
        &publish_run,
        [FormValidationError::field(name_path(), "publish_error")],
    );
    assert_eq!(
        form.intent(ContactSubmitIntent::Publish)
            .availability()
            .blockers(),
        &[SubmitBlocker::ValidationErrors]
    );

    assert!(
        !form
            .intent(ContactSubmitIntent::SaveDraft)
            .validate_for_submit()
    );
    let save_run = form
        .begin_async_form_validation_after_sync(id, ValidationTrigger::Submit)
        .expect("save draft submit async validation should start");
    form.complete_async_form_validation(id, &save_run, Vec::<FormValidationError<&str>>::new());
    assert!(
        form.intent(ContactSubmitIntent::SaveDraft)
            .availability()
            .is_available()
    );
    assert!(
        form.intent(ContactSubmitIntent::Publish)
            .availability()
            .is_available()
    );

    assert!(
        !form
            .intent(ContactSubmitIntent::Publish)
            .validate_for_submit()
    );
    let next_publish_run = form
        .begin_async_form_validation_after_sync(id, ValidationTrigger::Submit)
        .expect("next publish attempt should start fresh async validation");
    assert_eq!(
        next_publish_run
            .validator_context()
            .submit_intent::<ContactSubmitIntent>(),
        Some(&ContactSubmitIntent::Publish)
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
        "name_required_on_commit",
        ValidationTrigger::Commit,
        |value, _context| {
            if value.is_empty() {
                vec!["name_required"]
            } else {
                Vec::new()
            }
        },
    );

    form.validate_field(name_path(), ValidationTrigger::Commit);

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
    form.mark_field_committed(name_path());

    assert!(!form.can_submit());
    assert_eq!(form.visible_field_validation_errors(name_path()).len(), 1);

    form.set_user_field(name_path(), "Ada".to_owned());

    assert_eq!(
        form.validation_status(name_path(), "manual_hint"),
        Some(ValidationStatus::Unknown)
    );
    assert!(form.validation_errors().is_empty());
    assert!(form.visible_field_validation_errors(name_path()).is_empty());
    assert!(form.can_submit());

    let result = form.submit(|submitted| {
        assert_eq!(submitted.value().name, "Ada");
        called.set(true);
    });

    assert_eq!(result, SubmitResult::Succeeded);
    assert!(called.get());
    assert_eq!(
        form.validation_status(name_path(), "manual_hint"),
        Some(ValidationStatus::Unknown)
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
fn submit_validation_token_retires_when_validator_visible_metadata_changes() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    form.register_sync_field_validator_for_triggers(
        name_path(),
        "untouched",
        ValidationTrigger::Submit,
        |_value, context| {
            context
                .field_metadata()
                .is_touched()
                .then_some("already_touched")
                .into_iter()
                .collect()
        },
    );
    let validation = form.submit_validation_snapshot();

    assert!(form.validate_for_submit());
    form.mark_field_touched(name_path());

    assert_eq!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );
}

#[test]
fn submit_validation_token_retires_when_manual_validation_supersedes_its_evidence() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Submit]),
        |_value, _context| Vec::new(),
    );
    let validation = form.submit_validation_snapshot();
    assert!(form.validate_for_submit());

    form.validate_field(name_path(), ValidationTrigger::Manual);

    assert_eq!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );
}

#[test]
fn retired_submit_validation_token_outranks_superseding_manual_async_work() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTriggers::new([ValidationTrigger::Manual, ValidationTrigger::Submit]),
    );
    let validation = form.submit_validation_snapshot();
    assert!(!form.validate_for_submit());
    let submit_run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Submit)
        .expect("submit validation should start");
    assert_eq!(
        form.complete_async_field_validation(
            name_path(),
            availability,
            &submit_run,
            Vec::<&str>::new(),
        ),
        Some(ValidationStatus::Valid)
    );

    form.begin_async_field_validation(name_path(), availability, ValidationTrigger::Manual)
        .expect("manual validation should supersede submit evidence");

    assert_eq!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );
}

#[test]
fn managed_submit_continuation_distinguishes_eligible_and_ineligible_retirement() {
    let initial = ContactForm {
        name: "Ada".to_owned(),
    };

    let mut eligible: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(initial.clone());
    let before_write = eligible.submit_validation_snapshot();
    eligible.set_field(name_path(), "Grace".to_owned());
    let after_write = eligible.submit_validation_snapshot();
    assert!(before_write.permits_managed_submit_continuation(&after_write));

    let mut ineligible: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(initial.clone());
    let before_touch = ineligible.submit_validation_snapshot();
    ineligible.mark_field_touched(name_path());
    let after_touch = ineligible.submit_validation_snapshot();
    assert!(!before_touch.permits_managed_submit_continuation(&after_touch));

    let mut mixed: FormCore<ContactForm, &'static str> = FormCore::new_with_error_type(initial);
    let before_mixed = mixed.submit_validation_snapshot();
    mixed.set_field(name_path(), "Grace".to_owned());
    mixed.mark_field_touched(name_path());
    let after_mixed = mixed.submit_validation_snapshot();
    assert!(!before_mixed.permits_managed_submit_continuation(&after_mixed));
}

#[test]
fn managed_submit_continuation_accepts_successful_collection_operations() {
    let mut form = FormCore::new(invoice_form());
    let items = line_identities(&mut form);

    let before_item_field = form.submit_validation_snapshot();
    assert!(form.set_user_collection_item_field(
        lines_path(),
        items[0],
        line_description_path(),
        "Architecture".to_owned(),
    ));
    let after_item_field = form.submit_validation_snapshot();
    assert!(before_item_field.permits_managed_submit_continuation(&after_item_field));

    let before_item = form.submit_validation_snapshot();
    assert!(form.replace_collection_item(lines_path(), 0, line("Review")));
    let after_item = form.submit_validation_snapshot();
    assert!(before_item.permits_managed_submit_continuation(&after_item));

    let before_insert = form.submit_validation_snapshot();
    let inserted = form
        .insert_user_collection_item(lines_path(), 1, line("Ship"))
        .expect("insertion should succeed");
    let after_insert = form.submit_validation_snapshot();
    assert!(before_insert.permits_managed_submit_continuation(&after_insert));

    let before_move = form.submit_validation_snapshot();
    assert!(form.move_collection_item_to_index(lines_path(), inserted, 0));
    let after_move = form.submit_validation_snapshot();
    assert!(before_move.permits_managed_submit_continuation(&after_move));

    let before_swap = form.submit_validation_snapshot();
    assert!(form.swap_user_collection_items(lines_path(), 0, 1));
    let after_swap = form.submit_validation_snapshot();
    assert!(before_swap.permits_managed_submit_continuation(&after_swap));

    let before_remove = form.submit_validation_snapshot();
    assert!(
        form.remove_collection_item(lines_path(), inserted)
            .is_some()
    );
    let after_remove = form.submit_validation_snapshot();
    assert!(before_remove.permits_managed_submit_continuation(&after_remove));

    let before_clear = form.submit_validation_snapshot();
    assert!(!form.clear_user_collection_items(lines_path()).is_empty());
    let after_clear = form.submit_validation_snapshot();
    assert!(before_clear.permits_managed_submit_continuation(&after_clear));
}

#[test]
fn managed_submit_continuation_validation_stays_within_one_submit_attempt() {
    let validation_runs = Rc::new(Cell::new(0));
    let validation_runs_for_validator = Rc::clone(&validation_runs);
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTrigger::Submit,
        move |_value, _context| {
            validation_runs_for_validator.set(validation_runs_for_validator.get() + 1);
            Vec::new()
        },
    );
    assert!(form.validate_for_submit());
    let validation = form.submit_validation_snapshot();

    assert!(form.intent(()).validate_for_submit_same_attempt());

    assert_eq!(validation_runs.get(), 2);
    assert_eq!(form.submit_attempt_count(), 1);
    assert_eq!(form.last_submit_status(), None);
    assert!(
        form.begin_submission_after_validation(&validation)
            .is_started()
    );
}

#[test]
fn submit_validation_token_tracks_submit_validator_topology_only() {
    let initial = ContactForm {
        name: "Ada".to_owned(),
    };
    let mut submit_form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(initial.clone());
    let submit_validation = submit_form.submit_validation_snapshot();

    submit_form.register_sync_field_validator_for_triggers(
        name_path(),
        "submit",
        ValidationTrigger::Submit,
        |_value, _context| Vec::new(),
    );

    assert_eq!(
        submit_form.begin_submission_after_validation(&submit_validation),
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );

    let mut manual_form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(initial);
    let manual_validation = manual_form.submit_validation_snapshot();
    manual_form.register_sync_field_validator_for_triggers(
        name_path(),
        "manual",
        ValidationTrigger::Manual,
        |_value, _context| Vec::new(),
    );

    assert!(matches!(
        manual_form.begin_submission_after_validation(&manual_validation),
        SubmitAttempt::Started(_)
    ));
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
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );
    assert_eq!(
        reset_form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::StaleSubmitValidation))
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
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );
    assert_eq!(
        reinitialized_form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::StaleSubmitValidation))
    );
}

#[test]
fn retired_submit_validation_token_is_an_outcome_only_blocker_without_validators() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Ada".to_owned(),
        });
    let validation = form
        .intent(ContactSubmitIntent::Publish)
        .validation_snapshot();

    form.set_user_field(name_path(), "Grace".to_owned());

    assert_eq!(
        form.intent(ContactSubmitIntent::Publish)
            .begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );
    assert_eq!(
        form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::StaleSubmitValidation))
    );
    assert_eq!(
        form.intent(ContactSubmitIntent::Publish).last_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::StaleSubmitValidation))
    );
    assert!(form.validation_errors().is_empty());
    assert!(form.submit_availability().is_available());
    assert!(
        form.intent(ContactSubmitIntent::Publish)
            .availability()
            .is_available()
    );
    assert!(
        !form
            .submit_availability()
            .contains(SubmitBlocker::StaleSubmitValidation)
    );
    assert!(
        !form
            .intent(ContactSubmitIntent::Publish)
            .availability()
            .contains(SubmitBlocker::StaleSubmitValidation)
    );
}

#[test]
fn submit_validation_errors_outrank_a_retired_submit_validation_token() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });
    form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTrigger::Submit,
        |value, _context| value.is_empty().then_some("required").into_iter().collect(),
    );
    let validation = form.submit_validation_snapshot();

    form.set_user_field(name_path(), String::new());
    assert!(!form.validate_for_submit());

    assert_eq!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::ValidationErrors)
    );
}

#[test]
fn submit_skipped_async_validator_is_settled_for_its_sync_chain() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: String::new(),
        });
    form.register_sync_field_validator_for_triggers(
        name_path(),
        "required",
        ValidationTrigger::Submit,
        |value, _context| value.is_empty().then_some("required").into_iter().collect(),
    );
    let availability = form.register_async_field_validator_for_triggers(
        name_path(),
        "availability",
        ValidationTrigger::Submit,
    );

    assert_eq!(
        form.begin_submission(),
        SubmitAttempt::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Skipped)
    );

    let skipped_validation = form.submit_validation_snapshot();
    assert!(!form.validate_for_submit());
    assert_eq!(
        form.begin_submission_after_validation(&skipped_validation),
        SubmitAttempt::Blocked(SubmitBlocker::ValidationErrors)
    );

    form.set_user_field(name_path(), "Ada".to_owned());
    let passing_validation = form.submit_validation_snapshot();
    assert!(!form.validate_for_submit());
    assert_eq!(
        form.field_validation_status(name_path(), availability),
        Some(ValidationStatus::Pending)
    );
    let run = form
        .begin_async_field_validation(name_path(), availability, ValidationTrigger::Submit)
        .expect("async validator should run after the synchronous error clears");
    assert_eq!(
        form.complete_async_field_validation(name_path(), availability, &run, Vec::<&str>::new()),
        Some(ValidationStatus::Valid)
    );

    assert!(matches!(
        form.begin_submission_after_validation(&passing_validation),
        SubmitAttempt::Started(_)
    ));
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
fn writing_a_containing_field_clears_stale_submit_errors_for_the_fields_it_contains() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());

    assert_eq!(
        form.submit(|_submitted| vec![SubmitError::field_identity(
            nested_customer_name_path().identity(),
            "name_rejected",
        )]),
        SubmitResult::Rejected
    );

    form.set_user_field(nested_customer_path(), nested_customer("Ada"));

    assert!(
        form.field_validation_errors(nested_customer_name_path())
            .is_empty()
    );
}

#[test]
fn writing_a_contained_field_clears_stale_submit_errors_for_the_field_containing_it() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());

    assert_eq!(
        form.submit(|_submitted| vec![SubmitError::field_identity(
            nested_customer_path().identity(),
            "customer_rejected",
        )]),
        SubmitResult::Rejected
    );

    form.set_user_field(nested_customer_name_path(), "Ada".to_owned());

    assert!(
        form.field_validation_errors(nested_customer_path())
            .is_empty()
    );
    assert!(
        !form
            .submit_availability()
            .blockers()
            .contains(&SubmitBlocker::ValidationErrors)
    );
}

#[test]
fn writing_a_field_leaves_submit_errors_for_a_sibling_outside_field_ancestry() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());

    assert_eq!(
        form.submit(|_submitted| vec![SubmitError::field_identity(
            nested_customer_account_name_path().identity(),
            "account_rejected",
        )]),
        SubmitResult::Rejected
    );

    form.set_user_field(nested_customer_path(), nested_customer("Ada"));

    assert_eq!(
        form.field_validation_errors(nested_customer_account_name_path())[0].error(),
        &"account_rejected"
    );
}

#[test]
fn in_flight_submit_errors_are_discarded_when_a_field_in_ancestry_with_the_target_changed() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());

    let submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    form.set_user_field(nested_customer_name_path(), "Ada".to_owned());

    assert!(form.finish_submission_with_errors(
        submitted,
        vec![SubmitError::field_identity(
            nested_customer_path().identity(),
            "customer_rejected",
        )],
    ));

    assert!(
        form.field_validation_errors(nested_customer_path())
            .is_empty()
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn in_flight_submit_errors_survive_a_change_to_a_field_outside_the_target_ancestry() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());

    let submitted = match form.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    form.set_user_field(nested_customer_account_name_path(), "Ada".to_owned());

    assert!(form.finish_submission_with_errors(
        submitted,
        vec![SubmitError::field_identity(
            nested_customer_path().identity(),
            "customer_rejected",
        )],
    ));

    assert_eq!(
        form.field_validation_errors(nested_customer_path())[0].error(),
        &"customer_rejected"
    );
}

#[test]
fn writing_an_item_child_field_clears_stale_submit_errors_for_the_fields_it_contains() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(nested_page_with_one_line());
    let item = form.collection_items(nested_invoice_lines_path())[0].identity();

    assert_eq!(
        form.submit(|_submitted| vec![SubmitError::field_identity(
            line_field_identity_for(item, "customer.name"),
            "name_rejected",
        )]),
        SubmitResult::Rejected
    );

    assert!(form.set_user_collection_item_field(
        nested_invoice_lines_path(),
        item,
        line_customer_path(),
        nested_customer("Ada"),
    ));

    assert!(form.validation_errors().is_empty());
    assert!(
        !form
            .submit_availability()
            .blockers()
            .contains(&SubmitBlocker::ValidationErrors)
    );
}

#[test]
fn writing_a_whole_item_value_clears_stale_submit_errors_for_its_child_fields() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(nested_page_with_one_line());
    let item = form.collection_items(nested_invoice_lines_path())[0].identity();

    assert_eq!(
        form.submit(|_submitted| vec![SubmitError::field_identity(
            line_field_identity_for(item, "customer.name"),
            "name_rejected",
        )]),
        SubmitResult::Rejected
    );

    form.record_field_identity_user_change(&FieldIdentity::collection_item_value(
        "invoice.lines",
        item,
    ));

    assert!(form.validation_errors().is_empty());
}

#[test]
fn writing_an_item_child_field_clears_stale_submit_errors_for_the_whole_item_value() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(nested_page_with_one_line());
    let item = form.collection_items(nested_invoice_lines_path())[0].identity();

    assert_eq!(
        form.submit(|_submitted| vec![SubmitError::field_identity(
            FieldIdentity::collection_item_value("invoice.lines", item),
            "line_rejected",
        )]),
        SubmitResult::Rejected
    );

    assert!(form.set_user_collection_item_field(
        nested_invoice_lines_path(),
        item,
        line_customer_name_path(),
        "Ada".to_owned(),
    ));

    assert!(form.validation_errors().is_empty());
}

#[test]
fn writing_a_field_containing_a_collection_clears_stale_submit_errors_for_its_items() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(nested_page_with_one_line());
    let item = form.collection_items(nested_invoice_lines_path())[0].identity();

    assert_eq!(
        form.submit(|_submitted| vec![SubmitError::field_identity(
            line_field_identity_for(item, "customer.name"),
            "name_rejected",
        )]),
        SubmitResult::Rejected
    );

    form.set_user_field(nested_invoice_path(), NestedInvoice::default());

    assert!(form.validation_errors().is_empty());
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
            assert_eq!(context.trigger(), ValidationTrigger::Commit);
            field_order.borrow_mut().push("field");
            Vec::new()
        },
    );
    form.register_sync_form_validator("passwords_match", move |context| {
        assert_eq!(context.trigger(), ValidationTrigger::Commit);
        assert_eq!(form_order.borrow().as_slice(), &["field"]);
        form_order.borrow_mut().push("form");
        Vec::new()
    });

    form.validate_field(password_path(), ValidationTrigger::Commit);

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
fn field_write_clears_related_form_validator_errors_and_collapses_empty_source_to_unknown() {
    let mut form: FormCore<RegistrationForm, &'static str> =
        FormCore::new_with_error_type(RegistrationForm {
            email: "ada@example.com".to_owned(),
            password: "secret".to_owned(),
            confirm_password: String::new(),
        });
    let mixed = form.register_sync_form_validator_for_triggers(
        "mixed",
        ValidationTrigger::Manual,
        |_context| {
            vec![
                FormValidationError::field(confirm_password_path(), "confirm"),
                FormValidationError::field(email_path(), "email"),
                FormValidationError::form("form"),
            ]
        },
    );
    let confirm_only = form.register_sync_form_validator_for_triggers(
        "confirm_only",
        ValidationTrigger::Manual,
        |_context| vec![FormValidationError::field(confirm_password_path(), "only")],
    );
    form.validate_form(ValidationTrigger::Manual);

    form.set_field(confirm_password_path(), "secret".to_owned());

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![
            (ValidationTarget::Field(email_path().identity()), "email"),
            (ValidationTarget::Form, "form"),
        ]
    );
    assert_eq!(
        form.form_validation_status_by_id(mixed),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        form.form_validation_status_by_id(confirm_only),
        Some(ValidationStatus::Unknown)
    );
}

#[test]
fn reset_restores_baseline_values_and_clears_interaction_metadata() {
    let mut form = FormCore::new(ContactForm {
        name: "Grace".to_owned(),
    });

    form.set_user_field(name_path(), "Ada".to_owned());
    form.mark_field_blurred(name_path());
    form.mark_field_committed(name_path());

    form.reset();

    assert_eq!(form.field_value(name_path()), "Grace");
    assert!(!form.is_dirty());
    assert!(!form.is_field_touched(name_path()));
    assert!(!form.is_field_blurred(name_path()));
    assert!(!form.is_field_committed(name_path()));
}

#[test]
fn reset_field_is_a_no_op_when_an_optional_parent_is_absent() {
    let mut form: FormCore<Transaction, &'static str> =
        FormCore::new_with_error_type(Transaction { counterparty: None });
    let observed = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&observed);
    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));
    let field_versions = form.submit_validation_field_versions();

    form.reset_field(counterparty_name_path());

    assert_eq!(form.snapshot().counterparty, None);
    assert!(!form.is_dirty());
    assert_eq!(form.submit_validation_field_versions(), field_versions);
    assert!(observed.borrow().is_empty());
}

#[test]
fn reset_field_clears_interaction_state_when_the_value_matches_the_baseline() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Grace".to_owned(),
        });
    let observed = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&observed);
    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));
    form.set_user_field(name_path(), "Grace".to_owned());
    form.mark_field_blurred(name_path());
    form.mark_field_committed(name_path());
    observed.borrow_mut().clear();

    form.reset_field(name_path());

    assert!(!form.is_field_touched(name_path()));
    assert!(!form.is_field_blurred(name_path()));
    assert!(!form.is_field_committed(name_path()));
    assert!(observed.borrow().iter().any(|event| matches!(
        event,
        FormObserverEvent::FieldReset { field, .. }
            if field.identity() == FieldIdentity::new("name")
    )));
}

#[test]
fn reset_field_clears_validation_state_without_materializing_an_optional_parent() {
    let mut form: FormCore<Transaction, &'static str> =
        FormCore::new_with_error_type(Transaction { counterparty: None });
    form.register_sync_field_validator(counterparty_name_path(), "required", |_, _| {
        vec!["name required"]
    });
    form.validate_field(counterparty_name_path(), ValidationTrigger::Manual);

    form.reset_field(counterparty_name_path());

    assert_eq!(form.snapshot().counterparty, None);
    assert!(
        form.field_validation_errors(counterparty_name_path())
            .is_empty()
    );
}

#[test]
fn resetting_a_clean_container_clears_descendant_validator_results_without_rerunning_them() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    let required = form.register_sync_field_validator_for_triggers(
        nested_customer_name_path(),
        "required",
        ValidationTrigger::Manual,
        move |_value, _context| {
            validator_runs.set(validator_runs.get() + 1);
            vec!["name required"]
        },
    );
    form.validate_field(nested_customer_name_path(), ValidationTrigger::Manual);
    form.mark_field_touched(nested_customer_name_path());
    form.mark_field_blurred(nested_customer_name_path());

    form.reset_field(nested_customer_path());

    assert!(
        form.field_validation_errors(nested_customer_name_path())
            .is_empty()
    );
    assert_eq!(
        form.field_validation_status(nested_customer_name_path(), required),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(runs.get(), 1);
    assert!(form.is_field_touched(nested_customer_name_path()));
    assert!(form.is_field_blurred(nested_customer_name_path()));
}

#[test]
fn resetting_a_clean_leaf_clears_its_containers_validator_results() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    let customer_rule = form.register_sync_field_validator_for_triggers(
        nested_customer_path(),
        "customer rule",
        ValidationTrigger::Manual,
        |_value, _context| vec!["customer rejected"],
    );
    form.validate_field(nested_customer_path(), ValidationTrigger::Manual);

    form.reset_field(nested_customer_name_path());

    assert!(
        form.field_validation_errors(nested_customer_path())
            .is_empty()
    );
    assert_eq!(
        form.field_validation_status(nested_customer_path(), customer_rule),
        Some(ValidationStatus::Unknown)
    );
}

#[test]
fn resetting_a_sibling_preserves_unrelated_validator_results() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    let required = form.register_sync_field_validator_for_triggers(
        nested_customer_name_path(),
        "required",
        ValidationTrigger::Manual,
        |_value, _context| vec!["name required"],
    );
    form.validate_field(nested_customer_name_path(), ValidationTrigger::Manual);
    form.set_user_field(nested_customer_account_name_path(), "Acme".to_owned());

    form.reset_field(nested_customer_account_path());

    assert_eq!(
        form.field_validation_errors(nested_customer_name_path())[0].error(),
        &"name required"
    );
    assert_eq!(
        form.field_validation_status(nested_customer_name_path(), required),
        Some(ValidationStatus::Invalid)
    );
}

#[test]
fn clearing_a_related_validator_result_retires_a_submit_validation_token() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    form.register_sync_field_validator_for_triggers(
        nested_customer_name_path(),
        "valid name",
        ValidationTrigger::Manual,
        |_value, _context| Vec::new(),
    );
    form.validate_field(nested_customer_name_path(), ValidationTrigger::Manual);
    let validation = form.submit_validation_snapshot();

    form.reset_field(nested_customer_path());

    assert!(
        !form
            .begin_submission_after_validation(&validation)
            .is_started()
    );
}

#[test]
fn clearing_a_related_form_validator_error_retires_a_submit_validation_token() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    let rule = form.register_sync_form_validator_for_triggers(
        "valid name",
        ValidationTrigger::Manual,
        |_context| {
            vec![FormValidationError::field(
                nested_customer_name_path(),
                "name rejected",
            )]
        },
    );
    form.validate_form(ValidationTrigger::Manual);
    let validation = form.submit_validation_snapshot();

    form.reset_field(nested_customer_path());

    assert_eq!(
        form.form_validation_status_by_id(rule),
        Some(ValidationStatus::Unknown)
    );
    assert_eq!(
        form.begin_submission_after_validation(&validation),
        SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
    );
}

#[test]
fn reset_field_clears_submit_errors_without_materializing_an_optional_parent() {
    let mut form: FormCore<Transaction, &'static str> =
        FormCore::new_with_error_type(Transaction { counterparty: None });
    assert_eq!(
        form.submit(|_| SubmitError::field(counterparty_name_path(), "server rejected")),
        SubmitResult::Rejected
    );

    form.reset_field(counterparty_name_path());

    assert_eq!(form.snapshot().counterparty, None);
    assert!(
        form.field_validation_errors(counterparty_name_path())
            .is_empty()
    );
}

#[test]
fn resetting_a_clean_container_clears_submit_errors_for_fields_it_contains() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    assert_eq!(
        form.submit(|_| SubmitError::field(nested_customer_name_path(), "name rejected")),
        SubmitResult::Rejected
    );
    let observed = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&observed);
    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));

    form.reset_field(nested_customer_path());

    assert!(
        form.field_validation_errors(nested_customer_name_path())
            .is_empty()
    );
    assert!(observed.borrow().is_empty());
}

#[test]
fn resetting_a_clean_nested_field_clears_submit_errors_for_its_container() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    assert_eq!(
        form.submit(|_| SubmitError::field(nested_customer_path(), "customer rejected")),
        SubmitResult::Rejected
    );

    form.reset_field(nested_customer_name_path());

    assert!(
        form.field_validation_errors(nested_customer_path())
            .is_empty()
    );
}

#[test]
fn clearing_related_submit_errors_does_not_retire_another_intents_validation_snapshot() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(NestedPage::default());
    assert_eq!(
        form.intent(ContactSubmitIntent::Publish)
            .submit(|_| SubmitError::field(nested_customer_name_path(), "name rejected")),
        SubmitResult::Rejected
    );
    let validation = form
        .intent(ContactSubmitIntent::SaveDraft)
        .validation_snapshot();

    form.reset_field(nested_customer_path());

    assert!(
        form.intent(ContactSubmitIntent::SaveDraft)
            .begin_submission_after_validation(&validation)
            .is_started()
    );
}

#[test]
fn resetting_a_clean_field_without_submit_errors_is_a_complete_no_op() {
    let mut form: FormCore<ContactForm, &'static str> =
        FormCore::new_with_error_type(ContactForm {
            name: "Grace".to_owned(),
        });
    let validator =
        form.register_async_form_validator_for_triggers("account", ValidationTrigger::Manual);
    let run = form
        .begin_async_form_validation(validator, ValidationTrigger::Manual)
        .expect("manual async validation should start");
    let observed = Rc::new(RefCell::new(Vec::new()));
    let observed_events = Rc::clone(&observed);
    form.observe(move |event| observed_events.borrow_mut().push(event.clone()));
    let state = form.state_snapshot();
    let validation = form.submit_validation_snapshot();
    let field_versions = form.submit_validation_field_versions();

    form.reset_field(name_path());

    assert_eq!(form.state_snapshot(), state);
    assert_eq!(form.submit_validation_snapshot(), validation);
    assert_eq!(form.submit_validation_field_versions(), field_versions);
    assert!(observed.borrow().is_empty());
    assert_eq!(
        form.form_validation_status_by_id(validator),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(
        form.complete_async_form_validation(
            validator,
            &run,
            Vec::<FormValidationError<&str>>::new(),
        ),
        Some(ValidationStatus::Valid)
    );
}

#[test]
fn reinitialize_explicitly_replaces_baseline_and_current_values() {
    let mut form = FormCore::new(ContactForm {
        name: "Grace".to_owned(),
    });

    form.set_user_field(name_path(), "Ada".to_owned());
    form.mark_field_blurred(name_path());
    form.mark_field_committed(name_path());

    form.reinitialize(ContactForm {
        name: "Lin".to_owned(),
    });

    assert_eq!(form.draft().baseline().name, "Lin");
    assert_eq!(form.field_value(name_path()), "Lin");
    assert!(!form.is_dirty());
    assert!(!form.is_field_touched(name_path()));
    assert!(!form.is_field_blurred(name_path()));
    assert!(!form.is_field_committed(name_path()));
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

#[test]
fn a_derived_optional_path_keeps_the_parent_identity_and_field_name() {
    let derived = counterparty_path().or(&ABSENT_PARTY);

    assert_eq!(derived.identity(), FieldIdentity::new("counterparty"));
    assert_eq!(derived.field_name(), "counterparty");

    let name = derived.join(party_name_path());

    assert_eq!(name.identity(), FieldIdentity::new("counterparty.name"));
    assert_eq!(name.field_name(), "counterparty.name");
}

#[test]
fn reading_through_an_absent_parent_yields_the_supplied_fallback() {
    let form: FormCore<Transaction, &'static str> =
        FormCore::new_with_error_type(Transaction { counterparty: None });

    assert_eq!(form.field_value(counterparty_name_path()), "");
}

#[test]
fn reading_through_a_present_parent_yields_the_stored_value() {
    let form: FormCore<Transaction, &'static str> = FormCore::new_with_error_type(Transaction {
        counterparty: Some(Party {
            name: "Ada".to_owned(),
            address: None,
        }),
    });

    assert_eq!(form.field_value(counterparty_name_path()), "Ada");
}

#[test]
fn writing_through_an_absent_parent_materializes_the_fallback() {
    let mut form: FormCore<Transaction, &'static str> =
        FormCore::new_with_error_type(Transaction { counterparty: None });

    form.set_user_field(counterparty_name_path(), "Ada".to_owned());

    assert_eq!(
        form.snapshot().counterparty,
        Some(Party {
            name: "Ada".to_owned(),
            address: None,
        })
    );
}

#[test]
fn writing_an_inner_field_preserves_the_other_fields_of_a_present_parent() {
    let mut form: FormCore<Transaction, &'static str> =
        FormCore::new_with_error_type(Transaction { counterparty: None });

    form.set_user_field(
        counterparty_path(),
        Some(Party {
            name: "Ada".to_owned(),
            address: Some(PostalAddress {
                city: "London".to_owned(),
            }),
        }),
    );
    form.set_user_field(counterparty_name_path(), "Grace".to_owned());

    assert_eq!(
        form.snapshot().counterparty,
        Some(Party {
            name: "Grace".to_owned(),
            address: Some(PostalAddress {
                city: "London".to_owned(),
            }),
        })
    );
}

#[test]
fn read_shaped_operations_leave_an_absent_parent_absent() {
    let mut form: FormCore<Transaction, &'static str> =
        FormCore::new_with_error_type(Transaction { counterparty: None });
    form.register_sync_field_validator(counterparty_name_path(), "required", |_, _| {
        Vec::<&'static str>::new()
    });

    assert_eq!(form.field_value(counterparty_name_path()), "");
    assert!(!form.is_dirty());
    assert!(!form.is_field_dirty(counterparty_name_path()));

    let snapshot = form.state_snapshot();

    assert_eq!(snapshot.draft().current().counterparty, None);

    form.mark_field_touched(counterparty_name_path());
    form.mark_field_blurred(counterparty_name_path());
    form.validate_field(counterparty_name_path(), ValidationTrigger::Manual);
    form.validate_all(ValidationTrigger::Manual);

    assert!(form.validate_for_submit());
    assert_eq!(
        form.submit(|_| SubmitErrors::<Transaction, &'static str>::none()),
        SubmitResult::Succeeded
    );
    assert_eq!(form.snapshot().counterparty, None);
    assert!(!form.is_dirty());
}

#[test]
fn presence_reads_distinguish_an_absent_parent_from_a_present_default_one() {
    let mut form: FormCore<Transaction, &'static str> =
        FormCore::new_with_error_type(Transaction { counterparty: None });

    assert_eq!(
        counterparty_path().get_present(form.draft().current()),
        None
    );
    assert!(!counterparty_path().is_present(form.draft().current()));
    assert_eq!(form.field_value(counterparty_name_path()), "");

    form.set_user_field(counterparty_path(), Some(Party::default()));

    assert_eq!(
        counterparty_path().get_present(form.draft().current()),
        Some(&Party::default())
    );
    assert!(counterparty_path().is_present(form.draft().current()));
    assert_eq!(form.field_value(counterparty_name_path()), "");
}

#[test]
fn optional_traversal_composes_through_nested_optional_fields() {
    let mut form: FormCore<Transaction, &'static str> =
        FormCore::new_with_error_type(Transaction { counterparty: None });

    assert_eq!(
        counterparty_city_path().identity(),
        FieldIdentity::new("counterparty.address.city")
    );
    assert_eq!(
        counterparty_city_path().field_name(),
        "counterparty.address.city"
    );
    assert_eq!(form.field_value(counterparty_city_path()), "");

    form.set_user_field(counterparty_city_path(), "London".to_owned());

    assert_eq!(
        form.snapshot().counterparty,
        Some(Party {
            name: String::new(),
            address: Some(PostalAddress {
                city: "London".to_owned(),
            }),
        })
    );
}

#[test]
fn optional_traversal_composes_through_a_doubly_optional_field() {
    let mut form: FormCore<Settlement, &'static str> =
        FormCore::new_with_error_type(Settlement { nominee: None });

    assert_eq!(form.field_value(nominee_name_path()), "");

    form.set_user_field(nominee_name_path(), "Ada".to_owned());

    assert_eq!(
        form.snapshot().nominee,
        Some(Some(Party {
            name: "Ada".to_owned(),
            address: None,
        }))
    );
}

#[test]
fn reinitialize_retires_collection_item_identities_for_good() {
    let mut form = FormCore::new(invoice_form());
    let retained = line_identities(&mut form)[1];

    form.reinitialize(InvoiceForm {
        lines: vec![line("Consulting"), line("Hosting")],
    });

    assert_eq!(
        form.collection_item_field_value(lines_path(), retained, line_description_path()),
        None,
    );

    let reinitialized = line_identities(&mut form);

    assert!(!reinitialized.contains(&retained));
    assert_eq!(
        form.collection_item_field_value(lines_path(), retained, line_description_path()),
        None,
    );

    form.push_collection_item(lines_path(), line("Transient"));
    form.reset();
    assert_eq!(line_identities(&mut form), reinitialized);
}

#[test]
fn a_write_through_a_reinitialized_away_item_leaves_the_draft_unchanged() {
    let mut form = FormCore::new(invoice_form());
    let retained = line_identities(&mut form)[1];

    form.reinitialize(InvoiceForm {
        lines: vec![line("Consulting"), line("Hosting")],
    });
    let _ = line_identities(&mut form);

    assert!(!form.set_user_collection_item_field(
        lines_path(),
        retained,
        line_description_path(),
        "written through a retired identity".to_owned(),
    ));
    assert_eq!(
        form.snapshot().lines,
        vec![line("Consulting"), line("Hosting")],
    );
}

#[test]
fn reinitialize_mints_identities_above_every_one_it_retires() {
    let mut form = FormCore::new(invoice_form());
    let retired = line_identities(&mut form);

    form.reinitialize(InvoiceForm {
        lines: vec![line("Consulting"), line("Hosting")],
    });

    let minted = line_identities(&mut form);
    let highest_retired = retired
        .iter()
        .max()
        .expect("the baseline invoice should have lines");

    assert!(
        minted.iter().all(|item| item > highest_retired),
        "reinitialized identities {minted:?} should all exceed the retired {highest_retired}",
    );
}

#[test]
fn reset_keeps_a_baseline_item_bound_to_the_same_logical_item() {
    let mut form = FormCore::new(invoice_form());
    let retained = line_identities(&mut form)[1];

    form.set_user_collection_item_field(
        lines_path(),
        retained,
        line_description_path(),
        "Rebuild".to_owned(),
    );
    form.reset();

    assert_eq!(line_identities(&mut form)[1], retained);
    assert_eq!(
        form.collection_item_field_value(lines_path(), retained, line_description_path()),
        Some(&"Build".to_owned()),
    );
}

#[test]
fn an_append_after_reset_never_mints_an_identity_handed_out_before_it() {
    let mut form = FormCore::new(invoice_form());
    let before_reset = line_identities(&mut form);

    form.reset();
    let appended = form.push_user_collection_item(lines_path(), line("Support"));

    assert!(!before_reset.contains(&appended));
}

#[test]
fn restoring_a_snapshot_taken_before_the_collection_was_read_retires_the_live_identities() {
    let mut form = FormCore::new(invoice_form());
    let snapshot = form.state_snapshot();
    let issued = line_identities(&mut form);

    // A snapshot captured before the collection was ever read carries no identity for it, and the
    // live identities are not its to keep: this removal leaves "Build" holding the only live
    // identity, at the index the restored "Design" occupies. Carrying the live sequence across the
    // restore would silently rebind it onto a line it was never minted for, so the restore retires
    // it instead — an item absent from the collection is the **Unresolved Binding** ADR-0022
    // already answers for. The counter still never rewinds, so nothing is reissued either.
    form.remove_user_collection_item(lines_path(), issued[0]);

    form.restore_state_snapshot(snapshot)
        .expect("a snapshot of this form should restore");

    let after_restore = line_identities(&mut form);

    assert_eq!(after_restore.len(), 2);
    assert!(
        after_restore.iter().all(|item| !issued.contains(item)),
        "restored identities {after_restore:?} should reuse none of {issued:?}",
    );
    assert_eq!(
        form.collection_item_field_value(lines_path(), issued[1], line_description_path()),
        None,
    );
}

#[test]
fn restoring_a_snapshot_never_lowers_the_identity_counter() {
    let mut form = FormCore::new(invoice_form());
    let _ = line_identities(&mut form);
    let snapshot = form.state_snapshot();
    let appended = form.push_user_collection_item(lines_path(), line("Support"));

    form.restore_state_snapshot(snapshot)
        .expect("a snapshot of this form should restore");

    assert!(
        form.push_user_collection_item(lines_path(), line("Training")) > appended,
        "an append after a restore should mint above every identity issued before it",
    );
}

#[test]
fn resetting_the_collection_field_itself_restores_its_baseline_items() {
    let mut form = FormCore::new(invoice_form());
    let baseline = line_identities(&mut form);
    let appended = form.push_user_collection_item(lines_path(), line("Support"));
    let appended_description = line_field_identity(appended, "description");

    form.mark_collection_item_field_blurred(lines_path(), appended, line_description_path());
    form.reset_field(lines_path());

    assert_eq!(line_identities(&mut form), baseline);
    assert_eq!(
        form.collection_item_field_value(lines_path(), appended, line_description_path()),
        None,
    );
    assert!(!form.is_field_identity_blurred(&appended_description));
    assert!(form.push_user_collection_item(lines_path(), line("Training")) > appended);
}

#[test]
fn resetting_a_collection_clears_kept_item_validator_results() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let kept = form.collection_items(lines_path())[0].identity();
    let quantity = line_field_identity(kept, "quantity");

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
    form.set_user_collection_item_field(lines_path(), kept, line_quantity_path(), 0);
    form.validate_all(ValidationTrigger::Manual);

    assert_eq!(
        form.field_validation_statuses_by_identity(&quantity)[0].status(),
        ValidationStatus::Invalid
    );
    assert!(!form.submit_availability().is_available());

    form.reset_field(lines_path());

    assert_eq!(
        form.collection_item_field_value(lines_path(), kept, line_quantity_path()),
        Some(&2)
    );
    assert!(
        form.field_validation_errors_by_identity(&quantity)
            .is_empty()
    );
    assert_eq!(
        form.field_validation_statuses_by_identity(&quantity)[0].status(),
        ValidationStatus::Unknown
    );
    assert!(form.submit_availability().is_available());

    form.validate_all(ValidationTrigger::Manual);
    assert_eq!(
        form.field_validation_statuses_by_identity(&quantity)[0].status(),
        ValidationStatus::Valid
    );
}

#[test]
fn resetting_an_unchanged_collection_clears_item_validator_results() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let kept = form.collection_items(lines_path())[0].identity();
    let quantity = line_field_identity(kept, "quantity");

    form.register_sync_collection_item_field_validator(
        lines_path(),
        line_quantity_path(),
        "quantity",
        |_value, _context| vec!["quantity rejected"],
    );
    form.validate_all(ValidationTrigger::Manual);

    assert_eq!(
        form.field_validation_statuses_by_identity(&quantity)[0].status(),
        ValidationStatus::Invalid
    );

    form.reset_field(lines_path());

    assert!(
        form.field_validation_errors_by_identity(&quantity)
            .is_empty()
    );
    assert_eq!(
        form.field_validation_statuses_by_identity(&quantity)[0].status(),
        ValidationStatus::Unknown
    );
}

#[test]
fn resetting_a_collection_clears_direct_item_validator_results_in_place() {
    let mut form: FormCore<InvoiceForm, &'static str> =
        FormCore::new_with_error_type(invoice_form());
    let kept = form.collection_items(lines_path())[0].identity();
    let quantity = line_field_identity(kept, "quantity");

    form.register_sync_field_identity_validator_for_triggers(
        quantity.clone(),
        "direct quantity",
        ValidationTrigger::Manual,
        |_model, _context| vec!["quantity rejected"],
    );
    form.validate_all(ValidationTrigger::Manual);

    assert_eq!(
        form.field_validation_statuses_by_identity(&quantity)[0].status(),
        ValidationStatus::Invalid
    );

    form.reset_field(lines_path());

    assert_eq!(
        form.field_validation_statuses_by_identity(&quantity)[0].status(),
        ValidationStatus::Unknown
    );
    form.validate_all(ValidationTrigger::Manual);
    assert_eq!(
        form.field_validation_statuses_by_identity(&quantity)[0].status(),
        ValidationStatus::Invalid
    );
}

#[test]
fn resetting_a_static_field_preserves_unrelated_collection_item_validator_results() {
    let mut form: FormCore<NestedPage, &'static str> =
        FormCore::new_with_error_type(nested_page_with_one_line());
    let item = form.collection_items(nested_invoice_lines_path())[0].identity();
    let line_name = line_field_identity_for(item, "customer.name");

    form.register_sync_collection_item_field_validator(
        nested_invoice_lines_path(),
        line_customer_name_path(),
        "line name",
        |_value, _context| vec!["line name rejected"],
    );
    form.validate_all(ValidationTrigger::Manual);
    form.set_user_field(nested_customer_path(), nested_customer("Ada"));

    form.reset_field(nested_customer_path());

    assert_eq!(
        form.field_validation_statuses_by_identity(&line_name)[0].status(),
        ValidationStatus::Invalid
    );
    assert_eq!(
        form.field_validation_errors_by_identity(&line_name)[0].error(),
        &"line name rejected"
    );
}

#[test]
fn clearing_a_collection_that_has_no_baseline_rows_does_not_rewind_its_counter() {
    let mut form = FormCore::new(InvoiceForm { lines: Vec::new() });
    let appended = form.push_user_collection_item(lines_path(), line("Support"));

    // Both identity sequences are now empty, exactly as they are for a collection that has never
    // been read. Reading it must mint nothing rather than start over from the first identity.
    form.clear_user_collection_items(lines_path());

    assert!(line_identities(&mut form).is_empty());
    assert!(form.push_user_collection_item(lines_path(), line("Training")) > appended);
}

/// Records which line every identity the collection has held denotes, asserting on each reading
/// that no identity has come to denote a different line and that a freshly minted one is above
/// every identity issued before it.
///
/// Every line in the sequence below carries a distinct description, so the description is the
/// logical item. A previously issued identity may reappear — `reset` and snapshot restore reinstate
/// a sequence without minting anything — but it has to come back denoting the line it left with. A
/// *fresh* identity below the high-water mark cannot appear at all: the counter only moves forward,
/// so a number below it was already handed out for some other line.
#[derive(Default)]
struct IssuedIdentities {
    seen: Vec<(CollectionItemIdentity, String)>,
}

impl IssuedIdentities {
    fn record(&mut self, form: &mut FormCore<InvoiceForm>) {
        for item in form.collection_items(lines_path()) {
            let identity = item.identity();
            let description = form
                .collection_item_field_value(lines_path(), identity, line_description_path())
                .expect("a rendered item should resolve its own child field")
                .clone();

            if let Some((_, denoted)) = self.seen.iter().find(|(seen, _)| *seen == identity) {
                assert_eq!(
                    denoted, &description,
                    "identity {identity} denoted {denoted} and now denotes {description}",
                );
                continue;
            }

            assert!(
                self.seen.iter().all(|(previous, _)| identity > *previous),
                "identity {identity} was minted below already issued {:?}",
                self.seen,
            );
            self.seen.push((identity, description));
        }
    }
}

#[test]
fn no_collection_item_identity_is_ever_issued_twice() {
    let mut form = FormCore::new(invoice_form());
    let mut issued = IssuedIdentities::default();

    issued.record(&mut form);

    let removed = issued.seen[0].0;
    form.push_user_collection_item(lines_path(), line("Support"));
    issued.record(&mut form);
    form.remove_user_collection_item(lines_path(), removed);
    issued.record(&mut form);
    form.reset();
    issued.record(&mut form);
    form.push_user_collection_item(lines_path(), line("Training"));
    issued.record(&mut form);

    let snapshot = form.state_snapshot();

    form.reinitialize(InvoiceForm {
        lines: vec![line("Consulting"), line("Hosting")],
    });
    issued.record(&mut form);
    form.push_user_collection_item(lines_path(), line("Retainer"));
    issued.record(&mut form);

    form.restore_state_snapshot(snapshot)
        .expect("a snapshot of this form should restore");
    issued.record(&mut form);
    form.push_user_collection_item(lines_path(), line("Overtime"));
    issued.record(&mut form);
}
