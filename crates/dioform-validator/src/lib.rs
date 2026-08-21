//! Renderer-agnostic `validator` validation adapter for Dioform Core.

pub use dioform_core::CollectionValidationTargetRuleError;
use dioform_core::{
    CollectionValidationTargetRule, FieldPath, FormCore, FormValidationError, FormValidatorContext,
    ValidationTriggers, ValidatorId, ValidatorSource,
};
pub use dioform_validation_adapter::{
    CollectionValidationTargetResolutionFailure, DiagnosticRouteProvenance,
    DuplicateCollectionValidationTargetRule, ValidationAdapterConfigurationIssue,
};
use dioform_validation_adapter::{DiagnosticView, PathMap, route_diagnostic};
use validator::ValidationErrorsKind;

/// The default source label used by the `validator` adapter.
pub const DEFAULT_VALIDATOR_SOURCE: &str = "validator";

/// Explicit exact-path mapping from external `validator` diagnostic paths to typed form fields.
///
/// Paths are keyed by the adapter's canonical flattened external path (for example `account.email`).
/// Use [`ValidatorCollectionTargetRule`] rather than a captured-item exact target for collection
/// rows. Unregistered paths map to
/// [`ValidationTarget::form`](dioform_core::ValidationTarget::form) so **Unmapped Diagnostics** are
/// preserved as form-level validation errors instead of being dropped.
pub type ValidatorPathMap<Model> = PathMap<Model>;

/// A borrowed flattened `validator` diagnostic paired with its classified Dioform route.
///
/// This is the value passed to a `validator` mapper closure. Its [`path`](DiagnosticView::path) is
/// the adapter's canonical flattened external path (for example `account.email` or
/// `lines[0].quantity`) and its [`error`](DiagnosticView::error) is the original
/// `validator::ValidationError`. [`DiagnosticView::route_provenance`] describes whether an exact
/// target, live collection rule, resolution failure, or true miss selected the target.
pub type ValidatorDiagnostic<'a> = DiagnosticView<'a, str, validator::ValidationError>;

type UnmappedPathReporter = dyn Fn(&str) + 'static;
type CollectionResolutionFailureReporter =
    dyn Fn(&str, &CollectionValidationTargetResolutionFailure) + 'static;

/// A structural `validator` collection path with one list index between named components.
///
/// `before_index` names the fields traversed before `ValidationErrorsKind::List`, and
/// `after_index` names fields inside the selected row. Components are compared directly while the
/// validation error tree is traversed; they are never parsed from a flattened wildcard path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatorCollectionPath {
    before_index: Vec<String>,
    after_index: Vec<String>,
}

impl ValidatorCollectionPath {
    /// Creates a matcher with exactly one structural list index between the two component lists.
    pub fn new<Before, After, BeforeComponent, AfterComponent>(
        before_index: Before,
        after_index: After,
    ) -> Self
    where
        Before: IntoIterator<Item = BeforeComponent>,
        After: IntoIterator<Item = AfterComponent>,
        BeforeComponent: Into<String>,
        AfterComponent: Into<String>,
    {
        Self {
            before_index: before_index.into_iter().map(Into::into).collect(),
            after_index: after_index.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns the named components before the structural list index.
    pub fn before_index(&self) -> &[String] {
        &self.before_index
    }

    /// Returns the named components after the structural list index.
    pub fn after_index(&self) -> &[String] {
        &self.after_index
    }

    fn row_index(&self, path: &[ValidatorPathComponent<'_>]) -> Option<usize> {
        let expected_len = self.before_index.len() + 1 + self.after_index.len();
        if path.len() != expected_len {
            return None;
        }

        let (before, rest) = path.split_at(self.before_index.len());
        let (index, after) = rest.split_first()?;
        let ValidatorPathComponent::Index(index) = index else {
            return None;
        };
        let before_matches = before
            .iter()
            .zip(&self.before_index)
            .all(|(actual, expected)| actual.is_field(expected));
        let after_matches = after
            .iter()
            .zip(&self.after_index)
            .all(|(actual, expected)| actual.is_field(expected));

        (before_matches && after_matches).then_some(*index)
    }
}

/// One adapter matcher paired with its typed core collection target rule.
pub struct ValidatorCollectionTargetRule<Model> {
    path: ValidatorCollectionPath,
    core_rule: CollectionValidationTargetRule<Model>,
}

impl<Model> Clone for ValidatorCollectionTargetRule<Model> {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            core_rule: self.core_rule.clone(),
        }
    }
}

impl<Model> ValidatorCollectionTargetRule<Model> {
    /// Creates a rule that attaches each matched row diagnostic to its logical item value.
    pub fn item<Item>(
        path: ValidatorCollectionPath,
        collection: FieldPath<Model, Vec<Item>>,
    ) -> Result<Self, CollectionValidationTargetRuleError>
    where
        Model: 'static,
        Item: 'static,
    {
        Ok(Self {
            path,
            core_rule: CollectionValidationTargetRule::item(collection)?,
        })
    }

