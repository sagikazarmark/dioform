use std::{cell::Cell, rc::Rc};

use dioform_core::{
    FieldIdentity, FieldPath, FormCore, FormValidationError, SubmitBlocker, SubmitError,
    SubmitErrors, SubmitResult, SubmitStatus, ValidationMode, ValidationStatus, ValidationTarget,
    ValidationTrigger, ValidationTriggers,
};
use dioform_garde::{GardeDiagnostic, GardePathMap, GardeValidationExt};

#[derive(Clone)]
struct SignupForm {
    email: String,
    password: String,
    validation_runs: Rc<Cell<usize>>,
}

impl SignupForm {
    fn new(email: &str, password: &str, validation_runs: Rc<Cell<usize>>) -> Self {
        Self {
            email: email.to_owned(),
            password: password.to_owned(),
            validation_runs,
        }
    }
}

impl garde::Validate for SignupForm {
    type Context = ();

    fn validate_into(
        &self,
        _ctx: &Self::Context,
        parent: &mut dyn FnMut() -> garde::Path,
        report: &mut garde::Report,
    ) {
        self.validation_runs.set(self.validation_runs.get() + 1);

        if self.email.trim().is_empty() {
            report.append(parent().join("email"), garde::Error::new("email required"));
        }

        if self.password.len() < 8 {
            report.append(
                parent().join("password"),
                garde::Error::new("password too short"),
            );
        }
    }
}

#[derive(Clone)]
struct RenamedSignupForm {
    email: String,
    validation_runs: Rc<Cell<usize>>,
}

impl RenamedSignupForm {
    fn new(email: &str, validation_runs: Rc<Cell<usize>>) -> Self {
        Self {
            email: email.to_owned(),
            validation_runs,
        }
    }
}

impl garde::Validate for RenamedSignupForm {
    type Context = ();

    fn validate_into(
        &self,
        _ctx: &Self::Context,
        parent: &mut dyn FnMut() -> garde::Path,
        report: &mut garde::Report,
    ) {
        self.validation_runs.set(self.validation_runs.get() + 1);

        if self.email.trim().is_empty() {
            report.append(
                parent().join("contact-email"),
                garde::Error::new("email required"),
            );
        }
    }
}

#[derive(Clone)]
struct ContextSignupForm {
    email: String,
    password: String,
    required_email_suffix: String,
    validation_runs: Rc<Cell<usize>>,
}

impl ContextSignupForm {
    fn new(
        email: &str,
        password: &str,
        required_email_suffix: &str,
        validation_runs: Rc<Cell<usize>>,
    ) -> Self {
        Self {
            email: email.to_owned(),
            password: password.to_owned(),
            required_email_suffix: required_email_suffix.to_owned(),
            validation_runs,
        }
    }
}

struct SignupValidationContext {
    email_suffix: String,
    minimum_password_length: usize,
}

impl garde::Validate for ContextSignupForm {
    type Context = SignupValidationContext;

    fn validate_into(
        &self,
        ctx: &Self::Context,
        parent: &mut dyn FnMut() -> garde::Path,
        report: &mut garde::Report,
    ) {
        self.validation_runs.set(self.validation_runs.get() + 1);

        if !self.email.ends_with(&ctx.email_suffix) {
            report.append(
                parent().join("email"),
                garde::Error::new(format!("email must end with {}", ctx.email_suffix)),
            );
        }

        if self.password.len() < ctx.minimum_password_length {
            report.append(
                parent().join("password"),
                garde::Error::new(format!(
                    "password must be at least {} characters",
                    ctx.minimum_password_length
                )),
            );
        }
    }
}

fn email_path() -> FieldPath<SignupForm, String> {
    FieldPath::direct(
        FieldIdentity::new("email"),
        "email",
        |model: &SignupForm| &model.email,
        |model: &mut SignupForm| &mut model.email,
    )
}

fn password_path() -> FieldPath<SignupForm, String> {
    FieldPath::direct(
        FieldIdentity::new("password"),
        "password",
        |model: &SignupForm| &model.password,
        |model: &mut SignupForm| &mut model.password,
    )
}

