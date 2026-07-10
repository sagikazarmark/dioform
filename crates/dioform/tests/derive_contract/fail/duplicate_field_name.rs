#![allow(dead_code)]

use dioform::Form;

#[derive(Form)]
struct DuplicateFieldNameForm {
    email: String,
    #[form(name = "email")]
    contact_email: String,
}

fn main() {}
