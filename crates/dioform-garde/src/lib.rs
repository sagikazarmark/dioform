//! Renderer-agnostic `garde` validation adapter for Dioform Core.
//!
//! This crate is an opt-in validation adapter: it depends on `dioform-core` and `garde`, but
//! not on the Dioxus facade crate. The adapter registers one synchronous form-level validator and
//! maps every `garde::Report` diagnostic into the application's shared validation error type.
//! Simple forms whose validation error type is `String` can use
//! [`GardeValidationBuilder::register_string_errors`]. Richer applications should provide an
//! explicit mapper that preserves the external `garde` path, message, and selected Dioform
//! target in their own enum or struct error type.
//! Context-aware validation translates Dioform's [`FormValidatorContext`] into the separate
//! external `garde::Validate::Context` value passed to `garde::Validate::validate_with`.
//! See `docs/validation-adapters.md` in the workspace for usage patterns and dependency guidance.

use std::rc::Rc;

use dioform_core::{
    CollectionValidationTargetRule, CollectionValidationTargetRuleError, FieldPath, FormCore,
    FormValidationError, FormValidatorContext, ValidationTarget, ValidationTriggers, ValidatorId,
    ValidatorSource,
};
pub use dioform_validation_adapter::{
    CollectionValidationTargetResolutionFailure, DiagnosticRouteProvenance,
    ValidationAdapterConfigurationIssue,
};
use dioform_validation_adapter::{
    DiagnosticView, DuplicateCollectionValidationTargetRule, PathMap, route_diagnostic,
};

/// The default source label used by the `garde` adapter.
pub const DEFAULT_GARDE_SOURCE: &str = "garde";

/// Explicit exact-path mapping from external `garde` diagnostic paths to typed form fields.
///
/// Paths are keyed by the canonical `garde::Path::to_string` representation and are intended for
/// structurally static targets. Use [`GardeCollectionRowMatcher`] and the collection-row builder
/// methods for collection items. Unregistered paths and unsafe exact mappings that capture a
/// **Collection Item Identity** fail closed to form-level targets.
pub type GardePathMap<Model> = PathMap<Model>;

/// A borrowed `garde` diagnostic paired with the Dioform target it resolved to.
///
/// This is the value passed to a `garde` mapper closure. Its [`path`](DiagnosticView::path) is the
/// original `garde::Path` and its [`error`](DiagnosticView::error) is the original `garde::Error`.
/// [`DiagnosticView::route_provenance`] describes whether an exact mapping, live collection rule,
/// collection resolution failure, or true miss selected its target.
pub type GardeDiagnostic<'a> = DiagnosticView<'a, garde::Path, garde::Error>;

type UnmappedPathReporter = dyn Fn(&garde::Path) + 'static;
type CollectionResolutionFailureReporter =
    dyn Fn(&garde::Path, &CollectionValidationTargetResolutionFailure) + 'static;

/// A structured `garde::Path` matcher with exactly one collection row index.
///
/// Named components before and after the index are joined with public [`garde::Path`]
/// constructors. Matching uses structural path equality, so a numeric index is distinct from a
/// string key that happens to contain digits or brackets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GardeCollectionRowMatcher {
    named_before: Vec<String>,
    named_after: Vec<String>,
}

impl GardeCollectionRowMatcher {
    /// Creates a matcher from the named components before and after its one row index.
    pub fn new<Before, BeforeComponent, After, AfterComponent>(
        named_before: Before,
        named_after: After,
    ) -> Self
    where
        Before: IntoIterator<Item = BeforeComponent>,
        BeforeComponent: Into<String>,
        After: IntoIterator<Item = AfterComponent>,
        AfterComponent: Into<String>,
    {
        Self {
            named_before: named_before.into_iter().map(Into::into).collect(),
            named_after: named_after.into_iter().map(Into::into).collect(),
        }
    }

    fn path_for_row(&self, row: usize) -> garde::Path {
        let before = self
            .named_before
            .iter()
            .fold(garde::Path::empty(), |path, component| path.join(component));
        self.named_after
            .iter()
            .fold(before.join(row), |path, component| path.join(component))
    }

