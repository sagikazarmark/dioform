use std::{collections::HashSet, hash::Hash};

use dioform_core::__private::FieldAncestry;

use super::{
    FieldIdentity, FieldReactivity, FieldUpdateOrigin, FormReactivity, ReactiveSubscribers,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SelectorTransition {
    UnknownMutation,
    FieldValueChanged(FieldIdentity),
    FieldMetadataChanged(FieldIdentity),
    FieldValidationChanged(FieldIdentity),
    CollectionStructureChanged(FieldIdentity),
    CollectionStructureUserChanged(FieldIdentity),
    CollectionItemsRemoved {
        collection: FieldIdentity,
        items: Vec<FieldIdentity>,
        origin: FieldUpdateOrigin,
    },
    CollectionItemReplaced {
        collection: FieldIdentity,
        item: FieldIdentity,
        origin: FieldUpdateOrigin,
    },
    CollectionItemFieldValueChanged {
        collection: FieldIdentity,
        field: FieldIdentity,
    },
    CollectionItemFieldUserValueChanged {
        collection: FieldIdentity,
        field: FieldIdentity,
    },
    ValidationChanged,
    SubmitChanged,
    SubmitAttempted,
    ParseChanged(FieldIdentity),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum SelectorNotification {
    WholeForm,
    Snapshot,
    Submit,
    ValidationErrors,
    VisibleValidationErrors,
    FormValidationErrors,
    VisibleFormValidationErrors,
    ParseErrors,
    FieldValue(FieldIdentity),
    FieldMetadata(FieldIdentity),
    FieldValidationErrors(FieldIdentity),
    VisibleFieldValidationErrors(FieldIdentity),
    FieldParseErrors(FieldIdentity),
    AllFieldSelectors(FieldIdentity),
}

impl SelectorTransition {
    pub(super) fn wakes_validation_waiters(&self) -> bool {
        matches!(
            self,
            Self::UnknownMutation
                | Self::CollectionStructureChanged(_)
                | Self::CollectionStructureUserChanged(_)
                | Self::CollectionItemsRemoved { .. }
                | Self::CollectionItemReplaced { .. }
                | Self::CollectionItemFieldValueChanged { .. }
                | Self::CollectionItemFieldUserValueChanged { .. }
                | Self::ValidationChanged
        )
    }

    fn selector_notifications(
        self,
        tracked_fields: impl IntoIterator<Item = FieldIdentity>,
    ) -> Vec<SelectorNotification> {
        match self {
            Self::UnknownMutation => {
                let mut notifications = vec![
                    SelectorNotification::WholeForm,
                    SelectorNotification::Snapshot,
                    SelectorNotification::Submit,
                    SelectorNotification::ValidationErrors,
                    SelectorNotification::VisibleValidationErrors,
                    SelectorNotification::FormValidationErrors,
                    SelectorNotification::VisibleFormValidationErrors,
                    SelectorNotification::ParseErrors,
                ];
                notifications.extend(
                    tracked_fields
                        .into_iter()
                        .map(SelectorNotification::AllFieldSelectors),
                );
                notifications
            }
            Self::FieldValueChanged(field) => {
                let mut notifications = vec![
                    SelectorNotification::WholeForm,
                    SelectorNotification::Snapshot,
                    SelectorNotification::Submit,
                    SelectorNotification::ValidationErrors,
                    SelectorNotification::VisibleValidationErrors,
                    SelectorNotification::FieldValue(field.clone()),
                    SelectorNotification::FieldValidationErrors(field.clone()),
                    SelectorNotification::VisibleFieldValidationErrors(field.clone()),
                ];
                extend_field_value_ancestry(
                    &mut notifications,
                    tracked_fields,
                    std::slice::from_ref(&field),
                );
                notifications
            }
            Self::FieldMetadataChanged(field) => {
                let mut notifications = vec![
                    SelectorNotification::WholeForm,
                    SelectorNotification::VisibleValidationErrors,
                    SelectorNotification::FieldMetadata(field.clone()),
                    SelectorNotification::VisibleFieldValidationErrors(field.clone()),
                ];
                extend_visible_validation_ancestry(&mut notifications, tracked_fields, &field);
                notifications
            }
            Self::FieldValidationChanged(field) => vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Submit,
                SelectorNotification::ValidationErrors,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::FieldValidationErrors(field.clone()),
                SelectorNotification::VisibleFieldValidationErrors(field),
            ],
            Self::CollectionStructureChanged(collection) => Self::composite_notifications(
                [Self::FieldValueChanged(collection.clone()).selector_notifications([])],
                tracked_fields,
                std::slice::from_ref(&collection),
            ),
            Self::CollectionStructureUserChanged(collection) => Self::composite_notifications(
                [
                    Self::FieldValueChanged(collection.clone()).selector_notifications([]),
                    Self::FieldMetadataChanged(collection.clone()).selector_notifications([]),
                ],
                tracked_fields,
                std::slice::from_ref(&collection),
            ),
            Self::CollectionItemsRemoved {
                collection,
                items,
                origin,
            } => Self::collection_items_removed_notifications(
                collection,
                items,
                origin,
                tracked_fields,
            ),
            Self::CollectionItemReplaced {
                collection,
                item,
                origin,
            } => {
                let written = [collection, item];
                let mut legs = vec![
                    Self::FieldValueChanged(written[0].clone()).selector_notifications([]),
                    Self::FieldValueChanged(written[1].clone()).selector_notifications([]),
                ];

                if origin == FieldUpdateOrigin::User {
                    legs.push(
                        Self::FieldMetadataChanged(written[0].clone()).selector_notifications([]),
                    );
                }

                Self::composite_notifications(legs, tracked_fields, &written)
            }
            Self::CollectionItemFieldValueChanged { collection, field } => {
                let written = [collection, field];

                Self::composite_notifications(
                    [
                        Self::FieldValueChanged(written[0].clone()).selector_notifications([]),
                        Self::FieldValueChanged(written[1].clone()).selector_notifications([]),
                    ],
                    tracked_fields,
                    &written,
                )
            }
            Self::CollectionItemFieldUserValueChanged { collection, field } => {
                let written = [collection, field];

                Self::composite_notifications(
                    [
                        Self::FieldValueChanged(written[0].clone()).selector_notifications([]),
                        Self::FieldValueChanged(written[1].clone()).selector_notifications([]),
                        Self::FieldMetadataChanged(written[1].clone()).selector_notifications([]),
                    ],
                    tracked_fields,
                    &written,
                )
            }
            Self::ValidationChanged => {
                let mut notifications = vec![
                    SelectorNotification::WholeForm,
                    SelectorNotification::Submit,
                    SelectorNotification::ValidationErrors,
                    SelectorNotification::VisibleValidationErrors,
                    SelectorNotification::FormValidationErrors,
                    SelectorNotification::VisibleFormValidationErrors,
                ];
                for field in tracked_fields {
                    notifications.push(SelectorNotification::FieldValidationErrors(field.clone()));
                    notifications.push(SelectorNotification::VisibleFieldValidationErrors(field));
                }
                notifications
            }
            Self::SubmitChanged => vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Submit,
            ],
            Self::SubmitAttempted => {
                let mut notifications = vec![
                    SelectorNotification::WholeForm,
                    SelectorNotification::Submit,
                    SelectorNotification::VisibleValidationErrors,
                    SelectorNotification::VisibleFormValidationErrors,
                ];
                notifications.extend(
                    tracked_fields
                        .into_iter()
                        .map(SelectorNotification::VisibleFieldValidationErrors),
                );
                notifications
            }
            Self::ParseChanged(field) => vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Submit,
                SelectorNotification::ParseErrors,
                SelectorNotification::FieldParseErrors(field),
            ],
        }
    }

    pub(super) fn field_signal_notifications(
        self,
        tracked_fields: impl IntoIterator<Item = FieldIdentity>,
    ) -> Vec<FieldIdentity> {
        self.selector_notifications(tracked_fields)
            .into_iter()
            .filter_map(|notification| match notification {
                SelectorNotification::FieldValue(field)
                | SelectorNotification::AllFieldSelectors(field) => Some(field),
                _ => None,
            })
            .collect()
    }

    fn collection_items_removed_notifications(
        collection: FieldIdentity,
        items: Vec<FieldIdentity>,
        origin: FieldUpdateOrigin,
        tracked_fields: impl IntoIterator<Item = FieldIdentity>,
    ) -> Vec<SelectorNotification> {
        let tracked_fields: Vec<_> = tracked_fields.into_iter().collect();
        let mut legs = vec![Self::FieldValueChanged(collection.clone()).selector_notifications([])];

        if origin == FieldUpdateOrigin::User {
            legs.push(Self::FieldMetadataChanged(collection.clone()).selector_notifications([]));
        }

        for item in &items {
            legs.push(Self::FieldValueChanged(item.clone()).selector_notifications([]));
        }

        for field in &tracked_fields {
            if items.iter().any(|item| FieldAncestry::relates(item, field)) {
                legs.push(Self::FieldMetadataChanged(field.clone()).selector_notifications([]));
            }
        }

        let written: Vec<_> = std::iter::once(collection).chain(items).collect();

        Self::composite_notifications(legs, tracked_fields, &written)
    }

    /// Assembles a transition that is composed of several simpler ones over the same write.
    ///
    /// The legs are deduplicated against each other because they overlap by construction. The
    /// ancestry expansion runs after them and outside the dedup: it emits the value selector of
    /// tracked identities other than the written ones, which no leg emits, so it cannot repeat
    /// anything already collected.
    fn composite_notifications(
        legs: impl IntoIterator<Item = Vec<SelectorNotification>>,
        tracked_fields: impl IntoIterator<Item = FieldIdentity>,
        written: &[FieldIdentity],
    ) -> Vec<SelectorNotification> {
        let tracked_fields: Vec<_> = tracked_fields.into_iter().collect();
        let mut notifications = UniqueList::new();

        for leg in legs {
            notifications.extend(leg);
        }

        notifications
            .extend(Self::ValidationChanged.selector_notifications(tracked_fields.iter().cloned()));

        let mut assembled = notifications.into_items();

        extend_field_value_ancestry(&mut assembled, tracked_fields, written);
        assembled
    }
}

