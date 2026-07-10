use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use dioform::{
    Form, FormHandle, SubmitBlocker, SubmitError, SubmitResult, SubmitStatus, ValidationTarget,
};
use dioform_fullstack::{
    IntentSubmitBindingFullstackExt, ServerSubmitOutcome, SubmitBindingFullstackExt,
};
use dioxus_core::{Element, Event, VNode, VirtualDom, use_hook};
use dioxus_fullstack::ServerFnError;

fn managed_submit_event() -> Event<()> {
    Event::new(Rc::new(()), true)
}

struct AsyncGate<T> {
    inner: Rc<RefCell<AsyncGateState<T>>>,
}

impl<T> Clone for AsyncGate<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

struct AsyncGateState<T> {
    output: Option<T>,
    waker: Option<Waker>,
}

struct AsyncGateFuture<T> {
    inner: Rc<RefCell<AsyncGateState<T>>>,
}

impl<T> Default for AsyncGate<T> {
    fn default() -> Self {
        Self {
            inner: Rc::new(RefCell::new(AsyncGateState {
                output: None,
                waker: None,
            })),
        }
    }
}

impl<T> AsyncGate<T> {
    fn future(&self) -> AsyncGateFuture<T> {
        AsyncGateFuture {
            inner: Rc::clone(&self.inner),
        }
    }

