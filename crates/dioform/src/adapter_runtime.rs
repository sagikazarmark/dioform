//! Internal Dioxus adapter runtime lifecycle.
//!
//! This module concentrates task tracking, debounced validation wakeups, validation waiters,
//! cleanup state, and runtime async validator registration behind the Form Handle facade.
//! Deleting it would push those lifecycle rules back into every Form Handle start, submit,
//! reset, reinitialize, and cleanup path.

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    task::{Context, Poll, Waker},
};

use dioxus_core::Task;

use crate::{
    CollectionItemIdentity, FieldIdentity, FormCore, FormHandle, ParseError, SelectedFile,
    ValidationStatus, ValidationTarget, ValidationTrigger, ValidatorId,
    adapter_input_state::{FileSelections, ParseBindingId, ParseState},
};

/// Opaque identity for one spawned async validation task, owned by the adapter and understood by
/// whichever [`TaskSpawner`] executes it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct TaskId(u64);

/// The seam between the adapter's async-validation lifecycle and the underlying task executor.
///
/// The adapter allocates a [`TaskId`], threads it through so completion and cancellation reference
/// the task explicitly, and delegates spawning and cancellation here. Production uses
/// [`DioxusSpawner`]; tests substitute an inline spawner, which is why this is a real seam rather
/// than a hard-wired call to `dioxus_core::spawn`.
pub(super) trait TaskSpawner {
    /// Spawns `future`, associating it with `id` for later cancellation.
    fn spawn(&self, id: TaskId, future: Pin<Box<dyn Future<Output = ()>>>);

    /// Cancels the task previously spawned with `id`, if it is still running.
    fn cancel(&self, id: TaskId);

    /// Spawns a fire-and-forget `future` that carries no [`TaskId`] and is never cancelled.
    ///
    /// **Submission** orchestration uses this: the managed-submission wait loop and the application
    /// submit future self-terminate through `is_active` and submit-generation guards rather than
    /// spawner-side cancellation (see ADR-0015), so they need spawning without the cancellable-task
    /// bookkeeping.
    fn spawn_detached(&self, future: Pin<Box<dyn Future<Output = ()>>>);
}

/// The production [`TaskSpawner`], backed by the Dioxus scheduler.
pub(super) struct DioxusSpawner {
    tasks: Rc<RefCell<BTreeMap<TaskId, Task>>>,
}

impl Default for DioxusSpawner {
    fn default() -> Self {
        Self {
            tasks: Rc::new(RefCell::new(BTreeMap::new())),
        }
    }
}

impl TaskSpawner for DioxusSpawner {
    fn spawn(&self, id: TaskId, future: Pin<Box<dyn Future<Output = ()>>>) {
        let tasks = Rc::clone(&self.tasks);
        let task = dioxus_core::spawn(async move {
            future.await;
            tasks.borrow_mut().remove(&id);
        });
        self.tasks.borrow_mut().insert(id, task);
    }

    fn cancel(&self, id: TaskId) {
        if let Some(task) = self.tasks.borrow_mut().remove(&id)
            && dioxus_core::Runtime::try_current().is_some()
        {
            dioxus_core::remove_future(task);
        }
    }