fn renamed_email_path() -> FieldPath<RenamedSignupForm, String> {
    FieldPath::direct(
        FieldIdentity::new("email"),
        "contact-email",
        |model: &RenamedSignupForm| &model.email,
        |model: &mut RenamedSignupForm| &mut model.email,
    )
}

fn context_email_path() -> FieldPath<ContextSignupForm, String> {
    FieldPath::direct(
        FieldIdentity::new("email"),
        "email",
        |model: &ContextSignupForm| &model.email,
        |model: &mut ContextSignupForm| &mut model.email,
    )
}

fn context_password_path() -> FieldPath<ContextSignupForm, String> {
    FieldPath::direct(
        FieldIdentity::new("password"),
        "password",
        |model: &ContextSignupForm| &model.password,
        |model: &mut ContextSignupForm| &mut model.password,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AppError {
    path: String,
    message: String,
    target: ValidationTarget,
}

fn app_error(diagnostic: GardeDiagnostic<'_>) -> AppError {
    AppError {
        path: diagnostic.path().to_string(),
        message: diagnostic.error().to_string(),
        target: diagnostic.target(),
    }
}

fn app_error_value(path: &str, message: &str, target: ValidationTarget) -> AppError {
    AppError {
        path: path.to_owned(),
        message: message.to_owned(),
        target,
    }
}

#[test]
fn path_map_resolves_exact_garde_paths_to_typed_field_targets() {
    let path_map = GardePathMap::new().with_field("account.email", email_path());

    assert_eq!(
        path_map.target_for_path(&garde::Path::new("account").join("email").to_string()),
        ValidationTarget::field(email_path()),
    );
    assert_eq!(path_map.target_for_path("email"), ValidationTarget::Form);
    assert_eq!(
        path_map.target_for_path("account.email.extra"),
        ValidationTarget::Form,
    );
}

#[test]
fn registering_garde_validation_returns_validator_id_without_running() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));

    let validator_id = form.garde_validation().register(app_error);

    assert_eq!(validator_id.as_u64(), 0);
    assert_eq!(runs.get(), 0);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Unknown),
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn unmapped_garde_report_diagnostics_attach_to_form_level_errors_in_report_order() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));
    let validator_id = form.garde_validation().register(app_error);

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );
    assert!(form.field_validation_errors(email_path()).is_empty());
    assert!(form.field_validation_errors(password_path()).is_empty());

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str().to_owned(),
                error.target(),
                error.error().clone(),
            )
        })
        .collect();

    assert_eq!(
        errors,
        vec![
            (
                Some(validator_id),
                "garde".to_owned(),
                ValidationTarget::Form,
                AppError {
                    path: "email".to_owned(),
                    message: "email required".to_owned(),
                    target: ValidationTarget::Form,
                },
            ),
            (
                Some(validator_id),
                "garde".to_owned(),
                ValidationTarget::Form,
                AppError {
                    path: "password".to_owned(),
                    message: "password too short".to_owned(),
                    target: ValidationTarget::Form,
                },
            ),
        ],
    );
}

#[test]
fn mapped_garde_paths_attach_to_typed_field_errors_in_report_order() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));
    let path_map = GardePathMap::new()
        .with_field("email", email_path())
        .with_field("password", password_path());
    let validator_id = form
        .garde_validation()
        .path_map(path_map)
        .register(app_error);

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );
    assert!(form.form_validation_errors().is_empty());
    assert_eq!(form.field_validation_errors(email_path()).len(), 1);
    assert_eq!(form.field_validation_errors(password_path()).len(), 1);

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str().to_owned(),
                error.target(),
                error.error().clone(),
            )
        })
        .collect();

    assert_eq!(
        errors,
        vec![
            (
                Some(validator_id),
                "garde".to_owned(),
                ValidationTarget::field(email_path()),
                AppError {
                    path: "email".to_owned(),
                    message: "email required".to_owned(),
                    target: ValidationTarget::field(email_path()),
                },
            ),
            (
                Some(validator_id),
                "garde".to_owned(),
                ValidationTarget::field(password_path()),
                AppError {
                    path: "password".to_owned(),
                    message: "password too short".to_owned(),
                    target: ValidationTarget::field(password_path()),
                },
            ),
        ],
    );
}

