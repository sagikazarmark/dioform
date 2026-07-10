use dioform::prelude::*;

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct ContractForm {
    email: String,
}

fn main() {
    let form = FormHandle::new(ContractForm {
        email: String::new(),
    });
    let email = form.text(ContractForm::fields().email());

    let _ = email.parse_error();
}
