# Use typed scoped form context

Dioform will provide optional Dioxus context access through typed **Form Context Scopes** that wrap existing **Form Handles**, rather than type-only lookup or global form registries. This keeps explicit **Form Handles** as the primary interface, lets deeply nested UI avoid handle plumbing when appropriate, and keeps multiple same-model form instances such as shipping and billing addresses unambiguous without reusing **Form ID Namespaces** or rendered field names for context identity.
