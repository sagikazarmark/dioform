#![allow(dead_code)]

use dioform::{FieldPath, Form};

#[derive(Form)]
struct LocationForm {
    point: (i32, i32),
}

fn main() {
    let x: FieldPath<LocationForm, i32> = LocationForm::fields().point()._0();

    let _ = x;
}
