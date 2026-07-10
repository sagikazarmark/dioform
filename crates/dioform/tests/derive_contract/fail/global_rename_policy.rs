#![allow(dead_code)]

use dioform::Form;

#[derive(Form)]
#[form(rename_all = "PascalCase")]
struct GlobalRenameForm {
    first_name: String,
}

fn main() {}
