#![allow(dead_code)]

use dioform::{FieldPath, Form};

#[derive(Form)]
struct SignupForm {
    email: String,
    accepts_terms: bool,
}

fn main() {
    let fields = SignupForm::fields();
    let email: FieldPath<SignupForm, String> = fields.email();
    let accepts_terms: FieldPath<SignupForm, bool> = fields.accepts_terms();
    let mut form = SignupForm {
        email: "ada@example.com".to_owned(),
        accepts_terms: false,
    };

    assert_eq!(email.identity().as_str(), "email");
    assert_eq!(email.field_name(), "email");
    assert_eq!(email.get(&form).as_str(), "ada@example.com");

    *accepts_terms.get_mut(&mut form) = true;

    assert!(*accepts_terms.get(&form));
}
