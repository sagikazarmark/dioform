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

use dioform_core::{
    FormCore, FormValidationError, FormValidatorContext, ValidationTriggers, ValidatorId,
    ValidatorSource,
};
use dioform_validation_adapter::{DiagnosticView, PathMap};

/// The default source label used by the `garde` adapter.
pub const DEFAULT_GARDE_SOURCE: &str = "garde";

/// Explicit exact-path mapping from external `garde` diagnostic paths to typed form fields.
///
/// Paths are keyed by the canonical `garde::Path::to_string` representation. Unknown paths map
/// to form-level validation targets so external diagnostics are preserved as form-level validation
/// errors instead of being dropped. This is a re-export of the shared
/// [`PathMap`]; resolve a `garde::Path` with
/// `target_for_path(&path.to_string())`.
pub type GardePathMap<Model> = PathMap<Model>;

/// A borrowed `garde` diagnostic paired with the Dioform target it resolved to.
///
/// This is the value passed to a `garde` mapper closure. Its [`path`](DiagnosticView::path) is the
/// original `garde::Path` and its [`error`](DiagnosticView::error) is the original `garde::Error`.
pub type GardeDiagnostic<'a> = DiagnosticView<'a, garde::Path, garde::Error>;

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
        }
    }
}

/// Builder for one registered `garde` form validator.
pub struct GardeValidationBuilder<'form, Model, Error> {
    form: &'form mut FormCore<Model, Error>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
    path_map: GardePathMap<Model>,
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
    /// Mapped paths attach to the registered typed field targets. Unmapped paths attach to the
    /// form, preserving unknown diagnostics without implicit field-name or Rust-field matching.
    pub fn path_map(mut self, path_map: GardePathMap<Model>) -> Self {
        self.path_map = path_map;
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
        } = self;

        form.register_sync_form_validator_for_triggers(source, triggers, move |context| {
            let Err(report) = garde::Validate::validate(context.form()) else {
                return Vec::new();
            };

            map_report(&report, &path_map, &mapper)
        })
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
        } = self;

        form.register_sync_form_validator_for_triggers(source, triggers, move |context| {
            let form = context.form();
            let garde_context = context_provider(context);
            let Err(report) = garde::Validate::validate_with(form, &garde_context) else {
                return Vec::new();
            };

            map_report(&report, &path_map, &mapper)
        })
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

fn map_report<Model, Error, Mapper>(
    report: &garde::Report,
    path_map: &GardePathMap<Model>,
    mapper: &Mapper,
) -> Vec<FormValidationError<Error>>
where
    Mapper: for<'diagnostic> Fn(GardeDiagnostic<'diagnostic>) -> Error,
{
    report
        .iter()
        .map(|(path, error)| {
            let target = path_map.target_for_path(&path.to_string());
            let diagnostic = GardeDiagnostic::new(path, error, target.clone());
            FormValidationError::for_target(target, mapper(diagnostic))
        })
        .collect()
}
