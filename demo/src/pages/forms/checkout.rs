use dioform::advanced::FormCore;
use dioform::prelude::*;
use dioxus::prelude::*;

use crate::ui::{PageHeader, field_checkbox, field_select, field_text};

#[derive(Clone, Debug, PartialEq, Form)]
struct CheckoutForm {
    customer_name: String,
    email: String,
    billing_street: String,
    billing_city: String,
    billing_zip: String,
    same_shipping: bool,
    shipping_street: String,
    shipping_city: String,
    shipping_zip: String,
    delivery_window: String,
    payment_method: String,
}

fn initial() -> CheckoutForm {
    CheckoutForm {
        customer_name: String::new(),
        email: String::new(),
        billing_street: String::new(),
        billing_city: String::new(),
        billing_zip: String::new(),
        same_shipping: true,
        shipping_street: String::new(),
        shipping_city: String::new(),
        shipping_zip: String::new(),
        delivery_window: "standard".into(),
        payment_method: String::new(),
    }
}

fn required(
    core: &mut FormCore<CheckoutForm, String>,
    path: FieldPath<CheckoutForm, String>,
    source: &'static str,
    message: &'static str,
) {
    core.register_sync_field_validator(path, source, move |value: &String, _ctx| {
        if value.trim().is_empty() {
            vec![message.to_string()]
        } else {
            Vec::new()
        }
    });
}

fn build() -> FormHandle<CheckoutForm> {
    let form = FormHandle::<CheckoutForm>::from_config(
        FormConfig::new(initial()).validation_mode(ValidationMode::on_blur()),
    );
    form.write_advanced(|core| {
        let f = CheckoutForm::fields();
        required(
            core,
            f.customer_name(),
            "customer",
            "Enter the customer name.",
        );
        required(
            core,
            f.billing_street(),
            "b_street",
            "Enter the billing street.",
        );
        required(core, f.billing_city(), "b_city", "Enter the billing city.");
        required(core, f.billing_zip(), "b_zip", "Enter the billing ZIP.");
        required(
            core,
            f.payment_method(),
            "payment",
            "Choose a payment method.",
        );
        core.register_sync_field_validator(f.email(), "email", |value: &String, _ctx| {
            if value.contains('@') {
                Vec::new()
            } else {
                vec!["Enter a valid email.".to_string()]
            }
        });
        core.register_sync_form_validator("shipping", |ctx| {
            let form = ctx.form();
            let f = CheckoutForm::fields();
            if form.same_shipping {
                return Vec::new();
            }
            let mut errors = Vec::new();
            if form.shipping_street.trim().is_empty() {
                errors.push(FormValidationError::field(
                    f.shipping_street(),
                    "Enter the shipping street.".to_string(),
                ));
            }
            if form.shipping_city.trim().is_empty() {
                errors.push(FormValidationError::field(
                    f.shipping_city(),
                    "Enter the shipping city.".to_string(),
                ));
            }
            if form.shipping_zip.trim().is_empty() {
                errors.push(FormValidationError::field(
                    f.shipping_zip(),
                    "Enter the shipping ZIP.".to_string(),
                ));
            }
            errors
        });
    });
    form
}

fn money(cents: u32) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

#[component]
pub fn Checkout() -> Element {
    let form = use_form_handle(build);
    let f = CheckoutForm::fields();
    let mut status = use_signal(String::new);

    let customer = form.text(f.customer_name());
    let email = form.text(f.email());
    let billing_street = form.text(f.billing_street());
    let billing_city = form.text(f.billing_city());
    let billing_zip = form.text(f.billing_zip());
    let same_shipping = form.checkbox(f.same_shipping());
    let shipping_street = form.text(f.shipping_street());
    let shipping_city = form.text(f.shipping_city());
    let shipping_zip = form.text(f.shipping_zip());
    let delivery = form.select(f.delivery_window());
    let payment = form.select(f.payment_method());
    let submit = form.managed_submit();

    let submit_for_form = submit.clone();
    let snapshot = form.snapshot();
    let show_shipping = !snapshot.same_shipping;
    let shipping_cents = if snapshot.delivery_window == "express" {
        2200
    } else {
        900
    };
    let subtotal = 12800u32;
    let tax = 1136u32;
    let total = subtotal + shipping_cents + tax;

    rsx! {
        PageHeader {
            eyebrow: "Realistic forms",
            title: "Checkout",
            intro: "The same-as-billing checkbox hides the shipping controls, but a form validator still owns the rule that decides whether shipping fields are required; the requirement lives in the model, not the markup.",
        }
        div { class: "mt-8 grid gap-6 lg:grid-cols-[1fr_16rem]",
            div { class: "rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm",
                form {
                    class: "space-y-4",
                    onsubmit: move |event| {
                        let result = submit_for_form.on_submit(event, |_s: SubmissionSnapshot<CheckoutForm>| SubmitErrors::none());
                        status.set(match result {
                            SubmitResult::Succeeded => "Order placed.".to_string(),
                            SubmitResult::Blocked(_) => "Fix the highlighted fields first.".to_string(),
                            other => format!("{other:?}"),
                        });
                    },
                    div { class: "grid gap-4 sm:grid-cols-2",
                        {field_text("Customer name", &customer, "text", "Ada Lovelace")}
                        {field_text("Email", &email, "email", "ada@example.com")}
                    }
                    div { class: "rounded-xl border border-base-300 p-4",
                        p { class: "mb-3 text-sm font-semibold", "Billing address" }
                        div { class: "grid gap-4 sm:grid-cols-3",
                            {field_text("Street", &billing_street, "text", "12 Analytical Way")}
                            {field_text("City", &billing_city, "text", "London")}
                            {field_text("ZIP", &billing_zip, "text", "EC1A 1BB")}
                        }
                    }
                    div { class: "rounded-xl border border-base-300 p-4",
                        {field_checkbox("Shipping address matches billing", &same_shipping)}
                        if show_shipping {
                            div { class: "mt-3 grid gap-4 sm:grid-cols-3",
                                {field_text("Shipping street", &shipping_street, "text", "34 Compiler Lane")}
                                {field_text("Shipping city", &shipping_city, "text", "Oxford")}
                                {field_text("Shipping ZIP", &shipping_zip, "text", "OX1 1AA")}
                            }
                        }
                    }
                    div { class: "grid gap-4 sm:grid-cols-2",
                        {field_select("Delivery window", &delivery, [("standard", "Standard"), ("express", "Express")])}
                        {field_select("Payment method", &payment, [("", "Choose payment"), ("card", "Card"), ("invoice", "Invoice")])}
                    }
                    div { class: "border-t border-base-300 pt-4",
                        button { class: "btn btn-primary", r#type: "submit", "Place order" }
                    }
                }
            }
            aside { class: "space-y-2 rounded-2xl border border-base-300 bg-base-200/40 p-5 text-sm",
                p { class: "text-xs font-semibold uppercase tracking-wider text-base-content/45", "Order summary" }
                div { class: "flex justify-between", span { "Subtotal" } span { "{money(subtotal)}" } }
                div { class: "flex justify-between", span { "Shipping" } span { "{money(shipping_cents)}" } }
                div { class: "flex justify-between", span { "Tax" } span { "{money(tax)}" } }
                div { class: "flex justify-between border-t border-base-300 pt-2 font-bold",
                    span { "Total" }
                    span { "{money(total)}" }
                }
                if !status.read().is_empty() {
                    p { class: "pt-2 text-base-content/70", "{status}" }
                }
            }
        }
    }
}
