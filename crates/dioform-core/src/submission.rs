//! The submit lifecycle state of one **Form Core**.
//!
//! Submit-attempt counting, in-flight tracking, the last recorded outcome, the current
//! submit-validation intent, and stored **Submit Errors** previously lived as five loose fields on
//! `FormCore`, mutated across dozens of methods. This module concentrates them so the submit state
//! machine has one owner and illegal combinations are harder to produce.
//!
//! Orchestration that spans other subsystems stays in `FormCore`: it runs submit-triggered
//! validation, emits `Form Observer` events, and composes **Submit Availability** from this state
//! plus the validation chain's submit-blocking queries. `SubmissionState` performs only the state
//! transitions and holds the stored errors; it never reaches into the chain. **Submit Intent** is
//! stored erased here, so the typed intent stays at the intent-scoped boundary.

use crate::{FieldIdentity, StoredLastSubmitStatus, StoredSubmitError, SubmitIntentSnapshot};

/// Owns the submit lifecycle state of one **Form Core**.
pub(crate) struct SubmissionState<Error> {
    attempts: u64,
    in_flight: Option<SubmitIntentSnapshot>,
    last_status: Option<StoredLastSubmitStatus>,
    validation_intent: Option<SubmitIntentSnapshot>,
    errors: Vec<StoredSubmitError<Error>>,
}

impl<Error> Default for SubmissionState<Error> {
    fn default() -> Self {
        Self {
            attempts: 0,
            in_flight: None,
            last_status: None,
            validation_intent: None,
            errors: Vec::new(),
        }
    }
}

impl<Error> SubmissionState<Error> {
    // --- submit attempts ---

    /// Counts one submit attempt and returns the new total.
    pub(crate) fn increment_attempt(&mut self) -> u64 {
        self.attempts = self
            .attempts
            .checked_add(1)
            .expect("submit attempt counter exhausted");
        self.attempts
    }

    /// Returns how many submit attempts have been recorded.
    pub(crate) const fn attempt_count(&self) -> u64 {
        self.attempts
    }

    /// Restores the persisted submit-attempt count from a form-state snapshot.
    pub(crate) fn restore_attempt_count(&mut self, attempts: u64) {
        self.attempts = attempts;
    }

    // --- in-flight submission ---

    /// Returns whether a submission has started and not completed.
    pub(crate) const fn is_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Opens the in-flight submission window with its erased submit intent.
    pub(crate) fn start_in_flight(&mut self, intent: SubmitIntentSnapshot) {
        self.in_flight = Some(intent);
    }

    /// Borrows the erased submit intent of the in-flight submission.
    pub(crate) fn in_flight_intent(&self) -> Option<&SubmitIntentSnapshot> {
        self.in_flight.as_ref()
    }

    /// Closes the in-flight submission window and returns its erased submit intent.
    pub(crate) fn take_in_flight_intent(&mut self) -> Option<SubmitIntentSnapshot> {
        self.in_flight.take()
    }

    // --- submit-validation intent ---

    /// Sets the erased intent used by submit-triggered validation.
    pub(crate) fn set_validation_intent(&mut self, intent: Option<SubmitIntentSnapshot>) {
        self.validation_intent = intent;
    }

    /// Borrows the erased submit-validation intent.
    pub(crate) fn validation_intent(&self) -> Option<&SubmitIntentSnapshot> {
        self.validation_intent.as_ref()
    }

    // --- last submit status ---

    /// Records the latest meaningful submit outcome.
    pub(crate) fn record_status(&mut self, status: StoredLastSubmitStatus) {
        self.last_status = Some(status);
    }

    /// Borrows the latest recorded submit outcome, if any.
    pub(crate) fn last_status(&self) -> Option<&StoredLastSubmitStatus> {
        self.last_status.as_ref()
    }

    // --- stored submit errors ---

    /// Borrows the stored **Submit Errors**.
    pub(crate) fn errors(&self) -> &[StoredSubmitError<Error>] {
        &self.errors
    }

    /// Returns whether any **Submit Error** is currently stored.
    pub(crate) fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Returns whether a **Submit Error** is attached to one field.
    pub(crate) fn has_error_for_field(&self, field: &FieldIdentity) -> bool {
        self.errors
            .iter()
            .any(|error| error.target.as_field() == Some(field))
    }

    /// Replaces the stored **Submit Errors** with the outcome of one submission.
    pub(crate) fn set_errors(&mut self, errors: Vec<StoredSubmitError<Error>>) {
        self.errors = errors;
    }

    /// Clears all stored **Submit Errors**.
    pub(crate) fn clear_errors(&mut self) {
        self.errors.clear();
    }

    /// Retains stored **Submit Errors** matching `keep`.
    pub(crate) fn retain_errors(&mut self, keep: impl FnMut(&StoredSubmitError<Error>) -> bool) {
        self.errors.retain(keep);
    }

    // --- whole-lifecycle reset ---

    /// Clears all submit lifecycle state on reset, reinitialization, or snapshot restore.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}
