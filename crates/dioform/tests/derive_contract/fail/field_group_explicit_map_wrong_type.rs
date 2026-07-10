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

#[derive(Clone, Debug, Form)]
struct ProfileForm {
    street_line: String,
    locality: String,
    postcode: u32,
}

fn main() {
    let _fields = shared::AddressFieldGroupMap {
        street: ProfileForm::fields().street_line(),
        city: ProfileForm::fields().locality(),
        zip: ProfileForm::fields().postcode(),
    };
}
