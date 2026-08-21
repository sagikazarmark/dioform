use std::{borrow::Cow, cell::RefCell, collections::BTreeMap, rc::Rc};

use dioform_core::{CollectionItemIdentity, FieldIdentity, FieldPath, FormCore, ValidationTrigger};
use dioform_validator::{
    CollectionValidationTargetResolutionFailure, DiagnosticRouteProvenance,
    ValidationAdapterConfigurationIssue, ValidatorCollectionPath, ValidatorCollectionTargetRule,
    ValidatorPathMap, ValidatorValidationExt,
};
use validator::ValidationErrorsKind;

#[derive(Clone)]
struct Line {
    quantity: u32,
}

#[derive(Clone)]
struct Order {
    lines: Vec<Line>,
    reported_rows: Option<Vec<usize>>,
    emit_unmapped: bool,
}

impl validator::Validate for Order {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        let mut rows = BTreeMap::new();
        for index in self
            .reported_rows
            .clone()
            .unwrap_or_else(|| (0..self.lines.len()).collect())
        {
            let mut errors = validator::ValidationErrors::new();
            errors.add("quantity", validator::ValidationError::new("positive"));
            rows.insert(index, Box::new(errors));
        }

        let mut errors = validator::ValidationErrors::new();
        errors
            .0
            .insert(Cow::Borrowed("lines"), ValidationErrorsKind::List(rows));
        if self.emit_unmapped {
            errors.add("other", validator::ValidationError::new("invalid"));
        }
        Err(errors)
    }
}

impl<'args> validator::ValidateArgs<'args> for Order {
    type Args = &'args bool;

    fn validate_with_args(&self, enabled: Self::Args) -> Result<(), validator::ValidationErrors> {
        if *enabled {
            validator::Validate::validate(self)
        } else {
            Ok(())
        }
    }
}

fn lines_path() -> FieldPath<Order, Vec<Line>> {
    FieldPath::direct(
        FieldIdentity::new("lines"),
        "lines",
        |order: &Order| &order.lines,
        |order: &mut Order| &mut order.lines,
    )
}

fn quantity_path() -> FieldPath<Line, u32> {
    FieldPath::direct(
        FieldIdentity::new("quantity"),
        "quantity",
        |line: &Line| &line.quantity,
        |line: &mut Line| &mut line.quantity,
    )
}

fn captured_quantity_path(item: CollectionItemIdentity) -> FieldPath<Order, u32> {
    FieldPath::direct(
        FieldIdentity::collection_item("lines", item, "quantity"),
        "lines[0].quantity",
        |order: &Order| &order.lines[0].quantity,
        |order: &mut Order| &mut order.lines[0].quantity,
    )
}

fn line(quantity: u32) -> Line {
    Line { quantity }
}

fn order(lines: Vec<Line>) -> Order {
    Order {
        lines,
        reported_rows: None,
        emit_unmapped: false,
    }
}

fn current_identities(form: &mut FormCore<Order, String>) -> Vec<CollectionItemIdentity> {
    form.collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect()
}

fn error_identities(form: &FormCore<Order, String>) -> Vec<CollectionItemIdentity> {
    form.validation_errors()
        .into_iter()
        .map(|error| {
            error
                .expect_field()
                .collection_item_identity()
                .expect("collection diagnostics should target current items")
        })
        .collect()
}

#[test]
fn collection_diagnostics_follow_current_identities_after_append_and_remove() {
    let mut form = FormCore::new(order(vec![line(0), line(0)]));
    let rule = ValidatorCollectionTargetRule::descendant(
        ValidatorCollectionPath::new(["lines"], ["quantity"]),
        lines_path(),
        quantity_path(),
    )
    .expect("static collection and descendant paths should be supported");
    form.validator_validation()
        .collection_target_rule(rule)
        .register_string_errors();
    let initial = current_identities(&mut form);

    let appended = form.push_collection_item(lines_path(), line(0));
    form.validate_form(ValidationTrigger::Manual);
    assert_eq!(error_identities(&form), [initial[0], initial[1], appended]);

    form.remove_collection_item(lines_path(), initial[0])
        .expect("the first row should exist");
    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(error_identities(&form), [initial[1], appended]);
    assert!(!error_identities(&form).contains(&initial[0]));
}