/// Wakes the value selector of every tracked identity in **Field Ancestry** with a written field.
///
/// Only value selectors are expanded. Every field mutation already emits a validation-changed
/// transition that fans validation-error selectors out over every tracked identity, so expanding
/// those here would add a redundant second wake on the write path; metadata and parse errors are
/// scoped to the written field by the write itself.
///
/// Expansion filters the identities that are *already* registered rather than deriving ancestors
/// by splitting the written path: an identity nothing has read reactively has no subscriber to
/// wake, so naming it would only lengthen the emitted sequence. The written fields themselves are
/// skipped: their value selectors are already in `notifications`.
fn extend_field_value_ancestry(
    notifications: &mut Vec<SelectorNotification>,
    tracked_fields: impl IntoIterator<Item = FieldIdentity>,
    written: &[FieldIdentity],
) {
    for tracked in tracked_fields {
        if written.contains(&tracked) {
            continue;
        }

        if written
            .iter()
            .any(|field| FieldAncestry::relates(field, &tracked))
        {
            notifications.push(SelectorNotification::FieldValue(tracked));
        }
    }
}

/// Wakes visible-error selectors for registered fields that contain the metadata change.
///
/// Error Visibility reaches outward from a touched or blurred field, but the metadata selector
/// itself remains exact. Filtering existing registrations preserves lazy selector registration.
fn extend_visible_validation_ancestry(
    notifications: &mut Vec<SelectorNotification>,
    tracked_fields: impl IntoIterator<Item = FieldIdentity>,
    changed: &FieldIdentity,
) {
    for tracked in tracked_fields {
        if tracked != *changed && FieldAncestry::contains(&tracked, changed) {
            notifications.push(SelectorNotification::VisibleFieldValidationErrors(tracked));
        }
    }
}

