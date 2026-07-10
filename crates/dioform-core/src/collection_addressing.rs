use std::rc::Rc;

use super::{CollectionItemIdentity, FieldIdentity, FieldPath};

/// Derived addressing metadata for one logical collection item field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionItemFieldAddress {
    identity: FieldIdentity,
    field_name: String,
    accessibility_name: String,
}

impl CollectionItemFieldAddress {
    pub fn new<Model, Item, Value>(
        collection: &FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        index: usize,
        field: &FieldPath<Item, Value>,
    ) -> Self {
        Self {
            identity: Self::identity_for(collection, item, field),
            field_name: Self::field_name_for(collection, index, field),
            accessibility_name: Self::accessibility_name_for(collection, item, field),
        }
    }

    pub fn identity(&self) -> FieldIdentity {
        self.identity.clone()
    }

    pub fn field_name(&self) -> &str {
        &self.field_name
    }

    pub fn accessibility_name(&self) -> &str {
        &self.accessibility_name
    }

    pub fn identity_for<Model, Item, Value>(
        collection: &FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: &FieldPath<Item, Value>,
    ) -> FieldIdentity {
        let collection_identity = collection.identity();
        let collection = collection_identity
            .as_static_path()
            .expect("collection fields in the first slice must be direct static fields");
        let field_identity = field.identity();
        let field = field_identity
            .as_static_path()
            .expect("collection item fields in the first slice must be direct static fields");

        Self::identity_from_static_segments(collection, item, field)
    }

    pub fn identity_from_static_segments(
        collection: impl Into<Rc<str>>,
        item: CollectionItemIdentity,
        field: impl Into<Rc<str>>,
    ) -> FieldIdentity {
        let collection = collection.into();
        let field = field.into();

        if field.is_empty() {
            FieldIdentity::collection_item_value(collection, item)
        } else {
            FieldIdentity::collection_item(collection, item, field)
        }
    }

    pub fn field_name_for<Model, Item, Value>(
        collection: &FieldPath<Model, Vec<Item>>,
        index: usize,
        field: &FieldPath<Item, Value>,
    ) -> String {
        collection_item_field_name(collection.field_name(), index, field.field_name())
    }

    pub fn accessibility_name_for<Model, Item, Value>(
        collection: &FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: &FieldPath<Item, Value>,
    ) -> String {
        collection_item_accessibility_name(collection.field_name(), item, field.field_name())
    }

    pub fn matches_item(
        field: &FieldIdentity,
        collection: &FieldIdentity,
        item: CollectionItemIdentity,
    ) -> bool {
        collection
            .static_path()
            .is_some_and(|collection| field.is_collection_item_for(collection, item))
    }
}

fn collection_item_field_name(collection: &str, index: usize, field: &str) -> String {
    if field.is_empty() {
        format!("{collection}[{index}]")
    } else {
        format!("{collection}[{index}].{field}")
    }
}

fn collection_item_accessibility_name(
    collection: &str,
    item: CollectionItemIdentity,
    field: &str,
) -> String {
    if field.is_empty() {
        format!("{}.{}", collection, item.key())
    } else {
        format!("{}.{}.{}", collection, item.key(), field)
    }
}
