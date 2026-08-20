use dioform_core::{FieldIdentity, FieldPath};
use dioform_validation_adapter::PathMap;

struct Model {
    value: String,
}

fn value_path() -> FieldPath<Model, String> {
    FieldPath::direct(
        FieldIdentity::new("value"),
        "value",
        |model: &Model| &model.value,
        |model: &mut Model| &mut model.value,
    )
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
