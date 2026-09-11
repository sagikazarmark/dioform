//! The adapter half of a rejected browser response: typed rejection plus one-time raw input.

use super::*;

/// A mapped browser rejection and optional unparsable text from the same response.
///
/// Create this inside the restoration mapping callback. Collection identities must come from
/// its receiving [`BrowserRejectionTargets`], never from another form instance.
pub struct BrowserRejection<Model, Error = String> {
    errors: SubmitErrors<Model, Error>,
    raw: BTreeMap<FieldIdentity, String>,
}

impl<Model, Error> BrowserRejection<Model, Error> {
    /// Pairs typed rejection errors with optional adapter-owned raw text.
    pub fn new(errors: SubmitErrors<Model, Error>) -> Self {
        Self {
            errors,
            raw: BTreeMap::new(),
        }
    }

    /// Restores unparsable text for a typed field when its first parsed binding mounts.
    /// Valid text is discarded in favor of the supplied typed response value and formatter.
    pub fn raw_field<Value>(self, path: FieldPath<Model, Value>, text: impl Into<String>) -> Self {
        self.raw_field_identity(path.identity(), text)
    }

    /// Restores text for an identity resolved in the receiving restoration mapping callback.
    pub fn raw_field_identity(mut self, field: FieldIdentity, text: impl Into<String>) -> Self {
        self.raw.insert(field, text.into());
        self
    }
}

impl<Model: Clone + 'static, Error: 'static> FormConfig<Model, Error> {
    /// Initializes each form instance from this configuration's rejected response values.
    ///
    /// Runs once before configured registrations, so explicitly configured initialization
    /// validation can coexist with the rejection. Hook rerenders do not replay restoration.
    /// Supply `()` for a non-intentful form.
    pub fn browser_rejection<Intent: Clone + 'static>(
        mut self,
        intent: Intent,
        map: impl Fn(&mut BrowserRejectionTargets<'_, Model, Error>) -> BrowserRejection<Model, Error>
        + 'static,
    ) -> Self {
        self.browser_rejection = Some(Rc::new(move |handle| {
            handle.restore_browser_rejection(handle.snapshot(), intent.clone(), |targets| {
                map(targets)
            });
        }));
        self
    }
}

impl<Model: Clone, Error> FormHandle<Model, Error> {
    /// Reinitializes from one rejected browser response, including deferred unparsable text.
    ///
    /// Replaces draft and baseline, retires prior async work, and records one rejected attempt.
    /// Does not invoke submit behavior or submission listeners. Existing mounted parsed bindings
    /// consume matching text immediately; otherwise the first matching mount consumes it once.
    /// Unmounted pending text is not a Parse Blocker. Reset, reinitialization, related writes,
    /// and item removal retire pending text. Collection reordering preserves logical addressing.
    pub fn restore_browser_rejection<Intent: 'static>(
        &self,
        values: Model,
        intent: Intent,
        map: impl FnOnce(
            &mut BrowserRejectionTargets<'_, Model, Error>,
        ) -> BrowserRejection<Model, Error>,
    ) {
        self.retire_adapter_lifecycle();
        let mut raw = BTreeMap::new();
        self.write_core(|core| {
            core.restore_browser_rejection(values, intent, |targets| {
                let rejection = map(targets);
                raw = rejection.raw;
                rejection.errors
            })
        });
        self.adapter.restore_raw_input(raw);
        self.notify_changed();
    }
}

impl<Model, Error> FormHandle<Model, Error> {
    /// Retires adapter work and input state before installing a new form lifecycle.
    pub(super) fn retire_adapter_lifecycle(&self) {
        self.adapter.cancel_validation_tasks();
        self.adapter.invalidate_managed_async_submission();
        self.clear_active_submit_intent();
        self.advance_submit_generation();
        self.adapter.clear_parse_errors();
        self.adapter.clear_file_selections();
    }
}
