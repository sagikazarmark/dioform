#![allow(dead_code)]

use std::mem::ManuallyDrop;

use dioform::Form;

#[derive(Form)]
union UnsafeForm {
    text: ManuallyDrop<String>,
}

fn main() {}
