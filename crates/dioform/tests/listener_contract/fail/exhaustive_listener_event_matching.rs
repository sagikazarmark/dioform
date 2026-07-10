use dioform::{FormListenerEvent, SubmitListenerEvent};

fn exhaustive_form_listener_event_match(event: FormListenerEvent) -> &'static str {
    match event {
        FormListenerEvent::FieldReplaced => "field replaced",
    }
}

fn exhaustive_submit_listener_event_match(event: SubmitListenerEvent) -> &'static str {
    match event {
        SubmitListenerEvent::SubmitAttempted => "attempted",
        SubmitListenerEvent::SubmissionStarted => "started",
        SubmitListenerEvent::SubmitBlocked(_) => "blocked",
        SubmitListenerEvent::SubmissionRejected => "rejected",
        SubmitListenerEvent::SubmissionSucceeded => "succeeded",
    }
}

fn main() {}
