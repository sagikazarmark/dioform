//! Router and the shared shell (header + grouped sidebar).

use dioxus::prelude::*;

use crate::pages::*;

const TAILWIND_CSS: Asset = asset!("/assets/style.css");

/// Every page hangs off the one `Layout`, so the header, sidebar, and footer
/// render once and the active page swaps in through the `Outlet`.
#[derive(Routable, Clone, PartialEq, Debug)]
pub enum Route {
    #[layout(Layout)]
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
}

/// Grouped navigation shared by the desktop sidebar and the mobile strip.
fn nav_groups() -> Vec<(&'static str, Vec<(Route, &'static str)>)> {
    vec![
        (
            "Basics",
            vec![
                (Route::Home {}, "Overview"),
                (Route::Minimal {}, "Minimal form"),
                (Route::FieldBindings {}, "Field bindings"),
                (Route::ParsedInputs {}, "Parsed inputs"),
            ],
        ),
        (
            "Validation",
            vec![
                (Route::ValidationModes {}, "Modes & triggers"),
                (Route::Validators {}, "Field & form validators"),
                (Route::ErrorSummary {}, "Error visibility & summary"),
                (Route::AsyncValidation {}, "Async & debounced"),
                (Route::AdapterValidation {}, "Adapter validation"),
            ],
        ),
        (
            "Fields & state",
            vec![
                (Route::Collections {}, "Collections"),
                (Route::CollectionValidation {}, "Collection item validation"),
                (Route::FileFields {}, "File fields"),
                (Route::NestedPaths {}, "Nested structs & paths"),
                (Route::FieldGroups {}, "Field groups"),
                (Route::StateMeta {}, "State & meta"),
                (Route::Observers {}, "Selectors & observers"),
                (Route::Serialization {}, "State serialization"),
            ],
        ),
        (
            "Submission",
            vec![
                (Route::SubmitIntents {}, "Submit intents"),
                (Route::BrowserSubmission {}, "Browser & progressive"),
                (Route::SubmitErrors {}, "Submit errors"),
            ],
        ),
        (
            "Server",
            vec![(Route::ServerValidation {}, "Server validation")],
        ),
        (
            "Realistic forms",
            vec![
                (Route::Signup {}, "Signup"),
                (Route::Checkout {}, "Checkout"),
                (Route::Invoice {}, "Invoice"),
                (Route::ProjectPlanner {}, "Project planner"),
            ],
        ),
    ]
}

#[component]
pub fn App() -> Element {
    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
        Router::<Route> {}
    }
}

#[component]
fn Layout() -> Element {
    rsx! {
        div { class: "min-h-screen bg-base-100 text-base-content",
            Header {}
            MobileNav {}
            div { class: "mx-auto flex w-full max-w-7xl gap-8 px-4 sm:px-6",
                Sidebar {}
                main { class: "min-w-0 flex-1 py-8 lg:py-12", Outlet::<Route> {} }
            }
            Footer {}
        }
    }
}

