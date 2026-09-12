//! Adapter-owned **Raw Input State**: **Parse Errors** and **File Selection**.
//!
//! These are adapter concerns, separate from the async-validation runtime. `CONTEXT.md` places
//! **Raw Input State** and **Parse Errors** in the **Dioxus Adapter**, and **File Selection** stays
//! adapter-owned per ADR-0008. They previously lived as loose fields on the async `AdapterState`
//! purely for a shared `RefCell` home; concentrating each behind its own type keeps the runtime
//! state cohesive and gives each concern one place to find. Each type owns its own interior
//! mutability so the [`AdapterRuntime`](crate::adapter_runtime::AdapterRuntime) facade can hold it
//! behind an `Rc` and share it across `FormHandle` clones without a second `RefCell` layer.

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use dioform_core::__private::{CollectionItemFieldAddress, FieldAncestry};

use crate::{CollectionItemIdentity, FieldIdentity, ParseError, SelectedFile};

/// Opaque identity for one mounted parse binding.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ParseBindingId(u64);

/// One mounted parse binding: the field it currently addresses and the blocker it holds for it.
///
/// The field is the mount's address rather than a fixed key. A scope that renders a different
/// collection item re-addresses its entry through
/// [`re_address_parse_binding`](ParseState::re_address_parse_binding) (ADR-0026), which is why the
/// field-scoped queries, the collection item sweep, and unregistration all read it live.
struct ParseBindingState {
    field: FieldIdentity,
    parse_error: Option<ParseError>,
    parser: Rc<RestoredTextParser>,
    restoration: Option<Rc<()>>,
}

pub(super) type RestoredTextParser = dyn Fn(&str) -> Option<String>;

/// Owned callback work whose result is valid only while its binding retains the same token.
struct RestoredParse {
    id: ParseBindingId,
    field: FieldIdentity,
    raw_value: String,
    parser: Rc<RestoredTextParser>,
    token: Rc<()>,
}

/// Owns the adapter's mounted **Parse Errors**, keyed by parse binding.
#[derive(Default)]
pub(super) struct ParseState {
    next_id: Cell<u64>,
    bindings: RefCell<BTreeMap<ParseBindingId, ParseBindingState>>,
    restored: RefCell<BTreeMap<FieldIdentity, String>>,
}

impl ParseState {
    /// Mounts a parse binding for a field and returns its identity.
    pub(super) fn register_parse_binding(
        &self,
        field: FieldIdentity,
        parser: Rc<RestoredTextParser>,
    ) -> ParseBindingId {
        let id = ParseBindingId(self.next_id.get());
        self.next_id.set(self.next_id.get() + 1);
        self.bindings.borrow_mut().insert(
            id,
            ParseBindingState {
                parse_error: None,
                field,
                parser,
                restoration: None,
            },
        );
        self.apply_restored(id);
        id
    }

    /// Re-addresses a parse binding to the field its mount now renders, dropping the raw text and
    /// parse error it held for the previous field.
    ///
    /// Returns the previous field when a parse error was cleared as a result, so the caller can
    /// notify that field's parse selector: a blocker must never outlive the address it was raised
    /// for. Re-addressing a binding whose entry was swept away by a collection item removal
    /// registers it again under the same identity, so a mount that outlived the row it used to
    /// render is not left mute.
    pub(super) fn re_address_parse_binding(
        &self,
        id: ParseBindingId,
        field: FieldIdentity,
        parser: Rc<RestoredTextParser>,
    ) -> Option<FieldIdentity> {
        let mut bindings = self.bindings.borrow_mut();

        if bindings
            .get(&id)
            .is_some_and(|binding| binding.field == field)
        {
            return None;
        }

        let previous = bindings.insert(
            id,
            ParseBindingState {
                field,
                parser,
                parse_error: None,
                restoration: None,
            },
        );
        let cleared = previous.and_then(|binding| binding.parse_error.map(|_| binding.field));
        drop(bindings);
        self.apply_restored(id);
        cleared
    }

    fn prepare_restored(
        &self,
        id: ParseBindingId,
        binding: &mut ParseBindingState,
    ) -> Option<RestoredParse> {
        let raw_value = self.restored.borrow_mut().remove(&binding.field)?;
        let token = Rc::new(());
        binding.restoration = Some(token.clone());
        Some(RestoredParse {
            id,
            field: binding.field.clone(),
            raw_value,
            parser: binding.parser.clone(),
            token,
        })
    }

    fn apply_restored(&self, id: ParseBindingId) {
        let work = self
            .bindings
            .borrow_mut()
            .get_mut(&id)
            .and_then(|binding| self.prepare_restored(id, binding));
        if let Some(work) = work {
            self.run_restored(work);
        }
    }