#[test]
fn collection_rules_follow_insert_move_swap_replacement_and_clear_lifecycle() {
    let mut form = FormCore::new(order(vec![line(1), line(2)]));
    let rule = ValidatorCollectionTargetRule::descendant(
        ValidatorCollectionPath::new(["lines"], ["quantity"]),
        lines_path(),
        quantity_path(),
    )
    .expect("static paths should be supported");
    form.validator_validation()
        .collection_target_rule(rule)
        .register_string_errors();
    let initial = current_identities(&mut form);

    let inserted = form
        .insert_collection_item(lines_path(), 1, line(3))
        .expect("the insertion index should exist");
    form.validate_form(ValidationTrigger::Manual);
    assert_eq!(error_identities(&form), [initial[0], inserted, initial[1]]);

    assert!(form.move_collection_item_to_index(lines_path(), initial[1], 0));
    form.validate_form(ValidationTrigger::Manual);
    assert_eq!(error_identities(&form), [initial[1], initial[0], inserted]);

    assert!(form.swap_collection_items(lines_path(), 0, 2));
    form.validate_form(ValidationTrigger::Manual);
    assert_eq!(error_identities(&form), [inserted, initial[0], initial[1]]);

    assert!(form.replace_collection_item(lines_path(), 1, line(4)));
    form.validate_form(ValidationTrigger::Manual);
    assert_eq!(error_identities(&form), [inserted, initial[0], initial[1]]);

    let displaced = current_identities(&mut form);
    form.set_field(lines_path(), vec![line(5), line(6), line(7)]);
    let replacements = current_identities(&mut form);
    assert!(replacements.iter().all(|item| !displaced.contains(item)));
    form.validate_form(ValidationTrigger::Manual);
    assert_eq!(error_identities(&form), replacements);

    let cleared = form.clear_collection_items(lines_path());
    assert_eq!(cleared.len(), 3);
    form.validate_form(ValidationTrigger::Manual);
    assert!(form.validation_errors().is_empty());
}

#[test]
fn ambiguity_and_true_misses_use_their_separate_reporters() {
    let collection_failures = Rc::new(RefCell::new(Vec::new()));
    let failures_for_adapter = Rc::clone(&collection_failures);
    let unmapped = Rc::new(RefCell::new(Vec::new()));
    let unmapped_for_adapter = Rc::clone(&unmapped);
    let mut form = FormCore::new(Order {
        lines: vec![line(0)],
        reported_rows: Some(vec![0, 3]),
        emit_unmapped: true,
    });
    let rule = || {
        ValidatorCollectionTargetRule::descendant(
            ValidatorCollectionPath::new(["lines"], ["quantity"]),
            lines_path(),
            quantity_path(),
        )
        .expect("static paths should be supported")
    };
    form.validator_validation()
        .collection_target_rule(rule())
        .collection_target_rule(rule())
        .on_unmapped_path(move |path| unmapped_for_adapter.borrow_mut().push(path.to_owned()))
        .on_collection_resolution_failure(move |path, failure| {
            failures_for_adapter
                .borrow_mut()
                .push((path.to_owned(), failure.clone()));
        })
        .register_string_errors();

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(unmapped.borrow().as_slice(), ["other"]);
    assert_eq!(
        collection_failures.borrow().as_slice(),
        [
            (
                "lines[0].quantity".to_owned(),
                CollectionValidationTargetResolutionFailure::AmbiguousMatchingRules {
                    match_count: 2,
                },
            ),
            (
                "lines[3].quantity".to_owned(),
                CollectionValidationTargetResolutionFailure::AmbiguousMatchingRules {
                    match_count: 2,
                },
            ),
        ]
    );
    assert!(
        form.validation_errors()
            .iter()
            .all(|error| error.target().is_form())
    );
}

