# Use a renderer-agnostic form core with Dioxus adapter and derive macro crates

Dioform will start as a Cargo workspace with separate crates for renderer-agnostic form behavior, Dioxus integration, and `#[derive(Form)]` support. This preserves a headless core that can be tested and reused without Dioxus, keeps Dioxus signal/event/hydration concerns in the adapter, and avoids making validation libraries required dependencies of the core. A single crate would be simpler initially, but it would blur dependency boundaries that are central to the library's design.
