use dioxus::prelude::*;
use dioform::prelude::*;

/// Listeners are application-owned side-effect hooks for semantic form events
/// (autosave, analytics, dependent-field resets), kept out of validators. A
/// **form-level** listener here logs every field replacement (by name, never by
/// value). An **origin-filtered field** listener resets `region` whenever the
/// user changes `country`, without re-entering itself on its own write.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct AddressForm {
    country: String,
    region: String,
    note: String,
}

#[component]
pub fn ObserversExample() -> Element {
    let form = use_form(AddressForm {
        country: "us".into(),
        ..Default::default()
    });
    let mut log = use_signal(Vec::<String>::new);

    // Dependent reset: changing the country clears the region.
    use_field_listener_for_origin(
        form.clone(),
        AddressForm::fields().country(),
        FieldUpdateOrigin::User,
        move |ctx| {
            ctx.form()
                .set_field(AddressForm::fields().region(), String::new());
        },
    );

    // Analytics-style log of every field replacement.
    use_form_listener(form.clone(), move |ctx| {
        let entry = format!("replaced: {}", ctx.field_name());
        let mut log = log.write();
        log.push(entry);
        let len = log.len();
        if len > 8 {
            log.drain(0..len - 8);
        }
    });

    let fields = AddressForm::fields();
    let country = form.select(fields.country());
    let region = form.text(fields.region());
    let note = form.text(fields.note());

    let country_onchange = country.clone();
    let region_oninput = region.clone();
    let note_oninput = note.clone();

    let entries = log.read().clone();

    rsx! {
        div { class: "space-y-3",
            label { class: "block",
                span { class: "mb-1 block text-sm font-medium", "Country (changing this resets region)" }
                select {
                    class: "select select-bordered w-full",
                    name: country.name(),
                    value: country.value(),
                    onchange: move |e| country_onchange.on_change(e.value()),
                    option { value: "us", selected: country.is_selected(&"us".to_string()), "United States" }
                    option { value: "de", selected: country.is_selected(&"de".to_string()), "Germany" }
                    option { value: "jp", selected: country.is_selected(&"jp".to_string()), "Japan" }
                }
            }
            input {
                class: "input input-bordered w-full",
                placeholder: "Region",
                name: region.name(),
                value: region.value(),
                oninput: move |e| region_oninput.on_input(e.value()),
            }
            input {
                class: "input input-bordered w-full",
                placeholder: "Note",
                name: note.name(),
                value: note.value(),
                oninput: move |e| note_oninput.on_input(e.value()),
            }
        }
        div { class: "mt-4 border-t border-base-300 pt-4",
            p { class: "mb-2 text-xs font-semibold uppercase tracking-wider text-base-content/45", "Listener log" }
            if entries.is_empty() {
                p { class: "text-sm text-base-content/55", "Edit a field to see events." }
            } else {
                ul { class: "space-y-1 font-mono text-xs text-base-content/70",
                    for (i , entry) in entries.iter().enumerate() {
                        li { key: "{i}", "{entry}" }
                    }
                }
            }
        }
    }
}