#[test]
fn string_error_convenience_maps_garde_messages_to_strings() {
    let runs = Rc::new(Cell::new(0));
    let mut form = FormCore::new(SignupForm::new("", "short", Rc::clone(&runs)));
    let validator_id = form
        .garde_validation()
        .path_map(GardePathMap::new().with_field("email", email_path()))
        .register_string_errors();

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );
    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str().to_owned(),
                error.target(),
                error.error().clone(),
            )
        })
        .collect();

    assert_eq!(
        errors,
        vec![
            (
                Some(validator_id),
                "garde".to_owned(),
                ValidationTarget::field(email_path()),
                "email required".to_owned(),
            ),
            (
                Some(validator_id),
                "garde".to_owned(),
                ValidationTarget::Form,
                "password too short".to_owned(),
            ),
        ],
    );
}

#[test]
fn garde_errors_coexist_with_native_validators_and_submit_errors_in_flattened_order() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));
    let native_field_id = form.register_sync_field_validator_for_triggers(
        email_path(),
        "native-field",
        ValidationTrigger::Manual,
        |_value, _context| {
            vec![app_error_value(
                "native.email",
                "native field error",
                ValidationTarget::field(email_path()),
            )]
        },
    );
    let native_form_id = form.register_sync_form_validator_for_triggers(
        "native-form",
        ValidationTrigger::Manual,
        |_context| {
            vec![
                FormValidationError::field(
                    password_path(),
                    app_error_value(
                        "native.password",
                        "native form field error",
                        ValidationTarget::field(password_path()),
                    ),
                ),
                FormValidationError::form(app_error_value(
                    "native.form",
                    "native form error",
                    ValidationTarget::Form,
                )),
            ]
        },
    );
    let path_map = GardePathMap::new()
        .with_field("email", email_path())
        .with_field("password", password_path());
    let garde_id = form
        .garde_validation()
        .triggers(ValidationTrigger::Manual)
        .path_map(path_map)
        .register(app_error);

    form.validate_all(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.submit(|_submitted| {
            SubmitErrors::with_source(
                "server",
                [
                    SubmitError::field(
                        email_path(),
                        app_error_value(
                            "submit.email",
                            "server email error",
                            ValidationTarget::field(email_path()),
                        ),
                    ),
                    SubmitError::form(app_error_value(
                        "submit.form",
                        "server form error",
                        ValidationTarget::Form,
                    )),
                ],
            )
        }),
        SubmitResult::Rejected,
    );
    assert_eq!(runs.get(), 1);
    assert_eq!(form.last_submit_status(), Some(SubmitStatus::Rejected));

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str().to_owned(),
                error.target(),
                error.error().clone(),
            )
        })
        .collect();

    assert_eq!(
        errors,
        vec![
            (
                Some(native_field_id),
                "native-field".to_owned(),
                ValidationTarget::field(email_path()),
                app_error_value(
                    "native.email",
                    "native field error",
                    ValidationTarget::field(email_path()),
                ),
            ),
            (
                Some(native_form_id),
                "native-form".to_owned(),
                ValidationTarget::field(password_path()),
                app_error_value(
                    "native.password",
                    "native form field error",
                    ValidationTarget::field(password_path()),
                ),
            ),
            (
                Some(native_form_id),
                "native-form".to_owned(),
                ValidationTarget::Form,
                app_error_value("native.form", "native form error", ValidationTarget::Form),
            ),
            (
                Some(garde_id),
                "garde".to_owned(),
                ValidationTarget::field(email_path()),
                AppError {
                    path: "email".to_owned(),
                    message: "email required".to_owned(),
                    target: ValidationTarget::field(email_path()),
                },
            ),
            (
                Some(garde_id),
                "garde".to_owned(),
                ValidationTarget::field(password_path()),
                AppError {
                    path: "password".to_owned(),
                    message: "password too short".to_owned(),
                    target: ValidationTarget::field(password_path()),
                },
            ),
            (
                None,
                "server".to_owned(),
                ValidationTarget::field(email_path()),
                app_error_value(
                    "submit.email",
                    "server email error",
                    ValidationTarget::field(email_path()),
                ),
            ),
            (
                None,
                "server".to_owned(),
                ValidationTarget::Form,
                app_error_value("submit.form", "server form error", ValidationTarget::Form),
            ),
        ],
    );
}

