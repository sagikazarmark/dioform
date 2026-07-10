use std::{borrow::Cow, cell::Cell, collections::BTreeMap, rc::Rc};

use dioform_core::{
    FieldIdentity, FieldPath, FormCore, FormValidationError, SubmitError, SubmitErrors,
    SubmitResult, SubmitStatus, ValidationStatus, ValidationTarget, ValidationTrigger,
    ValidationTriggers,
};
use dioform_validator::{ValidatorDiagnostic, ValidatorPathMap, ValidatorValidationExt};
use validator::ValidationErrorsKind;

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

impl validator::Validate for SignupForm {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        self.validation_runs.set(self.validation_runs.get() + 1);

        let mut errors = validator::ValidationErrors::new();
        if self.email.trim().is_empty() {
            errors.add("email", validator_error("required", "email required"));
        }
        if self.password.len() < 8 {
            errors.add("password", validator_error("length", "password too short"));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn validator_error(code: &'static str, message: &'static str) -> validator::ValidationError {
    let mut error = validator::ValidationError::new(code);
    error.message = Some(Cow::Borrowed(message));
    error
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

#[derive(Clone)]
struct Address {
    street: String,
}

#[derive(Clone)]
struct Line {
    quantity: u32,
}

#[derive(Clone)]
struct OrderForm {
    address: Address,
    lines: Vec<Line>,
    validation_runs: Rc<Cell<usize>>,
}

impl OrderForm {
    fn new(validation_runs: Rc<Cell<usize>>) -> Self {
        Self {
            address: Address {
                street: String::new(),
            },
            lines: vec![Line { quantity: 0 }, Line { quantity: 0 }],
            validation_runs,
        }
    }
}

impl validator::Validate for OrderForm {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        self.validation_runs.set(self.validation_runs.get() + 1);

        // Build a nested struct + list diagnostic tree by hand so the flattening behavior is
        // exercised independently of any derive-generated shape.
        let mut errors = validator::ValidationErrors::new();

        let mut address_errors = validator::ValidationErrors::new();
        address_errors.add("street", validator_error("required", "street required"));
        errors.0.insert(
            Cow::Borrowed("address"),
            ValidationErrorsKind::Struct(Box::new(address_errors)),
        );

        let mut lines = BTreeMap::new();
        for index in 0..self.lines.len() {
            let mut line_errors = validator::ValidationErrors::new();
            line_errors.add("quantity", validator_error("range", "quantity too low"));
            lines.insert(index, Box::new(line_errors));
        }
        errors
            .0
            .insert(Cow::Borrowed("lines"), ValidationErrorsKind::List(lines));

        Err(errors)
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

struct SignupLimits {
    email_suffix: String,
    minimum_password_length: usize,
}

impl<'v_a> validator::ValidateArgs<'v_a> for ContextSignupForm {
    type Args = &'v_a SignupLimits;

    fn validate_with_args(&self, args: Self::Args) -> Result<(), validator::ValidationErrors> {
        self.validation_runs.set(self.validation_runs.get() + 1);

        let mut errors = validator::ValidationErrors::new();
        if !self.email.ends_with(&args.email_suffix) {
            let mut error = validator::ValidationError::new("suffix");
            error.message = Some(Cow::Owned(format!(
                "email must end with {}",
                args.email_suffix
            )));
            errors.add("email", error);
        }
        if self.password.len() < args.minimum_password_length {
            let mut error = validator::ValidationError::new("length");
            error.message = Some(Cow::Owned(format!(
                "password must be at least {} characters",
                args.minimum_password_length
            )));
            errors.add("password", error);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
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

#[derive(Clone)]
struct CodeOnlyForm;

impl validator::Validate for CodeOnlyForm {
    fn validate(&self) -> Result<(), validator::ValidationErrors> {
        let mut errors = validator::ValidationErrors::new();
        errors.add("token", validator::ValidationError::new("invalid"));
        Err(errors)
    }
}

fn street_path() -> FieldPath<OrderForm, String> {
    FieldPath::direct(
        FieldIdentity::new("address.street"),
        "address.street",
        |model: &OrderForm| &model.address.street,
        |model: &mut OrderForm| &mut model.address.street,
    )
}

fn first_line_quantity_path() -> FieldPath<OrderForm, u32> {
    FieldPath::direct(
        FieldIdentity::new("lines.0.quantity"),
        "lines.0.quantity",
        |model: &OrderForm| &model.lines[0].quantity,
        |model: &mut OrderForm| &mut model.lines[0].quantity,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AppError {
    path: String,
    code: String,
    message: Option<String>,
    target: ValidationTarget,
}

fn app_error(diagnostic: ValidatorDiagnostic<'_>) -> AppError {
    AppError {
        path: diagnostic.path().to_owned(),
        code: diagnostic.error().code.to_string(),
        message: diagnostic.error().message.as_ref().map(|m| m.to_string()),
        target: diagnostic.target(),
    }
}

fn app_error_value(path: &str, code: &str, message: &str, target: ValidationTarget) -> AppError {
    AppError {
        path: path.to_owned(),
        code: code.to_owned(),
        message: Some(message.to_owned()),
        target,
    }
}

#[test]
fn path_map_resolves_exact_validator_paths_to_typed_field_targets() {
    let path_map = ValidatorPathMap::new()
        .with_field("account.email", email_path())
        .with_field("password", password_path());

    assert_eq!(
        path_map.target_for_path("account.email"),
        ValidationTarget::field(email_path()),
    );
    assert_eq!(
        path_map.target_for_path("password"),
        ValidationTarget::field(password_path()),
    );
    assert_eq!(path_map.target_for_path("email"), ValidationTarget::Form);
    assert_eq!(
        path_map.target_for_path("account.email.extra"),
        ValidationTarget::Form,
    );
}

#[test]
fn registering_validator_validation_returns_validator_id_without_running() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));

    let validator_id = form.validator_validation().register(app_error);

    assert_eq!(validator_id.as_u64(), 0);
    assert_eq!(runs.get(), 0);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Unknown),
    );
    assert!(form.validation_errors().is_empty());
}

#[test]
fn unmapped_validator_diagnostics_attach_to_form_level_errors_sorted_by_path() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));
    let validator_id = form.validator_validation().register(app_error);

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
                "validator".to_owned(),
                ValidationTarget::Form,
                AppError {
                    path: "email".to_owned(),
                    code: "required".to_owned(),
                    message: Some("email required".to_owned()),
                    target: ValidationTarget::Form,
                },
            ),
            (
                Some(validator_id),
                "validator".to_owned(),
                ValidationTarget::Form,
                AppError {
                    path: "password".to_owned(),
                    code: "length".to_owned(),
                    message: Some("password too short".to_owned()),
                    target: ValidationTarget::Form,
                },
            ),
        ],
    );
}