    fn spawn_detached(&self, future: Pin<Box<dyn Future<Output = ()>>>) {
        dioxus_core::spawn(future);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValidationTaskRegistration {
    id: TaskId,
    target: ValidationTarget,
    validator_id: ValidatorId,
}

#[derive(Clone)]
pub(super) struct AdapterRuntime {
    state: Rc<RefCell<AdapterState>>,
    spawner: Rc<dyn TaskSpawner>,
    parse: Rc<ParseState>,
    files: Rc<FileSelections>,
}

impl Default for AdapterRuntime {
    fn default() -> Self {
        Self::with_spawner(Rc::new(DioxusSpawner::default()))
    }
}

impl AdapterRuntime {
    /// Builds a runtime that spawns async validation tasks through `spawner`.
    pub(super) fn with_spawner(spawner: Rc<dyn TaskSpawner>) -> Self {
        Self {
            state: Rc::new(RefCell::new(AdapterState::default())),
            spawner,
            parse: Rc::new(ParseState::default()),
            files: Rc::new(FileSelections::default()),
        }
    }
}

impl AdapterRuntime {
    pub(super) fn deactivate(&self) {
        let cancelled = self.state.borrow_mut().deactivate();
        self.cancel_tasks(cancelled);
        self.files.clear_file_selections();
    }

    fn cancel_tasks(&self, ids: Vec<TaskId>) {
        for id in ids {
            self.spawner.cancel(id);
        }
    }

    pub(super) fn is_active(&self) -> bool {
        self.state.borrow().is_active()
    }

    pub(super) fn begin_managed_async_submission(&self) -> bool {
        self.state.borrow_mut().begin_managed_async_submission()
    }

    pub(super) fn finish_managed_async_submission(&self) {
        self.state.borrow_mut().finish_managed_async_submission();
    }

    pub(super) fn has_managed_async_submission(&self) -> bool {
        self.state.borrow().has_managed_async_submission()
    }

    pub(super) fn has_validation_tasks(&self) -> bool {
        self.state.borrow().has_validation_tasks()
    }

    pub(super) fn has_active_debounced_validation(
        &self,
        target: ValidationTarget,
        id: ValidatorId,
    ) -> bool {
        self.state
            .borrow()
            .has_active_debounced_validation(target, id)
    }

    pub(super) fn cancel_validation_tasks(&self) {
        let cancelled = self.state.borrow_mut().cancel_validation_tasks();
        self.cancel_tasks(cancelled);
    }

    pub(super) fn cancel_debounced_validation(&self, target: ValidationTarget, id: ValidatorId) {
        self.state
            .borrow_mut()
            .cancel_debounced_validation(target, id);
    }

    pub(super) fn cancel_validation_task(&self, target: ValidationTarget, id: ValidatorId) {
        let cancelled = self.state.borrow_mut().cancel_validation_task(target, id);
        self.cancel_tasks(cancelled);
    }

    pub(super) fn register_debounced_validation(
        &self,
        target: ValidationTarget,
        validator_id: ValidatorId,
    ) -> DebouncedValidationRegistration {
        self.state
            .borrow_mut()
            .register_debounced_validation(target, validator_id)
    }

    pub(super) fn flush_submit_relevant_debounced_validations<Model, Error>(
        &self,
        core: &FormCore<Model, Error>,
    ) where
        Model: Clone,
    {
        self.state
            .borrow_mut()
            .flush_submit_relevant_debounced_validations(core);
    }

    pub(super) fn validation_change(&self) -> ValidationChangeFuture {
        ValidationChangeFuture::new(Rc::clone(&self.state))
    }

    pub(super) fn wake_validation_waiters(&self) {
        self.state.borrow_mut().wake_validation_waiters();
    }

    pub(super) fn spawn_validation_task(
        &self,
        target: ValidationTarget,
        id: ValidatorId,
        future: impl Future<Output = ()> + 'static,
    ) {
        let adapter = Rc::clone(&self.state);
        let task_id = self.state.borrow_mut().next_task_id();
        self.state
            .borrow_mut()
            .register_validation_task(task_id, target, id);

        let wrapped = Box::pin(async move {
            future.await;

            let mut adapter = adapter.borrow_mut();
            adapter.finish_validation_task(task_id);
            adapter.wake_validation_waiters();
        });

        self.spawner.spawn(task_id, wrapped);
    }

    /// Spawns a fire-and-forget submission task through the spawner seam (ADR-0015).
    pub(super) fn spawn_detached(&self, future: impl Future<Output = ()> + 'static) {
        self.spawner.spawn_detached(Box::pin(future));
    }

    pub(super) fn register_parse_binding(&self, field: FieldIdentity) -> ParseBindingId {
        self.parse.register_parse_binding(field)
    }

    pub(super) fn unregister_parse_binding(&self, id: ParseBindingId) -> bool {
        self.parse.unregister_parse_binding(id)
    }

    pub(super) fn unregister_collection_item_parse_bindings(
        &self,
        collection: FieldIdentity,
        item: CollectionItemIdentity,
    ) -> Vec<FieldIdentity> {
        self.parse
            .unregister_collection_item_parse_bindings(collection, item)
    }

    pub(super) fn set_parse_error(&self, id: ParseBindingId, raw_value: String, message: String) {
        self.parse.set_parse_error(id, raw_value, message);
    }

    pub(super) fn clear_parse_error(&self, id: ParseBindingId) {
        self.parse.clear_parse_error(id);
    }

    pub(super) fn clear_parse_errors(&self) {
        self.parse.clear_parse_errors();
    }

    pub(super) fn clear_field_parse_errors(&self, field: &FieldIdentity) -> bool {
        self.parse.clear_field_parse_errors(field)
    }

    pub(super) fn parse_error(&self, id: ParseBindingId) -> Option<ParseError> {
        self.parse.parse_error(id)
    }

    pub(super) fn parse_errors(&self) -> Vec<ParseError> {
        self.parse.parse_errors()
    }

    pub(super) fn field_parse_errors(&self, field: FieldIdentity) -> Vec<ParseError> {
        self.parse.field_parse_errors(field)
    }

    pub(super) fn has_field_parse_errors(&self, field: FieldIdentity) -> bool {
        self.parse.has_field_parse_errors(field)
    }

    pub(super) fn has_parse_blockers(&self) -> bool {
        self.parse.has_parse_blockers()
    }

    pub(super) fn set_file_selection(&self, field: FieldIdentity, files: Vec<SelectedFile>) {
        self.files.set_file_selection(field, files);
    }

    pub(super) fn clear_file_selections(&self) {
        self.files.clear_file_selections();
    }

    pub(super) fn file_selection_snapshot(&self) -> BTreeMap<FieldIdentity, Vec<SelectedFile>> {
        self.files.file_selection_snapshot()
    }

    pub(super) fn file_selection(&self, field: FieldIdentity) -> Vec<SelectedFile> {
        self.files.file_selection(field)
    }
}

struct AdapterState {
    active: bool,
    managed_async_submission_pending: bool,
    next_task_id: u64,
    validation_tasks: Vec<ValidationTaskRegistration>,
    validation_waiters: Vec<Rc<ValidationWaiterState>>,
    debounced_validations: Vec<Weak<DebouncedValidationState>>,
}

impl Default for AdapterState {
    fn default() -> Self {
        Self {
            active: true,
            managed_async_submission_pending: false,
            next_task_id: 0,
            validation_tasks: Vec::new(),
            validation_waiters: Vec::new(),
            debounced_validations: Vec::new(),
        }
    }
}

struct DebouncedValidationState {
    target: ValidationTarget,
    validator_id: ValidatorId,
    flush_trigger: Cell<Option<ValidationTrigger>>,
    cancelled: Cell<bool>,
    completed: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DebounceWake {
    TimerElapsed,
    Flushed(ValidationTrigger),
    Cancelled,
}

pub(super) struct DebouncedValidationRegistration {
    state: Rc<DebouncedValidationState>,
}

impl DebouncedValidationRegistration {
    fn new(target: ValidationTarget, validator_id: ValidatorId) -> Self {
        Self {
            state: Rc::new(DebouncedValidationState {
                target,
                validator_id,
                flush_trigger: Cell::new(None),
                cancelled: Cell::new(false),
                completed: Cell::new(false),
                waker: RefCell::new(None),
            }),
        }
    }

    fn downgrade(&self) -> Weak<DebouncedValidationState> {
        Rc::downgrade(&self.state)
    }

    pub(super) fn delay<Delay>(self, delay: Delay) -> FlushableDelay<Delay>
    where
        Delay: Future<Output = ()>,
    {
        FlushableDelay {
            delay: Box::pin(delay),
            state: self.state,
        }
    }
}

pub(super) struct FlushableDelay<Delay> {
    delay: Pin<Box<Delay>>,
    state: Rc<DebouncedValidationState>,
}

impl DebouncedValidationState {
    fn flush(&self, trigger: ValidationTrigger) {
        if self.cancelled.get() || self.completed.get() {
            return;
        }

        self.flush_trigger.set(Some(trigger));

        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }

    fn cancel(&self) {
        if self.completed.replace(true) {
            return;
        }

        self.cancelled.set(true);

        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }
}

impl<Delay> Future for FlushableDelay<Delay>
where
    Delay: Future<Output = ()>,
{
    type Output = DebounceWake;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.state.cancelled.get() {
            self.state.completed.set(true);
            return Poll::Ready(DebounceWake::Cancelled);
        }

        if let Some(trigger) = self.state.flush_trigger.get() {
            self.state.completed.set(true);
            return Poll::Ready(DebounceWake::Flushed(trigger));
        }

        match self.delay.as_mut().poll(context) {
            Poll::Ready(()) => {
                self.state.completed.set(true);
                Poll::Ready(DebounceWake::TimerElapsed)
            }
            Poll::Pending => {
                self.state
                    .waker
                    .borrow_mut()
                    .replace(context.waker().clone());
                Poll::Pending
            }
        }
    }
}

struct ValidationWaiterState {
    notified: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

pub(super) struct ValidationChangeFuture {
    adapter: Rc<RefCell<AdapterState>>,
    state: Rc<ValidationWaiterState>,
    registered: bool,
}

impl ValidationChangeFuture {
    fn new(adapter: Rc<RefCell<AdapterState>>) -> Self {
        Self {
            adapter,
            state: Rc::new(ValidationWaiterState {
                notified: Cell::new(false),
                waker: RefCell::new(None),
            }),
            registered: false,
        }
    }
}

impl Future for ValidationChangeFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.state.notified.get() {
            return Poll::Ready(());
        }

        self.state
            .waker
            .borrow_mut()
            .replace(context.waker().clone());

        if !self.registered {
            self.adapter
                .borrow_mut()
                .validation_waiters
                .push(Rc::clone(&self.state));
            self.registered = true;
        }

        Poll::Pending
    }
}

impl AdapterState {
    pub(super) fn deactivate(&mut self) -> Vec<TaskId> {
        self.active = false;
        let cancelled = self.cancel_validation_tasks();
        self.wake_validation_waiters();
        cancelled
    }

    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    pub(super) fn begin_managed_async_submission(&mut self) -> bool {
        if self.managed_async_submission_pending {
            return false;
        }

        self.managed_async_submission_pending = true;
        true
    }

