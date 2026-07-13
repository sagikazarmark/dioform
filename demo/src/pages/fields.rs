use dioxus::prelude::*;

use crate::components::{
    DocsCallout, ExampleSection, ExternalAction, InlineCode, PageHeader, snippet_theme,
};
use crate::examples::collection_validation::CollectionValidationExample;
use crate::examples::collections::CollectionsExample;
use crate::examples::field_groups::FieldGroupsExample;
use crate::examples::file_fields::FileFieldsExample;
use crate::examples::nested_paths::NestedPathsExample;
use crate::examples::observers::ObserversExample;
use crate::examples::serialization::SerializationExample;
use crate::examples::state_meta::StateMetaExample;
use dioxus_code::{Code, code};

#[component]
pub fn Collections() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Fields & state",
            title: "Collections",
            intro: "A CollectionBinding owns repeatable rows with library-managed item identity: append, insert, remove, move, swap, replace, and clear, all without app-supplied keys.",
        }
        ExampleSection {
            title: "form.collection(path) mutations",
            intro: rsx! {
                "Every button is a method on the binding. Per-row input state follows each item by "
                InlineCode { "identity" }
                ", so reordering never mixes up which text belongs to which row."
            },
            demo: rsx! { CollectionsExample {} },
            code: rsx! {
                Code { src: code!("src/examples/collections.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "Collection fields",
            action: Some(ExternalAction::new(
                "docs/collection-fields.md",
                "https://github.com/sagikazarmark/dioform/blob/main/docs/collection-fields.md",
            )),
            "Item identity, the full mutation set, and collection state semantics are documented in the collection fields guide."
        }
    }
}

#[component]
pub fn CollectionValidation() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Fields & state",
            title: "Collection item validation",
            intro: "item_field_validator registers a validator that runs against each row's field, with errors that follow rows through reordering and removal.",
        }
        ExampleSection {
            title: "collection.item_field_validator(...).check(...)",
            intro: rsx! {
                "Clear a name to see its row-scoped error. The error stays with the item's identity, not its index, so removing a row above it does not move the error."
            },
            demo: rsx! { CollectionValidationExample {} },
            code: rsx! {
                Code { src: code!("src/examples/collection_validation.rs"), theme: snippet_theme() }
            },
        }
    }
}

#[component]
pub fn FileFields() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Fields & state",
            title: "File fields",
            intro: "First-class file fields track selected files with cardinality, kept outside the form draft and surfaced in file-aware submission snapshots.",
        }
        ExampleSection {
            title: "FileFieldKey + form.file(key) + on_submit_with_files",
            intro: rsx! {
                "Submit with no file selected to see the required file validator block submission; pick an image and its metadata appears. Files never enter the typed draft."
            },
            demo: rsx! { FileFieldsExample {} },
            code: rsx! {
                Code { src: code!("src/examples/file_fields.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "File fields",
            action: Some(ExternalAction::new(
                "docs/file-fields.md",
                "https://github.com/sagikazarmark/dioform/blob/main/docs/file-fields.md",
            )),
            "Cardinality, file validators, and file-aware submission are documented in the file fields guide."
        }
    }
}

#[component]
pub fn NestedPaths() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Fields & state",
            title: "Nested structs & paths",
            intro: "Nested named structs compose typed field paths with FieldPath::join, while rendered field names stay HTML-friendly and can be overridden per segment.",
        }
        ExampleSection {
            title: "FieldPath::join + #[form(name)] + #[form(rename_all)]",
            intro: rsx! {
                "Access stays typed all the way down through "
                InlineCode { "join" }
                ". The state grid shows how the rendered HTML name ("
                InlineCode { "camelCase" }
                ", or an explicit override) differs from the durable field identity."
            },
            demo: rsx! { NestedPathsExample {} },
            code: rsx! {
                Code { src: code!("src/examples/nested_paths.rs"), theme: snippet_theme() }
            },
        }
    }
}

#[component]
pub fn FieldGroups() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Fields & state",
            title: "Field groups",
            intro: "#[derive(FieldGroup)] generates a reusable, typed field-group map that can be mounted under a nested path or mapped onto a differently-shaped form.",
        }
        ExampleSection {
            title: "#[derive(FieldGroup)] + Address::mount(path)",
            intro: rsx! {
                "One address renderer serves two mounts, "
                InlineCode { "billing" }
                " and "
                InlineCode { "shipping" }
                ", of the same field group. The group is a reusable bundle of typed paths; it owns no validation or identity."
            },
            demo: rsx! { FieldGroupsExample {} },
            code: rsx! {
                Code { src: code!("src/examples/field_groups.rs"), theme: snippet_theme() }
            },
        }
    }
}

#[component]
pub fn StateMeta() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Fields & state",
            title: "State & meta",
            intro: "Touched, blurred, dirty, and pristine tracking, plus reset, single-field reset_field, and explicit reinitialization to a new baseline.",
        }
        ExampleSection {
            title: "is_dirty / is_pristine / reset / reset_field / reinitialize",
            intro: rsx! {
                "Edit a field and watch the meta flip; revert it and dirty clears (non-sticky). "
                InlineCode { "reset" }
                " returns everything to baseline, "
                InlineCode { "reset_field" }
                " just one field, and "
                InlineCode { "reinitialize" }
                " installs a new baseline."
            },
            demo: rsx! { StateMetaExample {} },
            code: rsx! {
                Code { src: code!("src/examples/state_meta.rs"), theme: snippet_theme() }
            },
        }
    }
}

#[component]
pub fn Observers() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Fields & state",
            title: "Selectors & observers",
            intro: "Listeners are application-owned side-effect hooks for semantic form events; the value-redacted stream reports lifecycle events for diagnostics.",
        }
        ExampleSection {
            title: "use_form_listener + use_field_listener_for_origin",
            intro: rsx! {
                "The log records each field replacement by name (never by value). Changing the "
                InlineCode { "country" }
                " runs an origin-filtered listener that resets the "
                InlineCode { "region" }
                "."
            },
            demo: rsx! { ObserversExample {} },
            code: rsx! {
                Code { src: code!("src/examples/observers.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "Form listeners",
            action: Some(ExternalAction::new(
                "docs/form-listeners.md",
                "https://github.com/sagikazarmark/dioform/blob/main/docs/form-listeners.md",
            )),
            "Field, form, blur, binding-lifecycle, debounced, and submit listeners are documented in the form listeners guide."
        }
    }
}

#[component]
pub fn Serialization() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Fields & state",
            title: "State serialization",
            intro: "Capture a full form-state snapshot (draft, metadata, submit state, and collection item identity) and restore it later, exactly.",
        }
        ExampleSection {
            title: "form.state_snapshot() / form.restore_state_snapshot(...)",
            intro: rsx! {
                "Capture, then edit the title or add/remove tags, then restore. The draft and the library-owned item identities come back exactly as captured."
            },
            demo: rsx! { SerializationExample {} },
            code: rsx! {
                Code { src: code!("src/examples/serialization.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "Form state serialization",
            action: Some(ExternalAction::new(
                "docs/form-state-serialization.md",
                "https://github.com/sagikazarmark/dioform/blob/main/docs/form-state-serialization.md",
            )),
            "The snapshot contents and the collection-identity round trip are documented in the serialization guide."
        }
    }
}
