use dioxus::prelude::*;

use crate::examples::browser_submission::BrowserSubmissionExample;
use crate::examples::submit_errors::SubmitErrorsExample;
use crate::examples::submit_intents::SubmitIntentsExample;
use crate::ui::{DocsCallout, ExampleSection, InlineCode, PageHeader, snippet_theme};
use dioxus_code::{Code, code};

#[component]
pub fn SubmitIntents() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Submission",
            title: "Submit intents",
            intro: "A typed submit intent (Save draft vs Publish) gives each button its own availability, last status, and validation, from one form.",
        }
        ExampleSection {
            title: "managed_submit().intent(...) + submit-triggered validation",
            intro: rsx! {
                "Publishing requires a title; saving a draft does not. The intent flows to a submit validator and to per-intent "
                InlineCode { "last_status" }
                ", never inferred from the button."
            },
            demo: rsx! { SubmitIntentsExample {} },
            code: rsx! {
                Code { src: code!("src/examples/submit_intents.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "Submit intent",
            doc_label: "docs/submit-intent.md",
            doc_href: "https://github.com/sagikazarmark/dioform/blob/main/docs/submit-intent.md",
            "Per-intent availability, status, visible errors, and intent-aware validators are documented in the submit intent guide."
        }
    }
}

#[component]
pub fn BrowserSubmission() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Submission",
            title: "Browser & progressive submission",
            intro: "Beyond controlled submits, dioform supports native browser submission and progressive submission with a hydrated preflight.",
        }
        ExampleSection {
            title: "managed_submit · browser_submit · progressive_submit",
            intro: rsx! {
                "The live form uses managed submission (safe, no navigation) and shows the native-fallback attributes. The "
                InlineCode { "progressive_submit" }
                " mode, shown in the source, runs a preflight and only blocks the browser POST when a blocker exists."
            },
            demo: rsx! { BrowserSubmissionExample {} },
            code: rsx! {
                Code { src: code!("src/examples/browser_submission.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "Browser submission modes",
            doc_label: "docs/browser-submission.md",
            doc_href: "https://github.com/sagikazarmark/dioform/blob/main/docs/browser-submission.md",
            "The three submit modes and their ownership boundaries are documented in the browser submission guide."
        }
    }
}

#[component]
pub fn SubmitErrors() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Submission",
            title: "Submit errors",
            intro: "Application submit behavior returns structured submit errors targeting fields or the form; they clear as the offending values change.",
        }
        ExampleSection {
            title: "SubmitError::field(...) + stale-error clearing",
            intro: rsx! {
                "Submit "
                InlineCode { "taken" }
                " to attach a field-targeted submit error, then edit the field: the submit error clears on its own because its value changed."
            },
            demo: rsx! { SubmitErrorsExample {} },
            code: rsx! {
                Code { src: code!("src/examples/submit_errors.rs"), theme: snippet_theme() }
            },
        }
    }
}
