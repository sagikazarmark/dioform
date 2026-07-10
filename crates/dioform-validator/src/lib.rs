//! Renderer-agnostic `validator` validation adapter for Dioform Core.

use dioform_core::{
    FormCore, FormValidationError, FormValidatorContext, ValidationTriggers, ValidatorId,
    ValidatorSource,
};
use dioform_validation_adapter::{DiagnosticView, PathMap};
use validator::ValidationErrorsKind;

/// The default source label used by the `validator` adapter.
pub const DEFAULT_VALIDATOR_SOURCE: &str = "validator";

/// Explicit exact-path mapping from external `validator` diagnostic paths to typed form fields.
///
/// Paths are keyed by the adapter's canonical flattened external path (for example `account.email`
/// or `lines[0].quantity`). Unknown paths map to [`ValidationTarget::form`](dioform_core::ValidationTarget::form) so external diagnostics
/// are preserved as form-level validation errors instead of being dropped.
pub type ValidatorPathMap<Model> = PathMap<Model>;

/// A borrowed flattened `validator` diagnostic paired with the Dioform target it resolved to.
///
/// This is the value passed to a `validator` mapper closure. Its [`path`](DiagnosticView::path) is
/// the adapter's canonical flattened external path (for example `account.email` or
/// `lines[0].quantity`) and its [`error`](DiagnosticView::error) is the original
/// `validator::ValidationError`.
pub type ValidatorDiagnostic<'a> = DiagnosticView<'a, str, validator::ValidationError>;

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
        }
    }
}

/// Builder for one registered `validator` form validator.
pub struct ValidatorValidationBuilder<'form, Model, Error> {
    form: &'form mut FormCore<Model, Error>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
    path_map: ValidatorPathMap<Model>,
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
    /// Mapped paths attach to the registered typed field targets. Unmapped paths attach to the
    /// form, preserving unknown diagnostics without implicit field-name or Rust-field matching.
    pub fn path_map(mut self, path_map: ValidatorPathMap<Model>) -> Self {
        self.path_map = path_map;
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
        } = self;

        form.register_sync_form_validator_for_triggers(source, triggers, move |context| {
            let Err(errors) = validator::Validate::validate(context.form()) else {
                return Vec::new();
            };

            flatten_errors(&errors, &path_map, &mapper)
        })
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
        } = self;

        form.register_sync_form_validator_for_triggers(source, triggers, move |context| {
            let form = context.form();
            let args_context = context_provider(context);
            let Err(errors) = validator::ValidateArgs::validate_with_args(form, &args_context)
            else {
                return Vec::new();
            };

            flatten_errors(&errors, &path_map, &mapper)
        })
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
    mapper: &Mapper,
) -> Vec<FormValidationError<Error>>
where
    Mapper: for<'diagnostic> Fn(ValidatorDiagnostic<'diagnostic>) -> Error,
{
    let mut output = Vec::new();
    collect_errors(errors, "", path_map, mapper, &mut output);
    output
}

fn collect_errors<Model, Error, Mapper>(
    errors: &validator::ValidationErrors,
    prefix: &str,
    path_map: &ValidatorPathMap<Model>,
    mapper: &Mapper,
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
        let field_path = join_field(prefix, key);

        match kind {
            ValidationErrorsKind::Field(field_errors) => {
                let target = path_map.target_for_path(&field_path);
                // Per-field error vector order is preserved as reported by `validator`.
                for error in field_errors {
                    let diagnostic = ValidatorDiagnostic::new(&field_path, error, target.clone());
                    output.push(FormValidationError::for_target(
                        target.clone(),
                        mapper(diagnostic),
                    ));
                }
            }
            ValidationErrorsKind::Struct(inner) => {
                collect_errors(inner, &field_path, path_map, mapper, output);
            }
            ValidationErrorsKind::List(items) => {
                // `BTreeMap` iterates in ascending index order.
                for (index, inner) in items {
                    let indexed = format!("{field_path}[{index}]");
                    collect_errors(inner, &indexed, path_map, mapper, output);
                }
            }
        }
    }
}

fn join_field(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}
