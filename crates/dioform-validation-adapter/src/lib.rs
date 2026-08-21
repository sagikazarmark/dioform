//! Shared support for Dioform **Validation Adapters**.
//!
//! A **Validation Adapter** maps an external validation library's diagnostics into the form's shared
//! **Validation Error** type. Every adapter needs the same routing plumbing: a map from an **External
//! Diagnostic Path** to a typed **Validation Target**, a borrowed view of one external diagnostic
//! paired with the classified route it resolved through, and adapter-neutral collection-route
//! classification. This crate owns those pieces so each first-party adapter (`dioform-garde`,
//! `dioform-validator`, and any future adapter) does not re-derive them.
//!
//! What stays in each adapter is the part that genuinely differs: how the external library enumerates
//! its diagnostics, and the builder whose `register` bounds name that library's validation trait. The
//! field-versus-form routing lives in the **Form Core** as
//! [`FormValidationError::for_target`](dioform_core::FormValidationError::for_target); this crate
//! only bridges an **External Diagnostic Path** to the [`ValidationTarget`] that constructor consumes.

use std::{collections::BTreeMap, fmt, marker::PhantomData};

use dioform_core::{FieldPath, ValidationTarget};

/// The classified result of looking up one exact **External Diagnostic Path**.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactPathLookup {
    /// No exact mapping is registered for the external path.
    Missing,
    /// The exact mapping targets a structurally static field.
    EligibleStatic(ValidationTarget),
    /// The exact mapping captures one **Collection Item Identity** and cannot route safely.
    IneligibleCapturedCollectionItem(ValidationTarget),
}

/// Ephemeral information describing how an external diagnostic selected its target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticRouteProvenance {
    /// An eligible exact mapping selected a structurally static field target.
    ExactStaticTarget,
    /// Exactly one adapter-matched collection rule resolved a target for this validation run.
    CollectionValidationTargetRule,
    /// Collection-rule matching could not select one field target for this validation run.
    CollectionValidationTargetResolutionFailure(CollectionValidationTargetResolutionFailure),
    /// No eligible exact mapping or collection rule matched the diagnostic.
    UnmappedDiagnostic,
}

/// Why adapter-matched collection rules could not select one field target for this validation run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CollectionValidationTargetResolutionFailure {
    /// More than one collection rule matched the same external diagnostic.
    AmbiguousMatchingRules {
        /// The number of rules that matched.
        match_count: usize,
    },
    /// One matching rule could not resolve an authorized target for the emitted row index.
    UnresolvedTarget,
}

/// The classified routing result for one external diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticRoute {
    target: ValidationTarget,
    provenance: DiagnosticRouteProvenance,
}

impl DiagnosticRoute {
    /// Returns the selected Dioform target.
    pub fn target(&self) -> ValidationTarget {
        self.target.clone()
    }

    /// Returns how this diagnostic selected its target.
    pub const fn provenance(&self) -> &DiagnosticRouteProvenance {
        &self.provenance
    }

    /// Consumes the route and returns its selected target and provenance.
    pub fn into_parts(self) -> (ValidationTarget, DiagnosticRouteProvenance) {
        (self.target, self.provenance)
    }
}

/// Classifies one external diagnostic route without depending on adapter-specific path syntax.
///
/// Each collection candidate denotes one adapter-matched collection rule. `Some` carries its
/// resolved target; `None` denotes a matched rule that could not resolve an authorized target.
pub fn route_diagnostic(
    exact: ExactPathLookup,
    collection_candidates: impl IntoIterator<Item = Option<ValidationTarget>>,
) -> DiagnosticRoute {
    if let ExactPathLookup::EligibleStatic(target) = exact {
        return DiagnosticRoute {
            target,
            provenance: DiagnosticRouteProvenance::ExactStaticTarget,
        };
    }

    let collection_candidates: Vec<_> = collection_candidates.into_iter().collect();
    match collection_candidates.as_slice() {
        [Some(target)] => DiagnosticRoute {
            target: target.clone(),
            provenance: DiagnosticRouteProvenance::CollectionValidationTargetRule,
        },
        [None] => DiagnosticRoute {
            target: ValidationTarget::form(),
            provenance: DiagnosticRouteProvenance::CollectionValidationTargetResolutionFailure(
                CollectionValidationTargetResolutionFailure::UnresolvedTarget,
            ),
        },
        [_, _, ..] => DiagnosticRoute {
            target: ValidationTarget::form(),
            provenance: DiagnosticRouteProvenance::CollectionValidationTargetResolutionFailure(
                CollectionValidationTargetResolutionFailure::AmbiguousMatchingRules {
                    match_count: collection_candidates.len(),
                },
            ),
        },
        [] => DiagnosticRoute {
            target: ValidationTarget::form(),
            provenance: DiagnosticRouteProvenance::UnmappedDiagnostic,
        },
    }
}

