#![allow(dead_code)]

use dioform::FieldGroup;

#[derive(FieldGroup)]
struct GenericGroup<T> {
    value: T,
}

fn main() {}