    /// Creates a rule targeting one static descendant of each logical collection item.
    pub fn descendant<Item, Value>(
        path: ValidatorCollectionPath,
        collection: FieldPath<Model, Vec<Item>>,
        descendant: FieldPath<Item, Value>,
    ) -> Result<Self, CollectionValidationTargetRuleError>
    where
        Model: 'static,
        Item: 'static,
        Value: 'static,
    {
        Ok(Self {
            path,
            core_rule: CollectionValidationTargetRule::descendant(collection, descendant)?,
        })
    }

    /// Returns the adapter-owned structural external path matcher.
    pub const fn path(&self) -> &ValidatorCollectionPath {
        &self.path
    }
}

/// Extension methods for registering `validator` validation on [`FormCore`].
pub trait ValidatorValidationExt<Model, Error> {
    /// Starts configuring a `validator` form validator.
    fn validator_validation(&mut self) -> ValidatorValidationBuilder<'_, Model, Error>;
}

impl<Model, Error> ValidatorValidationExt<Model, Error> for FormCore<Model, Error> {
    fn validator_validation(&mut self) -> ValidatorValidationBuilder<'_, Model, Error> {
        ValidatorValidationBuilder {
            form: self,
            source: ValidatorSource::new(DEFAULT_VALIDATOR_SOURCE),
            triggers: ValidationTriggers::all(),
            path_map: ValidatorPathMap::new(),
            collection_target_rules: Vec::new(),
            unmapped_path_reporter: None,
            collection_resolution_failure_reporter: None,
        }
    }
}

/// Builder for one registered `validator` form validator.
pub struct ValidatorValidationBuilder<'form, Model, Error> {
    form: &'form mut FormCore<Model, Error>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
    path_map: ValidatorPathMap<Model>,
    collection_target_rules: Vec<ValidatorCollectionTargetRule<Model>>,
    unmapped_path_reporter: Option<Box<UnmappedPathReporter>>,
    collection_resolution_failure_reporter: Option<Box<CollectionResolutionFailureReporter>>,
}