    fn matching_row(&self, path: &garde::Path) -> Option<usize> {
        // Display text only supplies a finite set of candidate numbers. Structural equality is the
        // authority, so digits in named keys (including `"0"` and `"[0]"`) cannot become indices.
        path.to_string()
            .split(|character: char| !character.is_ascii_digit())
            .filter_map(|digits| digits.parse().ok())
            .find(|row| self.path_for_row(*row) == *path)
    }
}

struct GardeCollectionRowRule<Model> {
    matcher: GardeCollectionRowMatcher,
    target_rule: CollectionValidationTargetRule<Model>,
    row_count: Rc<dyn Fn(&Model) -> usize>,
}

struct PreparedCollectionRowRule<'a> {
    matcher: &'a GardeCollectionRowMatcher,
    targets: Vec<Option<ValidationTarget>>,
}

/// Extension methods for registering `garde` validation on [`FormCore`].
pub trait GardeValidationExt<Model, Error> {
    /// Starts configuring a `garde` form validator.
    fn garde_validation(&mut self) -> GardeValidationBuilder<'_, Model, Error>;
}

impl<Model, Error> GardeValidationExt<Model, Error> for FormCore<Model, Error> {
    fn garde_validation(&mut self) -> GardeValidationBuilder<'_, Model, Error> {
        GardeValidationBuilder {
            form: self,
            source: ValidatorSource::new(DEFAULT_GARDE_SOURCE),
            triggers: ValidationTriggers::all(),
            path_map: GardePathMap::new(),
            collection_row_rules: Vec::new(),
            unmapped_path_reporter: None,
            collection_resolution_failure_reporter: None,
        }
    }
}

/// Builder for one registered `garde` form validator.
pub struct GardeValidationBuilder<'form, Model, Error> {
    form: &'form mut FormCore<Model, Error>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
    path_map: GardePathMap<Model>,
    collection_row_rules: Vec<GardeCollectionRowRule<Model>>,
    unmapped_path_reporter: Option<Box<UnmappedPathReporter>>,
    collection_resolution_failure_reporter: Option<Box<CollectionResolutionFailureReporter>>,
}

