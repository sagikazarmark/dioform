use std::{collections::BTreeMap, rc::Rc};

use super::{
    CollectionItemIdentity, CollectionState, FieldIdentity, FormValidationError,
    FormValidatorContext, ValidationErrorView, ValidationStatusView, ValidationTarget,
    ValidationTrigger, ValidationTriggers, ValidatorContext, ValidatorId, ValidatorSource,
    validation_lifecycle,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ValidatorKey {
    pub(super) field: FieldIdentity,
    pub(super) id: ValidatorId,
}

impl ValidatorKey {
    pub(super) fn new(field: FieldIdentity, id: ValidatorId) -> Self {
        Self { field, id }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct CollectionItemValidatorTemplateKey {
    pub(super) collection: FieldIdentity,
    pub(super) field: Rc<str>,
    pub(super) id: ValidatorId,
}

impl CollectionItemValidatorTemplateKey {
    pub(super) fn new(
        collection: FieldIdentity,
        field: impl Into<Rc<str>>,
        id: ValidatorId,
    ) -> Self {
        Self {
            collection,
            field: field.into(),
            id,
        }
    }
}

pub(super) fn collection_item_template_key_for_field(
    field: &FieldIdentity,
    id: ValidatorId,
) -> Option<CollectionItemValidatorTemplateKey> {
    field.collection_item_parts().map(|(collection, _, field)| {
        CollectionItemValidatorTemplateKey::new(FieldIdentity::new(collection), field, id)
    })
}

pub(super) type SyncFieldValidator<Model, Error> =
    dyn for<'a> Fn(&'a Model, ValidatorContext<'a, Model>) -> Vec<Error> + 'static;

pub(super) type SyncCollectionItemFieldValidator<Model, Error> = dyn for<'a> Fn(
        &'a Model,
        &'a CollectionState,
        CollectionItemIdentity,
        ValidatorContext<'a, Model>,
    ) -> Vec<Error>
    + 'static;

pub(super) type SyncFormValidator<Model, Error> =
    dyn for<'a> Fn(FormValidatorContext<'a, Model>) -> Vec<FormValidationError<Error>> + 'static;

pub(super) struct RegisteredFieldValidator<Model, Error> {
    pub(super) lifecycle: validation_lifecycle::SourceState<Error>,
    pub(super) validate: Option<Rc<SyncFieldValidator<Model, Error>>>,
    pub(super) model_dependent: bool,
}

pub(super) struct RegisteredCollectionItemFieldValidator<Model, Error> {
    pub(super) source: ValidatorSource,
    pub(super) triggers: ValidationTriggers,
    pub(super) collection_len: Rc<dyn Fn(&Model) -> usize + 'static>,
    pub(super) validate: Rc<SyncCollectionItemFieldValidator<Model, Error>>,
}

pub(super) struct RegisteredFormValidator<Model, Error> {
    pub(super) lifecycle: validation_lifecycle::SourceState<FormValidationError<Error>>,
    pub(super) validate: Option<Box<SyncFormValidator<Model, Error>>>,
}

pub(super) struct ValidationChainRegistry<Model, Error> {
    next_validator_id: u64,
    field_validators: BTreeMap<ValidatorKey, RegisteredFieldValidator<Model, Error>>,
    collection_item_field_validator_templates: BTreeMap<
        CollectionItemValidatorTemplateKey,
        RegisteredCollectionItemFieldValidator<Model, Error>,
    >,
    collection_item_field_validator_states:
        BTreeMap<ValidatorKey, validation_lifecycle::SourceState<Error>>,
    form_validators: BTreeMap<ValidatorId, RegisteredFormValidator<Model, Error>>,
}

impl<Model, Error> ValidationChainRegistry<Model, Error> {
    pub(super) fn new() -> Self {
        Self {
            next_validator_id: 0,
            field_validators: BTreeMap::new(),
            collection_item_field_validator_templates: BTreeMap::new(),
            collection_item_field_validator_states: BTreeMap::new(),
            form_validators: BTreeMap::new(),
        }
    }

    pub(super) fn allocate_validator_id(&mut self) -> ValidatorId {
        let id = ValidatorId(self.next_validator_id);
        self.next_validator_id = self
            .next_validator_id
            .checked_add(1)
            .expect("validator id counter exhausted");
        id
    }

    pub(super) const fn next_validator_id(&self) -> u64 {
        self.next_validator_id
    }

    pub(super) fn advance_next_validator_id_to_at_least(&mut self, next_validator_id: u64) {
        self.next_validator_id = self.next_validator_id.max(next_validator_id);
    }

    pub(super) fn insert_field_validator(
        &mut self,
        key: ValidatorKey,
        validator: RegisteredFieldValidator<Model, Error>,
    ) {
        self.field_validators.insert(key, validator);
    }

    pub(super) fn remove_field_validator(&mut self, key: &ValidatorKey) -> bool {
        self.field_validators.remove(key).is_some()
    }

    pub(super) fn field_validator(
        &self,
        key: &ValidatorKey,
    ) -> Option<&RegisteredFieldValidator<Model, Error>> {
        self.field_validators.get(key)
    }

    pub(super) fn field_validator_mut(
        &mut self,
        key: &ValidatorKey,
    ) -> Option<&mut RegisteredFieldValidator<Model, Error>> {
        self.field_validators.get_mut(key)
    }

    pub(super) fn field_entries(
        &self,
    ) -> impl Iterator<Item = (&ValidatorKey, &RegisteredFieldValidator<Model, Error>)> {
        self.field_validators.iter()
    }

    pub(super) fn field_values(
        &self,
    ) -> impl Iterator<Item = &RegisteredFieldValidator<Model, Error>> {
        self.field_validators.values()
    }

    pub(super) fn field_values_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut RegisteredFieldValidator<Model, Error>> {
        self.field_validators.values_mut()
    }

    pub(super) fn retain_field_validators(
        &mut self,
        mut retain: impl FnMut(&ValidatorKey) -> bool,
    ) {
        self.field_validators.retain(|key, _| retain(key));
    }

    pub(super) fn insert_collection_item_template(
        &mut self,
        key: CollectionItemValidatorTemplateKey,
        validator: RegisteredCollectionItemFieldValidator<Model, Error>,
    ) {
        self.collection_item_field_validator_templates
            .insert(key, validator);
    }

    pub(super) fn collection_item_template(
        &self,
        key: &CollectionItemValidatorTemplateKey,
    ) -> Option<&RegisteredCollectionItemFieldValidator<Model, Error>> {
        self.collection_item_field_validator_templates.get(key)
    }

    pub(super) fn collection_item_template_entries(
        &self,
    ) -> impl Iterator<
        Item = (
            &CollectionItemValidatorTemplateKey,
            &RegisteredCollectionItemFieldValidator<Model, Error>,
        ),
    > {
        self.collection_item_field_validator_templates.iter()
    }

    pub(super) fn collection_item_template_keys(
        &self,
    ) -> impl Iterator<Item = &CollectionItemValidatorTemplateKey> {
        self.collection_item_field_validator_templates.keys()
    }

    pub(super) fn collection_item_state(
        &self,
        key: &ValidatorKey,
    ) -> Option<&validation_lifecycle::SourceState<Error>> {
        self.collection_item_field_validator_states.get(key)
    }

    pub(super) fn collection_item_state_mut(
        &mut self,
        key: &ValidatorKey,
    ) -> Option<&mut validation_lifecycle::SourceState<Error>> {
        self.collection_item_field_validator_states.get_mut(key)
    }

    pub(super) fn collection_item_state_entries(
        &self,
    ) -> impl Iterator<Item = (&ValidatorKey, &validation_lifecycle::SourceState<Error>)> {
        self.collection_item_field_validator_states.iter()
    }

    pub(super) fn collection_item_state_values(
        &self,
    ) -> impl Iterator<Item = &validation_lifecycle::SourceState<Error>> {
        self.collection_item_field_validator_states.values()
    }

    pub(super) fn ensure_collection_item_state(
        &mut self,
        key: ValidatorKey,
        source: ValidatorSource,
        triggers: ValidationTriggers,
    ) {
        self.collection_item_field_validator_states
            .entry(key)
            .or_insert_with(|| {
                validation_lifecycle::SourceState::new(
                    source,
                    triggers,
                    validation_lifecycle::SourceKind::Sync,
                )
            });
    }

    pub(super) fn retain_collection_item_states(
        &mut self,
        mut retain: impl FnMut(&ValidatorKey) -> bool,
    ) {
        self.collection_item_field_validator_states
            .retain(|key, _| retain(key));
    }

    pub(super) fn insert_form_validator(
        &mut self,
        id: ValidatorId,
        validator: RegisteredFormValidator<Model, Error>,
    ) {
        self.form_validators.insert(id, validator);
    }

    pub(super) fn remove_form_validator(&mut self, id: ValidatorId) -> bool {
        self.form_validators.remove(&id).is_some()
    }

    pub(super) fn form_validator(
        &self,
        id: ValidatorId,
    ) -> Option<&RegisteredFormValidator<Model, Error>> {
        self.form_validators.get(&id)
    }

    pub(super) fn form_validator_mut(
        &mut self,
        id: ValidatorId,
    ) -> Option<&mut RegisteredFormValidator<Model, Error>> {
        self.form_validators.get_mut(&id)
    }

    pub(super) fn form_entries(
        &self,
    ) -> impl Iterator<Item = (&ValidatorId, &RegisteredFormValidator<Model, Error>)> {
        self.form_validators.iter()
    }

    pub(super) fn form_values(
        &self,
    ) -> impl Iterator<Item = &RegisteredFormValidator<Model, Error>> {
        self.form_validators.values()
    }

    /// Applies a per-source `check` to every field and collection-item source.
    ///
    /// Field and collection-item validators both carry `SourceState<Error>`, so they share this
    /// helper. Form validators carry `SourceState<FormValidationError<Error>>` and are checked
    /// separately by each caller's `form_values()` branch.
    fn any_field_or_collection_state_matches<Intent>(
        &self,
        intent: &Intent,
        check: impl Fn(&validation_lifecycle::SourceState<Error>, &Intent) -> bool,
    ) -> bool {
        self.field_values()
            .any(|validator| check(&validator.lifecycle, intent))
            || self
                .collection_item_state_values()
                .any(|state| check(state, intent))
    }

    /// Returns whether any source has submit-triggered errors that block submission for `intent`.
    pub(super) fn has_errors_blocking_submit<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.any_field_or_collection_state_matches(
            intent,
            validation_lifecycle::SourceState::errors_block_submit_intent,
        ) || self
            .form_values()
            .any(|validator| validator.lifecycle.errors_block_submit_intent(intent))
    }

    /// Returns whether any source has errors that count against known submit availability for `intent`.
    pub(super) fn has_known_errors_affecting_availability<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.any_field_or_collection_state_matches(
            intent,
            validation_lifecycle::SourceState::errors_affect_submit_availability,
        ) || self.form_values().any(|validator| {
            validator
                .lifecycle
                .errors_affect_submit_availability(intent)
        })
    }

    /// Returns whether any source has pending submit-triggered validation blocking `intent`.
    pub(super) fn has_pending_submit_validation<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.any_field_or_collection_state_matches(
            intent,
            validation_lifecycle::SourceState::pending_blocks_submit_intent,
        ) || self
            .form_values()
            .any(|validator| validator.lifecycle.pending_blocks_submit_intent(intent))
    }

    /// Returns whether any source has unresolved async submit validation for `intent`.
    pub(super) fn has_unresolved_submit_async<Intent>(&self, intent: &Intent) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.any_field_or_collection_state_matches(
            intent,
            validation_lifecycle::SourceState::unresolved_submit_async,
        ) || self
            .form_values()
            .any(|validator| validator.lifecycle.unresolved_submit_async(intent))
    }

    pub(super) fn form_values_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut RegisteredFormValidator<Model, Error>> {
        self.form_validators.values_mut()
    }

    pub(super) fn retain_form_errors(
        &mut self,
        mut retain: impl FnMut(&FormValidationError<Error>) -> bool,
    ) {
        for validator in self.form_validators.values_mut() {
            validator.lifecycle.retain_errors(|error| retain(error));
        }
    }

    pub(super) fn clear_collection_item_field_validator_states(&mut self) {
        self.collection_item_field_validator_states.clear();
    }

    pub(super) fn clear_results(&mut self) {
        for validator in self.field_validators.values_mut() {
            validator.lifecycle.clear();
        }

        for validator in self.collection_item_field_validator_states.values_mut() {
            validator.clear();
        }

        for validator in self.form_validators.values_mut() {
            validator.lifecycle.clear();
        }
    }

    /// Clears the results and pending state of every field validator attached to one field.
    ///
    /// Only field-scoped validators for `field` are cleared; form validators (even those that
    /// emit field-targeted errors) and other fields' validators are untouched.
    pub(super) fn clear_field_results(&mut self, field: &FieldIdentity) {
        for (key, validator) in self.field_validators.iter_mut() {
            if &key.field == field {
                validator.lifecycle.clear();
            }
        }
    }

    pub(super) fn sorted_field_entries(
        &self,
    ) -> Vec<(&ValidatorKey, &RegisteredFieldValidator<Model, Error>)> {
        let mut entries: Vec<_> = self.field_validators.iter().collect();

        entries.sort_by(|(left_key, left), (right_key, right)| {
            left_key
                .id
                .cmp(&right_key.id)
                .then_with(|| left_key.field.cmp(&right_key.field))
                .then_with(|| left.lifecycle.source().cmp(right.lifecycle.source()))
        });

        entries
    }

    pub(super) fn sorted_collection_item_entries(
        &self,
    ) -> Vec<(&ValidatorKey, &validation_lifecycle::SourceState<Error>)> {
        let mut entries: Vec<_> = self.collection_item_field_validator_states.iter().collect();

        entries.sort_by(|(left_key, left), (right_key, right)| {
            left_key
                .id
                .cmp(&right_key.id)
                .then_with(|| left_key.field.cmp(&right_key.field))
                .then_with(|| left.source().cmp(right.source()))
        });

        entries
    }

    pub(super) fn sorted_form_entries(
        &self,
    ) -> Vec<(&ValidatorId, &RegisteredFormValidator<Model, Error>)> {
        let mut entries: Vec<_> = self.form_validators.iter().collect();

        entries.sort_by(|(left_id, left), (right_id, right)| {
            left_id
                .cmp(right_id)
                .then_with(|| left.lifecycle.source().cmp(right.lifecycle.source()))
        });

        entries
    }

    pub(super) fn field_validator_key_for_source(
        &self,
        field: FieldIdentity,
        source: &ValidatorSource,
    ) -> Option<ValidatorKey> {
        self.sorted_field_entries()
            .into_iter()
            .find(|(key, validator)| key.field == field && validator.lifecycle.source() == source)
            .map(|(key, _)| key.clone())
    }

    pub(super) fn form_validator_id_for_source(
        &self,
        source: &ValidatorSource,
    ) -> Option<ValidatorId> {
        self.sorted_form_entries()
            .into_iter()
            .find(|(_, validator)| validator.lifecycle.source() == source)
            .map(|(id, _)| *id)
    }

    pub(super) fn fields_for_trigger(&self, trigger: ValidationTrigger) -> Vec<FieldIdentity> {
        let mut fields = Vec::new();

        for (key, validator) in self.sorted_field_entries() {
            if validator.lifecycle.should_run(trigger) && !fields.contains(&key.field) {
                fields.push(key.field.clone());
            }
        }

        for (key, validator) in self.sorted_collection_item_entries() {
            if validator.should_run(trigger) && !fields.contains(&key.field) {
                fields.push(key.field.clone());
            }
        }

        fields
    }

    pub(super) fn sync_field_keys_for_chain(
        &self,
        field: &FieldIdentity,
        trigger: ValidationTrigger,
    ) -> Vec<ValidatorKey> {
        self.sorted_field_entries()
            .into_iter()
            .filter(|(key, validator)| {
                key.field == *field
                    && validator.lifecycle.is_sync()
                    && validator.lifecycle.should_run(trigger)
            })
            .map(|(key, _)| key.clone())
            .collect()
    }

    pub(super) fn sync_collection_item_keys_for_chain(
        &self,
        field: &FieldIdentity,
        trigger: ValidationTrigger,
    ) -> Vec<ValidatorKey> {
        self.sorted_collection_item_entries()
            .into_iter()
            .filter(|(key, validator)| {
                key.field == *field && validator.is_sync() && validator.should_run(trigger)
            })
            .map(|(key, _)| key.clone())
            .collect()
    }

    pub(super) fn async_field_keys_to_skip(
        &self,
        field: &FieldIdentity,
        trigger: ValidationTrigger,
    ) -> Vec<ValidatorKey> {
        self.sorted_field_entries()
            .into_iter()
            .filter(|(key, validator)| {
                key.field == *field
                    && validator
                        .lifecycle
                        .should_skip_async_after_sync_failure(trigger)
            })
            .map(|(key, _)| key.clone())
            .collect()
    }

    pub(super) fn skipped_async_field_keys_to_clear(
        &self,
        field: &FieldIdentity,
        trigger: ValidationTrigger,
    ) -> Vec<ValidatorKey> {
        self.sorted_field_entries()
            .into_iter()
            .filter(|(key, validator)| {
                key.field == *field
                    && validator
                        .lifecycle
                        .should_clear_async_skip_after_sync_success(trigger)
            })
            .map(|(key, _)| key.clone())
            .collect()
    }

    pub(super) fn sync_form_ids_for_chain(&self, trigger: ValidationTrigger) -> Vec<ValidatorId> {
        self.sorted_form_entries()
            .into_iter()
            .filter(|(_, validator)| {
                validator.lifecycle.is_sync() && validator.lifecycle.should_run(trigger)
            })
            .map(|(id, _)| *id)
            .collect()
    }

    pub(super) fn async_form_ids_to_skip(&self, trigger: ValidationTrigger) -> Vec<ValidatorId> {
        self.sorted_form_entries()
            .into_iter()
            .filter(|(_, validator)| {
                validator
                    .lifecycle
                    .should_skip_async_after_sync_failure(trigger)
            })
            .map(|(id, _)| *id)
            .collect()
    }

    pub(super) fn skipped_async_form_ids_to_clear(
        &self,
        trigger: ValidationTrigger,
    ) -> Vec<ValidatorId> {
        self.sorted_form_entries()
            .into_iter()
            .filter(|(_, validator)| {
                validator
                    .lifecycle
                    .should_clear_async_skip_after_sync_success(trigger)
            })
            .map(|(id, _)| *id)
            .collect()
    }

    pub(super) fn unresolved_submit_field_async_keys<Intent>(
        &self,
        intent: &Intent,
    ) -> Vec<ValidatorKey>
    where
        Intent: PartialEq + 'static,
    {
        self.sorted_field_entries()
            .into_iter()
            .filter(|(_, validator)| {
                validator.lifecycle.is_async()
                    && validator.lifecycle.should_run(ValidationTrigger::Submit)
                    && validator
                        .lifecycle
                        .async_status_needs_run_for_submit_intent(intent)
            })
            .map(|(key, _)| key.clone())
            .collect()
    }

    pub(super) fn unresolved_submit_form_async_ids<Intent>(
        &self,
        intent: &Intent,
    ) -> Vec<ValidatorId>
    where
        Intent: PartialEq + 'static,
    {
        self.sorted_form_entries()
            .into_iter()
            .filter(|(_, validator)| {
                validator.lifecycle.is_async()
                    && validator.lifecycle.should_run(ValidationTrigger::Submit)
                    && validator
                        .lifecycle
                        .async_status_needs_run_for_submit_intent(intent)
            })
            .map(|(id, _)| *id)
            .collect()
    }

    pub(super) fn validation_statuses(&self) -> Vec<ValidationStatusView> {
        let mut statuses: Vec<_> = self
            .sorted_field_entries()
            .into_iter()
            .map(|(key, validator)| ValidationStatusView {
                target: ValidationTarget::Field(key.field.clone()),
                validator_id: key.id,
                source: validator.lifecycle.source().clone(),
                status: validator.lifecycle.status(),
            })
            .collect();

        statuses.extend(self.sorted_collection_item_entries().into_iter().map(
            |(key, validator)| ValidationStatusView {
                target: ValidationTarget::Field(key.field.clone()),
                validator_id: key.id,
                source: validator.source().clone(),
                status: validator.status(),
            },
        ));

        statuses.extend(
            self.sorted_form_entries()
                .into_iter()
                .map(|(id, validator)| ValidationStatusView {
                    target: ValidationTarget::Form,
                    validator_id: *id,
                    source: validator.lifecycle.source().clone(),
                    status: validator.lifecycle.status(),
                }),
        );

        statuses
    }

    pub(super) fn field_validation_statuses(
        &self,
        field: &FieldIdentity,
    ) -> Vec<ValidationStatusView> {
        self.sorted_field_entries()
            .into_iter()
            .filter(|(key, _)| key.field == *field)
            .map(|(key, validator)| ValidationStatusView {
                target: ValidationTarget::Field(key.field.clone()),
                validator_id: key.id,
                source: validator.lifecycle.source().clone(),
                status: validator.lifecycle.status(),
            })
            .collect()
    }

    pub(super) fn field_identity_validation_statuses(
        &self,
        field: &FieldIdentity,
    ) -> Vec<ValidationStatusView> {
        let mut statuses = self.field_validation_statuses(field);

        statuses.extend(
            self.sorted_collection_item_entries()
                .into_iter()
                .filter(|(key, _)| key.field == *field)
                .map(|(key, validator)| ValidationStatusView {
                    target: ValidationTarget::Field(key.field.clone()),
                    validator_id: key.id,
                    source: validator.source().clone(),
                    status: validator.status(),
                }),
        );

        statuses
    }

    pub(super) fn form_validation_statuses(&self) -> Vec<ValidationStatusView> {
        self.sorted_form_entries()
            .into_iter()
            .map(|(id, validator)| ValidationStatusView {
                target: ValidationTarget::Form,
                validator_id: *id,
                source: validator.lifecycle.source().clone(),
                status: validator.lifecycle.status(),
            })
            .collect()
    }

    pub(super) fn append_validation_errors_matching<'a>(
        &'a self,
        errors: &mut Vec<ValidationErrorView<'a, Error>>,
        include: &impl Fn(&ValidationTarget) -> bool,
    ) {
        for (key, validator) in self.sorted_field_entries() {
            let target = ValidationTarget::Field(key.field.clone());

            if !include(&target) {
                continue;
            }

            for error in validator.lifecycle.errors() {
                errors.push(ValidationErrorView {
                    target: target.clone(),
                    source: validator.lifecycle.source(),
                    validator_id: Some(key.id),
                    error,
                });
            }
        }

        for (key, validator) in self.sorted_collection_item_entries() {
            let target = ValidationTarget::Field(key.field.clone());

            if !include(&target) {
                continue;
            }

            for error in validator.errors() {
                errors.push(ValidationErrorView {
                    target: target.clone(),
                    source: validator.source(),
                    validator_id: Some(key.id),
                    error,
                });
            }
        }

        for (id, validator) in self.sorted_form_entries() {
            for error in validator.lifecycle.errors() {
                if !include(&error.target) {
                    continue;
                }

                errors.push(ValidationErrorView {
                    target: error.target.clone(),
                    source: validator.lifecycle.source(),
                    validator_id: Some(*id),
                    error: &error.error,
                });
            }
        }
    }

    pub(super) fn append_validation_errors_matching_for_submit_intent<'a, Intent>(
        &'a self,
        errors: &mut Vec<ValidationErrorView<'a, Error>>,
        intent: &Intent,
        include: &impl Fn(&ValidationTarget) -> bool,
    ) where
        Intent: PartialEq + 'static,
    {
        for (key, validator) in self.sorted_field_entries() {
            let target = ValidationTarget::Field(key.field.clone());

            if !include(&target) || !validator.lifecycle.matches_submit_intent(intent) {
                continue;
            }

            for error in validator.lifecycle.errors() {
                errors.push(ValidationErrorView {
                    target: target.clone(),
                    source: validator.lifecycle.source(),
                    validator_id: Some(key.id),
                    error,
                });
            }
        }

        for (key, validator) in self.sorted_collection_item_entries() {
            let target = ValidationTarget::Field(key.field.clone());

            if !include(&target) || !validator.matches_submit_intent(intent) {
                continue;
            }

            for error in validator.errors() {
                errors.push(ValidationErrorView {
                    target: target.clone(),
                    source: validator.source(),
                    validator_id: Some(key.id),
                    error,
                });
            }
        }

        for (id, validator) in self.sorted_form_entries() {
            if !validator.lifecycle.matches_submit_intent(intent) {
                continue;
            }

            for error in validator.lifecycle.errors() {
                if !include(&error.target) {
                    continue;
                }

                errors.push(ValidationErrorView {
                    target: error.target.clone(),
                    source: validator.lifecycle.source(),
                    validator_id: Some(*id),
                    error: &error.error,
                });
            }
        }
    }
}