#[test]
fn mapped_validator_paths_attach_to_typed_field_errors_sorted_by_path() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));
    let path_map = ValidatorPathMap::new()
        .with_field("email", email_path())
        .with_field("password", password_path());
    let validator_id = form
        .validator_validation()
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
                "validator".to_owned(),
                ValidationTarget::field(email_path()),
                AppError {
                    path: "email".to_owned(),
                    code: "required".to_owned(),
                    message: Some("email required".to_owned()),
                    target: ValidationTarget::field(email_path()),
                },
            ),
            (
                Some(validator_id),
                "validator".to_owned(),
                ValidationTarget::field(password_path()),
                AppError {
                    path: "password".to_owned(),
                    code: "length".to_owned(),
                    message: Some("password too short".to_owned()),
                    target: ValidationTarget::field(password_path()),
                },
            ),
        ],
    );
}

#[test]
fn validator_errors_coexist_with_native_validators_and_submit_errors_in_flattened_order() {
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
                "native",
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
                        "native",
                        "native form field error",
                        ValidationTarget::field(password_path()),
                    ),
                ),
                FormValidationError::form(app_error_value(
                    "native.form",
                    "native",
                    "native form error",
                    ValidationTarget::Form,
                )),
            ]
        },
    );
    let path_map = ValidatorPathMap::new()
        .with_field("email", email_path())
        .with_field("password", password_path());
    let validator_id = form
        .validator_validation()
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
                            "server",
                            "server email error",
                            ValidationTarget::field(email_path()),
                        ),
                    ),
                    SubmitError::form(app_error_value(
                        "submit.form",
                        "server",
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
                    "native",
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
                    "native",
                    "native form field error",
                    ValidationTarget::field(password_path()),
                ),
            ),
            (
                Some(native_form_id),
                "native-form".to_owned(),
                ValidationTarget::Form,
                app_error_value(
                    "native.form",
                    "native",
                    "native form error",
                    ValidationTarget::Form
                ),
            ),
            (
                Some(validator_id),
                "validator".to_owned(),
                ValidationTarget::field(email_path()),
                AppError {
                    path: "email".to_owned(),
                    code: "required".to_owned(),
                    message: Some("email required".to_owned()),
                    target: ValidationTarget::field(email_path()),
                },
            ),
            (
                Some(validator_id),
                "validator".to_owned(),
                ValidationTarget::field(password_path()),
                AppError {
                    path: "password".to_owned(),
                    code: "length".to_owned(),
                    message: Some("password too short".to_owned()),
                    target: ValidationTarget::field(password_path()),
                },
            ),
            (
                None,
                "server".to_owned(),
                ValidationTarget::field(email_path()),
                app_error_value(
                    "submit.email",
                    "server",
                    "server email error",
                    ValidationTarget::field(email_path()),
                ),
            ),
            (
                None,
                "server".to_owned(),
                ValidationTarget::Form,
                app_error_value(
                    "submit.form",
                    "server",
                    "server form error",
                    ValidationTarget::Form
                ),
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
        .validator_validation()
        .source("signup-validator")
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

    let sources: Vec<_> = form
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
        sources,
        vec![
            (Some(validator_id), "signup-validator".to_owned()),
            (Some(validator_id), "signup-validator".to_owned()),
        ],
    );
}