impl<Model, Error> GardeValidationBuilder<'_, Model, Error> {
    /// Overrides the validator source label. The default is `garde`.
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

    /// Uses an explicit `garde` external-path map for field-level diagnostic attachment.
    ///
    /// Mapped paths attach to the registered typed field targets. **Unmapped Diagnostics** attach to
    /// the form without implicit field-name or Rust-field matching. The default map is empty.
    pub fn path_map(mut self, path_map: GardePathMap<Model>) -> Self {
        self.path_map = path_map;
        self
    }

    /// Adds a live collection-row rule targeting each item value.
    pub fn collection_row_item<Item>(
        mut self,
        matcher: GardeCollectionRowMatcher,
        collection: FieldPath<Model, Vec<Item>>,
    ) -> Result<Self, CollectionValidationTargetRuleError>
    where
        Model: 'static,
        Item: 'static,
    {
        let target_rule = CollectionValidationTargetRule::item(collection.clone())?;
        self.collection_row_rules.push(GardeCollectionRowRule {
            matcher,
            target_rule,
            row_count: Rc::new(move |model| collection.get(model).len()),
        });
        Ok(self)
    }

    /// Adds a live collection-row rule targeting one static descendant of each item.
    pub fn collection_row_descendant<Item, Value>(
        mut self,
        matcher: GardeCollectionRowMatcher,
        collection: FieldPath<Model, Vec<Item>>,
        descendant: FieldPath<Item, Value>,
    ) -> Result<Self, CollectionValidationTargetRuleError>
    where
        Model: 'static,
        Item: 'static,
        Value: 'static,
    {
        let target_rule =
            CollectionValidationTargetRule::descendant(collection.clone(), descendant)?;
        self.collection_row_rules.push(GardeCollectionRowRule {
            matcher,
            target_rule,
            row_count: Rc::new(move |model| collection.get(model).len()),
        });
        Ok(self)
    }

    /// Returns all statically detectable routing configuration issues.
    ///
    /// Registration remains infallible: applications can inspect these issues before choosing a
    /// terminal registration method.
    pub fn configuration_issues(&self) -> Vec<ValidationAdapterConfigurationIssue> {
        let mut issues = self.path_map.configuration_issues();
        for (duplicate_rule_index, duplicate) in self.collection_row_rules.iter().enumerate() {
            let Some(first_rule_index) = self.collection_row_rules[..duplicate_rule_index]
                .iter()
                .position(|rule| rule.matcher == duplicate.matcher)
            else {
                continue;
            };
            issues.push(
                ValidationAdapterConfigurationIssue::DuplicateCollectionRule(
                    DuplicateCollectionValidationTargetRule::new(
                        first_rule_index,
                        duplicate_rule_index,
                    ),
                ),
            );
        }
        issues
    }

    /// Reports each **External Diagnostic Path** that the configured path map could not route.
    ///
    /// The reporter runs once per **Unmapped Diagnostic**, in `garde` report order, and does not
    /// change where the diagnostic attaches. Genuinely whole-model diagnostics use an empty
    /// `garde::Path` and are not reported as unmapped. The reporter does not need to implement
    /// `Send`.
    pub fn on_unmapped_path<Reporter>(mut self, reporter: Reporter) -> Self
    where
        Reporter: Fn(&garde::Path) + 'static,
    {
        self.unmapped_path_reporter = Some(Box::new(reporter));
        self
    }

    /// Reports each collection target resolution failure without changing its form fallback.
    ///
    /// The reporter runs once per failed diagnostic in `garde` report order and does not need to
    /// implement `Send`. True unmapped diagnostics continue to use
    /// [`on_unmapped_path`](Self::on_unmapped_path) instead.
    pub fn on_collection_resolution_failure<Reporter>(mut self, reporter: Reporter) -> Self
    where
        Reporter: Fn(&garde::Path, &CollectionValidationTargetResolutionFailure) + 'static,
    {
        self.collection_resolution_failure_reporter = Some(Box::new(reporter));
        self
    }

    /// Registers the `garde` validator and returns its [`ValidatorId`].
    ///
    /// The mapper converts each external `garde` diagnostic into the application's shared
    /// validation error type. Registration has no validation side effects; validation runs only
    /// when the configured form validator is triggered through normal [`FormCore`] APIs. Use
    /// [`register_with_context`](Self::register_with_context) when the model's
    /// `garde::Validate::Context` is not `()`.
    pub fn register<Mapper>(self, mapper: Mapper) -> ValidatorId
    where
        Model: garde::Validate<Context = ()> + 'static,
        Mapper: for<'diagnostic> Fn(GardeDiagnostic<'diagnostic>) -> Error + 'static,
    {
        let Self {
            form,
            source,
            triggers,
            path_map,
            collection_row_rules,
            unmapped_path_reporter,
            collection_resolution_failure_reporter,
        } = self;

        let durable_rules = collection_row_rules
            .iter()
            .map(|rule| rule.target_rule.clone())
            .collect::<Vec<_>>();

        form.register_sync_form_validator_for_triggers_with_collection_target_rules(
            source,
            triggers,
            durable_rules,
            move |context| {
                let prepared_collection_rows =
                    prepare_collection_rows(&context, &collection_row_rules);
                let Err(report) = garde::Validate::validate(context.form()) else {
                    return Vec::new();
                };

                map_report(
                    &report,
                    &path_map,
                    &prepared_collection_rows,
                    unmapped_path_reporter.as_deref(),
                    collection_resolution_failure_reporter.as_deref(),
                    &mapper,
                )
            },
        )
    }

    /// Registers the `garde` validator with a per-run external context provider.
    ///
    /// Use this for models whose `garde::Validate::Context` is not `()`. The provider receives
    /// Dioform's [`FormValidatorContext`] for the current validation run and returns the
    /// separate external `garde::Validate::Context` value passed to
    /// [`garde::Validate::validate_with`]. The provider runs every time validation runs, not when
    /// the validator is registered.
    pub fn register_with_context<ContextProvider, Mapper>(
        self,
        context_provider: ContextProvider,
        mapper: Mapper,
    ) -> ValidatorId
    where
        Model: garde::Validate + 'static,
        ContextProvider:
            for<'context> Fn(FormValidatorContext<'context, Model>) -> Model::Context + 'static,
        Mapper: for<'diagnostic> Fn(GardeDiagnostic<'diagnostic>) -> Error + 'static,
    {
        let Self {
            form,
            source,
            triggers,
            path_map,
            collection_row_rules,
            unmapped_path_reporter,
            collection_resolution_failure_reporter,
        } = self;

        let durable_rules = collection_row_rules
            .iter()
            .map(|rule| rule.target_rule.clone())
            .collect::<Vec<_>>();

        form.register_sync_form_validator_for_triggers_with_collection_target_rules(
            source,
            triggers,
            durable_rules,
            move |context| {
                let form = context.form();
                let prepared_collection_rows =
                    prepare_collection_rows(&context, &collection_row_rules);
                let garde_context = context_provider(context);
                let Err(report) = garde::Validate::validate_with(form, &garde_context) else {
                    return Vec::new();
                };

                map_report(
                    &report,
                    &path_map,
                    &prepared_collection_rows,
                    unmapped_path_reporter.as_deref(),
                    collection_resolution_failure_reporter.as_deref(),
                    &mapper,
                )
            },
        )
    }
}