#[test]
fn field_name_overrides_do_not_affect_garde_mapping_without_explicit_external_path() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<RenamedSignupForm, AppError> =
        FormCore::new_with_error_type(RenamedSignupForm::new("", Rc::clone(&runs)));
    form.garde_validation().register(app_error);

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert!(
        form.field_validation_errors(renamed_email_path())
            .is_empty()
    );
    assert_eq!(form.form_validation_errors().len(), 1);
    assert_eq!(
        form.form_validation_errors()[0].error(),
        &AppError {
            path: "contact-email".to_owned(),
            message: "email required".to_owned(),
            target: ValidationTarget::Form,
        },
    );

    let mapped_runs = Rc::new(Cell::new(0));
    let mut mapped_form: FormCore<RenamedSignupForm, AppError> =
        FormCore::new_with_error_type(RenamedSignupForm::new("", Rc::clone(&mapped_runs)));
    let path_map = GardePathMap::new().with_field("contact-email", renamed_email_path());
    mapped_form
        .garde_validation()
        .path_map(path_map)
        .register(app_error);

    mapped_form.validate_form(ValidationTrigger::Manual);

    assert_eq!(mapped_runs.get(), 1);
    assert!(mapped_form.form_validation_errors().is_empty());
    assert_eq!(
        mapped_form.field_validation_errors(renamed_email_path())[0].error(),
        &AppError {
            path: "contact-email".to_owned(),
            message: "email required".to_owned(),
            target: ValidationTarget::field(renamed_email_path()),
        },
    );
}

#[test]
fn successful_garde_validation_clears_prior_adapter_errors() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));
    let validator_id = form.garde_validation().register(app_error);

    form.validate_form(ValidationTrigger::Manual);
    assert_eq!(form.validation_errors().len(), 2);

    form.set_field(email_path(), "ada@example.com".to_owned());
    form.set_field(password_path(), "long-enough".to_owned());
    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 2);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Valid),
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn rerunning_garde_replaces_only_adapter_errors_and_success_preserves_unrelated_errors() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));
    let native_field_id = form.register_sync_field_validator_for_triggers(
        email_path(),
        "native-field",
        ValidationTrigger::Manual,
        |_value, _context| {
            vec![app_error_value(
                "native.email",
                "native field error",
                ValidationTarget::field(email_path()),
            )]
        },
    );
    let native_form_id = form.register_sync_form_validator_for_triggers(
        "native-form",
        ValidationTrigger::Manual,
        |_context| {
            vec![FormValidationError::form(app_error_value(
                "native.form",
                "native form error",
                ValidationTarget::Form,
            ))]
        },
    );
    let path_map = GardePathMap::new()
        .with_field("email", email_path())
        .with_field("password", password_path());
    let garde_id = form
        .garde_validation()
        .triggers(ValidationTrigger::Manual)
        .path_map(path_map)
        .register(app_error);

    form.validate_all(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.submit(|_submitted| {
            SubmitErrors::with_source(
                "server",
                [SubmitError::form(app_error_value(
                    "submit.form",
                    "server form error",
                    ValidationTarget::Form,
                ))],
            )
        }),
        SubmitResult::Rejected,
    );

    form.set_field(email_path(), "ada@example.com".to_owned());
    assert_eq!(
        form.validate_form_validator(garde_id, ValidationTrigger::Manual),
        Some(ValidationStatus::Invalid),
    );

    assert_eq!(runs.get(), 2);
    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str().to_owned(),
                error.target(),
                error.error().clone(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                Some(native_field_id),
                "native-field".to_owned(),
                ValidationTarget::field(email_path()),
                app_error_value(
                    "native.email",
                    "native field error",
                    ValidationTarget::field(email_path()),
                ),
            ),
            (
                Some(native_form_id),
                "native-form".to_owned(),
                ValidationTarget::Form,
                app_error_value("native.form", "native form error", ValidationTarget::Form),
            ),
            (
                Some(garde_id),
                "garde".to_owned(),
                ValidationTarget::field(password_path()),
                AppError {
                    path: "password".to_owned(),
                    message: "password too short".to_owned(),
                    target: ValidationTarget::field(password_path()),
                },
            ),
            (
                None,
                "server".to_owned(),
                ValidationTarget::Form,
                app_error_value("submit.form", "server form error", ValidationTarget::Form),
            ),
        ],
    );

    form.set_field(password_path(), "long-enough".to_owned());
    assert_eq!(
        form.validate_form_validator(garde_id, ValidationTrigger::Manual),
        Some(ValidationStatus::Valid),
    );

    assert_eq!(runs.get(), 3);
    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.source().as_str().to_owned(),
                error.target(),
                error.error().clone(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                Some(native_field_id),
                "native-field".to_owned(),
                ValidationTarget::field(email_path()),
                app_error_value(
                    "native.email",
                    "native field error",
                    ValidationTarget::field(email_path()),
                ),
            ),
            (
                Some(native_form_id),
                "native-form".to_owned(),
                ValidationTarget::Form,
                app_error_value("native.form", "native form error", ValidationTarget::Form),
            ),
            (
                None,
                "server".to_owned(),
                ValidationTarget::Form,
                app_error_value("submit.form", "server form error", ValidationTarget::Form),
            ),
        ],
    );
}

