use dioform::prelude::*;
use dioxus::prelude::*;

use super::StateGrid;
use crate::components::{DemoPane, DemoSurface};

/// Parsed bindings keep the model typed while the `<input>` stays a string. The
/// binding holds the raw text, the last successfully parsed value, and a
/// separate `parse_error`, so unparseable input never corrupts the model and
/// never masquerades as a validation error. `use_number` parses with `FromStr`;
/// `use_number_with` takes a custom parser/formatter (here: dollars ⇆ integer
/// cents).
#[derive(Clone, Debug, PartialEq, Form)]
struct OrderForm {
    quantity: u32,
    unit_price_cents: u32,
}

fn parse_dollars_to_cents(raw: &str) -> Result<u32, String> {
    let raw = raw.trim();
    let value: f64 = raw
        .parse()
        .map_err(|_| "Enter a dollar amount.".to_string())?;
    if value < 0.0 {
        return Err("Price cannot be negative.".to_string());
    }
    Ok((value * 100.0).round() as u32)
}

fn format_cents(cents: u32) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

#[component]
pub fn ParsedInputsExample() -> Element {
    let form = use_form(OrderForm {
        quantity: 1,
        unit_price_cents: 4800,
    });
    let fields = OrderForm::fields();

    let quantity = use_number(form.clone(), fields.quantity());
    let price = use_number_with(
        form.clone(),
        fields.unit_price_cents(),
        parse_dollars_to_cents,
        |cents| format_cents(*cents),
    );

    let snapshot = form.snapshot();
    let line_total = snapshot.quantity.saturating_mul(snapshot.unit_price_cents);

    rsx! {
        DemoSurface {
            primary: rsx! {
                DemoPane { label: "Parsed inputs",
                    div { class: "space-y-4",
                        label { class: "block",
                            span { class: "mb-1 block text-sm font-medium", "Quantity (u32)" }
                            input {
                                class: "input input-bordered w-full",
                                r#type: "number",
                                min: "0",
                                name: quantity.name(),
                                value: quantity.value(),
                                oninput: quantity.oninput(),
                                onblur: quantity.onblur(),
                            }
                            if let Some(error) = quantity.parse_error() {
                                p { class: "mt-1 text-sm text-error", "Parse error: {error.message()}" }
                            }
                        }
                        label { class: "block",
                            span { class: "mb-1 block text-sm font-medium", "Unit price: dollars in, cents stored" }
                            input {
                                class: "input input-bordered w-full",
                                r#type: "text",
                                name: price.name(),
                                value: price.value(),
                                oninput: price.oninput(),
                                onblur: price.onblur(),
                            }
                            if let Some(error) = price.parse_error() {
                                p { class: "mt-1 text-sm text-error", "Parse error: {error.message()}" }
                            }
                        }
                    }
                }
            },
            secondary: rsx! {
                DemoPane { label: "Typed model",
                    StateGrid {
                        rows: vec![
                            ("quantity: u32", snapshot.quantity.to_string()),
                            ("unit_price_cents: u32", snapshot.unit_price_cents.to_string()),
                            ("line total", format!("${}", format_cents(line_total))),
                        ],
                    }
                }
            },
        }
    }
}