impl<Model> GardeValidationBuilder<'_, Model, String> {
    /// Registers the `garde` validator by converting each diagnostic message into a `String`.
    ///
    /// This is a convenience for simple forms whose shared validation error type is `String`.
    /// It stores `diagnostic.error().to_string()` as the validation error value; use
    /// [`register`](Self::register) with a custom enum or struct when the application needs to
    /// preserve the original external path or selected validation target inside the error value.
    pub fn register_string_errors(self) -> ValidatorId
    where
        Model: garde::Validate<Context = ()> + 'static,
    {
        self.register(garde_error_to_string)
    }

    /// Registers a context-aware `garde` validator that converts each diagnostic message into a
    /// `String`.
    ///
    /// The context provider receives Dioform's [`FormValidatorContext`] and returns the
    /// separate external `garde::Validate::Context` value used for this validation run.
    pub fn register_string_errors_with_context<ContextProvider>(
        self,
        context_provider: ContextProvider,
    ) -> ValidatorId
    where
        Model: garde::Validate + 'static,
        ContextProvider:
            for<'context> Fn(FormValidatorContext<'context, Model>) -> Model::Context + 'static,
    {
        self.register_with_context(context_provider, garde_error_to_string)
    }
}

fn garde_error_to_string(diagnostic: GardeDiagnostic<'_>) -> String {
    diagnostic.error().to_string()
}

fn prepare_collection_rows<'a, Model>(
    context: &FormValidatorContext<'_, Model>,
    rules: &'a [GardeCollectionRowRule<Model>],
) -> Vec<PreparedCollectionRowRule<'a>> {
    rules
        .iter()
        .map(|rule| PreparedCollectionRowRule {
            matcher: &rule.matcher,
            targets: (0..(rule.row_count)(context.form()))
                .map(|row| rule.target_rule.resolve(context, row))
                .collect(),
        })
        .collect()
}

fn map_report<Model, Error, Mapper>(
    report: &garde::Report,
    path_map: &GardePathMap<Model>,
    collection_row_rules: &[PreparedCollectionRowRule<'_>],
    unmapped_path_reporter: Option<&UnmappedPathReporter>,
    collection_resolution_failure_reporter: Option<&CollectionResolutionFailureReporter>,
    mapper: &Mapper,
) -> Vec<FormValidationError<Error>>
where
    Mapper: for<'diagnostic> Fn(GardeDiagnostic<'diagnostic>) -> Error,
{
    report
        .iter()
        .map(|(path, error)| {
            let collection_candidates = collection_row_rules.iter().filter_map(|rule| {
                rule.matcher
                    .matching_row(path)
                    .map(|row| rule.targets.get(row).cloned().flatten())
            });
            let route = route_diagnostic(
                path_map.exact_target_for_path(&path.to_string()),
                collection_candidates,
            );
            if matches!(
                route.provenance(),
                DiagnosticRouteProvenance::UnmappedDiagnostic
            ) && !path.is_empty()
                && let Some(reporter) = unmapped_path_reporter
            {
                reporter(path);
            }
            if let DiagnosticRouteProvenance::CollectionValidationTargetResolutionFailure(failure) =
                route.provenance()
                && let Some(reporter) = collection_resolution_failure_reporter
            {
                reporter(path, failure);
            }
            let target = route.target();
            let diagnostic = GardeDiagnostic::from_route(path, error, route);
            FormValidationError::for_target(target, mapper(diagnostic))
        })
        .collect()
}
