#![allow(dead_code)]

use dioform::{FieldGroup, FieldPath, Form, FormHandle};

#[derive(Clone, Debug, Form)]
struct CheckoutForm {
    billing: Address,
}

#[derive(Clone, Debug, Form, FieldGroup)]
struct Address {
    #[form(name = "street-line")]
    street: String,
    city: String,
    zip: String,
}

fn main() {
    let fields = Address::mount(CheckoutForm::fields().billing());
    let street: FieldPath<CheckoutForm, String> = fields.street();
    let city: FieldPath<CheckoutForm, String> = fields.city();
    let zip: FieldPath<CheckoutForm, String> = fields.zip();
    let handle = FormHandle::new(CheckoutForm {
        billing: Address {
            street: "12 Analytical Engine Way".to_owned(),
            city: "London".to_owned(),
            zip: "EC1A 1BB".to_owned(),
        },
    });
    let street_binding = handle.text(street.clone());

    assert_eq!(street.identity().as_str(), "billing.street");
    assert_eq!(street.field_name(), "billing.street-line");
    assert_eq!(city.identity().as_str(), "billing.city");
    assert_eq!(city.field_name(), "billing.city");
    assert_eq!(zip.identity().as_str(), "billing.zip");
    assert_eq!(zip.field_name(), "billing.zip");
    assert_eq!(street_binding.name(), "billing.street-line");
    assert_eq!(street_binding.value(), "12 Analytical Engine Way");

    street_binding.on_input("34 Compiler Lane");

    assert_eq!(handle.snapshot().billing.street, "34 Compiler Lane");
}
