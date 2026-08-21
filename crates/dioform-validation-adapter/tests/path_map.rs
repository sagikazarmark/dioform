use dioform_core::{FieldIdentity, FieldPath, FormCore, ValidationTarget};
use dioform_validation_adapter::{
    DuplicateCollectionValidationTargetRule, ExactPathLookup, PathMap,
    ValidationAdapterConfigurationIssue,
};

#[derive(Clone)]
struct Model {
    value: String,
    rows: Vec<String>,
}

fn value_path() -> FieldPath<Model, String> {
    FieldPath::direct(
        FieldIdentity::new("value"),
        "value",
        |model: &Model| &model.value,
        |model: &mut Model| &mut model.value,
    )
}

fn rows_path() -> FieldPath<Model, Vec<String>> {
    FieldPath::direct(
        FieldIdentity::new("rows"),
        "rows",
        |model: &Model| &model.rows,
        |model: &mut Model| &mut model.rows,
    )
}

fn captured_item_path() -> FieldPath<Model, String> {
    let mut form = FormCore::new(Model {
        value: String::new(),
        rows: vec![String::new()],
    });
    let item = form.collection_items(rows_path())[0].identity();

    FieldPath::direct(
        FieldIdentity::collection_item_value("rows", item),
        "rows[0]",
        |model: &Model| &model.value,
        |model: &mut Model| &mut model.value,
    )
}

#[test]
fn exact_lookup_classifies_missing_static_and_captured_item_targets() {
    let path_map = PathMap::<Model>::new()
        .with_field("value", value_path())
        .with_field("rows[0]", captured_item_path());

    assert_eq!(
        path_map.exact_target_for_path("missing"),
        ExactPathLookup::Missing
    );
    assert_eq!(
        path_map.exact_target_for_path("value"),
        ExactPathLookup::EligibleStatic(ValidationTarget::field(value_path()))
    );
    assert_eq!(
        path_map.exact_target_for_path("rows[0]"),
        ExactPathLookup::IneligibleCapturedCollectionItem(ValidationTarget::field(
            captured_item_path()
        ))
    );
}

#[test]
fn target_for_path_fails_captured_item_mappings_to_the_form() {
    let path_map = PathMap::<Model>::new().with_field("rows[0]", captured_item_path());

    assert_eq!(
        path_map.target_for_path("rows[0]"),
        ValidationTarget::form()
    );
}

#[test]
fn configuration_issues_report_every_ineligible_exact_target() {
    let path_map = PathMap::<Model>::new()
        .with_field("rows[1]", captured_item_path())
        .with_field("value", value_path())
        .with_field("rows[0]", captured_item_path());

    let issues = path_map.configuration_issues();

    assert_eq!(issues.len(), 2);
    assert!(matches!(
        &issues[0],
        ValidationAdapterConfigurationIssue::IneligibleExactTarget(issue)
            if issue.external_path() == "rows[0]"
                && issue.target() == ValidationTarget::field(captured_item_path())
    ));
    assert!(matches!(
        &issues[1],
        ValidationAdapterConfigurationIssue::IneligibleExactTarget(issue)
            if issue.external_path() == "rows[1]"
                && issue.target() == ValidationTarget::field(captured_item_path())
    ));
}

#[test]
fn adapters_can_aggregate_duplicate_collection_rule_and_path_map_issues() {
    let path_map = PathMap::<Model>::new().with_field("rows[0]", captured_item_path());
    let mut issues = path_map.configuration_issues();
    issues.push(
        ValidationAdapterConfigurationIssue::DuplicateCollectionRule(
            DuplicateCollectionValidationTargetRule::new(1, 3),
        ),
    );

    assert_eq!(issues.len(), 2);
    assert!(matches!(
        &issues[1],
        ValidationAdapterConfigurationIssue::DuplicateCollectionRule(issue)
            if issue.first_rule_index() == 1 && issue.duplicate_rule_index() == 3
    ));
}

#[test]
fn path_map_reports_its_registered_path_count() {
    let mut path_map = PathMap::<Model>::new();

    assert!(path_map.is_empty());
    assert_eq!(path_map.len(), 0);

    path_map.insert_field("value", value_path());

    assert!(!path_map.is_empty());
    assert_eq!(path_map.len(), 1);

    path_map.insert_field("value", value_path());

    assert_eq!(path_map.len(), 1);
}

#[test]
fn path_map_debug_does_not_require_the_model_to_implement_debug() {
    let path_map = PathMap::<Model>::new().with_field("value", value_path());

    let debug = format!("{path_map:?}");

    assert!(debug.contains("PathMap"));
    assert!(debug.contains("value"));
}