/// One statically detectable validation-adapter routing configuration issue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationAdapterConfigurationIssue {
    /// An exact mapping captures one collection item's runtime identity.
    IneligibleExactTarget(IneligibleExactTarget),
    /// Two adapter-owned matchers configure the same collection rule shape.
    DuplicateCollectionRule(DuplicateCollectionValidationTargetRule),
}

/// The registration positions of two duplicate **Collection Validation Target Rules**.
///
/// Adapters determine duplication using their own matcher grammar and can append this issue to the
/// issues returned by [`PathMap::configuration_issues`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuplicateCollectionValidationTargetRule {
    first_rule_index: usize,
    duplicate_rule_index: usize,
}

impl DuplicateCollectionValidationTargetRule {
    /// Creates a duplicate-rule issue from adapter-owned registration positions.
    pub const fn new(first_rule_index: usize, duplicate_rule_index: usize) -> Self {
        Self {
            first_rule_index,
            duplicate_rule_index,
        }
    }

    /// Returns the registration position of the first rule.
    pub const fn first_rule_index(&self) -> usize {
        self.first_rule_index
    }

    /// Returns the registration position of the duplicate rule.
    pub const fn duplicate_rule_index(&self) -> usize {
        self.duplicate_rule_index
    }
}

/// An exact path mapping that captures one **Collection Item Identity** and cannot route safely.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IneligibleExactTarget {
    external_path: String,
    target: ValidationTarget,
}

impl IneligibleExactTarget {
    /// Returns the configured external path.
    pub fn external_path(&self) -> &str {
        &self.external_path
    }

    /// Returns the captured-item target that was rejected.
    pub fn target(&self) -> ValidationTarget {
        self.target.clone()
    }
}

/// A map from an **External Diagnostic Path** (the string an external validation library emits) to a
/// typed **Validation Target** in one **Form Model**.
///
/// Registered paths attach to their typed field targets; unregistered paths resolve to the form, so an
/// **Unmapped Diagnostic** is preserved as a form-level error rather than dropped or matched by field
/// name.
pub struct PathMap<Model> {
    targets: BTreeMap<String, ValidationTarget>,
    _marker: PhantomData<fn() -> Model>,
}

impl<Model> Clone for PathMap<Model> {
    fn clone(&self) -> Self {
        Self {
            targets: self.targets.clone(),
            _marker: PhantomData,
        }
    }
}

impl<Model> fmt::Debug for PathMap<Model> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PathMap")
            .field("targets", &self.targets)
            .finish()
    }
}

impl<Model> Default for PathMap<Model> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Model> PathMap<Model> {
    /// Creates an empty path map. All diagnostics resolve to the form until fields are registered.
    pub fn new() -> Self {
        Self {
            targets: BTreeMap::new(),
            _marker: PhantomData,
        }
    }

    /// Returns the number of registered external paths.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Returns whether no external paths are registered.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Returns a new path map with one exact external path registered to a typed field path.
    pub fn with_field<Value>(
        mut self,
        external_path: impl Into<String>,
        field: FieldPath<Model, Value>,
    ) -> Self {
        self.insert_field(external_path, field);
        self
    }

    /// Registers one exact external path to a typed field path.
    ///
    /// If the path was already mapped, returns the previous target.
    pub fn insert_field<Value>(
        &mut self,
        external_path: impl Into<String>,
        field: FieldPath<Model, Value>,
    ) -> Option<ValidationTarget> {
        self.targets
            .insert(external_path.into(), ValidationTarget::field(field))
    }