    fn run_restored(&self, work: RestoredParse) {
        let is_current = |binding: &ParseBindingState| {
            binding.field == work.field
                && binding
                    .restoration
                    .as_ref()
                    .is_some_and(|token| Rc::ptr_eq(token, &work.token))
        };
        if !self.bindings.borrow().get(&work.id).is_some_and(is_current) {
            return;
        }
        // Application code may read or change parse state. Hold no borrow across this call.
        let message = (work.parser)(&work.raw_value);
        let mut bindings = self.bindings.borrow_mut();
        if let Some(binding) = bindings
            .get_mut(&work.id)
            .filter(|binding| is_current(binding))
        {
            binding.restoration = None;
            binding.parse_error = message.map(|message| ParseError {
                field: work.field,
                raw_value: work.raw_value,
                message,
            });
        }
    }

    pub(super) fn restore_raw_input(&self, raw: BTreeMap<FieldIdentity, String>) {
        *self.restored.borrow_mut() = raw;
        let work: Vec<_> = self
            .bindings
            .borrow_mut()
            .iter_mut()
            .filter_map(|(&id, binding)| {
                binding.parse_error = None;
                binding.restoration = None;
                self.prepare_restored(id, binding)
            })
            .collect();
        for work in work {
            self.run_restored(work);
        }
    }

    fn retire_pending_restorations(&self, matches: impl Fn(&FieldIdentity) -> bool) {
        for binding in self.bindings.borrow_mut().values_mut() {
            if matches(&binding.field) {
                binding.restoration = None;
            }
        }
    }

    pub(super) fn retire_restored_raw_input(&self, field: &FieldIdentity) {
        // Raw text describes a value, including a collection's contents. Unlike value-reader
        // notifications, retirement must cross the collection/item boundary in both directions.
        let reaches = |target: &FieldIdentity| {
            FieldAncestry::contains(target, field) || FieldAncestry::contains(field, target)
        };
        self.retire_pending_restorations(reaches);
        self.restored
            .borrow_mut()
            .retain(|target, _| !reaches(target));
    }

    fn retire_containing_raw_input(&self, field: &FieldIdentity) {
        self.retire_pending_restorations(|target| FieldAncestry::contains(target, field));
        self.restored
            .borrow_mut()
            .retain(|target, _| !FieldAncestry::contains(target, field));
    }

    /// Retires deferred text on core value transitions, including advanced writes.
    pub(super) fn on_core_transition(&self, event: &dioform_core::FormObserverEvent) {
        use dioform_core::FormObserverEvent;
        match event {
            FormObserverEvent::FieldUpdated { field, .. } => {
                self.retire_restored_raw_input(&field.identity());
            }
            FormObserverEvent::FieldReset { field, .. } => {
                self.retire_restored_raw_input(&field.identity());
                self.retire_collection_raw_input(&field.identity());
            }
            FormObserverEvent::CollectionItemInserted { collection, .. }
            | FormObserverEvent::CollectionItemMoved { collection, .. }
            | FormObserverEvent::CollectionItemsSwapped { collection, .. }
            | FormObserverEvent::CollectionItemsReordered { collection, .. } => {
                // Structure changes replace containing values, but logical items retain their
                // own response text when only their position changes.
                self.retire_containing_raw_input(collection);
            }
            FormObserverEvent::CollectionItemRemoved {
                collection, item, ..
            }
            | FormObserverEvent::CollectionItemReplaced {
                collection, item, ..
            } => {
                self.retire_containing_raw_input(collection);
                self.retire_pending_restorations(|field| {
                    CollectionItemFieldAddress::matches_item(field, collection, *item)
                });
                self.restored.borrow_mut().retain(|field, _| {
                    !CollectionItemFieldAddress::matches_item(field, collection, *item)
                });
            }
            FormObserverEvent::CollectionCleared { collection, .. }
            | FormObserverEvent::CollectionReplaced { collection, .. } => {
                self.retire_containing_raw_input(collection);
                self.retire_collection_raw_input(collection);
            }
            FormObserverEvent::Reset { .. } | FormObserverEvent::Reinitialized { .. } => {
                self.retire_pending_restorations(|_| true);
                self.restored.borrow_mut().clear();
            }
            _ => {}
        }
    }

    fn retire_collection_raw_input(&self, collection: &FieldIdentity) {
        self.retire_pending_restorations(|field| {
            CollectionItemFieldAddress::matches_collection(field, collection)
        });
        self.restored
            .borrow_mut()
            .retain(|field, _| !CollectionItemFieldAddress::matches_collection(field, collection));
    }

    pub(super) fn reset_restored_input(&self, field: Option<&FieldIdentity>) {
        if let Some(field) = field {
            self.retire_restored_raw_input(field);
            self.retire_collection_raw_input(field);
        } else {
            self.retire_pending_restorations(|_| true);
            self.restored.borrow_mut().clear();
        }
    }

