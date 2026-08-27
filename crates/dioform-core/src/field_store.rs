//! Single owner of per-field state keyed by [`FieldIdentity`].
//!
//! The **Form Core** previously held field version counters, interaction metadata, and collection
//! identity state in three parallel `BTreeMap`s, kept aligned by convention across every method.
//! This module concentrates them behind one interface so that lazy **Field Registration** (absent
//! fields read as version `0` and default metadata without allocating) and the coordinated
//! lifecycle (clear, retain, snapshot, and restore each state knowing what the others do) become
//! encapsulated invariants rather than caller discipline. The three do not all move together:
//! collection identity state outlives the clear that resets versions and metadata, because its
//! identities may never be reissued.
//!
//! Version has exactly one owner here. Downstream submission and validation logic reads
//! [`FieldStore::version`] for staleness; it never writes versions.

use std::collections::BTreeMap;

use crate::{
    CollectionItemIdentity, CollectionState, FieldIdentity, FieldMetadata,
    field_ancestry::FieldAncestry,
};

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

    /// Returns whether `container` or one of its contained fields has matching metadata.
    ///
    /// Identity ordering keeps possible descendants contiguous within the static and collection-item
    /// identity kinds. The ancestry check inside each range excludes separator-adjacent siblings.
    pub(crate) fn subtree_metadata_any(
        &self,
        container: &FieldIdentity,
        mut matches: impl FnMut(FieldMetadata) -> bool,
    ) -> bool {
        if let Some(path) = container.static_path() {
            if self
                .metadata
                .get(container)
                .is_some_and(|metadata| matches(*metadata))
            {
                return true;
            }

            if path.is_empty() {
                return false;
            }

            let static_lower = FieldIdentity::static_ordering_bound(format!("{path}."));
            let static_upper = FieldIdentity::static_ordering_bound(format!("{path}/"));
            if self
                .metadata
                .range(static_lower..static_upper)
                .any(|(field, metadata)| {
                    FieldAncestry::contains(container, field) && matches(*metadata)
                })
            {
                return true;
            }

            let exact_item_lower =
                FieldIdentity::collection_item(path, CollectionItemIdentity(0), "");
            if self
                .metadata
                .range(exact_item_lower..)
                .take_while(|(field, _)| field.collection_path() == Some(path))
                .any(|(field, metadata)| {
                    FieldAncestry::contains(container, field) && matches(*metadata)
                })
            {
                return true;
            }

            let descendant_item_lower =
                FieldIdentity::collection_item(format!("{path}."), CollectionItemIdentity(0), "");
            let descendant_item_upper =
                FieldIdentity::collection_item(format!("{path}/"), CollectionItemIdentity(0), "");

            return self
                .metadata
                .range(descendant_item_lower..descendant_item_upper)
                .any(|(field, metadata)| {
                    FieldAncestry::contains(container, field) && matches(*metadata)
                });
        }

        let Some((collection, item, field)) = container.collection_item_parts() else {
            return self
                .metadata
                .get(container)
                .is_some_and(|metadata| matches(*metadata));
        };

        if field.is_empty() {
            let Some(next_item) = item.as_u64().checked_add(1) else {
                return self
                    .metadata
                    .range(container.clone()..)
                    .take_while(|(candidate, _)| candidate.is_collection_item_for(collection, item))
                    .any(|(_, metadata)| matches(*metadata));
            };
            let upper =
                FieldIdentity::collection_item(collection, CollectionItemIdentity(next_item), "");

            return self
                .metadata
                .range(container.clone()..upper)
                .any(|(candidate, metadata)| {
                    FieldAncestry::contains(container, candidate) && matches(*metadata)
                });
        }

        let upper = FieldIdentity::collection_item(collection, item, format!("{field}/"));
        self.metadata
            .range(container.clone()..upper)
            .any(|(candidate, metadata)| {
                FieldAncestry::contains(container, candidate) && matches(*metadata)
            })
    }

    /// Returns whether a field has version or interaction state that a field reset must clear.
    pub(crate) fn has_reset_relevant_state(&self, field: &FieldIdentity) -> bool {
        self.version(field) != 0 || self.metadata(field) != FieldMetadata::default()
    }

    // --- collections (state stored opaquely; item mutation logic stays in Form Core, while the
    // whole-map lifecycle below owns the rule that no identity counter moves backward) ---

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

    /// Borrows every registered collection's state mutably for a lifecycle operation that moves
    /// them together, such as a form reset or reinitialization.
    pub(crate) fn collections_mut(&mut self) -> impl Iterator<Item = &mut CollectionState> {
        self.collections.values_mut()
    }

    /// Returns the identities of all registered collection fields.
    pub(crate) fn collection_keys(&self) -> Vec<FieldIdentity> {
        self.collections.keys().cloned().collect()
    }

    /// Borrows the collection state map for snapshot construction.
    pub(crate) fn collections(&self) -> &BTreeMap<FieldIdentity, CollectionState> {
        &self.collections
    }

    /// Adopts collection state while restoring a full form-state snapshot.
    ///
    /// Restored identity sequences replace the live ones, but no identity counter moves backward:
    /// each restored collection carries the higher of its own and the live counter, and a live
    /// collection the restored state says nothing about has its identities retired rather than
    /// renumbered from zero. Either way the next identity this form mints is one it has never
    /// issued before.
    pub(crate) fn adopt_collections(&mut self, restored: BTreeMap<FieldIdentity, CollectionState>) {
        let mut replaced = std::mem::replace(&mut self.collections, restored);

        for (field, adopted) in &mut self.collections {
            if let Some(replaced) = replaced.remove(field) {
                adopted.advance_next_item_identity_to_at_least(replaced.next_item_identity());
            }
        }

        for (field, mut unrestored) in replaced {
            unrestored.retire_items();
            self.collections.insert(field, unrestored);
        }
    }

    // --- coordinated lifecycle (versions and metadata move together) ---

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

    /// Clears version and metadata state on reset and reinitialization.
    ///
    /// Collection identity state survives, because clearing it would rewind every identity counter
    /// with it and hand the same **Collection Item Identity** to a different logical item. Reset and
    /// reinitialization move that state through their own operations on [`CollectionState`].
    pub(crate) fn clear_fields(&mut self) {
        self.versions.clear();
        self.metadata.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(value: u64) -> CollectionItemIdentity {
        CollectionItemIdentity(value)
    }

    fn next_random(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    #[test]
    fn subtree_metadata_ranges_agree_with_naive_ancestry_scans() {
        let mut store = FieldStore::default();
        let mut identities = vec![
            FieldIdentity::new("counterparty"),
            FieldIdentity::new("counterparty.name"),
            FieldIdentity::new("counterparty_account"),
            FieldIdentity::new("counterparty_account.name"),
            FieldIdentity::new("invoice.line"),
            FieldIdentity::new("invoice.lines"),
            FieldIdentity::new("invoice-line"),
            FieldIdentity::new("invoice_lines"),
            FieldIdentity::new("invoice.lines.description"),
            FieldIdentity::collection_item("invoice.lines", item(0), ""),
            FieldIdentity::collection_item("invoice.lines", item(0), "description"),
            FieldIdentity::collection_item("invoice.lines", item(1), "description"),
            FieldIdentity::collection_item("invoice-lines", item(0), "description"),
            FieldIdentity::collection_item("invoice_lines", item(0), "description"),
            FieldIdentity::collection_item("invoice.notes", item(0), "description"),
            FieldIdentity::collection_item("invoice.sections.lines", item(0), ""),
            FieldIdentity::collection_item("invoice.sections.lines", item(0), "product.name"),
            FieldIdentity::file("counterparty"),
        ];
        let segments = ["account", "customer", "line", "lines", "name", "product"];
        let collections = [
            "invoice.lines",
            "invoice.line_items",
            "invoice.notes",
            "invoice.sections.lines",
        ];
        let mut random = 0x005e_ed53_u64;

        for _ in 0..512 {
            let kind = next_random(&mut random) % 3;
            let first = segments[(next_random(&mut random) as usize) % segments.len()];
            let second = segments[(next_random(&mut random) as usize) % segments.len()];
            let third = segments[(next_random(&mut random) as usize) % segments.len()];
            let depth = (next_random(&mut random) % 3) + 1;
            let path = match depth {
                1 => first.to_owned(),
                2 => format!("{first}.{second}"),
                _ => format!("{first}.{second}.{third}"),
            };
            let identity = match kind {
                0 => FieldIdentity::new(path),
                1 => FieldIdentity::file(path),
                _ => {
                    let collection =
                        collections[(next_random(&mut random) as usize) % collections.len()];
                    let field = if next_random(&mut random).is_multiple_of(4) {
                        String::new()
                    } else {
                        path
                    };
                    FieldIdentity::collection_item(
                        collection,
                        item(next_random(&mut random) % 8),
                        field,
                    )
                }
            };
            identities.push(identity);
        }

        for identity in &identities {
            let value = next_random(&mut random);
            *store.metadata_mut(identity) = FieldMetadata {
                touched: value & 1 != 0,
                blurred: value & 2 != 0,
                committed: value & 4 != 0,
            };
        }

        let mut containers = identities.clone();
        containers.extend([
            FieldIdentity::new("invoice"),
            FieldIdentity::new("invoice.lines"),
            FieldIdentity::new("invoice.lines.description"),
            FieldIdentity::new("invoice.sections"),
            FieldIdentity::collection_item("invoice.lines", item(0), ""),
            FieldIdentity::collection_item("invoice.lines", item(0), "product"),
        ]);

        for container in containers {
            for predicate in [
                FieldMetadata::is_blurred,
                FieldMetadata::is_touched,
                FieldMetadata::is_committed,
            ] {
                let expected = store.metadata.iter().any(|(field, metadata)| {
                    FieldAncestry::contains(&container, field) && predicate(*metadata)
                });
                let actual = store.subtree_metadata_any(&container, predicate);

                assert_eq!(actual, expected, "subtree mismatch for {container:?}");
            }
        }
    }
}
