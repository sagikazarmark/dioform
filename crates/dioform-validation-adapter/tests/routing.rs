use dioform_core::{FieldIdentity, FieldPath, ValidationTarget};
use dioform_validation_adapter::{
    CollectionValidationTargetResolutionFailure, DiagnosticRouteProvenance, ExactPathLookup,
    route_diagnostic,
};

struct Model {
    primary: String,
    collection_candidate: String,
}

fn primary_path() -> FieldPath<Model, String> {
    FieldPath::direct(
        FieldIdentity::new("primary"),
        "primary",
        |model: &Model| &model.primary,
        |model: &mut Model| &mut model.primary,
    )
}

fn collection_candidate_path() -> FieldPath<Model, String> {
    FieldPath::direct(
        FieldIdentity::new("collection_candidate"),
        "collection_candidate",
        |model: &Model| &model.collection_candidate,
        |model: &mut Model| &mut model.collection_candidate,
    )
}

#[test]
fn eligible_exact_static_target_wins_over_collection_candidates() {
    let exact_target = ValidationTarget::field(primary_path());
    let route = route_diagnostic(
        ExactPathLookup::EligibleStatic(exact_target.clone()),
        [Some(ValidationTarget::field(collection_candidate_path()))],
    );

    assert_eq!(route.target(), exact_target);
    assert_eq!(
        route.provenance(),
        &DiagnosticRouteProvenance::ExactStaticTarget
    );
}

#[test]
fn one_live_collection_candidate_routes_to_its_current_target() {
    let collection_target = ValidationTarget::field(collection_candidate_path());
    let route = route_diagnostic(ExactPathLookup::Missing, [Some(collection_target.clone())]);

    assert_eq!(route.target(), collection_target);
    assert_eq!(
        route.provenance(),
        &DiagnosticRouteProvenance::CollectionValidationTargetRule
    );
}

#[test]
fn ineligible_exact_target_falls_through_to_one_live_collection_candidate() {
    let rejected_exact = ValidationTarget::field(primary_path());
    let collection_target = ValidationTarget::field(collection_candidate_path());
    let route = route_diagnostic(
        ExactPathLookup::IneligibleCapturedCollectionItem(rejected_exact),
        [Some(collection_target.clone())],
    );

    assert_eq!(route.target(), collection_target);
    assert_eq!(
        route.provenance(),
        &DiagnosticRouteProvenance::CollectionValidationTargetRule
    );
}

#[test]
fn multiple_collection_candidates_fail_to_form_as_ambiguous() {
    let route = route_diagnostic(
        ExactPathLookup::Missing,
        [
            None,
            Some(ValidationTarget::field(collection_candidate_path())),
        ],
    );

    assert_eq!(route.target(), ValidationTarget::form());
    assert_eq!(
        route.provenance(),
        &DiagnosticRouteProvenance::CollectionValidationTargetResolutionFailure(
            CollectionValidationTargetResolutionFailure::AmbiguousMatchingRules { match_count: 2 },
        )
    );
}

#[test]
fn matched_collection_candidate_with_an_unresolved_target_fails_to_form() {
    let route = route_diagnostic(ExactPathLookup::Missing, [None]);

    assert_eq!(route.target(), ValidationTarget::form());
    assert_eq!(
        route.provenance(),
        &DiagnosticRouteProvenance::CollectionValidationTargetResolutionFailure(
            CollectionValidationTargetResolutionFailure::UnresolvedTarget,
        )
    );
}

#[test]
fn no_eligible_exact_mapping_or_collection_candidate_is_a_true_miss() {
    let route = route_diagnostic(ExactPathLookup::Missing, []);

    assert_eq!(route.target(), ValidationTarget::form());
    assert_eq!(
        route.provenance(),
        &DiagnosticRouteProvenance::UnmappedDiagnostic
    );
}

#[test]
fn ineligible_exact_target_without_a_collection_candidate_is_a_true_miss() {
    let route = route_diagnostic(
        ExactPathLookup::IneligibleCapturedCollectionItem(ValidationTarget::field(primary_path())),
        [],
    );

    assert_eq!(route.target(), ValidationTarget::form());
    assert_eq!(
        route.provenance(),
        &DiagnosticRouteProvenance::UnmappedDiagnostic
    );
}
