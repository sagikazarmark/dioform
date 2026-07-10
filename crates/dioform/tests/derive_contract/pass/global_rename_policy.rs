#![allow(dead_code)]

use dioform::Form;

#[derive(Form)]
#[form(rename_all = "camelCase")]
struct ProfileForm {
    first_name: String,
    #[form(name = "family-name")]
    last_name: String,
}

fn main() {
    let first_name = ProfileForm::fields().first_name();
    let last_name = ProfileForm::fields().last_name();

    assert_eq!(first_name.identity().as_str(), "first_name");
    assert_eq!(first_name.field_name(), "firstName");
    assert_eq!(last_name.identity().as_str(), "last_name");
    assert_eq!(last_name.field_name(), "family-name");
}
