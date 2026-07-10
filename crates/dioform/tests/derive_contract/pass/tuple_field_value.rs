#![allow(dead_code)]

use dioform::{FieldPath, Form};

#[derive(Form)]
struct LocationForm {
    point: (i32, i32),
}

fn main() {
    let point: FieldPath<LocationForm, (i32, i32)> = LocationForm::fields().point();
    let form = LocationForm { point: (12, 34) };

    assert_eq!(point.identity().as_str(), "point");
    assert_eq!(point.field_name(), "point");
    assert_eq!(*point.get(&form), (12, 34));
}
