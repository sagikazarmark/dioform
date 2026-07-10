use dioform::advanced::{FieldUpdateOrigin, FormObserverEvent, FormObserverValue};

fn exhaustive_event_match(event: FormObserverEvent) -> &'static str {
    match event {
        FormObserverEvent::FieldUpdated { .. } => "field",
        FormObserverEvent::ValidationRan { .. } => "validation",
        FormObserverEvent::SubmitAttempted { .. } => "submit",
        FormObserverEvent::Reset { .. } => "reset",
        FormObserverEvent::Reinitialized { .. } => "reinitialized",
    }
}

fn exact_variant_field_match(event: FormObserverEvent) {
    match event {
        FormObserverEvent::FieldUpdated {
            field,
            origin,
            value,
        } => {
            let _ = (field, origin, value);
        }
        _ => {}
    }
}

fn exhaustive_value_match(value: FormObserverValue) -> &'static str {
    match value {
        FormObserverValue::Redacted => "redacted",
    }
}

fn exhaustive_origin_match(origin: FieldUpdateOrigin) -> &'static str {
    match origin {
        FieldUpdateOrigin::Programmatic => "programmatic",
        FieldUpdateOrigin::User => "user",
    }
}

fn main() {}
