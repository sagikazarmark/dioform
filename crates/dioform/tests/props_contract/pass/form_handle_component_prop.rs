#![allow(dead_code)]

use dioform::prelude::*;
use dioxus::prelude::*;

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct CheckoutForm {
    email: String,
}

#[component]
fn EmailField(form: FormHandle<CheckoutForm>) -> Element {
    let email = form.text(CheckoutForm::fields().email());

    rsx! {
        input { name: email.name(), value: email.value() }
    }
}

#[component]
fn Checkout() -> Element {
    let form = use_form(CheckoutForm {
        email: String::new(),
    });

    rsx! {
        EmailField { form }
    }
}

// A model and an error type that are not comparable themselves, to hold the handle's equality free
// of bounds on either.
#[derive(Clone, Form)]
struct UploadForm {
    caption: String,
}

#[derive(Clone)]
struct UploadError;

#[component]
fn CaptionField(form: FormHandle<UploadForm, UploadError>) -> Element {
    let caption = form.text(UploadForm::fields().caption());

    rsx! {
        input { name: caption.name(), value: caption.value() }
    }
}

fn main() {}
