use std::{cell::RefCell, rc::Rc};

use dioform_core::{
    CollectionItemIdentity, FieldIdentity, FieldPath, FormCore, ValidationTarget, ValidationTrigger,
};
use dioform_garde::{
    DiagnosticRouteProvenance, GardeCollectionRowMatcher, GardeDiagnostic, GardePathMap,
    GardeValidationExt, ValidationAdapterConfigurationIssue,
};

#[derive(Clone)]
struct Line {
    description: String,
}

#[derive(Clone)]
struct Invoice {
    lines: Vec<Line>,
    emit_missing_and_unmapped: bool,
}

impl garde::Validate for Invoice {
    type Context = ();

    fn validate_into(
        &self,
        _context: &Self::Context,
        parent: &mut dyn FnMut() -> garde::Path,
        report: &mut garde::Report,
    ) {
        for (index, line) in self.lines.iter().enumerate() {
            report.append(
                parent().join("lines").join(index).join("description"),
                garde::Error::new(line.description.clone()),
            );
        }
        if self.emit_missing_and_unmapped {
            report.append(
                parent()
                    .join("lines")
                    .join(self.lines.len() + 8)
                    .join("description"),
                garde::Error::new("missing row"),
            );
            report.append(parent().join("unconfigured"), garde::Error::new("unmapped"));
        }
    }
}

fn lines_path() -> FieldPath<Invoice, Vec<Line>> {
    FieldPath::direct(
        FieldIdentity::new("lines"),
        "lines",
        |invoice: &Invoice| &invoice.lines,
        |invoice: &mut Invoice| &mut invoice.lines,
    )
}

fn description_path() -> FieldPath<Line, String> {
    FieldPath::direct(
        FieldIdentity::new("description"),
        "description",
        |line: &Line| &line.description,
        |line: &mut Line| &mut line.description,
    )
}

fn static_override_path() -> FieldPath<Invoice, bool> {
    FieldPath::direct(
        FieldIdentity::new("static_override"),
        "static_override",
        |invoice: &Invoice| &invoice.emit_missing_and_unmapped,
        |invoice: &mut Invoice| &mut invoice.emit_missing_and_unmapped,
    )
}

fn line(description: &str) -> Line {
    Line {
        description: description.to_owned(),
    }
}

fn description_target(item: CollectionItemIdentity) -> ValidationTarget {
    ValidationTarget::field_identity(FieldIdentity::collection_item("lines", item, "description"))
}

#[derive(Clone)]
struct TagsForm {
    tags: Vec<String>,
}

impl garde::Validate for TagsForm {
    type Context = ();

    fn validate_into(
        &self,
        _context: &Self::Context,
        parent: &mut dyn FnMut() -> garde::Path,
        report: &mut garde::Report,
    ) {
        for (index, tag) in self.tags.iter().enumerate() {
            report.append(
                parent().join("tags").join(index),
                garde::Error::new(tag.clone()),
            );
        }
    }
}

fn tags_path() -> FieldPath<TagsForm, Vec<String>> {
    FieldPath::direct(
        FieldIdentity::new("tags"),
        "tags",
        |form: &TagsForm| &form.tags,
        |form: &mut TagsForm| &mut form.tags,
    )
}

#[derive(Clone)]
struct StructuralPathForm {
    lines: Vec<Line>,
}

impl garde::Validate for StructuralPathForm {
    type Context = ();

    fn validate_into(
        &self,
        _context: &Self::Context,
        parent: &mut dyn FnMut() -> garde::Path,
        report: &mut garde::Report,
    ) {
        for row_component in ["0", "[0]"] {
            report.append(
                parent()
                    .join("lines")
                    .join(row_component)
                    .join("description"),
                garde::Error::new("string key"),
            );
        }
        report.append(
            parent().join("lines").join(0usize).join("description"),
            garde::Error::new("numeric index"),
        );
    }
}

fn structural_lines_path() -> FieldPath<StructuralPathForm, Vec<Line>> {
    FieldPath::direct(
        FieldIdentity::new("lines"),
        "lines",
        |form: &StructuralPathForm| &form.lines,
        |form: &mut StructuralPathForm| &mut form.lines,
    )
}

#[test]
fn row_appended_after_registration_routes_to_its_current_identity() {
    let mut form = FormCore::new(Invoice {
        lines: vec![line("Design")],
        emit_missing_and_unmapped: false,
    });
    form.garde_validation()
        .collection_row_descendant(
            GardeCollectionRowMatcher::new(["lines"], ["description"]),
            lines_path(),
            description_path(),
        )
        .expect("static collection and descendant paths should be supported")
        .register_string_errors();

    let appended = form.push_collection_item(lines_path(), line("Build"));
    form.validate_form(ValidationTrigger::Manual);

    let targets: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| error.target())
        .collect();
    assert_eq!(
        targets,
        vec![
            description_target(form.collection_items(lines_path())[0].identity()),
            description_target(appended),
        ]
    );
}

