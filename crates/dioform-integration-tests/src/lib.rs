#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        fmt,
        rc::Rc,
    };

    use dioform::{
        ErrorVisibilityPolicy, FieldIdentity, FieldPath, Form, FormConfig, FormHandle,
        ValidationTarget, ValidationTrigger, advanced::FormCore,
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
        let form = FormHandle::from_config(
            FormConfig::new(TagsForm {
                tags: vec!["rust".to_owned()],
            })
            .register_core(|core| {
                core.garde_validation()
                    .collection_row_item(
                        GardeCollectionRowMatcher::new(["tags"], std::iter::empty::<&str>()),
                        tags_path(),
                    )
                    .expect("a static collection path should be supported")
                    .register_string_errors();
            }),
        );
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
        let field = probe.form.field(CheckboxForm::fields().accepted());
        let meta = field.meta_with_error_formatter(|error| format!("Error {}", error.code));
        let binding: Binding<bool> = field.into();
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

    #[derive(Clone, Debug, PartialEq)]
    struct Rating(u8);

    #[derive(Clone, dioform::Form)]
    struct GenericFieldForm {
        rating: Rating,
    }

    struct GenericBindingProbe {
        form: FormHandle<GenericFieldForm>,
        bindings: RefCell<Vec<Binding<Rating>>>,
        values: RefCell<Vec<Rating>>,
        blur_listener_runs: Cell<usize>,
    }

    fn generic_bindings(probe: Rc<GenericBindingProbe>) -> Element {
        let blur_probe = Rc::clone(&probe);
        dioform::use_field_blur_listener(
            probe.form.clone(),
            GenericFieldForm::fields().rating(),
            move |_| {
                blur_probe
                    .blur_listener_runs
                    .set(blur_probe.blur_listener_runs.get() + 1);
            },
        );
        let first: Binding<Rating> = probe.form.field(GenericFieldForm::fields().rating()).into();
        let second: Binding<Rating> = probe.form.field(GenericFieldForm::fields().rating()).into();
        let different_namespace: Binding<Rating> = probe
            .form
            .clone()
            .with_id_namespace("other")
            .field(GenericFieldForm::fields().rating())
            .into();
        probe.values.borrow_mut().push((first.read)());
        probe
            .bindings
            .borrow_mut()
            .extend([first, second, different_namespace]);

        VNode::empty()
    }

    #[test]
    fn field_handle_produces_an_identity_aware_generic_binding() {
        let probe = Rc::new(GenericBindingProbe {
            form: FormHandle::new(GenericFieldForm { rating: Rating(3) }),
            bindings: RefCell::new(Vec::new()),
            values: RefCell::new(Vec::new()),
            blur_listener_runs: Cell::new(0),
        });
        let mut dom = VirtualDom::new_with_props(generic_bindings, Rc::clone(&probe));

        dom.rebuild_in_place();

        let bindings = probe.bindings.borrow();
        assert_eq!(probe.values.borrow().as_slice(), [Rating(3)]);
        assert!(bindings[0] == bindings[1]);
        assert!(bindings[0] != bindings[2]);
    }

    #[test]
    fn generic_binding_preserves_user_and_programmatic_write_origins() {
        let form = FormHandle::new(GenericFieldForm { rating: Rating(3) });
        let probe = Rc::new(GenericBindingProbe {
            form: form.clone(),
            bindings: RefCell::new(Vec::new()),
            values: RefCell::new(Vec::new()),
            blur_listener_runs: Cell::new(0),
        });
        let mut dom = VirtualDom::new_with_props(generic_bindings, Rc::clone(&probe));
        dom.rebuild_in_place();
        let binding = probe.bindings.borrow()[0].clone();

        binding.write(Rating(4), ChangeOrigin::Programmatic);
        assert_eq!(
            form.field_value(GenericFieldForm::fields().rating()),
            Rating(4)
        );
        assert!(!form.is_field_touched(GenericFieldForm::fields().rating()));
        assert!(form.is_field_dirty(GenericFieldForm::fields().rating()));

        binding.write(Rating(5), ChangeOrigin::User);
        assert_eq!(
            form.field_value(GenericFieldForm::fields().rating()),
            Rating(5)
        );
        assert!(form.is_field_touched(GenericFieldForm::fields().rating()));
    }

    #[test]
    fn generic_binding_keeps_commit_and_focus_exit_independent() {
        let form = FormHandle::new(GenericFieldForm { rating: Rating(3) });
        let validation_runs = Rc::new(Cell::new(0));
        let validator_runs = Rc::clone(&validation_runs);
        form.field(GenericFieldForm::fields().rating())
            .validator("rating")
            .on(ValidationTrigger::Commit)
            .check(move |_rating, _context| {
                validator_runs.set(validator_runs.get() + 1);
                Vec::new()
            });
        let probe = Rc::new(GenericBindingProbe {
            form: form.clone(),
            bindings: RefCell::new(Vec::new()),
            values: RefCell::new(Vec::new()),
            blur_listener_runs: Cell::new(0),
        });
        let mut dom = VirtualDom::new_with_props(generic_bindings, Rc::clone(&probe));
        dom.rebuild_in_place();
        let binding = probe.bindings.borrow()[0].clone();

        binding.commit();
        assert!(form.is_field_committed(GenericFieldForm::fields().rating()));
        assert!(!form.is_field_touched(GenericFieldForm::fields().rating()));
        assert!(!form.is_field_blurred(GenericFieldForm::fields().rating()));
        assert_eq!(validation_runs.get(), 1);
        assert_eq!(probe.blur_listener_runs.get(), 0);

        binding.focus_exit();
        assert!(form.is_field_touched(GenericFieldForm::fields().rating()));
        assert!(form.is_field_blurred(GenericFieldForm::fields().rating()));
        assert_eq!(validation_runs.get(), 1);
        assert_eq!(probe.blur_listener_runs.get(), 1);
    }

    #[derive(Clone)]
    struct RatingError(&'static str);

    impl fmt::Display for RatingError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.0)
        }
    }

    struct GenericContextProbe {
        form: FormHandle<GenericFieldForm, RatingError>,
        binding: RefCell<Option<Binding<Rating>>>,
        meta: RefCell<Option<FieldMeta>>,
    }

    #[derive(Clone, Props)]
    struct GenericRatingProps {
        probe: Rc<GenericContextProbe>,
    }

    impl PartialEq for GenericRatingProps {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.probe, &other.probe)
        }
    }

    #[allow(non_snake_case)]
    fn GenericRating(props: GenericRatingProps) -> Element {
        let binding = use_binding(None, Rating(0));
        let meta = use_field_meta(None);
        props.probe.binding.borrow_mut().replace(binding);
        props.probe.meta.borrow_mut().replace(meta);

        rsx! { div { ..meta.attributes() } }
    }

    fn generic_context_field(probe: Rc<GenericContextProbe>) -> Element {
        rsx! {
            Field {
                context: probe.form.field(GenericFieldForm::fields().rating()),
                GenericRating { probe }
                FieldError { id: "rating-error" }
            }
        }
    }

    #[test]
    fn field_handle_directly_provides_context_metadata_and_visible_errors() {
        let form = FormHandle::new_with_error_type(GenericFieldForm { rating: Rating(3) })
            .with_id_namespace("survey");
        form.field(GenericFieldForm::fields().rating())
            .validator("minimum")
            .on(ValidationTrigger::Commit)
            .check_optional(|rating, _context| {
                (rating.0 < 5).then_some(RatingError("Choose at least 5"))
            });
        let probe = Rc::new(GenericContextProbe {
            form,
            binding: RefCell::new(None),
            meta: RefCell::new(None),
        });
        let mut dom = VirtualDom::new_with_props(generic_context_field, Rc::clone(&probe));
        dom.rebuild_in_place();

        let binding = probe
            .binding
            .borrow()
            .clone()
            .expect("component should expose the generic binding");
        let meta = probe
            .meta
            .borrow()
            .expect("component should expose generic field metadata");
        assert_eq!(meta.id().as_ref(), "survey-rating-input");
        assert_eq!(meta.name().as_deref(), Some("rating"));
        assert!(!meta.invalid());
        assert!(!meta.touched());
        assert!(!meta.dirty());

        binding.write(Rating(4), ChangeOrigin::User);
        binding.commit();
        render_reactive_updates(&mut dom);

        assert!(meta.invalid());
        assert!(meta.touched());
        assert!(meta.dirty());
        assert_eq!(meta.errors(), vec![Rc::from("Choose at least 5")]);
        assert!(dioxus_ssr::render(&dom).contains("<div>Choose at least 5</div>"));
    }

    #[derive(Clone, dioform::Form)]
    struct QuantityForm {
        quantity: u32,
    }

    struct ParsedTextProbe {
        form: FormHandle<QuantityForm>,
        binding: RefCell<Option<Binding<String>>>,
        rendered: RefCell<Vec<String>>,
        equal_bindings_match: Cell<bool>,
    }

    fn parsed_text_field(probe: Rc<ParsedTextProbe>) -> Element {
        let quantity = dioform::use_number(&probe.form, QuantityForm::fields().quantity());
        let binding: Binding<String> = quantity.clone().into();
        let same_binding: Binding<String> = quantity.clone().into();

        probe.rendered.borrow_mut().push((binding.read)());
        probe.equal_bindings_match.set(binding == same_binding);
        probe.binding.borrow_mut().replace(binding);

        rsx! {
            Field { context: quantity,
                FieldError { id: "quantity-error" }
            }
        }
    }

    fn parsed_text_probe() -> (Rc<ParsedTextProbe>, VirtualDom) {
        let probe = Rc::new(ParsedTextProbe {
            form: FormHandle::new(QuantityForm { quantity: 3 }),
            binding: RefCell::new(None),
            rendered: RefCell::new(Vec::new()),
            equal_bindings_match: Cell::new(false),
        });
        let mut dom = VirtualDom::new_with_props(parsed_text_field, Rc::clone(&probe));
        dom.rebuild_in_place();

        (probe, dom)
    }

    fn convention_binding<Value: 'static>(
        binding: &RefCell<Option<Binding<Value>>>,
    ) -> Binding<Value> {
        binding
            .borrow()
            .as_ref()
            .expect("component should expose the convention binding")
            .clone()
    }

    #[test]
    fn parsed_binding_renders_the_formatted_field_value() {
        let (probe, mut dom) = parsed_text_probe();

        convention_binding(&probe.binding).write("5".to_owned(), ChangeOrigin::User);
        render_reactive_updates(&mut dom);

        assert_eq!(probe.form.field_value(QuantityForm::fields().quantity()), 5);
        assert!(
            probe
                .form
                .is_field_touched(QuantityForm::fields().quantity())
        );
        assert_eq!(
            probe.rendered.borrow().first().map(String::as_str),
            Some("3")
        );
        assert_eq!(
            probe.rendered.borrow().last().map(String::as_str),
            Some("5")
        );
    }

    #[test]
    fn parsed_binding_keeps_a_programmatic_rendered_write_out_of_interaction_state() {
        let (probe, mut dom) = parsed_text_probe();

        convention_binding(&probe.binding).write("7".to_owned(), ChangeOrigin::Programmatic);
        render_reactive_updates(&mut dom);

        assert_eq!(probe.form.field_value(QuantityForm::fields().quantity()), 7);
        assert!(
            !probe
                .form
                .is_field_touched(QuantityForm::fields().quantity())
        );
        assert_eq!(
            probe.rendered.borrow().last().map(String::as_str),
            Some("7")
        );
    }

    #[test]
    fn parsed_binding_reports_an_unresolved_parse_error_as_a_convention_error() {
        let (probe, mut dom) = parsed_text_probe();
        let binding = convention_binding(&probe.binding);

        binding.write("twelve".to_owned(), ChangeOrigin::User);
        render_reactive_updates(&mut dom);

        // The typed field keeps its last parsable value while the rendered text holds what the user
        // typed, and the Parse Blocker is what the Field reports.
        assert_eq!(probe.form.field_value(QuantityForm::fields().quantity()), 3);
        assert_eq!(
            probe.rendered.borrow().last().map(String::as_str),
            Some("twelve")
        );
        assert_eq!(
            dioxus_ssr::render(&dom),
            "<div><div id=\"quantity-error\" aria-live=\"polite\" data-invalid=\"true\" data-touched=\"true\"><div>invalid digit found in string</div></div></div>"
        );

        binding.write("12".to_owned(), ChangeOrigin::User);
        render_reactive_updates(&mut dom);

        assert_eq!(
            probe.form.field_value(QuantityForm::fields().quantity()),
            12
        );
        assert_eq!(
            dioxus_ssr::render(&dom),
            "<div><div id=\"quantity-error\" aria-live=\"polite\" data-dirty=\"true\" data-touched=\"true\"></div></div>"
        );
    }

    #[derive(Clone, dioform::Form)]
    struct NicknameForm {
        nickname: Option<String>,
    }

    struct OptionalTextProbe {
        form: FormHandle<NicknameForm>,
        binding: RefCell<Option<Binding<String>>>,
        rendered: RefCell<Vec<String>>,
    }

    #[derive(Clone, Props)]
    struct OptionalTextInputProps {
        probe: Rc<OptionalTextProbe>,
    }

    impl PartialEq for OptionalTextInputProps {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.probe, &other.probe)
        }
    }

    /// A Field Convention text control: it resolves `Binding<String>` from the context, which is
    /// the composition where an optional-text context used to raise a `BindingTypeMismatch`.
    #[allow(non_snake_case)]
    fn OptionalTextInput(props: OptionalTextInputProps) -> Element {
        let binding = use_binding::<String>(None, String::new());
        props.probe.rendered.borrow_mut().push((binding.read)());
        props.probe.binding.borrow_mut().replace(binding);

        VNode::empty()
    }

    fn optional_text_field(probe: Rc<OptionalTextProbe>) -> Element {
        let nickname = dioform::use_optional_text(&probe.form, NicknameForm::fields().nickname());

        rsx! {
            Field { context: nickname,
                OptionalTextInput { probe }
            }
        }
    }

    fn optional_text_probe(initial: Option<String>) -> (Rc<OptionalTextProbe>, VirtualDom) {
        let probe = Rc::new(OptionalTextProbe {
            form: FormHandle::new(NicknameForm { nickname: initial }),
            binding: RefCell::new(None),
            rendered: RefCell::new(Vec::new()),
        });
        let mut dom = VirtualDom::new_with_props(optional_text_field, Rc::clone(&probe));
        dom.rebuild_in_place();

        (probe, dom)
    }

    #[test]
    fn optional_text_context_resolves_a_rendered_text_binding_for_a_text_control() {
        let (probe, mut dom) = optional_text_probe(Some("ada".to_owned()));
        let binding = convention_binding(&probe.binding);

        binding.write("grace".to_owned(), ChangeOrigin::User);
        render_reactive_updates(&mut dom);

        assert_eq!(
            probe.form.field_value(NicknameForm::fields().nickname()),
            Some("grace".to_owned())
        );
        assert!(
            probe
                .form
                .is_field_touched(NicknameForm::fields().nickname())
        );
        assert_eq!(
            probe.rendered.borrow().first().map(String::as_str),
            Some("ada")
        );
        assert_eq!(
            probe.rendered.borrow().last().map(String::as_str),
            Some("grace")
        );
    }

    #[test]
    fn optional_text_binding_renders_an_absent_value_as_empty_text() {
        let (probe, _dom) = optional_text_probe(None);

        assert_eq!(
            probe.rendered.borrow().first().map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn optional_text_binding_writes_empty_user_input_as_absent() {
        let (probe, mut dom) = optional_text_probe(Some("ada".to_owned()));
        let binding = convention_binding(&probe.binding);

        binding.write(String::new(), ChangeOrigin::User);
        render_reactive_updates(&mut dom);

        assert_eq!(
            probe.form.field_value(NicknameForm::fields().nickname()),
            None
        );
        assert!(
            probe
                .form
                .is_field_touched(NicknameForm::fields().nickname())
        );
        assert_eq!(probe.rendered.borrow().last().map(String::as_str), Some(""));
    }

    #[test]
    fn optional_text_binding_applies_the_presence_rule_to_programmatic_writes() {
        let (probe, mut dom) = optional_text_probe(Some("ada".to_owned()));
        let binding = convention_binding(&probe.binding);

        binding.write(String::new(), ChangeOrigin::Programmatic);
        render_reactive_updates(&mut dom);

        // A Programmatic convention write of "" lands as `None`, never as `Some("")`: the
        // ADR-0046 presence rule travels with the rendered-text write regardless of origin.
        assert_eq!(
            probe.form.field_value(NicknameForm::fields().nickname()),
            None
        );
        assert!(
            !probe
                .form
                .is_field_touched(NicknameForm::fields().nickname())
        );
        assert!(probe.form.is_field_dirty(NicknameForm::fields().nickname()));

        binding.write("grace".to_owned(), ChangeOrigin::Programmatic);
        render_reactive_updates(&mut dom);

        assert_eq!(
            probe.form.field_value(NicknameForm::fields().nickname()),
            Some("grace".to_owned())
        );
        assert!(
            !probe
                .form
                .is_field_touched(NicknameForm::fields().nickname())
        );
        assert_eq!(
            probe.rendered.borrow().last().map(String::as_str),
            Some("grace")
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

    struct ScalarBindingsProbe {
        form: FormHandle<ScalarForm>,
        values_match: Cell<bool>,
        equal_bindings_match: Cell<bool>,
    }

    fn scalar_bindings_convert(probe: Rc<ScalarBindingsProbe>) -> Element {
        let form = &probe.form;
        let fields = ScalarForm::fields();
        let text: Binding<String> = form.text(fields.text()).into();
        let textarea: Binding<String> = form.textarea(fields.text()).into();
        let optional_text: Binding<Option<String>> =
            form.optional_text(fields.optional_text()).into();
        let rendered_optional_text: Binding<String> =
            form.optional_text(fields.optional_text()).into();
        let checked: Binding<bool> = form.checkbox(fields.checked()).into();
        let same_checked: Binding<bool> = form.checkbox(fields.checked()).into();
        let tri_state: Binding<Option<bool>> = form.tri_state_checkbox(fields.tri_state()).into();
        let select: Binding<i32> = form.select(fields.choice()).into();
        let rendered_select: Binding<i32> = form
            .select_with(fields.choice(), str::parse::<i32>, i32::to_string)
            .into();
        let radio: Binding<i32> = form.radio_group(fields.choice()).into();

        probe.values_match.set(
            (text.read)().as_str() == "text"
                && (textarea.read)().as_str() == "text"
                && (optional_text.read)() == Some("optional".to_owned())
                && (rendered_optional_text.read)().as_str() == "optional"
                && (checked.read)()
                && (tri_state.read)().is_none()
                && (select.read)() == 7
                && (rendered_select.read)() == 7
                && (radio.read)() == 7,
        );
        probe.equal_bindings_match.set(checked == same_checked);

        VNode::empty()
    }

    #[test]
    fn every_scalar_binding_produces_a_field_convention_binding() {
        let probe = Rc::new(ScalarBindingsProbe {
            form: FormHandle::new(ScalarForm {
                text: "text".to_owned(),
                optional_text: Some("optional".to_owned()),
                checked: true,
                tri_state: None,
                choice: 7,
            }),
            values_match: Cell::new(false),
            equal_bindings_match: Cell::new(false),
        });
        let mut dom = VirtualDom::new_with_props(scalar_bindings_convert, Rc::clone(&probe));

        dom.rebuild_in_place();

        assert!(probe.values_match.get());
        assert!(probe.equal_bindings_match.get());
    }
}
