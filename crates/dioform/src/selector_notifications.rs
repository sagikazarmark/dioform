use super::{FieldIdentity, FormReactivity};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SelectorTransition {
    UnknownMutation,
    FieldValueChanged(FieldIdentity),
    FieldMetadataChanged(FieldIdentity),
    FieldValidationChanged(FieldIdentity),
    CollectionStructureChanged(FieldIdentity),
    CollectionStructureUserChanged(FieldIdentity),
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

#[derive(Clone, Debug, Eq, PartialEq)]
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
            Self::FieldValueChanged(field) => vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Snapshot,
                SelectorNotification::Submit,
                SelectorNotification::ValidationErrors,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::FieldValue(field.clone()),
                SelectorNotification::FieldValidationErrors(field.clone()),
                SelectorNotification::VisibleFieldValidationErrors(field),
            ],
            Self::FieldMetadataChanged(field) => vec![
                SelectorNotification::WholeForm,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::FieldMetadata(field.clone()),
                SelectorNotification::VisibleFieldValidationErrors(field),
            ],
            Self::FieldValidationChanged(field) => vec![
                SelectorNotification::WholeForm,
                SelectorNotification::Submit,
                SelectorNotification::ValidationErrors,
                SelectorNotification::VisibleValidationErrors,
                SelectorNotification::FieldValidationErrors(field.clone()),
                SelectorNotification::VisibleFieldValidationErrors(field),
            ],
            Self::CollectionStructureChanged(collection) => {
                let mut notifications = Vec::new();
                extend_unique(
                    &mut notifications,
                    Self::FieldValueChanged(collection).selector_notifications([]),
                );
                extend_unique(
                    &mut notifications,
                    Self::ValidationChanged.selector_notifications(tracked_fields),
                );
                notifications
            }
            Self::CollectionStructureUserChanged(collection) => {
                let mut notifications = Vec::new();
                extend_unique(
                    &mut notifications,
                    Self::FieldValueChanged(collection.clone()).selector_notifications([]),
                );
                extend_unique(
                    &mut notifications,
                    Self::FieldMetadataChanged(collection).selector_notifications([]),
                );
                extend_unique(
                    &mut notifications,
                    Self::ValidationChanged.selector_notifications(tracked_fields),
                );
                notifications
            }
            Self::CollectionItemFieldValueChanged { collection, field } => {
                let mut notifications = Vec::new();
                extend_unique(
                    &mut notifications,
                    Self::FieldValueChanged(collection).selector_notifications([]),
                );
                extend_unique(
                    &mut notifications,
                    Self::FieldValueChanged(field).selector_notifications([]),
                );
                extend_unique(
                    &mut notifications,
                    Self::ValidationChanged.selector_notifications(tracked_fields),
                );
                notifications
            }
            Self::CollectionItemFieldUserValueChanged { collection, field } => {
                let mut notifications = Vec::new();
                extend_unique(
                    &mut notifications,
                    Self::FieldValueChanged(collection).selector_notifications([]),
                );
                extend_unique(
                    &mut notifications,
                    Self::FieldValueChanged(field.clone()).selector_notifications([]),
                );
                extend_unique(
                    &mut notifications,
                    Self::FieldMetadataChanged(field).selector_notifications([]),
                );
                extend_unique(
                    &mut notifications,
                    Self::ValidationChanged.selector_notifications(tracked_fields),
                );
                notifications
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
}

fn extend_unique(
    notifications: &mut Vec<SelectorNotification>,
    new_notifications: impl IntoIterator<Item = SelectorNotification>,
) {
    for notification in new_notifications {
        if !notifications.contains(&notification) {
            notifications.push(notification);
        }
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
            SelectorNotification::FieldValue(field) => self.field(&field).value.notify_changed(),
            SelectorNotification::FieldMetadata(field) => {
                self.field(&field).metadata.notify_changed();
            }
            SelectorNotification::FieldValidationErrors(field) => {
                self.field(&field).validation_errors.notify_changed();
            }
            SelectorNotification::VisibleFieldValidationErrors(field) => {
                self.field(&field)
                    .visible_validation_errors
                    .notify_changed();
            }
            SelectorNotification::FieldParseErrors(field) => {
                self.field(&field).parse_errors.notify_changed();
            }
            SelectorNotification::AllFieldSelectors(field) => self.field(&field).notify_all(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