#[test]
fn removing_row_zero_routes_the_survivor_at_zero_to_its_live_identity() {
    let mut form = FormCore::new(Invoice {
        lines: vec![line("Design"), line("Build")],
        emit_missing_and_unmapped: false,
    });
    form.garde_validation()
        .collection_row_descendant(
            GardeCollectionRowMatcher::new(["lines"], ["description"]),
            lines_path(),
            description_path(),
        )
        .expect("static collection and descendant paths should be supported")
        .register_string_errors();
    let initial = form.collection_items(lines_path());
    let retired = initial[0].identity();
    let survivor = initial[1].identity();

    form.remove_collection_item(lines_path(), retired)
        .expect("the first row should exist");
    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(form.validation_errors().len(), 1);
    assert_eq!(
        form.validation_errors()[0].target(),
        description_target(survivor)
    );
    assert_ne!(
        form.validation_errors()[0].target(),
        description_target(retired)
    );
}

#[test]
fn bare_row_matcher_routes_to_the_collection_item_value() {
    let mut form = FormCore::new(TagsForm {
        tags: vec!["rust".to_owned()],
    });
    form.garde_validation()
        .collection_row_item(
            GardeCollectionRowMatcher::new(["tags"], std::iter::empty::<&str>()),
            tags_path(),
        )
        .expect("a static collection path should be supported")
        .register_string_errors();
    let item = form.collection_items(tags_path())[0].identity();

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(
        form.validation_errors()[0].target(),
        ValidationTarget::field_identity(FieldIdentity::collection_item_value("tags", item))
    );
}

#[test]
fn configuration_issues_aggregate_ineligible_exact_targets_and_duplicate_matchers() {
    let mut form = FormCore::new(Invoice {
        lines: vec![line("Design")],
        emit_missing_and_unmapped: false,
    });
    let captured = form.collection_items(lines_path())[0].identity();
    let captured_description = FieldPath::direct(
        FieldIdentity::collection_item("lines", captured, "description"),
        "lines[0].description",
        |invoice: &Invoice| &invoice.lines[0].description,
        |invoice: &mut Invoice| &mut invoice.lines[0].description,
    );
    let matcher = GardeCollectionRowMatcher::new(["lines"], ["description"]);
    let builder = form
        .garde_validation()
        .path_map(GardePathMap::new().with_field("lines[0].description", captured_description))
        .collection_row_descendant(matcher.clone(), lines_path(), description_path())
        .expect("static collection and descendant paths should be supported")
        .collection_row_descendant(matcher, lines_path(), description_path())
        .expect("duplicate rules remain an infallible registration configuration");

    let issues = builder.configuration_issues();

    assert_eq!(issues.len(), 2);
    assert!(matches!(
        &issues[0],
        ValidationAdapterConfigurationIssue::IneligibleExactTarget(issue)
            if issue.external_path() == "lines[0].description"
    ));
    assert!(matches!(
        &issues[1],
        ValidationAdapterConfigurationIssue::DuplicateCollectionRule(issue)
            if issue.first_rule_index() == 0 && issue.duplicate_rule_index() == 1
    ));
}

