#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        fmt,
        rc::Rc,
    };

    use dioform::{
        ErrorVisibilityPolicy, FieldIdentity, FieldPath, Form, FormHandle, ValidationTarget,
        ValidationTrigger, advanced::FormCore,
    };
    use dioform_garde::{
        DiagnosticRouteProvenance, GardeCollectionRowMatcher, GardeDiagnostic, GardeValidationExt,
    };
    use dioxus::prelude::*;
    use dioxus_field::{
        Binding, ChangeOrigin, Field, FieldContext, FieldError, FieldMeta, use_binding,
        use_field_meta,
    };

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

    #[derive(Clone, dioform::Form)]
    struct DerivedSignupForm {
        email: String,
        password: String,
    }

    impl garde::Validate for DerivedSignupForm {
        type Context = ();

        fn validate_into(
            &self,
            _context: &Self::Context,
            parent: &mut dyn FnMut() -> garde::Path,
            report: &mut garde::Report,
        ) {
            report.append(parent().join("email"), garde::Error::new("email invalid"));
            report.append(
                parent().join("password"),
                garde::Error::new("password invalid"),
            );
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct RoutedGardeError {
        target: ValidationTarget,
        provenance: DiagnosticRouteProvenance,
    }

    fn routed_garde_error(diagnostic: GardeDiagnostic<'_>) -> RoutedGardeError {
        RoutedGardeError {
            target: diagnostic.target(),
            provenance: diagnostic
                .route_provenance()
                .expect("first-party adapters classify every diagnostic route")
                .clone(),
        }
    }

    #[test]
    fn derived_garde_path_map_routes_direct_fields_as_exact_static_targets() {
        let mut form: FormCore<DerivedSignupForm, RoutedGardeError> =
            FormCore::new_with_error_type(DerivedSignupForm {
                email: String::new(),
                password: String::new(),
            });
        form.garde_validation()
            .derived_path_map()
            .register(routed_garde_error);

        form.validate_form(ValidationTrigger::Manual);

        let fields = DerivedSignupForm::fields();
        assert_eq!(
            form.field_validation_errors(fields.email())[0].error(),
            &RoutedGardeError {
                target: ValidationTarget::field(fields.email()),
                provenance: DiagnosticRouteProvenance::ExactStaticTarget,
            }
        );
        assert_eq!(
            form.field_validation_errors(fields.password())[0].error(),
            &RoutedGardeError {
                target: ValidationTarget::field(fields.password()),
                provenance: DiagnosticRouteProvenance::ExactStaticTarget,
            }
        );
        assert!(form.form_validation_errors().is_empty());
    }

    #[derive(Clone, dioform::Form)]
    struct DerivedNestedForm {
        account: NestedAccount,
    }

    #[derive(Clone)]
    struct NestedAccount {
        email: String,
    }

    impl garde::Validate for DerivedNestedForm {
        type Context = ();

        fn validate_into(
            &self,
            _context: &Self::Context,
            parent: &mut dyn FnMut() -> garde::Path,
            report: &mut garde::Report,
        ) {
            if self.account.email.is_empty() {
                report.append(
                    parent().join("account").join("email"),
                    garde::Error::new("email invalid"),
                );
            }
        }
    }

    #[test]
    fn derived_garde_path_map_still_reports_paths_beyond_direct_fields() {
        let reported = Rc::new(RefCell::new(Vec::new()));
        let reported_by_adapter = Rc::clone(&reported);
        let mut form = FormCore::new(DerivedNestedForm {
            account: NestedAccount {
                email: String::new(),
            },
        });
        form.garde_validation()
            .derived_path_map()
            .on_unmapped_path(move |path| {
                reported_by_adapter.borrow_mut().push(path.to_string());
            })
            .register_string_errors();

        form.validate_form(ValidationTrigger::Manual);

        assert_eq!(reported.borrow().as_slice(), ["account.email"]);
        assert_eq!(form.form_validation_errors().len(), 1);
    }

    #[derive(Clone, dioform::Form)]
    struct CheckboxForm {
        accepted: bool,
    }

    #[derive(Clone)]
    struct CheckboxError(&'static str);

    impl fmt::Display for CheckboxError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.0)
        }
    }

    struct CheckboxProbe {
        form: FormHandle<CheckboxForm, CheckboxError>,
        binding: RefCell<Option<Binding<bool>>>,
        meta: RefCell<Option<FieldMeta>>,
        on_change: RefCell<Option<Callback<bool>>>,
        on_commit: RefCell<Option<Callback<()>>>,
        blur_listener_runs: Cell<usize>,
        form_blur_listener_runs: Cell<usize>,
    }

    #[derive(Clone, Props)]
    struct ConventionCheckboxProps {
        probe: Rc<CheckboxProbe>,
    }

    impl PartialEq for ConventionCheckboxProps {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.probe, &other.probe)
        }
    }

    #[allow(non_snake_case)]
    fn ConventionCheckbox(props: ConventionCheckboxProps) -> Element {
        let binding = use_binding(None, false);
        let meta = use_field_meta(None);
        let trio = binding.clone().into_trio();
        props.probe.binding.borrow_mut().replace(binding.clone());
        props.probe.meta.borrow_mut().replace(meta);
        props.probe.on_change.borrow_mut().replace(trio.on_change);
        props.probe.on_commit.borrow_mut().replace(trio.on_commit);
        let on_change = trio.on_change;
        let on_commit = trio.on_commit;

        rsx! {
            input {
                r#type: "checkbox",
                checked: binding.read,
                oninput: move |event| on_change.call(event.checked()),
                onblur: move |_| on_commit.call(()),
                ..meta.attributes(),
            }
        }
    }

    fn checkbox_field(probe: Rc<CheckboxProbe>) -> Element {
        let blur_probe = Rc::clone(&probe);
        dioform::use_field_blur_listener(
            probe.form.clone(),
            CheckboxForm::fields().accepted(),
            move |_| {
                blur_probe
                    .blur_listener_runs
                    .set(blur_probe.blur_listener_runs.get() + 1)
            },
        );
        let form_blur_probe = Rc::clone(&probe);
        dioform::use_form_blur_listener(probe.form.clone(), move |_| {
            form_blur_probe
                .form_blur_listener_runs
                .set(form_blur_probe.form_blur_listener_runs.get() + 1)
        });

        rsx! {
            Field {
                context: probe.form.checkbox(CheckboxForm::fields().accepted()),
                ConventionCheckbox { probe }
                FieldError { id: "accepted-error" }
            }
        }
    }

    fn render_reactive_updates(dom: &mut VirtualDom) {
        dom.render_immediate_to_vec();
        dom.render_immediate_to_vec();
    }

    #[test]
    fn checkbox_binding_drives_field_convention_metadata_and_errors() {
        let form = FormHandle::new_with_error_type(CheckboxForm { accepted: false })
            .with_id_namespace("terms");
        let validation_runs = Rc::new(Cell::new(0));
        let validator_runs = Rc::clone(&validation_runs);
        form.field(CheckboxForm::fields().accepted())
            .validator("required")
            .on(ValidationTrigger::Commit)
            .check_optional(move |accepted, _context| {
                validator_runs.set(validator_runs.get() + 1);
                (!accepted).then_some(CheckboxError("Accept the terms"))
            });
        let probe = Rc::new(CheckboxProbe {
            form: form.clone(),
            binding: RefCell::new(None),
            meta: RefCell::new(None),
            on_change: RefCell::new(None),
            on_commit: RefCell::new(None),
            blur_listener_runs: Cell::new(0),
            form_blur_listener_runs: Cell::new(0),
        });
        let mut dom = VirtualDom::new_with_props(checkbox_field, Rc::clone(&probe));
        dom.rebuild_in_place();

        let binding = probe
            .binding
            .borrow()
            .clone()
            .expect("component should expose the convention binding");
        let meta = probe
            .meta
            .borrow()
            .expect("component should expose metadata");
        assert_eq!(meta.id().as_ref(), "terms-accepted-input");
        assert_eq!(meta.name().as_deref(), Some("accepted"));
        assert!(!meta.touched());
        assert!(!meta.dirty());
        assert!(!form.is_field_committed(CheckboxForm::fields().accepted()));
        assert_eq!(
            dioxus_ssr::render(&dom),
            "<div><input type=\"checkbox\" aria-invalid=\"false\" id=\"terms-accepted-input\" name=\"accepted\"/><div id=\"accepted-error\" aria-live=\"polite\"></div></div>"
        );

        binding.commit();
        render_reactive_updates(&mut dom);
        assert!(!form.is_field_touched(CheckboxForm::fields().accepted()));
        assert!(!form.is_field_blurred(CheckboxForm::fields().accepted()));
        assert!(form.is_field_committed(CheckboxForm::fields().accepted()));
        assert!(!meta.touched());
        assert_eq!(probe.blur_listener_runs.get(), 0);
        assert_eq!(probe.form_blur_listener_runs.get(), 0);
        assert_eq!(validation_runs.get(), 1);

        binding.write(true, ChangeOrigin::Programmatic);
        render_reactive_updates(&mut dom);
        assert!(form.field_value(CheckboxForm::fields().accepted()));
        assert!(!form.is_field_touched(CheckboxForm::fields().accepted()));
        assert!(!meta.touched());
        assert!(meta.dirty());

        probe
            .on_change
            .borrow()
            .expect("checkbox should expose its change handler")
            .call(false);
        render_reactive_updates(&mut dom);
        assert!(!form.field_value(CheckboxForm::fields().accepted()));
        assert!(form.is_field_touched(CheckboxForm::fields().accepted()));
        assert!(meta.touched());
        assert!(!meta.dirty());

        probe
            .on_commit
            .borrow()
            .expect("checkbox should expose its commit handler")
            .call(());
        render_reactive_updates(&mut dom);
        assert!(!form.is_field_blurred(CheckboxForm::fields().accepted()));
        assert_eq!(probe.blur_listener_runs.get(), 0);
        assert_eq!(probe.form_blur_listener_runs.get(), 0);
        assert_eq!(validation_runs.get(), 2);
        assert_eq!(meta.errors(), vec![Rc::from("Accept the terms")]);
        assert_eq!(
            dioxus_ssr::render(&dom),
            "<div><input type=\"checkbox\" aria-describedby=\"accepted-error\" aria-errormessage=\"accepted-error\" aria-invalid=\"true\" data-invalid=\"true\" data-touched=\"true\" id=\"terms-accepted-input\" name=\"accepted\"/><div id=\"accepted-error\" aria-live=\"polite\" data-invalid=\"true\" data-touched=\"true\"><div>Accept the terms</div></div></div>"
        );

        binding.focus_exit();
        render_reactive_updates(&mut dom);
        assert!(form.is_field_blurred(CheckboxForm::fields().accepted()));
        assert_eq!(probe.blur_listener_runs.get(), 1);
        assert_eq!(probe.form_blur_listener_runs.get(), 1);
        assert_eq!(validation_runs.get(), 2);

        form.reset();
        render_reactive_updates(&mut dom);
        assert!(!form.is_field_touched(CheckboxForm::fields().accepted()));
        assert!(!form.is_field_blurred(CheckboxForm::fields().accepted()));
        assert!(!form.is_field_committed(CheckboxForm::fields().accepted()));
        assert!(!meta.touched());

        binding.focus_exit();
        render_reactive_updates(&mut dom);
        assert!(form.is_field_touched(CheckboxForm::fields().accepted()));
        assert!(form.is_field_blurred(CheckboxForm::fields().accepted()));
        assert!(!form.is_field_committed(CheckboxForm::fields().accepted()));
        assert!(meta.touched());
        assert_eq!(probe.blur_listener_runs.get(), 2);
        assert_eq!(probe.form_blur_listener_runs.get(), 2);
        assert_eq!(validation_runs.get(), 2);
        assert!(meta.errors().is_empty());
    }

    #[derive(Clone)]
    struct CodeError {
        code: &'static str,
    }

    struct FormatterProbe {
        form: FormHandle<CheckboxForm, CodeError>,
        binding: RefCell<Option<Binding<bool>>>,
    }

    fn formatted_checkbox_field(probe: Rc<FormatterProbe>) -> Element {
        let checkbox = probe.form.checkbox(CheckboxForm::fields().accepted());
        let meta = checkbox.meta_with_error_formatter(|error| format!("Error {}", error.code));
        let binding: Binding<bool> = checkbox.into();
        probe.binding.borrow_mut().replace(binding.clone());

        rsx! {
            Field {
                context: FieldContext::new(binding).with_meta(meta),
                FieldError { id: "formatted-error" }
            }
        }
    }

    #[test]
    fn field_meta_accepts_an_error_formatter_without_requiring_display() {
        let form = FormHandle::new_with_error_type(CheckboxForm { accepted: false });
        form.set_error_visibility_policy(ErrorVisibilityPolicy::Always);
        form.field(CheckboxForm::fields().accepted())
            .validator("required")
            .on(ValidationTrigger::Commit)
            .check_optional(|accepted, _context| {
                (!accepted).then_some(CodeError { code: "TERMS" })
            });
        let probe = Rc::new(FormatterProbe {
            form,
            binding: RefCell::new(None),
        });
        let mut dom = VirtualDom::new_with_props(formatted_checkbox_field, Rc::clone(&probe));
        dom.rebuild_in_place();

        probe
            .binding
            .borrow()
            .as_ref()
            .expect("component should expose the convention binding")
            .commit();
        render_reactive_updates(&mut dom);

        assert_eq!(
            dioxus_ssr::render(&dom),
            "<div><div id=\"formatted-error\" aria-live=\"polite\" data-invalid=\"true\"><div>Error TERMS</div></div></div>"
        );
    }

    #[derive(Clone, dioform::Form)]
    struct ScalarForm {
        text: String,
        optional_text: Option<String>,
        checked: bool,
        tri_state: Option<bool>,
        choice: i32,
    }

    fn scalar_bindings_convert(form: FormHandle<ScalarForm>) -> Element {
        let fields = ScalarForm::fields();
        let text: Binding<String> = form.text(fields.text()).into();
        let textarea: Binding<String> = form.textarea(fields.text()).into();
        let optional_text: Binding<Option<String>> =
            form.optional_text(fields.optional_text()).into();
        let checked: Binding<bool> = form.checkbox(fields.checked()).into();
        let same_checked: Binding<bool> = form.checkbox(fields.checked()).into();
        let tri_state: Binding<Option<bool>> = form.tri_state_checkbox(fields.tri_state()).into();
        let select: Binding<i32> = form.select(fields.choice()).into();
        let rendered_select: Binding<i32> = form
            .select_with(fields.choice(), str::parse::<i32>, i32::to_string)
            .into();
        let radio: Binding<i32> = form.radio_group(fields.choice()).into();

        assert_eq!((text.read)().as_str(), "text");
        assert_eq!((textarea.read)().as_str(), "text");
        assert_eq!((optional_text.read)(), Some("optional".to_owned()));
        assert!((checked.read)());
        assert!(checked == same_checked);
        assert_eq!((tri_state.read)(), None);
        assert_eq!((select.read)(), 7);
        assert_eq!((rendered_select.read)(), 7);
        assert_eq!((radio.read)(), 7);

        VNode::empty()
    }

    #[test]
    fn every_scalar_binding_produces_a_field_convention_binding() {
        let form = FormHandle::new(ScalarForm {
            text: "text".to_owned(),
            optional_text: Some("optional".to_owned()),
            checked: true,
            tri_state: None,
            choice: 7,
        });
        let mut dom = VirtualDom::new_with_props(scalar_bindings_convert, form);

        dom.rebuild_in_place();
    }
}
