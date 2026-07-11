use dioform::prelude::*;
use dioxus::prelude::*;

/// `item_field_validator` registers a validator that runs against *each* row's
/// field. Errors are stored per item identity, so they follow a row when it is
/// reordered or removed: index 1's error never leaks onto whatever ends up at
/// index 1 next. Read a row's errors from its own binding with
/// `visible_validation_errors()`.
#[derive(Clone, Debug, PartialEq, Form)]
struct GuestList {
    guests: Vec<Guest>,
}

#[derive(Clone, Debug, PartialEq, Form)]
struct Guest {
    name: String,
}

#[component]
pub fn CollectionValidationExample() -> Element {
    let form = use_form_handle(|| {
        let handle = FormHandle::<GuestList>::from_config(
            FormConfig::new(GuestList {
                guests: vec![
                    Guest { name: "Ada".into() },
                    Guest {
                        name: String::new(),
                    },
                ],
            })
            .validation_mode(ValidationMode::on_change()),
        );
        handle
            .collection(GuestList::fields().guests())
            .item_field_validator(Guest::fields().name(), "name-required")
            .check(|value, _ctx| {
                if value.trim().is_empty() {
                    vec!["Every guest needs a name.".to_string()]
                } else {
                    Vec::new()
                }
            });
        handle
    });

    let guests = form.collection(GuestList::fields().guests());
    let items = guests.items();
    let add = guests.clone();

    rsx! {
        button {
            class: "btn btn-sm btn-primary",
            onclick: move |_| { add.append(Guest { name: String::new() }); },
            "Add guest"
        }
        ul { class: "mt-4 space-y-3",
            for item in items.iter().cloned() {
                {
                    let index = item.index();
                    let name = item.text(Guest::fields().name());
                    let name_oninput = name.clone();
                    let errors = name.visible_validation_errors();
                    let remove = guests.clone();
                    let id = item.identity();
                    rsx! {
                        li { key: "{index}",
                            div { class: "flex items-center gap-2",
                                input {
                                    class: "input input-bordered input-sm flex-1",
                                    name: name.name(),
                                    value: name.value(),
                                    oninput: move |e| name_oninput.on_input(e.value()),
                                    onblur: move |_| name.on_blur(),
                                }
                                button {
                                    class: "btn btn-xs btn-outline btn-error",
                                    onclick: move |_| { remove.remove(id); },
                                    "remove"
                                }
                            }
                            for error in errors {
                                p { class: "mt-1 text-sm text-error", "{error.error()}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
