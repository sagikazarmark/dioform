use dioform::prelude::*;
use dioxus::prelude::*;

#[derive(Clone, PartialEq, Form)]
struct RejectedValues {
    count: u32,
}

fn rejected_page() -> Element {
    let form = use_form_config(
        FormConfig::new(RejectedValues { count: 7 })
            .id_namespace("rejected-post")
            .browser_rejection((), |_| {
                BrowserRejection::new(SubmitErrors::new([SubmitError::field(
                    RejectedValues::fields().count(),
                    "server-only rejection".to_owned(),
                )]))
                .raw_field(RejectedValues::fields().count(), "abc")
            }),
    );
    let count = use_number(&form, RejectedValues::fields().count());
    let submit = form.progressive_submit();
    let raw_error = count
        .parse_error()
        .map(|error| error.message().to_owned())
        .unwrap_or_default();
    rsx! {
        form {
            method: "post", action: "/counts",
            onsubmit: move |event| { submit.on_submit(event); },
            input { name: count.name(), value: count.value(), oninput: count.oninput() }
            p { "{raw_error}" }
            for error in count.visible_validation_errors() {
                p { "{error.error()}" }
            }
            button { r#type: "submit", "Retry" }
        }
    }
}

#[test]
fn browser_rejection_server_and_client_initial_html_match() {
    let mut server = VirtualDom::new(rejected_page);
    server.rebuild_in_place();
    let mut client = VirtualDom::new(rejected_page);
    client.rebuild_in_place();
    let server_html = dioxus_ssr::render(&server);
    assert!(server_html.contains("value=\"abc\""));
    assert!(server_html.contains("server-only rejection"));
    assert!(server_html.contains("invalid digit"));
    assert_eq!(server_html, dioxus_ssr::render(&client));
    client.render_immediate_to_vec();
    assert_eq!(server_html, dioxus_ssr::render(&client));
}