#[test]
fn missing_rows_and_true_misses_use_separate_reporters_once_per_diagnostic() {
    let collection_failures = Rc::new(RefCell::new(Vec::new()));
    let failures_for_reporter = Rc::clone(&collection_failures);
    let unmapped = Rc::new(RefCell::new(Vec::new()));
    let unmapped_for_reporter = Rc::clone(&unmapped);
    let mut form = FormCore::new(Invoice {
        lines: vec![line("Design")],
        emit_missing_and_unmapped: true,
    });
    form.garde_validation()
        .collection_row_descendant(
            GardeCollectionRowMatcher::new(["lines"], ["description"]),
            lines_path(),
            description_path(),
        )
        .expect("static collection and descendant paths should be supported")
        .on_collection_resolution_failure(move |path, failure| {
            failures_for_reporter
                .borrow_mut()
                .push((path.clone(), failure.clone()));
        })
        .on_unmapped_path(move |path| unmapped_for_reporter.borrow_mut().push(path.clone()))
        .register_string_errors();

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(collection_failures.borrow().len(), 1);
    assert_eq!(
        collection_failures.borrow()[0].0,
        garde::Path::new("lines").join(9usize).join("description")
    );
    assert_eq!(
        collection_failures.borrow()[0].1,
        dioform_garde::CollectionValidationTargetResolutionFailure::MissingRow
    );
    assert_eq!(
        unmapped.borrow().as_slice(),
        [garde::Path::new("unconfigured")]
    );
    assert_eq!(form.form_validation_errors().len(), 2);
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RoutedError {
    message: String,
    provenance: DiagnosticRouteProvenance,
}

fn routed_error(diagnostic: GardeDiagnostic<'_>) -> RoutedError {
    RoutedError {
        message: diagnostic.error().to_string(),
        provenance: diagnostic
            .route_provenance()
            .expect("adapter diagnostics should carry route provenance")
            .clone(),
    }
}

#[test]
fn ambiguous_rules_fail_to_form_report_once_and_expose_mapper_provenance() {
    let failures = Rc::new(RefCell::new(Vec::new()));
    let failures_for_reporter = Rc::clone(&failures);
    let matcher = GardeCollectionRowMatcher::new(["lines"], ["description"]);
    let mut form: FormCore<Invoice, RoutedError> = FormCore::new_with_error_type(Invoice {
        lines: vec![line("Design")],
        emit_missing_and_unmapped: false,
    });
    form.garde_validation()
        .collection_row_descendant(matcher.clone(), lines_path(), description_path())
        .expect("static collection and descendant paths should be supported")
        .collection_row_descendant(matcher, lines_path(), description_path())
        .expect("duplicate rules remain registrable")
        .on_collection_resolution_failure(move |path, failure| {
            failures_for_reporter
                .borrow_mut()
                .push((path.clone(), failure.clone()));
        })
        .register(routed_error);

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(failures.borrow().len(), 1);
    assert_eq!(form.form_validation_errors().len(), 1);
    assert_eq!(
        form.form_validation_errors()[0].error(),
        &RoutedError {
            message: "Design".to_owned(),
            provenance: DiagnosticRouteProvenance::CollectionValidationTargetResolutionFailure(
                dioform_garde::CollectionValidationTargetResolutionFailure::AmbiguousMatchingRules {
                    match_count: 2,
                },
            ),
        }
    );
}

#[test]
fn eligible_static_exact_mapping_wins_over_a_matching_collection_rule() {
    let external_path = garde::Path::new("lines")
        .join(0usize)
        .join("description")
        .to_string();
    let mut form: FormCore<Invoice, RoutedError> = FormCore::new_with_error_type(Invoice {
        lines: vec![line("Design")],
        emit_missing_and_unmapped: false,
    });
    form.garde_validation()
        .path_map(GardePathMap::new().with_field(external_path, static_override_path()))
        .collection_row_descendant(
            GardeCollectionRowMatcher::new(["lines"], ["description"]),
            lines_path(),
            description_path(),
        )
        .expect("static collection and descendant paths should be supported")
        .register(routed_error);

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(
        form.validation_errors()[0].target(),
        ValidationTarget::field(static_override_path())
    );
    assert_eq!(
        form.validation_errors()[0].error().provenance,
        DiagnosticRouteProvenance::ExactStaticTarget
    );
}

#[test]
fn captured_exact_mapping_never_overrides_the_live_collection_rule() {
    let external_path = garde::Path::new("lines")
        .join(0usize)
        .join("description")
        .to_string();
    let mut form: FormCore<Invoice, RoutedError> = FormCore::new_with_error_type(Invoice {
        lines: vec![line("Retired"), line("Survivor")],
        emit_missing_and_unmapped: false,
    });
    let initial = form.collection_items(lines_path());
    let retired = initial[0].identity();
    let survivor = initial[1].identity();
    let captured_description = FieldPath::direct(
        FieldIdentity::collection_item("lines", retired, "description"),
        "lines[0].description",
        |invoice: &Invoice| &invoice.lines[0].description,
        |invoice: &mut Invoice| &mut invoice.lines[0].description,
    );
    form.garde_validation()
        .path_map(GardePathMap::new().with_field(external_path, captured_description))
        .collection_row_descendant(
            GardeCollectionRowMatcher::new(["lines"], ["description"]),
            lines_path(),
            description_path(),
        )
        .expect("static collection and descendant paths should be supported")
        .register(routed_error);

    form.remove_collection_item(lines_path(), retired)
        .expect("the captured row should exist");
    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(
        form.validation_errors()[0].target(),
        description_target(survivor)
    );
    assert_eq!(
        form.validation_errors()[0].error().provenance,
        DiagnosticRouteProvenance::CollectionValidationTargetRule
    );
}

#[test]
fn structural_matching_distinguishes_indices_from_numeric_and_bracketed_keys() {
    let unmapped = Rc::new(RefCell::new(Vec::new()));
    let unmapped_for_reporter = Rc::clone(&unmapped);
    let mut form = FormCore::new(StructuralPathForm {
        lines: vec![line("Design")],
    });
    form.garde_validation()
        .collection_row_descendant(
            GardeCollectionRowMatcher::new(["lines"], ["description"]),
            structural_lines_path(),
            description_path(),
        )
        .expect("static collection and descendant paths should be supported")
        .on_unmapped_path(move |path| unmapped_for_reporter.borrow_mut().push(path.clone()))
        .register_string_errors();
    let item = form.collection_items(structural_lines_path())[0].identity();

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(unmapped.borrow().len(), 2);
    assert!(
        unmapped
            .borrow()
            .contains(&garde::Path::new("lines").join("0").join("description"))
    );
    assert!(
        unmapped
            .borrow()
            .contains(&garde::Path::new("lines").join("[0]").join("description"))
    );
    assert_eq!(form.form_validation_errors().len(), 2);
    assert_eq!(
        form.field_validation_errors(FieldPath::direct(
            FieldIdentity::collection_item("lines", item, "description"),
            "lines[0].description",
            |form: &StructuralPathForm| &form.lines[0].description,
            |form: &mut StructuralPathForm| &mut form.lines[0].description,
        ))
        .len(),
        1
    );
}

#[test]
fn row_routes_follow_insert_move_swap_item_replacement_reorder_and_clear() {
    let mut form = FormCore::new(Invoice {
        lines: vec![line("A"), line("B"), line("C")],
        emit_missing_and_unmapped: false,
    });
    form.garde_validation()
        .collection_row_descendant(
            GardeCollectionRowMatcher::new(["lines"], ["description"]),
            lines_path(),
            description_path(),
        )
        .expect("static collection and descendant paths should be supported")
        .register_string_errors();
    let initial = form.collection_items(lines_path());
    let [a, b, c] = [
        initial[0].identity(),
        initial[1].identity(),
        initial[2].identity(),
    ];

    let d = form
        .insert_collection_item(lines_path(), 1, line("D"))
        .expect("insertion should succeed");
    assert!(form.move_collection_item_to_index(lines_path(), c, 0));
    assert!(form.swap_collection_items(lines_path(), 1, 3));
    assert!(form.replace_collection_item(lines_path(), 2, line("D replaced")));
    form.validate_form(ValidationTrigger::Manual);

    let routed: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), error.error().clone()))
        .collect();
    assert_eq!(
        routed,
        vec![
            (description_target(c), "C".to_owned()),
            (description_target(b), "B".to_owned()),
            (description_target(d), "D replaced".to_owned()),
            (description_target(a), "A".to_owned()),
        ]
    );

    assert_eq!(form.clear_collection_items(lines_path()).len(), 4);
    form.validate_form(ValidationTrigger::Manual);
    assert!(form.validation_errors().is_empty());
}