/// An append-only list that skips values it already holds, keeping them in encounter order.
///
/// Membership is answered by a set kept alongside the list rather than by scanning it. The
/// `ValidationChanged` leg of a composite transition contributes two notifications per registered
/// identity, so a scan makes assembly quadratic in that count on every collection-item write.
///
/// It is generic over its item type only so that cost can be asserted: comparisons are what a scan
/// spends, and a `SelectorNotification` cannot count its own.
struct UniqueList<T> {
    items: Vec<T>,
    seen: HashSet<T>,
}

impl<T: Clone + Eq + Hash> UniqueList<T> {
    fn new() -> Self {
        Self {
            items: Vec::new(),
            seen: HashSet::new(),
        }
    }

    fn extend(&mut self, values: impl IntoIterator<Item = T>) {
        for value in values {
            if self.seen.insert(value.clone()) {
                self.items.push(value);
            }
        }
    }

    fn into_items(self) -> Vec<T> {
        self.items
    }
}

impl FormReactivity {
    pub(super) fn notify_selector_transition(&self, transition: SelectorTransition) {
        for notification in transition.selector_notifications(self.tracked_field_identities()) {
            self.notify_selector(notification);
        }
    }

    fn notify_selector(&self, notification: SelectorNotification) {
        match notification {
            SelectorNotification::WholeForm => self.whole.notify_changed(),
            SelectorNotification::Snapshot => self.snapshot.notify_changed(),
            SelectorNotification::Submit => self.submit.notify_changed(),
            SelectorNotification::ValidationErrors => self.validation_errors.notify_changed(),
            SelectorNotification::VisibleValidationErrors => {
                self.visible_validation_errors.notify_changed();
            }
            SelectorNotification::FormValidationErrors => {
                self.form_validation_errors.notify_changed()
            }
            SelectorNotification::VisibleFormValidationErrors => {
                self.visible_form_validation_errors.notify_changed();
            }
            SelectorNotification::ParseErrors => self.parse_errors.notify_changed(),
            SelectorNotification::FieldValue(field) => {
                self.notify_registered_field(&field, |reactivity| &reactivity.value);
            }
            SelectorNotification::FieldMetadata(field) => {
                self.notify_registered_field(&field, |reactivity| &reactivity.metadata);
            }
            SelectorNotification::FieldValidationErrors(field) => {
                self.notify_registered_field(&field, |reactivity| &reactivity.validation_errors);
            }
            SelectorNotification::VisibleFieldValidationErrors(field) => {
                self.notify_registered_field(&field, |reactivity| {
                    &reactivity.visible_validation_errors
                });
            }
            SelectorNotification::FieldParseErrors(field) => {
                self.notify_registered_field(&field, |reactivity| &reactivity.parse_errors);
            }
            SelectorNotification::AllFieldSelectors(field) => {
                if let Some(reactivity) = self.registered_field(&field) {
                    reactivity.notify_all();
                }
            }
        }
    }

