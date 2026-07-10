#![allow(dead_code)]

use dioform::Form;

#[derive(Form)]
struct GenericForm<T> {
    value: T,
}

fn main() {}
