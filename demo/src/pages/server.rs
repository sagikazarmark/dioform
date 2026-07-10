use dioxus::prelude::*;

use crate::examples::server_validation::ServerValidationExample;
use crate::ui::{DocsCallout, ExampleSection, InlineCode, PageHeader, snippet_theme};
use dioxus_code::{Code, code};

#[component]
pub fn ServerValidation() -> Element {
    rsx! {
        PageHeader {
            eyebrow: "Server",
            title: "Server validation",
            intro: "The dioform-fullstack adapter routes managed submission through a #[server] function. Application rejections map into structured field submit errors; transport failures stay on a separate path.",
        }
        ExampleSection {
            title: "managed_submit().on_submit_server_fn(...)",
            intro: rsx! {
                "The submit calls a "
                InlineCode { "#[server]" }
                " function that checks the email on the server. "
                InlineCode { "taken@example.com" }
                " comes back as a typed rejection and lands on the email field; a broken connection would land on the form instead."
            },
            demo: rsx! { ServerValidationExample {} },
            code: rsx! {
                Code { src: code!("src/examples/server_validation.rs"), theme: snippet_theme() }
            },
        }
        DocsCallout {
            title: "The #[server] function",
            doc_label: "src/server_api.rs",
            doc_href: "https://github.com/sagikazarmark/dioform/blob/main/demo/src/server_api.rs",
            "The server side returns ServerSubmitOutcome::rejected / accepted. On the client the same call becomes a network request; on the server it runs the body."
        }
    }
}
