//! **Field Ancestry**: the containment relation between two [`FieldIdentity`] values.
//!
//! Writing a field must reach the fields it contains and the fields that contain it. The relation
//! is *derived* by comparing identity paths rather than stored as a parent chain on the identity
//! itself, because [`FieldIdentity`] derives `Eq`, `Ord`, and `Hash` and is a map key throughout
//! the workspace, and because identities also arrive from `FieldIdentity::new`,
//! `CollectionItemFieldAddress::identity_from_static_segments`, and serde `Deserialize`, which a
//! stored chain would not round-trip. See ADR-0020.
//!
//! The cost of deriving it is a contract: the dot is the **Identity Path Separator** and is
//! reserved. Segment ancestry is separator-anchored, so `counterparty` does not relate to the
//! sibling `counterparty_account`.

use super::FieldIdentity;

/// The character reserved to delimit static path segments inside a **Field Identity**.
const IDENTITY_PATH_SEPARATOR: char = '.';

/// The containment relation between two **Field Identities**.
///
/// This is a predicate and nothing more: it exposes no `parent()`, `segments()`, or `depth()`, so
/// the representation stays swappable if map and array traversal ever make segments stop being
/// separator-splittable.
pub struct FieldAncestry;

impl FieldAncestry {
    /// Returns whether either identity addresses a field whose value contains the other's.
    ///
    /// The relation is symmetric and reflexive: it answers ancestor-or-descendant-or-equal, so no
    /// call site has to reason about direction.
    ///
    /// - `Static` to `Static`: separator-anchored segment ancestry in either direction
    /// - `CollectionItem` to `CollectionItem`: same collection, same item, and segment ancestry on
    ///   the child-field component, where the empty segment is the item root
    /// - `Static` to `CollectionItem`: the static path is a *strict* ancestor of the collection
    ///   component, so a collection write never reaches its own items' value readers
    /// - `File`: relates to nothing but itself
    pub fn relates(left: &FieldIdentity, right: &FieldIdentity) -> bool {
        if left == right {
            return true;
        }

        match (left.collection_item_parts(), right.collection_item_parts()) {
            (
                Some((left_collection, left_item, left_field)),
                Some((right_collection, right_item, right_field)),
            ) => {
                left_collection == right_collection
                    && left_item == right_item
                    && (is_item_field_ancestor(left_field, right_field)
                        || is_item_field_ancestor(right_field, left_field))
            }
            (Some((collection, _, _)), None) => is_strict_ancestor_of_collection(right, collection),
            (None, Some((collection, _, _))) => is_strict_ancestor_of_collection(left, collection),
            (None, None) => match (left.static_path(), right.static_path()) {
                (Some(left), Some(right)) => {
                    is_static_ancestor(left, right) || is_static_ancestor(right, left)
                }
                // At least one side is a file selection outside the form model.
                _ => false,
            },
        }
    }
}

/// Returns whether a path's segments are all non-empty, so the separator delimits real segments.
pub(crate) fn has_well_formed_segments(path: &str) -> bool {
    path.is_empty() || !path.split(IDENTITY_PATH_SEPARATOR).any(str::is_empty)
}

/// Returns whether `ancestor` is a strict segment ancestor of `descendant`.
///
/// An empty static path is an ancestor of nothing: it addresses a value with no path of its own,
/// not the root of every field.
fn is_static_ancestor(ancestor: &str, descendant: &str) -> bool {
    !ancestor.is_empty() && has_segment_prefix(ancestor, descendant)
}

/// Returns whether `ancestor` is a strict segment ancestor of `descendant` inside one item.
///
/// Within an item the empty segment is the item root, and so an ancestor of every non-empty
/// sibling segment. This is what makes writing a whole item value reach its child fields.
fn is_item_field_ancestor(ancestor: &str, descendant: &str) -> bool {
    if ancestor.is_empty() {
        return !descendant.is_empty();
    }

    has_segment_prefix(ancestor, descendant)
}

/// Returns whether a static identity is a strict ancestor of a collection path.
///
/// Strict, never ancestor-or-equal: a collection field and its own items are the same write, and
/// treating them as related would re-render every row's value reader whenever the structure
/// changes.
fn is_strict_ancestor_of_collection(candidate: &FieldIdentity, collection: &str) -> bool {
    candidate
        .static_path()
        .is_some_and(|path| is_static_ancestor(path, collection))
}

