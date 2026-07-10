use dioxus::prelude::*;

use crate::examples::adapter_validation::AdapterValidationExample;
use crate::examples::async_validation::AsyncValidationExample;
use crate::examples::error_summary::ErrorSummaryExample;
use crate::examples::validation_modes::ValidationModesExample;
use crate::examples::validators::ValidatorsExample;
use crate::ui::{DocsCallout, ExampleSection, InlineCode, PageHeader, snippet_theme};
use dioxus_code::{Code, code};

#[component]
pub fn ValidationModes() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Validation",
            title: "Modes & triggers",
            intro: "ValidationMode controls when validators run automatically: on blur (default), on change, on submit only, or submit-then-revalidate. ErrorVisibilityPolicy controls when stored errors show.",
        }
        ExampleSection {
            title: "ValidationMode::on_change() vs on_blur()",
            intro: rsx! {
                "The same rule under two modes. On-change flags the error as you type; on-blur waits until the field loses focus. "
                InlineCode { "on_submit()" }
                " and "
                InlineCode { "submit_then_revalidate()" }
                " round out the set."
            },
            demo: rsx! { ValidationModesExample {} },
            code: rsx! {
                Code { src: code!("src/examples/validation_modes.rs"), theme: snippet_theme() }
            },
        }
    }
}

#[component]
pub fn Validators() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Validation",
            title: "Field & form validators",
            intro: "Register a sync validator on a single field, or a form-level validator that reads the whole model to enforce cross-field rules and attach errors to specific fields.",
        }
        ExampleSection {
            title: "register_sync_field_validator + register_sync_form_validator",
            intro: rsx! {
                "The email field is guarded in isolation; the password confirmation is checked by a form validator that reads both fields and attaches its error to the confirmation field."
            },
            demo: rsx! { ValidatorsExample {} },
            code: rsx! {
                Code { src: code!("src/examples/validators.rs"), theme: snippet_theme() }
            },
        }
    }
}

#[component]
pub fn ErrorSummary() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Validation",
            title: "Error visibility & summary",
            intro: "visible_validation_errors respects the visibility policy per field; the whole-form aggregate lists every error with its target and source for an accessible error summary.",
        }
        ExampleSection {
            title: "form.visible_validation_errors()",
            intro: rsx! {
                "One call returns every stored error across the whole form. Each entry keeps its "
                InlineCode { "target" }
                ", "
                InlineCode { "field_identity" }
                ", "
                InlineCode { "source" }
                ", and typed value, so you can build a jump-to-field summary."
            },
            demo: rsx! { ErrorSummaryExample {} },
            code: rsx! {
                Code { src: code!("src/examples/error_summary.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "Whole-form error summaries",
            doc_label: "docs/error-summary.md",
            doc_href: "https://github.com/sagikazarmark/dioform/blob/main/docs/error-summary.md",
            "The aggregate accessors and how to map field identity back to a rendered name are documented in the error-summary guide."
        }
    }
}

#[component]
pub fn AsyncValidation() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Validation",
            title: "Async & debounced validation",
            intro: "Debounced async field and form validators run on Dioxus-spawned tasks from owned snapshots, with stale-result protection and submit-time flushing.",
        }
        ExampleSection {
            title: "field(path).async_validator(...).debounce(...).check(...)",
            intro: rsx! {
                "Stop typing and the debounced check fires. Try "
                InlineCode { "ada" }
                ", "
                InlineCode { "admin" }
                ", or "
                InlineCode { "taken" }
                " to see it reject. Edits during a pending check discard the stale result."
            },
            demo: rsx! { AsyncValidationExample {} },
            code: rsx! {
                Code { src: code!("src/examples/async_validation.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "Async & debounced validation",
            doc_label: "docs/async-validation.md",
            doc_href: "https://github.com/sagikazarmark/dioform/blob/main/docs/async-validation.md",
            "Task spawning, debounce, stale-result protection, submit-time flushing, and cleanup-safe late results are documented in the async validation guide."
        }
    }
}

#[component]
pub fn AdapterValidation() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Validation",
            title: "Adapter validation",
            intro: "The dioform-garde adapter maps garde's renderer-agnostic diagnostics onto typed field paths, so an existing garde schema drives form validation.",
        }
        ExampleSection {
            title: "core.garde_validation()...register_string_errors()",
            intro: rsx! {
                "The model derives "
                InlineCode { "garde::Validate" }
                "; the adapter runs garde and attaches each diagnostic to a typed field path through an explicit path map. No native validators were written for these rules."
            },
            demo: rsx! { AdapterValidationExample {} },
            code: rsx! {
                Code { src: code!("src/examples/adapter_validation.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "Validation adapters",
            doc_label: "docs/validation-adapters.md",
            doc_href: "https://github.com/sagikazarmark/dioform/blob/main/docs/validation-adapters.md",
            "Both the garde and validator adapters, custom error mapping, path maps, and trigger choices are documented in the validation adapters guide."
        }
    }
}