    /// Wakes one of a field's selectors, doing nothing when the field has no registration.
    ///
    /// A subscriber is added only by a reactive read, which registers the field first, so a field
    /// with no registration provably has no subscriber this notification could have missed. A
    /// reader mounting afterwards registers the field on its own read and reads current state
    /// (ADR-0029).
    ///
    /// This is the notifying counterpart of [`FormReactivity::track_field`] and takes the same
    /// selector accessor, so the two sides of one selector are named the same way.
    fn notify_registered_field(
        &self,
        field: &FieldIdentity,
        selector: impl FnOnce(&FieldReactivity) -> &ReactiveSubscribers,
    ) {
        if let Some(reactivity) = self.registered_field(field) {
            selector(&reactivity).notify_changed();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, hash::Hasher, rc::Rc};

    use super::*;

    /// A stand-in notification that counts how many times it is compared for equality.
    ///
    /// Equality is what a dedup backed by a scan spends its time on, and it is invisible to the
    /// assertions over emitted notifications, which are identical either way.
    #[derive(Clone, Debug, Eq)]
    struct CountedNotification {
        value: usize,
        comparisons: Rc<Cell<usize>>,
    }

    impl PartialEq for CountedNotification {
        fn eq(&self, other: &Self) -> bool {
            self.comparisons.set(self.comparisons.get() + 1);
            self.value == other.value
        }
    }

    impl Hash for CountedNotification {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.value.hash(state);
        }
    }

    /// How many comparisons per item a constant-cost dedup is allowed. A set answers membership in
    /// about one comparison per repeated item; a scan needs half the collected length per item,
    /// which over [`DEDUPLICATED_ITEMS`] items exceeds this by two orders of magnitude.
    const COMPARISONS_PER_ITEM: usize = 4;

    /// Enough items that a quadratic dedup separates from a constant-cost one by a wide margin.
    const DEDUPLICATED_ITEMS: usize = 512;

    /// A collection-item write feeds two notifications per registered identity through the dedup,
    /// so a membership test that scans what is already collected makes assembly quadratic in that
    /// count on every keystroke in a collection row.
    #[test]
    fn deduplicating_notifications_costs_a_bounded_number_of_comparisons_per_item() {
        let comparisons = Rc::new(Cell::new(0));
        let counted = |value| CountedNotification {
            value,
            comparisons: Rc::clone(&comparisons),
        };

        let mut unique = UniqueList::new();

        unique.extend((0..DEDUPLICATED_ITEMS).map(counted));
        unique.extend((0..DEDUPLICATED_ITEMS).map(counted));

        assert_eq!(
            unique
                .into_items()
                .iter()
                .map(|notification| notification.value)
                .collect::<Vec<_>>(),
            (0..DEDUPLICATED_ITEMS).collect::<Vec<_>>(),
            "the dedup keeps encounter order and drops repeats"
        );
        assert!(
            comparisons.get() <= COMPARISONS_PER_ITEM * DEDUPLICATED_ITEMS,
            "deduplicating {DEDUPLICATED_ITEMS} items took {} comparisons, which is more than a \
             constant per item",
            comparisons.get()
        );
    }

