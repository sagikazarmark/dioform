use dioform::{Form, FormHandle};

#[derive(Clone, Form)]
struct Model {
    reference: Option<String>,
}

fn main() {
    let form = FormHandle::new(Model { reference: None });
    let reference = form.optional_text(Model::fields().reference());

    let _ = reference.parse_error();
}