    pub(super) fn finish_managed_async_submission(&mut self) {
        self.managed_async_submission_pending = false;
    }

    pub(super) fn has_managed_async_submission(&self) -> bool {
        self.managed_async_submission_pending
    }

    pub(super) fn register_debounced_validation(
        &mut self,
        target: ValidationTarget,
        validator_id: ValidatorId,
    ) -> DebouncedValidationRegistration {
        let registration = DebouncedValidationRegistration::new(target, validator_id);
        self.debounced_validations.push(registration.downgrade());
        registration
    }

    pub(super) fn next_task_id(&mut self) -> TaskId {
        let id = TaskId(self.next_task_id);
        self.next_task_id += 1;
        id
    }

    pub(super) fn register_validation_task(
        &mut self,
        id: TaskId,
        target: ValidationTarget,
        validator_id: ValidatorId,
    ) {
        self.validation_tasks.push(ValidationTaskRegistration {
            id,
            target,
            validator_id,
        });
    }

    pub(super) fn has_validation_tasks(&self) -> bool {
        !self.validation_tasks.is_empty()
    }

    pub(super) fn has_active_debounced_validation(
        &self,
        target: ValidationTarget,
        id: ValidatorId,
    ) -> bool {
        self.debounced_validations.iter().any(|validation| {
            let Some(validation) = validation.upgrade() else {
                return false;
            };

            validation.target == target
                && validation.validator_id == id
                && !validation.completed.get()
                && !validation.cancelled.get()
        })
    }