#[test]
fn custom_source_and_trigger_configuration_are_used() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));
    let validator_id = form
        .garde_validation()
        .source("signup-garde")
        .triggers(ValidationTriggers::new([ValidationTrigger::Submit]))
        .register(app_error);

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 0);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Unknown),
    );
    assert!(form.validation_errors().is_empty());

    form.validate_form(ValidationTrigger::Submit);

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.validator_id(), error.source().as_str().to_owned()))
        .collect();

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );
    assert_eq!(
        errors,
        vec![
            (Some(validator_id), "signup-garde".to_owned()),
            (Some(validator_id), "signup-garde".to_owned()),
        ],
    );
}

#[test]
fn submit_triggered_garde_validation_blocks_submission_when_invalid() {
    let runs = Rc::new(Cell::new(0));
    let called = Cell::new(false);
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));
    let path_map = GardePathMap::new()
        .with_field("email", email_path())
        .with_field("password", password_path());
    let validator_id = form
        .garde_validation()
        .triggers(ValidationTrigger::Submit)
        .path_map(path_map)
        .register(app_error);

    let result = form.submit(|_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(!called.get());
    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );
    assert_eq!(
        form.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors)),
    );
    assert_eq!(form.visible_validation_errors().len(), 2);
}

#[test]
fn garde_value_change_validation_requires_form_policy_and_value_change_trigger() {
    let no_policy_runs = Rc::new(Cell::new(0));
    let mut no_policy_form: FormCore<SignupForm, AppError> = FormCore::new_with_error_type(
        SignupForm::new("ada@example.com", "long-enough", Rc::clone(&no_policy_runs)),
    );
    no_policy_form
        .garde_validation()
        .triggers(ValidationTrigger::Change)
        .register(app_error);

    no_policy_form.set_field(email_path(), "".to_owned());

    assert_eq!(no_policy_runs.get(), 0);
    assert!(no_policy_form.validation_errors().is_empty());

    let submit_only_runs = Rc::new(Cell::new(0));
    let mut submit_only_form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new(
            "ada@example.com",
            "long-enough",
            Rc::clone(&submit_only_runs),
        ))
        .with_validation_mode(ValidationMode::on_change());
    submit_only_form
        .garde_validation()
        .triggers(ValidationTrigger::Submit)
        .register(app_error);

    submit_only_form.set_field(email_path(), "".to_owned());

    assert_eq!(submit_only_runs.get(), 0);
    assert!(submit_only_form.validation_errors().is_empty());

    let value_change_runs = Rc::new(Cell::new(0));
    let mut value_change_form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new(
            "ada@example.com",
            "long-enough",
            Rc::clone(&value_change_runs),
        ))
        .with_validation_mode(ValidationMode::on_change());
    let validator_id = value_change_form
        .garde_validation()
        .triggers(ValidationTrigger::Change)
        .path_map(GardePathMap::new().with_field("email", email_path()))
        .register(app_error);

    value_change_form.set_field(email_path(), "".to_owned());

    assert_eq!(value_change_runs.get(), 1);
    assert_eq!(
        value_change_form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );
    assert_eq!(
        value_change_form
            .field_validation_errors(email_path())
            .len(),
        1
    );
}