    #[test]
    fn deduplicating_notifications_appends_in_encounter_order() {
        let mut unique = UniqueList::new();

        unique.extend([
            SelectorNotification::Submit,
            SelectorNotification::WholeForm,
            SelectorNotification::Submit,
        ]);
        unique.extend([
            SelectorNotification::WholeForm,
            SelectorNotification::ParseErrors,
        ]);

        assert_eq!(
            unique.into_items(),
            vec![
                SelectorNotification::Submit,
                SelectorNotification::WholeForm,
                SelectorNotification::ParseErrors,
            ]
        );
    }

    /// Assembles a collection-item write against the written identities plus `children` registered
    /// child fields of the same collection.
    fn collection_item_write_notifications(children: usize) -> Vec<SelectorNotification> {
        let collection = FieldIdentity::new("lines");
        let field = FieldIdentity::new("lines.description");
        let registered: Vec<_> = [collection.clone(), field.clone()]
            .into_iter()
            .chain((0..children).map(|index| FieldIdentity::new(format!("lines.child{index}"))))
            .collect();

        SelectorTransition::CollectionItemFieldValueChanged { collection, field }
            .selector_notifications(registered)
    }

    /// Each registered identity must cost the emitted sequence the same fixed number of
    /// notifications however many are already registered, and none of them twice. A dedup that
    /// scaled with what it had already collected would still emit this sequence, so the comparison
    /// bound above is what pins its cost; this pins the sequence the bound is allowed to assume.
    #[test]
    fn a_collection_item_write_emits_a_fixed_number_of_notifications_per_registered_identity() {
        let none = collection_item_write_notifications(0);
        let some = collection_item_write_notifications(DEDUPLICATED_ITEMS);
        let twice_as_many = collection_item_write_notifications(2 * DEDUPLICATED_ITEMS);

        assert_eq!(
            twice_as_many.len() - some.len(),
            some.len() - none.len(),
            "the notifications one registered identity adds depend on how many came before it"
        );

        let unique: HashSet<_> = twice_as_many.iter().collect();

        assert_eq!(
            unique.len(),
            twice_as_many.len(),
            "a notification is emitted more than once"
        );
    }