impl<Model, Error> ValidatorValidationBuilder<'_, Model, Error> {
    /// Overrides the validator source label. The default is `validator`.
    pub fn source<Source>(mut self, source: Source) -> Self
    where
        Source: Into<ValidatorSource>,
    {
        self.source = source.into();
        self
    }

    /// Overrides the validation triggers. The default is [`ValidationTriggers::all`].
    pub fn triggers<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Uses an explicit `validator` external-path map for field-level diagnostic attachment.
    ///
    /// Mapped paths attach to the registered typed field targets. **Unmapped Diagnostics** attach to
    /// the form without implicit field-name or Rust-field matching. The default map is empty.
    pub fn path_map(mut self, path_map: ValidatorPathMap<Model>) -> Self {
        self.path_map = path_map;
        self
    }

    /// Adds one structural collection-row matcher and typed live target rule.
    pub fn collection_target_rule(mut self, rule: ValidatorCollectionTargetRule<Model>) -> Self {
        self.collection_target_rules.push(rule);
        self
    }

    /// Returns every statically detectable exact-map and collection-matcher issue.
    pub fn configuration_issues(&self) -> Vec<ValidationAdapterConfigurationIssue> {
        let mut issues = self.path_map.configuration_issues();
        for first in 0..self.collection_target_rules.len() {
            for duplicate in (first + 1)..self.collection_target_rules.len() {
                if self.collection_target_rules[first].path
                    == self.collection_target_rules[duplicate].path
                {
                    issues.push(
                        ValidationAdapterConfigurationIssue::DuplicateCollectionRule(
                            DuplicateCollectionValidationTargetRule::new(first, duplicate),
                        ),
                    );
                }
            }
        }
        issues
    }

    /// Reports each **External Diagnostic Path** that the configured path map could not route.
    ///
    /// The reporter runs once per **Unmapped Diagnostic**, in flattened diagnostic order, and does
    /// not change where the diagnostic attaches. Duplicate diagnostics at one path produce duplicate
    /// reports. The reporter does not need to implement `Send`.
    pub fn on_unmapped_path<Reporter>(mut self, reporter: Reporter) -> Self
    where
        Reporter: Fn(&str) + 'static,
    {
        self.unmapped_path_reporter = Some(Box::new(reporter));
        self
    }

    /// Reports each collection diagnostic that could not select one field target for its run.
    ///
    /// The reporter runs once per diagnostic for ambiguous matching rules or a matched rule with an
    /// unresolved target. It does not run for true unmapped paths and cannot alter routing. The
    /// reporter does not need to implement `Send`.
    pub fn on_collection_resolution_failure<Reporter>(mut self, reporter: Reporter) -> Self
    where
        Reporter: Fn(&str, &CollectionValidationTargetResolutionFailure) + 'static,
    {
        self.collection_resolution_failure_reporter = Some(Box::new(reporter));
        self
    }

    /// Registers the `validator` validator and returns its [`ValidatorId`].
    ///
    /// The mapper converts each flattened external `validator` diagnostic into the application's
    /// shared validation error type. Registration has no validation side effects; validation runs
    /// only when the configured form validator is triggered through normal [`FormCore`] APIs.
    pub fn register<Mapper>(self, mapper: Mapper) -> ValidatorId
    where
        Model: validator::Validate + 'static,
        Mapper: for<'diagnostic> Fn(ValidatorDiagnostic<'diagnostic>) -> Error + 'static,
    {
        let Self {
            form,
            source,
            triggers,
            path_map,
            collection_target_rules,
            unmapped_path_reporter,
            collection_resolution_failure_reporter,
        } = self;

        let core_rules: Vec<_> = collection_target_rules
            .iter()
            .map(|rule| rule.core_rule.clone())
            .collect();

        form.register_sync_form_validator_for_triggers_with_collection_target_rules(
            source,
            triggers,
            core_rules,
            move |context| {
                let resolved_collection_targets =
                    resolve_collection_targets(&collection_target_rules, &context);
                let Err(errors) = validator::Validate::validate(context.form()) else {
                    return Vec::new();
                };

                flatten_errors(
                    &errors,
                    &path_map,
                    &resolved_collection_targets,
                    unmapped_path_reporter.as_deref(),
                    collection_resolution_failure_reporter.as_deref(),
                    &mapper,
                )
            },
        )
    }

    /// Registers a `validator` validator with a per-run external argument provider.
    ///
    /// Use this for models validated through `validator::ValidateArgs` (derived with
    /// `#[validate(context = ...)]`). The provider receives Dioform's [`FormValidatorContext`]
    /// for the current validation run and returns the owned external context value; the adapter
    /// passes a reference to it as the model's `ValidateArgs::Args`. The provider runs every time
    /// validation runs, not when the validator is registered.
    pub fn register_with_context<Context, ContextProvider, Mapper>(
        self,
        context_provider: ContextProvider,
        mapper: Mapper,
    ) -> ValidatorId
    where
        Model: for<'args> validator::ValidateArgs<'args, Args = &'args Context> + 'static,
        Context: 'static,
        ContextProvider:
            for<'context> Fn(FormValidatorContext<'context, Model>) -> Context + 'static,
        Mapper: for<'diagnostic> Fn(ValidatorDiagnostic<'diagnostic>) -> Error + 'static,
    {
        let Self {
            form,
            source,
            triggers,
            path_map,
            collection_target_rules,
            unmapped_path_reporter,
            collection_resolution_failure_reporter,
        } = self;

        let core_rules: Vec<_> = collection_target_rules
            .iter()
            .map(|rule| rule.core_rule.clone())
            .collect();

        form.register_sync_form_validator_for_triggers_with_collection_target_rules(
            source,
            triggers,
            core_rules,
            move |context| {
                let resolved_collection_targets =
                    resolve_collection_targets(&collection_target_rules, &context);
                let form = context.form();
                let args_context = context_provider(context);
                let Err(errors) = validator::ValidateArgs::validate_with_args(form, &args_context)
                else {
                    return Vec::new();
                };

                flatten_errors(
                    &errors,
                    &path_map,
                    &resolved_collection_targets,
                    unmapped_path_reporter.as_deref(),
                    collection_resolution_failure_reporter.as_deref(),
                    &mapper,
                )
            },
        )
    }
}