    pub(super) fn finish_validation_task(&mut self, id: TaskId) {
        self.validation_tasks
            .retain(|registered| registered.id != id);
    }

    pub(super) fn cancel_validation_tasks(&mut self) -> Vec<TaskId> {
        let cancelled = self
            .validation_tasks
            .drain(..)
            .map(|registered| registered.id)
            .collect();

        self.debounced_validations.clear();
        cancelled
    }

    pub(super) fn cancel_debounced_validation(
        &mut self,
        target: ValidationTarget,
        id: ValidatorId,
    ) {
        self.debounced_validations.retain(|validation| {
            let Some(validation) = validation.upgrade() else {
                return false;
            };

            if validation.completed.get() {
                return false;
            }

            if validation.target == target && validation.validator_id == id {
                validation.cancel();
                return false;
            }

            true
        });
    }

    pub(super) fn cancel_validation_task(
        &mut self,
        target: ValidationTarget,
        id: ValidatorId,
    ) -> Vec<TaskId> {
        let mut cancelled = Vec::new();
        self.validation_tasks.retain(|registered| {
            if registered.target == target && registered.validator_id == id {
                cancelled.push(registered.id);
                false
            } else {
                true
            }
        });

        cancelled
    }

    pub(super) fn flush_submit_relevant_debounced_validations<Model, Error>(
        &mut self,
        core: &FormCore<Model, Error>,
    ) where
        Model: Clone,
    {
        self.debounced_validations.retain(|validation| {
            let Some(validation) = validation.upgrade() else {
                return false;
            };

            if validation.completed.get() || validation.cancelled.get() {
                return false;
            }

            if core.should_flush_debounced_validation_for_submit(
                &validation.target,
                validation.validator_id,
            ) {
                validation.flush(ValidationTrigger::Submit);
            }

            true
        });
    }

