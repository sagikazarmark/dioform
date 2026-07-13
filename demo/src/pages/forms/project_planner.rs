use dioform::prelude::*;
use dioxus::prelude::*;

use super::support::{DateYmd, cents_to_dollars, money, parse_dollars_to_cents};
use super::{field_select, field_text};
use crate::components::PageHeader;

#[derive(Clone, Debug, Default, PartialEq, Form)]
struct ProjectForm {
    client: ClientDetails,
    delivery: DeliveryPlan,
    budget: ProjectBudget,
}

#[derive(Clone, Debug, Default, PartialEq, Form)]
struct ClientDetails {
    company: String,
    #[form(name = "client-email")]
    contact_email: String,
}

#[derive(Clone, Debug, Default, PartialEq, Form)]
struct DeliveryPlan {
    start_date: DateYmd,
    milestones: Vec<Milestone>,
}

#[derive(Clone, Debug, Default, PartialEq, Form)]
struct ProjectBudget {
    currency: String,
    cap_cents: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Form)]
struct Milestone {
    title: String,
    estimate_days: u32,
}

fn initial() -> ProjectForm {
    ProjectForm {
        client: ClientDetails {
            company: "Analytical Engines Ltd".into(),
            contact_email: "team@example.com".into(),
        },
        delivery: DeliveryPlan {
            start_date: DateYmd::new(2026, 7, 1),
            milestones: vec![
                Milestone {
                    title: "Kickoff".into(),
                    estimate_days: 3,
                },
                Milestone {
                    title: "Beta".into(),
                    estimate_days: 12,
                },
            ],
        },
        budget: ProjectBudget {
            currency: "usd".into(),
            cap_cents: 5_000_000,
        },
    }
}

fn milestone_row(
    collection: &CollectionBinding<ProjectForm, Milestone, String>,
    item: CollectionItemBinding<ProjectForm, Milestone, String>,
    count: usize,
) -> Element {
    let f = Milestone::fields();
    let title = item.text(f.title());
    let days = use_collection_item_number(item.clone(), f.estimate_days());

    let title_oninput = title.clone();
    let days_oninput = days.clone();
    let remove = collection.clone();
    let id = item.identity();

    rsx! {
        div { class: "grid gap-2 sm:grid-cols-[1fr_6rem_auto] sm:items-end rounded-xl border border-base-300 p-3",
            label { class: "block",
                span { class: "mb-1 block text-xs text-base-content/55", "Milestone" }
                input {
                    class: "input input-bordered input-sm w-full",
                    name: title.name(),
                    value: title.value(),
                    oninput: move |e| title_oninput.on_input(e.value()),
                    onblur: move |_| title.on_blur(),
                }
            }
            label { class: "block",
                span { class: "mb-1 block text-xs text-base-content/55", "Days" }
                input {
                    class: "input input-bordered input-sm w-full",
                    r#type: "number",
                    min: "0",
                    name: days.name(),
                    value: days.value(),
                    oninput: move |e| days_oninput.on_input(e.value()),
                    onblur: move |_| days.on_blur(),
                }
            }
            button {
                class: "btn btn-xs btn-outline btn-error",
                r#type: "button",
                disabled: count <= 1,
                onclick: move |_| { remove.remove(id); },
                "remove"
            }
        }
    }
}

#[component]
pub fn ProjectPlanner() -> Element {
    let form = use_form(initial());
    let fields = ProjectForm::fields();

    let company_path = fields.client().join(ClientDetails::fields().company());
    let email_path = fields
        .client()
        .join(ClientDetails::fields().contact_email());
    let currency_path = fields.budget().join(ProjectBudget::fields().currency());
    let cap_path = fields.budget().join(ProjectBudget::fields().cap_cents());
    let milestones_path = fields.delivery().join(DeliveryPlan::fields().milestones());

    let company = form.text(company_path);
    let contact_email = form.text(email_path.clone());
    let currency = form.select(currency_path);
    let cap = use_number_with(form.clone(), cap_path, parse_dollars_to_cents, |cents| {
        cents_to_dollars(*cents)
    });
    let milestones = form.collection(milestones_path);

    let cap_oninput = cap.clone();
    let milestones_for_add = milestones.clone();

    let snapshot = form.snapshot();
    let items = milestones.items();
    let total_days: u32 = snapshot
        .delivery
        .milestones
        .iter()
        .map(|m| m.estimate_days)
        .sum();
    let rendered_email_name = email_path.field_name().to_string();

    rsx! {
        PageHeader {
            eyebrow: "Realistic forms",
            title: "Project planner",
            intro: "Nested named structs composed with FieldPath::join keep access typed all the way down, a field-name override renders the contact email as a custom HTML name, and a nested collection holds the milestone rows.",
        }
        div { class: "mt-8 grid gap-6 lg:grid-cols-[1fr_15rem]",
            div { class: "rounded-2xl border border-base-300 bg-base-100 p-6 shadow-sm space-y-5",
                div { class: "rounded-xl border border-base-300 p-4",
                    p { class: "mb-3 text-sm font-semibold", "Client" }
                    div { class: "grid gap-4 sm:grid-cols-2",
                        {field_text("Company", &company, "text", "Company name")}
                        {field_text("Contact email", &contact_email, "email", "team@example.com")}
                    }
                    p { class: "mt-2 font-mono text-xs text-base-content/55", "rendered name = \"{rendered_email_name}\"" }
                }
                div { class: "rounded-xl border border-base-300 p-4",
                    p { class: "mb-3 text-sm font-semibold", "Budget" }
                    div { class: "grid gap-4 sm:grid-cols-2",
                        {field_select("Currency", &currency, [("usd", "USD"), ("eur", "EUR"), ("gbp", "GBP")])}
                        label { class: "block space-y-1",
                            span { class: "text-sm font-medium", "Cap ($)" }
                            input {
                                class: "input input-bordered w-full",
                                name: cap.name(),
                                value: cap.value(),
                                oninput: move |e| cap_oninput.on_input(e.value()),
                                onblur: move |_| cap.on_blur(),
                            }
                        }
                    }
                }
                div {
                    div { class: "mb-2 flex items-center justify-between",
                        p { class: "text-sm font-semibold", "Milestones" }
                        button {
                            class: "btn btn-xs btn-outline",
                            r#type: "button",
                            onclick: move |_| { milestones_for_add.append(Milestone { title: String::new(), estimate_days: 5 }); },
                            "Add milestone"
                        }
                    }
                    div { class: "space-y-2",
                        for item in items.iter().cloned() {
                            {milestone_row(&milestones, item, items.len())}
                        }
                    }
                }
            }
            aside { class: "space-y-2 rounded-2xl border border-base-300 bg-base-200/40 p-5 text-sm",
                p { class: "text-xs font-semibold uppercase tracking-wider text-base-content/45", "Summary" }
                div { class: "flex justify-between", span { "Milestones" } span { "{items.len()}" } }
                div { class: "flex justify-between", span { "Total days" } span { "{total_days}" } }
                div { class: "flex justify-between", span { "Budget cap" } span { "{money(snapshot.budget.cap_cents)}" } }
            }
        }
    }
}
