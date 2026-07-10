#![allow(dead_code)]

use dioform::{FieldPath, Form};

mod account {
    use super::{FieldPath, Form};

    #[derive(Form)]
    pub struct AccountForm {
        secret: String,
        pub email: String,
    }

    pub fn secret_path() -> FieldPath<AccountForm, String> {
        AccountForm::fields().secret()
    }

    pub fn check_private_field_access() {
        let secret = AccountForm::fields().secret();
        let account = AccountForm {
            secret: "token".to_owned(),
            email: "ada@example.com".to_owned(),
        };

        assert_eq!(secret.identity().as_str(), "secret");
        assert_eq!(secret.field_name(), "secret");
        assert_eq!(secret.get(&account).as_str(), "token");
    }
}

fn main() {
    let _secret: FieldPath<account::AccountForm, String> = account::secret_path();

    account::check_private_field_access();
}