    fn complete(&self, output: T) {
        let waker = {
            let mut state = self.inner.borrow_mut();

            assert!(state.output.is_none(), "async gate completed twice");
            state.output = Some(output);
            state.waker.take()
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<T> Future for AsyncGateFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.inner.borrow_mut();

        match state.output.take() {
            Some(output) => Poll::Ready(output),
            None => {
                state.waker = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct SignupForm {
    email: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SignupError {
    EmailTaken,
    FormRejected,
    TransportUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SignupSubmitIntent {
    SaveDraft,
    Publish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SignupRejection {
    EmailTaken,
    FormRejected,
}

#[derive(Default)]
struct FullstackSubmitProbe {
    handle: RefCell<Option<FormHandle<SignupForm, SignupError>>>,
    result: RefCell<Option<SubmitResult>>,
    submitted_email: RefCell<Option<String>>,
}

#[derive(Default)]
struct TransportFailureProbe {
    handle: RefCell<Option<FormHandle<SignupForm, SignupError>>>,
    result: RefCell<Option<SubmitResult>>,
    failure_message: RefCell<Option<&'static str>>,
}

#[derive(Default)]
struct ServerFnErrorProbe {
    handle: RefCell<Option<FormHandle<SignupForm, SignupError>>>,
    result: RefCell<Option<SubmitResult>>,
    failure_message: RefCell<Option<String>>,
}

#[derive(Default)]
struct FormRejectionProbe {
    handle: RefCell<Option<FormHandle<SignupForm, SignupError>>>,
    result: RefCell<Option<SubmitResult>>,
}

#[derive(Default)]
struct IntentFullstackSubmitProbe {
    handle: RefCell<Option<FormHandle<SignupForm, SignupError>>>,
    result: RefCell<Option<SubmitResult>>,
    submitted_intent: RefCell<Option<SignupSubmitIntent>>,
}

#[derive(Default)]
struct StaleFieldErrorProbe {
    gate: AsyncGate<Result<ServerSubmitOutcome<SignupRejection>, &'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, SignupError>>>,
    submitted_email: RefCell<Option<String>>,
}

fn fullstack_submit_rejection_probe(probe: Rc<FullstackSubmitProbe>) -> Element {
    let form = dioform::use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "taken@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let submitted_probe = Rc::clone(&probe);
            let result = form.managed_submit().on_submit_server_fn(
                managed_submit_event(),
                move |submitted| {
                    submitted_probe
                        .submitted_email
                        .borrow_mut()
                        .replace(submitted.value().email.clone());

                    async move {
                        Ok::<_, &'static str>(ServerSubmitOutcome::rejected(
                            SignupRejection::EmailTaken,
                        ))
                    }
                },
                |rejection| match rejection {
                    SignupRejection::EmailTaken => {
                        SubmitError::field(SignupForm::fields().email(), SignupError::EmailTaken)
                            .into()
                    }
                    SignupRejection::FormRejected => {
                        SubmitError::form(SignupError::FormRejected).into()
                    }
                },
                |_failure| SubmitError::form(SignupError::TransportUnavailable).into(),
            );

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

fn transport_failure_probe(probe: Rc<TransportFailureProbe>) -> Element {
    let form = dioform::use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let failure_probe = Rc::clone(&probe);
            let result = form.managed_submit().on_submit_server_fn(
                managed_submit_event(),
                |_submitted| async move {
                    Err::<ServerSubmitOutcome<SignupRejection>, _>("network unavailable")
                },
                |_rejection| {
                    SubmitError::field(SignupForm::fields().email(), SignupError::EmailTaken).into()
                },
                move |failure| {
                    failure_probe.failure_message.borrow_mut().replace(failure);
                    SubmitError::form(SignupError::TransportUnavailable).into()
                },
            );

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

fn server_fn_error_probe(probe: Rc<ServerFnErrorProbe>) -> Element {
    let form = dioform::use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let failure_probe = Rc::clone(&probe);
            let result = form.managed_submit().on_submit_server_fn(
                managed_submit_event(),
                |_submitted| async move {
                    Err::<ServerSubmitOutcome<SignupRejection>, _>(ServerFnError::new(
                        "server function unavailable",
                    ))
                },
                |_rejection| {
                    SubmitError::field(SignupForm::fields().email(), SignupError::EmailTaken).into()
                },
                move |failure| {
                    let ServerFnError::ServerError { message, .. } = failure else {
                        panic!("expected server function error payload");
                    };
                    failure_probe.failure_message.borrow_mut().replace(message);
                    SubmitError::form(SignupError::TransportUnavailable).into()
                },
            );

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

fn form_rejection_probe(probe: Rc<FormRejectionProbe>) -> Element {
    let form = dioform::use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let result = form.managed_submit().on_submit_server_fn(
                managed_submit_event(),
                |_submitted| async move {
                    Ok::<_, &'static str>(ServerSubmitOutcome::rejected(
                        SignupRejection::FormRejected,
                    ))
                },
                |rejection| match rejection {
                    SignupRejection::EmailTaken => {
                        SubmitError::field(SignupForm::fields().email(), SignupError::EmailTaken)
                            .into()
                    }
                    SignupRejection::FormRejected => {
                        SubmitError::form(SignupError::FormRejected).into()
                    }
                },
                |_failure| SubmitError::form(SignupError::TransportUnavailable).into(),
            );

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

fn intent_fullstack_submit_probe(probe: Rc<IntentFullstackSubmitProbe>) -> Element {
    let form = dioform::use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let intent_probe = Rc::clone(&probe);
            let result =
                form.managed_submit()
                    .intent(SignupSubmitIntent::Publish)
                    .on_submit_server_fn(
                        managed_submit_event(),
                        move |submitted| {
                            intent_probe
                                .submitted_intent
                                .borrow_mut()
                                .replace(*submitted.intent());

                            async move {
                                Ok::<_, &'static str>(
                                    ServerSubmitOutcome::<SignupRejection>::accepted(),
                                )
                            }
                        },
                        |_rejection| {
                            SubmitError::field(
                                SignupForm::fields().email(),
                                SignupError::EmailTaken,
                            )
                            .into()
                        },
                        |_failure| SubmitError::form(SignupError::TransportUnavailable).into(),
                    );

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

fn stale_field_error_probe(probe: Rc<StaleFieldErrorProbe>) -> Element {
    let form = dioform::use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "taken@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let gate = probe.gate.clone();
            let submit_probe = Rc::clone(&probe);

            form.managed_submit().on_submit_server_fn(
                managed_submit_event(),
                move |submitted| {
                    submit_probe
                        .submitted_email
                        .borrow_mut()
                        .replace(submitted.value().email.clone());
                    gate.future()
                },
                |rejection| match rejection {
                    SignupRejection::EmailTaken => {
                        SubmitError::field(SignupForm::fields().email(), SignupError::EmailTaken)
                            .into()
                    }
                    SignupRejection::FormRejected => {
                        SubmitError::form(SignupError::FormRejected).into()
                    }
                },
                |_failure| SubmitError::form(SignupError::TransportUnavailable).into(),
            );
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

#[test]
fn fullstack_submit_rejection_maps_to_field_submit_error() {
    let probe = Rc::new(FullstackSubmitProbe::default());
    let mut dom = VirtualDom::new_with_props(fullstack_submit_rejection_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());

    dom.render_immediate_to_vec();

    assert_eq!(
        probe.submitted_email.borrow().as_deref(),
        Some("taken@example.com")
    );
    assert!(!handle.is_submitting());
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Rejected));

    let errors: Vec<_> = handle
        .field_validation_errors(SignupForm::fields().email())
        .into_iter()
        .map(|error| (error.target(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![(
            ValidationTarget::Field(SignupForm::fields().email().identity()),
            SignupError::EmailTaken,
        )]
    );
}

#[test]
fn transport_failure_maps_through_application_failure_mapper() {
    let probe = Rc::new(TransportFailureProbe::default());
    let mut dom = VirtualDom::new_with_props(transport_failure_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());

    dom.render_immediate_to_vec();

    assert_eq!(
        probe.failure_message.borrow().as_deref(),
        Some("network unavailable")
    );
    assert!(!handle.is_submitting());
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Rejected));

    let errors: Vec<_> = handle
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![(ValidationTarget::Form, SignupError::TransportUnavailable)]
    );
}

#[test]
fn dioxus_fullstack_server_fn_error_maps_through_failure_mapper() {
    let probe = Rc::new(ServerFnErrorProbe::default());
    let mut dom = VirtualDom::new_with_props(server_fn_error_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));

    dom.render_immediate_to_vec();

    assert_eq!(
        probe.failure_message.borrow().as_deref(),
        Some("server function unavailable")
    );
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Rejected));
    assert_eq!(
        handle.validation_errors()[0].target(),
        ValidationTarget::Form
    );
    assert_eq!(
        handle.validation_errors()[0].error(),
        &SignupError::TransportUnavailable
    );
}

#[test]
fn fullstack_submit_rejection_maps_to_form_submit_error() {
    let probe = Rc::new(FormRejectionProbe::default());
    let mut dom = VirtualDom::new_with_props(form_rejection_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());

    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Rejected));

    let errors: Vec<_> = handle
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![(ValidationTarget::Form, SignupError::FormRejected)]
    );
}

