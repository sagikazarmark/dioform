#![allow(dead_code)]

use dioform as renamed_form;
use renamed_form::{FieldPath, Form};

#[derive(Form)]
#[form(crate = "crate::renamed_form")]
struct SignupForm {
    email: String,
}

fn main() {
    let fields = SignupForm::fields();
    let email: FieldPath<SignupForm, String> = fields.email();
    let form = SignupForm {
        email: "ada@example.com".to_owned(),
    };

    assert_eq!(email.identity().as_str(), "email");
    assert_eq!(email.get(&form).as_str(), "ada@example.com");
}