    pub(super) fn wake_validation_waiters(&mut self) {
        let waiters = std::mem::take(&mut self.validation_waiters);

        for waiter in waiters {
            waiter.notified.set(true);

            if let Some(waker) = waiter.waker.borrow_mut().take() {
                waker.wake();
            }
        }
    }
}

pub(super) type RuntimeFieldStart<Model, Error> =
    dyn Fn(FormHandle<Model, Error>, ValidationTrigger, bool) -> Option<ValidationStatus> + 'static;

pub(super) type RuntimeFormStart<Model, Error> =
    dyn Fn(FormHandle<Model, Error>, ValidationTrigger, bool) -> Option<ValidationStatus> + 'static;

pub(super) struct RuntimeAsyncFieldValidator<Model, Error> {
    pub(super) start: Rc<RuntimeFieldStart<Model, Error>>,
}

pub(super) struct RuntimeAsyncFormValidator<Model, Error> {
    pub(super) start: Rc<RuntimeFormStart<Model, Error>>,
}

// Dioxus adapter-owned async validation registry: the validators themselves, keyed for lookup.
// Task execution is separately abstracted behind the `TaskSpawner` seam (see ADR-0009); this
// registry only holds validator closures, not the spawning mechanism.
pub(super) struct ValidationRuntime<Model, Error> {
    pub(super) field_validators:
        BTreeMap<(FieldIdentity, ValidatorId), RuntimeAsyncFieldValidator<Model, Error>>,
    pub(super) form_validators: BTreeMap<ValidatorId, RuntimeAsyncFormValidator<Model, Error>>,
}

impl<Model, Error> Default for ValidationRuntime<Model, Error> {
    fn default() -> Self {
        Self {
            field_validators: BTreeMap::new(),
            form_validators: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::task::{Context, Poll, Waker};

    use super::*;

    fn form_validator_id() -> ValidatorId {
        let mut core = FormCore::new(());
        core.register_async_form_validator_for_triggers("runtime", ValidationTrigger::Change)
    }

    type BoxedTask = Pin<Box<dyn Future<Output = ()>>>;

    /// A test [`TaskSpawner`] that holds spawned futures until explicitly run and records
    /// cancellations. This is the second adapter that makes the spawner seam real: it exercises
    /// spawn, completion, and cancellation without a Dioxus runtime.
    #[derive(Default)]
    struct InlineSpawner {
        pending: RefCell<BTreeMap<TaskId, BoxedTask>>,
        detached: RefCell<Vec<BoxedTask>>,
        cancelled: RefCell<Vec<TaskId>>,
    }

    impl InlineSpawner {
        fn run_all(&self) {
            let tasks = std::mem::take(&mut *self.pending.borrow_mut());
            let waker = Waker::noop();
            let mut context = Context::from_waker(waker);
            for (_id, mut future) in tasks {
                let _ = future.as_mut().poll(&mut context);
            }
        }

        /// Drives every spawned future, cancellable and detached, to a fixed point, re-polling
        /// across the multi-await coordination a managed submission performs. A pass that completes
        /// no future means nothing can wake (wakes in this runtime follow task completion), so the
        /// executor has stalled and returns.
        fn run_until_stalled(&self) {
            let waker = Waker::noop();
            let mut context = Context::from_waker(waker);

            for _ in 0..1000 {
                // Poll detached submission tasks (the consumers that register interest in
                // validation settling) before the keyed validation tasks (the producers that wake
                // them), so a wake is never emitted before its waiter has registered.
                let mut batch: Vec<(Option<TaskId>, BoxedTask)> = Vec::new();
                batch.extend(
                    std::mem::take(&mut *self.detached.borrow_mut())
                        .into_iter()
                        .map(|future| (None, future)),
                );
                batch.extend(
                    std::mem::take(&mut *self.pending.borrow_mut())
                        .into_iter()
                        .map(|(id, future)| (Some(id), future)),
                );

                if batch.is_empty() {
                    return;
                }

                let mut completed_any = false;
                for (id, mut future) in batch {
                    if future.as_mut().poll(&mut context).is_ready() {
                        completed_any = true;
                    } else {
                        match id {
                            Some(id) => {
                                self.pending.borrow_mut().insert(id, future);
                            }
                            None => self.detached.borrow_mut().push(future),
                        }
                    }
                }

                if !completed_any {
                    return;
                }
            }

            panic!("inline spawner did not settle within the iteration bound");
        }
    }

    impl TaskSpawner for InlineSpawner {
        fn spawn(&self, id: TaskId, future: Pin<Box<dyn Future<Output = ()>>>) {
            self.pending.borrow_mut().insert(id, future);
        }

        fn cancel(&self, id: TaskId) {
            self.pending.borrow_mut().remove(&id);
            self.cancelled.borrow_mut().push(id);
        }

        fn spawn_detached(&self, future: Pin<Box<dyn Future<Output = ()>>>) {
            self.detached.borrow_mut().push(future);
        }
    }

    #[test]
    fn inline_spawner_runs_a_validation_task_to_completion() {
        let spawner = Rc::new(InlineSpawner::default());
        let runtime = AdapterRuntime::with_spawner(spawner.clone());
        let ran = Rc::new(Cell::new(false));
        let ran_in_task = Rc::clone(&ran);

        runtime.spawn_validation_task(ValidationTarget::Form, form_validator_id(), async move {
            ran_in_task.set(true);
        });

        assert!(runtime.has_validation_tasks());
        assert!(!ran.get());

        spawner.run_all();

        assert!(ran.get());
        assert!(!runtime.has_validation_tasks());
    }

    #[test]
    fn cancelling_validation_tasks_routes_through_the_spawner() {
        let spawner = Rc::new(InlineSpawner::default());
        let runtime = AdapterRuntime::with_spawner(spawner.clone());

        runtime.spawn_validation_task(
            ValidationTarget::Form,
            form_validator_id(),
            std::future::pending::<()>(),
        );
        assert!(runtime.has_validation_tasks());

        runtime.cancel_validation_tasks();

        assert!(!runtime.has_validation_tasks());
        assert_eq!(spawner.cancelled.borrow().len(), 1);
    }

    #[test]
    fn spawn_detached_runs_a_submission_task_to_completion() {
        let spawner = Rc::new(InlineSpawner::default());
        let runtime = AdapterRuntime::with_spawner(spawner.clone());
        let ran = Rc::new(Cell::new(false));
        let ran_in_task = Rc::clone(&ran);

        runtime.spawn_detached(async move {
            ran_in_task.set(true);
        });

        assert!(!ran.get());

        spawner.run_until_stalled();

        assert!(ran.get());
    }

    #[test]
    fn run_until_stalled_resumes_a_detached_task_after_validation_settles() {
        let spawner = Rc::new(InlineSpawner::default());
        let runtime = AdapterRuntime::with_spawner(spawner.clone());
        let resumed = Rc::new(Cell::new(false));

        // A detached submission-style task blocks until validation settles, then resumes: the
        // multi-await coordination the managed-submit wait loop performs.
        let change = runtime.validation_change();
        let resumed_in_task = Rc::clone(&resumed);
        runtime.spawn_detached(async move {
            change.await;
            resumed_in_task.set(true);
        });

        // A validation task whose completion wakes validation waiters.
        runtime.spawn_validation_task(ValidationTarget::Form, form_validator_id(), async {});

        assert!(!resumed.get());

        spawner.run_until_stalled();

        assert!(
            resumed.get(),
            "the detached task should resume once validation settles, driven without a VirtualDom"
        );
    }

    #[test]
    fn flushable_delay_can_be_flushed_without_waiting_for_timer() {
        let id = form_validator_id();
        let registration = DebouncedValidationRegistration::new(ValidationTarget::Form, id);
        let state = Rc::clone(&registration.state);
        let mut delay = Box::pin(registration.delay(std::future::pending::<()>()));
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);

        assert!(matches!(delay.as_mut().poll(&mut context), Poll::Pending));

        state.flush(ValidationTrigger::Submit);

        assert_eq!(
            delay.as_mut().poll(&mut context),
            Poll::Ready(DebounceWake::Flushed(ValidationTrigger::Submit))
        );
    }

    #[test]
    fn cancelling_debounced_validation_wakes_delay_as_cancelled() {
        let id = form_validator_id();
        let target = ValidationTarget::Form;
        let mut adapter = AdapterState::default();
        let registration = adapter.register_debounced_validation(target.clone(), id);
        let mut delay = Box::pin(registration.delay(std::future::pending::<()>()));
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);

        assert!(matches!(delay.as_mut().poll(&mut context), Poll::Pending));

        adapter.cancel_debounced_validation(target, id);

        assert_eq!(
            delay.as_mut().poll(&mut context),
            Poll::Ready(DebounceWake::Cancelled)
        );
    }

    #[test]
    fn validation_change_future_is_woken_by_adapter_state() {
        let adapter = Rc::new(RefCell::new(AdapterState::default()));
        let mut future = Box::pin(ValidationChangeFuture::new(Rc::clone(&adapter)));
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);

        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        assert_eq!(adapter.borrow().validation_waiters.len(), 1);

        adapter.borrow_mut().wake_validation_waiters();

        assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(()));
        assert!(adapter.borrow().validation_waiters.is_empty());
    }
}
