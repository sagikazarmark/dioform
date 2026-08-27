#![allow(dead_code)]

use dioform::{EnumerableStaticFields, Form, ValidationTarget};

#[derive(Form)]
#[form(rename_all = "camelCase")]
struct ProfileForm {
    first_name: String,
    #[form(name = "family-name")]
    last_name: String,
    #[form(skip)]
    internal_token: String,
}

#[test]
fn derived_static_fields_use_rust_identifiers_and_exclude_skipped_fields() {
    let entries = ProfileForm::static_field_entries();

    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.rust_identifier())
            .collect::<Vec<_>>(),
        ["first_name", "last_name"]
    );
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.target())
            .collect::<Vec<_>>(),
        [
            ValidationTarget::field(ProfileForm::fields().first_name()),
            ValidationTarget::field(ProfileForm::fields().last_name()),
        ]
    );
}
