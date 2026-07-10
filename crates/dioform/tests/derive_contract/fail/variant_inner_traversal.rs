#![allow(dead_code)]

use dioform::{FieldPath, Form};

#[derive(Form)]
struct CheckoutForm {
    payment: Payment,
}

enum Payment {
    Card { card_number: String },
    Cash,
}

fn main() {
    let card_number: FieldPath<CheckoutForm, String> = CheckoutForm::fields()
        .payment()
        .card_number();

    let _ = card_number;
}