#[test]
fn garde_initialization_validation_runs_only_when_explicitly_validated_and_triggered() {
    let submit_only_runs = Rc::new(Cell::new(0));
    let mut submit_only_form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&submit_only_runs)));
    let submit_only_id = submit_only_form
        .garde_validation()
        .triggers(ValidationTrigger::Submit)
        .register(app_error);

    assert!(submit_only_form.validate_initialization());

    assert_eq!(submit_only_runs.get(), 0);
    assert_eq!(
        submit_only_form.form_validation_status_by_id(submit_only_id),
        Some(ValidationStatus::Unknown),
    );
    assert!(submit_only_form.validation_errors().is_empty());

    let initialization_runs = Rc::new(Cell::new(0));
    let mut initialization_form: FormCore<SignupForm, AppError> = FormCore::new_with_error_type(
        SignupForm::new("", "short", Rc::clone(&initialization_runs)),
    );
    let initialization_id = initialization_form
        .garde_validation()
        .triggers(ValidationTrigger::Initial)
        .register(app_error);

    assert_eq!(initialization_runs.get(), 0);
    assert!(initialization_form.validation_errors().is_empty());
    assert!(!initialization_form.validate_initialization());

    assert_eq!(initialization_runs.get(), 1);
    assert_eq!(
        initialization_form.form_validation_status_by_id(initialization_id),
        Some(ValidationStatus::Invalid),
    );
    assert_eq!(initialization_form.validation_errors().len(), 2);
}

#[test]
fn garde_errors_follow_default_blur_and_submit_visibility() {
    let field_runs = Rc::new(Cell::new(0));
    let mut field_form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "long-enough", Rc::clone(&field_runs)));
    let field_validator_id = field_form
        .garde_validation()
        .triggers(ValidationTrigger::Manual)
        .path_map(GardePathMap::new().with_field("email", email_path()))
        .register(app_error);

    field_form.validate_form(ValidationTrigger::Manual);

    assert_eq!(field_runs.get(), 1);
    assert_eq!(field_form.field_validation_errors(email_path()).len(), 1);
    assert!(field_form.visible_validation_errors().is_empty());
    assert!(
        field_form
            .visible_field_validation_errors(email_path())
            .is_empty()
    );

    field_form.mark_field_blurred(email_path());

    let visible_field_errors: Vec<_> = field_form
        .visible_field_validation_errors(email_path())
        .into_iter()
        .map(|error| (error.validator_id(), error.source().as_str().to_owned()))
        .collect();
    assert_eq!(
        visible_field_errors,
        vec![(Some(field_validator_id), "garde".to_owned())],
    );

    let form_runs = Rc::new(Cell::new(0));
    let mut form_level_form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "long-enough", Rc::clone(&form_runs)));
    let form_validator_id = form_level_form
        .garde_validation()
        .triggers(ValidationTrigger::Manual)
        .register(app_error);

    form_level_form.validate_form(ValidationTrigger::Manual);

    assert_eq!(form_runs.get(), 1);
    assert_eq!(form_level_form.form_validation_errors().len(), 1);
    assert!(form_level_form.visible_form_validation_errors().is_empty());

    form_level_form.mark_submit_attempt();

    let visible_form_errors: Vec<_> = form_level_form
        .visible_form_validation_errors()
        .into_iter()
        .map(|error| (error.validator_id(), error.source().as_str().to_owned()))
        .collect();
    assert_eq!(
        visible_form_errors,
        vec![(Some(form_validator_id), "garde".to_owned())],
    );
}