impl<Model> ValidatorValidationBuilder<'_, Model, String> {
    /// Registers the `validator` validator by converting each diagnostic into a `String`.
    ///
    /// This is a convenience for simple forms whose shared validation error type is `String`. It
    /// stores the diagnostic message when present, otherwise the diagnostic code. The `String`
    /// itself is lossy: use [`register`](Self::register) with a custom enum or struct when the
    /// application needs to preserve the original external path, code, params, or selected
    /// validation target inside the error value.
    pub fn register_string_errors(self) -> ValidatorId
    where
        Model: validator::Validate + 'static,
    {
        self.register(validator_error_to_string)
    }

    /// Registers a context-aware `validator` validator that converts each diagnostic into a
    /// `String`.
    ///
    /// The context provider receives Dioform's [`FormValidatorContext`] and returns the owned
    /// external context value used as the model's `ValidateArgs::Args` for this validation run.
    /// The `String` value is lossy in the same way as [`register_string_errors`](Self::register_string_errors).
    pub fn register_string_errors_with_context<Context, ContextProvider>(
        self,
        context_provider: ContextProvider,
    ) -> ValidatorId
    where
        Model: for<'args> validator::ValidateArgs<'args, Args = &'args Context> + 'static,
        Context: 'static,
        ContextProvider:
            for<'context> Fn(FormValidatorContext<'context, Model>) -> Context + 'static,
    {
        self.register_with_context(context_provider, validator_error_to_string)
    }
}

fn validator_error_to_string(diagnostic: ValidatorDiagnostic<'_>) -> String {
    let error = diagnostic.error();
    error
        .message
        .as_ref()
        .map(|message| message.to_string())
        .unwrap_or_else(|| error.code.to_string())
}

fn flatten_errors<Model, Error, Mapper>(
    errors: &validator::ValidationErrors,
    path_map: &ValidatorPathMap<Model>,
    collection_target_rules: &[ResolvedValidatorCollectionTargetRule],
    unmapped_path_reporter: Option<&UnmappedPathReporter>,
    collection_resolution_failure_reporter: Option<&CollectionResolutionFailureReporter>,
    mapper: &Mapper,
) -> Vec<FormValidationError<Error>>
where
    Mapper: for<'diagnostic> Fn(ValidatorDiagnostic<'diagnostic>) -> Error,
{
    let mut output = Vec::new();
    let mut path = Vec::new();
    let routing = ValidatorErrorRouting {
        path_map,
        collection_target_rules,
        unmapped_path_reporter,
        collection_resolution_failure_reporter,
        mapper,
    };
    collect_errors(errors, &mut path, &routing, &mut output);
    output
}

struct ValidatorErrorRouting<'a, Model, Mapper> {
    path_map: &'a ValidatorPathMap<Model>,
    collection_target_rules: &'a [ResolvedValidatorCollectionTargetRule],
    unmapped_path_reporter: Option<&'a UnmappedPathReporter>,
    collection_resolution_failure_reporter: Option<&'a CollectionResolutionFailureReporter>,
    mapper: &'a Mapper,
}

