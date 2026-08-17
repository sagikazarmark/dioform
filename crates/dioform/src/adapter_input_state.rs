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
    collections::BTreeMap,
    mem,
};

use dioform_core::__private::CollectionItemFieldAddress;

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
}

/// Owns the adapter's mounted **Parse Errors**, keyed by parse binding.
#[derive(Default)]
pub(super) struct ParseState {
    next_id: Cell<u64>,
    bindings: RefCell<BTreeMap<ParseBindingId, ParseBindingState>>,
}

impl ParseState {
    /// Mounts a parse binding for a field and returns its identity.
    pub(super) fn register_parse_binding(&self, field: FieldIdentity) -> ParseBindingId {
        let id = ParseBindingId(self.next_id.get());
        self.next_id.set(self.next_id.get() + 1);
        self.bindings.borrow_mut().insert(
            id,
            ParseBindingState {
                field,
                parse_error: None,
            },
        );
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
    ) -> Option<FieldIdentity> {
        let mut bindings = self.bindings.borrow_mut();

        let Some(binding) = bindings.get_mut(&id) else {
            bindings.insert(
                id,
                ParseBindingState {
                    field,
                    parse_error: None,
                },
            );

            return None;
        };

        if binding.field == field {
            return None;
        }

        let previous = mem::replace(&mut binding.field, field);

        binding.parse_error.take().map(|_| previous)
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

        binding.parse_error = Some(ParseError {
            field: binding.field.clone(),
            raw_value,
            message,
        });
    }

    /// Clears the parse error for one binding.
    pub(super) fn clear_parse_error(&self, id: ParseBindingId) {
        if let Some(binding) = self.bindings.borrow_mut().get_mut(&id) {
            binding.parse_error = None;
        }
    }

    /// Clears every mounted parse error.
    pub(super) fn clear_parse_errors(&self) {
        for binding in self.bindings.borrow_mut().values_mut() {
            binding.parse_error = None;
        }
    }

    /// Clears mounted parse errors for one field, returning whether any error was cleared.
    pub(super) fn clear_field_parse_errors(&self, field: &FieldIdentity) -> bool {
        let mut cleared = false;
        for binding in self.bindings.borrow_mut().values_mut() {
            if &binding.field == field && binding.parse_error.is_some() {
                binding.parse_error = None;
                cleared = true;
            }
        }
        cleared
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
