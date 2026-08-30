use dioxus::prelude::*;

use crate::app::Route;
use crate::components::{
    DocsCallout, ExampleLayout, ExampleSection, ExternalAction, InlineCode, PageHeader,
    snippet_theme,
};
use crate::examples::dioxus_field_registry::DioxusFieldRegistryExample;
use crate::examples::field_bindings::FieldBindingsExample;
use crate::examples::happy_path::HappyPathExample;
use crate::examples::minimal::MinimalExample;
use crate::examples::parsed_inputs::ParsedInputsExample;
use dioxus_code::{Code, code};

#[component]
pub fn Home() -> Element {
    let groups = [
        (
            "Basics",
            "Bindings for text, choices, and parsed inputs.",
            Route::FieldBindings {},
        ),
        (
            "Validation",
            "Sync, cross-field, async, debounced, and adapter validation.",
            Route::Validators {},
        ),
        (
            "Fields & state",
            "Collections, files, nested paths, groups, and state snapshots.",
            Route::Collections {},
        ),
        (
            "Submission",
            "Managed submit, intents, browser forms, and submit errors.",
            Route::SubmitIntents {},
        ),
        (
            "Server",
            "Fullstack server-function validation with typed rejections.",
            Route::ServerValidation {},
        ),
        (
            "Realistic forms",
            "Signup, checkout, invoice, and a nested project planner.",
            Route::Signup {},
        ),
    ];

    rsx! {
        PageHeader {
            eyebrow: "dioform",
            title: "Headless form state, typed to your model",
            intro: "dioform keeps form state, validation, and submission in a form-owned draft addressed by compile-time FieldPath values. Your components own all the markup. Focused feature pages mount real examples next to their exact source; realistic forms combine several features into complete pages.",
        }

        div { class: "mt-10 grid gap-4 sm:grid-cols-2 lg:grid-cols-3",
            for (title , blurb , route) in groups {
                Link {
                    to: route,
                    class: "group rounded-2xl border border-base-300 bg-base-100 p-5 shadow-sm transition-colors hover:border-primary/40 hover:bg-base-200/40",
                    p { class: "font-semibold tracking-tight group-hover:text-primary", "{title}" }
                    p { class: "mt-1 text-sm text-base-content/65", "{blurb}" }
                }
            }
        }

        DocsCallout {
            title: "Start with the README",
            action: Some(ExternalAction::new(
                "Repository & docs",
                "https://github.com/sagikazarmark/dioform",
            )),
            "Every feature area links to the doc that owns it. The library is split into a renderer-agnostic core, a Dioxus facade, a derive crate, and optional validation adapters."
        }
    }
}

#[component]
pub fn Minimal() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Basics",
            title: "A minimal form",
            intro: "The smallest useful form: one text field, a managed submit, and a status readout. use_form builds the form-owned draft; form.text binds a typed field path to an input.",
        }
        ExampleSection {
            title: "use_form + form.text + managed_submit",
            intro: rsx! {
                InlineCode { "managed_submit" }
                " owns the submission lifecycle, so the submit closure only ever sees a validated, owned "
                InlineCode { "SubmissionSnapshot" }
                ". Returning "
                InlineCode { "SubmitErrors::none()" }
                " reports success."
            },
            demo: rsx! { MinimalExample {} },
            code: rsx! {
                Code { src: code!("src/examples/minimal.rs"), theme: snippet_theme() }
            },
            layout: ExampleLayout::Columns,
        }
    }
}

#[component]
pub fn HappyPath() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Basics",
            title: "The complete happy path",
            intro: "A practical form using Dioform's form-owned Form Draft and Dioxus-Managed Submission together with dioxus-field registry controls, garde validation, a reusable Field Group, and field-scoped Form Selectors.",
        }
        ExampleSection {
            title: "One form, end to end",
            intro: rsx! {
                InlineCode { "TextField" }
                ", "
                InlineCode { "TextareaField" }
                ", and "
                InlineCode { "SwitchField" }
                " cover common controls; compound registry parts handle the choice controls. "
                InlineCode { "derived_path_map" }
                " routes garde diagnostics to typed direct Field Paths, while "
                InlineCode { "FieldGroup" }
                " mounts the nested preferences paths."
            },
            demo: rsx! { HappyPathExample {} },
            code: rsx! {
                Code { src: code!("src/examples/happy_path.rs"), theme: snippet_theme() }
            },
        }
    }
}

#[component]
pub fn FieldBindings() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Basics",
            title: "Field bindings",
            intro: "Text, textarea, checkbox, select, and radio-group bindings, each driven by a typed field path off the same form handle.",
        }
        ExampleSection {
            title: "text · textarea · checkbox · select · radio_group",
            intro: rsx! {
                "Each binding exposes the props an element needs ("
                InlineCode { "name" }
                "/"
                InlineCode { "value" }
                " or "
                InlineCode { "checked" }
                ") and the handlers to feed changes back into the form-owned draft. The state grid reflects the live snapshot."
            },
            demo: rsx! { FieldBindingsExample {} },
            code: rsx! {
                Code { src: code!("src/examples/field_bindings.rs"), theme: snippet_theme() }
            },
        }
    }
}

#[component]
pub fn DioxusFieldRegistry() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Basics",
            title: "dioxus-field registry integration",
            intro: "Dioform bindings plug directly into dioxus-field compatible controls. This example uses dioxus-daisyui from Git and shows both concise field composition and the customizable parts beneath it.",
        }
        ExampleSection {
            title: "Dioform Field Context + dioxus-daisyui",
            intro: rsx! {
                InlineCode { "TextField" }
                " and "
                InlineCode { "TextareaField" }
                " compose the common label, control, description, and error path. The radio group and switch use "
                InlineCode { "Field" }
                " parts directly when their structure needs more control. Both layers resolve values, user writes, commits, field metadata, and formatted errors from Dioform Field Context."
            },
            demo: rsx! { DioxusFieldRegistryExample {} },
            code: rsx! {
                Code { src: code!("src/examples/dioxus_field_registry.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "External widget registry",
            action: Some(ExternalAction::new(
                "dioxus-daisyui registry",
                "https://github.com/sagikazarmark/dioxus-daisyui-registry.orig",
            )),
            "The demo depends on the registry's library crate directly from Git because it is not published yet. Normal registry users can also install individual components into their own source tree."
        }
    }
}

#[component]
pub fn ParsedInputs() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Basics",
            title: "Parsed inputs",
            intro: "Number, money, and date fields parse raw input into typed model values, keeping the unparseable raw text and a parse error separate from validation.",
        }
        ExampleSection {
            title: "use_number + use_number_with",
            intro: rsx! {
                "Type letters into the quantity field to see the "
                InlineCode { "parse_error" }
                " appear while the last valid value stays in the model. The unit price parses dollars into integer cents with a custom parser/formatter."
            },
            demo: rsx! { ParsedInputsExample {} },
            code: rsx! {
                Code { src: code!("src/examples/parsed_inputs.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "Input helpers",
            action: Some(ExternalAction::new(
                "docs/input-helpers.md",
                "https://github.com/sagikazarmark/dioform/blob/main/docs/input-helpers.md",
            )),
            "Parsed bindings, blockers, and the raw-input/parse-error split are documented in the input helpers guide. Date parsing works the same way; see the Invoice form."
        }
    }
}
