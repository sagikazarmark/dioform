#![allow(dead_code)]

use dioform::{FieldGroup, FieldPath, Form};

// `#[derive(Form)]` stays bound-free: an optional field whose inner type implements neither `Clone`
// nor `Default` still derives, because the optional-traversal bound lives at the call site.
#[derive(Debug, Form)]
struct AuditForm {
    receipt: Option<Receipt>,
}

#[derive(Debug)]
struct Receipt {
    id: String,
}

#[derive(Clone, Debug, Form)]
struct TransferForm {
    counterparty: Option<Party>,
}

// The combinator asks for `Clone`, not `Default`.
#[derive(Clone, Debug, FieldGroup, Form)]
struct Party {
    name: String,
    account: String,
}

static ABSENT_PARTY: Party = Party {
    name: String::new(),
    account: String::new(),
};

fn main() {
    let _receipt: FieldPath<AuditForm, Option<Receipt>> = AuditForm::fields().receipt();
    let counterparty = TransferForm::fields().counterparty();
    let name: FieldPath<TransferForm, String> = counterparty
        .clone()
        .or(&ABSENT_PARTY)
        .join(Party::fields().name());
    let mounted = Party::mount(counterparty.or(&ABSENT_PARTY));
    let _account: FieldPath<TransferForm, String> = mounted.account();

    assert_eq!(name.identity().as_str(), "counterparty.name");
    assert_eq!(name.field_name(), "counterparty.name");
}
