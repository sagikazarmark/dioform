//! Internal Dioxus-managed submission orchestration.
//!
//! The module owns adapter-side async submit sequencing around parse blockers, debounced and
//! pending validation, duplicate managed submissions, cleanup checks, and final submit spawning.
//! Renderer-agnostic submission state and submit intent rules remain in Form Core.

use std::future::Future;

use super::{
    FileSubmissionSnapshot, FormHandle, SubmissionSnapshot, SubmitAttempt, SubmitBlocker,
    SubmitErrors, SubmitListenerEvent, SubmitResult, SubmitValidationSnapshot, ValidationTrigger,
};

pub(super) struct ManagedSubmission<Model, Error = String> {
    handle: FormHandle<Model, Error>,
}

impl<Model, Error> ManagedSubmission<Model, Error> {
    pub(super) fn new(handle: FormHandle<Model, Error>) -> Self {
        Self { handle }
    }
}

impl<Model: Clone, Error> ManagedSubmission<Model, Error> {
    pub(super) fn submit_async<Intent, Submit, Fut, Outcome>(
        &self,
        intent: Intent,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        self.submit_async_with_payload(intent, |_handle| (), move |submitted, ()| submit(submitted))
    }

    pub(super) fn submit_async_with_files<Intent, Submit, Fut, Outcome>(
        &self,
        intent: Intent,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>, FileSubmissionSnapshot<Model>) -> Fut
            + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        self.submit_async_with_payload(intent, |handle| handle.file_submission_snapshot(), submit)
    }

    fn submit_async_with_payload<Intent, Payload, PayloadFactory, Submit, Fut, Outcome>(
        &self,
        intent: Intent,
        payload_factory: PayloadFactory,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Payload: 'static,
        PayloadFactory: FnOnce(&FormHandle<Model, Error>) -> Payload + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>, Payload) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        if self.handle.adapter.has_managed_async_submission() {
            return self.block_duplicate_submission(intent);
        }

        if self.handle.has_parse_blockers() {
            return self.block_with_parse_errors(intent);
        }

        if self.handle.core.borrow().is_submitting() {
            return self.block_duplicate_submission(intent.clone());
        }

        let validation = self
            .handle
            .core
            .borrow()
            .intent_validation_snapshot(intent.clone());
        let availability_intent = intent.clone();
        let listener_intent = intent.clone();
        self.handle.write_core(|core| {
            core.intent(intent).validate_for_submit();
        });
        self.handle.notify_validation_changed();
        self.handle
            .start_runtime_async_validators(ValidationTrigger::Submit);
        self.handle.dispatch_submit_listeners(
            SubmitListenerEvent::SubmitAttempted,
            listener_intent.clone(),
        );

        let availability = self.handle.intent_availability(&availability_intent);

        if availability.contains(SubmitBlocker::PendingValidation) {
            if !self.handle.adapter.has_validation_tasks() {
                self.handle.dispatch_submit_listeners(
                    SubmitListenerEvent::SubmitBlocked(SubmitBlocker::PendingValidation),
                    listener_intent,
                );
                return SubmitResult::Blocked(SubmitBlocker::PendingValidation);
            }

            return self.wait_for_pending_submit_validation(validation, payload_factory, submit);
        }

        if !availability.is_available() {
            let blocker = availability
                .blockers()
                .first()
                .copied()
                .unwrap_or(SubmitBlocker::ValidationErrors);
            self.handle.dispatch_submit_listeners(
                SubmitListenerEvent::SubmitBlocked(blocker),
                listener_intent,
            );
            return SubmitResult::Blocked(blocker);
        }

        match self.handle.write_core(|core| {
            core.intent(validation.intent().clone())
                .begin_submission_after_validation(&validation)
        }) {
            SubmitAttempt::Started(submitted) => {
                let payload = payload_factory(&self.handle);
                self.handle
                    .remember_active_submit_intent(validation.intent().clone());
                self.handle.dispatch_submit_listeners(
                    SubmitListenerEvent::SubmissionStarted,
                    validation.intent().clone(),
                );
                self.spawn_async_submit(submitted, payload, submit);
                SubmitResult::Started
            }
            SubmitAttempt::Blocked(
                blocker @ (SubmitBlocker::ValidationErrors | SubmitBlocker::PendingValidation),
            ) => {
                self.handle.notify_validation_changed();
                self.handle.dispatch_submit_listeners(
                    SubmitListenerEvent::SubmitBlocked(blocker),
                    validation.intent().clone(),
                );
                SubmitResult::Blocked(blocker)
            }
            SubmitAttempt::Blocked(blocker) => {
                self.handle.dispatch_submit_listeners(
                    SubmitListenerEvent::SubmitBlocked(blocker),
                    validation.intent().clone(),
                );
                SubmitResult::Blocked(blocker)
            }
        }
    }

    fn block_with_parse_errors<Intent>(&self, intent: Intent) -> SubmitResult
    where
        Intent: Clone + 'static,
    {
        let blocker = self
            .handle
            .write_core(|core| {
                core.intent(intent.clone())
                    .block_submission_with_parse_errors()
            })
            .expect_blocker();
        self.handle
            .notify_and_dispatch_submit_blocked(blocker, intent);
        SubmitResult::Blocked(blocker)
    }

    fn block_duplicate_submission<Intent>(&self, intent: Intent) -> SubmitResult
    where
        Intent: Clone + 'static,
    {
        let blocker = self
            .handle
            .write_core(|core| core.intent(intent.clone()).block_duplicate_submission())
            .expect_blocker();
        self.handle
            .notify_and_dispatch_submit_blocked(blocker, intent);
        SubmitResult::Blocked(blocker)
    }

    fn finish_with_parse_blocker<Intent>(handle: &FormHandle<Model, Error>, intent: Intent)
    where
        Intent: Clone + 'static,
    {
        let listener_intent = intent.clone();
        handle.write_core(|core| {
            core.intent(intent)
                .block_submission_with_parse_errors_after_validation()
        });
        handle.adapter.finish_managed_async_submission();
        handle.notify_submit_changed();
        handle.dispatch_submit_listeners(
            SubmitListenerEvent::SubmitBlocked(SubmitBlocker::ParseErrors),
            listener_intent,
        );
    }

    fn wait_for_pending_submit_validation<Intent, Payload, PayloadFactory, Submit, Fut, Outcome>(
        &self,
        validation: SubmitValidationSnapshot<Intent>,
        payload_factory: PayloadFactory,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Payload: 'static,
        PayloadFactory: FnOnce(&FormHandle<Model, Error>) -> Payload + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>, Payload) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        {
            let core = self.handle.core.borrow();
            self.handle
                .adapter
                .flush_submit_relevant_debounced_validations(&core);
        }

        if !self.handle.adapter.begin_managed_async_submission() {
            return self.block_duplicate_submission(validation.intent().clone());
        }

        let handle = self.handle.clone();
        self.handle.notify_submit_changed();

        self.handle.spawn_detached(async move {
            loop {
                if !handle.is_active() {
                    handle.adapter.finish_managed_async_submission();
                    return;
                }

                let availability = handle.intent_availability(validation.intent());
                if availability.contains(SubmitBlocker::ParseErrors) {
                    Self::finish_with_parse_blocker(&handle, validation.intent().clone());
                    return;
                }

                if !availability.contains(SubmitBlocker::PendingValidation) {
                    break;
                }

                if !handle.adapter.has_validation_tasks() {
                    handle.adapter.finish_managed_async_submission();
                    handle.notify_validation_changed();
                    handle.dispatch_submit_listeners(
                        SubmitListenerEvent::SubmitBlocked(SubmitBlocker::PendingValidation),
                        validation.intent().clone(),
                    );
                    return;
                }

                handle.adapter.validation_change().await;
            }

            if !handle.is_active() {
                handle.adapter.finish_managed_async_submission();
                return;
            }

            if handle.has_parse_blockers() {
                Self::finish_with_parse_blocker(&handle, validation.intent().clone());
                return;
            }

            match handle.write_core(|core| {
                core.intent(validation.intent().clone())
                    .begin_submission_after_validation(&validation)
            }) {
                SubmitAttempt::Started(submitted) => {
                    let payload = payload_factory(&handle);
                    handle.remember_active_submit_intent(validation.intent().clone());
                    handle.adapter.finish_managed_async_submission();
                    handle.notify_submit_changed();
                    handle.dispatch_submit_listeners(
                        SubmitListenerEvent::SubmissionStarted,
                        validation.intent().clone(),
                    );
                    Self::spawn_async_submit_for_handle(handle, submitted, payload, submit);
                }
                SubmitAttempt::Blocked(
                    blocker @ (SubmitBlocker::ValidationErrors | SubmitBlocker::PendingValidation),
                ) => {
                    handle.adapter.finish_managed_async_submission();
                    handle.notify_validation_changed();
                    handle.dispatch_submit_listeners(
                        SubmitListenerEvent::SubmitBlocked(blocker),
                        validation.intent().clone(),
                    );
                }
                SubmitAttempt::Blocked(
                    blocker @ (SubmitBlocker::InFlightSubmission | SubmitBlocker::ParseErrors),
                ) => {
                    handle.adapter.finish_managed_async_submission();
                    handle.notify_submit_changed();
                    handle.dispatch_submit_listeners(
                        SubmitListenerEvent::SubmitBlocked(blocker),
                        validation.intent().clone(),
                    );
                }
                SubmitAttempt::Blocked(blocker) => {
                    handle.adapter.finish_managed_async_submission();
                    handle.notify_changed();
                    handle.dispatch_submit_listeners(
                        SubmitListenerEvent::SubmitBlocked(blocker),
                        validation.intent().clone(),
                    );
                }
            }
        });

        SubmitResult::Started
    }

    fn spawn_async_submit<Intent, Payload, Submit, Fut, Outcome>(
        &self,
        submitted: SubmissionSnapshot<Model, Intent>,
        payload: Payload,
        submit: Submit,
    ) where
        Intent: Clone + PartialEq + 'static,
        Payload: 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>, Payload) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        Self::spawn_async_submit_for_handle(self.handle.clone(), submitted, payload, submit);
    }

    fn spawn_async_submit_for_handle<Intent, Payload, Submit, Fut, Outcome>(
        handle: FormHandle<Model, Error>,
        submitted: SubmissionSnapshot<Model, Intent>,
        payload: Payload,
        submit: Submit,
    ) where
        Intent: Clone + PartialEq + 'static,
        Payload: 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>, Payload) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        let submitted_for_result = submitted.clone();
        let submit_generation = handle.submit_generation();
        let runner = handle.clone();

        runner.spawn_detached(async move {
            let submit_errors: SubmitErrors<Model, Error> = submit(submitted, payload).await.into();

            if !handle.submit_generation_matches(submit_generation) {
                return;
            }

            if submit_errors.is_empty() {
                handle.finish_submission_success_for_intent(submitted_for_result.intent().clone());
            } else {
                handle.finish_submission_with_errors(submitted_for_result, submit_errors);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
    };

    use super::*;
    use crate::{
        FieldPath, Form, FormValidationError, SubmitBlocker, SubmitStatus, ValidationTrigger,
        ValidationTriggers, advanced::ValidatorId,
    };
    use dioxus_core::{Element, VNode, VirtualDom, use_hook};

    #[derive(Clone, Debug, Eq, Form, PartialEq)]
    #[form(crate = "crate")]
    struct AccountForm {
        age: u8,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ManagedSubmitIntent {
        SaveDraft,
        Publish,
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

    #[derive(Default)]
    struct PendingValidationProbe {
        validation: AsyncGate<Vec<&'static str>>,
        submit: AsyncGate<()>,
        handle: RefCell<Option<FormHandle<AccountForm, &'static str>>>,
        result: RefCell<Option<SubmitResult>>,
        validation_snapshot: RefCell<Option<(u8, u8)>>,
        submitted_snapshot: RefCell<Option<AccountForm>>,
        submit_calls: Cell<u32>,
    }

    #[derive(Default)]
    struct DebouncedFlushProbe {
        delay: AsyncGate<()>,
        validation: AsyncGate<Vec<&'static str>>,
        submit: AsyncGate<()>,
        handle: RefCell<Option<FormHandle<AccountForm, &'static str>>>,
        validator_id: RefCell<Option<ValidatorId>>,
        result: RefCell<Option<SubmitResult>>,
        validation_snapshot: RefCell<Option<(u8, u8)>>,
        submitted_snapshot: RefCell<Option<AccountForm>>,
        submit_calls: Cell<u32>,
        events: RefCell<Vec<SubmitListenerEvent>>,
    }

    #[derive(Default)]
    struct CleanupProbe {
        submit: AsyncGate<()>,
        handle: RefCell<Option<FormHandle<AccountForm, &'static str>>>,
        submitted: RefCell<Option<SubmissionSnapshot<AccountForm>>>,
    }

    #[derive(Default)]
    struct IntentProbe {
        handle: RefCell<Option<FormHandle<AccountForm, &'static str>>>,
        save_result: RefCell<Option<SubmitResult>>,
        submitted_intent: RefCell<Option<ManagedSubmitIntent>>,
    }

    fn pending_validation_probe(probe: Rc<PendingValidationProbe>) -> Element {
        let form = crate::use_form_handle({
            let probe = Rc::clone(&probe);

            move || {
                let form: FormHandle<AccountForm, &'static str> =
                    FormHandle::new_with_error_type(AccountForm { age: 42 });
                let validation = probe.validation.clone();
                let captured_probe = Rc::clone(&probe);

                form.field(AccountForm::fields().age())
                    .async_validator("age_check")
                    .on(ValidationTrigger::Submit)
                    .check(move |value, snapshot| {
                        captured_probe
                            .validation_snapshot
                            .borrow_mut()
                            .replace((value, snapshot.value().age));
                        validation.future()
                    });

                form
            }
        });

        use_hook({
            let form = form.clone();
            let probe = Rc::clone(&probe);

            move || {
                let submit = probe.submit.clone();
                let submit_probe = Rc::clone(&probe);
                let result =
                    ManagedSubmission::new(form.clone()).submit_async((), move |submitted| {
                        submit_probe
                            .submit_calls
                            .set(submit_probe.submit_calls.get() + 1);
                        submit_probe
                            .submitted_snapshot
                            .borrow_mut()
                            .replace(submitted.value().clone());
                        submit.future()
                    });

                probe.result.borrow_mut().replace(result);
            }
        });

        probe.handle.borrow_mut().replace(form);
        VNode::empty()
    }

    fn debounced_flush_probe(probe: Rc<DebouncedFlushProbe>) -> Element {
        let form = crate::use_form_handle({
            let probe = Rc::clone(&probe);

            move || {
                let form: FormHandle<AccountForm, &'static str> =
                    FormHandle::new_with_error_type(AccountForm { age: 42 });
                let validator_id = form.write_advanced(|core| {
                    core.register_async_field_validator_for_triggers(
                        AccountForm::fields().age(),
                        "age_check",
                        ValidationTriggers::new([
                            ValidationTrigger::Change,
                            ValidationTrigger::Submit,
                        ]),
                    )
                });

                probe.validator_id.borrow_mut().replace(validator_id);

                form
            }
        });
        let age = AccountForm::fields().age();
        let validator_id = probe
            .validator_id
            .borrow()
            .expect("probe should store validator id");

        let listener_probe = Rc::clone(&probe);
        crate::use_submit_listener(form.clone(), move |context| {
            listener_probe.events.borrow_mut().push(context.event());
        });

        use_hook({
            let form = form.clone();
            let probe = Rc::clone(&probe);

            move || {
                let delay = probe.delay.future();
                let validation = probe.validation.clone();
                let captured_probe = Rc::clone(&probe);

                form.validate_async_field_validator_with_debounce(
                    age.clone(),
                    validator_id,
                    ValidationTrigger::Change,
                    delay,
                    move |value, snapshot| {
                        captured_probe
                            .validation_snapshot
                            .borrow_mut()
                            .replace((value, snapshot.value().age));
                        validation.future()
                    },
                );
            }
        });

        use_hook({
            let form = form.clone();
            let probe = Rc::clone(&probe);

            move || {
                let submit = probe.submit.clone();
                let submit_probe = Rc::clone(&probe);
                let result =
                    ManagedSubmission::new(form.clone()).submit_async((), move |submitted| {
                        submit_probe
                            .submit_calls
                            .set(submit_probe.submit_calls.get() + 1);
                        submit_probe
                            .submitted_snapshot
                            .borrow_mut()
                            .replace(submitted.value().clone());
                        submit.future()
                    });

                probe.result.borrow_mut().replace(result);
            }
        });

        probe.handle.borrow_mut().replace(form);
        VNode::empty()
    }

    fn cleanup_probe(probe: Rc<CleanupProbe>) -> Element {
        let form =
            crate::use_form_handle(|| FormHandle::new_with_error_type(AccountForm { age: 42 }));

        use_hook({
            let form = form.clone();
            let probe = Rc::clone(&probe);

            move || {
                let submit = probe.submit.clone();
                let submit_probe = Rc::clone(&probe);
                let result =
                    ManagedSubmission::new(form.clone()).submit_async((), move |submitted| {
                        submit_probe
                            .submitted
                            .borrow_mut()
                            .replace(submitted.clone());
                        submit.future()
                    });

                assert_eq!(result, SubmitResult::Started);
            }
        });

        probe.handle.borrow_mut().replace(form);
        VNode::empty()
    }

    fn intent_probe(probe: Rc<IntentProbe>) -> Element {
        let form = crate::use_form_handle({
            move || {
                let form: FormHandle<AccountForm, &'static str> =
                    FormHandle::new_with_error_type(AccountForm { age: 0 });
                let age = AccountForm::fields().age();

                form.validator("publish_age_required")
                    .on(ValidationTrigger::Submit)
                    .check(move |context| {
                        if context.submit_intent::<ManagedSubmitIntent>()
                            == Some(&ManagedSubmitIntent::Publish)
                            && context.form().age == 0
                        {
                            vec![FormValidationError::field(
                                age.clone(),
                                "age_required_for_publish",
                            )]
                        } else {
                            Vec::new()
                        }
                    });

                form
            }
        });

        use_hook({
            let form = form.clone();
            let probe = Rc::clone(&probe);

            move || {
                let submit_probe = Rc::clone(&probe);
                let result = ManagedSubmission::new(form.clone()).submit_async(
                    ManagedSubmitIntent::SaveDraft,
                    move |submitted| {
                        submit_probe
                            .submitted_intent
                            .borrow_mut()
                            .replace(*submitted.intent());
                        async {}
                    },
                );

                probe.save_result.borrow_mut().replace(result);
            }
        });

        probe.handle.borrow_mut().replace(form);
        VNode::empty()
    }

    #[test]
    fn managed_submission_blocks_parse_errors_before_validation_or_submit_handler() {
        let handle: FormHandle<AccountForm, &'static str> =
            FormHandle::new_with_error_type(AccountForm { age: 42 });
        let age_path: FieldPath<AccountForm, u8> = AccountForm::fields().age();
        let age = handle.parsed_text(age_path.clone());
        let validation_calls = Rc::new(Cell::new(0));
        let validation_calls_for_validator = Rc::clone(&validation_calls);

        handle
            .field(age_path)
            .async_validator("age_check")
            .on(ValidationTrigger::Submit)
            .check(move |_value, _snapshot| {
                validation_calls_for_validator.set(validation_calls_for_validator.get() + 1);
                async { Vec::<&'static str>::new() }
            });

        age.on_input("not-a-number");

        let submit_calls = Rc::new(Cell::new(0));
        let submit_calls_for_handler = Rc::clone(&submit_calls);
        let result = ManagedSubmission::new(handle.clone()).submit_async((), move |_submitted| {
            submit_calls_for_handler.set(submit_calls_for_handler.get() + 1);
            async {}
        });

        assert_eq!(result, SubmitResult::Blocked(SubmitBlocker::ParseErrors));
        assert_eq!(validation_calls.get(), 0);
        assert_eq!(submit_calls.get(), 0);
        assert_eq!(handle.submit_attempt_count(), 1);
        assert_eq!(
            handle.last_submit_status(),
            Some(SubmitStatus::Blocked(SubmitBlocker::ParseErrors))
        );
    }

    #[test]
    fn managed_submission_waits_for_pending_submit_validation_before_spawning_handler() {
        let probe = Rc::new(PendingValidationProbe::default());
        let mut dom = VirtualDom::new_with_props(pending_validation_probe, Rc::clone(&probe));

        dom.rebuild_in_place();

        let handle = probe
            .handle
            .borrow()
            .as_ref()
            .expect("probe should expose its form handle")
            .clone();

        assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
        assert!(handle.is_submitting());
        assert_eq!(probe.submit_calls.get(), 0);

        dom.render_immediate_to_vec();

        assert_eq!(probe.validation_snapshot.borrow().as_ref(), Some(&(42, 42)));
        assert_eq!(probe.submit_calls.get(), 0);

        probe.validation.complete(Vec::new());
        dom.render_immediate_to_vec();

        assert_eq!(probe.submit_calls.get(), 1);
        assert_eq!(
            probe.submitted_snapshot.borrow().as_ref(),
            Some(&AccountForm { age: 42 })
        );
        assert!(handle.is_submitting());

        probe.submit.complete(());
        dom.render_immediate_to_vec();

        assert!(!handle.is_submitting());
        assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Succeeded));
    }

    #[test]
    fn managed_submission_flushes_submit_relevant_debounced_validation() {
        let probe = Rc::new(DebouncedFlushProbe::default());
        let mut dom = VirtualDom::new_with_props(debounced_flush_probe, Rc::clone(&probe));

        dom.rebuild_in_place();

        let handle = probe
            .handle
            .borrow()
            .as_ref()
            .expect("probe should expose its form handle")
            .clone();

        assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
        assert!(handle.is_submitting());
        assert_eq!(probe.submit_calls.get(), 0);

        dom.render_immediate_to_vec();

        assert_eq!(probe.validation_snapshot.borrow().as_ref(), Some(&(42, 42)));
        assert_eq!(probe.submit_calls.get(), 0);

        probe.validation.complete(Vec::new());
        dom.render_immediate_to_vec();

        assert_eq!(probe.submit_calls.get(), 1);
        assert_eq!(
            probe.submitted_snapshot.borrow().as_ref(),
            Some(&AccountForm { age: 42 })
        );

        probe.submit.complete(());
        dom.render_immediate_to_vec();

        assert!(!handle.is_submitting());
        assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Succeeded));
    }

    #[test]
    fn managed_submission_does_not_submit_stale_validation_after_draft_edit() {
        let probe = Rc::new(DebouncedFlushProbe::default());
        let mut dom = VirtualDom::new_with_props(debounced_flush_probe, Rc::clone(&probe));

        dom.rebuild_in_place();
        dom.render_immediate_to_vec();

        let handle = probe
            .handle
            .borrow()
            .as_ref()
            .expect("probe should expose its form handle")
            .clone();
        let age = AccountForm::fields().age();

        assert_eq!(probe.validation_snapshot.borrow().as_ref(), Some(&(42, 42)));
        assert!(handle.is_submitting());
        assert_eq!(
            probe.events.borrow().as_slice(),
            [SubmitListenerEvent::SubmitAttempted]
        );

        handle.set_user_field(age.clone(), 43);
        dom.render_immediate_to_vec();

        assert_eq!(probe.submit_calls.get(), 0);
        assert!(!handle.is_submitting());
        assert_eq!(
            handle.last_submit_status(),
            Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
        );
        assert_eq!(
            probe.events.borrow().as_slice(),
            [
                SubmitListenerEvent::SubmitAttempted,
                SubmitListenerEvent::SubmitBlocked(SubmitBlocker::PendingValidation),
            ]
        );
        assert_eq!(handle.field_value(age.clone()), 43);

        probe.validation.complete(Vec::new());
        dom.render_immediate_to_vec();

        assert_eq!(probe.submit_calls.get(), 0);
        assert!(!handle.is_submitting());
        assert!(handle.validation_errors().is_empty());
        assert_eq!(handle.field_value(age), 43);
    }

    #[test]
    fn managed_submission_blocks_duplicate_managed_submit_while_validation_waits() {
        let probe = Rc::new(PendingValidationProbe::default());
        let mut dom = VirtualDom::new_with_props(pending_validation_probe, Rc::clone(&probe));

        dom.rebuild_in_place();

        let handle = probe
            .handle
            .borrow()
            .as_ref()
            .expect("probe should expose its form handle")
            .clone();
        let duplicate_calls = Rc::new(Cell::new(0));
        let duplicate_calls_for_handler = Rc::clone(&duplicate_calls);

        assert!(handle.is_submitting());
        assert_eq!(handle.submit_attempt_count(), 1);

        let duplicate =
            ManagedSubmission::new(handle.clone()).submit_async((), move |_submitted| {
                duplicate_calls_for_handler.set(duplicate_calls_for_handler.get() + 1);
                async {}
            });

        assert_eq!(
            duplicate,
            SubmitResult::Blocked(SubmitBlocker::InFlightSubmission)
        );
        assert_eq!(duplicate_calls.get(), 0);
        assert_eq!(handle.submit_attempt_count(), 1);
        assert_eq!(
            handle.last_submit_status(),
            Some(SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission))
        );
    }

    #[test]
    fn managed_submission_ignores_late_success_after_cleanup() {
        let probe = Rc::new(CleanupProbe::default());
        let mut dom = VirtualDom::new_with_props(cleanup_probe, Rc::clone(&probe));

        dom.rebuild_in_place();
        dom.render_immediate_to_vec();

        let handle = probe
            .handle
            .borrow()
            .as_ref()
            .expect("probe should expose its form handle")
            .clone();

        assert!(handle.is_submitting());
        assert_eq!(
            probe
                .submitted
                .borrow()
                .as_ref()
                .map(|submitted| submitted.value()),
            Some(&AccountForm { age: 42 })
        );

        drop(dom);
        probe.submit.complete(());

        assert!(!handle.finish_submission_success());
        assert!(handle.is_submitting());
        assert!(handle.validation_errors().is_empty());
    }

    #[test]
    fn managed_submission_preserves_typed_submit_intent_status_and_availability() {
        let probe = Rc::new(IntentProbe::default());
        let mut dom = VirtualDom::new_with_props(intent_probe, Rc::clone(&probe));

        dom.rebuild_in_place();
        dom.render_immediate_to_vec();

        let handle = probe
            .handle
            .borrow()
            .as_ref()
            .expect("probe should expose its form handle")
            .clone();

        assert_eq!(*probe.save_result.borrow(), Some(SubmitResult::Started));
        assert_eq!(
            probe.submitted_intent.borrow().as_ref(),
            Some(&ManagedSubmitIntent::SaveDraft)
        );
        assert_eq!(
            handle.intent(ManagedSubmitIntent::SaveDraft).last_status(),
            Some(SubmitStatus::Succeeded)
        );

        let publish_called = Rc::new(Cell::new(false));
        let publish_called_for_handler = Rc::clone(&publish_called);
        let publish_result = ManagedSubmission::new(handle.clone()).submit_async(
            ManagedSubmitIntent::Publish,
            move |_submitted| {
                publish_called_for_handler.set(true);
                async {}
            },
        );

        assert_eq!(
            publish_result,
            SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
        );
        assert!(!publish_called.get());
        assert!(handle.intent(ManagedSubmitIntent::SaveDraft).can_submit());
        assert!(!handle.intent(ManagedSubmitIntent::Publish).can_submit());
        assert_eq!(
            handle.intent(ManagedSubmitIntent::Publish).last_status(),
            Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
        );
        assert_eq!(
            handle.intent(ManagedSubmitIntent::SaveDraft).last_status(),
            None
        );

        let latest = handle
            .last_submit_status_as::<ManagedSubmitIntent>()
            .expect("latest submit status should carry typed intent");
        assert_eq!(latest.intent(), &ManagedSubmitIntent::Publish);
        assert_eq!(
            latest.status(),
            SubmitStatus::Blocked(SubmitBlocker::ValidationErrors)
        );
    }
}