#[component]
fn Header() -> Element {
    rsx! {
        header { class: "sticky top-0 z-20 border-b border-base-300 bg-base-100/90 backdrop-blur",
            div { class: "mx-auto flex min-h-16 w-full max-w-7xl items-center justify-between gap-4 px-4 sm:px-6",
                Link {
                    to: Route::Home {},
                    class: "flex min-w-0 items-center gap-3 rounded-2xl p-1.5 pr-3 transition-colors hover:bg-base-200",
                    span { class: "grid h-9 w-9 shrink-0 place-items-center rounded-2xl bg-primary text-sm font-bold text-primary-content shadow-sm",
                        "df"
                    }
                    span { class: "min-w-0",
                        span { class: "block truncate text-sm font-semibold tracking-tight", "dioform" }
                    }
                }
                a {
                    class: "btn btn-ghost btn-sm btn-circle",
                    href: "https://github.com/sagikazarmark/dioform",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    "aria-label": "View dioform on GitHub",
                    title: "View on GitHub",
                    svg {
                        view_box: "0 0 24 24",
                        width: "20",
                        height: "20",
                        fill: "currentColor",
                        "aria-hidden": "true",
                        path { d: "M12 .5C5.37.5 0 5.78 0 12.29c0 5.2 3.44 9.6 8.2 11.16.6.1.82-.25.82-.56v-2c-3.34.72-4.04-1.6-4.04-1.6-.55-1.36-1.33-1.72-1.33-1.72-1.09-.73.08-.72.08-.72 1.2.08 1.84 1.22 1.84 1.22 1.07 1.8 2.8 1.28 3.49.98.1-.77.42-1.28.76-1.58-2.67-.3-5.47-1.31-5.47-5.84 0-1.29.47-2.34 1.24-3.17-.13-.3-.54-1.52.12-3.16 0 0 1.01-.32 3.3 1.21a11.6 11.6 0 0 1 6 0c2.28-1.53 3.29-1.21 3.29-1.21.66 1.64.25 2.86.12 3.16.77.83 1.24 1.88 1.24 3.17 0 4.54-2.81 5.53-5.49 5.83.43.36.81 1.09.81 2.2v3.26c0 .31.21.67.82.56A11.8 11.8 0 0 0 24 12.29C24 5.78 18.63.5 12 .5z" }
                    }
                }
            }
        }
    }
}

#[component]
fn Footer() -> Element {
    rsx! {
        footer { class: "mt-8 border-t border-base-300",
            div { class: "mx-auto flex w-full max-w-7xl flex-col items-center justify-between gap-3 px-4 py-6 text-sm text-base-content/55 sm:flex-row sm:px-6",
                span { "A docs-by-example gallery for the dioform library." }
                div { class: "flex items-center gap-4",
                    a {
                        class: "hover:text-base-content",
                        href: "https://github.com/sagikazarmark/dioform",
                        "Repository"
                    }
                }
            }
        }
    }
}

/// Whether `target` should be highlighted given the `current` route.
fn nav_active(current: &Route, target: &Route) -> bool {
    current == target
}

/// A nav `Link` whose active styling is driven by the parsed route.
#[component]
fn NavLink(route: Route, #[props(into)] label: String, #[props(into)] class: String) -> Element {
    let current = use_route::<Route>();
    let class = if nav_active(&current, &route) {
        format!("{class} bg-primary/10 font-semibold text-primary")
    } else {
        class
    };
    rsx! {
        Link { to: route, class: "{class}", "{label}" }
    }
}

#[component]
fn Sidebar() -> Element {
    rsx! {
        aside { class: "hidden w-60 shrink-0 lg:block",
            nav { class: "sticky top-24 max-h-[calc(100vh-7rem)] space-y-6 overflow-y-auto py-8 pr-2",
                for (section , items) in nav_groups() {
                    div {
                        p { class: "px-3 text-xs font-semibold uppercase tracking-wider text-base-content/45",
                            "{section}"
                        }
                        ul { class: "mt-2 space-y-0.5",
                            for (route , label) in items {
                                li {
                                    NavLink {
                                        route,
                                        label,
                                        class: "block rounded-lg px-3 py-1.5 text-sm text-base-content/75 transition-colors hover:bg-base-200 hover:text-base-content",
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn MobileNav() -> Element {
    rsx! {
        nav { class: "border-b border-base-300 bg-base-100 lg:hidden",
            div { class: "mx-auto flex w-full max-w-7xl gap-1 overflow-x-auto px-4 py-2 sm:px-6",
                for (_section , items) in nav_groups() {
                    for (route , label) in items {
                        NavLink {
                            route,
                            label,
                            class: "whitespace-nowrap rounded-lg px-3 py-1.5 text-sm text-base-content/75 hover:bg-base-200",
                        }
                    }
                }
            }
        }
    }
}