    /// Resolves an exact external path string into a Dioform validation target.
    ///
    /// Missing paths and mappings that capture a collection item both fail closed to the form.
    pub fn target_for_path(&self, external_path: &str) -> ValidationTarget {
        match self.exact_target_for_path(external_path) {
            ExactPathLookup::EligibleStatic(target) => target,
            ExactPathLookup::Missing | ExactPathLookup::IneligibleCapturedCollectionItem(_) => {
                ValidationTarget::form()
            }
        }
    }

    /// Classifies an exact external path without converting misses or unsafe captured-item mappings
    /// into an indistinguishable form target.
    pub fn exact_target_for_path(&self, external_path: &str) -> ExactPathLookup {
        let Some(target) = self.targets.get(external_path).cloned() else {
            return ExactPathLookup::Missing;
        };

        if target
            .as_field()
            .and_then(|field| field.collection_item_identity())
            .is_some()
        {
            ExactPathLookup::IneligibleCapturedCollectionItem(target)
        } else {
            ExactPathLookup::EligibleStatic(target)
        }
    }

    /// Returns every statically detectable issue in this exact path map.
    pub fn configuration_issues(&self) -> Vec<ValidationAdapterConfigurationIssue> {
        self.targets
            .iter()
            .filter_map(|(external_path, target)| {
                target
                    .as_field()
                    .and_then(|field| field.collection_item_identity())
                    .map(|_| {
                        ValidationAdapterConfigurationIssue::IneligibleExactTarget(
                            IneligibleExactTarget {
                                external_path: external_path.clone(),
                                target: target.clone(),
                            },
                        )
                    })
            })
            .collect()
    }
}

/// A borrowed view of one **External Validation Diagnostic** paired with the **Validation Target** it
/// resolved to.
///
/// This is the value an adapter hands to a mapper closure so the application can inspect the original
/// external path and error, and the chosen target, before mapping the diagnostic into the shared
/// **Validation Error** type. `Path` and `Err` are the external library's own types, borrowed for the
/// duration of one mapper call; both are `?Sized` so an adapter can view a `str` path directly.
pub struct DiagnosticView<'a, Path: ?Sized, Err: ?Sized> {
    path: &'a Path,
    error: &'a Err,
    target: ValidationTarget,
    route_provenance: Option<DiagnosticRouteProvenance>,
}

impl<'a, Path: ?Sized, Err: ?Sized> DiagnosticView<'a, Path, Err> {
    /// Pairs a borrowed external diagnostic with the target it resolved to.
    pub const fn new(path: &'a Path, error: &'a Err, target: ValidationTarget) -> Self {
        Self {
            path,
            error,
            target,
            route_provenance: None,
        }
    }

    /// Pairs a borrowed external diagnostic with a classified route and its provenance.
    pub fn from_route(path: &'a Path, error: &'a Err, route: DiagnosticRoute) -> Self {
        let (target, route_provenance) = route.into_parts();
        Self {
            path,
            error,
            target,
            route_provenance: Some(route_provenance),
        }
    }

    /// Returns the original external diagnostic path.
    pub const fn path(&self) -> &'a Path {
        self.path
    }

    /// Returns the original external diagnostic error.
    pub const fn error(&self) -> &'a Err {
        self.error
    }

    /// Returns the Dioform target selected for this diagnostic.
    pub fn target(&self) -> ValidationTarget {
        self.target.clone()
    }

    /// Returns how this diagnostic selected its target, when constructed from a classified route.
    pub const fn route_provenance(&self) -> Option<&DiagnosticRouteProvenance> {
        self.route_provenance.as_ref()
    }
}

impl<Path: ?Sized, Err: ?Sized> Clone for DiagnosticView<'_, Path, Err> {
    fn clone(&self) -> Self {
        Self {
            path: self.path,
            error: self.error,
            target: self.target.clone(),
            route_provenance: self.route_provenance.clone(),
        }
    }
}

impl<Path: ?Sized + fmt::Debug, Err: ?Sized + fmt::Debug> fmt::Debug
    for DiagnosticView<'_, Path, Err>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiagnosticView")
            .field("path", &self.path)
            .field("error", &self.error)
            .field("target", &self.target)
            .field("route_provenance", &self.route_provenance)
            .finish()
    }
}