#[test]
fn intentful_fullstack_submit_preserves_submit_intent_status() {
    let probe = Rc::new(IntentFullstackSubmitProbe::default());
    let mut dom = VirtualDom::new_with_props(intent_fullstack_submit_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());

    dom.render_immediate_to_vec();

    assert_eq!(
        *probe.submitted_intent.borrow(),
        Some(SignupSubmitIntent::Publish)
    );
    assert!(!handle.is_submitting());
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Succeeded));
    assert_eq!(
        handle
            .last_submit_status_as::<SignupSubmitIntent>()
            .expect("latest status should carry submit intent")
            .intent(),
        &SignupSubmitIntent::Publish
    );
    assert_eq!(
        handle.intent(SignupSubmitIntent::Publish).last_status(),
        Some(SubmitStatus::Succeeded)
    );
    assert_eq!(
        handle.intent(SignupSubmitIntent::SaveDraft).last_status(),
        None
    );
}

#[test]
fn stale_fullstack_field_rejection_is_discarded_after_draft_edit() {
    let probe = Rc::new(StaleFieldErrorProbe::default());
    let mut dom = VirtualDom::new_with_props(stale_field_error_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert!(handle.is_submitting());

    handle.text(email.clone()).on_input("new@example.com");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.submitted_email.borrow().as_deref(),
        Some("taken@example.com")
    );
    assert_eq!(handle.field_value(email.clone()), "new@example.com");

    probe.gate.complete(Ok(ServerSubmitOutcome::rejected(
        SignupRejection::EmailTaken,
    )));
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert!(handle.field_validation_errors(email.clone()).is_empty());
    assert!(handle.validation_errors().is_empty());
    assert_eq!(handle.field_value(email), "new@example.com");
}

#[test]
fn fullstack_submit_blocks_duplicate_submission_while_in_flight() {
    let probe = Rc::new(StaleFieldErrorProbe::default());
    let mut dom = VirtualDom::new_with_props(stale_field_error_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert!(handle.is_submitting());
    assert_eq!(handle.submit_attempt_count(), 1);

    let duplicate = handle.managed_submit().on_submit_server_fn(
        managed_submit_event(),
        |_submitted| async move {
            Ok::<_, &'static str>(ServerSubmitOutcome::<SignupRejection>::accepted())
        },
        |_rejection| {
            SubmitError::field(SignupForm::fields().email(), SignupError::EmailTaken).into()
        },
        |_failure| SubmitError::form(SignupError::TransportUnavailable).into(),
    );

    assert_eq!(
        duplicate,
        SubmitResult::Blocked(SubmitBlocker::InFlightSubmission)
    );
    assert_eq!(handle.submit_attempt_count(), 1);
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission))
    );
}
