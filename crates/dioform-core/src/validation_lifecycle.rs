//! Per-registered-validator validation lifecycle state.
//!
//! This module owns source-level status transitions, stored validator errors, pending async run
//! freshness, trigger relevance, and debounced scheduling rules. One `SourceState` owns exactly
//! one registered validator source.
//!
//! Keep target-aware work outside this source-level state: `validation_chain` owns registry ordering
//! and flattened validation views, while Form Core owns field/form snapshot capture, observer storage
//! and emission, submit availability, and submit error storage. Runtime execution stays in renderer
//! adapters, so this private slice records pending work and stale completions without spawning tasks,
//! polling timers, or depending on Dioxus. See `docs/async-validation.md` for the durable boundary
//! note that follows ADR-0001's renderer-agnostic core direction.

use super::{
    SubmitIntentSnapshot, ValidationStatus, ValidationTrigger, ValidationTriggers, ValidatorSource,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceKind {
    Sync,
    Async,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingRun {
    Required { trigger: ValidationTrigger },
    Started { trigger: ValidationTrigger },
    Debounced { trigger: ValidationTrigger },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TransitionKind {
    ValidationRan,
    AsyncValidationScheduled,
    AsyncValidationCompleted,
    AsyncValidationSkipped,
    AsyncValidationStaleIgnored,
    DebouncedAsyncValidationScheduled,
    DebouncedAsyncValidationFlushed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TransitionOutcome {
    source: ValidatorSource,
    trigger: ValidationTrigger,
    status: ValidationStatus,
    kind: TransitionKind,
}

impl TransitionOutcome {
    pub(super) fn source(&self) -> &ValidatorSource {
        &self.source
    }

    pub(super) const fn trigger(&self) -> ValidationTrigger {
        self.trigger
    }

    pub(super) const fn status(&self) -> ValidationStatus {
        self.status
    }

    pub(super) const fn kind(&self) -> TransitionKind {
        self.kind
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SourceState<Error> {
    source: ValidatorSource,
    triggers: ValidationTriggers,
    kind: SourceKind,
    status: ValidationStatus,
    status_trigger: Option<ValidationTrigger>,
    #[cfg_attr(feature = "serde", serde(skip))]
    submit_intent: Option<SubmitIntentSnapshot>,
    errors: Vec<Error>,
    async_run: u64,
    pending_run: Option<PendingRun>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SourceResultSnapshot<Error> {
    status: ValidationStatus,
    status_trigger: Option<ValidationTrigger>,
    errors: Vec<Error>,
}

impl<Error> SourceResultSnapshot<Error> {
    pub(super) fn retain_errors(&mut self, mut retain: impl FnMut(&Error) -> bool) {
        self.errors.retain(|error| retain(error));

        if self.status == ValidationStatus::Invalid && self.errors.is_empty() {
            self.status = ValidationStatus::Unknown;
            self.status_trigger = None;
        }
    }
}

impl<Error> SourceState<Error> {
    pub(super) fn new(
        source: ValidatorSource,
        triggers: ValidationTriggers,
        kind: SourceKind,
    ) -> Self {
        Self {
            source,
            triggers,
            kind,
            status: ValidationStatus::Unknown,
            status_trigger: None,
            submit_intent: None,
            errors: Vec::new(),
            async_run: 0,
            pending_run: None,
        }
    }

    pub(super) fn source(&self) -> &ValidatorSource {
        &self.source
    }

    pub(super) fn errors(&self) -> &[Error] {
        &self.errors
    }

    pub(super) fn retain_errors(&mut self, mut retain: impl FnMut(&Error) -> bool) {
        self.errors.retain(|error| retain(error));

        if self.status == ValidationStatus::Invalid && self.errors.is_empty() {
            self.status = ValidationStatus::Valid;
        }
    }

    pub(super) fn status(&self) -> ValidationStatus {
        self.status
    }

    pub(super) fn has_submit_scoped_status(&self) -> bool {
        self.status_trigger == Some(ValidationTrigger::Submit)
    }

    pub(super) fn snapshot_result(&self) -> SourceResultSnapshot<Error>
    where
        Error: Clone,
    {
        if self.status == ValidationStatus::Pending
            || self.status_trigger == Some(ValidationTrigger::Submit)
        {
            return SourceResultSnapshot {
                status: ValidationStatus::Unknown,
                status_trigger: None,
                errors: Vec::new(),
            };
        }

        SourceResultSnapshot {
            status: self.status,
            status_trigger: self.status_trigger,
            errors: self.errors.clone(),
        }
    }

    pub(super) fn restore_result_from_snapshot(&mut self, snapshot: SourceResultSnapshot<Error>) {
        if snapshot.status == ValidationStatus::Pending
            || snapshot.status_trigger == Some(ValidationTrigger::Submit)
        {
            self.clear();
            return;
        }

        self.errors = snapshot.errors;
        self.status = snapshot.status;
        self.status_trigger = snapshot.status_trigger;
        self.submit_intent = None;
        self.pending_run = None;
    }

    pub(super) fn matches_submit_intent<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.status_trigger != Some(ValidationTrigger::Submit)
            || self.submit_status_matches_intent(intent)
    }

    fn submit_status_matches_intent<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.status_trigger == Some(ValidationTrigger::Submit)
            && self
                .submit_intent
                .as_ref()
                .is_none_or(|submit_intent| submit_intent.matches(intent))
    }

    pub(super) const fn is_sync(&self) -> bool {
        matches!(self.kind, SourceKind::Sync)
    }

    pub(super) const fn is_async(&self) -> bool {
        matches!(self.kind, SourceKind::Async)
    }

    pub(super) fn should_run(&self, trigger: ValidationTrigger) -> bool {
        self.triggers.contains(trigger)
    }

    /// Returns whether this source has errors that count against submit availability for `intent`.
    pub(crate) fn errors_affect_submit_availability<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        !self.errors().is_empty() && self.matches_submit_intent(intent)
    }

    /// Returns whether this source has submit-triggered errors that block submission for `intent`.
    pub(crate) fn errors_block_submit_intent<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        if self.errors().is_empty() {
            return false;
        }

        self.should_run(ValidationTrigger::Submit) && self.matches_submit_intent(intent)
    }

    /// Returns whether this source has pending submit-triggered validation blocking `intent`.
    pub(crate) fn pending_blocks_submit_intent<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.should_run(ValidationTrigger::Submit)
            && self.status() == ValidationStatus::Pending
            && self.matches_submit_intent(intent)
    }

    /// Returns whether this source has unresolved async submit validation for `intent`.
    pub(crate) fn unresolved_submit_async<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.is_async()
            && self.should_run(ValidationTrigger::Submit)
            && !self.async_status_resolved_for_submit_intent(intent)
    }

    pub(super) fn should_skip_async_after_sync_failure(&self, trigger: ValidationTrigger) -> bool {
        self.is_async() && self.should_run(trigger)
    }

    pub(super) fn should_clear_async_skip_after_sync_success(
        &self,
        trigger: ValidationTrigger,
    ) -> bool {
        self.should_skip_async_after_sync_failure(trigger)
            && self.status == ValidationStatus::Skipped
    }

    pub(super) fn should_schedule_debounced_async(&self, trigger: ValidationTrigger) -> bool {
        trigger == ValidationTrigger::Change && self.should_run(trigger)
    }

    pub(super) fn should_schedule_debounced_async_after_sync(
        &self,
        trigger: ValidationTrigger,
    ) -> bool {
        self.should_schedule_debounced_async(trigger) && self.status != ValidationStatus::Skipped
    }

    pub(super) fn should_begin_debounced_async(
        &self,
        scheduled_trigger: ValidationTrigger,
        scheduled_run_id: u64,
    ) -> bool {
        self.should_run(scheduled_trigger)
            && self.has_debounced_pending_run_for(scheduled_trigger)
            && !self.should_ignore_async_completion(scheduled_run_id)
    }

    pub(super) fn should_begin_async_after_sync(&self, trigger: ValidationTrigger) -> bool {
        self.is_async()
            && self.should_run(trigger)
            && self.status != ValidationStatus::Skipped
            && !(trigger == ValidationTrigger::Submit
                && self.has_started_pending_run_for(ValidationTrigger::Submit))
    }

    pub(super) fn should_flush_debounced_async_for_trigger(
        &self,
        scheduled_run_id: u64,
        trigger: ValidationTrigger,
    ) -> bool {
        self.should_run(trigger)
            && self.has_debounced_pending_run()
            && !self.should_ignore_async_completion(scheduled_run_id)
    }

    pub(super) fn should_flush_debounced_async_for_submit(&self) -> bool {
        self.should_run(ValidationTrigger::Submit)
            && self.status == ValidationStatus::Pending
            && self.has_debounced_pending_run()
    }

    pub(super) fn should_ignore_async_completion(&self, run_id: u64) -> bool {
        self.async_run != run_id
    }

    pub(super) fn has_started_pending_run_for(&self, trigger: ValidationTrigger) -> bool {
        matches!(
            self.pending_run,
            Some(PendingRun::Started { trigger: pending_trigger }) if pending_trigger == trigger
        )
    }

    fn has_debounced_pending_run(&self) -> bool {
        matches!(self.pending_run, Some(PendingRun::Debounced { .. }))
    }

    fn has_debounced_pending_run_for(&self, trigger: ValidationTrigger) -> bool {
        matches!(
            self.pending_run,
            Some(PendingRun::Debounced { trigger: pending_trigger }) if pending_trigger == trigger
        )
    }

    #[cfg(test)]
    pub(super) fn async_status_needs_run_for_trigger(&self, trigger: ValidationTrigger) -> bool {
        match self.status {
            ValidationStatus::Unknown | ValidationStatus::Stale => true,
            ValidationStatus::Valid | ValidationStatus::Invalid => {
                self.status_trigger != Some(trigger)
            }
            ValidationStatus::Pending | ValidationStatus::Skipped => false,
        }
    }

    #[cfg(test)]
    pub(super) fn async_status_resolved_for_trigger(&self, trigger: ValidationTrigger) -> bool {
        matches!(
            self.status,
            ValidationStatus::Valid | ValidationStatus::Invalid
        ) && self.status_trigger == Some(trigger)
    }

    pub(super) fn async_status_needs_run_for_submit_intent<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        match self.status {
            ValidationStatus::Unknown | ValidationStatus::Stale => true,
            ValidationStatus::Valid | ValidationStatus::Invalid | ValidationStatus::Pending => {
                !self.submit_status_matches_intent(intent)
            }
            ValidationStatus::Skipped => {
                self.status_trigger != Some(ValidationTrigger::Submit)
                    || !self.submit_status_matches_intent(intent)
            }
        }
    }

    pub(super) fn async_status_resolved_for_submit_intent<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        matches!(
            self.status,
            ValidationStatus::Valid | ValidationStatus::Invalid
        ) && self.submit_status_matches_intent(intent)
    }

    pub(super) fn replace_errors(
        &mut self,
        trigger: ValidationTrigger,
        submit_intent: Option<SubmitIntentSnapshot>,
        errors: Vec<Error>,
    ) -> TransitionOutcome {
        self.replace_errors_for_transition(
            trigger,
            submit_intent,
            errors,
            TransitionKind::ValidationRan,
        )
    }

    fn replace_errors_for_transition(
        &mut self,
        trigger: ValidationTrigger,
        submit_intent: Option<SubmitIntentSnapshot>,
        errors: Vec<Error>,
        kind: TransitionKind,
    ) -> TransitionOutcome {
        let status = status_for_errors(&errors);

        self.errors = errors;
        self.status = status;
        self.status_trigger = Some(trigger);
        self.submit_intent = submit_intent;
        self.pending_run = None;

        self.outcome(trigger, kind)
    }

    pub(super) fn skip_async(
        &mut self,
        trigger: ValidationTrigger,
        submit_intent: Option<SubmitIntentSnapshot>,
    ) -> TransitionOutcome {
        self.errors.clear();
        self.status = ValidationStatus::Skipped;
        self.status_trigger = Some(trigger);
        self.submit_intent = submit_intent;
        self.pending_run = None;
        self.advance_async_run();

        self.outcome(trigger, TransitionKind::AsyncValidationSkipped)
    }

    pub(super) fn mark_skipped_without_trigger(&mut self) {
        self.errors.clear();
        self.status = ValidationStatus::Skipped;
        self.status_trigger = None;
        self.submit_intent = None;
        self.pending_run = None;
        if self.is_async() {
            self.advance_async_run();
        }
    }

    pub(super) fn clear(&mut self) {
        self.errors.clear();
        self.status = ValidationStatus::Unknown;
        self.status_trigger = None;
        self.submit_intent = None;
        self.pending_run = None;
        if self.is_async() {
            self.advance_async_run();
        }
    }

    pub(super) fn clear_async_skip(&mut self) {
        self.status = ValidationStatus::Unknown;
        self.status_trigger = None;
        self.submit_intent = None;
        self.pending_run = None;
    }

    pub(super) fn mark_async_started(
        &mut self,
        trigger: ValidationTrigger,
        submit_intent: Option<SubmitIntentSnapshot>,
    ) -> (u64, TransitionOutcome) {
        self.mark_async_pending(
            trigger,
            submit_intent,
            PendingRun::Started { trigger },
            TransitionKind::AsyncValidationScheduled,
        )
    }

    pub(super) fn mark_async_required(
        &mut self,
        trigger: ValidationTrigger,
        submit_intent: Option<SubmitIntentSnapshot>,
    ) -> (u64, TransitionOutcome) {
        if trigger == ValidationTrigger::Submit && self.has_debounced_pending_run() {
            return self.claim_debounced_async_for_submit(trigger, submit_intent);
        }

        self.mark_async_pending(
            trigger,
            submit_intent,
            PendingRun::Required { trigger },
            TransitionKind::AsyncValidationScheduled,
        )
    }

    pub(super) fn mark_debounced_async_pending(
        &mut self,
        trigger: ValidationTrigger,
        submit_intent: Option<SubmitIntentSnapshot>,
    ) -> (u64, TransitionOutcome) {
        self.mark_async_pending(
            trigger,
            submit_intent,
            PendingRun::Debounced { trigger },
            TransitionKind::DebouncedAsyncValidationScheduled,
        )
    }

    pub(super) fn begin_debounced_async(
        &mut self,
        trigger: ValidationTrigger,
    ) -> (u64, TransitionOutcome) {
        self.status = ValidationStatus::Pending;
        self.status_trigger = Some(trigger);
        self.pending_run = Some(PendingRun::Started { trigger });
        let run_id = self.advance_async_run();

        (
            run_id,
            self.outcome(trigger, TransitionKind::DebouncedAsyncValidationFlushed),
        )
    }

    fn claim_debounced_async_for_submit(
        &mut self,
        trigger: ValidationTrigger,
        submit_intent: Option<SubmitIntentSnapshot>,
    ) -> (u64, TransitionOutcome) {
        self.errors.clear();
        self.status = ValidationStatus::Pending;
        self.status_trigger = Some(trigger);
        self.submit_intent = submit_intent;

        (
            self.async_run,
            self.outcome(trigger, TransitionKind::AsyncValidationScheduled),
        )
    }

    pub(super) fn debounced_async_flushed(&self, trigger: ValidationTrigger) -> TransitionOutcome {
        self.outcome(trigger, TransitionKind::DebouncedAsyncValidationFlushed)
    }

    pub(super) fn complete_async(
        &mut self,
        trigger: ValidationTrigger,
        submit_intent: Option<SubmitIntentSnapshot>,
        errors: Vec<Error>,
    ) -> TransitionOutcome {
        let outcome = self.replace_errors_for_transition(
            trigger,
            submit_intent,
            errors,
            TransitionKind::AsyncValidationCompleted,
        );
        self.advance_async_run();

        outcome
    }

    pub(super) fn stale_async_completion_ignored(
        &self,
        trigger: ValidationTrigger,
    ) -> TransitionOutcome {
        TransitionOutcome {
            source: self.source.clone(),
            trigger,
            status: ValidationStatus::Stale,
            kind: TransitionKind::AsyncValidationStaleIgnored,
        }
    }

    pub(super) fn mark_stale(&mut self) {
        self.errors.clear();
        self.status = ValidationStatus::Stale;
        self.status_trigger = None;
        self.submit_intent = None;
        self.advance_async_run();
        self.pending_run = None;
    }

    fn mark_async_pending(
        &mut self,
        trigger: ValidationTrigger,
        submit_intent: Option<SubmitIntentSnapshot>,
        pending_run: PendingRun,
        kind: TransitionKind,
    ) -> (u64, TransitionOutcome) {
        self.errors.clear();
        self.status = ValidationStatus::Pending;
        self.status_trigger = Some(trigger);
        self.submit_intent = submit_intent;
        self.pending_run = Some(pending_run);
        let run_id = self.advance_async_run();

        (run_id, self.outcome(trigger, kind))
    }

    fn advance_async_run(&mut self) -> u64 {
        self.async_run = self
            .async_run
            .checked_add(1)
            .expect("async validation run counter exhausted");

        self.async_run
    }

    fn outcome(&self, trigger: ValidationTrigger, kind: TransitionKind) -> TransitionOutcome {
        TransitionOutcome {
            source: self.source.clone(),
            trigger,
            status: self.status,
            kind,
        }
    }
}

