#![allow(dead_code)]

use dioform::Form;

#[derive(Form)]
struct SessionForm {
    email: String,
    #[form(skip)]
    internal_token: String,
}

fn main() {
    let fields = SessionForm::fields();

    let _internal_token = fields.internal_token();
}
