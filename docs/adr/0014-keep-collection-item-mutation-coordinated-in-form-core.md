# Keep collection-item mutation coordinated in Form Core

Dioform will keep **Collection Field** item insertion, removal, and reordering coordinated in the **Form
Core** rather than moving the mutation into the **Field Store**. This extends
[ADR-0011](0011-do-not-extract-a-chain-executor-module.md) from validator execution to collection mutation.

Running one collection mutation touches five subsystems at once: the **Form Draft** (the `Vec<Item>` value),
the **Field Store** (**Collection Item Identity** ordering and item **Field** metadata), the
`ValidationChainRegistry` (per-item validator states), the **Submission State** (submit errors cleared for the
collection), and the **Form Observer**. `insert_collection_item_with_origin` writes all five; the removal path
`clear_collection_item_state` retains across the **Field Store** plus three `ValidationChainRegistry` queries,
keyed by **Collection Item Identity** through `CollectionItemFieldAddress::matches_item`.

A **Field-Store**-owned mutation would therefore have to borrow four sibling subsystems, so it would relocate
the coupling into another module without concentrating it; deleting that module would not collapse
complexity, which means it is not a real seam. The **Field Store**'s collection accessors are the necessary
driving seam for **Form Core** to coordinate the mutation (the same accepted cost
[ADR-0010](0010-carve-form-core-into-field-store-submission-state-and-chain-executor.md) noted for the
`ValidationChainRegistry` accessors), not a leak to be closed. This is why `FieldStore::retain_fields`
deliberately touches only version and metadata and leaves collection-item removal to **Form Core**, and why
the co-designed `SubmissionState` remains a state holder whose submit-availability invariants are composed in
**Form Core**. Future architecture reviews should not re-suggest a **Field-Store**-owned collection mutation
unless per-item mutation stops needing draft, field-store, registry, submission, and observer access together.
