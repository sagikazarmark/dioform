//! Dioform-specific presentation shared by feature examples.

use dioxus::prelude::*;

/// Key/value readout rendered as a definition grid.
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
