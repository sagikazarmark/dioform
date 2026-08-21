use dioform_core::{FieldIdentity, FieldPath, ValidationTarget};
use dioform_validation_adapter::{
    DiagnosticRouteProvenance, DiagnosticView, ExactPathLookup, route_diagnostic,
};

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
fn diagnostic_view_exposes_provenance_from_a_classified_route() {
    let path = "external.value";
    let error = "invalid";
    let target = ValidationTarget::field(value_path());
    let route = route_diagnostic(
        ExactPathLookup::EligibleStatic(target.clone()),
        std::iter::empty(),
    );

    let view = DiagnosticView::from_route(path, error, route);

    assert_eq!(view.path(), path);
    assert_eq!(view.error(), error);
    assert_eq!(view.target(), target);
    assert_eq!(
        view.route_provenance(),
        Some(&DiagnosticRouteProvenance::ExactStaticTarget)
    );
}

#[test]
fn compatibility_constructor_retains_target_without_claiming_provenance() {
    let target = ValidationTarget::field(value_path());

    let view = DiagnosticView::new("external.value", "invalid", target.clone());

    assert_eq!(view.target(), target);
    assert_eq!(view.route_provenance(), None);
}
