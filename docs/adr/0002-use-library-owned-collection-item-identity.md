# Use library-owned collection item identity

Post-MVP **Collection Fields** will preserve item metadata with a library-owned opaque **Collection Item Identity**, not with rendered indices or application-provided row keys. This keeps draft-only rows usable before they have domain IDs, prevents metadata and errors from sticking to old positions after reorder, and lets rendered **Field Names** remain index-based for HTML interoperability while **Field Identity** stays tied to the logical item.

The first collection slice should support direct `Vec<Item>` fields on named **Form Models**. Broader traversal, maps, sets, arrays, dynamic forms, and enum-variant collection paths remain separate follow-up decisions.