fn collect_errors<'errors, Model, Error, Mapper>(
    errors: &'errors validator::ValidationErrors,
    path: &mut Vec<ValidatorPathComponent<'errors>>,
    routing: &ValidatorErrorRouting<'_, Model, Mapper>,
    output: &mut Vec<FormValidationError<Error>>,
) where
    Mapper: for<'diagnostic> Fn(ValidatorDiagnostic<'diagnostic>) -> Error,
{
    // `validator` stores field entries in a `HashMap`, whose iteration order is not stable. Sort
    // the field keys so the flattened diagnostics have deterministic ordering.
    let mut entries: Vec<(&str, &ValidationErrorsKind)> = errors
        .errors()
        .iter()
        .map(|(key, kind)| (key.as_ref(), kind))
        .collect();
    entries.sort_by(|left, right| left.0.cmp(right.0));

    for (key, kind) in entries {
        path.push(ValidatorPathComponent::Field(key));

        match kind {
            ValidationErrorsKind::Field(field_errors) => {
                let field_path = canonical_path(path);
                let route = route_for_path(
                    path,
                    &field_path,
                    routing.path_map,
                    routing.collection_target_rules,
                );
                // Per-field error vector order is preserved as reported by `validator`.
                for error in field_errors {
                    if matches!(
                        route.provenance(),
                        DiagnosticRouteProvenance::UnmappedDiagnostic
                    ) && let Some(reporter) = routing.unmapped_path_reporter
                    {
                        reporter(&field_path);
                    }
                    if let DiagnosticRouteProvenance::CollectionValidationTargetResolutionFailure(
                        failure,
                    ) = route.provenance()
                        && let Some(reporter) = routing.collection_resolution_failure_reporter
                    {
                        reporter(&field_path, failure);
                    }
                    let diagnostic =
                        ValidatorDiagnostic::from_route(&field_path, error, route.clone());
                    output.push(FormValidationError::for_target(
                        route.target(),
                        (routing.mapper)(diagnostic),
                    ));
                }
            }
            ValidationErrorsKind::Struct(inner) => {
                collect_errors(inner, path, routing, output);
            }
            ValidationErrorsKind::List(items) => {
                // `BTreeMap` iterates in ascending index order.
                for (index, inner) in items {
                    path.push(ValidatorPathComponent::Index(*index));
                    collect_errors(inner, path, routing, output);
                    path.pop();
                }
            }
        }

        path.pop();
    }
}

#[derive(Clone, Copy)]
enum ValidatorPathComponent<'a> {
    Field(&'a str),
    Index(usize),
}

impl ValidatorPathComponent<'_> {
    fn is_field(self, expected: &str) -> bool {
        matches!(self, Self::Field(actual) if actual == expected)
    }
}

fn route_for_path<Model>(
    path: &[ValidatorPathComponent<'_>],
    canonical_path: &str,
    path_map: &ValidatorPathMap<Model>,
    collection_target_rules: &[ResolvedValidatorCollectionTargetRule],
) -> dioform_validation_adapter::DiagnosticRoute {
    let candidates = collection_target_rules.iter().filter_map(|rule| {
        rule.path
            .row_index(path)
            .map(|index| rule.targets.get(index).cloned())
    });
    route_diagnostic(path_map.exact_target_for_path(canonical_path), candidates)
}

struct ResolvedValidatorCollectionTargetRule {
    path: ValidatorCollectionPath,
    targets: Vec<dioform_core::ValidationTarget>,
}

fn resolve_collection_targets<Model>(
    rules: &[ValidatorCollectionTargetRule<Model>],
    context: &FormValidatorContext<'_, Model>,
) -> Vec<ResolvedValidatorCollectionTargetRule> {
    rules
        .iter()
        .map(|rule| {
            let targets = (0..)
                .map_while(|index| rule.core_rule.resolve(context, index))
                .collect();
            ResolvedValidatorCollectionTargetRule {
                path: rule.path.clone(),
                targets,
            }
        })
        .collect()
}

fn canonical_path(path: &[ValidatorPathComponent<'_>]) -> String {
    let mut output = String::new();
    for component in path {
        match component {
            ValidatorPathComponent::Field(field) => {
                if !output.is_empty() {
                    output.push('.');
                }
                output.push_str(field);
            }
            ValidatorPathComponent::Index(index) => {
                output.push('[');
                output.push_str(&index.to_string());
                output.push(']');
            }
        }
    }
    output
}