fn has_segment_prefix(prefix: &str, path: &str) -> bool {
    path.len() > prefix.len()
        && path.starts_with(prefix)
        && path[prefix.len()..].starts_with(IDENTITY_PATH_SEPARATOR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CollectionItemIdentity;

    fn item(key: u64) -> CollectionItemIdentity {
        CollectionItemIdentity(key)
    }

    fn assert_relates(left: &FieldIdentity, right: &FieldIdentity) {
        assert!(
            FieldAncestry::relates(left, right),
            "{left:?} and {right:?}"
        );
        assert!(
            FieldAncestry::relates(right, left),
            "{right:?} and {left:?}"
        );
    }

    fn assert_unrelated(left: &FieldIdentity, right: &FieldIdentity) {
        assert!(
            !FieldAncestry::relates(left, right),
            "{left:?} and {right:?}"
        );
        assert!(
            !FieldAncestry::relates(right, left),
            "{right:?} and {left:?}"
        );
    }

    #[test]
    fn static_identity_relates_to_itself() {
        let field = FieldIdentity::new("invoice.customer");

        assert_relates(&field, &field.clone());
    }

    #[test]
    fn static_ancestry_holds_in_both_directions() {
        assert_relates(
            &FieldIdentity::new("invoice.customer"),
            &FieldIdentity::new("invoice.customer.name"),
        );
        assert_relates(
            &FieldIdentity::new("invoice"),
            &FieldIdentity::new("invoice.customer.name"),
        );
    }

    #[test]
    fn static_ancestry_is_anchored_on_the_separator() {
        assert_unrelated(
            &FieldIdentity::new("counterparty"),
            &FieldIdentity::new("counterparty_account"),
        );
        assert_unrelated(
            &FieldIdentity::new("counterparty"),
            &FieldIdentity::new("counterparty_account.name"),
        );
    }

    #[test]
    fn unrelated_static_paths_do_not_relate() {
        assert_unrelated(
            &FieldIdentity::new("invoice.customer"),
            &FieldIdentity::new("invoice.supplier"),
        );
    }

    #[test]
    fn an_empty_static_path_is_an_ancestor_of_nothing() {
        assert_unrelated(&FieldIdentity::new(""), &FieldIdentity::new("invoice"));
    }

    #[test]
    fn item_child_fields_relate_within_one_item() {
        assert_relates(
            &FieldIdentity::collection_item("invoice.lines", item(1), "customer"),
            &FieldIdentity::collection_item("invoice.lines", item(1), "customer.name"),
        );
    }

    #[test]
    fn item_child_fields_do_not_relate_across_items_or_collections() {
        assert_unrelated(
            &FieldIdentity::collection_item("invoice.lines", item(1), "customer"),
            &FieldIdentity::collection_item("invoice.lines", item(2), "customer.name"),
        );
        assert_unrelated(
            &FieldIdentity::collection_item("invoice.lines", item(1), "customer"),
            &FieldIdentity::collection_item("invoice.notes", item(1), "customer.name"),
        );
    }

    #[test]
    fn an_item_value_is_an_ancestor_of_its_child_fields() {
        assert_relates(
            &FieldIdentity::collection_item_value("invoice.lines", item(1)),
            &FieldIdentity::collection_item("invoice.lines", item(1), "customer.name"),
        );
    }

    #[test]
    fn a_collection_field_does_not_relate_to_its_own_items() {
        assert_unrelated(
            &FieldIdentity::new("invoice.lines"),
            &FieldIdentity::collection_item("invoice.lines", item(1), "description"),
        );
        assert_unrelated(
            &FieldIdentity::new("invoice.lines"),
            &FieldIdentity::collection_item_value("invoice.lines", item(1)),
        );
    }

    #[test]
    fn a_strict_static_ancestor_of_a_collection_relates_to_its_items() {
        assert_relates(
            &FieldIdentity::new("invoice"),
            &FieldIdentity::collection_item("invoice.lines", item(1), "description"),
        );
    }

    #[test]
    fn a_static_descendant_of_a_collection_path_does_not_relate_to_its_items() {
        assert_unrelated(
            &FieldIdentity::new("invoice.lines.description"),
            &FieldIdentity::collection_item("invoice.lines", item(1), "description"),
        );
    }

    #[test]
    fn a_file_selection_relates_to_nothing_but_itself() {
        let attachment = FieldIdentity::file("attachment");

        assert_relates(&attachment, &attachment.clone());
        assert_unrelated(&attachment, &FieldIdentity::new("attachment.name"));
        assert_unrelated(&attachment, &FieldIdentity::file("attachment.name"));
    }

    #[test]
    fn well_formed_segments_reject_empty_segments() {
        assert!(has_well_formed_segments(""));
        assert!(has_well_formed_segments("invoice.customer.name"));
        assert!(!has_well_formed_segments("."));
        assert!(!has_well_formed_segments(".invoice"));
        assert!(!has_well_formed_segments("invoice."));
        assert!(!has_well_formed_segments("invoice..customer"));
    }
}
