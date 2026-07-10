#![allow(dead_code)]

use dioform::{FieldGroup, Form};

mod shared {
    use super::{FieldGroup, Form};

    #[derive(Clone, Debug, Form, FieldGroup)]
    pub struct Address {
        pub street: String,
        pub city: String,
        pub zip: String,
    }
}

#[derive(Form)]
struct ProfileForm {
    street_line: String,
    locality: String,
    postcode: String,
}

fn main() {
    let fields = shared::AddressFieldGroupMap {
        street: ProfileForm::fields().street_line(),
        city: ProfileForm::fields().locality(),
        zip: ProfileForm::fields().postcode(),
    };
    let cloned = fields.clone();

    assert_eq!(cloned.street().identity().as_str(), "street_line");
}
