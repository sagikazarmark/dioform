//! Shared presentation helpers used across the demo pages.
//!
//! Nothing here touches `dioform`: it is pure layout chrome so the `pages`
//! and `examples` modules can stay focused on the library being demonstrated.

use dioxus::prelude::*;
use dioxus_code::{CodeTheme, Theme};
use dioform::{CheckboxBinding, SelectBinding, TextBinding};

/// Theme for every on-page code snippet. Defined once so all snippets match and
/// the palette is trivial to swap. `system()` follows the viewer's light/dark
/// preference via CSS media queries. Pair with the compile-time `code!` macro,
/// so the highlighted snippet shown is exactly the code that runs.
pub fn snippet_theme() -> CodeTheme {
    CodeTheme::system(Theme::GITHUB_LIGHT, Theme::TOKYO_NIGHT)
}

/// Consistent page heading: a small colored eyebrow, a title, and a lead
/// paragraph.
#[component]
pub fn PageHeader(
    #[props(into)] eyebrow: String,
    #[props(into)] title: String,
    #[props(into)] intro: String,
) -> Element {
    rsx! {
        header { class: "max-w-3xl",
            p { class: "text-sm font-semibold uppercase tracking-[0.18em] text-primary", "{eyebrow}" }
            h1 { class: "mt-3 text-4xl font-bold tracking-tight text-balance", "{title}" }
            p { class: "mt-4 text-lg leading-8 text-base-content/70", "{intro}" }
        }
    }
}

/// Inline monospace styling for an API name or identifier mentioned in prose
/// (e.g. `InlineCode { "use_form" }`). Keeps the styling in one place so every
/// reference reads the same.
#[component]
pub fn InlineCode(children: Element) -> Element {
    rsx! {
        code { class: "rounded bg-base-200 px-1.5 py-0.5 font-mono text-[0.85em] text-base-content/80",
            {children}
        }
    }
}

/// A single documented example: a heading, a short explanation, the live
/// component, and the exact source that produced it.
///
/// `demo` is the live render; `code` is the source block. `intro` is an
/// `Element` so it can carry inline [`InlineCode`] and links.
#[component]
pub fn ExampleSection(
    #[props(into)] title: String,
    intro: Element,
    demo: Element,
    code: Element,
) -> Element {
    rsx! {
        section { class: "mt-10 rounded-[2rem] border border-base-300 bg-base-100 p-6 shadow-sm sm:p-8",
            h2 { class: "text-xl font-semibold tracking-tight", "{title}" }
            p { class: "mt-2 max-w-[70ch] text-sm leading-6 text-base-content/65", {intro} }
            div { class: "mt-6 grid gap-6 lg:grid-cols-2",
                // Live column.
                div {
                    p { class: "mb-3 text-xs font-semibold uppercase tracking-wider text-base-content/45", "Live" }
                    div { class: "rounded-2xl border border-base-300 bg-base-200/40 p-5", {demo} }
                }
                // Source column.
                div {
                    p { class: "mb-3 text-xs font-semibold uppercase tracking-wider text-base-content/45", "Source" }
                    div { class: "overflow-x-auto rounded-2xl border border-base-300 bg-base-200/60 p-4 text-sm [&_pre]:!bg-transparent",
                        {code}
                    }
                }
            }
        }
    }
}

/// Inline link to project documentation (rendered as an external link).
#[component]
pub fn DocLink(#[props(into)] href: String, children: Element) -> Element {
    rsx! {
        a {
            class: "link link-primary",
            href: "{href}",
            target: "_blank",
            rel: "noopener noreferrer",
            {children}
        }
    }
}

