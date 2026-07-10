//! Single owner of per-field state keyed by [`FieldIdentity`].
//!
//! The **Form Core** previously held field version counters, interaction metadata, and collection
//! identity state in three parallel `BTreeMap`s, kept aligned by convention across every method.
//! This module concentrates them behind one interface so that lazy **Field Registration** (absent
//! fields read as version `0` and default metadata without allocating) and the coordinated
//! lifecycle (clear, retain, snapshot, and restore always touch the three maps together) become
//! encapsulated invariants rather than caller discipline.
//!
//! Version has exactly one owner here. Downstream submission and validation logic reads
//! [`FieldStore::version`] for staleness; it never writes versions.

use std::collections::BTreeMap;

use crate::{CollectionState, FieldIdentity, FieldMetadata};

/// Owns the field-keyed state of one **Form Core**: version counters, interaction metadata, and
/// collection identity state.
#[derive(Default)]
pub(crate) struct FieldStore {
    versions: BTreeMap<FieldIdentity, u64>,
    metadata: BTreeMap<FieldIdentity, FieldMetadata>,
    collections: BTreeMap<FieldIdentity, CollectionState>,
}

impl FieldStore {
    // --- versions (single owner; lazy read) ---

    /// Returns the current version of a field, or `0` for a field that has never been touched.
    pub(crate) fn version(&self, field: &FieldIdentity) -> u64 {
        self.versions.get(field).copied().unwrap_or_default()
    }

    /// Increments a field's version, materializing the field on first write.
    pub(crate) fn increment_version(&mut self, field: &FieldIdentity) {
        let version = self.versions.entry(field.clone()).or_default();
        *version = version
            .checked_add(1)
            .expect("field version counter exhausted");
    }

    /// Borrows the version map for comparison against a submit-validation snapshot.
    pub(crate) fn versions(&self) -> &BTreeMap<FieldIdentity, u64> {
        &self.versions
    }

    /// Clones the version map for capture into a submission or validation snapshot.
    pub(crate) fn versions_cloned(&self) -> BTreeMap<FieldIdentity, u64> {
        self.versions.clone()
    }

    // --- metadata (lazy read) ---

    /// Returns interaction metadata for a field, or the default for an unregistered field.
    pub(crate) fn metadata(&self, field: &FieldIdentity) -> FieldMetadata {
        self.metadata.get(field).copied().unwrap_or_default()
    }

    /// Borrows a field's metadata mutably, materializing the field on first write.
    pub(crate) fn metadata_mut(&mut self, field: &FieldIdentity) -> &mut FieldMetadata {
        self.metadata.entry(field.clone()).or_default()
    }

    // --- collections (state stored opaquely; mutation logic stays in Form Core) ---

    /// Borrows the collection state for a collection field, if it has been registered.
    pub(crate) fn collection(&self, field: &FieldIdentity) -> Option<&CollectionState> {
        self.collections.get(field)
    }

    /// Borrows the collection state for a collection field mutably, if it has been registered.
    pub(crate) fn collection_mut(&mut self, field: &FieldIdentity) -> Option<&mut CollectionState> {
        self.collections.get_mut(field)
    }

    /// Borrows the collection state for a collection field, inserting a fresh state on first use.
    pub(crate) fn collection_or_insert_with(
        &mut self,
        field: FieldIdentity,
        new_state: impl FnOnce() -> CollectionState,
    ) -> &mut CollectionState {
        self.collections.entry(field).or_insert_with(new_state)
    }

    /// Returns the identities of all registered collection fields.
    pub(crate) fn collection_keys(&self) -> Vec<FieldIdentity> {
        self.collections.keys().cloned().collect()
    }

    /// Borrows the collection state map for snapshot construction.
    pub(crate) fn collections(&self) -> &BTreeMap<FieldIdentity, CollectionState> {
        &self.collections
    }

    /// Replaces the collection state map during snapshot restore or explicit identity restore.
    pub(crate) fn set_collections(
        &mut self,
        collections: BTreeMap<FieldIdentity, CollectionState>,
    ) {
        self.collections = collections;
    }

    // --- coordinated lifecycle (the three maps move together) ---

    /// Iterates version entries for snapshot construction.
    pub(crate) fn iter_versions(&self) -> impl Iterator<Item = (&FieldIdentity, &u64)> {
        self.versions.iter()
    }

    /// Iterates metadata entries for snapshot construction.
    pub(crate) fn iter_metadata(&self) -> impl Iterator<Item = (&FieldIdentity, &FieldMetadata)> {
        self.metadata.iter()
    }

    /// Retains version and metadata entries for fields matching `keep`.
    ///
    /// Collection state is retained separately by its own removal path, matching the previous
    /// behavior of clearing collection-item metadata without touching collection identity state.
    pub(crate) fn retain_fields(&mut self, mut keep: impl FnMut(&FieldIdentity) -> bool) {
        self.versions.retain(|field, _| keep(field));
        self.metadata.retain(|field, _| keep(field));
    }

    /// Replaces version and metadata maps during snapshot restore.
    pub(crate) fn restore_fields(
        &mut self,
        versions: BTreeMap<FieldIdentity, u64>,
        metadata: BTreeMap<FieldIdentity, FieldMetadata>,
    ) {
        self.versions = versions;
        self.metadata = metadata;
    }

    /// Clears all field-keyed state on reset and reinitialization.
    pub(crate) fn clear(&mut self) {
        self.versions.clear();
        self.metadata.clear();
        self.collections.clear();
    }
}