#[test]
fn rerunning_validator_replaces_only_its_source_and_success_preserves_other_sources() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<SignupForm, AppError> =
        FormCore::new_with_error_type(SignupForm::new("", "short", Rc::clone(&runs)));
    let native_form_id = form.register_sync_form_validator_for_triggers(
        "native-form",
        ValidationTrigger::Manual,
        |_context| {
            vec![FormValidationError::form(app_error_value(
                "native.form",
                "native",
                "native form error",
                ValidationTarget::Form,
            ))]
        },
    );
    let path_map = ValidatorPathMap::new()
        .with_field("email", email_path())
        .with_field("password", password_path());
    let validator_id = form
        .validator_validation()
        .triggers(ValidationTrigger::Manual)
        .path_map(path_map)
        .register(app_error);

    form.validate_all(ValidationTrigger::Manual);
    assert_eq!(runs.get(), 1);
    assert_eq!(form.validation_errors().len(), 3);

    // Fix only the email; rerunning the adapter replaces its own errors but leaves the native
    // form error untouched.
    form.set_field(email_path(), "ada@example.com".to_owned());
    assert_eq!(
        form.validate_form_validator(validator_id, ValidationTrigger::Manual),
        Some(ValidationStatus::Invalid),
    );

    assert_eq!(runs.get(), 2);
    let sources: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.source().as_str().to_owned(), error.error().clone()))
        .collect();
    assert_eq!(
        sources,
        vec![
            (
                "native-form".to_owned(),
                app_error_value(
                    "native.form",
                    "native",
                    "native form error",
                    ValidationTarget::Form
                ),
            ),
            (
                "validator".to_owned(),
                AppError {
                    path: "password".to_owned(),
                    code: "length".to_owned(),
                    message: Some("password too short".to_owned()),
                    target: ValidationTarget::field(password_path()),
                },
            ),
        ],
    );

    // Fix the password too; a successful adapter run clears its own errors and keeps the native one.
    form.set_field(password_path(), "long-enough".to_owned());
    assert_eq!(
        form.validate_form_validator(validator_id, ValidationTrigger::Manual),
        Some(ValidationStatus::Valid),
    );

    assert_eq!(runs.get(), 3);
    let remaining: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.validator_id(), error.source().as_str().to_owned()))
        .collect();
    assert_eq!(
        remaining,
        vec![(Some(native_form_id), "native-form".to_owned())]
    );
}

#[test]
fn string_error_convenience_maps_validator_messages_to_strings() {
    let runs = Rc::new(Cell::new(0));
    let mut form = FormCore::new(SignupForm::new("", "short", Rc::clone(&runs)));
    let validator_id = form
        .validator_validation()
        .path_map(ValidatorPathMap::new().with_field("email", email_path()))
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
                "validator".to_owned(),
                ValidationTarget::field(email_path()),
                "email required".to_owned(),
            ),
            (
                Some(validator_id),
                "validator".to_owned(),
                ValidationTarget::Form,
                "password too short".to_owned(),
            ),
        ],
    );
}