#[test]
fn context_provider_derives_garde_context_from_form_draft_for_success_and_failure() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<ContextSignupForm, AppError> =
        FormCore::new_with_error_type(ContextSignupForm::new(
            "ada@example.net",
            "long-enough",
            "@example.com",
            Rc::clone(&runs),
        ));
    let path_map = GardePathMap::new().with_field("email", context_email_path());
    let validator_id = form
        .garde_validation()
        .path_map(path_map)
        .register_with_context(
            |context| SignupValidationContext {
                email_suffix: context.form().required_email_suffix.clone(),
                minimum_password_length: 0,
            },
            app_error,
        );

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );
    assert_eq!(form.field_validation_errors(context_email_path()).len(), 1);
    assert_eq!(
        form.field_validation_errors(context_email_path())[0].error(),
        &AppError {
            path: "email".to_owned(),
            message: "email must end with @example.com".to_owned(),
            target: ValidationTarget::field(context_email_path()),
        },
    );

    form.set_field(context_email_path(), "ada@example.com".to_owned());
    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 2);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Valid),
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn context_string_error_convenience_maps_garde_messages_to_strings() {
    let runs = Rc::new(Cell::new(0));
    let mut form = FormCore::new(ContextSignupForm::new(
        "ada@example.net",
        "long-enough",
        "@example.com",
        Rc::clone(&runs),
    ));
    let validator_id = form
        .garde_validation()
        .path_map(GardePathMap::new().with_field("email", context_email_path()))
        .register_string_errors_with_context(|context| SignupValidationContext {
            email_suffix: context.form().required_email_suffix.clone(),
            minimum_password_length: 0,
        });

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );
    assert_eq!(
        form.field_validation_errors(context_email_path())[0]
            .error()
            .as_str(),
        "email must end with @example.com",
    );
}

#[test]
fn context_provider_runs_for_each_validation_with_fresh_garde_context() {
    let runs = Rc::new(Cell::new(0));
    let provider_runs = Rc::new(Cell::new(0));
    let provider_runs_for_context = Rc::clone(&provider_runs);
    let mut form: FormCore<ContextSignupForm, AppError> =
        FormCore::new_with_error_type(ContextSignupForm::new(
            "ada@example.com",
            "long-enough",
            "@example.com",
            Rc::clone(&runs),
        ));
    let path_map = GardePathMap::new().with_field("email", context_email_path());
    let validator_id = form
        .garde_validation()
        .path_map(path_map)
        .register_with_context(
            move |_context| {
                let run = provider_runs_for_context.get() + 1;
                provider_runs_for_context.set(run);

                SignupValidationContext {
                    email_suffix: if run == 1 {
                        "@example.org".to_owned()
                    } else {
                        "@example.com".to_owned()
                    },
                    minimum_password_length: 0,
                }
            },
            app_error,
        );

    form.validate_form(ValidationTrigger::Manual);
    assert_eq!(provider_runs.get(), 1);
    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(provider_runs.get(), 2);
    assert_eq!(runs.get(), 2);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Valid),
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn context_provider_can_derive_garde_context_from_validation_trigger() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<ContextSignupForm, AppError> =
        FormCore::new_with_error_type(ContextSignupForm::new(
            "ada@example.com",
            "long-pass!",
            "@example.com",
            Rc::clone(&runs),
        ));
    let path_map = GardePathMap::new().with_field("password", context_password_path());
    let validator_id = form
        .garde_validation()
        .path_map(path_map)
        .register_with_context(
            |context| SignupValidationContext {
                email_suffix: "@example.com".to_owned(),
                minimum_password_length: match context.trigger() {
                    ValidationTrigger::Submit => 12,
                    _ => 8,
                },
            },
            app_error,
        );

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Valid),
    );
    assert!(form.validation_errors().is_empty());

    form.validate_form(ValidationTrigger::Submit);

    assert_eq!(runs.get(), 2);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );
    assert_eq!(
        form.field_validation_errors(context_password_path()).len(),
        1
    );
    assert_eq!(
        form.field_validation_errors(context_password_path())[0].error(),
        &AppError {
            path: "password".to_owned(),
            message: "password must be at least 12 characters".to_owned(),
            target: ValidationTarget::field(context_password_path()),
        },
    );
}

