#![allow(dead_code)]

use dioform::Form;

#[derive(Form)]
struct UnsupportedAttributeForm {
    #[form(widget = "text")]
    email: String,
}

fn main() {}