#[test]
fn one_matching_rule_with_a_missing_row_reports_missing_row_not_unmapped() {
    let failures = Rc::new(RefCell::new(Vec::new()));
    let failures_for_adapter = Rc::clone(&failures);
    let unmapped_count = Rc::new(RefCell::new(0));
    let unmapped_for_adapter = Rc::clone(&unmapped_count);
    let mut form = FormCore::new(Order {
        lines: vec![line(0)],
        reported_rows: Some(vec![4]),
        emit_unmapped: false,
    });
    let rule = ValidatorCollectionTargetRule::descendant(
        ValidatorCollectionPath::new(["lines"], ["quantity"]),
        lines_path(),
        quantity_path(),
    )
    .expect("static paths should be supported");
    form.validator_validation()
        .collection_target_rule(rule)
        .on_unmapped_path(move |_| *unmapped_for_adapter.borrow_mut() += 1)
        .on_collection_resolution_failure(move |path, failure| {
            failures_for_adapter
                .borrow_mut()
                .push((path.to_owned(), failure.clone()));
        })
        .register_string_errors();

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(*unmapped_count.borrow(), 0);
    assert_eq!(
        failures.borrow().as_slice(),
        [(
            "lines[4].quantity".to_owned(),
            CollectionValidationTargetResolutionFailure::MissingRow,
        )]
    );
    assert!(form.validation_errors()[0].target().is_form());
}

#[test]
fn configuration_issues_aggregate_ineligible_exact_targets_and_duplicate_matchers() {
    let mut form = FormCore::new(order(vec![line(0)]));
    let captured = current_identities(&mut form)[0];
    let rule = || {
        ValidatorCollectionTargetRule::descendant(
            ValidatorCollectionPath::new(["lines"], ["quantity"]),
            lines_path(),
            quantity_path(),
        )
        .expect("static paths should be supported")
    };
    let builder = form
        .validator_validation()
        .path_map(
            ValidatorPathMap::new()
                .with_field("lines[0].quantity", captured_quantity_path(captured)),
        )
        .collection_target_rule(rule())
        .collection_target_rule(rule());

    let issues = builder.configuration_issues();

    assert_eq!(issues.len(), 2);
    assert!(matches!(
        &issues[0],
        ValidationAdapterConfigurationIssue::IneligibleExactTarget(issue)
            if issue.external_path() == "lines[0].quantity"
    ));
    assert!(matches!(
        &issues[1],
        ValidationAdapterConfigurationIssue::DuplicateCollectionRule(issue)
            if issue.first_rule_index() == 0 && issue.duplicate_rule_index() == 1
    ));

    builder.register_string_errors();
}

#[derive(Clone)]
struct LiteralKeyOrder {
    lines: Vec<Line>,
}

impl validator::Validate for LiteralKeyOrder {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        let mut errors = validator::ValidationErrors::new();
        errors.add(
            "lines[0].quantity",
            validator::ValidationError::new("literal"),
        );
        Err(errors)
    }
}

fn literal_lines_path() -> FieldPath<LiteralKeyOrder, Vec<Line>> {
    FieldPath::direct(
        FieldIdentity::new("lines"),
        "lines",
        |order: &LiteralKeyOrder| &order.lines,
        |order: &mut LiteralKeyOrder| &mut order.lines,
    )
}

