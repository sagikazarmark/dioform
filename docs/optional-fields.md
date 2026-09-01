# Optional Fields

An **Optional Field** is a **Field** whose value may be absent. A bare `FieldPath<Model, Option<Inner>>` addresses the whole `Option` and refuses traversal: nothing produces `&Inner` from an absent value, so nested values are never implicitly created by traversal.

Choose the editing surface by shape. For an optional record whose inner fields need separate paths,
derive total inner paths with `FieldPath::or` as described below. For an optional scalar edited by
one control, write the `Option`-typed path directly with `use_optional_text`,
`use_optional_number`, or `use_optional_date`; see [Dioxus Adapter Input Helpers](input-helpers.md).
Those helpers perform the sanctioned presence write, so `""` to `None` and back is total and
reversible rather than subject to the record-materialization ratchet.

`FieldPath::or` derives a **total** path through an optional field. The caller supplies the value that stands in for absence, which is what makes the materialization opt-in and named rather than invented by traversal:

```rust
use dioxus::prelude::*;
use dioform::{FieldGroup, Form, FormHandle};

#[derive(Clone, Form)]
pub struct Transaction {
    pub reference: String,
    pub counterparty: Option<Party>,
}

#[derive(Clone, Default, Form, FieldGroup)]
pub struct Party {
    pub name: String,
    pub account: String,
}

static ABSENT_PARTY: Party = Party {
    name: String::new(),
    account: String::new(),
};

fn counterparty_editor(form: FormHandle<Transaction>) -> Element {
    let counterparty = Transaction::fields().counterparty();
    let fields = Party::mount(counterparty.clone().or(&ABSENT_PARTY));
    let name = form.text(fields.name());
    let account = form.text(fields.account());

    rsx! {
        fieldset {
            input { name: name.name(), value: name.value(), oninput: name.oninput() }
            input { name: account.name(), value: account.value(), oninput: account.oninput() }
            button {
                disabled: !counterparty.is_present(&form.snapshot()),
                onclick: move |_| form.set_user_field(counterparty.clone(), None),
                "Clear counterparty"
            }
        }
    }
}
```

The result is an ordinary **Field Path**. It composes with `join`, `#[derive(FieldGroup)]` mounting, bindings, validators, listeners, state snapshots, and submission, and it needs no new method anywhere.

## What the derived path does

- Reading through an absent parent yields the supplied fallback. Reading through a present parent yields the stored value.
- Writing through an absent parent materializes a clone of that same fallback and then applies the write. One value answers both halves, so the read and the write cannot diverge.
- Writing an inner field of a present parent leaves the parent's other fields alone. A `Party` picked from a lookup keeps its account when only its name is edited.
- The derived path keeps the parent's **Field Identity** and rendered **Field Name**, so a joined path renders as `counterparty.name`, exactly as the equivalent non-optional nesting would.

Because identity is shared, the `Option`-typed path and the derived path are two views of one field: they share touched, blurred, version, and submit-error state. Each validator captures its own typed path, so the right closure still runs against the right value. That shared identity also puts the derived path and its inner fields in **Field Ancestry**, so toggling presence notifies the inner bindings and an inner edit notifies whatever reads presence.

Read-shaped operations never materialize. `field_value`, `is_dirty`, `is_field_dirty`, `state_snapshot`, `mark_field_touched`, `mark_field_blurred`, `validate_all`, `validate_field`, `validate_for_submit`, `submit`, and `reset_field` all leave an absent value absent.

The bound is `Clone`, not `Default`. `#[derive(Form)]` emits no bounds of its own, so a model may hold an optional field whose inner type has neither.

Optional traversal composes with itself. Chain `or` for an optional record inside an optional record — given a `Party` that holds an `address: Option<PostalAddress>` — and for `Option<Option<Inner>>`:

```rust
let city = Transaction::fields()
    .counterparty()
    .or(&ABSENT_PARTY)
    .join(Party::fields().address())
    .or(&ABSENT_ADDRESS)
    .join(PostalAddress::fields().city());
```

## Reading presence honestly

A total path erases the difference between an absent value and a present one holding the fallback. Use the `Option`-typed path where that difference matters:

```rust
let counterparty = Transaction::fields().counterparty();

counterparty.is_present(&model);   // bool
counterparty.get_present(&model);  // Option<&Party>
```

Both take a `&Model`, so they read directly from a validator context, a listener, or a submission snapshot. Through a `FormHandle` the cheaper reactive read is `form.field_value(counterparty)`, which clones the `Option<Party>` alone rather than the whole model.

Presence is set the ordinary way, by writing the `Option`-typed path: `set_user_field(counterparty, None)` clears the record, and `set_user_field(counterparty, Some(party))` sets it whole.

Optional scalar input helpers use this same direct write. `optional_text` deliberately collapses
empty rendered input to `None`: `None` and `Some("")` both render as `""`, but the next empty input
writes `None`. That collapse is scalar presence semantics, not traversal or implicit construction.

With the `dioxus-field` feature, that same collapse is what an optional-text binding speaks to the
Field Convention: `TextField { context: use_optional_text(&form, path) }` binds a Widget Registry
text control in one line, with `None` rendered as `""` and empty input written back as `None` —
alongside `use_optional_number` and `use_optional_date`, which reach text controls as rendered text
too. See [Dioxus Adapter Input Helpers](input-helpers.md#optional-scalar-text) and
[ADR-0053](adr/0053-bind-optional-text-to-the-field-convention-as-rendered-text.md).

## Materialization is a ratchet

Materialization is one-way, and this is the price of the design rather than a defect. Clearing an inner value does not un-materialize the parent.

- **Type-then-backspace leaves a phantom.** Typing one character into an inner field of an absent parent and deleting it again leaves a present record holding the fallback. Every rendered field reports clean while the form reports dirty, and the submitted payload carries a defaulted record where omission may have been expected. Clear the parent explicitly to get back to absence.
- **Validators on inner paths run while the parent is absent.** A required rule on `counterparty.name` reports invalid and blocks submit for a section the user never opened. There is no field-scoped way around this: skipping the rule when the parent is absent means consulting `ValidatorContext::form()` from a form validator, which reaches around the abstraction. Attaching the rule to the `Option`-typed path instead keeps it honest at the cost of a coarser error target.

Do not try to recover absence with a listener that collapses a present-but-default record back to `None`. It is unsound: it cannot tell "the user cleared the last field" from "this section is legitimately present and empty", so it deletes sections that were present in the baseline.

Presence as a first-class, metadata-carrying concept — with its own identity, validation, and dirty state, the way **Collection Fields** model item existence — is deliberately deferred, not cancelled. See [ADR-0021](adr/0021-traverse-optional-fields-with-a-named-path-combinator.md) for the decision and [`docs/research/optional-field-traversal.md`](research/optional-field-traversal.md) for the prior-art survey behind it.
