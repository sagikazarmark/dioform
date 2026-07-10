#![allow(dead_code)]

use dioform::{FieldPath, Form};

#[derive(Form)]
struct ProfileForm {
    #[form(name = "contact-email")]
    email: String,
    #[form(name = "accepted_terms")]
    accepts_terms: bool,
    #[form(skip)]
    internal_token: String,
}

fn main() {
    let fields = ProfileForm::fields();
    let email: FieldPath<ProfileForm, String> = fields.email();
    let accepts_terms: FieldPath<ProfileForm, bool> = fields.accepts_terms();
    let form = ProfileForm {
        email: "ada@example.com".to_owned(),
        accepts_terms: true,
        internal_token: "not-a-field".to_owned(),
    };

    assert_eq!(email.identity().as_str(), "email");
    assert_eq!(email.field_name(), "contact-email");
    assert_eq!(email.get(&form).as_str(), "ada@example.com");

    assert_eq!(accepts_terms.identity().as_str(), "accepts_terms");
    assert_eq!(accepts_terms.field_name(), "accepted_terms");
    assert!(*accepts_terms.get(&form));
}