    #[test]
    fn field_value_change_maps_to_form_and_field_notifications() {
        let field = FieldIdentity::new("email");

        assert_eq!(
            SelectorTransition::FieldValueChanged(field.clone()).selector_notifications([]),
            vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Snapshot,
                SelectorNotification::Submit,
                SelectorNotification::ValidationErrors,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::FieldValue(field.clone()),
                SelectorNotification::FieldValidationErrors(field.clone()),
                SelectorNotification::VisibleFieldValidationErrors(field),
            ]
        );
    }

    #[test]
    fn collection_item_user_field_change_maps_to_collection_field_and_validation_notifications() {
        let collection = FieldIdentity::new("items");
        let field = FieldIdentity::new("items.name");

        assert_eq!(
            SelectorTransition::CollectionItemFieldUserValueChanged {
                collection: collection.clone(),
                field: field.clone(),
            }
            .selector_notifications([collection.clone(), field.clone()]),
            vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Snapshot,
                SelectorNotification::Submit,
                SelectorNotification::ValidationErrors,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::FieldValue(collection.clone()),
                SelectorNotification::FieldValidationErrors(collection.clone()),
                SelectorNotification::VisibleFieldValidationErrors(collection.clone()),
                SelectorNotification::FieldValue(field.clone()),
                SelectorNotification::FieldValidationErrors(field.clone()),
                SelectorNotification::VisibleFieldValidationErrors(field.clone()),
                SelectorNotification::FieldMetadata(field.clone()),
                SelectorNotification::FormValidationErrors,
                SelectorNotification::VisibleFormValidationErrors,
            ]
        );
    }

    #[test]
    fn field_value_change_wakes_tracked_value_selectors_in_field_ancestry() {
        let customer = FieldIdentity::new("invoice.customer");
        let name = FieldIdentity::new("invoice.customer.name");
        let invoice = FieldIdentity::new("invoice");
        let sibling = FieldIdentity::new("invoice.customer_account");

        assert_eq!(
            SelectorTransition::FieldValueChanged(customer.clone()).selector_notifications([
                name.clone(),
                invoice.clone(),
                sibling,
                customer.clone(),
            ]),
            vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Snapshot,
                SelectorNotification::Submit,
                SelectorNotification::ValidationErrors,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::FieldValue(customer.clone()),
                SelectorNotification::FieldValidationErrors(customer.clone()),
                SelectorNotification::VisibleFieldValidationErrors(customer),
                SelectorNotification::FieldValue(name),
                SelectorNotification::FieldValue(invoice),
            ]
        );
    }

    #[test]
    fn field_metadata_change_maps_to_metadata_and_visible_validation_notifications() {
        let field = FieldIdentity::new("email");

        assert_eq!(
            SelectorTransition::FieldMetadataChanged(field.clone()).selector_notifications([]),
            vec![
                SelectorNotification::WholeForm,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::FieldMetadata(field.clone()),
                SelectorNotification::VisibleFieldValidationErrors(field),
            ]
        );
    }

    #[test]
    fn field_metadata_change_wakes_containing_visible_validation_error_readers() {
        let mut core = dioform_core::FormCore::<Vec<()>>::new(vec![(), ()]);
        let path = dioform_core::FieldPath::direct(
            FieldIdentity::new("invoice.lines"),
            "invoice.lines",
            |items| items,
            |items| items,
        );
        let items = core.collection_items(path);
        let item = items[0].identity();
        let sibling = items[1].identity();
        let field = FieldIdentity::collection_item("invoice.lines", item, "description");
        let item_root = FieldIdentity::collection_item_value("invoice.lines", item);
        let collection = FieldIdentity::new("invoice.lines");
        let invoice = FieldIdentity::new("invoice");
        let descendant = FieldIdentity::collection_item("invoice.lines", item, "description.label");
        let sibling_root = FieldIdentity::collection_item_value("invoice.lines", sibling);

        assert_eq!(
            SelectorTransition::FieldMetadataChanged(field.clone()).selector_notifications([
                item_root.clone(),
                collection.clone(),
                invoice.clone(),
                descendant,
                sibling_root,
                field.clone(),
            ]),
            vec![
                SelectorNotification::WholeForm,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::FieldMetadata(field.clone()),
                SelectorNotification::VisibleFieldValidationErrors(field),
                SelectorNotification::VisibleFieldValidationErrors(item_root),
                SelectorNotification::VisibleFieldValidationErrors(collection),
                SelectorNotification::VisibleFieldValidationErrors(invoice),
            ]
        );
    }

    #[test]
    fn validation_change_maps_to_form_error_and_tracked_field_notifications() {
        let email = FieldIdentity::new("email");
        let password = FieldIdentity::new("password");

        assert_eq!(
            SelectorTransition::ValidationChanged
                .selector_notifications([email.clone(), password.clone()]),
            vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Submit,
                SelectorNotification::ValidationErrors,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::FormValidationErrors,
                SelectorNotification::VisibleFormValidationErrors,
                SelectorNotification::FieldValidationErrors(email.clone()),
                SelectorNotification::VisibleFieldValidationErrors(email),
                SelectorNotification::FieldValidationErrors(password.clone()),
                SelectorNotification::VisibleFieldValidationErrors(password),
            ]
        );
    }

    #[test]
    fn submit_attempt_maps_to_visible_validation_notifications() {
        let field = FieldIdentity::new("email");

        assert_eq!(
            SelectorTransition::SubmitAttempted.selector_notifications([field.clone()]),
            vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Submit,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::VisibleFormValidationErrors,
                SelectorNotification::VisibleFieldValidationErrors(field),
            ]
        );
    }

    #[test]
    fn parse_change_maps_to_parse_error_notifications() {
        let field = FieldIdentity::new("age");

        assert_eq!(
            SelectorTransition::ParseChanged(field.clone()).selector_notifications([]),
            vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Submit,
                SelectorNotification::ParseErrors,
                SelectorNotification::FieldParseErrors(field),
            ]
        );
    }

    #[test]
    fn collection_user_structure_change_maps_to_collection_metadata_and_validation_notifications() {
        let collection = FieldIdentity::new("items");

        assert_eq!(
            SelectorTransition::CollectionStructureUserChanged(collection.clone())
                .selector_notifications([collection.clone()]),
            vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Snapshot,
                SelectorNotification::Submit,
                SelectorNotification::ValidationErrors,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::FieldValue(collection.clone()),
                SelectorNotification::FieldValidationErrors(collection.clone()),
                SelectorNotification::VisibleFieldValidationErrors(collection.clone()),
                SelectorNotification::FieldMetadata(collection.clone()),
                SelectorNotification::FormValidationErrors,
                SelectorNotification::VisibleFormValidationErrors,
            ]
        );
    }

    #[test]
    fn collection_structure_transitions_isolate_unrelated_and_retained_item_selectors() {
        let mut core = dioform_core::FormCore::<Vec<()>>::new(vec![()]);
        let path = dioform_core::FieldPath::direct(
            FieldIdentity::new("lines"),
            "lines",
            |items| items,
            |items| items,
        );
        let retained_item = core.collection_items(path)[0].identity();
        let collection = FieldIdentity::new("lines");
        let retained_field = FieldIdentity::collection_item("lines", retained_item, "description");
        let unrelated_collection = FieldIdentity::new("other_lines");
        let ordinary_field = FieldIdentity::new("title");
        let tracked = [
            collection.clone(),
            retained_field.clone(),
            unrelated_collection.clone(),
            ordinary_field.clone(),
        ];

        for (transition, user_originated) in [
            (
                SelectorTransition::CollectionStructureChanged(collection.clone()),
                false,
            ),
            (
                SelectorTransition::CollectionStructureUserChanged(collection.clone()),
                true,
            ),
        ] {
            let notifications = transition.selector_notifications(tracked.clone());

            assert!(notifications.contains(&SelectorNotification::FieldValue(collection.clone())));
            assert_eq!(
                notifications.contains(&SelectorNotification::FieldMetadata(collection.clone())),
                user_originated
            );

            for isolated in [
                retained_field.clone(),
                unrelated_collection.clone(),
                ordinary_field.clone(),
            ] {
                assert!(
                    !notifications.contains(&SelectorNotification::FieldValue(isolated.clone()))
                );
                assert!(!notifications.contains(&SelectorNotification::FieldMetadata(isolated)));
            }
        }
    }

    #[test]
    fn collection_item_removal_transitions_wake_removed_items_but_isolate_unrelated_fields() {
        let mut core = dioform_core::FormCore::<Vec<()>>::new(vec![()]);
        let path = dioform_core::FieldPath::direct(
            FieldIdentity::new("lines"),
            "lines",
            |items| items,
            |items| items,
        );
        let removed_item = core.collection_items(path)[0].identity();
        let collection = FieldIdentity::new("lines");
        let removed_root = FieldIdentity::collection_item_value("lines", removed_item);
        let removed_field = FieldIdentity::collection_item("lines", removed_item, "description");
        let unrelated_collection = FieldIdentity::new("other_lines");
        let ordinary_field = FieldIdentity::new("title");
        let tracked = [
            collection.clone(),
            removed_field.clone(),
            unrelated_collection.clone(),
            ordinary_field.clone(),
        ];

        for origin in [FieldUpdateOrigin::Programmatic, FieldUpdateOrigin::User] {
            let notifications = SelectorTransition::CollectionItemsRemoved {
                collection: collection.clone(),
                items: vec![removed_root.clone()],
                origin,
            }
            .selector_notifications(tracked.clone());

            assert!(notifications.contains(&SelectorNotification::FieldValue(collection.clone())));
            assert!(
                notifications.contains(&SelectorNotification::FieldValue(removed_root.clone()))
            );
            assert!(
                notifications.contains(&SelectorNotification::FieldValue(removed_field.clone()))
            );
            assert!(
                notifications.contains(&SelectorNotification::FieldMetadata(removed_field.clone()))
            );
            assert_eq!(
                notifications.contains(&SelectorNotification::FieldMetadata(collection.clone())),
                origin == FieldUpdateOrigin::User
            );

            for isolated in [unrelated_collection.clone(), ordinary_field.clone()] {
                assert!(
                    !notifications.contains(&SelectorNotification::FieldValue(isolated.clone()))
                );
                assert!(!notifications.contains(&SelectorNotification::FieldMetadata(isolated)));
            }
        }
    }
}