    /// Returns whether one parse binding currently addresses a field.
    ///
    /// This is how a caller proves it still belongs to the mount it shares a registration with: a
    /// binding clone retained past a re-addressing keeps its own address and stops matching. A
    /// binding whose entry was swept away addresses nothing.
    pub(super) fn parse_binding_addresses(
        &self,
        id: ParseBindingId,
        field: &FieldIdentity,
    ) -> bool {
        self.bindings
            .borrow()
            .get(&id)
            .is_some_and(|binding| &binding.field == field)
    }

    /// Removes a parse binding, returning the field it addressed when it held a parse error.
    pub(super) fn unregister_parse_binding(&self, id: ParseBindingId) -> Option<FieldIdentity> {
        let binding = self.bindings.borrow_mut().remove(&id)?;

        binding.parse_error.is_some().then_some(binding.field)
    }

    /// Removes all parse bindings addressed to one collection item, returning the fields whose
    /// parse errors were cleared as a result.
    pub(super) fn unregister_collection_item_parse_bindings(
        &self,
        collection: FieldIdentity,
        item: CollectionItemIdentity,
    ) -> Vec<FieldIdentity> {
        let mut changed_fields = Vec::new();

        self.restored
            .borrow_mut()
            .retain(|field, _| !CollectionItemFieldAddress::matches_item(field, &collection, item));

        self.bindings.borrow_mut().retain(|_, binding| {
            let remove =
                CollectionItemFieldAddress::matches_item(&binding.field, &collection, item);

            if remove {
                if binding.parse_error.is_some() {
                    changed_fields.push(binding.field.clone());
                }

                false
            } else {
                true
            }
        });

        changed_fields
    }

    /// Records a parse error for one binding.
    pub(super) fn set_parse_error(&self, id: ParseBindingId, raw_value: String, message: String) {
        let mut bindings = self.bindings.borrow_mut();
        let Some(binding) = bindings.get_mut(&id) else {
            return;
        };

        binding.restoration = None;
        binding.parse_error = Some(ParseError {
            field: binding.field.clone(),
            raw_value,
            message,
        });
    }

    /// Clears the parse error for one binding.
    pub(super) fn clear_parse_error(&self, id: ParseBindingId) {
        if let Some(binding) = self.bindings.borrow_mut().get_mut(&id) {
            binding.restoration = None;
            binding.parse_error = None;
        }
    }

    /// Clears every mounted parse error.
    pub(super) fn clear_parse_errors(&self) {
        self.restored.borrow_mut().clear();
        for binding in self.bindings.borrow_mut().values_mut() {
            binding.restoration = None;
            binding.parse_error = None;
        }
    }

    /// Clears mounted parse errors for one field, returning whether any error was cleared.
    pub(super) fn clear_field_parse_errors(&self, field: &FieldIdentity) -> bool {
        let mut cleared = false;
        for binding in self.bindings.borrow_mut().values_mut() {
            if &binding.field == field {
                binding.restoration = None;
            }
            if &binding.field == field && binding.parse_error.is_some() {
                binding.parse_error = None;
                cleared = true;
            }
        }
        cleared
    }

    /// Clears parse errors for mounted fields belonging to one collection while retaining their
    /// bindings, returning each field whose rendered raw state changed.
    pub(super) fn clear_collection_item_parse_errors(
        &self,
        collection: &FieldIdentity,
    ) -> Vec<FieldIdentity> {
        self.retire_collection_raw_input(collection);
        let mut changed_fields = BTreeSet::new();
        for binding in self.bindings.borrow_mut().values_mut() {
            if CollectionItemFieldAddress::matches_collection(&binding.field, collection)
                && binding.parse_error.is_some()
            {
                binding.parse_error = None;
                changed_fields.insert(binding.field.clone());
            }
        }
        changed_fields.into_iter().collect()
    }

    /// Returns the parse error for one binding, if any.
    pub(super) fn parse_error(&self, id: ParseBindingId) -> Option<ParseError> {
        self.bindings
            .borrow()
            .get(&id)
            .and_then(|binding| binding.parse_error.clone())
    }

    /// Returns every mounted parse error.
    pub(super) fn parse_errors(&self) -> Vec<ParseError> {
        self.bindings
            .borrow()
            .values()
            .filter_map(|binding| binding.parse_error.clone())
            .collect()
    }

    /// Returns the parse errors mounted for one field.
    pub(super) fn field_parse_errors(&self, field: FieldIdentity) -> Vec<ParseError> {
        self.bindings
            .borrow()
            .values()
            .filter(|binding| binding.field == field)
            .filter_map(|binding| binding.parse_error.clone())
            .collect()
    }

    /// Returns whether one field currently has a mounted parse error.
    pub(super) fn has_field_parse_errors(&self, field: FieldIdentity) -> bool {
        self.bindings
            .borrow()
            .values()
            .any(|binding| binding.field == field && binding.parse_error.is_some())
    }

