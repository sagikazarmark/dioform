use dioxus::prelude::*;
use dioform::prelude::*;

use super::support::{DateYmd, cents_to_dollars, money, parse_dollars_to_cents};
use crate::ui::{PageHeader, field_select, field_text};

#[derive(Clone, Debug, PartialEq, Form)]
struct InvoiceForm {
    customer: String,
    due_date: DateYmd,
    terms: String,
    lines: Vec<InvoiceLine>,
}

#[derive(Clone, Debug, PartialEq, Form)]
struct InvoiceLine {
    description: String,
    quantity: u32,
    unit_cents: u32,
}

impl InvoiceLine {
    fn total_cents(&self) -> u32 {
        self.quantity.saturating_mul(self.unit_cents)
    }
}

fn initial() -> InvoiceForm {
    InvoiceForm {
        customer: "Analytical Engines Ltd".into(),
        due_date: DateYmd::new(2026, 6, 30),
        terms: "net-14".into(),
        lines: vec![
            InvoiceLine {
                description: "Integration review".into(),
                quantity: 2,
                unit_cents: 12500,
            },
            InvoiceLine {
                description: "Accessibility pass".into(),
                quantity: 1,
                unit_cents: 9800,
            },
        ],
    }
}

fn build() -> FormHandle<InvoiceForm> {
    let form = FormHandle::<InvoiceForm>::from_config(
        FormConfig::new(initial()).validation_mode(ValidationMode::on_blur()),
    );
    let f = InvoiceForm::fields();
    form.write_advanced(|core| {
        core.register_sync_field_validator(f.customer(), "customer", |value: &String, _ctx| {
            if value.trim().is_empty() {
                vec!["Enter a customer.".to_string()]
            } else {
                Vec::new()
            }
        });
    });
    let line = InvoiceLine::fields();
    let lines = form.collection(f.lines());
    lines
        .item_field_validator(line.description(), "line-desc")
        .check(|value, _ctx| {
            if value.trim().is_empty() {
                vec!["Describe this line item.".to_string()]
            } else {
                Vec::new()
            }
        });
    lines
        .item_field_validator(line.quantity(), "line-qty")
        .check(|value: &u32, _ctx| {
            if *value == 0 {
                vec!["Quantity must be at least 1.".to_string()]
            } else {
                Vec::new()
            }
        });
    form
}

