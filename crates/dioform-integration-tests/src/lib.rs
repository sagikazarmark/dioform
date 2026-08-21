#[cfg(test)]
mod tests {
    use dioform::{FieldIdentity, FieldPath, FormHandle, ValidationTrigger};
    use dioform_garde::{GardeCollectionRowMatcher, GardeValidationExt};

    #[derive(Clone)]
    struct TagsForm {
        tags: Vec<String>,
    }

    impl garde::Validate for TagsForm {
        type Context = ();

        fn validate_into(
            &self,
            _context: &Self::Context,
            parent: &mut dyn FnMut() -> garde::Path,
            report: &mut garde::Report,
        ) {
            for (index, tag) in self.tags.iter().enumerate() {
                report.append(
                    parent().join("tags").join(index),
                    garde::Error::new(tag.clone()),
                );
            }
        }
    }

    fn tags_path() -> FieldPath<TagsForm, Vec<String>> {
        FieldPath::direct(
            FieldIdentity::new("tags"),
            "tags",
            |form: &TagsForm| &form.tags,
            |form: &mut TagsForm| &mut form.tags,
        )
    }

    #[test]
    fn bare_row_diagnostic_is_readable_from_the_collection_item_binding() {
        let form = FormHandle::new(TagsForm {
            tags: vec!["rust".to_owned()],
        });
        form.write_advanced(|core| {
            core.garde_validation()
                .collection_row_item(
                    GardeCollectionRowMatcher::new(["tags"], std::iter::empty::<&str>()),
                    tags_path(),
                )
                .expect("a static collection path should be supported")
                .register_string_errors();
        });
        let item = form.collection(tags_path()).items()[0].clone();

        form.validate_all(ValidationTrigger::Manual);

        assert_eq!(item.validation_errors()[0].error(), "rust");
    }
}
