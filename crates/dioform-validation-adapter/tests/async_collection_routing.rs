use dioform_core::{
    CollectionValidationTargetRule, FieldIdentity, FieldPath, FormCore, FormValidationError,
    ValidationTarget, ValidationTrigger,
};
use dioform_validation_adapter::{
    CollectionValidationTargetResolutionFailure, DiagnosticRouteProvenance, ExactPathLookup,
    route_diagnostic,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Model {
    rows: Vec<Row>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Row {
    value: String,
}

fn rows_path() -> FieldPath<Model, Vec<Row>> {
    FieldPath::direct(
        FieldIdentity::new("rows"),
        "rows",
        |model: &Model| &model.rows,
        |model: &mut Model| &mut model.rows,
    )
}

fn value_path() -> FieldPath<Row, String> {
    FieldPath::direct(
        FieldIdentity::new("value"),
        "value",
        |row: &Row| &row.value,
        |row: &mut Row| &mut row.value,
    )
}

fn row(value: &str) -> Row {
    Row {
        value: value.to_owned(),
    }
}

#[test]
fn third_party_adapter_routes_diagnostics_against_the_async_validation_addressing_snapshot() {
    let mut form: FormCore<Model, &'static str> = FormCore::new_with_error_type(Model {
        rows: vec![row("first"), row("second")],
    });
    let registered_rule = CollectionValidationTargetRule::descendant(rows_path(), value_path())
        .expect("static collection targeting should be supported");
    let validator = form.register_async_form_validator_for_triggers_with_collection_target_rules(
        "synthetic adapter",
        ValidationTrigger::Manual,
        [registered_rule],
    );
    let validated_first = form.collection_items(rows_path())[0].identity();
    let run = form
        .begin_async_form_validation(validator, ValidationTrigger::Manual)
        .expect("the async validator should start");

    assert!(form.move_collection_item_to_index(rows_path(), validated_first, 1));

    let reconstructed_rule = CollectionValidationTargetRule::descendant(rows_path(), value_path())
        .expect("an equivalent rule shape should be supported");
    let context = run.validator_context();
    let route = route_diagnostic(
        ExactPathLookup::Missing,
        [context.resolve_collection_target(&reconstructed_rule, 0)],
    );
    let expected_target = ValidationTarget::field_identity(FieldIdentity::collection_item(
        "rows",
        validated_first,
        "value",
    ));

    assert_eq!(route.target(), expected_target);
    assert_eq!(
        route.provenance(),
        &DiagnosticRouteProvenance::CollectionValidationTargetRule
    );
    assert_eq!(
        form.complete_async_form_validation(
            validator,
            &run,
            [FormValidationError::for_target(
                route.target(),
                "old diagnostic",
            )],
        ),
        None
    );
    assert!(form.validation_errors().is_empty());

    let unresolved = route_diagnostic(
        ExactPathLookup::Missing,
        [context.resolve_collection_target(&reconstructed_rule, 99)],
    );
    assert_eq!(unresolved.target(), ValidationTarget::form());
    assert_eq!(
        unresolved.provenance(),
        &DiagnosticRouteProvenance::CollectionValidationTargetResolutionFailure(
            CollectionValidationTargetResolutionFailure::UnresolvedTarget,
        )
    );

    let unmapped = route_diagnostic(ExactPathLookup::Missing, []);
    assert_eq!(
        unmapped.provenance(),
        &DiagnosticRouteProvenance::UnmappedDiagnostic
    );
}