fn line_row(
    collection: &CollectionBinding<InvoiceForm, InvoiceLine, String>,
    item: CollectionItemBinding<InvoiceForm, InvoiceLine, String>,
    line: InvoiceLine,
    count: usize,
) -> Element {
    let f = InvoiceLine::fields();
    let index = item.index();
    let description = item.text(f.description());
    let quantity = use_collection_item_number(item.clone(), f.quantity());
    let unit = use_collection_item_number_with(
        item.clone(),
        f.unit_cents(),
        parse_dollars_to_cents,
        |cents| cents_to_dollars(*cents),
    );

    let description_oninput = description.clone();
    let quantity_oninput = quantity.clone();
    let unit_oninput = unit.clone();
    let up = collection.clone();
    let down = collection.clone();
    let remove = collection.clone();
    let id_up = item.identity();
    let id_down = item.identity();
    let id_remove = item.identity();

    let mut errors: Vec<String> = description
        .visible_validation_errors()
        .into_iter()
        .map(|e| e.error().to_string())
        .collect();
    errors.extend(
        quantity
            .visible_validation_errors()
            .into_iter()
            .map(|e| e.error().to_string()),
    );

    rsx! {
        div { class: "rounded-xl border border-base-300 p-3",
            div { class: "grid gap-2 sm:grid-cols-[1fr_5rem_7rem_auto] sm:items-end",
                label { class: "block",
                    span { class: "mb-1 block text-xs text-base-content/55", "Description" }
                    input {
                        class: "input input-bordered input-sm w-full",
                        name: description.name(),
                        value: description.value(),
                        oninput: move |e| description_oninput.on_input(e.value()),
                        onblur: move |_| description.on_blur(),
                    }
                }
                label { class: "block",
                    span { class: "mb-1 block text-xs text-base-content/55", "Qty" }
                    input {
                        class: "input input-bordered input-sm w-full",
                        r#type: "number",
                        min: "0",
                        name: quantity.name(),
                        value: quantity.value(),
                        oninput: move |e| quantity_oninput.on_input(e.value()),
                        onblur: move |_| quantity.on_blur(),
                    }
                }
                label { class: "block",
                    span { class: "mb-1 block text-xs text-base-content/55", "Unit ($)" }
                    input {
                        class: "input input-bordered input-sm w-full",
                        name: unit.name(),
                        value: unit.value(),
                        oninput: move |e| unit_oninput.on_input(e.value()),
                        onblur: move |_| unit.on_blur(),
                    }
                }
                div { class: "flex gap-1",
                    button { class: "btn btn-xs btn-ghost", r#type: "button", disabled: index == 0, onclick: move |_| { up.move_to_index(id_up, index - 1); }, "↑" }
                    button { class: "btn btn-xs btn-ghost", r#type: "button", disabled: index + 1 == count, onclick: move |_| { down.move_to_index(id_down, index + 1); }, "↓" }
                    button { class: "btn btn-xs btn-outline btn-error", r#type: "button", disabled: count <= 1, onclick: move |_| { remove.remove(id_remove); }, "✕" }
                }
            }
            div { class: "mt-1 flex items-center justify-between",
                div {
                    for error in errors {
                        p { class: "text-xs text-error", "{error}" }
                    }
                }
                p { class: "text-sm font-semibold", "Line total {money(line.total_cents())}" }
            }
        }
    }
}

#[component]
pub fn Invoice() -> Element {
    let form = use_form_handle(build);
    let f = InvoiceForm::fields();
    let mut status = use_signal(String::new);

    let customer = form.text(f.customer());
    let due_date = use_date(form.clone(), f.due_date());
    let terms = form.select(f.terms());
    let lines = form.collection(f.lines());
    let submit = form.managed_submit();

    let submit_for_form = submit.clone();
    let lines_for_add = lines.clone();
    let form_for_reset = form.clone();

    let snapshot = form.snapshot();
    let rows = snapshot.lines.clone();
    let items = lines.items();
    let subtotal: u32 = rows.iter().map(InvoiceLine::total_cents).sum();
    let tax = subtotal / 10;
    let total = subtotal + tax;

    let due_date_oninput = due_date.clone();

    rsx! {
        PageHeader {
            eyebrow: "Realistic forms",
            title: "Invoice",
            intro: "Form-owned repeatable line items with parsed quantity and money inputs, reorder and remove, per-item validators, and a live total, all held inside the form draft while the page owns the markup.",
        }
        div { class: "mt-8 grid gap-6 lg:grid-cols-[1fr_15rem]",
            div { class: "rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm",
                form {
                    class: "space-y-4",
                    onsubmit: move |event| {
                        let result = submit_for_form.on_submit(event, |_s: SubmissionSnapshot<InvoiceForm>| SubmitErrors::none());
                        status.set(match result {
                            SubmitResult::Succeeded => "Invoice sent.".to_string(),
                            SubmitResult::Blocked(_) => "Fix the highlighted lines first.".to_string(),
                            other => format!("{other:?}"),
                        });
                    },
                    div { class: "grid gap-4 sm:grid-cols-3",
                        {field_text("Customer", &customer, "text", "Customer name")}
                        label { class: "block space-y-1",
                            span { class: "text-sm font-medium", "Due date" }
                            input {
                                class: "input input-bordered w-full",
                                r#type: "date",
                                name: due_date.name(),
                                value: due_date.value(),
                                oninput: move |e| due_date_oninput.on_input(e.value()),
                                onblur: move |_| due_date.on_blur(),
                            }
                        }
                        {field_select("Terms", &terms, [("net-7", "Net 7"), ("net-14", "Net 14"), ("net-30", "Net 30")])}
                    }
                    div {
                        div { class: "mb-2 flex items-center justify-between",
                            p { class: "text-sm font-semibold", "Line items" }
                            button {
                                class: "btn btn-xs btn-outline",
                                r#type: "button",
                                onclick: move |_| { lines_for_add.append(InvoiceLine { description: String::new(), quantity: 1, unit_cents: 5000 }); },
                                "Add line"
                            }
                        }
                        div { class: "space-y-2",
                            for item in items.iter().cloned() {
                                if let Some(line) = rows.get(item.index()).cloned() {
                                    {line_row(&lines, item, line, rows.len())}
                                }
                            }
                        }
                    }
                    div { class: "flex gap-2 border-t border-base-300 pt-4",
                        button { class: "btn btn-primary", r#type: "submit", "Send invoice" }
                        button {
                            class: "btn btn-ghost",
                            r#type: "button",
                            onclick: move |_| { form_for_reset.reset(); },
                            "Reset"
                        }
                    }
                }
            }
            aside { class: "space-y-2 rounded-2xl border border-base-300 bg-base-200/40 p-5 text-sm",
                p { class: "text-xs font-semibold uppercase tracking-wider text-base-content/45", "Total" }
                div { class: "flex justify-between", span { "Subtotal" } span { "{money(subtotal)}" } }
                div { class: "flex justify-between", span { "Tax (10%)" } span { "{money(tax)}" } }
                div { class: "flex justify-between border-t border-base-300 pt-2 font-bold", span { "Total" } span { "{money(total)}" } }
                if !status.read().is_empty() {
                    p { class: "pt-2 text-base-content/70", "{status}" }
                }
            }
        }
    }
}