/// A callout pointing at the doc that owns a feature, plus optional extra notes.
#[component]
pub fn DocsCallout(
    #[props(into)] title: String,
    #[props(into)] doc_label: String,
    #[props(into)] doc_href: String,
    children: Element,
) -> Element {
    rsx! {
        div { class: "mt-8 rounded-2xl border border-info/40 bg-info/5 p-5",
            div { class: "flex items-center gap-2",
                span { class: "text-lg", "📄" }
                p { class: "font-semibold text-base-content", "{title}" }
            }
            div { class: "mt-2 max-w-[70ch] text-sm leading-6 text-base-content/70", {children} }
            div { class: "mt-4",
                a {
                    class: "btn btn-sm btn-outline btn-info",
                    href: "{doc_href}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    "{doc_label} ↗"
                }
            }
        }
    }
}

/// Key/value readout rendered as a definition grid. Lets state-inspection
/// examples list the fields they read without repeating the grid markup.
#[component]
pub fn StateGrid(rows: Vec<(&'static str, String)>) -> Element {
    rsx! {
        dl { class: "grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 font-mono text-sm",
            for (label , value) in rows {
                dt { class: "text-base-content/55", "{label}" }
                dd { class: "break-all", "{value}" }
            }
        }
    }
}

/// Muted one-line status/result readout for interactive examples. Renders
/// nothing while `status` is empty so callers can mount it unconditionally.
#[component]
pub fn StatusLine(#[props(into)] status: String) -> Element {
    if status.is_empty() {
        return rsx! {};
    }
    rsx! {
        p { class: "mt-3 rounded-lg bg-base-100 px-3 py-2 text-sm text-base-content/75", "{status}" }
    }
}

// --- Shared field helpers for the realistic-forms pages ---------------------
//
// These keep the multi-field product forms readable. Feature examples deliberately
// inline their markup instead (so the quoted source is self-contained).

fn field_errors(errors: Vec<dioform::ValidationErrorSnapshot<String>>) -> Element {
    rsx! {
        div { class: "min-h-4",
            for error in errors {
                p { class: "text-sm text-error", "{error.error()}" }
            }
        }
    }
}

/// A labeled text input bound to a `TextBinding`, with inline validation errors.
pub fn field_text<Model: 'static>(
    label: &str,
    binding: &TextBinding<Model, String>,
    input_type: &'static str,
    placeholder: &str,
) -> Element {
    let accessibility = binding.accessibility();
    rsx! {
        label { class: "block space-y-1",
            span { class: "text-sm font-medium", "{label}" }
            input {
                class: "input input-bordered w-full",
                class: if accessibility.aria_invalid() { "input-error" },
                r#type: "{input_type}",
                name: binding.name(),
                value: binding.value(),
                placeholder: "{placeholder}",
                oninput: binding.oninput(),
                onblur: binding.onblur(),
            }
            {field_errors(binding.visible_validation_errors())}
        }
    }
}

/// A labeled checkbox bound to a `CheckboxBinding`.
pub fn field_checkbox<Model: 'static>(
    label: &str,
    binding: &CheckboxBinding<Model, String>,
) -> Element {
    rsx! {
        label { class: "flex items-center gap-2",
            input {
                class: "checkbox checkbox-primary",
                r#type: "checkbox",
                name: binding.name(),
                checked: binding.checked(),
                oninput: binding.onchange(),
                onblur: binding.onblur(),
            }
            span { class: "text-sm font-medium", "{label}" }
        }
    }
}

/// A labeled string `<select>` bound to a `SelectBinding`, with inline errors.
pub fn field_select<Model: 'static, const N: usize>(
    label: &str,
    binding: &SelectBinding<Model, String, String>,
    options: [(&'static str, &'static str); N],
) -> Element {
    let accessibility = binding.accessibility();
    rsx! {
        label { class: "block space-y-1",
            span { class: "text-sm font-medium", "{label}" }
            select {
                class: "select select-bordered w-full",
                class: if accessibility.aria_invalid() { "select-error" },
                name: binding.name(),
                value: binding.value(),
                onchange: binding.onchange(),
                onblur: binding.onblur(),
                for (value , option_label) in options {
                    option {
                        value: "{value}",
                        selected: binding.is_selected(&value.to_string()),
                        "{option_label}"
                    }
                }
            }
            {field_errors(binding.visible_validation_errors())}
        }
    }
}
