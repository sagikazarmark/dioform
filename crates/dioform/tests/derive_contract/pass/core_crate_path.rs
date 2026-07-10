#![allow(dead_code)]

use dioform_core::{FieldPath, Form as CoreForm};
use dioform_derive::Form;

#[derive(Form)]
#[form(crate = "::dioform_core")]
struct SignupForm {
    email: String,
}

fn main() {
    let fields = <SignupForm as CoreForm>::fields();
    let email: FieldPath<SignupForm, String> = fields.email();
    let form = SignupForm {
        email: "ada@example.com".to_owned(),
    };

    assert_eq!(email.identity().as_str(), "email");
    assert_eq!(email.get(&form).as_str(), "ada@example.com");
}
