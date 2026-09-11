use dioform_core::{
    ErrorVisibilityPolicy, FieldIdentity, FieldPath, Form, FormCore, SubmitAttempt, SubmitBlocker,
    SubmitError, SubmitErrors, SubmitStatus, ValidationTrigger, ValidationTriggers,
};

#[derive(Clone, Debug, PartialEq)]
struct Model {
    name: String,
}

#[test]
fn replacement_retires_pending_validation_and_authorization_and_resets_history() {
    let mut form = FormCore::new(Model {
        name: "before".into(),
    });
    let validator = form.register_async_field_validator_for_triggers(
        name(),
        "remote",
        ValidationTriggers::new([ValidationTrigger::Manual]),
    );
    form.set_user_field(name(), "edited".into());
    form.mark_submit_attempt();
    let run = form
        .begin_async_field_validation(name(), validator, ValidationTrigger::Manual)
        .unwrap();
    let token = form.submit_validation_snapshot();
    for intent in [Intent::Publish, Intent::Save] {
        form.restore_browser_rejection(
            Model {
                name: "response".into(),
            },
            intent,
            |_| SubmitErrors::new([SubmitError::form("rejected".into())]),
        );
        assert_eq!(form.submit_attempt_count(), 1);
        assert!(!form.field_metadata(name()).is_touched());
        assert!(!form.is_dirty());
        assert_eq!(form.validation_errors().len(), 1);
        assert_eq!(
            form.last_submit_status_as::<Intent>().unwrap().intent(),
            &intent
        );
        assert_eq!(
            form.complete_async_field_validation(name(), validator, &run, ["old".into()]),
            None
        );
        // The unit-intent proof was current immediately before restoration. Errors from this
        // intentful response do not mask its retirement with a ValidationErrors blocker.
        assert!(matches!(
            form.begin_submission_after_validation(&token),
            SubmitAttempt::Blocked(SubmitBlocker::StaleSubmitValidation)
        ));
    }
    assert_eq!(
        form.complete_async_field_validation(name(), validator, &run, ["old".into()]),
        None
    );
    form.reset();
    assert!(form.validation_errors().is_empty());
    assert_eq!(form.submit_attempt_count(), 0);
    assert_eq!(form.last_submit_status(), None);
    assert_eq!(form.field_value(name()), "response");
}

#[test]
fn explicit_client_validation_coexists_and_retry_replaces_server_batch() {
    let mut form = FormCore::new(Model { name: "".into() });
    form.register_sync_field_validator_for_triggers(
        name(),
        "client",
        ValidationTriggers::all(),
        |_, _| vec!["client".into()],
    );
    form.restore_browser_rejection(Model { name: "".into() }, (), |_| {
        SubmitError::form("server".into()).into()
    });
    form.validate_all(ValidationTrigger::Manual);
    assert_eq!(form.visible_validation_errors().len(), 2);
    assert_eq!(
        form.validate_for_submit_preflight(),
        Some(SubmitBlocker::ValidationErrors)
    );
    assert_eq!(form.validation_errors().len(), 1);
    assert_eq!(form.validation_errors()[0].error(), "client");
    form.reinitialize(Model { name: "new".into() });
    assert!(form.validation_errors().is_empty());
    assert_eq!(form.submit_attempt_count(), 0);
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Intent {
    Save,
    Publish,
}

fn name() -> FieldPath<Model, String> {
    FieldPath::direct(
        FieldIdentity::new("name"),
        "name",
        |m: &Model| &m.name,
        |m: &mut Model| &mut m.name,
    )
}

#[derive(Clone, Debug, PartialEq, dioform_derive::Form)]
#[form(crate = "::dioform_core")]
struct Nested {
    child: Child,
    unrelated: String,
}

#[derive(Clone, Debug, PartialEq, dioform_derive::Form)]
#[form(crate = "::dioform_core")]
struct Child {
    value: u32,
}

#[test]
fn rejection_field_clearing_reaches_both_ancestry_directions_only() {
    let values = Nested {
        child: Child { value: 7 },
        unrelated: "same".into(),
    };
    let mut form = FormCore::new(values.clone());
    let child = Nested::fields().child();
    let leaf = child.clone().join(Child::fields().value());
    for replace_parent in [true, false] {
        form.restore_browser_rejection(values.clone(), (), |_| {
            SubmitErrors::new([
                SubmitError::field(child.clone(), "parent".into()),
                SubmitError::field(leaf.clone(), "leaf".into()),
                SubmitError::field(Nested::fields().unrelated(), "unrelated".into()),
                SubmitError::form("form".into()),
            ])
        });
        if replace_parent {
            form.set_field(child.clone(), Child { value: 8 });
        } else {
            form.set_field(leaf.clone(), 8);
        }
        let errors: Vec<_> = form
            .validation_errors()
            .into_iter()
            .map(|error| error.error().as_str())
            .collect();
        assert_eq!(errors, ["unrelated", "form"]);
    }
}

#[test]
fn browser_rejection_restores_response_and_intent_without_starting_submission() {
    for policy in [
        ErrorVisibilityPolicy::Always,
        ErrorVisibilityPolicy::SubmitOnly,
        ErrorVisibilityPolicy::CommitOrSubmit,
        ErrorVisibilityPolicy::BlurOrSubmit,
        ErrorVisibilityPolicy::TouchedOrSubmit,
    ] {
        let mut form =
            FormCore::new(Model { name: "old".into() }).with_error_visibility_policy(policy);
        form.restore_browser_rejection(
            Model {
                name: "response".into(),
            },
            Intent::Publish,
            |_| {
                SubmitErrors::new([
                    SubmitError::field(name(), "duplicate".into()),
                    SubmitError::form("server unavailable".into()),
                ])
            },
        );

        assert_eq!(form.field_value(name()), "response");
        assert!(!form.is_dirty());
        assert!(!form.is_submitting());
        assert_eq!(form.submit_attempt_count(), 1);
        assert_eq!(form.last_submit_status(), Some(SubmitStatus::Rejected));
        assert_eq!(
            form.visible_validation_errors_for_intent(&Intent::Publish)
                .len(),
            2
        );
        assert!(
            form.visible_validation_errors_for_intent(&Intent::Save)
                .is_empty()
        );
        assert!(!form.intent(Intent::Publish).can_submit());
        assert!(form.intent(Intent::Save).can_submit());

        form.set_field(name(), "edited".into());
        assert_eq!(form.validation_errors().len(), 1);
        assert_eq!(
            form.intent(Intent::Publish).validate_for_submit_preflight(),
            None
        );
        assert!(form.validation_errors().is_empty());
    }
}