#[test]
fn context_provider_derives_validator_args_from_form_draft_for_success_and_failure() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<ContextSignupForm, AppError> =
        FormCore::new_with_error_type(ContextSignupForm::new(
            "ada@example.net",
            "long-enough",
            "@example.com",
            Rc::clone(&runs),
        ));
    let path_map = ValidatorPathMap::new().with_field("email", context_email_path());
    let validator_id = form
        .validator_validation()
        .path_map(path_map)
        .register_with_context(
            |context| SignupLimits {
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
            code: "suffix".to_owned(),
            message: Some("email must end with @example.com".to_owned()),
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
fn context_provider_can_derive_validator_args_from_validation_trigger() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<ContextSignupForm, AppError> =
        FormCore::new_with_error_type(ContextSignupForm::new(
            "ada@example.com",
            "long-pass!",
            "@example.com",
            Rc::clone(&runs),
        ));
    let path_map = ValidatorPathMap::new().with_field("password", context_password_path());
    let validator_id = form
        .validator_validation()
        .path_map(path_map)
        .register_with_context(
            |context| SignupLimits {
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
        form.field_validation_errors(context_password_path())[0].error(),
        &AppError {
            path: "password".to_owned(),
            code: "length".to_owned(),
            message: Some("password must be at least 12 characters".to_owned()),
            target: ValidationTarget::field(context_password_path()),
        },
    );
}

#[test]
fn context_string_error_convenience_maps_validator_messages_to_strings() {
    let runs = Rc::new(Cell::new(0));
    let mut form = FormCore::new(ContextSignupForm::new(
        "ada@example.net",
        "long-enough",
        "@example.com",
        Rc::clone(&runs),
    ));
    let validator_id = form
        .validator_validation()
        .path_map(ValidatorPathMap::new().with_field("email", context_email_path()))
        .register_string_errors_with_context(|context| SignupLimits {
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
fn string_error_convenience_falls_back_to_code_when_message_is_absent() {
    let mut form = FormCore::new(CodeOnlyForm);
    form.validator_validation().register_string_errors();

    form.validate_form(ValidationTrigger::Manual);

    let messages: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| error.error().clone())
        .collect();

    assert_eq!(messages, vec!["invalid".to_owned()]);
}

#[test]
fn adapter_crate_stays_between_form_core_and_validator_without_dioxus_or_garde_dependencies() {
    let adapter_manifest = include_str!("../Cargo.toml");
    let core_manifest = include_str!("../../dioform-core/Cargo.toml");
    let facade_manifest = include_str!("../../dioform/Cargo.toml");

    assert_manifest_dependency(adapter_manifest, "dioform-core");
    assert_manifest_dependency(adapter_manifest, "validator");
    let validator_dependency = manifest_dependency_line(adapter_manifest, "validator")
        .expect("manifest should contain a validator dependency line");
    assert!(
        validator_dependency.contains("default-features = false"),
        "adapter should not enable validator default features",
    );
    assert!(
        !validator_dependency.contains("derive"),
        "adapter should not enable validator/derive",
    );
    assert_no_manifest_dependency(adapter_manifest, "dioform");
    assert_no_manifest_dependency(adapter_manifest, "dioxus-core");
    assert_no_manifest_dependency(adapter_manifest, "garde");
    assert_no_manifest_dependency(core_manifest, "validator");
    assert_no_manifest_dependency(facade_manifest, "validator");
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

#[test]
fn nested_struct_and_list_diagnostics_flatten_to_canonical_paths_in_order() {
    let runs = Rc::new(Cell::new(0));
    let mut form: FormCore<OrderForm, AppError> =
        FormCore::new_with_error_type(OrderForm::new(Rc::clone(&runs)));
    // Map two canonical external paths to typed fields; leave `lines[1].quantity` unmapped so its
    // diagnostic is preserved on the form instead of dropped.
    let path_map = ValidatorPathMap::new()
        .with_field("address.street", street_path())
        .with_field("lines[0].quantity", first_line_quantity_path());
    let validator_id = form
        .validator_validation()
        .path_map(path_map)
        .register(app_error);

    form.validate_form(ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        form.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Invalid),
    );

    let errors: Vec<_> = form
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), error.error().clone()))
        .collect();

    assert_eq!(
        errors,
        vec![
            (
                ValidationTarget::field(street_path()),
                AppError {
                    path: "address.street".to_owned(),
                    code: "required".to_owned(),
                    message: Some("street required".to_owned()),
                    target: ValidationTarget::field(street_path()),
                },
            ),
            (
                ValidationTarget::field(first_line_quantity_path()),
                AppError {
                    path: "lines[0].quantity".to_owned(),
                    code: "range".to_owned(),
                    message: Some("quantity too low".to_owned()),
                    target: ValidationTarget::field(first_line_quantity_path()),
                },
            ),
            (
                ValidationTarget::Form,
                AppError {
                    path: "lines[1].quantity".to_owned(),
                    code: "range".to_owned(),
                    message: Some("quantity too low".to_owned()),
                    target: ValidationTarget::Form,
                },
            ),
        ],
    );
}
