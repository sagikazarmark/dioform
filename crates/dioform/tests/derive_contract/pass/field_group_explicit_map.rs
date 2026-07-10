#![allow(dead_code)]

use dioform::{FieldGroup, Form, FormHandle};

mod shared {
    use super::{FieldGroup, Form};

    #[derive(Clone, Debug, Form, FieldGroup)]
    pub struct Address {
        pub street: String,
        pub city: String,
        pub zip: String,
    }
}

#[derive(Clone, Debug, Form)]
struct CheckoutForm {
    billing: shared::Address,
}

#[derive(Clone, Debug, Form)]
struct ProfileForm {
    street_line: String,
    locality: String,
    postcode: String,
}

fn edit_street<Model>(
    handle: &FormHandle<Model>,
    fields: shared::AddressFieldGroupMap<Model>,
    value: &'static str,
) where
    Model: 'static,
{
    handle.text(fields.street()).on_input(value);
}

fn main() {
    let checkout_fields = shared::Address::mount(CheckoutForm::fields().billing());
    let profile_fields = shared::AddressFieldGroupMap {
        street: ProfileForm::fields().street_line(),
        city: ProfileForm::fields().locality(),
        zip: ProfileForm::fields().postcode(),
    };
    let checkout = FormHandle::new(CheckoutForm {
        billing: shared::Address {
            street: "12 Analytical Engine Way".to_owned(),
            city: "London".to_owned(),
            zip: "EC1A 1BB".to_owned(),
        },
    });
    let profile = FormHandle::new(ProfileForm {
        street_line: "1 Lambda Lane".to_owned(),
        locality: "Oxford".to_owned(),
        postcode: "OX1 1AA".to_owned(),
    });

    assert_eq!(checkout_fields.street().identity().as_str(), "billing.street");
    assert_eq!(profile_fields.street().identity().as_str(), "street_line");
    assert_eq!(profile_fields.street().field_name(), "street_line");
    assert_eq!(profile_fields.city().identity().as_str(), "locality");
    assert_eq!(profile_fields.zip().identity().as_str(), "postcode");

    edit_street(&checkout, checkout_fields, "34 Compiler Lane");
    edit_street(&profile, profile_fields, "2 Parser Place");

    assert_eq!(checkout.snapshot().billing.street, "34 Compiler Lane");
    assert_eq!(profile.snapshot().street_line, "2 Parser Place");
}