#[test]
fn context_aware_validation_replaces_own_source_errors_without_clearing_other_sources() {
    let runs = Rc::new(Cell::new(0));
    let use_matching_suffix = Rc::new(Cell::new(false));
    let use_matching_suffix_for_context = Rc::clone(&use_matching_suffix);
    let source_seen = Rc::new(Cell::new(false));
    let source_seen_for_context = Rc::clone(&source_seen);
    let mut form: FormCore<ContextSignupForm, AppError> =
        FormCore::new_with_error_type(ContextSignupForm::new(
            "ada@example.com",
            "long-enough",
            "@example.com",
            Rc::clone(&runs),
        ));
    form.register_sync_form_validator("native", |_context| {
        vec![FormValidationError::form(AppError {
            path: "native".to_owned(),
            message: "native invalid".to_owned(),
            target: ValidationTarget::Form,
        })]
    });
    let validator_id = form
        .garde_validation()
        .source("signup-garde")
        .path_map(GardePathMap::new().with_field("email", context_email_path()))
        .register_with_context(
            move |context| {
                source_seen_for_context.set(context.source().as_str() == "signup-garde");

                SignupValidationContext {
                    email_suffix: if use_matching_suffix_for_context.get() {
                        "@example.com".to_owned()
                    } else {
                        "@example.org".to_owned()
                    },
                    minimum_password_length: 0,
                }
            },
            app_error,
        );

    form.validate_form(ValidationTrigger::Manual);

    assert!(source_seen.get());
    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );
    let sources: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| error.source().as_str().to_owned())
        .collect();
    assert_eq!(
        sources,
        vec!["native".to_owned(), "signup-garde".to_owned()]
    );

    use_matching_suffix.set(true);
    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 2);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Valid),
    );
    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.source().as_str().to_owned(), error.error().clone()))
        .collect();
    assert_eq!(
        errors,
        vec![(
            "native".to_owned(),
            AppError {
                path: "native".to_owned(),
                message: "native invalid".to_owned(),
                target: ValidationTarget::Form,
            },
        ),],
    );
}

#[test]
fn adapter_crate_stays_between_form_core_and_garde_without_dioxus_dependencies() {
    let adapter_manifest = include_str!("../Cargo.toml");
    let core_manifest = include_str!("../../dioform-core/Cargo.toml");
    let facade_manifest = include_str!("../../dioform/Cargo.toml");

    assert_manifest_dependency(adapter_manifest, "dioform-core");
    assert_manifest_dependency(adapter_manifest, "garde");
    let garde_dependency = manifest_dependency_line(adapter_manifest, "garde")
        .expect("manifest should contain a garde dependency line");
    assert!(
        garde_dependency.contains("default-features = false"),
        "adapter should not enable garde default features",
    );
    assert!(
        !garde_dependency.contains("derive"),
        "adapter should not enable garde/derive",
    );
    assert!(
        !garde_dependency.contains("full"),
        "adapter should not enable garde/full",
    );
    assert_no_manifest_dependency(adapter_manifest, "dioform");
    assert_no_manifest_dependency(adapter_manifest, "dioxus-core");
    assert_no_manifest_dependency(core_manifest, "garde");
    assert_no_manifest_dependency(facade_manifest, "garde");
}

fn assert_manifest_dependency(manifest: &str, crate_name: &str) {
    assert!(
        has_manifest_dependency(manifest, crate_name),
        "manifest should depend on {crate_name}",
    );
}

fn assert_no_manifest_dependency(manifest: &str, crate_name: &str) {
    assert!(
        !has_manifest_dependency(manifest, crate_name),
        "manifest should not depend on {crate_name}",
    );
}

fn has_manifest_dependency(manifest: &str, crate_name: &str) -> bool {
    manifest_dependency_line(manifest, crate_name).is_some()
}

fn manifest_dependency_line<'a>(manifest: &'a str, crate_name: &str) -> Option<&'a str> {
    manifest.lines().find(|line| {
        line.trim_start()
            .strip_prefix(crate_name)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    })
}
