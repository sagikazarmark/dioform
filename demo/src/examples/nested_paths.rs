use dioxus::prelude::*;
use dioform::prelude::*;

use crate::ui::StateGrid;

/// Nested named structs compose typed paths with `FieldPath::join`, so field
/// access stays fully typed all the way down. Rendered **field names** are a
/// separate concern: `#[form(rename_all = "camelCase")]` and per-field
/// `#[form(name = "...")]` change the HTML `name` without touching the durable
/// **field identity** used for validation and state.
#[derive(Clone, Debug, Default, PartialEq, Form)]
struct Account {
    profile: Profile,
    billing: Billing,
}

#[derive(Clone, Debug, Default, PartialEq, Form)]
#[form(rename_all = "camelCase")]
struct Profile {
    first_name: String,
    #[form(name = "family-name")]
    last_name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Form)]
struct Billing {
    street: String,
}

#[component]
pub fn NestedPathsExample() -> Element {
    let form = use_form(Account::default());

    let profile = Account::fields().profile();
    let first_name_path = profile.clone().join(Profile::fields().first_name());
    let last_name_path = profile.join(Profile::fields().last_name());
    let street_path = Account::fields().billing().join(Billing::fields().street());

    let first_name = form.text(first_name_path.clone());
    let last_name = form.text(last_name_path.clone());
    let street = form.text(street_path.clone());

    let first_name_oninput = first_name.clone();
    let last_name_oninput = last_name.clone();
    let street_oninput = street.clone();

    rsx! {
        div { class: "space-y-3",
            input {
                class: "input input-bordered w-full",
                placeholder: "First name",
                name: first_name.name(),
                value: first_name.value(),
                oninput: move |e| first_name_oninput.on_input(e.value()),
            }
            input {
                class: "input input-bordered w-full",
                placeholder: "Last name",
                name: last_name.name(),
                value: last_name.value(),
                oninput: move |e| last_name_oninput.on_input(e.value()),
            }
            input {
                class: "input input-bordered w-full",
                placeholder: "Billing street",
                name: street.name(),
                value: street.value(),
                oninput: move |e| street_oninput.on_input(e.value()),
            }
        }
        div { class: "mt-5 border-t border-base-300 pt-4",
            p { class: "mb-2 text-xs font-semibold uppercase tracking-wider text-base-content/45",
                "rendered field name (HTML) ← → field identity (durable)"
            }
            StateGrid {
                rows: vec![
                    (
                        "first_name",
                        format!(
                            "{}  ·  {}",
                            first_name_path.field_name(),
                            first_name_path.identity().as_str(),
                        ),
                    ),
                    (
                        "last_name",
                        format!(
                            "{}  ·  {}",
                            last_name_path.field_name(),
                            last_name_path.identity().as_str(),
                        ),
                    ),
                    (
                        "street",
                        format!(
                            "{}  ·  {}",
                            street_path.field_name(),
                            street_path.identity().as_str(),
                        ),
                    ),
                ],
            }
        }
    }
}
