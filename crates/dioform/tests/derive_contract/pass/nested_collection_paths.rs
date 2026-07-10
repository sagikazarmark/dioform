#![allow(dead_code)]

use dioform::advanced::{FieldIdentity, FormCore};
use dioform::{FieldPath, Form, FormHandle};

#[derive(Clone, Debug, Form)]
struct InvoicePage {
    invoice: Invoice,
}

#[derive(Clone, Debug, Form)]
struct Invoice {
    #[form(name = "invoice_lines")]
    lines: Vec<InvoiceLine>,
}

#[derive(Clone, Debug, Form)]
struct InvoiceLine {
    product: Product,
    quantity: u32,
}

#[derive(Clone, Debug, Form)]
struct Product {
    #[form(name = "product-name")]
    name: String,
}

fn main() {
    let invoice_path = InvoicePage::fields().invoice();
    let lines_path: FieldPath<InvoicePage, Vec<InvoiceLine>> =
        invoice_path.join(Invoice::fields().lines());
    let product_name_path: FieldPath<InvoiceLine, String> = InvoiceLine::fields()
        .product()
        .join(Product::fields().name());
    let page = InvoicePage {
        invoice: Invoice {
            lines: vec![InvoiceLine {
                product: Product {
                    name: "Keyboard".to_owned(),
                },
                quantity: 1,
            }],
        },
    };

    assert_eq!(lines_path.identity().as_str(), "invoice.lines");
    assert_eq!(lines_path.field_name(), "invoice.invoice_lines");
    assert_eq!(product_name_path.identity().as_str(), "product.name");
    assert_eq!(product_name_path.field_name(), "product.product-name");
    assert_eq!(lines_path.get(&page).len(), 1);
    assert_eq!(product_name_path.get(&page.invoice.lines[0]), "Keyboard");

    let mut core = FormCore::new(page.clone());
    let core_item = core.collection_items(lines_path.clone())[0].identity();
    core.mark_collection_item_field_touched(
        lines_path.clone(),
        core_item,
        product_name_path.clone(),
    );

    assert!(core.is_field_identity_touched(&FieldIdentity::collection_item(
        "invoice.lines",
        core_item,
        "product.name",
    )));

    let handle = FormHandle::new(page);
    let lines = handle.collection(lines_path);
    let item = lines.items()[0].clone();
    let product_name = item.text(product_name_path);

    assert_eq!(product_name.name(), "invoice.invoice_lines[0].product.product-name");
    assert_eq!(product_name.value(), "Keyboard");

    product_name.on_input("Mouse");
    assert_eq!(handle.snapshot().invoice.lines[0].product.name, "Mouse");
}
