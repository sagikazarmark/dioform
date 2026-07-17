//! Router and the shared shell (header + grouped sidebar).

use dioxus::prelude::*;

use crate::components::{DemoFooter, DemoHeader, Sidebar, SidebarNavLink, SidebarNavSection};
use crate::pages::*;

const STYLE: Asset = asset!("/build/style.css");

/// Every page hangs off the one `Layout`, so the header, sidebar, and footer
/// render once and the active page swaps in through the `Outlet`.
#[derive(Routable, Clone, PartialEq, Debug)]
pub enum Route {
    #[layout(DemoLayout)]
    #[route("/")]
    Home {},
    // Basics
    #[route("/minimal")]
    Minimal {},
    #[route("/fields")]
    FieldBindings {},
    #[route("/parsed")]
    ParsedInputs {},
    // Validation
    #[route("/validation/modes")]
    ValidationModes {},
    #[route("/validation/validators")]
    Validators {},
    #[route("/validation/errors")]
    ErrorSummary {},
    #[route("/validation/async")]
    AsyncValidation {},
    #[route("/validation/adapters")]
    AdapterValidation {},
    // Fields & state
    #[route("/collections")]
    Collections {},
    #[route("/collections/validation")]
    CollectionValidation {},
    #[route("/files")]
    FileFields {},
    #[route("/nested")]
    NestedPaths {},
    #[route("/field-groups")]
    FieldGroups {},
    #[route("/state")]
    StateMeta {},
    #[route("/observers")]
    Observers {},
    #[route("/serialization")]
    Serialization {},
    // Submission
    #[route("/submit/intents")]
    SubmitIntents {},
    #[route("/submit/browser")]
    BrowserSubmission {},
    #[route("/submit/errors")]
    SubmitErrors {},
    // Server
    #[route("/server")]
    ServerValidation {},
    // Realistic forms
    #[route("/forms/signup")]
    Signup {},
    #[route("/forms/checkout")]
    Checkout {},
    #[route("/forms/invoice")]
    Invoice {},
    #[route("/forms/project-planner")]
    ProjectPlanner {},
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

#[component]
pub fn App() -> Element {
    rsx! {
        document::Stylesheet { href: STYLE }
        Router::<Route> {}
    }
}

/// Shared application shell for every demo route.
#[component]
fn DemoLayout() -> Element {
    let mut hydrated = use_signal(|| false);
    use_effect(move || hydrated.set(true));

    rsx! {
        div {
            class: "min-h-screen bg-base-100 text-base-content",
            "data-demo-hydrated": if hydrated() { "true" } else { "false" },
            DemoHeader {
                home: Route::Home {},
                mark: "df",
                name: "dioform",
                github_url: "https://github.com/sagikazarmark/dioform",
            }
            div { class: "mx-auto w-full max-w-7xl lg:flex lg:gap-8 lg:px-6",
                Sidebar {
                    SidebarNavSection { label: "Basics",
                        SidebarNavLink { route: Route::Home {}, label: "Overview" }
                        SidebarNavLink { route: Route::Minimal {}, label: "Minimal form" }
                        SidebarNavLink { route: Route::FieldBindings {}, label: "Field bindings" }
                        SidebarNavLink { route: Route::ParsedInputs {}, label: "Parsed inputs" }
                    }
                    SidebarNavSection { label: "Validation",
                        SidebarNavLink { route: Route::ValidationModes {}, label: "Modes & triggers" }
                        SidebarNavLink { route: Route::Validators {}, label: "Field & form validators" }
                        SidebarNavLink { route: Route::ErrorSummary {}, label: "Error visibility & summary" }
                        SidebarNavLink { route: Route::AsyncValidation {}, label: "Async & debounced" }
                        SidebarNavLink { route: Route::AdapterValidation {}, label: "Adapter validation" }
                    }
                    SidebarNavSection { label: "Fields & state",
                        SidebarNavLink { route: Route::Collections {}, label: "Collections" }
                        SidebarNavLink { route: Route::CollectionValidation {}, label: "Collection item validation" }
                        SidebarNavLink { route: Route::FileFields {}, label: "File fields" }
                        SidebarNavLink { route: Route::NestedPaths {}, label: "Nested structs & paths" }
                        SidebarNavLink { route: Route::FieldGroups {}, label: "Field groups" }
                        SidebarNavLink { route: Route::StateMeta {}, label: "State & meta" }
                        SidebarNavLink { route: Route::Observers {}, label: "Selectors & observers" }
                        SidebarNavLink { route: Route::Serialization {}, label: "State serialization" }
                    }
                    SidebarNavSection { label: "Submission",
                        SidebarNavLink { route: Route::SubmitIntents {}, label: "Submit intents" }
                        SidebarNavLink { route: Route::BrowserSubmission {}, label: "Browser & progressive" }
                        SidebarNavLink { route: Route::SubmitErrors {}, label: "Submit errors" }
                    }
                    SidebarNavSection { label: "Server",
                        SidebarNavLink { route: Route::ServerValidation {}, label: "Server validation" }
                    }
                    SidebarNavSection { label: "Realistic forms",
                        SidebarNavLink { route: Route::Signup {}, label: "Signup" }
                        SidebarNavLink { route: Route::Checkout {}, label: "Checkout" }
                        SidebarNavLink { route: Route::Invoice {}, label: "Invoice" }
                        SidebarNavLink { route: Route::ProjectPlanner {}, label: "Project planner" }
                    }
                }
                main { id: "main-content", class: "min-w-0 flex-1 px-4 py-8 sm:px-6 lg:px-0 lg:py-12",
                    Outlet::<Route> {}
                }
            }
            DemoFooter {
                description: "A docs-by-example gallery for the dioform library.",
                links: rsx! {
                    a {
                        class: "hover:text-base-content",
                        href: "https://github.com/sagikazarmark/dioform",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "Repository"
                    }
                },
            }
        }
    }
}