#[test]
fn bracket_like_text_in_a_field_key_is_not_a_structural_list_index() {
    let mut form = FormCore::new(LiteralKeyOrder {
        lines: vec![line(0)],
    });
    let rule = ValidatorCollectionTargetRule::descendant(
        ValidatorCollectionPath::new(["lines"], ["quantity"]),
        literal_lines_path(),
        quantity_path(),
    )
    .expect("static paths should be supported");
    form.validator_validation()
        .collection_target_rule(rule)
        .register_string_errors();

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(form.validation_errors().len(), 1);
    assert!(form.validation_errors()[0].target().is_form());
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RoutedError {
    path: String,
    provenance: DiagnosticRouteProvenance,
}

#[test]
fn captured_exact_target_is_ineligible_and_custom_mapper_sees_live_rule_provenance() {
    let mut form: FormCore<Order, RoutedError> =
        FormCore::new_with_error_type(order(vec![line(0), line(0)]));
    let initial = current_identities_for_error(&mut form);
    let rule = ValidatorCollectionTargetRule::descendant(
        ValidatorCollectionPath::new(["lines"], ["quantity"]),
        lines_path(),
        quantity_path(),
    )
    .expect("static paths should be supported");
    form.validator_validation()
        .path_map(
            ValidatorPathMap::new()
                .with_field("lines[0].quantity", captured_quantity_path(initial[0])),
        )
        .collection_target_rule(rule)
        .register(|diagnostic| RoutedError {
            path: diagnostic.path().to_owned(),
            provenance: diagnostic
                .route_provenance()
                .expect("adapter diagnostics should carry provenance")
                .clone(),
        });

    form.remove_collection_item(lines_path(), initial[0])
        .expect("the captured row should exist");
    form.validate_form(ValidationTrigger::Manual);

    let error = &form.validation_errors()[0];
    assert_eq!(
        error.expect_field().collection_item_identity(),
        Some(initial[1])
    );
    assert_eq!(
        error.error(),
        &RoutedError {
            path: "lines[0].quantity".to_owned(),
            provenance: DiagnosticRouteProvenance::CollectionValidationTargetRule,
        }
    );
}

#[test]
fn eligible_static_exact_mapping_wins_over_a_matching_collection_rule() {
    let mut form: FormCore<Order, RoutedError> =
        FormCore::new_with_error_type(order(vec![line(0)]));
    let rule = ValidatorCollectionTargetRule::descendant(
        ValidatorCollectionPath::new(["lines"], ["quantity"]),
        lines_path(),
        quantity_path(),
    )
    .expect("static paths should be supported");
    form.validator_validation()
        .path_map(ValidatorPathMap::new().with_field("lines[0].quantity", lines_path()))
        .collection_target_rule(rule)
        .register(|diagnostic| RoutedError {
            path: diagnostic.path().to_owned(),
            provenance: diagnostic
                .route_provenance()
                .expect("adapter diagnostics should carry provenance")
                .clone(),
        });

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(
        form.validation_errors()[0].expect_field(),
        FieldIdentity::new("lines")
    );
    assert_eq!(
        form.validation_errors()[0].error().provenance,
        DiagnosticRouteProvenance::ExactStaticTarget
    );
}

#[test]
fn context_and_context_string_terminals_register_live_collection_rules() {
    let make_rule = || {
        ValidatorCollectionTargetRule::descendant(
            ValidatorCollectionPath::new(["lines"], ["quantity"]),
            lines_path(),
            quantity_path(),
        )
        .expect("static paths should be supported")
    };

    let mut custom: FormCore<Order, RoutedError> =
        FormCore::new_with_error_type(order(vec![line(0)]));
    custom
        .validator_validation()
        .collection_target_rule(make_rule())
        .register_with_context(
            |_| true,
            |diagnostic| RoutedError {
                path: diagnostic.path().to_owned(),
                provenance: diagnostic
                    .route_provenance()
                    .expect("adapter diagnostics should carry provenance")
                    .clone(),
            },
        );
    let custom_appended = custom.push_collection_item(lines_path(), line(0));
    custom.validate_form(ValidationTrigger::Manual);
    assert_eq!(
        custom.validation_errors()[1]
            .expect_field()
            .collection_item_identity(),
        Some(custom_appended)
    );
    assert_eq!(
        custom.validation_errors()[1].error().provenance,
        DiagnosticRouteProvenance::CollectionValidationTargetRule
    );

    let mut strings = FormCore::new(order(vec![line(0)]));
    strings
        .validator_validation()
        .collection_target_rule(make_rule())
        .register_string_errors_with_context(|_| true);
    let string_appended = strings.push_collection_item(lines_path(), line(0));
    strings.validate_form(ValidationTrigger::Manual);
    assert_eq!(strings.validation_errors()[1].error(), "positive");
    assert_eq!(
        strings.validation_errors()[1]
            .expect_field()
            .collection_item_identity(),
        Some(string_appended)
    );
}

#[test]
fn item_value_rule_routes_a_matched_row_diagnostic_to_the_item_value() {
    let item_rule = ValidatorCollectionTargetRule::item(
        ValidatorCollectionPath::new(["lines"], ["quantity"]),
        lines_path(),
    )
    .expect("a static collection item target should be supported");
    let mut form = FormCore::new(order(vec![line(0)]));
    let item = current_identities(&mut form)[0];
    form.validator_validation()
        .collection_target_rule(item_rule)
        .register_string_errors();

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(
        form.validation_errors()[0].expect_field(),
        FieldIdentity::collection_item_value("lines", item)
    );
}

fn current_identities_for_error<Error>(
    form: &mut FormCore<Order, Error>,
) -> Vec<CollectionItemIdentity> {
    form.collection_items(lines_path())
        .into_iter()
        .map(|item| item.identity())
        .collect()
}
