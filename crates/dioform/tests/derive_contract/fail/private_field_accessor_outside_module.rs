#![allow(dead_code)]

use dioform::Form;

mod account {
    use super::Form;

    #[derive(Form)]
    pub struct AccountForm {
        secret: String,
        pub email: String,
    }
}

fn main() {
    let fields = account::AccountForm::fields();

    let _secret = fields.secret();
}