    /// Returns whether any mounted parse error currently blocks submission.
    pub(super) fn has_parse_blockers(&self) -> bool {
        self.bindings
            .borrow()
            .values()
            .any(|binding| binding.parse_error.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readdressed_and_reregistered_parsers_can_read_parse_state() {
        for remove in [false, true] {
            let state = Rc::new(ParseState::default());
            let old = FieldIdentity::new("old");
            let target = FieldIdentity::new("target");
            let id = state.register_parse_binding(old, Rc::new(|_| None));
            if remove {
                state.unregister_parse_binding(id);
            }
            state.restore_raw_input(BTreeMap::from([(target.clone(), "raw".into())]));
            let weak = Rc::downgrade(&state);
            state.re_address_parse_binding(
                id,
                target.clone(),
                Rc::new(move |_| {
                    assert!(weak.upgrade().unwrap().parse_errors().is_empty());
                    Some("invalid".into())
                }),
            );
            let error = state.parse_error(id).unwrap();
            assert_eq!(error.field, target);
            assert_eq!(error.raw_value, "raw");
        }
    }

    #[test]
    fn restored_result_cannot_survive_readdressing_away_and_back() {
        let state = Rc::new(ParseState::default());
        let target = FieldIdentity::new("target");
        let other = FieldIdentity::new("other");
        let id = state.register_parse_binding(other.clone(), Rc::new(|_| None));
        state.restore_raw_input(BTreeMap::from([(target.clone(), "old raw".into())]));
        let weak = Rc::downgrade(&state);
        let target_again = target.clone();
        state.re_address_parse_binding(
            id,
            target.clone(),
            Rc::new(move |_| {
                let state = weak.upgrade().unwrap();
                state.re_address_parse_binding(id, other.clone(), Rc::new(|_| None));
                state.re_address_parse_binding(id, target_again.clone(), Rc::new(|_| None));
                Some("old error".into())
            }),
        );
        assert!(state.parse_binding_addresses(id, &target));
        assert!(state.parse_errors().is_empty());
    }

    #[test]
    fn restored_result_cannot_overwrite_new_input_at_the_same_address() {
        let state = Rc::new(ParseState::default());
        let target = FieldIdentity::new("target");
        let id = state.register_parse_binding(FieldIdentity::new("old"), Rc::new(|_| None));
        state.restore_raw_input(BTreeMap::from([(target.clone(), "old raw".into())]));
        let weak = Rc::downgrade(&state);
        state.re_address_parse_binding(
            id,
            target.clone(),
            Rc::new(move |_| {
                weak.upgrade()
                    .unwrap()
                    .set_parse_error(id, "new raw".into(), "new error".into());
                Some("old error".into())
            }),
        );
        let error = state.parse_error(id).unwrap();
        assert_eq!(error.field, target);
        assert_eq!(error.raw_value, "new raw");
        assert_eq!(error.message, "new error");
    }

    #[test]
    fn restored_result_cannot_resurrect_an_unregistered_binding() {
        let state = Rc::new(ParseState::default());
        let id_slot = Rc::new(Cell::new(None));
        let callback_id = id_slot.clone();
        let weak = Rc::downgrade(&state);
        let field = FieldIdentity::new("field");
        let id = state.register_parse_binding(
            field.clone(),
            Rc::new(move |_| {
                weak.upgrade()
                    .unwrap()
                    .unregister_parse_binding(callback_id.get().unwrap());
                Some("invalid".into())
            }),
        );
        id_slot.set(Some(id));
        state.restore_raw_input(BTreeMap::from([(field.clone(), "raw".into())]));
        assert!(!state.parse_binding_addresses(id, &field));
        assert!(state.parse_errors().is_empty());
    }
}

/// Owns the adapter's **File Selection** state, keyed by field.
#[derive(Default)]
pub(super) struct FileSelections {
    selections: RefCell<BTreeMap<FieldIdentity, Vec<SelectedFile>>>,
}

impl FileSelections {
    /// Replaces the selected files for one field.
    pub(super) fn set_file_selection(&self, field: FieldIdentity, files: Vec<SelectedFile>) {
        self.selections.borrow_mut().insert(field, files);
    }

    /// Clears every field's file selection.
    pub(super) fn clear_file_selections(&self) {
        self.selections.borrow_mut().clear();
    }

    /// Returns an owned snapshot of every field's file selection.
    pub(super) fn file_selection_snapshot(&self) -> BTreeMap<FieldIdentity, Vec<SelectedFile>> {
        self.selections.borrow().clone()
    }

    /// Returns the selected files for one field.
    pub(super) fn file_selection(&self, field: FieldIdentity) -> Vec<SelectedFile> {
        self.selections
            .borrow()
            .get(&field)
            .cloned()
            .unwrap_or_default()
    }
}
