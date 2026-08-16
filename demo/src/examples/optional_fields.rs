use dioform::prelude::*;
use dioxus::prelude::*;

use super::StateGrid;
use crate::components::{DemoPane, DemoSurface};

/// A bare path to an `Option<T>` addresses the whole optional value and refuses
/// traversal. `FieldPath::or` derives a **total** path through it from a
/// fallback supplied at the call site: reading an absent counterparty yields
/// that fallback, and writing one materializes a clone of it. The derived path
/// keeps the parent's field identity, so it renders as `counterparty.name` and
/// mounts a field group like any other nested path.
///
/// Presence stays readable through the `Option`-typed path with `is_present`
/// and `get_present`. Materialization is one-way; see `docs/optional-fields.md`
/// for the ratchet that comes with it.
#[derive(Clone, Debug, Default, PartialEq, Form, FieldGroup)]
struct Party {
    name: String,
    #[form(name = "account-number")]
    account: String,
}

#[derive(Clone, Debug, Default, PartialEq, Form)]
struct Transfer {
    counterparty: Option<Party>,
}

static ABSENT_PARTY: Party = Party {
    name: String::new(),
    account: String::new(),
};

#[component]
pub fn OptionalFieldsExample() -> Element {
    let form = use_form(Transfer::default());

    let counterparty_path = Transfer::fields().counterparty();
    let fields = Party::mount(counterparty_path.clone().or(&ABSENT_PARTY));

    let name = form.text(fields.name());
    let account = form.text(fields.account());
    let name_oninput = name.clone();
    let account_oninput = account.clone();

    let transfer = form.snapshot();
    let is_present = counterparty_path.is_present(&transfer);
    let counterparty = counterparty_path.get_present(&transfer).cloned();
    let clear = form.clone();
    let clear_path = counterparty_path.clone();
    let pick = form.clone();
    let pick_path = counterparty_path.clone();

    rsx! {
        DemoSurface {
            primary: rsx! {
                DemoPane { label: "Counterparty (optional record)",
                    div { class: "space-y-3",
                        label { class: "block",
                            span { class: "mb-1 block text-sm font-medium", "Name" }
                            input {
                                class: "input input-bordered w-full",
                                name: name.name(),
                                value: name.value(),
                                oninput: move |e| name_oninput.on_input(e.value()),
                            }
                        }
                        label { class: "block",
                            span { class: "mb-1 block text-sm font-medium", "Account" }
                            input {
                                class: "input input-bordered w-full",
                                name: account.name(),
                                value: account.value(),
                                oninput: move |e| account_oninput.on_input(e.value()),
                            }
                        }
                    }

                    div { class: "mt-4 flex flex-wrap gap-2",
                        button {
                            class: "btn btn-sm btn-outline",
                            onclick: move |_| {
                                pick.set_user_field(
                                    pick_path.clone(),
                                    Some(Party {
                                        name: "Ada Lovelace".into(),
                                        account: "GB29 NWBK 6016 1331 9268 19".into(),
                                    }),
                                );
                            },
                            "Pick counterparty"
                        }
                        button {
                            class: "btn btn-sm btn-ghost",
                            onclick: move |_| clear.set_user_field(clear_path.clone(), None),
                            "Set null"
                        }
                    }
                }
            },
            secondary: rsx! {
                DemoPane { label: "Presence stays observable",
                    StateGrid {
                        rows: vec![
                            ("counterparty.is_present()", is_present.to_string()),
                            ("counterparty.get_present()", format!("{counterparty:?}")),
                            ("name.name() (rendered)", name.name().to_string()),
                            ("account.name() (rendered)", account.name().to_string()),
                            ("form.is_dirty()", form.is_dirty().to_string()),
                        ],
                    }
                }
            },
        }
    }
}
