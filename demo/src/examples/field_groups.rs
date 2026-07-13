use dioform::prelude::*;
use dioxus::prelude::*;

use crate::components::DemoSurface;

/// `#[derive(FieldGroup)]` generates a typed field-group map so a reusable
/// cluster of fields can be rendered once and mounted anywhere. Here the same
/// `Address` group is mounted under two different paths, `billing` and
/// `shipping`, and one `address_fields` renderer serves both. The group owns
/// no validation or identity of its own; it is purely a reusable path bundle.
#[derive(Clone, Debug, Default, PartialEq, Form, FieldGroup)]
struct Address {
    street: String,
    city: String,
    zip: String,
}

#[derive(Clone, Debug, Default, PartialEq, Form)]
struct CheckoutForm {
    billing: Address,
    shipping: Address,
}

fn address_fields(
    form: &FormHandle<CheckoutForm>,
    fields: AddressFieldGroupMap<CheckoutForm>,
    legend: &'static str,
) -> Element {
    let street = form.text(fields.street());
    let city = form.text(fields.city());
    let zip = form.text(fields.zip());

    let street_oninput = street.clone();
    let city_oninput = city.clone();
    let zip_oninput = zip.clone();

    rsx! {
        fieldset { class: "rounded-xl border border-base-300 p-4",
            legend { class: "px-1 text-sm font-semibold", "{legend}" }
            div { class: "space-y-2",
                input {
                    class: "input input-bordered input-sm w-full",
                    placeholder: "Street",
                    name: street.name(),
                    value: street.value(),
                    oninput: move |e| street_oninput.on_input(e.value()),
                }
                div { class: "flex gap-2",
                    input {
                        class: "input input-bordered input-sm flex-1",
                        placeholder: "City",
                        name: city.name(),
                        value: city.value(),
                        oninput: move |e| city_oninput.on_input(e.value()),
                    }
                    input {
                        class: "input input-bordered input-sm w-24",
                        placeholder: "ZIP",
                        name: zip.name(),
                        value: zip.value(),
                        oninput: move |e| zip_oninput.on_input(e.value()),
                    }
                }
            }
        }
    }
}

#[component]
pub fn FieldGroupsExample() -> Element {
    let form = use_form(CheckoutForm::default());

    let billing = Address::mount(CheckoutForm::fields().billing());
    let shipping = Address::mount(CheckoutForm::fields().shipping());

    rsx! {
        DemoSurface {
            primary: rsx! { {address_fields(&form, billing, "Billing address")} },
            secondary: rsx! { {address_fields(&form, shipping, "Shipping address")} },
        }
    }
}