pub(super) fn is_async_form_stale(current_form_version: u64, run_form_version: u64) -> bool {
    current_form_version != run_form_version
}

pub(super) fn is_async_field_stale(
    current_form_version: u64,
    run_form_version: u64,
    current_field_version: u64,
    run_field_version: u64,
) -> bool {
    is_async_form_stale(current_form_version, run_form_version)
        || current_field_version != run_field_version
}

fn status_for_errors<Error>(errors: &[Error]) -> ValidationStatus {
    if errors.is_empty() {
        ValidationStatus::Valid
    } else {
        ValidationStatus::Invalid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn async_state(triggers: ValidationTriggers) -> SourceState<&'static str> {
        SourceState::new(
            ValidatorSource::new("availability"),
            triggers,
            SourceKind::Async,
        )
    }

    #[test]
    fn replace_errors_sets_status_and_replaces_stored_errors() {
        let mut state = SourceState::new(
            ValidatorSource::new("required"),
            ValidationTriggers::all(),
            SourceKind::Sync,
        );

        let outcome =
            state.replace_errors(ValidationTrigger::Manual, None, vec!["required", "blank"]);

        assert_eq!(outcome.source().as_str(), "required");
        assert_eq!(outcome.trigger(), ValidationTrigger::Manual);
        assert_eq!(outcome.status(), ValidationStatus::Invalid);
        assert_eq!(outcome.kind(), TransitionKind::ValidationRan);
        assert_eq!(state.status(), ValidationStatus::Invalid);
        assert_eq!(state.errors(), &["required", "blank"]);

        let outcome = state.replace_errors(ValidationTrigger::Blur, None, Vec::new());

        assert_eq!(outcome.trigger(), ValidationTrigger::Blur);
        assert_eq!(outcome.status(), ValidationStatus::Valid);
        assert_eq!(outcome.kind(), TransitionKind::ValidationRan);
        assert_eq!(state.status(), ValidationStatus::Valid);
        assert!(state.errors().is_empty());
    }

    #[test]
    fn skip_async_clears_errors_marks_skipped_and_invalidates_pending_run() {
        let mut state = async_state(ValidationTrigger::Manual.into());
        let (run_id, _) = state.mark_async_started(ValidationTrigger::Manual, None);

        assert_eq!(state.status(), ValidationStatus::Pending);
        assert!(!state.should_ignore_async_completion(run_id));

        state.replace_errors(ValidationTrigger::Manual, None, vec!["old_error"]);

        assert_eq!(state.errors(), &["old_error"]);

        let outcome = state.skip_async(ValidationTrigger::Manual, None);

        assert_eq!(outcome.status(), ValidationStatus::Skipped);
        assert_eq!(outcome.kind(), TransitionKind::AsyncValidationSkipped);
        assert_eq!(state.status(), ValidationStatus::Skipped);
        assert!(state.errors().is_empty());
        assert!(state.should_ignore_async_completion(run_id));
        assert!(state.should_clear_async_skip_after_sync_success(ValidationTrigger::Manual));

        state.clear_async_skip();

        assert_eq!(state.status(), ValidationStatus::Unknown);
    }

    #[test]
    fn async_run_freshness_ignores_old_and_duplicate_completions() {
        let mut state = async_state(ValidationTriggers::all());
        let (first_run_id, first_outcome) =
            state.mark_async_started(ValidationTrigger::Manual, None);
        let (second_run_id, second_outcome) =
            state.mark_async_started(ValidationTrigger::Manual, None);

        assert_eq!(
            first_outcome.kind(),
            TransitionKind::AsyncValidationScheduled
        );
        assert_eq!(
            second_outcome.kind(),
            TransitionKind::AsyncValidationScheduled
        );
        assert!(state.should_ignore_async_completion(first_run_id));
        assert!(!state.should_ignore_async_completion(second_run_id));

        let outcome = state.complete_async(ValidationTrigger::Manual, None, Vec::new());

        assert_eq!(outcome.status(), ValidationStatus::Valid);
        assert_eq!(outcome.kind(), TransitionKind::AsyncValidationCompleted);
        assert_eq!(state.status(), ValidationStatus::Valid);
        assert!(state.should_ignore_async_completion(second_run_id));

        let outcome = state.stale_async_completion_ignored(ValidationTrigger::Manual);

        assert_eq!(outcome.status(), ValidationStatus::Stale);
        assert_eq!(outcome.kind(), TransitionKind::AsyncValidationStaleIgnored);
    }

    #[test]
    fn submit_trigger_relevance_requires_a_submit_triggered_result() {
        let mut state = async_state(ValidationTriggers::new([
            ValidationTrigger::Change,
            ValidationTrigger::Submit,
        ]));
        let (value_change_run_id, _) = state.mark_async_started(ValidationTrigger::Change, None);

        assert!(!state.should_ignore_async_completion(value_change_run_id));
        state.complete_async(ValidationTrigger::Change, None, Vec::new());

        assert!(state.async_status_resolved_for_trigger(ValidationTrigger::Change));
        assert!(!state.async_status_resolved_for_trigger(ValidationTrigger::Submit));
        assert!(state.async_status_needs_run_for_trigger(ValidationTrigger::Submit));

        let (_, outcome) = state.mark_async_required(
            ValidationTrigger::Submit,
            Some(SubmitIntentSnapshot::new("publish")),
        );

        assert_eq!(outcome.status(), ValidationStatus::Pending);
        assert_eq!(outcome.kind(), TransitionKind::AsyncValidationScheduled);
        assert!(!state.async_status_needs_run_for_trigger(ValidationTrigger::Submit));

        state.complete_async(
            ValidationTrigger::Submit,
            Some(SubmitIntentSnapshot::new("publish")),
            Vec::new(),
        );

        assert!(state.async_status_resolved_for_trigger(ValidationTrigger::Submit));
    }

    #[test]
    fn submit_trigger_relevance_includes_submit_intent() {
        #[derive(Eq, PartialEq)]
        enum Intent {
            SaveDraft,
            Publish,
        }

        let mut state = async_state(ValidationTrigger::Submit.into());

        state.complete_async(
            ValidationTrigger::Submit,
            Some(SubmitIntentSnapshot::new(Intent::Publish)),
            Vec::<&'static str>::new(),
        );

        assert!(state.async_status_resolved_for_submit_intent(&Intent::Publish));
        assert!(!state.async_status_resolved_for_submit_intent(&Intent::SaveDraft));
        assert!(!state.async_status_needs_run_for_submit_intent(&Intent::Publish));
        assert!(state.async_status_needs_run_for_submit_intent(&Intent::SaveDraft));
    }

    #[test]
    fn debounced_transitions_track_latest_schedule_and_submit_flush_freshness() {
        let mut state = async_state(ValidationTriggers::new([
            ValidationTrigger::Change,
            ValidationTrigger::Submit,
        ]));
        let (first_run_id, outcome) =
            state.mark_debounced_async_pending(ValidationTrigger::Change, None);

        assert_eq!(outcome.status(), ValidationStatus::Pending);
        assert_eq!(
            outcome.kind(),
            TransitionKind::DebouncedAsyncValidationScheduled
        );
        assert!(state.should_begin_debounced_async(ValidationTrigger::Change, first_run_id));
        assert!(state.should_flush_debounced_async_for_submit());

        let (second_run_id, _) =
            state.mark_debounced_async_pending(ValidationTrigger::Change, None);

        assert!(!state.should_begin_debounced_async(ValidationTrigger::Change, first_run_id));
        assert!(state.should_begin_debounced_async(ValidationTrigger::Change, second_run_id));
        assert!(state.should_flush_debounced_async_for_trigger(
            second_run_id,
            ValidationTrigger::Submit,
        ));

        let (claimed_run_id, outcome) = state.mark_async_required(
            ValidationTrigger::Submit,
            Some(SubmitIntentSnapshot::new("publish")),
        );

        assert_eq!(claimed_run_id, second_run_id);
        assert_eq!(outcome.status(), ValidationStatus::Pending);
        assert_eq!(outcome.kind(), TransitionKind::AsyncValidationScheduled);
        assert!(state.should_flush_debounced_async_for_trigger(
            second_run_id,
            ValidationTrigger::Submit,
        ));

        let (started_run_id, outcome) = state.begin_debounced_async(ValidationTrigger::Submit);

        assert_eq!(outcome.status(), ValidationStatus::Pending);
        assert_eq!(
            outcome.kind(),
            TransitionKind::DebouncedAsyncValidationFlushed
        );
        assert!(state.should_ignore_async_completion(second_run_id));
        assert!(!state.should_ignore_async_completion(started_run_id));

        let outcome = state.complete_async(
            ValidationTrigger::Submit,
            Some(SubmitIntentSnapshot::new("publish")),
            vec!["submit_error"],
        );

        assert_eq!(outcome.status(), ValidationStatus::Invalid);
        assert_eq!(state.errors(), &["submit_error"]);
    }
}