#[test]
fn context_aware_custom_and_string_terminals_register_live_collection_rules() {
    let mut custom: FormCore<Invoice, RoutedError> = FormCore::new_with_error_type(Invoice {
        lines: vec![line("Custom")],
        emit_missing_and_unmapped: false,
    });
    custom
        .garde_validation()
        .collection_row_descendant(
            GardeCollectionRowMatcher::new(["lines"], ["description"]),
            lines_path(),
            description_path(),
        )
        .expect("static collection and descendant paths should be supported")
        .register_with_context(|_context| (), routed_error);
    let custom_item = custom.collection_items(lines_path())[0].identity();

    custom.validate_form(ValidationTrigger::Manual);

    assert_eq!(
        custom.validation_errors()[0].target(),
        description_target(custom_item)
    );
    assert_eq!(
        custom.validation_errors()[0].error().provenance,
        DiagnosticRouteProvenance::CollectionValidationTargetRule
    );

    let mut strings = FormCore::new(Invoice {
        lines: vec![line("Initial")],
        emit_missing_and_unmapped: false,
    });
    strings
        .garde_validation()
        .collection_row_descendant(
            GardeCollectionRowMatcher::new(["lines"], ["description"]),
            lines_path(),
            description_path(),
        )
        .expect("static collection and descendant paths should be supported")
        .register_string_errors_with_context(|_context| ());
    let appended = strings.push_collection_item(lines_path(), line("Appended"));

    strings.validate_form(ValidationTrigger::Manual);

    assert_eq!(strings.validation_errors().len(), 2);
    assert_eq!(
        strings.validation_errors()[1].target(),
        description_target(appended)
    );
    assert_eq!(strings.validation_errors()[1].error(), "Appended");
}
