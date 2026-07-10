use std::{
    cell::{Cell, RefCell},
    fmt::Debug,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
    time::Duration,
};

#[cfg(feature = "serde")]
use dioform::advanced::FormStateRestoreError;
use dioform::advanced::{
    CollectionItemIdentity, FieldUpdateOrigin, FormCore, FormObserverEvent, SubmitAttempt,
    ValidatorId,
};
use dioform::{
    CollectionParsedTextBinding, CollectionTextBinding, FieldAccessibility, FieldBindingLifecycle,
    FieldPath, FileFieldKey, Form, FormConfig, FormHandle, FormIdNamespace, FormListenerEvent,
    FormValidationError, ParsedTextBinding, ProgressiveSubmitResult, SelectedFile,
    SelectedFileMetadata, SubmissionSnapshot, SubmitBlocker, SubmitError, SubmitErrors,
    SubmitListenerEvent, SubmitResult, SubmitStatus, ValidationMode, ValidationStatus,
    ValidationTarget, ValidationTrigger, ValidationTriggers, debounce_duration,
    provide_form_context, try_use_form_context, use_collection_item_date,
    use_collection_item_number, use_date, use_date_with, use_debounced_field_listener_for_origin,
    use_debounced_form_listener_for_origin, use_field_binding_listener, use_field_blur_listener,
    use_field_listener, use_field_listener_for_origin, use_form_blur_listener, use_form_config,
    use_form_context, use_form_handle, use_form_listener, use_form_listener_for_origin,
    use_multi_select, use_number, use_number_with, use_parsed_text, use_parsed_text_with,
    use_radio_group, use_select, use_select_with, use_submit_listener,
};
use dioxus::prelude::{Props, dioxus_signals, rsx};
use dioxus_core::{Element, Event, VNode, VirtualDom, use_hook};

fn managed_submit_event() -> Event<()> {
    Event::new(Rc::new(()), true)
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct SignupForm {
    email: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SignupSubmitIntent {
    SaveDraft,
    Publish,
}

#[derive(Clone, Debug, Form)]
struct UploadForm {
    token: UploadToken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UploadToken {
    token: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DateYmd {
    year: u16,
    month: u8,
    day: u8,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct DateForm {
    check_in: DateYmd,
    check_out: DateYmd,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct ParsedLifecycleForm {
    age: u8,
    token: UploadToken,
    check_in: DateYmd,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct InvoiceCollectionForm {
    lines: Vec<InvoiceCollectionLine>,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct InvoiceCollectionLine {
    description: String,
    quantity: u32,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct NestedInvoiceCollectionForm {
    invoice: NestedInvoice,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct NestedInvoice {
    #[form(name = "invoice_lines")]
    lines: Vec<NestedInvoiceLine>,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct NestedInvoiceLine {
    product: NestedProduct,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct NestedProduct {
    #[form(name = "product-name")]
    name: String,
}

fn invoice_collection_form() -> InvoiceCollectionForm {
    InvoiceCollectionForm {
        lines: vec![
            InvoiceCollectionLine {
                description: "Design".to_owned(),
                quantity: 2,
            },
            InvoiceCollectionLine {
                description: "Build".to_owned(),
                quantity: 1,
            },
        ],
    }
}

fn nested_invoice_collection_form() -> NestedInvoiceCollectionForm {
    NestedInvoiceCollectionForm {
        invoice: NestedInvoice {
            lines: vec![NestedInvoiceLine {
                product: NestedProduct {
                    name: "Keyboard".to_owned(),
                },
            }],
        },
    }
}

struct AsyncGate<T> {
    inner: Rc<RefCell<AsyncGateState<T>>>,
}

impl<T> Clone for AsyncGate<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

struct AsyncGateState<T> {
    output: Option<T>,
    waker: Option<Waker>,
}

struct AsyncGateFuture<T> {
    inner: Rc<RefCell<AsyncGateState<T>>>,
}

#[derive(Clone, Default)]
struct ManualDelays {
    states: Rc<RefCell<Vec<Rc<RefCell<ManualDelayState>>>>>,
}

#[derive(Default)]
struct ManualDelayState {
    completed: bool,
    waker: Option<Waker>,
}

struct ManualDelay {
    state: Rc<RefCell<ManualDelayState>>,
}

type InputHandler = dyn Fn(String);
type ActionHandler = dyn Fn();

impl<T> Default for AsyncGate<T> {
    fn default() -> Self {
        Self {
            inner: Rc::new(RefCell::new(AsyncGateState {
                output: None,
                waker: None,
            })),
        }
    }
}

impl<T> AsyncGate<T> {
    fn future(&self) -> AsyncGateFuture<T> {
        AsyncGateFuture {
            inner: Rc::clone(&self.inner),
        }
    }

    fn complete(&self, output: T) {
        let waker = {
            let mut state = self.inner.borrow_mut();

            assert!(state.output.is_none(), "async gate completed twice");
            state.output = Some(output);
            state.waker.take()
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl ManualDelays {
    fn future(&self) -> ManualDelay {
        let state = Rc::new(RefCell::new(ManualDelayState::default()));
        self.states.borrow_mut().push(Rc::clone(&state));
        ManualDelay { state }
    }

    fn complete(&self, index: usize) {
        let state = self
            .states
            .borrow()
            .get(index)
            .expect("manual delay should exist")
            .clone();
        let waker = {
            let mut state = state.borrow_mut();

            assert!(!state.completed, "manual delay completed twice");
            state.completed = true;
            state.waker.take()
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }

    fn len(&self) -> usize {
        self.states.borrow().len()
    }
}

impl<T> Future for AsyncGateFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.inner.borrow_mut();

        match state.output.take() {
            Some(output) => Poll::Ready(output),
            None => {
                state.waker = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl Future for ManualDelay {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.borrow_mut();

        if state.completed {
            Poll::Ready(())
        } else {
            state.waker = Some(context.waker().clone());
            Poll::Pending
        }
    }
}

struct DropCountingDelay {
    drops: Rc<Cell<usize>>,
}

impl Future for DropCountingDelay {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

impl Drop for DropCountingDelay {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

struct DropCountingValidation {
    drops: Rc<Cell<usize>>,
}

impl Future for DropCountingValidation {
    type Output = Vec<&'static str>;

    fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

impl Drop for DropCountingValidation {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

struct DropCountingFormValidation {
    drops: Rc<Cell<usize>>,
}

impl Future for DropCountingFormValidation {
    type Output = Vec<FormValidationError<&'static str>>;

    fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

impl Drop for DropCountingFormValidation {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn derive_generates_a_typed_direct_field_path_for_a_text_field() {
    let email = SignupForm::fields().email();
    let model = SignupForm {
        email: "ada@example.com".to_owned(),
    };

    assert_eq!(email.identity().as_str(), "email");
    assert_eq!(email.field_name(), "email");
    assert_eq!(email.get(&model), "ada@example.com");
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct ProfileForm {
    #[form(name = "contact-email")]
    email: String,
    #[form(name = "accepted_terms")]
    accepts_terms: bool,
}

#[derive(Default)]
struct FieldListenerProbe {
    handle: RefCell<Option<FormHandle<ProfileForm>>>,
    listener_runs: Cell<usize>,
}

#[derive(Default)]
struct ListenerInvariantProbe {
    handle: RefCell<Option<FormHandle<ProfileForm, &'static str>>>,
    observer_events: RefCell<Vec<FormObserverEvent>>,
    selector_snapshots: RefCell<Vec<(bool, bool, usize)>>,
}

#[derive(Default)]
struct FormListenerProbe {
    handle: RefCell<Option<FormHandle<ProfileForm>>>,
    events: RefCell<Vec<(String, FieldUpdateOrigin)>>,
    rich_events: RefCell<Vec<(String, String, FormListenerEvent, FieldUpdateOrigin)>>,
    blur_events: RefCell<Vec<String>>,
    rich_blur_events: RefCell<Vec<(String, String)>>,
    autosaved_snapshots: RefCell<Vec<ProfileForm>>,
}

#[derive(Default)]
struct CollectionBlurListenerProbe {
    description:
        RefCell<Option<CollectionTextBinding<InvoiceCollectionForm, InvoiceCollectionLine>>>,
    events: RefCell<Vec<(String, String, Option<String>)>>,
}

#[derive(Default)]
struct BindingListenerProbe {
    events: RefCell<Vec<(String, FieldBindingLifecycle)>>,
}

#[derive(Default)]
struct DebouncedListenerProbe {
    delays: ManualDelays,
    handle: RefCell<Option<FormHandle<ProfileForm>>>,
    snapshots: RefCell<Vec<String>>,
}

#[derive(Default)]
struct DebouncedFormListenerProbe {
    delays: ManualDelays,
    handle: RefCell<Option<FormHandle<ProfileForm>>>,
    events: RefCell<Vec<(String, String)>>,
}

#[derive(Default)]
struct SubmitListenerProbe {
    handle: RefCell<Option<FormHandle<ProfileForm>>>,
    events: RefCell<Vec<SubmitListenerEvent>>,
}

#[derive(Default)]
struct IntentSubmitListenerProbe {
    handle: RefCell<Option<FormHandle<ProfileForm>>>,
    events: RefCell<Vec<(SubmitListenerEvent, Option<SignupSubmitIntent>)>>,
}

#[derive(Default)]
struct MultiSelectListenerProbe {
    topics: RefCell<Option<dioform::MultiSelectBinding<MultiSelectForm, Topic, &'static str>>>,
    field_events: RefCell<Vec<FieldUpdateOrigin>>,
    form_events: RefCell<Vec<(String, String, FormListenerEvent, FieldUpdateOrigin)>>,
}

#[derive(Default)]
struct ManagedSubmitListenerProbe {
    submit: AsyncGate<()>,
    handle: RefCell<Option<FormHandle<ProfileForm>>>,
    submit_result: RefCell<Option<SubmitResult>>,
    submit_calls: Cell<u32>,
    events: RefCell<Vec<SubmitListenerEvent>>,
}

#[derive(Debug, Eq, PartialEq)]
struct ListenerParseBlockerSnapshot {
    field_name: String,
    parse_error_count: usize,
    can_submit: bool,
    submit_result: SubmitResult,
}

#[derive(Default)]
struct ParsedListenerParseBlockerProbe {
    age: RefCell<Option<ParsedTextBinding<ParsedLifecycleForm, u8>>>,
    snapshots: RefCell<Vec<ListenerParseBlockerSnapshot>>,
}

#[derive(Default)]
struct CollectionListenerParseBlockerProbe {
    quantity: RefCell<
        Option<CollectionParsedTextBinding<InvoiceCollectionForm, InvoiceCollectionLine, u32>>,
    >,
    snapshots: RefCell<Vec<ListenerParseBlockerSnapshot>>,
}

#[derive(Default)]
struct ParsedSubmitListenerProbe {
    handle: RefCell<Option<FormHandle<ParsedLifecycleForm>>>,
    age: RefCell<Option<ParsedTextBinding<ParsedLifecycleForm, u8>>>,
    events: RefCell<Vec<SubmitListenerEvent>>,
}

fn field_listener_dependent_reset_probe(probe: Rc<FieldListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: true,
        })
    });
    let email_path = ProfileForm::fields().email();
    let accepts_terms_path = ProfileForm::fields().accepts_terms();
    let listener_probe = Rc::clone(&probe);

    use_field_listener(form.clone(), email_path, move |context| {
        assert_eq!(context.origin(), FieldUpdateOrigin::User);
        listener_probe
            .listener_runs
            .set(listener_probe.listener_runs.get() + 1);
        context.form().set_field(accepts_terms_path.clone(), false);
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn listener_caused_update_invariant_probe(probe: Rc<ListenerInvariantProbe>) -> Element {
    let init_probe = Rc::clone(&probe);
    let form = use_form_handle(move || {
        let accepts_terms_path = ProfileForm::fields().accepts_terms();
        let handle: FormHandle<ProfileForm, &'static str> = FormHandle::from_config(
            FormConfig::new(ProfileForm {
                email: "initial@example.com".to_owned(),
                accepts_terms: true,
            })
            .validation_mode(ValidationMode::on_change())
            .field_validator(accepts_terms_path, "accepted")
            .on(ValidationTrigger::Change)
            .check_optional(|value, _context| (!*value).then_some("terms_required")),
        );
        let observer_probe = Rc::clone(&init_probe);

        handle.write_advanced(|core| {
            core.observe(move |event| {
                observer_probe
                    .observer_events
                    .borrow_mut()
                    .push(event.clone());
            });
        });

        handle
    });
    let email_path = ProfileForm::fields().email();
    let accepts_terms_path = ProfileForm::fields().accepts_terms();
    let listener_accepts_terms_path = accepts_terms_path.clone();

    use_field_listener_for_origin(
        form.clone(),
        email_path,
        FieldUpdateOrigin::User,
        move |context| {
            context
                .form()
                .set_field(listener_accepts_terms_path.clone(), false);
        },
    );

    probe.selector_snapshots.borrow_mut().push((
        form.field_value(accepts_terms_path.clone()),
        form.is_field_touched(accepts_terms_path.clone()),
        form.field_validation_errors(accepts_terms_path.clone())
            .len(),
    ));
    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn field_listener_same_field_programmatic_update_probe(probe: Rc<FieldListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: true,
        })
    });
    let email_path = ProfileForm::fields().email();
    let listener_email_path = email_path.clone();
    let listener_probe = Rc::clone(&probe);

    use_field_listener_for_origin(
        form.clone(),
        email_path,
        FieldUpdateOrigin::User,
        move |context| {
            assert_eq!(context.origin(), FieldUpdateOrigin::User);
            listener_probe
                .listener_runs
                .set(listener_probe.listener_runs.get() + 1);
            assert_eq!(
                context.form().field_value(listener_email_path.clone()),
                "Ada@Example.COM"
            );
            context
                .form()
                .set_field(listener_email_path.clone(), "ada@example.com".to_owned());
        },
    );

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn field_listener_same_field_cycle_probe(probe: Rc<FieldListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: true,
        })
    });
    let email_path = ProfileForm::fields().email();
    let listener_email_path = email_path.clone();
    let listener_probe = Rc::clone(&probe);

    use_field_listener(form.clone(), email_path, move |context| {
        listener_probe
            .listener_runs
            .set(listener_probe.listener_runs.get() + 1);
        context
            .form()
            .set_field(listener_email_path.clone(), "ada@example.com".to_owned());
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn form_listener_field_identification_probe(probe: Rc<FormListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: false,
        })
    });
    let listener_probe = Rc::clone(&probe);

    use_form_listener(form.clone(), move |context| {
        listener_probe.rich_events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.field_name().to_owned(),
            context.event(),
            context.origin(),
        ));
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.origin(),
        ));
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn field_blur_listener_dependent_reset_probe(probe: Rc<FieldListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: true,
        })
    });
    let email_path = ProfileForm::fields().email();
    let accepts_terms_path = ProfileForm::fields().accepts_terms();
    let listener_probe = Rc::clone(&probe);

    use_field_blur_listener(form.clone(), email_path, move |context| {
        assert_eq!(context.field_identity().as_str(), "email");
        listener_probe
            .listener_runs
            .set(listener_probe.listener_runs.get() + 1);
        context.form().set_field(accepts_terms_path.clone(), false);
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn form_blur_listener_field_identification_probe(probe: Rc<FormListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: false,
        })
    });
    let listener_probe = Rc::clone(&probe);

    use_form_blur_listener(form.clone(), move |context| {
        let field = context.field_identity();
        listener_probe
            .blur_events
            .borrow_mut()
            .push(field.as_str().to_owned());
        listener_probe
            .rich_blur_events
            .borrow_mut()
            .push((field.as_str().to_owned(), context.field_name().to_owned()));
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn collection_item_form_blur_listener_probe(probe: Rc<CollectionBlurListenerProbe>) -> Element {
    let form = use_form_handle(|| FormHandle::new(invoice_collection_form()));
    let listener_probe = Rc::clone(&probe);

    use_form_blur_listener(form.clone(), move |context| {
        let field = context.field_identity();
        listener_probe.events.borrow_mut().push((
            field.as_str().to_owned(),
            context.field_name().to_owned(),
            field.collection_path().map(str::to_owned),
        ));
    });

    let lines = form.collection(InvoiceCollectionForm::fields().lines());
    let description = lines.items()[0].text(InvoiceCollectionLine::fields().description());
    probe.description.borrow_mut().replace(description);

    VNode::empty()
}

fn form_listener_autosave_snapshot_probe(probe: Rc<FormListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: false,
        })
    });
    let listener_probe = Rc::clone(&probe);

    use_form_listener_for_origin(form.clone(), FieldUpdateOrigin::User, move |context| {
        listener_probe
            .autosaved_snapshots
            .borrow_mut()
            .push(context.form().snapshot());
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn debounced_field_listener_probe(probe: Rc<DebouncedListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: false,
        })
    });
    let email_path = ProfileForm::fields().email();
    let delays = probe.delays.clone();
    let listener_probe = Rc::clone(&probe);

    use_debounced_field_listener_for_origin(
        form.clone(),
        email_path,
        FieldUpdateOrigin::User,
        move || delays.future(),
        move |context| {
            listener_probe
                .snapshots
                .borrow_mut()
                .push(context.form().field_value(ProfileForm::fields().email()));
        },
    );

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn debounced_form_listener_probe(probe: Rc<DebouncedFormListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: false,
        })
    });
    let delays = probe.delays.clone();
    let listener_probe = Rc::clone(&probe);

    use_debounced_form_listener_for_origin(
        form.clone(),
        FieldUpdateOrigin::User,
        move || delays.future(),
        move |context| {
            listener_probe.events.borrow_mut().push((
                context.field_identity().as_str().to_owned(),
                context.form().field_value(ProfileForm::fields().email()),
            ));
        },
    );

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn field_binding_lifecycle_probe(probe: Rc<BindingListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: false,
        })
    });
    let email_path = ProfileForm::fields().email();
    let listener_probe = Rc::clone(&probe);

    use_field_binding_listener(form.clone(), email_path.clone(), move |context| {
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.lifecycle(),
        ));
    });
    let _binding = use_parsed_text(form, email_path);

    VNode::empty()
}

fn field_binding_listener_after_binding_probe(probe: Rc<BindingListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: false,
        })
    });
    let email_path = ProfileForm::fields().email();
    let listener_probe = Rc::clone(&probe);

    let _binding = use_parsed_text(form.clone(), email_path.clone());
    use_field_binding_listener(form, email_path, move |context| {
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.lifecycle(),
        ));
    });

    VNode::empty()
}

fn number_binding_lifecycle_probe(probe: Rc<BindingListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ParsedLifecycleForm {
            age: 42,
            token: UploadToken {
                token: "initial".to_owned(),
            },
            check_in: DateYmd {
                year: 2026,
                month: 7,
                day: 2,
            },
        })
    });
    let age_path = ParsedLifecycleForm::fields().age();
    let listener_probe = Rc::clone(&probe);

    use_field_binding_listener(form.clone(), age_path.clone(), move |context| {
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.lifecycle(),
        ));
    });
    let _binding = use_number(form, age_path);

    VNode::empty()
}

fn custom_parsed_binding_lifecycle_probe(probe: Rc<BindingListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ParsedLifecycleForm {
            age: 42,
            token: UploadToken {
                token: "initial".to_owned(),
            },
            check_in: DateYmd {
                year: 2026,
                month: 7,
                day: 2,
            },
        })
    });
    let token_path = ParsedLifecycleForm::fields().token();
    let listener_probe = Rc::clone(&probe);

    use_field_binding_listener(form.clone(), token_path.clone(), move |context| {
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.lifecycle(),
        ));
    });
    let _binding = use_parsed_text_with(
        form,
        token_path,
        |value: &str| {
            Ok::<_, String>(UploadToken {
                token: value.to_owned(),
            })
        },
        |value| value.token.clone(),
    );

    VNode::empty()
}

fn date_binding_lifecycle_probe(probe: Rc<BindingListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ParsedLifecycleForm {
            age: 42,
            token: UploadToken {
                token: "initial".to_owned(),
            },
            check_in: DateYmd {
                year: 2026,
                month: 7,
                day: 2,
            },
        })
    });
    let check_in_path = ParsedLifecycleForm::fields().check_in();
    let listener_probe = Rc::clone(&probe);

    use_field_binding_listener(form.clone(), check_in_path.clone(), move |context| {
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.lifecycle(),
        ));
    });
    let _binding = use_date_with(form, check_in_path, parse_date_ymd, format_date_ymd);

    VNode::empty()
}

fn default_date_binding_lifecycle_probe(probe: Rc<BindingListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ParsedLifecycleForm {
            age: 42,
            token: UploadToken {
                token: "initial".to_owned(),
            },
            check_in: DateYmd {
                year: 2026,
                month: 7,
                day: 2,
            },
        })
    });
    let check_in_path = ParsedLifecycleForm::fields().check_in();
    let listener_probe = Rc::clone(&probe);

    use_field_binding_listener(form.clone(), check_in_path.clone(), move |context| {
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.lifecycle(),
        ));
    });
    let _binding = use_date(form, check_in_path);

    VNode::empty()
}

fn custom_number_binding_lifecycle_probe(probe: Rc<BindingListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ParsedLifecycleForm {
            age: 42,
            token: UploadToken {
                token: "initial".to_owned(),
            },
            check_in: DateYmd {
                year: 2026,
                month: 7,
                day: 2,
            },
        })
    });
    let age_path = ParsedLifecycleForm::fields().age();
    let listener_probe = Rc::clone(&probe);

    use_field_binding_listener(form.clone(), age_path.clone(), move |context| {
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.lifecycle(),
        ));
    });
    let _binding = use_number_with(
        form,
        age_path,
        |value: &str| value.parse::<u8>().map_err(|error| error.to_string()),
        |value| value.to_string(),
    );

    VNode::empty()
}

fn select_binding_lifecycle_probe(probe: Rc<BindingListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(PlanForm {
            plan: Plan::Starter,
        })
    });
    let plan_path = PlanForm::fields().plan();
    let listener_probe = Rc::clone(&probe);

    use_field_binding_listener(form.clone(), plan_path.clone(), move |context| {
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.lifecycle(),
        ));
    });
    let _binding = use_select(form, plan_path);

    VNode::empty()
}

fn radio_binding_lifecycle_probe(probe: Rc<BindingListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(PlanForm {
            plan: Plan::Starter,
        })
    });
    let plan_path = PlanForm::fields().plan();
    let listener_probe = Rc::clone(&probe);

    use_field_binding_listener(form.clone(), plan_path.clone(), move |context| {
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.lifecycle(),
        ));
    });
    let _binding = use_radio_group(form, plan_path);

    VNode::empty()
}

fn rendered_select_binding_lifecycle_probe(probe: Rc<BindingListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(PlanForm {
            plan: Plan::Starter,
        })
    });
    let plan_path = PlanForm::fields().plan();
    let listener_probe = Rc::clone(&probe);

    use_field_binding_listener(form.clone(), plan_path.clone(), move |context| {
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.lifecycle(),
        ));
    });
    let _binding = use_select_with(form, plan_path, parse_plan, format_plan);

    VNode::empty()
}

fn multi_select_binding_lifecycle_probe(probe: Rc<BindingListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(MultiSelectForm {
            topics: vec![Topic::Rust],
        })
    });
    let topics_path = MultiSelectForm::fields().topics();
    let listener_probe = Rc::clone(&probe);

    use_field_binding_listener(form.clone(), topics_path.clone(), move |context| {
        listener_probe.events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.lifecycle(),
        ));
    });
    let _binding = use_multi_select(form, topics_path);

    VNode::empty()
}

fn multi_select_listener_probe(probe: Rc<MultiSelectListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new_with_error_type(MultiSelectForm {
            topics: vec![Topic::Rust],
        })
    });
    let topics_path = MultiSelectForm::fields().topics();
    let field_listener_probe = Rc::clone(&probe);
    let form_listener_probe = Rc::clone(&probe);

    use_field_listener(form.clone(), topics_path.clone(), move |context| {
        field_listener_probe
            .field_events
            .borrow_mut()
            .push(context.origin());
    });
    use_form_listener(form.clone(), move |context| {
        form_listener_probe.form_events.borrow_mut().push((
            context.field_identity().as_str().to_owned(),
            context.field_name().to_owned(),
            context.event(),
            context.origin(),
        ));
    });

    let topics = use_multi_select(form, topics_path);
    probe.topics.borrow_mut().replace(topics);

    VNode::empty()
}

fn submit_listener_probe(probe: Rc<SubmitListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: false,
        })
    });
    let listener_probe = Rc::clone(&probe);

    use_submit_listener(form.clone(), move |context| {
        listener_probe.events.borrow_mut().push(context.event());
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn intent_submit_listener_probe(probe: Rc<IntentSubmitListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: false,
        })
    });
    let listener_probe = Rc::clone(&probe);

    use_submit_listener(form.clone(), move |context| {
        listener_probe.events.borrow_mut().push((
            context.event(),
            context.submit_intent::<SignupSubmitIntent>().copied(),
        ));
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn managed_submit_listener_probe(probe: Rc<ManagedSubmitListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ProfileForm {
            email: "initial@example.com".to_owned(),
            accepts_terms: false,
        })
    });
    let listener_probe = Rc::clone(&probe);

    use_submit_listener(form.clone(), move |context| {
        listener_probe.events.borrow_mut().push(context.event());
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let submit = probe.submit.clone();
            let submit_probe = Rc::clone(&probe);
            let result =
                form.managed_submit()
                    .on_submit_async(managed_submit_event(), move |_submitted| {
                        submit_probe
                            .submit_calls
                            .set(submit_probe.submit_calls.get() + 1);
                        submit.future()
                    });

            probe.submit_result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn parsed_listener_parse_blocker_probe(probe: Rc<ParsedListenerParseBlockerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ParsedLifecycleForm {
            age: 42,
            token: UploadToken {
                token: "initial".to_owned(),
            },
            check_in: DateYmd {
                year: 2026,
                month: 7,
                day: 2,
            },
        })
    });
    let listener_probe = Rc::clone(&probe);

    use_form_listener_for_origin(form.clone(), FieldUpdateOrigin::User, move |context| {
        listener_probe
            .snapshots
            .borrow_mut()
            .push(ListenerParseBlockerSnapshot {
                field_name: context.field_name().to_owned(),
                parse_error_count: context.form().parse_errors().len(),
                can_submit: context.form().can_submit(),
                submit_result: context
                    .form()
                    .submit(|_submitted| SubmitErrors::<ParsedLifecycleForm, String>::none()),
            });
    });

    let age = use_number(form, ParsedLifecycleForm::fields().age());
    probe.age.borrow_mut().replace(age);

    VNode::empty()
}

fn collection_listener_parse_blocker_probe(
    probe: Rc<CollectionListenerParseBlockerProbe>,
) -> Element {
    let form = use_form_handle(|| FormHandle::new(invoice_collection_form()));
    let listener_probe = Rc::clone(&probe);

    use_form_listener_for_origin(form.clone(), FieldUpdateOrigin::User, move |context| {
        listener_probe
            .snapshots
            .borrow_mut()
            .push(ListenerParseBlockerSnapshot {
                field_name: context.field_name().to_owned(),
                parse_error_count: context.form().parse_errors().len(),
                can_submit: context.form().can_submit(),
                submit_result: context
                    .form()
                    .submit(|_submitted| SubmitErrors::<InvoiceCollectionForm, String>::none()),
            });
    });

    let lines = form.collection(InvoiceCollectionForm::fields().lines());
    let quantity = use_collection_item_number(
        lines.items()[0].clone(),
        InvoiceCollectionLine::fields().quantity(),
    );
    probe.quantity.borrow_mut().replace(quantity);

    VNode::empty()
}

fn parsed_submit_listener_probe(probe: Rc<ParsedSubmitListenerProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ParsedLifecycleForm {
            age: 42,
            token: UploadToken {
                token: "initial".to_owned(),
            },
            check_in: DateYmd {
                year: 2026,
                month: 7,
                day: 2,
            },
        })
    });
    let listener_probe = Rc::clone(&probe);

    use_submit_listener(form.clone(), move |context| {
        listener_probe.events.borrow_mut().push(context.event());
    });

    let age = use_number(form.clone(), ParsedLifecycleForm::fields().age());
    probe.handle.borrow_mut().replace(form);
    probe.age.borrow_mut().replace(age);

    VNode::empty()
}

#[test]
fn derive_keeps_field_identity_independent_from_overridden_field_names() {
    let fields = ProfileForm::fields();
    let email = fields.email();
    let accepts_terms = fields.accepts_terms();
    let model = ProfileForm {
        email: "ada@example.com".to_owned(),
        accepts_terms: true,
    };

    assert_eq!(email.identity().as_str(), "email");
    assert_eq!(email.field_name(), "contact-email");
    assert_eq!(email.get(&model), "ada@example.com");

    assert_eq!(accepts_terms.identity().as_str(), "accepts_terms");
    assert_eq!(accepts_terms.field_name(), "accepted_terms");
    assert!(*accepts_terms.get(&model));
}

#[test]
fn field_listener_can_reset_dependent_field_after_user_update() {
    let probe = Rc::new(FieldListenerProbe::default());
    let mut dom =
        VirtualDom::new_with_props(field_listener_dependent_reset_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert!(handle.field_value(ProfileForm::fields().accepts_terms()));

    handle.set_user_field(ProfileForm::fields().email(), "ada@example.com".to_owned());

    assert_eq!(probe.listener_runs.get(), 1);
    assert!(!handle.field_value(ProfileForm::fields().accepts_terms()));
}

#[test]
fn listener_caused_update_preserves_update_invariants() {
    let probe = Rc::new(ListenerInvariantProbe::default());
    let mut dom =
        VirtualDom::new_with_props(listener_caused_update_invariant_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let accepts_terms_path = ProfileForm::fields().accepts_terms();

    handle.set_user_field(ProfileForm::fields().email(), "ada@example.com".to_owned());
    dom.render_immediate_to_vec();

    assert!(!handle.field_value(accepts_terms_path.clone()));
    assert!(!handle.is_field_touched(accepts_terms_path.clone()));
    assert_eq!(
        handle.field_validation_errors(accepts_terms_path.clone())[0].error(),
        &"terms_required"
    );
    assert_eq!(
        probe.selector_snapshots.borrow().as_slice(),
        [(true, false, 0), (false, false, 1)]
    );
    assert!(probe.observer_events.borrow().iter().any(|event| matches!(
        event,
        FormObserverEvent::FieldUpdated { field, origin: FieldUpdateOrigin::User, .. }
            if field.identity() == ProfileForm::fields().email().identity()
    )));
    assert!(probe.observer_events.borrow().iter().any(|event| matches!(
        event,
        FormObserverEvent::FieldUpdated { field, origin: FieldUpdateOrigin::Programmatic, .. }
            if field.identity() == accepts_terms_path.identity()
    )));
}

#[test]
fn field_listener_origin_filter_prevents_same_field_programmatic_reentry() {
    let probe = Rc::new(FieldListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(
        field_listener_same_field_programmatic_update_probe,
        Rc::clone(&probe),
    );

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle.set_user_field(ProfileForm::fields().email(), "Ada@Example.COM".to_owned());

    assert_eq!(probe.listener_runs.get(), 1);
    assert_eq!(
        handle.field_value(ProfileForm::fields().email()),
        "ada@example.com"
    );
}

#[test]
#[should_panic(expected = "field listener re-entered while it was already running")]
fn field_listener_same_callback_cycle_panics_with_listener_message() {
    let probe = Rc::new(FieldListenerProbe::default());
    let mut dom =
        VirtualDom::new_with_props(field_listener_same_field_cycle_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle.set_user_field(ProfileForm::fields().email(), "Ada@Example.COM".to_owned());
}

#[test]
fn form_listener_identifies_value_replacement_field_without_default_values() {
    let probe = Rc::new(FormListenerProbe::default());
    let mut dom =
        VirtualDom::new_with_props(form_listener_field_identification_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle.set_user_field(ProfileForm::fields().email(), "ada@example.com".to_owned());
    handle.set_field(ProfileForm::fields().accepts_terms(), true);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("email".to_owned(), FieldUpdateOrigin::User),
            ("accepts_terms".to_owned(), FieldUpdateOrigin::Programmatic),
        ]
    );
}

#[test]
fn form_listener_exposes_rendered_field_name_and_event_kind() {
    let probe = Rc::new(FormListenerProbe::default());
    let mut dom =
        VirtualDom::new_with_props(form_listener_field_identification_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle.set_user_field(ProfileForm::fields().email(), "ada@example.com".to_owned());

    assert_eq!(
        probe.rich_events.borrow().as_slice(),
        [(
            "email".to_owned(),
            "contact-email".to_owned(),
            FormListenerEvent::FieldReplaced,
            FieldUpdateOrigin::User,
        )]
    );
}

#[test]
fn field_blur_listener_can_reset_dependent_field_after_blur() {
    let probe = Rc::new(FieldListenerProbe::default());
    let mut dom =
        VirtualDom::new_with_props(field_blur_listener_dependent_reset_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email_path = ProfileForm::fields().email();

    assert!(handle.field_value(ProfileForm::fields().accepts_terms()));

    handle.mark_field_blurred(email_path.clone());

    assert_eq!(probe.listener_runs.get(), 1);
    assert!(handle.is_field_blurred(email_path));
    assert!(!handle.field_value(ProfileForm::fields().accepts_terms()));
}

#[test]
fn form_blur_listener_identifies_blurred_field_without_default_values() {
    let probe = Rc::new(FormListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(
        form_blur_listener_field_identification_probe,
        Rc::clone(&probe),
    );

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email_path = ProfileForm::fields().email();

    handle.mark_field_blurred(email_path);

    assert_eq!(probe.blur_events.borrow().as_slice(), ["email".to_owned()]);
}

#[test]
fn form_blur_listener_exposes_rendered_field_name() {
    let probe = Rc::new(FormListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(
        form_blur_listener_field_identification_probe,
        Rc::clone(&probe),
    );

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle.mark_field_blurred(ProfileForm::fields().email());

    assert_eq!(
        probe.rich_blur_events.borrow().as_slice(),
        [("email".to_owned(), "contact-email".to_owned())]
    );
}

#[test]
fn form_listener_reports_file_selection_changes() {
    let probe = Rc::new(FormListenerProbe::default());
    let mut dom =
        VirtualDom::new_with_props(form_listener_field_identification_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle
        .file(FileFieldKey::new("attachments"))
        .select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

    assert_eq!(
        probe.rich_events.borrow().as_slice(),
        [(
            "attachments".to_owned(),
            "attachments".to_owned(),
            FormListenerEvent::FieldReplaced,
            FieldUpdateOrigin::User,
        )]
    );
}

#[test]
fn form_blur_listener_reports_file_selection_blur() {
    let probe = Rc::new(FormListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(
        form_blur_listener_field_identification_probe,
        Rc::clone(&probe),
    );

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle.file(FileFieldKey::new("attachments")).on_blur();

    assert_eq!(
        probe.rich_blur_events.borrow().as_slice(),
        [("attachments".to_owned(), "attachments".to_owned())]
    );
}

#[test]
fn form_blur_listener_reports_collection_item_field_blur() {
    let probe = Rc::new(CollectionBlurListenerProbe::default());
    let mut dom =
        VirtualDom::new_with_props(collection_item_form_blur_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let description = probe
        .description
        .borrow()
        .as_ref()
        .expect("probe should expose collection item description binding")
        .clone();

    description.on_blur();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [(
            "description".to_owned(),
            "lines[0].description".to_owned(),
            Some("lines".to_owned()),
        )]
    );
}

#[test]
fn form_listener_autosave_reads_snapshot_explicitly() {
    let probe = Rc::new(FormListenerProbe::default());
    let mut dom =
        VirtualDom::new_with_props(form_listener_autosave_snapshot_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle.set_user_field(ProfileForm::fields().email(), "ada@example.com".to_owned());
    handle.set_field(ProfileForm::fields().accepts_terms(), true);

    assert_eq!(
        probe.autosaved_snapshots.borrow().as_slice(),
        [ProfileForm {
            email: "ada@example.com".to_owned(),
            accepts_terms: false,
        }]
    );
}

#[test]
fn multi_select_user_mutations_dispatch_value_replacement_listeners() {
    let probe = Rc::new(MultiSelectListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(multi_select_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let topics = probe
        .topics
        .borrow()
        .as_ref()
        .expect("probe should expose multi-select binding")
        .clone();

    topics.option(Topic::Dioxus).on_change(true);

    assert_eq!(
        probe.field_events.borrow().as_slice(),
        [FieldUpdateOrigin::User]
    );
    assert_eq!(
        probe.form_events.borrow().as_slice(),
        [(
            "topics".to_owned(),
            "topics".to_owned(),
            FormListenerEvent::FieldReplaced,
            FieldUpdateOrigin::User,
        )]
    );
}

#[test]
fn debounced_field_listener_runs_once_for_latest_user_update_after_delay() {
    let probe = Rc::new(DebouncedListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(debounced_field_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle.set_user_field(ProfileForm::fields().email(), "ada@example.com".to_owned());
    handle.set_user_field(
        ProfileForm::fields().email(),
        "grace@example.com".to_owned(),
    );

    assert_eq!(probe.delays.len(), 2);
    assert!(probe.snapshots.borrow().is_empty());

    probe.delays.complete(0);
    dom.render_immediate_to_vec();

    assert!(probe.snapshots.borrow().is_empty());

    probe.delays.complete(1);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        ["grace@example.com".to_owned()]
    );
}

#[test]
fn debounced_form_listener_runs_once_for_latest_user_update_after_delay() {
    let probe = Rc::new(DebouncedFormListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(debounced_form_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle.set_user_field(ProfileForm::fields().email(), "ada@example.com".to_owned());
    handle.set_user_field(
        ProfileForm::fields().email(),
        "grace@example.com".to_owned(),
    );

    assert_eq!(probe.delays.len(), 2);
    assert!(probe.events.borrow().is_empty());

    probe.delays.complete(0);
    dom.render_immediate_to_vec();

    assert!(probe.events.borrow().is_empty());

    probe.delays.complete(1);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("email".to_owned(), "grace@example.com".to_owned())]
    );
}

#[test]
fn debounced_field_listener_does_not_flush_for_submit() {
    let probe = Rc::new(DebouncedListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(debounced_field_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle.set_user_field(ProfileForm::fields().email(), "ada@example.com".to_owned());

    assert_eq!(probe.delays.len(), 1);
    assert_eq!(
        handle.submit(|_submitted| SubmitErrors::<ProfileForm, String>::none()),
        SubmitResult::Succeeded
    );
    assert!(probe.snapshots.borrow().is_empty());

    probe.delays.complete(0);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        ["ada@example.com".to_owned()]
    );
}

#[test]
fn field_binding_listener_reports_hook_mount_and_unmount() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom = VirtualDom::new_with_props(field_binding_lifecycle_probe, Rc::clone(&probe));

    let mut dom = dom;
    dom.rebuild_in_place();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("email".to_owned(), FieldBindingLifecycle::Mounted)]
    );

    drop(dom);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("email".to_owned(), FieldBindingLifecycle::Mounted),
            ("email".to_owned(), FieldBindingLifecycle::Unmounted),
        ]
    );
}

#[test]
fn field_binding_listener_reports_current_mount_when_registered_after_binding_hook() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom = VirtualDom::new_with_props(
        field_binding_listener_after_binding_probe,
        Rc::clone(&probe),
    );

    let mut dom = dom;
    dom.rebuild_in_place();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("email".to_owned(), FieldBindingLifecycle::Mounted)]
    );
}

#[test]
fn field_binding_listener_registered_after_binding_hook_reports_unmount_on_drop() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom = VirtualDom::new_with_props(
        field_binding_listener_after_binding_probe,
        Rc::clone(&probe),
    );

    let mut dom = dom;
    dom.rebuild_in_place();
    drop(dom);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("email".to_owned(), FieldBindingLifecycle::Mounted),
            ("email".to_owned(), FieldBindingLifecycle::Unmounted),
        ]
    );
}

#[test]
fn field_binding_listener_reports_number_hook_mount_and_unmount() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom = VirtualDom::new_with_props(number_binding_lifecycle_probe, Rc::clone(&probe));

    let mut dom = dom;
    dom.rebuild_in_place();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("age".to_owned(), FieldBindingLifecycle::Mounted)]
    );

    drop(dom);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("age".to_owned(), FieldBindingLifecycle::Mounted),
            ("age".to_owned(), FieldBindingLifecycle::Unmounted),
        ]
    );
}

#[test]
fn field_binding_listener_reports_custom_parsed_hook_mount_and_unmount() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom = VirtualDom::new_with_props(custom_parsed_binding_lifecycle_probe, Rc::clone(&probe));

    let mut dom = dom;
    dom.rebuild_in_place();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("token".to_owned(), FieldBindingLifecycle::Mounted)]
    );

    drop(dom);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("token".to_owned(), FieldBindingLifecycle::Mounted),
            ("token".to_owned(), FieldBindingLifecycle::Unmounted),
        ]
    );
}

#[test]
fn field_binding_listener_reports_date_hook_mount_and_unmount() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom = VirtualDom::new_with_props(date_binding_lifecycle_probe, Rc::clone(&probe));

    let mut dom = dom;
    dom.rebuild_in_place();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("check_in".to_owned(), FieldBindingLifecycle::Mounted)]
    );

    drop(dom);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("check_in".to_owned(), FieldBindingLifecycle::Mounted),
            ("check_in".to_owned(), FieldBindingLifecycle::Unmounted),
        ]
    );
}

#[test]
fn field_binding_listener_reports_default_date_hook_mount_and_unmount() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom = VirtualDom::new_with_props(default_date_binding_lifecycle_probe, Rc::clone(&probe));

    let mut dom = dom;
    dom.rebuild_in_place();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("check_in".to_owned(), FieldBindingLifecycle::Mounted)]
    );

    drop(dom);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("check_in".to_owned(), FieldBindingLifecycle::Mounted),
            ("check_in".to_owned(), FieldBindingLifecycle::Unmounted),
        ]
    );
}

#[test]
fn field_binding_listener_reports_custom_number_hook_mount_and_unmount() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom = VirtualDom::new_with_props(custom_number_binding_lifecycle_probe, Rc::clone(&probe));

    let mut dom = dom;
    dom.rebuild_in_place();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("age".to_owned(), FieldBindingLifecycle::Mounted)]
    );

    drop(dom);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("age".to_owned(), FieldBindingLifecycle::Mounted),
            ("age".to_owned(), FieldBindingLifecycle::Unmounted),
        ]
    );
}

#[test]
fn field_binding_listener_reports_select_hook_mount_and_unmount() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom = VirtualDom::new_with_props(select_binding_lifecycle_probe, Rc::clone(&probe));

    let mut dom = dom;
    dom.rebuild_in_place();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("plan".to_owned(), FieldBindingLifecycle::Mounted)]
    );

    drop(dom);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("plan".to_owned(), FieldBindingLifecycle::Mounted),
            ("plan".to_owned(), FieldBindingLifecycle::Unmounted),
        ]
    );
}

#[test]
fn field_binding_listener_reports_radio_hook_mount_and_unmount() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom = VirtualDom::new_with_props(radio_binding_lifecycle_probe, Rc::clone(&probe));

    let mut dom = dom;
    dom.rebuild_in_place();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("plan".to_owned(), FieldBindingLifecycle::Mounted)]
    );

    drop(dom);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("plan".to_owned(), FieldBindingLifecycle::Mounted),
            ("plan".to_owned(), FieldBindingLifecycle::Unmounted),
        ]
    );
}

#[test]
fn field_binding_listener_reports_rendered_select_hook_mount_and_unmount() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom =
        VirtualDom::new_with_props(rendered_select_binding_lifecycle_probe, Rc::clone(&probe));

    let mut dom = dom;
    dom.rebuild_in_place();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("plan".to_owned(), FieldBindingLifecycle::Mounted)]
    );

    drop(dom);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("plan".to_owned(), FieldBindingLifecycle::Mounted),
            ("plan".to_owned(), FieldBindingLifecycle::Unmounted),
        ]
    );
}

#[test]
fn field_binding_listener_reports_multi_select_hook_mount_and_unmount() {
    let probe = Rc::new(BindingListenerProbe::default());
    let dom = VirtualDom::new_with_props(multi_select_binding_lifecycle_probe, Rc::clone(&probe));

    let mut dom = dom;
    dom.rebuild_in_place();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [("topics".to_owned(), FieldBindingLifecycle::Mounted)]
    );

    drop(dom);

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            ("topics".to_owned(), FieldBindingLifecycle::Mounted),
            ("topics".to_owned(), FieldBindingLifecycle::Unmounted),
        ]
    );
}

#[test]
fn submit_listener_reports_successful_sync_submit_lifecycle() {
    let probe = Rc::new(SubmitListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(submit_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(
        handle.submit(|_submitted| SubmitErrors::<ProfileForm, String>::none()),
        SubmitResult::Succeeded
    );

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmissionStarted,
            SubmitListenerEvent::SubmissionSucceeded,
        ]
    );
}

#[test]
fn submit_listener_exposes_typed_submit_intent() {
    let probe = Rc::new(IntentSubmitListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(intent_submit_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(
        handle
            .intent(SignupSubmitIntent::Publish)
            .submit(|_submitted| SubmitErrors::<ProfileForm, String>::none()),
        SubmitResult::Succeeded
    );

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            (
                SubmitListenerEvent::SubmitAttempted,
                Some(SignupSubmitIntent::Publish),
            ),
            (
                SubmitListenerEvent::SubmissionStarted,
                Some(SignupSubmitIntent::Publish),
            ),
            (
                SubmitListenerEvent::SubmissionSucceeded,
                Some(SignupSubmitIntent::Publish),
            ),
        ]
    );
}

#[test]
fn submit_listener_preserves_intent_when_manual_submission_finishes_successfully() {
    let probe = Rc::new(IntentSubmitListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(intent_submit_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert!(
        handle
            .intent(SignupSubmitIntent::SaveDraft)
            .begin_submission()
            .is_started()
    );
    assert!(handle.finish_submission_success());

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            (
                SubmitListenerEvent::SubmitAttempted,
                Some(SignupSubmitIntent::SaveDraft),
            ),
            (
                SubmitListenerEvent::SubmissionStarted,
                Some(SignupSubmitIntent::SaveDraft),
            ),
            (
                SubmitListenerEvent::SubmissionSucceeded,
                Some(SignupSubmitIntent::SaveDraft),
            ),
        ]
    );
}

#[test]
fn submit_listener_reports_blocked_sync_submit_lifecycle() {
    let probe = Rc::new(SubmitListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(submit_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle
        .validator("blocked")
        .on(ValidationTrigger::Submit)
        .check(|_context| vec![FormValidationError::form("blocked".to_owned())]);

    assert_eq!(
        handle.submit(|_submitted| SubmitErrors::<ProfileForm, String>::none()),
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmitBlocked(SubmitBlocker::ValidationErrors),
        ]
    );
}

#[test]
fn dioxus_submission_attempts_and_success_track_lifecycle_and_reset() {
    let handle = FormHandle::new(ProfileForm {
        email: "ada@example.com".to_owned(),
        accepts_terms: true,
    });

    // Nothing submitted yet.
    assert_eq!(handle.submission_attempts(), 0);
    assert!(!handle.is_submit_successful());

    // A successful submit counts once and flips the success flag.
    assert_eq!(
        handle.submit(|_submitted| SubmitErrors::<ProfileForm, String>::none()),
        SubmitResult::Succeeded
    );
    assert_eq!(handle.submission_attempts(), 1);
    assert!(handle.is_submit_successful());

    // A subsequent blocked attempt still increments the counter and clears the success flag.
    handle
        .validator("blocked")
        .on(ValidationTrigger::Submit)
        .check(|_context| vec![FormValidationError::form("blocked".to_owned())]);
    assert_eq!(
        handle.submit(|_submitted| SubmitErrors::<ProfileForm, String>::none()),
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert_eq!(handle.submission_attempts(), 2);
    assert!(!handle.is_submit_successful());

    // Reset clears the attempt counter and the derived success flag.
    handle.reset();
    assert_eq!(handle.submission_attempts(), 0);
    assert!(!handle.is_submit_successful());
}

#[test]
fn dioxus_intent_is_submit_successful_scopes_to_the_latest_intent() {
    let handle = FormHandle::new(SignupForm {
        email: "ada@example.com".to_owned(),
    });
    let submit = handle.managed_submit();

    // Submit succeeds under the SaveDraft intent.
    assert_eq!(
        submit
            .intent(SignupSubmitIntent::SaveDraft)
            .on_submit(managed_submit_event(), |_submitted| ()),
        SubmitResult::Succeeded
    );

    // Per-intent success is scoped to the intent that produced the latest status.
    assert!(
        handle
            .intent(SignupSubmitIntent::SaveDraft)
            .is_submit_successful()
    );
    assert!(
        !handle
            .intent(SignupSubmitIntent::Publish)
            .is_submit_successful()
    );

    // The global readers also report the successful outcome and one attempt.
    assert!(handle.is_submit_successful());
    assert_eq!(handle.submission_attempts(), 1);
}

#[test]
fn dioxus_whole_form_validation_errors_span_fields_form_and_collection_items() {
    let handle: FormHandle<InvoiceCollectionForm, String> =
        FormHandle::new_with_error_type(invoice_collection_form());
    handle.set_error_visibility_policy(dioform::ErrorVisibilityPolicy::Always);

    let lines_path = InvoiceCollectionForm::fields().lines();
    let description_path = InvoiceCollectionLine::fields().description();
    let quantity_path = InvoiceCollectionLine::fields().quantity();
    let lines = handle.collection(lines_path);

    // Two collection-item child validators, each firing only on the first line (Design / qty 2).
    lines
        .item_field_validator(description_path, "description-reserved")
        .on(ValidationTrigger::Manual)
        .check(|value, _context| {
            if value == "Design" {
                vec!["description reserved".to_owned()]
            } else {
                Vec::new()
            }
        });
    lines
        .item_field_validator(quantity_path, "quantity-even")
        .on(ValidationTrigger::Manual)
        .check(|value, _context| {
            if value % 2 == 0 {
                vec!["quantity even".to_owned()]
            } else {
                Vec::new()
            }
        });

    // A form-level validator that emits a form-targeted error.
    handle
        .validator("form-total")
        .on(ValidationTrigger::Manual)
        .check(|_context| vec![FormValidationError::form("form total invalid".to_owned())]);

    handle.validate_all(ValidationTrigger::Manual);

    // The whole-form aggregate carries every stored error across fields + form in one call.
    let all = handle.validation_errors();
    assert_eq!(all.len(), 3);

    // Exactly one form-level error, tagged with its form target and source.
    let form_errors: Vec<_> = all
        .iter()
        .filter(|error| error.target().is_form())
        .collect();
    assert_eq!(form_errors.len(), 1);
    assert_eq!(form_errors[0].error(), "form total invalid");
    assert_eq!(form_errors[0].source().as_str(), "form-total");

    // Two field errors, each a collection-item child (identity carries the item), source preserved.
    let mut field_errors: Vec<_> = all
        .iter()
        .filter_map(|error| {
            error.field_identity().map(|identity| {
                (
                    identity,
                    error.error().clone(),
                    error.source().as_str().to_owned(),
                )
            })
        })
        .collect();
    assert_eq!(field_errors.len(), 2);
    assert!(
        field_errors
            .iter()
            .all(|(identity, _, _)| identity.collection_item_identity().is_some()),
        "field errors should be collection-item child errors"
    );
    field_errors.sort_by(|a, b| a.1.cmp(&b.1));
    assert_eq!(field_errors[0].1, "description reserved");
    assert_eq!(field_errors[0].2, "description-reserved");
    assert_eq!(field_errors[1].1, "quantity even");
    assert_eq!(field_errors[1].2, "quantity-even");

    // The visible variant spans the same aggregate under an Always visibility policy.
    assert_eq!(handle.visible_validation_errors().len(), 3);
}

#[test]
fn dioxus_whole_form_validation_errors_include_submit_errors() {
    let handle: FormHandle<ProfileForm, String> = FormHandle::new_with_error_type(ProfileForm {
        email: "ada@example.com".to_owned(),
        accepts_terms: true,
    });

    // A rejected submit stores a form-targeted submit error under the `submit` source.
    assert_eq!(
        handle.submit(|_submitted| SubmitError::form("server rejected".to_owned())),
        SubmitResult::Rejected
    );

    let all = handle.validation_errors();
    assert_eq!(all.len(), 1);
    assert!(all[0].target().is_form());
    assert_eq!(all[0].error(), "server rejected");
    assert_eq!(all[0].source().as_str(), "submit");
}

#[test]
fn dioxus_reset_field_restores_baseline_and_clears_field_state_selectively() {
    let handle = FormHandle::new(ProfileForm {
        email: String::new(),
        accepts_terms: false,
    });
    let email = ProfileForm::fields().email();
    let accepts_terms = ProfileForm::fields().accepts_terms();

    // A field-scoped validator that rejects one specific email.
    handle
        .field(email.clone())
        .validator("reserved")
        .on(ValidationTrigger::Manual)
        .check(|value, _context| {
            if value == "reserved@example.com" {
                vec!["reserved email".to_owned()]
            } else {
                Vec::new()
            }
        });

    // Dirty and touch both fields; the email field also picks up a validation error.
    handle.set_user_field(email.clone(), "reserved@example.com".to_owned());
    handle.mark_field_blurred(email.clone());
    handle.set_user_field(accepts_terms.clone(), true);
    handle.validate_all(ValidationTrigger::Manual);

    assert!(handle.is_field_dirty(email.clone()));
    assert!(handle.is_field_touched(email.clone()));
    assert!(handle.is_field_blurred(email.clone()));
    assert_eq!(handle.field_validation_errors(email.clone()).len(), 1);

    // Reset only the email field.
    handle.reset_field(email.clone());

    // Value restored to the baseline; interaction and field-scoped validation state cleared.
    assert_eq!(handle.field_value(email.clone()), "");
    assert!(!handle.is_field_dirty(email.clone()));
    assert!(!handle.is_field_touched(email.clone()));
    assert!(!handle.is_field_blurred(email.clone()));
    assert!(handle.field_validation_errors(email.clone()).is_empty());
    assert!(handle.is_default_value(email));

    // The other field is untouched by the single-field reset.
    assert!(handle.field_value(accepts_terms.clone()));
    assert!(handle.is_field_dirty(accepts_terms));
}

#[test]
fn dioxus_reset_field_honors_a_reinitialized_baseline() {
    let handle = FormHandle::new(ProfileForm {
        email: "start@example.com".to_owned(),
        accepts_terms: false,
    });
    let email = ProfileForm::fields().email();

    // Reinitialize moves the baseline; reset_field restores to the new baseline, not the config.
    handle.reinitialize(ProfileForm {
        email: "baseline@example.com".to_owned(),
        accepts_terms: true,
    });
    handle.set_user_field(email.clone(), "edited@example.com".to_owned());
    assert!(handle.is_field_dirty(email.clone()));

    handle.reset_field(email.clone());

    assert_eq!(handle.field_value(email.clone()), "baseline@example.com");
    assert!(!handle.is_field_dirty(email));
}

#[derive(Default)]
struct ResetFieldParseProbe {
    handle: RefCell<Option<FormHandle<ParsedLifecycleForm>>>,
    age: RefCell<Option<ParsedTextBinding<ParsedLifecycleForm, u8>>>,
}

fn reset_field_parse_probe(probe: Rc<ResetFieldParseProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(ParsedLifecycleForm {
            age: 42,
            token: UploadToken {
                token: "initial".to_owned(),
            },
            check_in: DateYmd {
                year: 2026,
                month: 7,
                day: 2,
            },
        })
    });
    let age = use_number(form.clone(), ParsedLifecycleForm::fields().age());
    probe.age.borrow_mut().replace(age);
    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

#[test]
fn dioxus_reset_field_clears_a_mounted_parse_error() {
    let probe = Rc::new(ResetFieldParseProbe::default());
    let mut dom = VirtualDom::new_with_props(reset_field_parse_probe, Rc::clone(&probe));
    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    // Enter unparsable input: a mounted parse error blocks Dioxus-managed submission.
    probe
        .age
        .borrow()
        .as_ref()
        .expect("probe should expose the age binding")
        .on_input("not a number");
    dom.render_immediate_to_vec();

    assert!(probe.age.borrow().as_ref().unwrap().parse_error().is_some());
    assert_eq!(handle.parse_errors().len(), 1);
    assert!(!handle.can_submit());

    // Resetting the field clears the mounted parse error and raw input through the adapter.
    handle.reset_field(ParsedLifecycleForm::fields().age());
    dom.render_immediate_to_vec();

    let age = probe.age.borrow();
    let age = age.as_ref().unwrap();
    assert!(age.parse_error().is_none());
    assert!(handle.parse_errors().is_empty());
    assert!(handle.can_submit());
    // Raw Input State is gone, so the binding shows the baseline typed value again.
    assert_eq!(age.value(), "42");
}

#[test]
fn dioxus_reset_field_emits_a_field_reset_observer_event() {
    let handle = FormHandle::new(ProfileForm {
        email: "start@example.com".to_owned(),
        accepts_terms: false,
    });
    let captured: Rc<RefCell<Vec<FormObserverEvent>>> = Rc::new(RefCell::new(Vec::new()));
    let captured_events = Rc::clone(&captured);
    handle.write_advanced(|core| {
        core.observe(move |event| captured_events.borrow_mut().push(event.clone()));
    });

    handle.reset_field(ProfileForm::fields().email());

    assert!(captured.borrow().iter().any(|event| matches!(
        event,
        FormObserverEvent::FieldReset { field, .. } if field.field_name() == "contact-email"
    )));
}

#[test]
fn submit_listener_reports_validate_for_submit_attempt_and_blocker() {
    let probe = Rc::new(SubmitListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(submit_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert!(handle.validate_for_submit());

    handle
        .validator("blocked")
        .on(ValidationTrigger::Submit)
        .check(|_context| vec![FormValidationError::form("blocked".to_owned())]);

    assert!(!handle.validate_for_submit());

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmitBlocked(SubmitBlocker::ValidationErrors),
        ]
    );
}

#[test]
fn submit_listener_reports_validate_for_submit_parse_blocker() {
    let probe = Rc::new(ParsedSubmitListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(parsed_submit_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let age = probe
        .age
        .borrow()
        .as_ref()
        .expect("probe should expose the parsed number binding")
        .clone();
    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    age.on_input("not-a-number");

    assert!(handle.validate_for_submit());
    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmitBlocked(SubmitBlocker::ParseErrors),
        ]
    );
}

#[test]
fn submit_listener_reports_validate_for_submit_in_flight_blocker() {
    let probe = Rc::new(SubmitListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(submit_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert!(handle.begin_submission().is_started());
    assert!(handle.validate_for_submit());

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmissionStarted,
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmitBlocked(SubmitBlocker::InFlightSubmission),
        ]
    );
}

#[test]
fn submit_listener_reports_rejected_sync_submit_lifecycle() {
    let probe = Rc::new(SubmitListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(submit_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(
        handle.submit(|_submitted| SubmitError::form("rejected".to_owned())),
        SubmitResult::Rejected
    );

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmissionStarted,
            SubmitListenerEvent::SubmissionRejected,
        ]
    );
}

#[test]
fn submit_listener_reports_managed_async_submit_lifecycle() {
    let probe = Rc::new(ManagedSubmitListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(managed_submit_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.submit_result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());
    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmissionStarted,
        ]
    );

    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 1);

    probe.submit.complete(());
    dom.render_immediate_to_vec();

    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Succeeded));
    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmissionStarted,
            SubmitListenerEvent::SubmissionSucceeded,
        ]
    );
}

#[test]
fn submit_listener_reports_plain_in_flight_duplicate_submit() {
    let probe = Rc::new(SubmitListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(submit_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert!(handle.begin_submission().is_started());
    assert_eq!(
        handle.submit(|_submitted| SubmitErrors::<ProfileForm, String>::none()),
        SubmitResult::Blocked(SubmitBlocker::InFlightSubmission)
    );

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmissionStarted,
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmitBlocked(SubmitBlocker::InFlightSubmission),
        ]
    );
}

#[test]
fn submit_listener_reports_managed_in_flight_duplicate_submit() {
    let probe = Rc::new(ManagedSubmitListenerProbe::default());
    let mut dom = VirtualDom::new_with_props(managed_submit_listener_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.submit_result.borrow(), Some(SubmitResult::Started));
    assert_eq!(
        handle
            .managed_submit()
            .on_submit_async(managed_submit_event(), |_submitted| async {}),
        SubmitResult::Blocked(SubmitBlocker::InFlightSubmission)
    );

    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmissionStarted,
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmitBlocked(SubmitBlocker::InFlightSubmission),
        ]
    );
}

#[test]
fn form_core_replaces_a_text_field_value() {
    let email = SignupForm::fields().email();
    let mut form = FormCore::new(SignupForm {
        email: String::new(),
    });

    form.set_field(email.clone(), "ada@example.com".to_owned());

    assert_eq!(form.field_value(email), "ada@example.com");
    assert_eq!(
        form.snapshot(),
        SignupForm {
            email: "ada@example.com".to_owned()
        }
    );
}

#[test]
fn dioxus_facade_text_binding_updates_the_typed_field() {
    let handle = FormHandle::new(SignupForm {
        email: String::new(),
    });
    let email_path = SignupForm::fields().email();
    let email = handle.text(email_path.clone());

    assert_eq!(email.name(), "email");
    assert_eq!(email.value(), "");
    assert!(!handle.is_field_touched(email_path.clone()));
    assert!(!handle.is_field_blurred(email_path.clone()));

    email.on_input("ada@example.com");

    assert_eq!(email.value(), "ada@example.com");
    assert!(handle.is_field_touched(email_path.clone()));
    assert!(!handle.is_field_blurred(email_path.clone()));
    assert_eq!(
        handle.field_value(SignupForm::fields().email()),
        "ada@example.com"
    );

    email.on_blur();

    assert!(handle.is_field_touched(email_path.clone()));
    assert!(handle.is_field_blurred(email_path));
}

#[test]
fn dioxus_collection_binding_updates_reorders_and_names_item_fields() {
    let handle = FormHandle::new(invoice_collection_form());
    let lines_path = InvoiceCollectionForm::fields().lines();
    let description_path = InvoiceCollectionLine::fields().description();
    let lines = handle.collection(lines_path);
    let items = lines.items();
    let second = items[1].clone();
    let description = second.text(description_path.clone());

    assert_eq!(items[0].index(), 0);
    assert_eq!(items[1].index(), 1);
    assert_ne!(items[0].identity(), items[1].identity());
    assert_eq!(description.name(), "lines[1].description");
    assert_eq!(description.value(), "Build");

    description.on_input("Build v2");

    assert_eq!(handle.snapshot().lines[1].description, "Build v2");
    assert!(lines.move_to_index(second.identity(), 0));

    let reordered = lines.items();
    let moved = reordered[0].text(description_path);

    assert_eq!(reordered[0].identity(), second.identity());
    assert_eq!(moved.name(), "lines[0].description");
    assert_eq!(moved.value(), "Build v2");
    assert_eq!(handle.snapshot().lines[0].description, "Build v2");
}

#[test]
fn dioxus_collection_binding_composes_nested_collection_and_child_field_paths() {
    let handle = FormHandle::new(nested_invoice_collection_form());
    let lines_path = NestedInvoiceCollectionForm::fields()
        .invoice()
        .join(NestedInvoice::fields().lines());
    let product_name_path = NestedInvoiceLine::fields()
        .product()
        .join(NestedProduct::fields().name());
    let lines = handle.collection(lines_path.clone());
    let item = lines.items()[0].clone();
    let product_name = item.text(product_name_path.clone());

    assert_eq!(lines_path.identity().as_str(), "invoice.lines");
    assert_eq!(lines_path.field_name(), "invoice.invoice_lines");
    assert_eq!(product_name_path.identity().as_str(), "product.name");
    assert_eq!(product_name_path.field_name(), "product.product-name");
    assert_eq!(
        product_name.name(),
        "invoice.invoice_lines[0].product.product-name",
    );
    assert_eq!(product_name.value(), "Keyboard");

    product_name.on_input("Mouse");

    assert_eq!(handle.snapshot().invoice.lines[0].product.name, "Mouse");
}

#[test]
fn dioform_handle_state_snapshot_restores_collection_item_identities() {
    let handle = FormHandle::new(invoice_collection_form());
    let lines_path = InvoiceCollectionForm::fields().lines();
    let lines = handle.collection(lines_path.clone());
    let initial_items = lines.items();
    let first = initial_items[0].identity();
    let second = initial_items[1].identity();
    let inserted = lines
        .insert(
            1,
            InvoiceCollectionLine {
                description: "Review".to_owned(),
                quantity: 3,
            },
        )
        .expect("insert index should be valid");

    assert!(inserted > second);
    assert!(lines.remove(first).is_some());
    assert!(lines.move_to_index(second, 0));

    let snapshot = handle.state_snapshot();
    let restored = FormHandle::new(InvoiceCollectionForm { lines: Vec::new() });

    restored
        .restore_state_snapshot(snapshot)
        .expect("form state snapshot should restore through the Dioxus handle");

    let restored_lines = restored.collection(lines_path);
    let restored_items = restored_lines.items();

    assert_eq!(restored_items[0].identity(), second);
    assert_eq!(restored_items[1].identity(), inserted);
    assert_eq!(
        restored_items[0]
            .text(InvoiceCollectionLine::fields().description())
            .value(),
        "Build"
    );
    assert_eq!(
        restored_items[1]
            .text(InvoiceCollectionLine::fields().description())
            .value(),
        "Review"
    );
    let next = restored_lines.append(InvoiceCollectionLine {
        description: "Ship".to_owned(),
        quantity: 1,
    });
    assert!(next > inserted);
}

#[cfg(feature = "serde")]
#[test]
fn dioform_state_snapshot_serializes_deserializes_and_restores_core_state() {
    let source: FormHandle<InvoiceCollectionForm, String> =
        FormHandle::new_with_error_type(invoice_collection_form());
    source.set_validation_mode(ValidationMode::on_change());
    source.set_error_visibility_policy(dioform::ErrorVisibilityPolicy::Always);
    let lines_path = InvoiceCollectionForm::fields().lines();
    let description_path = InvoiceCollectionLine::fields().description();
    let lines = source.collection(lines_path.clone());
    let initial_items = lines.items();
    let first = initial_items[0].identity();
    let second = initial_items[1].identity();

    lines
        .item_field_validator(description_path.clone(), "required")
        .on(ValidationTrigger::Manual)
        .check(|value, _context| {
            if value.trim().is_empty() {
                vec!["required".to_owned()]
            } else {
                Vec::new()
            }
        });

    let inserted = lines
        .insert(
            1,
            InvoiceCollectionLine {
                description: "Review".to_owned(),
                quantity: 3,
            },
        )
        .expect("insert index should be valid");
    assert!(lines.remove(first).is_some());
    assert!(lines.move_to_index(second, 0));

    let inserted_description = lines.items()[1].text(description_path.clone());
    inserted_description.on_input("");
    inserted_description.on_blur();
    source.validate_all(ValidationTrigger::Manual);

    assert_eq!(source.submit_attempt_count(), 0);
    assert_eq!(
        source
            .managed_submit()
            .on_submit(managed_submit_event(), |_submitted| ()),
        SubmitResult::Succeeded
    );
    assert_eq!(source.submit_attempt_count(), 1);
    assert_eq!(
        inserted_description.validation_errors()[0].error(),
        "required"
    );

    let serialized = serde_json::to_string(&source.state_snapshot())
        .expect("form state snapshot should serialize");
    let serialized_value: serde_json::Value =
        serde_json::from_str(&serialized).expect("serialized snapshot should be JSON");
    let empty_states = Vec::new();
    let validator_states = serialized_value["collection_item_validator_states"]
        .as_array()
        .unwrap_or(&empty_states);

    assert!(!validator_states.is_empty());
    for validator_state in validator_states {
        let state = &validator_state["state"];

        assert!(state.get("source").is_none());
        assert!(state.get("triggers").is_none());
        assert!(state.get("kind").is_none());
        assert!(state.get("async_run").is_none());
        assert!(state.get("pending_run").is_none());
    }

    let snapshot: dioform::advanced::FormStateSnapshot<InvoiceCollectionForm, String> =
        serde_json::from_str(&serialized).expect("form state snapshot should deserialize");
    let restored: FormHandle<InvoiceCollectionForm, String> =
        FormHandle::new_with_error_type(InvoiceCollectionForm { lines: Vec::new() });
    let restored_lines = restored.collection(lines_path.clone());
    restored_lines
        .item_field_validator(description_path.clone(), "required")
        .on(ValidationTrigger::Manual)
        .check(|value, _context| {
            if value.trim().is_empty() {
                vec!["required".to_owned()]
            } else {
                Vec::new()
            }
        });

    restored
        .restore_state_snapshot(snapshot)
        .expect("deserialized form state snapshot should restore");

    let restored_lines = restored.collection(lines_path);
    let restored_items = restored_lines.items();
    let restored_description = restored_items[1].text(description_path);

    assert_eq!(restored_items[0].identity(), second);
    assert_eq!(restored_items[1].identity(), inserted);
    assert_eq!(restored.snapshot().lines[0].description, "Build");
    assert_eq!(restored.snapshot().lines[1].description, "");
    assert!(restored_description.is_touched());
    assert!(restored_description.is_blurred());
    assert_eq!(
        restored_description.validation_errors()[0].error(),
        "required"
    );
    assert_eq!(restored.submit_attempt_count(), 1);
    assert_eq!(restored.validation_mode(), ValidationMode::on_change());
    assert_eq!(
        restored.error_visibility_policy(),
        dioform::ErrorVisibilityPolicy::Always
    );
}

#[test]
fn dioxus_collection_parsed_binding_blocks_submit_until_recovered() {
    let handle = FormHandle::new(invoice_collection_form());
    let lines = handle.collection(InvoiceCollectionForm::fields().lines());
    let quantity = lines.items()[0].number(InvoiceCollectionLine::fields().quantity());

    assert_eq!(quantity.name(), "lines[0].quantity");
    assert_eq!(quantity.value(), "2");

    quantity.on_input("not-a-number");

    assert!(quantity.parse_error().is_some());
    assert!(
        handle
            .submit_availability()
            .contains(SubmitBlocker::ParseErrors)
    );

    quantity.on_input("4");

    assert!(quantity.parse_error().is_none());
    assert_eq!(handle.snapshot().lines[0].quantity, 4);
    assert!(
        !handle
            .submit_availability()
            .contains(SubmitBlocker::ParseErrors)
    );
}

#[test]
fn parsed_input_listener_sees_cleared_parse_blocker_after_successful_parse() {
    let probe = Rc::new(ParsedListenerParseBlockerProbe::default());
    let mut dom =
        VirtualDom::new_with_props(parsed_listener_parse_blocker_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let age = probe
        .age
        .borrow()
        .as_ref()
        .expect("probe should expose the parsed number binding")
        .clone();

    age.on_input("not-a-number");
    age.on_input("7");

    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [ListenerParseBlockerSnapshot {
            field_name: "age".to_owned(),
            parse_error_count: 0,
            can_submit: true,
            submit_result: SubmitResult::Succeeded,
        }]
    );
}

#[test]
fn collection_parsed_input_listener_sees_cleared_parse_blocker_after_successful_parse() {
    let probe = Rc::new(CollectionListenerParseBlockerProbe::default());
    let mut dom =
        VirtualDom::new_with_props(collection_listener_parse_blocker_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let quantity = probe
        .quantity
        .borrow()
        .as_ref()
        .expect("probe should expose the collection parsed number binding")
        .clone();

    quantity.on_input("not-a-number");
    quantity.on_input("4");

    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [ListenerParseBlockerSnapshot {
            field_name: "lines[0].quantity".to_owned(),
            parse_error_count: 0,
            can_submit: true,
            submit_result: SubmitResult::Succeeded,
        }]
    );
}

#[test]
fn dioxus_collection_parsed_binding_blur_with_parse_error_does_not_validate_stale_typed_value() {
    let validation_runs = Rc::new(Cell::new(0));
    let validation_runs_for_validator = Rc::clone(&validation_runs);
    let seen_values = Rc::new(RefCell::new(Vec::new()));
    let seen_values_for_validator = Rc::clone(&seen_values);
    let handle: FormHandle<InvoiceCollectionForm, &'static str> =
        FormHandle::new_with_error_type(invoice_collection_form());
    let lines_path = InvoiceCollectionForm::fields().lines();
    let quantity_path = InvoiceCollectionLine::fields().quantity();
    let lines = handle.collection(lines_path.clone());
    let item = lines.items()[0].clone();
    let quantity = item.number(quantity_path.clone());

    lines
        .item_field_validator(quantity_path.clone(), "minimum_quantity_on_blur")
        .on(ValidationTrigger::Blur)
        .check(move |value, context| {
            validation_runs_for_validator.set(validation_runs_for_validator.get() + 1);
            seen_values_for_validator.borrow_mut().push(*value);
            assert_eq!(context.trigger(), ValidationTrigger::Blur);

            if *value < 3 {
                vec!["quantity_too_low"]
            } else {
                Vec::new()
            }
        });

    quantity.on_input("not-a-number");
    quantity.on_blur();

    assert!(quantity.is_touched());
    assert!(quantity.is_blurred());
    assert_eq!(handle.snapshot().lines[0].quantity, 2);
    assert!(quantity.parse_error().is_some());
    assert_eq!(validation_runs.get(), 0);
    assert!(seen_values.borrow().is_empty());
    assert!(quantity.validation_errors().is_empty());

    quantity.on_input("1");
    quantity.on_blur();

    assert!(quantity.parse_error().is_none());
    assert_eq!(handle.snapshot().lines[0].quantity, 1);
    assert_eq!(validation_runs.get(), 1);
    assert_eq!(seen_values.borrow().as_slice(), &[1]);
    assert_eq!(quantity.validation_errors()[0].error(), &"quantity_too_low");
}

#[derive(Debug, Eq, PartialEq)]
struct CollectionParsedHookSnapshot {
    name: String,
    rendered_value: String,
    parse_error_count: usize,
    form_parse_error_count: usize,
    can_submit: bool,
    draft_quantities: Vec<u32>,
}

#[derive(Default)]
struct CollectionParsedHookProbe {
    handle: RefCell<Option<FormHandle<InvoiceCollectionForm>>>,
    quantity: RefCell<
        Option<CollectionParsedTextBinding<InvoiceCollectionForm, InvoiceCollectionLine, u32>>,
    >,
    tracked_item: RefCell<Option<CollectionItemIdentity>>,
    snapshots: RefCell<Vec<CollectionParsedHookSnapshot>>,
}

fn collection_item_parsed_hook_probe(probe: Rc<CollectionParsedHookProbe>) -> Element {
    let form = use_form_handle(|| FormHandle::new(invoice_collection_form()));
    let lines = form.collection(InvoiceCollectionForm::fields().lines());
    let items = lines.items();
    let tracked_item = {
        let mut tracked_item = probe.tracked_item.borrow_mut();

        match *tracked_item {
            Some(item) => item,
            None => {
                let item = items[1].identity();
                tracked_item.replace(item);
                item
            }
        }
    };
    let item = items
        .into_iter()
        .find(|item| item.identity() == tracked_item)
        .expect("tracked collection item should still be mounted");
    let quantity = use_collection_item_number(item, InvoiceCollectionLine::fields().quantity());
    let snapshot = form.snapshot();

    probe.handle.borrow_mut().replace(form.clone());
    probe.quantity.borrow_mut().replace(quantity.clone());
    probe
        .snapshots
        .borrow_mut()
        .push(CollectionParsedHookSnapshot {
            name: quantity.name(),
            rendered_value: quantity.value(),
            parse_error_count: usize::from(quantity.parse_error().is_some()),
            form_parse_error_count: form.parse_errors().len(),
            can_submit: form.can_submit(),
            draft_quantities: snapshot.lines.iter().map(|line| line.quantity).collect(),
        });

    VNode::empty()
}

#[test]
fn collection_item_number_hook_preserves_parse_state_and_updates_name_after_reorder() {
    let probe = Rc::new(CollectionParsedHookProbe::default());
    let mut dom = VirtualDom::new_with_props(collection_item_parsed_hook_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [CollectionParsedHookSnapshot {
            name: "lines[1].quantity".to_owned(),
            rendered_value: "1".to_owned(),
            parse_error_count: 0,
            form_parse_error_count: 0,
            can_submit: true,
            draft_quantities: vec![2, 1],
        }]
    );

    let quantity = probe
        .quantity
        .borrow()
        .as_ref()
        .expect("probe should expose the collection item parsed binding")
        .clone();

    quantity.on_input("not-a-number");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&CollectionParsedHookSnapshot {
            name: "lines[1].quantity".to_owned(),
            rendered_value: "not-a-number".to_owned(),
            parse_error_count: 1,
            form_parse_error_count: 1,
            can_submit: false,
            draft_quantities: vec![2, 1],
        })
    );

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose the form handle")
        .clone();
    let tracked_item = probe
        .tracked_item
        .borrow()
        .expect("probe should track a collection item identity");

    assert!(
        handle
            .collection(InvoiceCollectionForm::fields().lines())
            .move_to_index(tracked_item, 0)
    );
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&CollectionParsedHookSnapshot {
            name: "lines[0].quantity".to_owned(),
            rendered_value: "not-a-number".to_owned(),
            parse_error_count: 1,
            form_parse_error_count: 1,
            can_submit: false,
            draft_quantities: vec![1, 2],
        })
    );

    let quantity = probe
        .quantity
        .borrow()
        .as_ref()
        .expect("probe should expose the moved collection item parsed binding")
        .clone();

    quantity.on_input("5");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&CollectionParsedHookSnapshot {
            name: "lines[0].quantity".to_owned(),
            rendered_value: "5".to_owned(),
            parse_error_count: 0,
            form_parse_error_count: 0,
            can_submit: true,
            draft_quantities: vec![5, 2],
        })
    );
}

#[test]
fn removing_collection_item_clears_parse_state_and_submit_blocker() {
    let handle = FormHandle::new(invoice_collection_form());
    let lines = handle.collection(InvoiceCollectionForm::fields().lines());
    let item = lines.items()[1].clone();
    let quantity = item.number(InvoiceCollectionLine::fields().quantity());

    quantity.on_input("not-a-number");

    assert_eq!(quantity.value(), "not-a-number");
    assert_eq!(handle.parse_errors().len(), 1);
    assert_eq!(
        handle.submit_availability().blockers(),
        &[SubmitBlocker::ParseErrors]
    );

    let removed = lines
        .remove(item.identity())
        .expect("collection item should be removable");

    assert_eq!(removed.description, "Build");
    assert!(quantity.parse_error().is_none());
    assert!(handle.parse_errors().is_empty());
    assert!(handle.can_submit());

    let called = Cell::new(false);

    assert_eq!(
        handle
            .managed_submit()
            .on_submit(managed_submit_event(), |_submitted| called.set(true)),
        SubmitResult::Succeeded
    );
    assert!(called.get());
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Succeeded));
}

#[test]
fn dioxus_collection_item_validator_templates_cover_inserted_and_reordered_items() {
    let handle: FormHandle<InvoiceCollectionForm, &'static str> =
        FormHandle::new_with_error_type(invoice_collection_form());
    let lines = handle.collection(InvoiceCollectionForm::fields().lines());
    let description_path = InvoiceCollectionLine::fields().description();

    lines
        .item_field_validator(description_path.clone(), "required")
        .check(|value, _context| {
            if value.trim().is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        });

    let inserted = lines.append(InvoiceCollectionLine {
        description: String::new(),
        quantity: 1,
    });

    handle.validate_all(ValidationTrigger::Manual);

    let inserted_item = lines
        .items()
        .into_iter()
        .find(|item| item.identity() == inserted)
        .expect("inserted item should be present");
    let description = inserted_item.text(description_path.clone());

    assert_eq!(description.name(), "lines[2].description");
    assert_eq!(description.validation_errors()[0].error(), &"required");
    assert!(description.visible_validation_errors().is_empty());

    description.on_blur();

    assert_eq!(
        description.visible_validation_errors()[0].error(),
        &"required"
    );
    assert!(lines.move_to_index(inserted, 0));

    let moved_item = lines
        .items()
        .into_iter()
        .find(|item| item.identity() == inserted)
        .expect("moved item should be present");
    let moved_description = moved_item.text(description_path);

    assert_eq!(moved_description.name(), "lines[0].description");
    assert_eq!(
        moved_description.validation_errors()[0].error(),
        &"required"
    );
    assert_eq!(
        moved_description.visible_validation_errors()[0].error(),
        &"required"
    );
}

#[test]
fn collection_item_addressing_tracks_one_logical_item_across_public_surfaces() {
    let observer_events = Rc::new(RefCell::new(Vec::new()));
    let captured_events = Rc::clone(&observer_events);
    let handle: FormHandle<InvoiceCollectionForm, &'static str> =
        FormHandle::new_with_error_type(invoice_collection_form())
            .with_id_namespace("collection-addressing")
            .with_validation_mode(ValidationMode::on_change());
    let lines_path = InvoiceCollectionForm::fields().lines();
    let description_path = InvoiceCollectionLine::fields().description();
    let quantity_path = InvoiceCollectionLine::fields().quantity();
    let lines = handle.collection(lines_path.clone());

    handle.write_advanced(|core| {
        core.observe(move |event| captured_events.borrow_mut().push(event.clone()));
    });
    lines
        .item_field_validator(description_path.clone(), "description_required")
        .check(|value, _context| {
            if value.trim().is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        });

    let tracked_item = lines.items()[1].clone();
    let tracked_identity = tracked_item.identity();
    let description = tracked_item.text(description_path.clone());
    let quantity = tracked_item.number(quantity_path.clone());
    let initial_description_input_id = description.accessibility().input_id().to_owned();
    let initial_quantity_input_id = quantity.accessibility().input_id().to_owned();

    description.on_input("");
    description.on_blur();
    quantity.on_input("not-a-number");

    let description_identity = description.validation_errors()[0].expect_field();
    let quantity_identity = quantity
        .parse_error()
        .expect("quantity should have a parse blocker")
        .field_identity();

    assert_eq!(description_identity.collection_path(), Some("lines"));
    assert_eq!(
        description_identity.collection_item_identity(),
        Some(tracked_identity)
    );
    assert_eq!(quantity_identity.collection_path(), Some("lines"));
    assert_eq!(
        quantity_identity.collection_item_identity(),
        Some(tracked_identity)
    );
    assert!(description.accessibility().aria_invalid());
    assert!(quantity.accessibility().aria_invalid());
    assert!(
        handle
            .submit_availability()
            .contains(SubmitBlocker::ParseErrors)
    );

    let inserted = lines
        .insert(
            0,
            InvoiceCollectionLine {
                description: "Plan".to_owned(),
                quantity: 1,
            },
        )
        .expect("insert index should be valid");
    assert_ne!(inserted, tracked_identity);
    assert!(lines.move_to_index(tracked_identity, 0));

    let moved_item = lines
        .items()
        .into_iter()
        .find(|item| item.identity() == tracked_identity)
        .expect("tracked item should still be present");
    let moved_description = moved_item.text(description_path);
    let moved_quantity = moved_item.number(quantity_path);

    assert_eq!(moved_item.index(), 0);
    assert_eq!(moved_description.name(), "lines[0].description");
    assert_eq!(moved_quantity.name(), "lines[0].quantity");
    assert_eq!(moved_description.value(), "");
    assert_eq!(
        moved_description.validation_errors()[0].expect_field(),
        description_identity
    );
    assert_eq!(
        moved_description.visible_validation_errors()[0].error(),
        &"required"
    );
    assert_eq!(
        moved_description.accessibility().input_id(),
        initial_description_input_id.as_str()
    );
    assert_eq!(
        moved_quantity.accessibility().input_id(),
        initial_quantity_input_id.as_str()
    );
    assert!(moved_quantity.accessibility().has_parse_errors());
    assert_eq!(handle.parse_errors()[0].field_identity(), quantity_identity);

    moved_description.on_input("Build recovered");

    assert!(observer_events.borrow().iter().any(|event| matches!(
        event,
        FormObserverEvent::CollectionItemInserted { item, index: 0, origin: FieldUpdateOrigin::User, .. }
            if *item == inserted
    )));
    assert!(observer_events.borrow().iter().any(|event| matches!(
        event,
        FormObserverEvent::CollectionItemMoved { item, from: 2, to: 0, origin: FieldUpdateOrigin::User, .. }
            if *item == tracked_identity
    )));
    assert!(observer_events.borrow().iter().any(|event| matches!(
        event,
        FormObserverEvent::FieldUpdated { field, origin: FieldUpdateOrigin::User, .. }
            if field.identity() == description_identity && field.field_name() == "lines[0].description"
    )));

    let removed = lines
        .remove(tracked_identity)
        .expect("tracked item should be removable");

    assert_eq!(removed.description, "Build recovered");
    assert!(moved_description.validation_errors().is_empty());
    assert!(!moved_description.metadata().is_touched());
    assert!(moved_quantity.parse_error().is_none());
    assert!(handle.parse_errors().is_empty());
    assert!(
        !handle
            .submit_availability()
            .contains(SubmitBlocker::ParseErrors)
    );
    assert!(observer_events.borrow().iter().any(|event| matches!(
        event,
        FormObserverEvent::CollectionItemRemoved { item, index: 0, origin: FieldUpdateOrigin::User, .. }
            if *item == tracked_identity
    )));
}

#[derive(Debug, Eq, PartialEq)]
struct ReactiveSnapshot {
    email_value: String,
    validation_error_count: usize,
    visible_error_count: usize,
    can_submit: bool,
    dirty: bool,
}

#[derive(Default)]
struct ReactiveProbe {
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    snapshots: RefCell<Vec<ReactiveSnapshot>>,
}

fn reactive_signup_probe(probe: Rc<ReactiveProbe>) -> Element {
    let form = use_form_handle(|| {
        let form: FormHandle<SignupForm, &'static str> =
            FormHandle::new_with_error_type(SignupForm {
                email: String::new(),
            });
        let email = SignupForm::fields().email();

        form.write_advanced(|core| {
            core.register_sync_field_validator(email, "required", |value, _context| {
                if value.is_empty() {
                    vec!["required"]
                } else {
                    Vec::new()
                }
            });
        });

        form
    });
    let email_path = SignupForm::fields().email();
    let email = form.text(email_path.clone());
    let email_value = email.value();
    let validation_error_count = form.field_validation_errors(email_path.clone()).len();
    let visible_error_count = form.visible_field_validation_errors(email_path).len();
    let can_submit = form.can_submit();
    let dirty = form.is_dirty();

    probe.handle.borrow_mut().replace(form);
    probe.snapshots.borrow_mut().push(ReactiveSnapshot {
        email_value: email_value.clone(),
        validation_error_count,
        visible_error_count,
        can_submit,
        dirty,
    });

    VNode::empty()
}

#[derive(Debug, Eq, PartialEq)]
struct HookInitializationSnapshot {
    email_value: String,
    email_blurred: bool,
}

#[derive(Default)]
struct HookInitializationProbe {
    initial_email: RefCell<String>,
    create_runs: Cell<usize>,
    handle: RefCell<Option<FormHandle<SignupForm>>>,
    snapshots: RefCell<Vec<HookInitializationSnapshot>>,
}

fn hook_initialization_probe(probe: Rc<HookInitializationProbe>) -> Element {
    let initial_email = probe.initial_email.borrow().clone();
    let create_probe = Rc::clone(&probe);
    let form = use_form_handle(move || {
        create_probe
            .create_runs
            .set(create_probe.create_runs.get() + 1);

        FormHandle::new(SignupForm {
            email: initial_email,
        })
    });
    let email_path = SignupForm::fields().email();

    probe.handle.borrow_mut().replace(form.clone());
    probe
        .snapshots
        .borrow_mut()
        .push(HookInitializationSnapshot {
            email_value: form.field_value(email_path.clone()),
            email_blurred: form.is_field_blurred(email_path),
        });

    VNode::empty()
}

struct SignupScope;
struct ShippingSignupScope;
struct BillingSignupScope;

#[derive(Default)]
struct FormContextProbe {
    consumed_email: RefCell<Option<String>>,
    missing_context_available: Cell<bool>,
    same_model_scoped_emails: RefCell<Option<(String, String)>>,
    nested_provider_email: RefCell<Option<String>>,
    required_context_email: RefCell<Option<String>>,
}

#[derive(Clone, Props)]
struct FormContextProbeProps {
    probe: Rc<FormContextProbe>,
}

impl PartialEq for FormContextProbeProps {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.probe, &other.probe)
    }
}

fn scoped_form_context_provider(props: FormContextProbeProps) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(SignupForm {
            email: "ada@example.com".to_owned(),
        })
    });

    provide_form_context::<SignupScope, _, _>(form);

    rsx!(scoped_form_context_consumer {
        probe: Rc::clone(&props.probe)
    })
}

fn scoped_form_context_consumer(props: FormContextProbeProps) -> Element {
    let form = try_use_form_context::<SignupScope, SignupForm, String>()
        .expect("scoped form context should be provided");

    props
        .probe
        .consumed_email
        .borrow_mut()
        .replace(form.field_value(SignupForm::fields().email()));

    VNode::empty()
}

fn missing_form_context_consumer(props: FormContextProbeProps) -> Element {
    props
        .probe
        .missing_context_available
        .set(try_use_form_context::<SignupScope, SignupForm, String>().is_some());

    VNode::empty()
}

fn same_model_context_provider(props: FormContextProbeProps) -> Element {
    let shipping = use_form_handle(|| {
        FormHandle::new(SignupForm {
            email: "shipping@example.com".to_owned(),
        })
    });
    let billing = use_form_handle(|| {
        FormHandle::new(SignupForm {
            email: "billing@example.com".to_owned(),
        })
    });

    provide_form_context::<ShippingSignupScope, _, _>(shipping);
    provide_form_context::<BillingSignupScope, _, _>(billing);

    rsx!(same_model_context_consumer {
        probe: Rc::clone(&props.probe)
    })
}

fn same_model_context_consumer(props: FormContextProbeProps) -> Element {
    let shipping = try_use_form_context::<ShippingSignupScope, SignupForm, String>()
        .expect("shipping form context should be provided");
    let billing = try_use_form_context::<BillingSignupScope, SignupForm, String>()
        .expect("billing form context should be provided");
    let email = SignupForm::fields().email();

    props.probe.same_model_scoped_emails.borrow_mut().replace((
        shipping.field_value(email.clone()),
        billing.field_value(email),
    ));

    VNode::empty()
}

fn outer_scoped_context_provider(props: FormContextProbeProps) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(SignupForm {
            email: "outer@example.com".to_owned(),
        })
    });

    provide_form_context::<SignupScope, _, _>(form);

    rsx!(inner_scoped_context_provider {
        probe: Rc::clone(&props.probe)
    })
}

fn inner_scoped_context_provider(props: FormContextProbeProps) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(SignupForm {
            email: "inner@example.com".to_owned(),
        })
    });

    provide_form_context::<SignupScope, _, _>(form);

    rsx!(nested_scoped_context_consumer {
        probe: Rc::clone(&props.probe)
    })
}

fn nested_scoped_context_consumer(props: FormContextProbeProps) -> Element {
    let form = try_use_form_context::<SignupScope, SignupForm, String>()
        .expect("nested form context should be provided");

    props
        .probe
        .nested_provider_email
        .borrow_mut()
        .replace(form.field_value(SignupForm::fields().email()));

    VNode::empty()
}

fn required_form_context_provider(props: FormContextProbeProps) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(SignupForm {
            email: "required@example.com".to_owned(),
        })
    });

    provide_form_context::<SignupScope, _, _>(form);

    rsx!(required_form_context_consumer {
        probe: Rc::clone(&props.probe)
    })
}

fn required_form_context_consumer(props: FormContextProbeProps) -> Element {
    let form = use_form_context::<SignupScope, SignupForm, String>();

    props
        .probe
        .required_context_email
        .borrow_mut()
        .replace(form.field_value(SignupForm::fields().email()));

    VNode::empty()
}

#[test]
fn dioform_handle_updates_dependent_ui_reactively() {
    let probe = Rc::new(ReactiveProbe::default());
    let mut dom = VirtualDom::new_with_props(reactive_signup_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [ReactiveSnapshot {
            email_value: String::new(),
            validation_error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            dirty: false,
        }]
    );

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email_path = SignupForm::fields().email();

    handle.validate_field(email_path.clone(), ValidationTrigger::Manual);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&ReactiveSnapshot {
            email_value: String::new(),
            validation_error_count: 1,
            visible_error_count: 0,
            can_submit: false,
            dirty: false,
        })
    );

    handle.mark_field_blurred(email_path.clone());
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&ReactiveSnapshot {
            email_value: String::new(),
            validation_error_count: 1,
            visible_error_count: 1,
            can_submit: false,
            dirty: false,
        })
    );

    handle.text(email_path.clone()).on_input("ada@example.com");
    handle.validate_field(email_path, ValidationTrigger::Manual);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&ReactiveSnapshot {
            email_value: "ada@example.com".to_owned(),
            validation_error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            dirty: true,
        })
    );

    handle.reset();
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&ReactiveSnapshot {
            email_value: String::new(),
            validation_error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            dirty: false,
        })
    );
}

#[test]
fn scoped_form_context_consumer_reads_provided_form_handle() {
    let probe = Rc::new(FormContextProbe::default());
    let mut dom = VirtualDom::new_with_props(
        scoped_form_context_provider,
        FormContextProbeProps {
            probe: Rc::clone(&probe),
        },
    );

    dom.rebuild_in_place();

    assert_eq!(
        probe.consumed_email.borrow().as_deref(),
        Some("ada@example.com")
    );
}

#[test]
fn missing_scoped_form_context_returns_none() {
    let probe = Rc::new(FormContextProbe::default());
    let mut dom = VirtualDom::new_with_props(
        missing_form_context_consumer,
        FormContextProbeProps {
            probe: Rc::clone(&probe),
        },
    );

    dom.rebuild_in_place();

    assert!(!probe.missing_context_available.get());
}

#[test]
fn same_model_forms_with_distinct_context_scopes_are_unambiguous() {
    let probe = Rc::new(FormContextProbe::default());
    let mut dom = VirtualDom::new_with_props(
        same_model_context_provider,
        FormContextProbeProps {
            probe: Rc::clone(&probe),
        },
    );

    dom.rebuild_in_place();

    assert_eq!(
        probe.same_model_scoped_emails.borrow().as_ref(),
        Some(&(
            "shipping@example.com".to_owned(),
            "billing@example.com".to_owned()
        ))
    );
}

#[test]
fn nested_scoped_form_context_uses_nearest_provider() {
    let probe = Rc::new(FormContextProbe::default());
    let mut dom = VirtualDom::new_with_props(
        outer_scoped_context_provider,
        FormContextProbeProps {
            probe: Rc::clone(&probe),
        },
    );

    dom.rebuild_in_place();

    assert_eq!(
        probe.nested_provider_email.borrow().as_deref(),
        Some("inner@example.com")
    );
}

#[test]
fn required_scoped_form_context_reads_provided_form_handle() {
    let probe = Rc::new(FormContextProbe::default());
    let mut dom = VirtualDom::new_with_props(
        required_form_context_provider,
        FormContextProbeProps {
            probe: Rc::clone(&probe),
        },
    );

    dom.rebuild_in_place();

    assert_eq!(
        probe.required_context_email.borrow().as_deref(),
        Some("required@example.com")
    );
}

#[test]
fn use_form_handle_initializes_once_and_does_not_synchronize_parent_values() {
    let probe = Rc::new(HookInitializationProbe {
        initial_email: RefCell::new("first@example.com".to_owned()),
        ..HookInitializationProbe::default()
    });
    let mut dom = VirtualDom::new_with_props(hook_initialization_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert_eq!(probe.create_runs.get(), 1);
    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [HookInitializationSnapshot {
            email_value: "first@example.com".to_owned(),
            email_blurred: false,
        }]
    );

    *probe.initial_email.borrow_mut() = "parent-update@example.com".to_owned();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email_path = SignupForm::fields().email();

    handle.mark_field_blurred(email_path);
    dom.render_immediate_to_vec();

    assert_eq!(probe.create_runs.get(), 1);
    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&HookInitializationSnapshot {
            email_value: "first@example.com".to_owned(),
            email_blurred: true,
        })
    );

    handle.reinitialize(SignupForm {
        email: "explicit@example.com".to_owned(),
    });
    dom.render_immediate_to_vec();

    assert_eq!(probe.create_runs.get(), 1);
    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&HookInitializationSnapshot {
            email_value: "explicit@example.com".to_owned(),
            email_blurred: false,
        })
    );
}

#[test]
fn dioform_handle_runs_initialization_validation_only_when_requested() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: String::new(),
        });
    let email = SignupForm::fields().email();

    handle.write_advanced(|core| {
        core.register_sync_field_validator_for_triggers(
            email.clone(),
            "initial_required",
            ValidationTrigger::Initial,
            move |value, context| {
                validator_runs.set(validator_runs.get() + 1);
                assert_eq!(context.trigger(), ValidationTrigger::Initial);

                if value.is_empty() {
                    vec!["required"]
                } else {
                    Vec::new()
                }
            },
        );
    });

    assert_eq!(runs.get(), 0);
    assert!(handle.validation_errors().is_empty());
    assert!(handle.can_submit());

    assert!(!handle.validate_initialization());

    assert_eq!(runs.get(), 1);
    assert_eq!(
        handle.read_core(|core| core.validation_status(email.clone(), "initial_required")),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = handle
        .field_validation_errors(email.clone())
        .into_iter()
        .map(|error| (error.source().as_str().to_owned(), *error.error()))
        .collect();
    assert_eq!(errors, vec![("initial_required".to_owned(), "required")]);
    assert!(
        handle
            .visible_field_validation_errors(email.clone())
            .is_empty()
    );

    handle.mark_field_blurred(email.clone());

    assert_eq!(
        handle.visible_field_validation_errors(email)[0].error(),
        &"required"
    );
}

#[test]
fn form_config_registers_sync_field_validator_without_initial_validation() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let email = SignupForm::fields().email();
    let handle: FormHandle<SignupForm, &'static str> = FormHandle::from_config(
        FormConfig::new(SignupForm {
            email: String::new(),
        })
        .field_validator(email.clone(), "required")
        .on(ValidationTrigger::Manual)
        .check_optional(move |value, context| {
            validator_runs.set(validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Manual);

            value.is_empty().then_some("required")
        }),
    );

    assert_eq!(runs.get(), 0);
    assert_eq!(
        handle.validation_status(email.clone(), "required"),
        Some(ValidationStatus::Unknown)
    );
    assert!(handle.validation_errors().is_empty());
    assert!(handle.can_submit());

    handle.validate_field(email.clone(), ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        handle.validation_status(email.clone(), "required"),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        handle.field_validation_errors(email)[0].error(),
        &"required"
    );
}

#[test]
fn form_config_registers_form_validators_before_later_direct_registration() {
    let fields = ProfileForm::fields();
    let email_path = fields.email();
    let terms_path = fields.accepts_terms();
    let terms_for_validator = terms_path.clone();
    let handle: FormHandle<ProfileForm, &'static str> = FormHandle::from_config(
        FormConfig::new(ProfileForm {
            email: String::new(),
            accepts_terms: false,
        })
        .field_validator(email_path.clone(), "email")
        .on(ValidationTrigger::Manual)
        .check_optional(|value, _context| value.is_empty().then_some("email_required"))
        .form_validator("terms")
        .on(ValidationTrigger::Manual)
        .check_optional(move |context| {
            (!context.form().accepts_terms).then_some(FormValidationError::field(
                terms_for_validator.clone(),
                "terms_required",
            ))
        }),
    );

    let direct = handle
        .validator("direct")
        .on(ValidationTrigger::Manual)
        .check(|_context| vec![FormValidationError::form("direct")]);

    handle.validate_all(ValidationTrigger::Manual);

    let statuses: Vec<_> = handle
        .validation_statuses()
        .into_iter()
        .map(|status| {
            (
                status.validator_id(),
                status.source().as_str().to_owned(),
                status.status(),
            )
        })
        .collect();
    assert_eq!(statuses.len(), 3);
    assert_eq!(statuses[0].1, "email");
    assert_eq!(statuses[1].1, "terms");
    assert_eq!(statuses[2].1, "direct");
    assert!(statuses[0].0 < statuses[1].0);
    assert!(statuses[1].0 < statuses[2].0);
    assert_eq!(statuses[2].0, direct);
    assert!(
        statuses
            .iter()
            .all(|(_, _, status)| *status == ValidationStatus::Invalid)
    );

    let errors: Vec<_> = handle
        .validation_errors()
        .into_iter()
        .map(|error| (error.target(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                ValidationTarget::Field(email_path.identity()),
                "email_required"
            ),
            (
                ValidationTarget::Field(terms_path.identity()),
                "terms_required"
            ),
            (ValidationTarget::Form, "direct"),
        ]
    );
}

#[test]
fn form_config_validators_survive_reset_and_reinitialization() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let email = SignupForm::fields().email();
    let handle: FormHandle<SignupForm, &'static str> = FormHandle::from_config(
        FormConfig::new(SignupForm {
            email: String::new(),
        })
        .field_validator(email.clone(), "required")
        .on(ValidationTrigger::Manual)
        .check_optional(move |value, _context| {
            validator_runs.set(validator_runs.get() + 1);

            value.is_empty().then_some("required")
        }),
    );

    handle.validate_field(email.clone(), ValidationTrigger::Manual);

    assert_eq!(runs.get(), 1);
    assert_eq!(
        handle.validation_status(email.clone(), "required"),
        Some(ValidationStatus::Invalid)
    );

    handle.set_user_field(email.clone(), "ada@example.com".to_owned());
    handle.reset();

    assert_eq!(handle.field_value(email.clone()), "");
    assert!(handle.validation_errors().is_empty());
    assert_eq!(
        handle.validation_status(email.clone(), "required"),
        Some(ValidationStatus::Unknown)
    );

    handle.validate_field(email.clone(), ValidationTrigger::Manual);

    assert_eq!(runs.get(), 2);
    assert_eq!(
        handle.field_validation_errors(email.clone())[0].error(),
        &"required"
    );

    handle.reinitialize(SignupForm {
        email: "grace@example.com".to_owned(),
    });

    assert_eq!(handle.field_value(email.clone()), "grace@example.com");
    assert!(handle.validation_errors().is_empty());
    assert_eq!(
        handle.validation_status(email.clone(), "required"),
        Some(ValidationStatus::Unknown)
    );

    handle.validate_field(email.clone(), ValidationTrigger::Manual);

    assert_eq!(runs.get(), 3);
    assert_eq!(
        handle.validation_status(email, "required"),
        Some(ValidationStatus::Valid)
    );
    assert!(handle.validation_errors().is_empty());
}

#[test]
fn form_config_applies_validation_and_error_visibility_policies() {
    let email = SignupForm::fields().email();
    let handle: FormHandle<SignupForm, &'static str> = FormHandle::from_config(
        FormConfig::new(SignupForm {
            email: "ada@example.com".to_owned(),
        })
        .validation_mode(ValidationMode::on_change())
        .error_visibility_policy(dioform::ErrorVisibilityPolicy::Always)
        .field_validator(email.clone(), "required")
        .on(ValidationTrigger::Change)
        .check_optional(|value, _context| value.is_empty().then_some("required")),
    );

    handle.text(email.clone()).on_input("");

    assert_eq!(
        handle.validation_status(email.clone(), "required"),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        handle.field_validation_errors(email.clone())[0].error(),
        &"required"
    );
    assert_eq!(
        handle.visible_field_validation_errors(email)[0].error(),
        &"required"
    );
}

#[test]
fn dioxus_submit_then_revalidate_mode_runs_change_validation_after_submit_attempt() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let email = SignupForm::fields().email();
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        })
        .with_validation_mode(ValidationMode::submit_then_revalidate());

    handle
        .field(email.clone())
        .validator("required")
        .on(ValidationTrigger::Change)
        .check(move |value, context| {
            validator_runs.set(validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);

            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        });

    let email_binding = handle.text(email.clone());
    email_binding.on_input(String::new());

    assert_eq!(runs.get(), 0);
    assert_eq!(
        handle.validation_status(email.clone(), "required"),
        Some(ValidationStatus::Unknown)
    );
    assert!(handle.field_validation_errors(email.clone()).is_empty());

    assert_eq!(
        handle
            .managed_submit()
            .on_submit(managed_submit_event(), |_submitted| ()),
        SubmitResult::Succeeded
    );
    email_binding.on_input("grace@example.com");

    assert_eq!(runs.get(), 1);
    assert_eq!(
        handle.validation_status(email, "required"),
        Some(ValidationStatus::Valid)
    );
}

#[test]
fn dioxus_submit_then_revalidate_mode_runs_blur_validation_after_submit_attempt() {
    let runs = Rc::new(Cell::new(0));
    let validator_runs = Rc::clone(&runs);
    let email = SignupForm::fields().email();
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: String::new(),
        })
        .with_validation_mode(ValidationMode::submit_then_revalidate());

    handle
        .field(email.clone())
        .validator("required")
        .on(ValidationTrigger::Blur)
        .check(move |value, context| {
            validator_runs.set(validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Blur);

            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        });

    let email_binding = handle.text(email.clone());
    email_binding.on_blur();

    assert_eq!(runs.get(), 0);
    assert_eq!(
        handle.validation_status(email.clone(), "required"),
        Some(ValidationStatus::Unknown)
    );
    assert!(handle.field_validation_errors(email.clone()).is_empty());

    assert_eq!(
        handle
            .managed_submit()
            .on_submit(managed_submit_event(), |_submitted| ()),
        SubmitResult::Succeeded
    );
    email_binding.on_blur();

    assert_eq!(runs.get(), 1);
    assert_eq!(
        handle.field_validation_errors(email)[0].error(),
        &"required"
    );
}

#[test]
fn form_config_registers_collection_item_field_validator_templates() {
    let lines_path = InvoiceCollectionForm::fields().lines();
    let quantity_path = InvoiceCollectionLine::fields().quantity();
    let handle: FormHandle<InvoiceCollectionForm, &'static str> = FormHandle::from_config(
        FormConfig::new(invoice_collection_form())
            .collection_item_field_validator(lines_path.clone(), quantity_path.clone(), "quantity")
            .on(ValidationTrigger::Manual)
            .check_optional(|value, _context| (*value == 0).then_some("quantity_required")),
    );
    let lines = handle.collection(lines_path.clone());
    let inserted = lines.append(InvoiceCollectionLine {
        description: "Review".to_owned(),
        quantity: 0,
    });

    handle.validate_all(ValidationTrigger::Manual);

    let inserted_item = lines
        .items()
        .into_iter()
        .find(|item| item.identity() == inserted)
        .expect("inserted item should be present");
    let quantity = inserted_item.number(quantity_path);

    assert_eq!(quantity.name(), "lines[2].quantity");
    assert_eq!(
        quantity.validation_errors()[0].error(),
        &"quantity_required"
    );

    assert!(lines.move_to_index(inserted, 0));

    let moved_item = lines
        .items()
        .into_iter()
        .find(|item| item.identity() == inserted)
        .expect("moved item should be present");
    let moved_quantity = moved_item.number(InvoiceCollectionLine::fields().quantity());

    assert_eq!(moved_quantity.name(), "lines[0].quantity");
    assert_eq!(
        moved_quantity.validation_errors()[0].error(),
        &"quantity_required"
    );
}

#[test]
fn form_config_registers_collection_item_value_validator_templates() {
    let topics_path = MultiSelectForm::fields().topics();
    let handle: FormHandle<MultiSelectForm, &'static str> = FormHandle::from_config(
        FormConfig::new(MultiSelectForm {
            topics: vec![Topic::Rust],
        })
        .collection_item_validator(topics_path.clone(), "allowed_topic")
        .on(ValidationTrigger::Manual)
        .check_optional(|value, _context| (*value == Topic::Dioxus).then_some("topic_unavailable")),
    );
    let topics = handle.multi_select(topics_path);

    topics.option(Topic::Dioxus).on_change(true);
    handle.validate_all(ValidationTrigger::Manual);

    let dioxus = topics
        .selected_item(&Topic::Dioxus)
        .expect("selected value should expose its logical item");

    assert_eq!(dioxus.name(), "topics[1]");
    assert_eq!(dioxus.validation_errors()[0].error(), &"topic_unavailable");

    topics.option(Topic::Dioxus).on_change(false);

    assert!(dioxus.validation_errors().is_empty());
    assert!(handle.validation_errors().is_empty());
}

#[derive(Default)]
struct AsyncInitializationProbe {
    validation: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validate_initialization: RefCell<Option<Box<ActionHandler>>>,
    validation_calls: Cell<u32>,
}

fn async_initialization_probe(probe: Rc<AsyncInitializationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: String::new(),
                });
            let email = SignupForm::fields().email();
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.field(email.clone())
                .async_validator("initial_async")
                .on(ValidationTrigger::Initial)
                .check(move |value, snapshot| {
                    captured_probe
                        .validation_calls
                        .set(captured_probe.validation_calls.get() + 1);
                    assert_eq!(value, "");
                    assert_eq!(snapshot.value().email, "");

                    let validation = validation.clone();

                    async move { validation.future().await }
                });

            form
        }
    });

    let runtime = dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let validate_initialization = {
        let runtime = Rc::clone(&runtime);
        let form = form.clone();

        move || {
            runtime.in_scope(scope, || {
                assert!(form.validate_initialization());
            });
        }
    };

    probe.handle.borrow_mut().replace(form.clone());
    probe
        .validate_initialization
        .borrow_mut()
        .replace(Box::new(validate_initialization));

    VNode::empty()
}

#[test]
fn dioxus_async_initialization_validation_runs_only_when_requested() {
    let probe = Rc::new(AsyncInitializationProbe::default());
    let mut dom = VirtualDom::new_with_props(async_initialization_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(probe.validation_calls.get(), 0);
    assert!(handle.validation_errors().is_empty());
    assert_eq!(
        handle.validation_status(email.clone(), "initial_async"),
        Some(ValidationStatus::Unknown)
    );
    assert!(handle.can_submit());

    {
        let validate_initialization = probe.validate_initialization.borrow();
        let validate_initialization = validate_initialization
            .as_ref()
            .expect("probe should expose validation action");
        validate_initialization();
    }

    assert_eq!(probe.validation_calls.get(), 1);
    assert_eq!(
        handle.validation_status(email.clone(), "initial_async"),
        Some(ValidationStatus::Pending)
    );

    probe.validation.complete(vec!["initial_async_error"]);
    dom.render_immediate_to_vec();

    assert_eq!(
        handle.validation_status(email.clone(), "initial_async"),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        handle.field_validation_errors(email)[0].error(),
        &"initial_async_error"
    );
}

#[test]
fn dioxus_is_validating_reflects_pending_async_validation() {
    let probe = Rc::new(AsyncInitializationProbe::default());
    let mut dom = VirtualDom::new_with_props(async_initialization_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    // No validation has run yet, so nothing is pending.
    assert!(!handle.is_validating());
    assert!(!handle.is_field_validating(email.clone()));

    {
        let validate_initialization = probe.validate_initialization.borrow();
        let validate_initialization = validate_initialization
            .as_ref()
            .expect("probe should expose validation action");
        validate_initialization();
    }

    // The async validator is in flight: both the whole-form and per-field reads report pending.
    assert!(handle.is_validating());
    assert!(handle.is_field_validating(email.clone()));

    probe.validation.complete(vec!["initial_async_error"]);
    dom.render_immediate_to_vec();

    // Validation resolved to a terminal status, so nothing is pending anymore.
    assert!(!handle.is_validating());
    assert!(!handle.is_field_validating(email));
}

#[derive(Default)]
struct ConfigAsyncInitializationProbe {
    validation: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validate_initialization: RefCell<Option<Box<ActionHandler>>>,
    validation_calls: Cell<u32>,
}

fn config_async_initialization_probe(probe: Rc<ConfigAsyncInitializationProbe>) -> Element {
    let validation = probe.validation.clone();
    let captured_probe = Rc::clone(&probe);
    let form = use_form_config(
        FormConfig::new(SignupForm {
            email: String::new(),
        })
        .async_field_validator(SignupForm::fields().email(), "initial_async")
        .on(ValidationTrigger::Initial)
        .check(move |value, snapshot| {
            captured_probe
                .validation_calls
                .set(captured_probe.validation_calls.get() + 1);
            assert_eq!(value, "");
            assert_eq!(snapshot.value().email, "");

            validation.future()
        }),
    );

    let runtime = dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let validate_initialization = {
        let runtime = Rc::clone(&runtime);
        let form = form.clone();

        move || {
            runtime.in_scope(scope, || {
                assert!(form.validate_initialization());
            });
        }
    };

    probe.handle.borrow_mut().replace(form.clone());
    probe
        .validate_initialization
        .borrow_mut()
        .replace(Box::new(validate_initialization));

    VNode::empty()
}

#[test]
fn use_form_config_registers_async_field_validator_without_initial_validation() {
    let probe = Rc::new(ConfigAsyncInitializationProbe::default());
    let mut dom = VirtualDom::new_with_props(config_async_initialization_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(probe.validation_calls.get(), 0);
    assert!(handle.validation_errors().is_empty());
    assert_eq!(
        handle.validation_status(email.clone(), "initial_async"),
        Some(ValidationStatus::Unknown)
    );
    assert!(handle.can_submit());

    {
        let validate_initialization = probe.validate_initialization.borrow();
        let validate_initialization = validate_initialization
            .as_ref()
            .expect("probe should expose validation action");
        validate_initialization();
    }

    assert_eq!(probe.validation_calls.get(), 1);
    assert_eq!(
        handle.validation_status(email.clone(), "initial_async"),
        Some(ValidationStatus::Pending)
    );

    probe.validation.complete(vec!["initial_async_error"]);
    dom.render_immediate_to_vec();

    assert_eq!(
        handle.validation_status(email.clone(), "initial_async"),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        handle.field_validation_errors(email)[0].error(),
        &"initial_async_error"
    );
}

#[derive(Default)]
struct ConfigAsyncFormInitializationProbe {
    validation: AsyncGate<Vec<FormValidationError<&'static str>>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validate_initialization: RefCell<Option<Box<ActionHandler>>>,
    validation_calls: Cell<u32>,
}

fn config_async_form_initialization_probe(
    probe: Rc<ConfigAsyncFormInitializationProbe>,
) -> Element {
    let validation = probe.validation.clone();
    let captured_probe = Rc::clone(&probe);
    let form = use_form_config(
        FormConfig::new(SignupForm {
            email: String::new(),
        })
        .async_form_validator("initial_form")
        .on(ValidationTrigger::Initial)
        .check(move |snapshot| {
            captured_probe
                .validation_calls
                .set(captured_probe.validation_calls.get() + 1);
            assert_eq!(snapshot.value().email, "");

            validation.future()
        }),
    );

    let runtime = dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let validate_initialization = {
        let runtime = Rc::clone(&runtime);
        let form = form.clone();

        move || {
            runtime.in_scope(scope, || {
                assert!(form.validate_initialization());
            });
        }
    };

    probe.handle.borrow_mut().replace(form.clone());
    probe
        .validate_initialization
        .borrow_mut()
        .replace(Box::new(validate_initialization));

    VNode::empty()
}

#[test]
fn use_form_config_registers_async_form_validator_without_initial_validation() {
    let probe = Rc::new(ConfigAsyncFormInitializationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(config_async_form_initialization_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(probe.validation_calls.get(), 0);
    assert!(handle.validation_errors().is_empty());
    assert_eq!(
        handle.form_validation_status("initial_form"),
        Some(ValidationStatus::Unknown)
    );
    assert!(handle.can_submit());

    {
        let validate_initialization = probe.validate_initialization.borrow();
        let validate_initialization = validate_initialization
            .as_ref()
            .expect("probe should expose validation action");
        validate_initialization();
    }

    assert_eq!(probe.validation_calls.get(), 1);
    assert_eq!(
        handle.form_validation_status("initial_form"),
        Some(ValidationStatus::Pending)
    );

    probe
        .validation
        .complete(vec![FormValidationError::form("form_async_error")]);
    dom.render_immediate_to_vec();

    assert_eq!(
        handle.form_validation_status("initial_form"),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        handle.form_validation_errors()[0].error(),
        &"form_async_error"
    );
}

#[test]
fn dioform_handle_preserves_duplicate_validator_registrations() {
    let first_runs = Rc::new(Cell::new(0));
    let second_runs = Rc::new(Cell::new(0));
    let first_validator_runs = Rc::clone(&first_runs);
    let second_validator_runs = Rc::clone(&second_runs);
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: String::new(),
        });
    let email = SignupForm::fields().email();

    let first = handle
        .field(email.clone())
        .validator("email")
        .check(move |value, context| {
            first_validator_runs.set(first_validator_runs.get() + 1);
            assert_eq!(context.source().as_str(), "email");

            if value.is_empty() {
                vec!["field_first"]
            } else {
                Vec::new()
            }
        });
    let second = handle
        .field(email.clone())
        .validator("email")
        .check(move |_value, context| {
            second_validator_runs.set(second_validator_runs.get() + 1);
            assert_eq!(context.source().as_str(), "email");
            vec!["field_second"]
        });
    let form_first_email = email.clone();
    let form_first = handle.validator("email").check(move |context| {
        assert_eq!(context.source().as_str(), "email");

        if context.form().email.is_empty() {
            vec![FormValidationError::field(
                form_first_email.clone(),
                "form_first",
            )]
        } else {
            Vec::new()
        }
    });
    let form_second = handle.validator("email").check(|context| {
        assert_eq!(context.source().as_str(), "email");
        vec![FormValidationError::form("form_second")]
    });

    assert_ne!(first, second);
    assert_ne!(form_first, form_second);
    assert!(first < second);
    assert!(second < form_first);
    assert!(form_first < form_second);
    assert_eq!(first_runs.get(), 0);
    assert_eq!(second_runs.get(), 0);
    assert!(handle.validation_errors().is_empty());

    let initial_statuses: Vec<_> = handle
        .validation_statuses()
        .into_iter()
        .map(|status| {
            (
                status.validator_id(),
                status.source().as_str().to_owned(),
                status.status(),
            )
        })
        .collect();
    assert_eq!(
        initial_statuses,
        vec![
            (first, "email".to_owned(), ValidationStatus::Unknown),
            (second, "email".to_owned(), ValidationStatus::Unknown),
            (form_first, "email".to_owned(), ValidationStatus::Unknown),
            (form_second, "email".to_owned(), ValidationStatus::Unknown),
        ]
    );

    handle.validate_all(ValidationTrigger::Manual);

    assert_eq!(first_runs.get(), 1);
    assert_eq!(second_runs.get(), 1);
    assert_eq!(
        handle.field_validation_status(email.clone(), first),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        handle.field_validation_status(email.clone(), second),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        handle.form_validation_status_by_id(form_first),
        Some(ValidationStatus::Invalid)
    );
    assert_eq!(
        handle.form_validation_status_by_id(form_second),
        Some(ValidationStatus::Invalid)
    );

    let errors: Vec<_> = handle
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.target(),
                error.source().as_str().to_owned(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                Some(first),
                ValidationTarget::Field(email.identity()),
                "email".to_owned(),
                "field_first",
            ),
            (
                Some(second),
                ValidationTarget::Field(email.identity()),
                "email".to_owned(),
                "field_second",
            ),
            (
                Some(form_first),
                ValidationTarget::Field(email.identity()),
                "email".to_owned(),
                "form_first",
            ),
            (
                Some(form_second),
                ValidationTarget::Form,
                "email".to_owned(),
                "form_second",
            ),
        ]
    );

    handle.set_user_field(email.clone(), "ada@example.com".to_owned());

    assert_eq!(
        handle.validate_field_validator(email.clone(), first, ValidationTrigger::Manual),
        Some(ValidationStatus::Valid)
    );
    assert_eq!(
        handle.validate_form_validator(form_first, ValidationTrigger::Manual),
        Some(ValidationStatus::Valid)
    );

    let remaining_errors: Vec<_> = handle
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.validator_id(),
                error.target(),
                error.source().as_str().to_owned(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        remaining_errors,
        vec![
            (
                Some(second),
                ValidationTarget::Field(email.identity()),
                "email".to_owned(),
                "field_second",
            ),
            (
                Some(form_second),
                ValidationTarget::Form,
                "email".to_owned(),
                "form_second",
            ),
        ]
    );
    assert_eq!(
        handle.validation_status(email, "email"),
        Some(ValidationStatus::Valid)
    );
    assert_eq!(
        handle.form_validation_status("email"),
        Some(ValidationStatus::Valid)
    );

    let final_statuses: Vec<_> = handle
        .validation_statuses()
        .into_iter()
        .map(|status| (status.validator_id(), status.status()))
        .collect();
    assert_eq!(
        final_statuses,
        vec![
            (first, ValidationStatus::Valid),
            (second, ValidationStatus::Invalid),
            (form_first, ValidationStatus::Valid),
            (form_second, ValidationStatus::Invalid),
        ]
    );
}

#[test]
fn dioform_handle_reactivity_is_scoped_per_handle() {
    let first_probe = Rc::new(ReactiveProbe::default());
    let second_probe = Rc::new(ReactiveProbe::default());
    let mut first_dom = VirtualDom::new_with_props(reactive_signup_probe, Rc::clone(&first_probe));
    let mut second_dom =
        VirtualDom::new_with_props(reactive_signup_probe, Rc::clone(&second_probe));

    first_dom.rebuild_in_place();
    second_dom.rebuild_in_place();

    let first_handle = first_probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    first_handle
        .text(SignupForm::fields().email())
        .on_input("ada@example.com");
    first_dom.render_immediate_to_vec();
    second_dom.render_immediate_to_vec();

    assert_eq!(first_probe.snapshots.borrow().len(), 2);
    assert_eq!(second_probe.snapshots.borrow().len(), 1);
    assert_eq!(
        first_probe.snapshots.borrow().last(),
        Some(&ReactiveSnapshot {
            email_value: "ada@example.com".to_owned(),
            validation_error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            dirty: true,
        })
    );
    assert_eq!(
        second_probe.snapshots.borrow().last(),
        Some(&ReactiveSnapshot {
            email_value: String::new(),
            validation_error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            dirty: false,
        })
    );
}

struct FieldSelectorProbe {
    form: FormHandle<ProfileForm>,
    email_values: RefCell<Vec<String>>,
    terms_values: RefCell<Vec<bool>>,
}

impl FieldSelectorProbe {
    fn new() -> Self {
        Self {
            form: FormHandle::new(ProfileForm {
                email: String::new(),
                accepts_terms: false,
            }),
            email_values: RefCell::new(Vec::new()),
            terms_values: RefCell::new(Vec::new()),
        }
    }
}

fn email_value_selector_probe(probe: Rc<FieldSelectorProbe>) -> Element {
    let value = probe.form.field_value(ProfileForm::fields().email());

    probe.email_values.borrow_mut().push(value);

    VNode::empty()
}

fn terms_value_selector_probe(probe: Rc<FieldSelectorProbe>) -> Element {
    let value = probe
        .form
        .field_value(ProfileForm::fields().accepts_terms());

    probe.terms_values.borrow_mut().push(value);

    VNode::empty()
}

struct FileSelectionSelectorProbe {
    form: FormHandle<SignupForm>,
    selected_names: RefCell<Vec<Vec<String>>>,
}

impl FileSelectionSelectorProbe {
    fn new() -> Self {
        Self {
            form: FormHandle::new(SignupForm {
                email: String::new(),
            }),
            selected_names: RefCell::new(Vec::new()),
        }
    }
}

fn file_selection_selector_probe(probe: Rc<FileSelectionSelectorProbe>) -> Element {
    let names = probe
        .form
        .file(FileFieldKey::new("attachments"))
        .selected_files()
        .into_iter()
        .map(|file| file.name().to_owned())
        .collect();

    probe.selected_names.borrow_mut().push(names);

    VNode::empty()
}

#[test]
fn field_value_selectors_do_not_rerender_unrelated_field_readers() {
    let probe = Rc::new(FieldSelectorProbe::new());
    let mut email_dom = VirtualDom::new_with_props(email_value_selector_probe, Rc::clone(&probe));
    let mut terms_dom = VirtualDom::new_with_props(terms_value_selector_probe, Rc::clone(&probe));

    email_dom.rebuild_in_place();
    terms_dom.rebuild_in_place();

    assert_eq!(probe.email_values.borrow().as_slice(), [String::new()]);
    assert_eq!(probe.terms_values.borrow().as_slice(), [false]);

    probe
        .form
        .set_user_field(ProfileForm::fields().email(), "ada@example.com".to_owned());
    email_dom.render_immediate_to_vec();
    terms_dom.render_immediate_to_vec();

    assert_eq!(
        probe.email_values.borrow().as_slice(),
        [String::new(), "ada@example.com".to_owned()]
    );
    assert_eq!(probe.terms_values.borrow().as_slice(), [false]);

    probe
        .form
        .set_user_field(ProfileForm::fields().accepts_terms(), true);
    email_dom.render_immediate_to_vec();
    terms_dom.render_immediate_to_vec();

    assert_eq!(probe.email_values.borrow().len(), 2);
    assert_eq!(probe.terms_values.borrow().as_slice(), [false, true]);
}

#[test]
fn file_selection_selector_rerenders_when_selected_files_change() {
    let probe = Rc::new(FileSelectionSelectorProbe::new());
    let mut dom = VirtualDom::new_with_props(file_selection_selector_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert_eq!(
        probe.selected_names.borrow().as_slice(),
        [Vec::<String>::new()]
    );

    probe
        .form
        .file(FileFieldKey::new("attachments"))
        .select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.selected_names.borrow().as_slice(),
        [Vec::<String>::new(), vec!["resume.pdf".to_owned()]]
    );
}

struct ValidationNotifyProbe {
    form: FormHandle<ProfileForm>,
    form_error_reads: RefCell<usize>,
}

impl ValidationNotifyProbe {
    fn new() -> Self {
        Self {
            // Submit-only mode: neither a value change nor a blur runs validation until the
            // form has been submitted, so both mutations exercise the "no validation ran" path.
            form: FormHandle::from_config(
                FormConfig::new(ProfileForm {
                    email: String::new(),
                    accepts_terms: false,
                })
                .validation_mode(ValidationMode::on_submit()),
            ),
            form_error_reads: RefCell::new(0),
        }
    }
}

fn validation_notify_probe(probe: Rc<ValidationNotifyProbe>) -> Element {
    // Subscribes to the form-level validation-error selector. That selector is woken only by a
    // validation-changed notification, never by a field's own value or metadata transition, so a
    // re-render here means the mutation notified validation subscribers.
    let _ = probe.form.form_validation_errors();

    *probe.form_error_reads.borrow_mut() += 1;

    VNode::empty()
}

/// Pins issue #129: a value write and a blur that both run no validation must still notify
/// validation subscribers. Value writes clear submit errors and blurs flip blur-scoped error
/// visibility, so either mutation can change what a validation subscriber should see even when no
/// validator runs. The two paths converge on one rule (every field mutation notifies) rather than
/// the pre-#129 divergence where blur stayed silent when it ran no validation.
#[test]
fn field_mutations_notify_validation_subscribers_even_without_running_validation() {
    let probe = Rc::new(ValidationNotifyProbe::new());
    let mut dom = VirtualDom::new_with_props(validation_notify_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    assert_eq!(*probe.form_error_reads.borrow(), 1);

    // Value write that runs no validation: still notifies validation subscribers.
    probe
        .form
        .set_user_field(ProfileForm::fields().email(), "ada@example.com".to_owned());
    dom.render_immediate_to_vec();
    assert_eq!(*probe.form_error_reads.borrow(), 2);

    // Blur that runs no validation: also notifies validation subscribers.
    probe.form.mark_field_blurred(ProfileForm::fields().email());
    dom.render_immediate_to_vec();
    assert_eq!(*probe.form_error_reads.borrow(), 3);
}

struct CollectionSelectorProbe {
    form: FormHandle<InvoiceCollectionForm>,
    first_description: CollectionTextBinding<InvoiceCollectionForm, InvoiceCollectionLine>,
    second_description: CollectionTextBinding<InvoiceCollectionForm, InvoiceCollectionLine>,
    line_counts: RefCell<Vec<usize>>,
    first_description_values: RefCell<Vec<String>>,
    second_description_values: RefCell<Vec<String>>,
}

impl CollectionSelectorProbe {
    fn new() -> Self {
        let form = FormHandle::new(invoice_collection_form());
        let lines = form.collection(InvoiceCollectionForm::fields().lines());
        let items = lines.items();
        let description_path = InvoiceCollectionLine::fields().description();

        Self {
            form,
            first_description: items[0].text(description_path.clone()),
            second_description: items[1].text(description_path),
            line_counts: RefCell::new(Vec::new()),
            first_description_values: RefCell::new(Vec::new()),
            second_description_values: RefCell::new(Vec::new()),
        }
    }
}

fn collection_count_selector_probe(probe: Rc<CollectionSelectorProbe>) -> Element {
    let count = probe
        .form
        .collection(InvoiceCollectionForm::fields().lines())
        .items()
        .len();

    probe.line_counts.borrow_mut().push(count);

    VNode::empty()
}

fn first_collection_item_value_selector_probe(probe: Rc<CollectionSelectorProbe>) -> Element {
    let value = probe.first_description.value();

    probe.first_description_values.borrow_mut().push(value);

    VNode::empty()
}

fn second_collection_item_value_selector_probe(probe: Rc<CollectionSelectorProbe>) -> Element {
    let value = probe.second_description.value();

    probe.second_description_values.borrow_mut().push(value);

    VNode::empty()
}

#[test]
fn collection_item_value_selectors_do_not_rerender_unrelated_item_readers() {
    let probe = Rc::new(CollectionSelectorProbe::new());
    let mut first_dom = VirtualDom::new_with_props(
        first_collection_item_value_selector_probe,
        Rc::clone(&probe),
    );
    let mut second_dom = VirtualDom::new_with_props(
        second_collection_item_value_selector_probe,
        Rc::clone(&probe),
    );

    first_dom.rebuild_in_place();
    second_dom.rebuild_in_place();

    assert_eq!(
        probe.first_description_values.borrow().as_slice(),
        ["Design"]
    );
    assert_eq!(
        probe.second_description_values.borrow().as_slice(),
        ["Build"]
    );

    probe.second_description.on_input("Build v2");
    first_dom.render_immediate_to_vec();
    second_dom.render_immediate_to_vec();

    assert_eq!(
        probe.first_description_values.borrow().as_slice(),
        ["Design"]
    );
    assert_eq!(
        probe.second_description_values.borrow().as_slice(),
        ["Build", "Build v2"]
    );
}

#[test]
fn collection_structure_selectors_rerender_without_rerendering_item_value_readers() {
    let probe = Rc::new(CollectionSelectorProbe::new());
    let mut count_dom =
        VirtualDom::new_with_props(collection_count_selector_probe, Rc::clone(&probe));
    let mut first_dom = VirtualDom::new_with_props(
        first_collection_item_value_selector_probe,
        Rc::clone(&probe),
    );

    count_dom.rebuild_in_place();
    first_dom.rebuild_in_place();

    assert_eq!(probe.line_counts.borrow().as_slice(), [2]);
    assert_eq!(
        probe.first_description_values.borrow().as_slice(),
        ["Design"]
    );

    probe
        .form
        .collection(InvoiceCollectionForm::fields().lines())
        .append(InvoiceCollectionLine {
            description: "Ship".to_owned(),
            quantity: 1,
        });
    count_dom.render_immediate_to_vec();
    first_dom.render_immediate_to_vec();

    assert_eq!(probe.line_counts.borrow().as_slice(), [2, 3]);
    assert_eq!(
        probe.first_description_values.borrow().as_slice(),
        ["Design"]
    );
}

struct FieldMetadataSelectorProbe {
    form: FormHandle<ProfileForm>,
    email_touched_values: RefCell<Vec<bool>>,
    terms_touched_values: RefCell<Vec<bool>>,
}

impl FieldMetadataSelectorProbe {
    fn new() -> Self {
        Self {
            form: FormHandle::new(ProfileForm {
                email: String::new(),
                accepts_terms: false,
            }),
            email_touched_values: RefCell::new(Vec::new()),
            terms_touched_values: RefCell::new(Vec::new()),
        }
    }
}

fn email_metadata_selector_probe(probe: Rc<FieldMetadataSelectorProbe>) -> Element {
    let is_touched = probe
        .form
        .field_metadata(ProfileForm::fields().email())
        .is_touched();

    probe.email_touched_values.borrow_mut().push(is_touched);

    VNode::empty()
}

fn terms_metadata_selector_probe(probe: Rc<FieldMetadataSelectorProbe>) -> Element {
    let is_touched = probe
        .form
        .field_metadata(ProfileForm::fields().accepts_terms())
        .is_touched();

    probe.terms_touched_values.borrow_mut().push(is_touched);

    VNode::empty()
}

#[test]
fn field_metadata_selectors_do_not_rerender_unrelated_field_readers() {
    let probe = Rc::new(FieldMetadataSelectorProbe::new());
    let mut email_dom =
        VirtualDom::new_with_props(email_metadata_selector_probe, Rc::clone(&probe));
    let mut terms_dom =
        VirtualDom::new_with_props(terms_metadata_selector_probe, Rc::clone(&probe));

    email_dom.rebuild_in_place();
    terms_dom.rebuild_in_place();

    assert_eq!(probe.email_touched_values.borrow().as_slice(), [false]);
    assert_eq!(probe.terms_touched_values.borrow().as_slice(), [false]);

    probe.form.mark_field_touched(ProfileForm::fields().email());
    email_dom.render_immediate_to_vec();
    terms_dom.render_immediate_to_vec();

    assert_eq!(
        probe.email_touched_values.borrow().as_slice(),
        [false, true]
    );
    assert_eq!(probe.terms_touched_values.borrow().as_slice(), [false]);

    probe
        .form
        .mark_field_touched(ProfileForm::fields().accepts_terms());
    email_dom.render_immediate_to_vec();
    terms_dom.render_immediate_to_vec();

    assert_eq!(probe.email_touched_values.borrow().len(), 2);
    assert_eq!(
        probe.terms_touched_values.borrow().as_slice(),
        [false, true]
    );
}

struct FieldValidationSelectorProbe {
    form: FormHandle<ProfileForm, &'static str>,
    email_validator: ValidatorId,
    email_error_counts: RefCell<Vec<usize>>,
    terms_error_counts: RefCell<Vec<usize>>,
}

impl FieldValidationSelectorProbe {
    fn new() -> Self {
        let form: FormHandle<ProfileForm, &'static str> =
            FormHandle::new_with_error_type(ProfileForm {
                email: String::new(),
                accepts_terms: false,
            });
        let fields = ProfileForm::fields();
        let email_validator = form
            .field(fields.email())
            .validator("required")
            .on(ValidationTrigger::Manual)
            .check(|value, _context| {
                if value.is_empty() {
                    vec!["email_required"]
                } else {
                    Vec::new()
                }
            });
        form.field(fields.accepts_terms())
            .validator("accepted")
            .on(ValidationTrigger::Manual)
            .check(|value, _context| {
                if *value {
                    Vec::new()
                } else {
                    vec!["terms_required"]
                }
            });

        Self {
            form,
            email_validator,
            email_error_counts: RefCell::new(Vec::new()),
            terms_error_counts: RefCell::new(Vec::new()),
        }
    }
}

fn email_validation_selector_probe(probe: Rc<FieldValidationSelectorProbe>) -> Element {
    let error_count = probe
        .form
        .field_validation_errors(ProfileForm::fields().email())
        .len();

    probe.email_error_counts.borrow_mut().push(error_count);

    VNode::empty()
}

fn terms_validation_selector_probe(probe: Rc<FieldValidationSelectorProbe>) -> Element {
    let error_count = probe
        .form
        .field_validation_errors(ProfileForm::fields().accepts_terms())
        .len();

    probe.terms_error_counts.borrow_mut().push(error_count);

    VNode::empty()
}

#[test]
fn field_validation_selectors_do_not_rerender_unrelated_field_readers_for_one_field_validator() {
    let probe = Rc::new(FieldValidationSelectorProbe::new());
    let mut email_dom =
        VirtualDom::new_with_props(email_validation_selector_probe, Rc::clone(&probe));
    let mut terms_dom =
        VirtualDom::new_with_props(terms_validation_selector_probe, Rc::clone(&probe));

    email_dom.rebuild_in_place();
    terms_dom.rebuild_in_place();

    assert_eq!(probe.email_error_counts.borrow().as_slice(), [0]);
    assert_eq!(probe.terms_error_counts.borrow().as_slice(), [0]);

    probe.form.validate_field_validator(
        ProfileForm::fields().email(),
        probe.email_validator,
        ValidationTrigger::Manual,
    );
    email_dom.render_immediate_to_vec();
    terms_dom.render_immediate_to_vec();

    assert_eq!(probe.email_error_counts.borrow().as_slice(), [0, 1]);
    assert_eq!(probe.terms_error_counts.borrow().as_slice(), [0]);
}

// Registers a description validator that only rejects the reserved "Design" value, then validates.
fn invoice_form_with_reserved_description() -> FormHandle<InvoiceCollectionForm, String> {
    let handle: FormHandle<InvoiceCollectionForm, String> =
        FormHandle::new_with_error_type(invoice_collection_form());
    handle.set_error_visibility_policy(dioform::ErrorVisibilityPolicy::Always);
    let lines = handle.collection(InvoiceCollectionForm::fields().lines());
    lines
        .item_field_validator(InvoiceCollectionLine::fields().description(), "reserved")
        .on(ValidationTrigger::Manual)
        .check(|value, _context| {
            if value == "Design" {
                vec!["reserved".to_owned()]
            } else {
                Vec::new()
            }
        });
    handle.validate_all(ValidationTrigger::Manual);
    handle
}

#[test]
fn dioxus_collection_swap_preserves_item_identity_and_state() {
    let handle = invoice_form_with_reserved_description();
    let lines = handle.collection(InvoiceCollectionForm::fields().lines());
    let description = InvoiceCollectionLine::fields().description();

    let items = lines.items();
    let first_id = items[0].identity(); // the "Design" line, which carries the reserved error
    let second_id = items[1].identity(); // the "Build" line, no error
    assert_eq!(
        items[0].text(description.clone()).validation_errors().len(),
        1
    );
    assert!(
        items[1]
            .text(description.clone())
            .validation_errors()
            .is_empty()
    );

    // Swapping an index with itself is a no-op and reports no change.
    assert!(!lines.swap(1, 1));

    // Swap the two positions.
    assert!(lines.swap(0, 1));

    let swapped = lines.items();
    // Values exchanged...
    assert_eq!(swapped[0].text(description.clone()).value(), "Build");
    assert_eq!(swapped[1].text(description.clone()).value(), "Design");
    // ...but each item keeps its logical identity, and item-scoped state follows it.
    assert_eq!(swapped[1].identity(), first_id);
    assert_eq!(swapped[0].identity(), second_id);
    assert_eq!(
        swapped[1]
            .text(description.clone())
            .validation_errors()
            .len(),
        1
    );
    assert!(swapped[0].text(description).validation_errors().is_empty());
}

#[test]
fn dioxus_collection_replace_is_in_place_and_keeps_identity() {
    let handle = invoice_form_with_reserved_description();
    let lines = handle.collection(InvoiceCollectionForm::fields().lines());
    let description = InvoiceCollectionLine::fields().description();

    let first_id = lines.items()[0].identity();
    let second_id = lines.items()[1].identity();

    // Replace the first item's value in place.
    assert!(lines.replace(
        0,
        InvoiceCollectionLine {
            description: "Deploy".to_owned(),
            quantity: 5,
        },
    ));

    let items = lines.items();
    // The value changed but the item kept its identity, and the sibling is untouched.
    assert_eq!(items[0].text(description).value(), "Deploy");
    assert_eq!(items[0].identity(), first_id);
    assert_eq!(items[1].identity(), second_id);
    assert_eq!(items.len(), 2);
}

#[test]
fn dioxus_collection_clear_removes_all_items_and_releases_item_state() {
    let handle = invoice_form_with_reserved_description();
    let lines = handle.collection(InvoiceCollectionForm::fields().lines());

    // The reserved error is present before clearing.
    assert_eq!(handle.validation_errors().len(), 1);
    assert_eq!(lines.items().len(), 2);

    assert!(lines.clear());

    // Every item and its item-scoped validation state is gone; the collection is empty.
    assert!(lines.items().is_empty());
    assert!(
        handle
            .field_value(InvoiceCollectionForm::fields().lines())
            .is_empty()
    );
    assert!(handle.validation_errors().is_empty());

    // Clearing an already-empty collection is a no-op.
    assert!(!lines.clear());
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct PreferencesForm {
    biography: String,
    accepts_terms: bool,
}

#[test]
fn dioxus_facade_textarea_binding_updates_the_typed_field() {
    let handle = FormHandle::new(PreferencesForm {
        biography: String::new(),
        accepts_terms: false,
    });
    let biography_path = PreferencesForm::fields().biography();
    let biography = handle.textarea(biography_path.clone());

    assert_eq!(biography.name(), "biography");
    assert_eq!(biography.value(), "");
    assert!(!handle.is_field_touched(biography_path.clone()));
    assert!(!handle.is_field_blurred(biography_path.clone()));

    biography.set_value("programmatic draft");

    assert_eq!(biography.value(), "programmatic draft");
    assert!(handle.is_field_dirty(biography_path.clone()));
    assert!(!handle.is_field_touched(biography_path.clone()));

    biography.on_input("Ada\nLovelace");

    assert_eq!(biography.value(), "Ada\nLovelace");
    assert!(handle.is_field_dirty(biography_path.clone()));
    assert!(handle.is_field_touched(biography_path.clone()));
    assert!(!handle.is_field_blurred(biography_path.clone()));
    assert_eq!(
        handle.field_value(PreferencesForm::fields().biography()),
        "Ada\nLovelace"
    );

    biography.on_blur();

    assert!(handle.is_field_touched(biography_path.clone()));
    assert!(handle.is_field_blurred(biography_path));
}

#[test]
fn dioxus_pristine_and_default_value_readers_track_the_non_sticky_baseline() {
    let handle = FormHandle::new(PreferencesForm {
        biography: String::new(),
        accepts_terms: false,
    });
    let biography = PreferencesForm::fields().biography();

    // A freshly initialized form is pristine and every field holds its default (baseline) value.
    assert!(handle.is_pristine());
    assert!(handle.is_default_value(biography.clone()));

    handle.set_user_field(biography.clone(), "Ada".to_owned());

    assert!(!handle.is_pristine());
    assert!(!handle.is_default_value(biography.clone()));
    // Pristine is the exact inverse of dirty.
    assert_eq!(handle.is_pristine(), !handle.is_dirty());

    // Non-sticky: reverting a field to its baseline value makes it default/pristine again.
    handle.set_user_field(biography.clone(), String::new());

    assert!(handle.is_pristine());
    assert!(handle.is_default_value(biography));
}

#[test]
fn dioxus_facade_checkbox_binding_updates_the_typed_field() {
    let handle = FormHandle::new(PreferencesForm {
        biography: String::new(),
        accepts_terms: false,
    });
    let accepts_terms_path = PreferencesForm::fields().accepts_terms();
    let accepts_terms = handle.checkbox(accepts_terms_path.clone());

    assert_eq!(accepts_terms.name(), "accepts_terms");
    assert!(!accepts_terms.checked());
    assert!(!handle.is_field_touched(accepts_terms_path.clone()));
    assert!(!handle.is_field_blurred(accepts_terms_path.clone()));

    accepts_terms.set_checked(true);

    assert!(accepts_terms.checked());
    assert!(handle.is_field_dirty(accepts_terms_path.clone()));
    assert!(!handle.is_field_touched(accepts_terms_path.clone()));

    accepts_terms.on_change(false);

    assert!(!accepts_terms.checked());
    assert!(!handle.is_field_dirty(accepts_terms_path.clone()));
    assert!(handle.is_field_touched(accepts_terms_path.clone()));
    assert!(!handle.is_field_blurred(accepts_terms_path.clone()));
    assert!(!handle.field_value(PreferencesForm::fields().accepts_terms()));

    accepts_terms.on_change(true);

    assert!(accepts_terms.checked());
    assert!(handle.is_field_dirty(accepts_terms_path.clone()));

    accepts_terms.on_blur();

    assert!(handle.is_field_touched(accepts_terms_path.clone()));
    assert!(handle.is_field_blurred(accepts_terms_path));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Plan {
    Starter,
    Pro,
    Enterprise,
}

fn parse_plan(value: &str) -> Result<Plan, &'static str> {
    match value {
        "starter" => Ok(Plan::Starter),
        "pro" => Ok(Plan::Pro),
        "enterprise" => Ok(Plan::Enterprise),
        _ => Err("unknown plan"),
    }
}

fn format_plan(value: &Plan) -> String {
    match value {
        Plan::Starter => "starter",
        Plan::Pro => "pro",
        Plan::Enterprise => "enterprise",
    }
    .to_owned()
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct PlanForm {
    plan: Plan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Topic {
    Rust,
    Dioxus,
    Accessibility,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct MultiSelectForm {
    topics: Vec<Topic>,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct CollectionHelperForm {
    rows: Vec<CollectionHelperRow>,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct CollectionHelperRow {
    enabled: bool,
    plan: Plan,
    starts_on: DateYmd,
}

fn collection_helper_form() -> CollectionHelperForm {
    CollectionHelperForm {
        rows: vec![
            CollectionHelperRow {
                enabled: false,
                plan: Plan::Starter,
                starts_on: DateYmd {
                    year: 2026,
                    month: 1,
                    day: 1,
                },
            },
            CollectionHelperRow {
                enabled: true,
                plan: Plan::Pro,
                starts_on: DateYmd {
                    year: 2026,
                    month: 2,
                    day: 2,
                },
            },
        ],
    }
}

fn collection_helper_metadata<Value>(
    handle: &FormHandle<CollectionHelperForm>,
    item: CollectionItemIdentity,
    field: FieldPath<CollectionHelperRow, Value>,
) -> (bool, bool) {
    handle.read_core(|core| {
        let metadata =
            core.collection_item_field_metadata(CollectionHelperForm::fields().rows(), item, field);

        (metadata.is_touched(), metadata.is_blurred())
    })
}

#[test]
fn dioxus_collection_checkbox_binding_updates_metadata_and_reorders() {
    let handle = FormHandle::new_with_id_namespace(collection_helper_form(), "collection-checks");
    let rows_path = CollectionHelperForm::fields().rows();
    let enabled_path = CollectionHelperRow::fields().enabled();
    let rows = handle.collection(rows_path);
    let items = rows.items();
    let second = items[1].clone();
    let enabled = second.checkbox(enabled_path.clone());
    let input_id = enabled.accessibility().input_id().to_owned();

    assert_eq!(enabled.name(), "rows[1].enabled");
    assert_eq!(
        input_id,
        format!(
            "collection%2dchecks-rows%2e{}%2eenabled-input",
            second.identity().key().replace('-', "%2d")
        )
    );
    assert!(enabled.checked());
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), enabled_path.clone()),
        (false, false)
    );

    enabled.set_checked(false);

    assert!(!enabled.checked());
    assert!(!handle.snapshot().rows[1].enabled);
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), enabled_path.clone()),
        (false, false)
    );

    enabled.on_change(true);

    assert!(enabled.checked());
    assert!(handle.snapshot().rows[1].enabled);
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), enabled_path.clone()),
        (true, false)
    );

    enabled.on_blur();

    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), enabled_path.clone()),
        (true, true)
    );
    assert!(rows.move_to_index(second.identity(), 0));

    let moved = rows.items()[0].checkbox(enabled_path.clone());

    assert_eq!(moved.name(), "rows[0].enabled");
    assert_eq!(moved.accessibility().input_id(), input_id.as_str());
    assert!(moved.checked());
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), enabled_path),
        (true, true)
    );
}

#[test]
fn dioxus_collection_select_binding_updates_metadata_and_reorders() {
    let handle = FormHandle::new_with_id_namespace(collection_helper_form(), "collection-selects");
    let rows_path = CollectionHelperForm::fields().rows();
    let plan_path = CollectionHelperRow::fields().plan();
    let rows = handle.collection(rows_path);
    let second = rows.items()[1].clone();
    let plan = second.select(plan_path.clone());
    let input_id = plan.accessibility().input_id().to_owned();

    assert_eq!(plan.name(), "rows[1].plan");
    assert_eq!(
        input_id,
        format!(
            "collection%2dselects-rows%2e{}%2eplan-input",
            second.identity().key().replace('-', "%2d")
        )
    );
    assert_eq!(plan.value(), Plan::Pro);
    assert!(plan.is_selected(&Plan::Pro));
    assert!(!plan.is_selected(&Plan::Enterprise));

    plan.set_value(Plan::Enterprise);

    assert_eq!(plan.value(), Plan::Enterprise);
    assert_eq!(handle.snapshot().rows[1].plan, Plan::Enterprise);
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path.clone()),
        (false, false)
    );

    plan.on_change(Plan::Starter);

    assert_eq!(plan.value(), Plan::Starter);
    assert_eq!(handle.snapshot().rows[1].plan, Plan::Starter);
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path.clone()),
        (true, false)
    );

    plan.on_blur();

    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path.clone()),
        (true, true)
    );
    assert!(rows.move_to_index(second.identity(), 0));

    let moved = rows.items()[0].select(plan_path.clone());

    assert_eq!(moved.name(), "rows[0].plan");
    assert_eq!(moved.accessibility().input_id(), input_id.as_str());
    assert_eq!(moved.value(), Plan::Starter);
    assert!(moved.is_selected(&Plan::Starter));
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path),
        (true, true)
    );
}

#[test]
fn dioxus_collection_rendered_select_binding_updates_metadata_and_reorders() {
    let handle =
        FormHandle::new_with_id_namespace(collection_helper_form(), "collection-rendered-selects");
    let rows_path = CollectionHelperForm::fields().rows();
    let plan_path = CollectionHelperRow::fields().plan();
    let rows = handle.collection(rows_path);
    let second = rows.items()[1].clone();
    let plan = second.select_with(plan_path.clone(), parse_plan, format_plan);
    let input_id = plan.accessibility().input_id().to_owned();

    assert_eq!(plan.name(), "rows[1].plan");
    assert_eq!(plan.value(), "pro");
    assert_eq!(plan.typed_value(), Plan::Pro);
    assert!(plan.is_rendered_selected("pro"));
    assert!(plan.is_selected(&Plan::Pro));

    plan.set_value(Plan::Enterprise);

    assert_eq!(plan.value(), "enterprise");
    assert_eq!(handle.snapshot().rows[1].plan, Plan::Enterprise);
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path.clone()),
        (false, false)
    );

    assert_eq!(
        plan.try_on_change("missing"),
        Err("unknown plan".to_owned())
    );

    assert_eq!(plan.typed_value(), Plan::Enterprise);
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path.clone()),
        (true, false)
    );

    plan.on_change("starter");

    assert_eq!(plan.value(), "starter");
    assert_eq!(handle.snapshot().rows[1].plan, Plan::Starter);
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path.clone()),
        (true, false)
    );

    plan.on_blur();

    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path.clone()),
        (true, true)
    );
    assert!(rows.move_to_index(second.identity(), 0));

    let moved = rows.items()[0].select_with(plan_path.clone(), parse_plan, format_plan);

    assert_eq!(moved.name(), "rows[0].plan");
    assert_eq!(moved.accessibility().input_id(), input_id.as_str());
    assert_eq!(moved.value(), "starter");
    assert_eq!(moved.typed_value(), Plan::Starter);
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path),
        (true, true)
    );
}

#[test]
fn dioxus_collection_radio_group_binding_updates_metadata_and_reorders() {
    let handle = FormHandle::new_with_id_namespace(collection_helper_form(), "collection-radios");
    let rows_path = CollectionHelperForm::fields().rows();
    let plan_path = CollectionHelperRow::fields().plan();
    let rows = handle.collection(rows_path);
    let second = rows.items()[1].clone();
    let plan = second.radio_group(plan_path.clone());
    let input_id = plan.accessibility().input_id().to_owned();

    assert_eq!(plan.name(), "rows[1].plan");
    assert_eq!(plan.value(), Plan::Pro);
    assert!(plan.is_selected(&Plan::Pro));
    assert!(!plan.is_selected(&Plan::Enterprise));

    plan.set_value(Plan::Enterprise);

    assert_eq!(plan.value(), Plan::Enterprise);
    assert_eq!(handle.snapshot().rows[1].plan, Plan::Enterprise);
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path.clone()),
        (false, false)
    );

    plan.select(Plan::Starter);

    assert_eq!(plan.value(), Plan::Starter);
    assert_eq!(handle.snapshot().rows[1].plan, Plan::Starter);
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path.clone()),
        (true, false)
    );

    plan.on_blur();

    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path.clone()),
        (true, true)
    );
    assert!(rows.move_to_index(second.identity(), 0));

    let moved = rows.items()[0].radio_group(plan_path.clone());

    assert_eq!(moved.name(), "rows[0].plan");
    assert_eq!(moved.accessibility().input_id(), input_id.as_str());
    assert_eq!(moved.value(), Plan::Starter);
    assert!(moved.is_selected(&Plan::Starter));
    assert_eq!(
        collection_helper_metadata(&handle, second.identity(), plan_path),
        (true, true)
    );
}

#[derive(Debug, Eq, PartialEq)]
struct CollectionDateHookSnapshot {
    name: String,
    rendered_value: String,
    parse_error_count: usize,
    form_parse_error_count: usize,
    can_submit: bool,
    draft_dates: Vec<String>,
    input_id: String,
}

#[derive(Default)]
struct CollectionDateHookProbe {
    handle: RefCell<Option<FormHandle<CollectionHelperForm>>>,
    starts_on: RefCell<
        Option<CollectionParsedTextBinding<CollectionHelperForm, CollectionHelperRow, DateYmd>>,
    >,
    tracked_item: RefCell<Option<CollectionItemIdentity>>,
    snapshots: RefCell<Vec<CollectionDateHookSnapshot>>,
}

fn collection_item_date_hook_probe(probe: Rc<CollectionDateHookProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new_with_id_namespace(collection_helper_form(), "collection-dates")
    });
    let rows = form.collection(CollectionHelperForm::fields().rows());
    let items = rows.items();
    let tracked_item = {
        let mut tracked_item = probe.tracked_item.borrow_mut();

        match *tracked_item {
            Some(item) => item,
            None => {
                let item = items[1].identity();
                tracked_item.replace(item);
                item
            }
        }
    };
    let item = items
        .into_iter()
        .find(|item| item.identity() == tracked_item)
        .expect("tracked collection item should still be mounted");
    let starts_on = use_collection_item_date(item, CollectionHelperRow::fields().starts_on());
    let snapshot = form.snapshot();

    probe.handle.borrow_mut().replace(form.clone());
    probe.starts_on.borrow_mut().replace(starts_on.clone());
    probe
        .snapshots
        .borrow_mut()
        .push(CollectionDateHookSnapshot {
            name: starts_on.name(),
            rendered_value: starts_on.value(),
            parse_error_count: usize::from(starts_on.parse_error().is_some()),
            form_parse_error_count: form.parse_errors().len(),
            can_submit: form.can_submit(),
            draft_dates: snapshot
                .rows
                .iter()
                .map(|row| format_date_ymd(&row.starts_on))
                .collect(),
            input_id: starts_on.accessibility().input_id().to_owned(),
        });

    VNode::empty()
}

#[test]
fn collection_item_date_hook_preserves_parse_state_and_updates_name_after_reorder() {
    let probe = Rc::new(CollectionDateHookProbe::default());
    let mut dom = VirtualDom::new_with_props(collection_item_date_hook_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let tracked_item = probe
        .tracked_item
        .borrow()
        .expect("probe should track a collection item identity");
    let expected_input_id = format!(
        "collection%2ddates-rows%2e{}%2estarts_on-input",
        tracked_item.key().replace('-', "%2d")
    );

    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [CollectionDateHookSnapshot {
            name: "rows[1].starts_on".to_owned(),
            rendered_value: "2026-02-02".to_owned(),
            parse_error_count: 0,
            form_parse_error_count: 0,
            can_submit: true,
            draft_dates: vec!["2026-01-01".to_owned(), "2026-02-02".to_owned()],
            input_id: expected_input_id.clone(),
        }]
    );

    let starts_on = probe
        .starts_on
        .borrow()
        .as_ref()
        .expect("probe should expose the collection item date binding")
        .clone();

    starts_on.on_input("not-a-date");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&CollectionDateHookSnapshot {
            name: "rows[1].starts_on".to_owned(),
            rendered_value: "not-a-date".to_owned(),
            parse_error_count: 1,
            form_parse_error_count: 1,
            can_submit: false,
            draft_dates: vec!["2026-01-01".to_owned(), "2026-02-02".to_owned()],
            input_id: expected_input_id.clone(),
        })
    );

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose the form handle")
        .clone();
    let starts_on_path = CollectionHelperRow::fields().starts_on();

    assert_eq!(
        collection_helper_metadata(&handle, tracked_item, starts_on_path.clone()),
        (true, false)
    );

    starts_on.on_blur();

    assert_eq!(
        collection_helper_metadata(&handle, tracked_item, starts_on_path.clone()),
        (true, true)
    );
    assert!(
        handle
            .collection(CollectionHelperForm::fields().rows())
            .move_to_index(tracked_item, 0)
    );
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&CollectionDateHookSnapshot {
            name: "rows[0].starts_on".to_owned(),
            rendered_value: "not-a-date".to_owned(),
            parse_error_count: 1,
            form_parse_error_count: 1,
            can_submit: false,
            draft_dates: vec!["2026-02-02".to_owned(), "2026-01-01".to_owned()],
            input_id: expected_input_id.clone(),
        })
    );

    let starts_on = probe
        .starts_on
        .borrow()
        .as_ref()
        .expect("probe should expose the moved collection item date binding")
        .clone();

    starts_on.on_input("2026-03-04");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&CollectionDateHookSnapshot {
            name: "rows[0].starts_on".to_owned(),
            rendered_value: "2026-03-04".to_owned(),
            parse_error_count: 0,
            form_parse_error_count: 0,
            can_submit: true,
            draft_dates: vec!["2026-03-04".to_owned(), "2026-01-01".to_owned()],
            input_id: expected_input_id,
        })
    );
    assert_eq!(
        collection_helper_metadata(&handle, tracked_item, starts_on_path),
        (true, true)
    );
}

#[derive(Debug, Eq, PartialEq)]
struct NonCloneChoice(&'static str);

#[derive(Debug, Eq, Form, PartialEq)]
struct NonCloneChoiceForm {
    choice: NonCloneChoice,
}

impl Clone for NonCloneChoiceForm {
    fn clone(&self) -> Self {
        Self {
            choice: NonCloneChoice(self.choice.0),
        }
    }
}

trait CommonFieldBindingContract<Value> {
    fn contract_name(&self) -> &str;

    fn contract_accessibility(&self) -> FieldAccessibility;

    fn contract_value(&self) -> Value;

    fn contract_set_programmatic(&self, value: Value);

    fn contract_set_user(&self, value: Value);

    fn contract_blur(&self);
}

impl<Model> CommonFieldBindingContract<String> for dioform::TextBinding<Model, &'static str> {
    fn contract_name(&self) -> &str {
        self.name()
    }

    fn contract_accessibility(&self) -> FieldAccessibility {
        self.accessibility()
    }

    fn contract_value(&self) -> String {
        self.value()
    }

    fn contract_set_programmatic(&self, value: String) {
        self.set_value(value);
    }

    fn contract_set_user(&self, value: String) {
        self.on_input(value);
    }

    fn contract_blur(&self) {
        self.on_blur();
    }
}

impl<Model> CommonFieldBindingContract<String> for dioform::TextareaBinding<Model, &'static str> {
    fn contract_name(&self) -> &str {
        self.name()
    }

    fn contract_accessibility(&self) -> FieldAccessibility {
        self.accessibility()
    }

    fn contract_value(&self) -> String {
        self.value()
    }

    fn contract_set_programmatic(&self, value: String) {
        self.set_value(value);
    }

    fn contract_set_user(&self, value: String) {
        self.on_input(value);
    }

    fn contract_blur(&self) {
        self.on_blur();
    }
}

impl<Model> CommonFieldBindingContract<bool> for dioform::CheckboxBinding<Model, &'static str> {
    fn contract_name(&self) -> &str {
        self.name()
    }

    fn contract_accessibility(&self) -> FieldAccessibility {
        self.accessibility()
    }

    fn contract_value(&self) -> bool {
        self.checked()
    }

    fn contract_set_programmatic(&self, value: bool) {
        self.set_checked(value);
    }

    fn contract_set_user(&self, value: bool) {
        self.on_change(value);
    }

    fn contract_blur(&self) {
        self.on_blur();
    }
}

impl<Model, Value> CommonFieldBindingContract<Value>
    for dioform::SelectBinding<Model, Value, &'static str>
where
    Value: Clone,
{
    fn contract_name(&self) -> &str {
        self.name()
    }

    fn contract_accessibility(&self) -> FieldAccessibility {
        self.accessibility()
    }

    fn contract_value(&self) -> Value {
        self.value()
    }

    fn contract_set_programmatic(&self, value: Value) {
        self.set_value(value);
    }

    fn contract_set_user(&self, value: Value) {
        self.on_change(value);
    }

    fn contract_blur(&self) {
        self.on_blur();
    }
}

impl<Model, Value> CommonFieldBindingContract<Value>
    for dioform::RenderedSelectBinding<Model, Value, &'static str>
where
    Value: Clone,
{
    fn contract_name(&self) -> &str {
        self.name()
    }

    fn contract_accessibility(&self) -> FieldAccessibility {
        self.accessibility()
    }

    fn contract_value(&self) -> Value {
        self.typed_value()
    }

    fn contract_set_programmatic(&self, value: Value) {
        self.set_value(value);
    }

    fn contract_set_user(&self, value: Value) {
        self.select(value);
    }

    fn contract_blur(&self) {
        self.on_blur();
    }
}

impl<Model, Value> CommonFieldBindingContract<Value>
    for dioform::RadioGroupBinding<Model, Value, &'static str>
where
    Value: Clone,
{
    fn contract_name(&self) -> &str {
        self.name()
    }

    fn contract_accessibility(&self) -> FieldAccessibility {
        self.accessibility()
    }

    fn contract_value(&self) -> Value {
        self.value()
    }

    fn contract_set_programmatic(&self, value: Value) {
        self.set_value(value);
    }

    fn contract_set_user(&self, value: Value) {
        self.on_change(value);
    }

    fn contract_blur(&self) {
        self.on_blur();
    }
}

fn assert_common_field_binding_contract<Model, Value, Binding>(
    initial: Model,
    path: FieldPath<Model, Value>,
    initial_value: Value,
    programmatic_value: Value,
    user_value: Value,
    create_binding: impl FnOnce(&FormHandle<Model, &'static str>, FieldPath<Model, Value>) -> Binding,
) where
    Model: Clone + 'static,
    Value: Clone + Debug + PartialEq + 'static,
    Binding: CommonFieldBindingContract<Value>,
{
    let handle: FormHandle<Model, &'static str> =
        FormHandle::new_with_error_type(initial).with_id_namespace("binding-contract");
    let observer_events = Rc::new(RefCell::new(Vec::new()));
    let captured_events = Rc::clone(&observer_events);
    let value_change_runs = Rc::new(Cell::new(0));
    let value_change_runs_for_validator = Rc::clone(&value_change_runs);
    let blur_runs = Rc::new(Cell::new(0));
    let blur_runs_for_validator = Rc::clone(&blur_runs);

    handle.write_advanced(|core| {
        core.observe(move |event| captured_events.borrow_mut().push(event.clone()));
        core.register_sync_field_validator_for_triggers(
            path.clone(),
            "contract_value_change",
            ValidationTrigger::Change,
            move |_value, context| {
                value_change_runs_for_validator.set(value_change_runs_for_validator.get() + 1);
                assert_eq!(context.trigger(), ValidationTrigger::Change);
                Vec::new()
            },
        );
        core.register_sync_field_validator_for_triggers(
            path.clone(),
            "contract_blur",
            ValidationTrigger::Blur,
            move |_value, context| {
                blur_runs_for_validator.set(blur_runs_for_validator.get() + 1);
                assert_eq!(context.trigger(), ValidationTrigger::Blur);
                Vec::new()
            },
        );
    });

    let binding = create_binding(&handle, path.clone());

    assert_eq!(binding.contract_name(), path.field_name());
    assert_eq!(
        binding.contract_accessibility(),
        handle.field_accessibility(path.clone())
    );
    assert_eq!(binding.contract_value(), initial_value);

    binding.contract_set_programmatic(programmatic_value.clone());

    assert_eq!(binding.contract_value(), programmatic_value);
    assert_eq!(handle.field_value(path.clone()), programmatic_value);
    assert!(!handle.is_field_touched(path.clone()));
    assert!(observer_events.borrow().iter().any(|event| {
        matches!(
            event,
            FormObserverEvent::FieldUpdated { field, origin: FieldUpdateOrigin::Programmatic, .. }
                if field.identity() == path.identity()
        )
    }));

    handle.set_validation_mode(ValidationMode::on_change());
    binding.contract_set_user(user_value.clone());

    assert_eq!(binding.contract_value(), user_value);
    assert_eq!(handle.field_value(path.clone()), user_value);
    assert!(handle.is_field_touched(path.clone()));
    assert_eq!(value_change_runs.get(), 1);
    assert!(observer_events.borrow().iter().any(|event| {
        matches!(
            event,
            FormObserverEvent::FieldUpdated { field, origin: FieldUpdateOrigin::User, .. }
                if field.identity() == path.identity()
        )
    }));

    binding.contract_blur();

    assert!(handle.is_field_blurred(path));
    assert_eq!(blur_runs.get(), 1);
}

#[test]
fn dioxus_controlled_bindings_share_common_field_binding_contract() {
    assert_common_field_binding_contract(
        SignupForm {
            email: String::new(),
        },
        SignupForm::fields().email(),
        String::new(),
        "programmatic@example.com".to_owned(),
        "user@example.com".to_owned(),
        |handle, path| handle.text(path),
    );
    assert_common_field_binding_contract(
        PreferencesForm {
            biography: String::new(),
            accepts_terms: false,
        },
        PreferencesForm::fields().biography(),
        String::new(),
        "programmatic biography".to_owned(),
        "user biography".to_owned(),
        |handle, path| handle.textarea(path),
    );
    assert_common_field_binding_contract(
        PreferencesForm {
            biography: String::new(),
            accepts_terms: false,
        },
        PreferencesForm::fields().accepts_terms(),
        false,
        true,
        false,
        |handle, path| handle.checkbox(path),
    );
    assert_common_field_binding_contract(
        PlanForm {
            plan: Plan::Starter,
        },
        PlanForm::fields().plan(),
        Plan::Starter,
        Plan::Pro,
        Plan::Enterprise,
        |handle, path| handle.select(path),
    );
    assert_common_field_binding_contract(
        PlanForm {
            plan: Plan::Starter,
        },
        PlanForm::fields().plan(),
        Plan::Starter,
        Plan::Pro,
        Plan::Enterprise,
        |handle, path| handle.select_with(path, parse_plan, format_plan),
    );
    assert_common_field_binding_contract(
        PlanForm {
            plan: Plan::Starter,
        },
        PlanForm::fields().plan(),
        Plan::Starter,
        Plan::Pro,
        Plan::Enterprise,
        |handle, path| handle.radio_group(path),
    );
}

#[test]
fn dioxus_facade_select_binding_updates_a_typed_field() {
    let observer_events = Rc::new(RefCell::new(Vec::new()));
    let captured_events = Rc::clone(&observer_events);
    let handle: FormHandle<PlanForm, &'static str> = FormHandle::new_with_error_type(PlanForm {
        plan: Plan::Starter,
    })
    .with_id_namespace("plans")
    .with_validation_mode(ValidationMode::on_change());
    let plan_path = PlanForm::fields().plan();
    let validation_runs = Rc::new(Cell::new(0));
    let captured_validation_runs = Rc::clone(&validation_runs);

    handle.write_advanced(|core| {
        core.observe(move |event| captured_events.borrow_mut().push(event.clone()));
        core.register_sync_field_validator_for_triggers(
            plan_path.clone(),
            "enterprise_requires_sales",
            ValidationTrigger::Change,
            move |value, context| {
                captured_validation_runs.set(captured_validation_runs.get() + 1);
                assert_eq!(context.trigger(), ValidationTrigger::Change);

                if *value == Plan::Enterprise {
                    vec!["contact_sales"]
                } else {
                    Vec::new()
                }
            },
        );
    });

    let plan = handle.select(plan_path.clone());

    assert_eq!(plan.name(), "plan");
    assert_eq!(plan.value(), Plan::Starter);
    assert!(plan.is_selected(&Plan::Starter));
    assert!(!plan.is_selected(&Plan::Pro));
    assert_eq!(plan.accessibility().input_id(), "plans-plan-input");
    assert_eq!(plan.accessibility().help_id(), "plans-plan-help");
    assert_eq!(plan.accessibility().error_id(), "plans-plan-error");
    assert!(!plan.accessibility().aria_invalid());
    assert!(!handle.is_field_touched(plan_path.clone()));
    assert!(!handle.is_field_blurred(plan_path.clone()));

    plan.on_change(Plan::Pro);

    assert_eq!(plan.value(), Plan::Pro);
    assert!(!plan.is_selected(&Plan::Starter));
    assert!(plan.is_selected(&Plan::Pro));
    assert_eq!(handle.field_value(plan_path.clone()), Plan::Pro);
    assert!(handle.is_field_touched(plan_path.clone()));
    assert!(!handle.is_field_blurred(plan_path.clone()));
    assert_eq!(validation_runs.get(), 1);
    assert!(handle.field_validation_errors(plan_path.clone()).is_empty());
    assert!(observer_events
        .borrow()
        .iter()
        .any(|event| matches!(event, FormObserverEvent::FieldUpdated { field, origin: FieldUpdateOrigin::User, .. } if field.field_name() == "plan")));

    plan.on_blur();

    assert!(handle.is_field_touched(plan_path.clone()));
    assert!(handle.is_field_blurred(plan_path.clone()));

    plan.select(Plan::Enterprise);

    assert_eq!(validation_runs.get(), 2);
    assert_eq!(
        handle.field_validation_errors(plan_path)[0].error(),
        &"contact_sales"
    );
    assert!(plan.accessibility().aria_invalid());
}

#[test]
fn dioxus_facade_rendered_select_binding_maps_native_select_values() {
    let observer_events = Rc::new(RefCell::new(Vec::new()));
    let captured_events = Rc::clone(&observer_events);
    let handle: FormHandle<PlanForm, &'static str> = FormHandle::new_with_error_type(PlanForm {
        plan: Plan::Starter,
    })
    .with_id_namespace("rendered-plans")
    .with_validation_mode(ValidationMode::on_change());
    let plan_path = PlanForm::fields().plan();
    let validation_runs = Rc::new(Cell::new(0));
    let captured_validation_runs = Rc::clone(&validation_runs);

    handle.write_advanced(|core| {
        core.observe(move |event| captured_events.borrow_mut().push(event.clone()));
        core.register_sync_field_validator_for_triggers(
            plan_path.clone(),
            "enterprise_requires_sales",
            ValidationTrigger::Change,
            move |value, context| {
                captured_validation_runs.set(captured_validation_runs.get() + 1);
                assert_eq!(context.trigger(), ValidationTrigger::Change);

                if *value == Plan::Enterprise {
                    vec!["contact_sales"]
                } else {
                    Vec::new()
                }
            },
        );
    });

    let plan = handle.select_with(plan_path.clone(), parse_plan, format_plan);

    assert_eq!(plan.name(), "plan");
    assert_eq!(plan.value(), "starter");
    assert_eq!(plan.typed_value(), Plan::Starter);
    assert!(plan.is_selected(&Plan::Starter));
    assert!(plan.is_rendered_selected("starter"));
    assert!(!plan.is_rendered_selected("pro"));
    assert_eq!(
        plan.accessibility().input_id(),
        "rendered%2dplans-plan-input"
    );

    plan.on_change("pro");

    assert_eq!(plan.value(), "pro");
    assert_eq!(plan.typed_value(), Plan::Pro);
    assert_eq!(handle.field_value(plan_path.clone()), Plan::Pro);
    assert!(handle.is_field_touched(plan_path.clone()));
    assert!(!handle.is_field_blurred(plan_path.clone()));
    assert_eq!(validation_runs.get(), 1);
    assert!(observer_events
        .borrow()
        .iter()
        .any(|event| matches!(event, FormObserverEvent::FieldUpdated { field, origin: FieldUpdateOrigin::User, .. } if field.field_name() == "plan")));

    assert_eq!(
        plan.try_on_change("missing"),
        Err("unknown plan".to_owned())
    );
    assert_eq!(handle.field_value(plan_path.clone()), Plan::Pro);
    assert_eq!(validation_runs.get(), 1);

    plan.try_on_change("enterprise")
        .expect("rendered option should parse");

    assert_eq!(plan.value(), "enterprise");
    assert_eq!(validation_runs.get(), 2);
    assert_eq!(
        handle.field_validation_errors(plan_path.clone())[0].error(),
        &"contact_sales"
    );

    plan.on_blur();

    assert!(handle.is_field_blurred(plan_path));
}

#[test]
fn dioxus_choice_selected_state_does_not_require_cloning_field_values() {
    let handle = FormHandle::new(NonCloneChoiceForm {
        choice: NonCloneChoice("starter"),
    });
    let choice_path = NonCloneChoiceForm::fields().choice();
    let select = handle.select(choice_path.clone());
    let radio = handle.radio_group(choice_path);

    assert!(select.is_selected(&NonCloneChoice("starter")));
    assert!(!select.is_selected(&NonCloneChoice("pro")));
    assert!(radio.is_selected(&NonCloneChoice("starter")));
    assert!(!radio.is_selected(&NonCloneChoice("pro")));

    select.on_change(NonCloneChoice("pro"));

    assert!(!select.is_selected(&NonCloneChoice("starter")));
    assert!(select.is_selected(&NonCloneChoice("pro")));
    assert!(radio.is_selected(&NonCloneChoice("pro")));
}

#[derive(Default)]
struct ChoiceHookProbe {
    handle: RefCell<Option<FormHandle<PlanForm>>>,
    typed_select_selected: Cell<bool>,
    rendered_select_value: RefCell<Option<String>>,
    radio_selected: Cell<bool>,
}

fn choice_hooks_probe(probe: Rc<ChoiceHookProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(PlanForm {
            plan: Plan::Starter,
        })
    });
    let plan_path = PlanForm::fields().plan();
    let typed_select = use_select(form.clone(), plan_path.clone());
    let rendered_select = use_select_with(form.clone(), plan_path.clone(), parse_plan, format_plan);
    let radio = use_radio_group(form.clone(), plan_path);

    probe
        .typed_select_selected
        .set(typed_select.is_selected(&Plan::Starter));
    probe
        .rendered_select_value
        .borrow_mut()
        .replace(rendered_select.value());
    probe.radio_selected.set(radio.is_selected(&Plan::Starter));
    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

#[test]
fn dioxus_choice_hooks_create_controlled_bindings() {
    let probe = Rc::new(ChoiceHookProbe::default());
    let mut dom = VirtualDom::new_with_props(choice_hooks_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert!(probe.typed_select_selected.get());
    assert_eq!(
        probe.rendered_select_value.borrow().as_deref(),
        Some("starter")
    );
    assert!(probe.radio_selected.get());
    assert_eq!(
        probe
            .handle
            .borrow()
            .as_ref()
            .expect("probe should expose its form handle")
            .field_value(PlanForm::fields().plan()),
        Plan::Starter
    );
}

#[test]
fn dioxus_multi_select_binding_selects_deselects_resets_and_submits_values() {
    let handle: FormHandle<MultiSelectForm, &'static str> =
        FormHandle::new_with_error_type(MultiSelectForm {
            topics: vec![Topic::Rust],
        })
        .with_id_namespace("multi-topics");
    let topics_path = MultiSelectForm::fields().topics();
    let topics = handle.multi_select(topics_path);

    assert_eq!(topics.name(), "topics");
    assert_eq!(
        topics.accessibility().input_id(),
        "multi%2dtopics-topics-input"
    );
    assert_eq!(topics.selected_values(), vec![Topic::Rust]);
    assert!(topics.is_selected(&Topic::Rust));
    assert!(!topics.is_selected(&Topic::Dioxus));
    assert!(!topics.is_dirty());

    let rust = topics
        .selected_item(&Topic::Rust)
        .expect("initial value should be selected");
    assert_eq!(rust.name(), "topics[0]");
    assert_eq!(rust.value(), Topic::Rust);
    assert!(!rust.is_dirty());
    assert!(!rust.is_touched());

    let dioxus = topics.option(Topic::Dioxus);

    assert_eq!(dioxus.name(), "topics");
    assert!(!dioxus.checked());

    dioxus.on_change(true);

    let selected_dioxus = dioxus
        .selected_item()
        .expect("user-selected value should have an item identity");
    assert!(selected_dioxus.field_identity().is_collection_item_value());
    assert_eq!(selected_dioxus.index(), 1);
    assert_eq!(selected_dioxus.name(), "topics[1]");
    assert_eq!(selected_dioxus.value(), Topic::Dioxus);
    assert!(selected_dioxus.is_touched());
    assert!(selected_dioxus.is_dirty());
    assert_eq!(handle.snapshot().topics, vec![Topic::Rust, Topic::Dioxus]);
    assert!(topics.is_dirty());

    topics.option(Topic::Rust).on_change(false);

    assert_eq!(topics.selected_values(), vec![Topic::Dioxus]);
    assert!(topics.selected_item(&Topic::Rust).is_none());

    let submitted_topics = Rc::new(RefCell::new(None));
    let captured_submitted_topics = Rc::clone(&submitted_topics);

    assert_eq!(
        handle.submit(move |submitted| {
            captured_submitted_topics
                .borrow_mut()
                .replace(submitted.value().topics.clone());
        }),
        SubmitResult::Succeeded
    );
    assert_eq!(
        submitted_topics.borrow().as_ref(),
        Some(&vec![Topic::Dioxus])
    );

    handle.reset();

    assert_eq!(topics.selected_values(), vec![Topic::Rust]);
    assert!(!topics.is_dirty());
    assert!(topics.selected_item(&Topic::Dioxus).is_none());

    handle.reinitialize(MultiSelectForm {
        topics: vec![Topic::Accessibility],
    });

    assert_eq!(topics.selected_values(), vec![Topic::Accessibility]);
    assert!(!topics.is_dirty());
    assert!(topics.selected_item(&Topic::Rust).is_none());
}

#[test]
fn dioxus_multi_select_item_validation_attaches_to_selected_value_identity() {
    let handle: FormHandle<MultiSelectForm, &'static str> =
        FormHandle::new_with_error_type(MultiSelectForm {
            topics: vec![Topic::Rust],
        })
        .with_validation_mode(ValidationMode::on_change());
    let topics = handle.multi_select(MultiSelectForm::fields().topics());
    let validator_fields = Rc::new(RefCell::new(Vec::new()));
    let captured_validator_fields = Rc::clone(&validator_fields);

    topics
        .item_validator("allowed_topic")
        .check(move |value, context| {
            captured_validator_fields
                .borrow_mut()
                .push(context.field_identity());

            if *value == Topic::Dioxus {
                vec!["topic_unavailable"]
            } else {
                Vec::new()
            }
        });

    topics.option(Topic::Dioxus).on_change(true);

    let dioxus = topics
        .selected_item(&Topic::Dioxus)
        .expect("selected value should expose its logical item");
    let dioxus_identity = dioxus.field_identity();

    assert_eq!(validator_fields.borrow().last(), Some(&dioxus_identity));
    assert_eq!(dioxus.validation_errors()[0].field(), Some(dioxus_identity));
    assert_eq!(dioxus.validation_errors()[0].error(), &"topic_unavailable");
    assert!(dioxus.visible_validation_errors().is_empty());

    topics.on_blur();

    assert!(dioxus.is_blurred());
    assert_eq!(
        dioxus.visible_validation_errors()[0].error(),
        &"topic_unavailable"
    );
    assert_eq!(
        handle.submit(|_submitted| ()),
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );

    topics.option(Topic::Dioxus).on_change(false);

    assert!(dioxus.validation_errors().is_empty());
    assert!(handle.validation_errors().is_empty());

    let submitted_topics = Rc::new(RefCell::new(None));
    let captured_submitted_topics = Rc::clone(&submitted_topics);

    assert_eq!(
        handle.submit(move |submitted| {
            captured_submitted_topics
                .borrow_mut()
                .replace(submitted.value().topics.clone());
        }),
        SubmitResult::Succeeded
    );
    assert_eq!(submitted_topics.borrow().as_ref(), Some(&vec![Topic::Rust]));
}

#[test]
fn dioxus_facade_radio_group_binding_updates_a_typed_field() {
    let observer_events = Rc::new(RefCell::new(Vec::new()));
    let captured_events = Rc::clone(&observer_events);
    let handle: FormHandle<PlanForm, &'static str> = FormHandle::new_with_error_type(PlanForm {
        plan: Plan::Starter,
    })
    .with_id_namespace("radio-plans")
    .with_validation_mode(ValidationMode::on_change());
    let plan_path = PlanForm::fields().plan();
    let validation_runs = Rc::new(Cell::new(0));
    let captured_validation_runs = Rc::clone(&validation_runs);

    handle.write_advanced(|core| {
        core.observe(move |event| captured_events.borrow_mut().push(event.clone()));
        core.register_sync_field_validator_for_triggers(
            plan_path.clone(),
            "enterprise_requires_sales",
            ValidationTrigger::Change,
            move |value, context| {
                captured_validation_runs.set(captured_validation_runs.get() + 1);
                assert_eq!(context.trigger(), ValidationTrigger::Change);

                if *value == Plan::Enterprise {
                    vec!["contact_sales"]
                } else {
                    Vec::new()
                }
            },
        );
    });

    let plan = handle.radio_group(plan_path.clone());

    assert_eq!(plan.name(), "plan");
    assert_eq!(plan.value(), Plan::Starter);
    assert!(plan.is_selected(&Plan::Starter));
    assert!(!plan.is_selected(&Plan::Pro));
    assert_eq!(plan.accessibility().input_id(), "radio%2dplans-plan-input");
    assert_eq!(plan.accessibility().help_id(), "radio%2dplans-plan-help");
    assert_eq!(plan.accessibility().error_id(), "radio%2dplans-plan-error");
    assert!(!plan.accessibility().aria_invalid());
    assert!(!handle.is_field_touched(plan_path.clone()));
    assert!(!handle.is_field_blurred(plan_path.clone()));

    plan.select(Plan::Pro);

    assert_eq!(plan.value(), Plan::Pro);
    assert!(!plan.is_selected(&Plan::Starter));
    assert!(plan.is_selected(&Plan::Pro));
    assert_eq!(handle.field_value(plan_path.clone()), Plan::Pro);
    assert!(handle.is_field_touched(plan_path.clone()));
    assert!(!handle.is_field_blurred(plan_path.clone()));
    assert_eq!(validation_runs.get(), 1);
    assert!(handle.field_validation_errors(plan_path.clone()).is_empty());
    assert!(observer_events
        .borrow()
        .iter()
        .any(|event| matches!(event, FormObserverEvent::FieldUpdated { field, origin: FieldUpdateOrigin::User, .. } if field.field_name() == "plan")));

    plan.on_blur();

    assert!(handle.is_field_touched(plan_path.clone()));
    assert!(handle.is_field_blurred(plan_path.clone()));

    plan.on_change(Plan::Enterprise);

    assert_eq!(validation_runs.get(), 2);
    assert_eq!(
        handle.field_validation_errors(plan_path)[0].error(),
        &"contact_sales"
    );
    assert!(plan.accessibility().aria_invalid());
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct InputValidationForm {
    title: String,
    biography: String,
    accepts_terms: bool,
}

#[test]
fn dioxus_controlled_helpers_run_configured_value_change_validation() {
    let title_runs = Rc::new(Cell::new(0));
    let biography_runs = Rc::new(Cell::new(0));
    let form_runs = Rc::new(Cell::new(0));
    let title_validator_runs = Rc::clone(&title_runs);
    let biography_validator_runs = Rc::clone(&biography_runs);
    let form_validator_runs = Rc::clone(&form_runs);
    let handle: FormHandle<InputValidationForm, &'static str> =
        FormHandle::new_with_error_type(InputValidationForm {
            title: "Profile".to_owned(),
            biography: String::new(),
            accepts_terms: false,
        })
        .with_validation_mode(ValidationMode::on_change());
    let fields = InputValidationForm::fields();
    let title_path = fields.title();
    let biography_path = fields.biography();
    let accepts_terms_path = fields.accepts_terms();

    let accepts_terms_for_validator = accepts_terms_path.clone();
    handle
        .field(title_path.clone())
        .validator("title_required")
        .on(ValidationTrigger::Change)
        .check(move |value, context| {
            title_validator_runs.set(title_validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);

            if value.is_empty() {
                vec!["title_required"]
            } else {
                Vec::new()
            }
        });
    handle
        .field(biography_path.clone())
        .validator("biography_length")
        .on(ValidationTrigger::Change)
        .check(move |value, context| {
            biography_validator_runs.set(biography_validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);

            if value.len() < 10 {
                vec!["biography_too_short"]
            } else {
                Vec::new()
            }
        });
    handle
        .validator("terms_required")
        .on(ValidationTrigger::Change)
        .check(move |context| {
            form_validator_runs.set(form_validator_runs.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);

            if context.form().accepts_terms {
                Vec::new()
            } else {
                vec![FormValidationError::field(
                    accepts_terms_for_validator.clone(),
                    "terms_required",
                )]
            }
        });

    handle.text(title_path.clone()).on_input("");

    assert_eq!(title_runs.get(), 1);
    assert_eq!(biography_runs.get(), 0);
    assert_eq!(form_runs.get(), 1);
    assert_eq!(
        handle.field_validation_errors(title_path.clone())[0].error(),
        &"title_required"
    );
    assert!(
        handle
            .visible_field_validation_errors(title_path.clone())
            .is_empty()
    );

    handle.text(title_path.clone()).on_blur();

    assert_eq!(
        handle.visible_field_validation_errors(title_path)[0].error(),
        &"title_required"
    );

    handle.textarea(biography_path.clone()).on_input("short");

    assert_eq!(title_runs.get(), 1);
    assert_eq!(biography_runs.get(), 1);
    assert_eq!(form_runs.get(), 2);
    assert_eq!(
        handle.field_validation_errors(biography_path)[0].error(),
        &"biography_too_short"
    );
    assert_eq!(
        handle.field_validation_errors(accepts_terms_path.clone())[0].error(),
        &"terms_required"
    );

    handle.checkbox(accepts_terms_path).on_change(true);

    assert_eq!(title_runs.get(), 1);
    assert_eq!(biography_runs.get(), 1);
    assert_eq!(form_runs.get(), 3);

    let errors: Vec<_> = handle
        .validation_errors()
        .into_iter()
        .map(|error| (error.source().as_str().to_owned(), *error.error()))
        .collect();
    assert_eq!(
        errors,
        vec![
            ("title_required".to_owned(), "title_required"),
            ("biography_length".to_owned(), "biography_too_short"),
        ]
    );
}

#[derive(Default)]
struct ValueChangeAsyncValidationProbe {
    gate: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    on_input: RefCell<Option<Box<InputHandler>>>,
    captured: RefCell<Option<(String, String)>>,
    snapshots: RefCell<Vec<AsyncFieldValidationSnapshot>>,
}

fn value_change_async_validation_probe(probe: Rc<ValueChangeAsyncValidationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "first@example.com".to_owned(),
                })
                .with_validation_mode(ValidationMode::on_change());
            let email = SignupForm::fields().email();
            let validation = probe.gate.clone();
            let captured_probe = Rc::clone(&probe);

            form.field(email.clone())
                .async_validator("availability")
                .on(ValidationTrigger::Change)
                .check(move |value, snapshot| {
                    captured_probe
                        .captured
                        .borrow_mut()
                        .replace((value, snapshot.value().email.clone()));
                    validation.future()
                });

            form
        }
    });
    let email = SignupForm::fields().email();
    let status = form
        .validation_status(email.clone(), "availability")
        .expect("async validator status should be readable");
    let error_count = form.field_validation_errors(email.clone()).len();
    let visible_error_count = form.visible_field_validation_errors(email.clone()).len();
    let can_submit = form.can_submit();
    let aria_invalid = form.field_accessibility(email.clone()).aria_invalid();
    let runtime = dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let on_input = {
        let runtime = Rc::clone(&runtime);
        let form = form.clone();
        let email = email.clone();

        move |value: String| runtime.in_scope(scope, || form.text(email.clone()).on_input(value))
    };

    probe.handle.borrow_mut().replace(form);
    probe.on_input.borrow_mut().replace(Box::new(on_input));
    probe
        .snapshots
        .borrow_mut()
        .push(AsyncFieldValidationSnapshot {
            status,
            error_count,
            visible_error_count,
            can_submit,
            aria_invalid,
        });

    VNode::empty()
}

#[test]
fn dioxus_value_change_policy_starts_registered_async_field_validation() {
    let probe = Rc::new(ValueChangeAsyncValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(value_change_async_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [AsyncFieldValidationSnapshot {
            status: ValidationStatus::Unknown,
            error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        }]
    );

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("taken@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.captured.borrow().as_ref(),
        Some(&(
            "taken@example.com".to_owned(),
            "taken@example.com".to_owned()
        ))
    );
    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFieldValidationSnapshot {
            status: ValidationStatus::Pending,
            error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        })
    );

    handle.mark_field_blurred(email);
    probe.gate.complete(vec!["email_unavailable"]);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFieldValidationSnapshot {
            status: ValidationStatus::Invalid,
            error_count: 1,
            visible_error_count: 1,
            can_submit: false,
            aria_invalid: true,
        })
    );
}

#[test]
fn dioxus_file_change_does_not_strand_pending_ordinary_async_field_validation() {
    let probe = Rc::new(ValueChangeAsyncValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(value_change_async_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("available@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(
        handle.validation_status(email.clone(), "availability"),
        Some(ValidationStatus::Pending)
    );

    handle
        .file(FileFieldKey::new("attachments"))
        .select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);
    probe.gate.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(
        handle.validation_status(email, "availability"),
        Some(ValidationStatus::Valid)
    );
    assert_eq!(handle.submit_availability().blockers(), &[]);
}

#[derive(Default)]
struct ManualAsyncValidationProbe {
    gate: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    captured: RefCell<Option<(String, String)>>,
    snapshots: RefCell<Vec<AsyncFieldValidationSnapshot>>,
}

fn manual_async_validation_probe(probe: Rc<ManualAsyncValidationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "manual@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();
            let validation = probe.gate.clone();
            let captured_probe = Rc::clone(&probe);

            form.field(email.clone())
                .async_validator("availability")
                .on(ValidationTrigger::Manual)
                .check(move |value, snapshot| {
                    captured_probe
                        .captured
                        .borrow_mut()
                        .replace((value, snapshot.value().email.clone()));
                    validation.future()
                });

            form
        }
    });
    let email = SignupForm::fields().email();

    let validation_email = email.clone();
    use_hook({
        let form = form.clone();

        move || form.validate_field(validation_email.clone(), ValidationTrigger::Manual)
    });

    let status = form
        .validation_status(email.clone(), "availability")
        .expect("async validator status should be readable");
    let error_count = form.field_validation_errors(email.clone()).len();
    let visible_error_count = form.visible_field_validation_errors(email.clone()).len();
    let can_submit = form.can_submit();
    let aria_invalid = form.field_accessibility(email).aria_invalid();

    probe.handle.borrow_mut().replace(form);
    probe
        .snapshots
        .borrow_mut()
        .push(AsyncFieldValidationSnapshot {
            status,
            error_count,
            visible_error_count,
            can_submit,
            aria_invalid,
        });

    VNode::empty()
}

#[derive(Default)]
struct RuntimeSyncRunProbe {
    field_gate: AsyncGate<Vec<&'static str>>,
    form_gate: AsyncGate<Vec<FormValidationError<&'static str>>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validate_all: RefCell<Option<Box<ActionHandler>>>,
    field_sync_runs: Cell<u32>,
    form_sync_runs: Cell<u32>,
}

fn runtime_sync_run_probe(probe: Rc<RuntimeSyncRunProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "manual@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();

            let sync_email = email.clone();
            form.write_advanced({
                let probe = Rc::clone(&probe);

                move |core| {
                    core.register_sync_field_validator_for_triggers(
                        sync_email.clone(),
                        "format",
                        ValidationTrigger::Manual,
                        move |_value, _context| {
                            probe.field_sync_runs.set(probe.field_sync_runs.get() + 1);
                            Vec::new()
                        },
                    );
                }
            });
            form.write_advanced({
                let probe = Rc::clone(&probe);

                move |core| {
                    core.register_sync_form_validator_for_triggers(
                        "account",
                        ValidationTrigger::Manual,
                        move |_context| {
                            probe.form_sync_runs.set(probe.form_sync_runs.get() + 1);
                            Vec::new()
                        },
                    );
                }
            });

            let field_gate = probe.field_gate.clone();
            form.field(email.clone())
                .async_validator("availability")
                .on(ValidationTrigger::Manual)
                .check(move |_value, _snapshot| field_gate.future());

            let form_gate = probe.form_gate.clone();
            form.async_validator("account_async")
                .on(ValidationTrigger::Manual)
                .check(move |_snapshot| form_gate.future());

            form
        }
    });

    let runtime = dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let validate_all = {
        let runtime = Rc::clone(&runtime);
        let form = form.clone();

        move || runtime.in_scope(scope, || form.validate_all(ValidationTrigger::Manual))
    };

    probe.handle.borrow_mut().replace(form);
    probe
        .validate_all
        .borrow_mut()
        .replace(Box::new(validate_all));

    VNode::empty()
}

#[derive(Default)]
struct DebounceBypassProbe {
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    run_non_value_change_validations: RefCell<Option<Box<ActionHandler>>>,
    delay_calls: Cell<u32>,
    validation_calls: Cell<u32>,
}

fn debounce_bypass_probe(probe: Rc<DebounceBypassProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "manual@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();

            form.field(email.clone())
                .async_validator("availability")
                .on(ValidationTriggers::new([
                    ValidationTrigger::Initial,
                    ValidationTrigger::Manual,
                    ValidationTrigger::Blur,
                    ValidationTrigger::Submit,
                ]))
                .debounce({
                    let probe = Rc::clone(&probe);

                    move || {
                        probe.delay_calls.set(probe.delay_calls.get() + 1);
                        async {}
                    }
                })
                .check({
                    let probe = Rc::clone(&probe);

                    move |_value, _snapshot| {
                        probe.validation_calls.set(probe.validation_calls.get() + 1);
                        async { Vec::<&'static str>::new() }
                    }
                });

            form
        }
    });

    let email = SignupForm::fields().email();
    let runtime = dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let run_non_value_change_validations = {
        let runtime = Rc::clone(&runtime);
        let form = form.clone();

        move || {
            runtime.in_scope(scope, || {
                form.validate_field(email.clone(), ValidationTrigger::Manual)
            });
            runtime.in_scope(scope, || form.mark_field_blurred(email.clone()));
            runtime.in_scope(scope, || {
                assert!(form.validate_initialization());
            });
            runtime.in_scope(scope, || {
                assert!(!form.validate_for_submit());
            });
        }
    };

    probe.handle.borrow_mut().replace(form);
    probe
        .run_non_value_change_validations
        .borrow_mut()
        .replace(Box::new(run_non_value_change_validations));

    VNode::empty()
}

#[test]
fn dioxus_manual_validation_starts_registered_async_field_validation() {
    let probe = Rc::new(ManualAsyncValidationProbe::default());
    let mut dom = VirtualDom::new_with_props(manual_async_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(
        probe.captured.borrow().as_ref(),
        Some(&(
            "manual@example.com".to_owned(),
            "manual@example.com".to_owned()
        ))
    );
    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [AsyncFieldValidationSnapshot {
            status: ValidationStatus::Pending,
            error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        }]
    );

    handle.mark_field_blurred(email);
    probe.gate.complete(vec!["email_unavailable"]);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFieldValidationSnapshot {
            status: ValidationStatus::Invalid,
            error_count: 1,
            visible_error_count: 1,
            can_submit: false,
            aria_invalid: true,
        })
    );
}

#[test]
fn dioxus_async_field_validator_accepts_non_send_future() {
    let probe = Rc::new(ManualAsyncValidationProbe::default());
    let mut dom = VirtualDom::new_with_props(manual_async_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFieldValidationSnapshot {
            status: ValidationStatus::Pending,
            error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        })
    );

    // AsyncGateFuture carries Rc<RefCell<_>>, so this exercises a non-Send validator future.
    probe.gate.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFieldValidationSnapshot {
            status: ValidationStatus::Valid,
            error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        })
    );
}

#[derive(Default)]
struct TargetedAsyncValidationProbe {
    field_gate: AsyncGate<Vec<&'static str>>,
    form_gate: AsyncGate<Vec<FormValidationError<&'static str>>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    form_validator_id: RefCell<Option<ValidatorId>>,
    field_status: RefCell<Option<ValidationStatus>>,
    form_status: RefCell<Option<ValidationStatus>>,
    captured_field: RefCell<Option<(String, String)>>,
    captured_form: RefCell<Option<String>>,
}

fn targeted_async_validation_probe(probe: Rc<TargetedAsyncValidationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "targeted@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();

            let field_gate = probe.field_gate.clone();
            let captured_field_probe = Rc::clone(&probe);
            form.field(email.clone())
                .async_validator("availability")
                .on(ValidationTrigger::Manual)
                .check(move |value, snapshot| {
                    captured_field_probe
                        .captured_field
                        .borrow_mut()
                        .replace((value, snapshot.value().email.clone()));
                    field_gate.future()
                });

            let form_gate = probe.form_gate.clone();
            let captured_form_probe = Rc::clone(&probe);
            let form_validator_id = form
                .async_validator("account")
                .on(ValidationTrigger::Manual)
                .check(move |snapshot| {
                    captured_form_probe
                        .captured_form
                        .borrow_mut()
                        .replace(snapshot.value().email.clone());
                    form_gate.future()
                });

            probe
                .form_validator_id
                .borrow_mut()
                .replace(form_validator_id);
            form
        }
    });
    let email = SignupForm::fields().email();
    let form_validator_id = probe
        .form_validator_id
        .borrow()
        .expect("probe should store form validator id");

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            probe.field_status.borrow_mut().replace(
                form.validate_field_source(email, "availability", ValidationTrigger::Manual)
                    .expect("targeted async field validation should start"),
            );
            probe.form_status.borrow_mut().replace(
                form.validate_form_validator(form_validator_id, ValidationTrigger::Manual)
                    .expect("targeted async form validation should start"),
            );
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

#[test]
fn dioxus_targeted_validation_apis_start_registered_async_validators() {
    let probe = Rc::new(TargetedAsyncValidationProbe::default());
    let mut dom = VirtualDom::new_with_props(targeted_async_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert_eq!(
        *probe.field_status.borrow(),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(*probe.form_status.borrow(), Some(ValidationStatus::Pending));
    assert_eq!(
        probe.captured_field.borrow().as_ref(),
        Some(&(
            "targeted@example.com".to_owned(),
            "targeted@example.com".to_owned()
        ))
    );
    assert_eq!(
        probe.captured_form.borrow().as_deref(),
        Some("targeted@example.com")
    );
}

#[test]
fn dioxus_runtime_async_validation_does_not_rerun_sync_validators() {
    let probe = Rc::new(RuntimeSyncRunProbe::default());
    let mut dom = VirtualDom::new_with_props(runtime_sync_run_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    {
        let validate_all = probe.validate_all.borrow();
        let validate_all = validate_all
            .as_ref()
            .expect("probe should expose validation action");
        validate_all();
    }

    assert_eq!(probe.field_sync_runs.get(), 1);
    assert_eq!(probe.form_sync_runs.get(), 1);
    assert_eq!(
        handle.validation_status(SignupForm::fields().email(), "availability"),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(
        handle.form_validation_status("account_async"),
        Some(ValidationStatus::Pending)
    );
}

#[test]
fn dioxus_debounced_async_validator_bypasses_delay_for_non_value_change_triggers() {
    let probe = Rc::new(DebounceBypassProbe::default());
    let mut dom = VirtualDom::new_with_props(debounce_bypass_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    {
        let run_validations = probe.run_non_value_change_validations.borrow();
        let run_validations = run_validations
            .as_ref()
            .expect("probe should expose validation action");
        run_validations();
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.delay_calls.get(), 0);
    assert_eq!(probe.validation_calls.get(), 4);
    assert_eq!(
        handle.validation_status(email, "availability"),
        Some(ValidationStatus::Valid)
    );
}

#[test]
fn dioxus_debounce_duration_provides_reusable_timer_delay_factory() {
    let delay = debounce_duration(Duration::from_millis(1));

    let _first = delay();
    let _second = delay();
}

#[test]
fn dioxus_managed_submit_prevents_native_submission_stops_propagation_and_passes_owned_snapshot() {
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        });
    let submit = handle.managed_submit();
    let event = managed_submit_event();
    let submitted = Rc::new(RefCell::new(None));
    let submitted_snapshot = Rc::clone(&submitted);

    let result = submit.on_submit(event.clone(), move |submitted| {
        submitted_snapshot
            .borrow_mut()
            .replace(submitted.into_value());
    });

    assert_eq!(result, SubmitResult::Succeeded);
    assert!(!event.default_action_enabled());
    assert!(!event.propagates());
    assert_eq!(
        submitted.borrow().as_ref(),
        Some(&SignupForm {
            email: "ada@example.com".to_owned()
        })
    );
    assert_eq!(handle.submit_attempt_count(), 1);
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Succeeded));
}

#[test]
fn dioxus_managed_submit_passes_explicit_submit_intent_to_validation_and_handler() {
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: String::new(),
        });
    let email = SignupForm::fields().email();

    let validator_email = email.clone();
    handle.write_advanced(|core| {
        core.register_sync_form_validator_for_triggers(
            "publish_email_required",
            ValidationTrigger::Submit,
            move |context| {
                if context.submit_intent::<SignupSubmitIntent>()
                    == Some(&SignupSubmitIntent::Publish)
                    && context.form().email.is_empty()
                {
                    vec![FormValidationError::field(
                        validator_email.clone(),
                        "email_required_for_publish",
                    )]
                } else {
                    Vec::new()
                }
            },
        );
    });

    let submit = handle.managed_submit();
    let draft_event = managed_submit_event();
    let draft_intent = Rc::new(RefCell::new(None));
    let draft_intent_for_handler = Rc::clone(&draft_intent);

    let draft_result = submit.intent(SignupSubmitIntent::SaveDraft).on_submit(
        draft_event.clone(),
        move |submitted| {
            draft_intent_for_handler
                .borrow_mut()
                .replace(*submitted.intent());
        },
    );

    assert_eq!(draft_result, SubmitResult::Succeeded);
    assert!(!draft_event.default_action_enabled());
    assert!(!draft_event.propagates());
    assert_eq!(
        draft_intent.borrow().as_ref(),
        Some(&SignupSubmitIntent::SaveDraft)
    );
    assert_eq!(
        handle.intent(SignupSubmitIntent::SaveDraft).last_status(),
        Some(SubmitStatus::Succeeded)
    );

    let publish_event = managed_submit_event();
    let publish_called = Cell::new(false);
    let publish_result = submit
        .intent(SignupSubmitIntent::Publish)
        .on_submit(publish_event.clone(), |_submitted| publish_called.set(true));

    assert_eq!(
        publish_result,
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(!publish_event.default_action_enabled());
    assert!(!publish_event.propagates());
    assert!(!publish_called.get());
    assert_eq!(
        handle.field_validation_errors(email.clone())[0].error(),
        &"email_required_for_publish"
    );
    assert_eq!(
        handle.intent(SignupSubmitIntent::Publish).last_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );
    assert_eq!(
        handle.intent(SignupSubmitIntent::SaveDraft).last_status(),
        None
    );
    let latest = handle
        .last_submit_status_as::<SignupSubmitIntent>()
        .expect("latest submit status should carry typed intent");
    assert_eq!(latest.intent(), &SignupSubmitIntent::Publish);
    assert_eq!(
        latest.status(),
        SubmitStatus::Blocked(SubmitBlocker::ValidationErrors)
    );
    let submit_latest = submit
        .last_submit_status_as::<SignupSubmitIntent>()
        .expect("submit binding should expose typed latest status");
    assert_eq!(submit_latest.intent(), &SignupSubmitIntent::Publish);
    assert_eq!(submit_latest.status(), latest.status());
    assert!(handle.intent(SignupSubmitIntent::SaveDraft).can_submit());
    assert!(!handle.intent(SignupSubmitIntent::Publish).can_submit());
    assert_eq!(
        handle.visible_field_validation_errors_for_intent(
            email.clone(),
            &SignupSubmitIntent::Publish
        )[0]
        .error(),
        &"email_required_for_publish"
    );
    assert!(
        handle
            .visible_field_validation_errors_for_intent(
                email.clone(),
                &SignupSubmitIntent::SaveDraft
            )
            .is_empty()
    );
    assert!(
        handle
            .intent(SignupSubmitIntent::Publish)
            .field_accessibility(email.clone())
            .has_visible_validation_errors()
    );
    assert!(
        !handle
            .intent(SignupSubmitIntent::SaveDraft)
            .field_accessibility(email)
            .has_visible_validation_errors()
    );
}

#[test]
fn dioxus_intent_submit_binding_passes_file_snapshot_to_handler() {
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        });
    let attachments_key = FileFieldKey::new("attachments");
    let attachments = handle.file(attachments_key.clone());
    let submit = handle.managed_submit().intent(SignupSubmitIntent::Publish);
    let event = managed_submit_event();
    let submitted_intent = Rc::new(RefCell::new(None));
    let submitted_files = Rc::new(RefCell::new(Vec::new()));
    let submitted_intent_for_handler = Rc::clone(&submitted_intent);
    let submitted_files_for_handler = Rc::clone(&submitted_files);

    attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

    let result = submit.on_submit_with_files(event.clone(), move |submitted, files| {
        submitted_intent_for_handler
            .borrow_mut()
            .replace(*submitted.intent());
        submitted_files_for_handler
            .borrow_mut()
            .extend(files.selected_files(&attachments_key));
    });

    assert_eq!(result, SubmitResult::Succeeded);
    assert!(!event.default_action_enabled());
    assert!(!event.propagates());
    assert_eq!(
        submitted_intent.borrow().as_ref(),
        Some(&SignupSubmitIntent::Publish)
    );
    assert_eq!(submitted_files.borrow()[0].name(), "resume.pdf");
}

#[test]
fn dioxus_managed_submit_records_structured_submit_errors() {
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: "taken@example.com".to_owned(),
        });
    let email = SignupForm::fields().email();
    let submit = handle.managed_submit();
    let event = managed_submit_event();

    let result = submit.on_submit(event.clone(), |_submitted| {
        SubmitError::field(email.clone(), "email_unavailable")
    });

    assert_eq!(result, SubmitResult::Rejected);
    assert!(!event.default_action_enabled());
    assert!(!event.propagates());

    let errors: Vec<_> = handle
        .field_validation_errors(email)
        .into_iter()
        .map(|error| {
            (
                error.field().unwrap().as_str().to_owned(),
                error.source().as_str().to_owned(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![("email".to_owned(), "submit".to_owned(), "email_unavailable")]
    );
}

#[test]
fn dioform_handle_selector_reads_form_errors_and_snapshots() {
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        });

    assert_eq!(
        handle.snapshot(),
        SignupForm {
            email: "ada@example.com".to_owned()
        }
    );

    let result = handle.submit(|_submitted| SubmitError::form("try_later"));

    assert_eq!(result, SubmitResult::Rejected);

    let form_errors: Vec<_> = handle
        .form_validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.target(),
                error.source().as_str().to_owned(),
                *error.error(),
            )
        })
        .collect();

    assert_eq!(
        form_errors,
        vec![(
            dioform::ValidationTarget::Form,
            "submit".to_owned(),
            "try_later"
        )]
    );
    assert_eq!(
        handle.visible_form_validation_errors()[0].error(),
        &"try_later"
    );
}

#[test]
fn dioxus_managed_submit_prevents_native_submission_and_blocks_invalid_drafts() {
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: String::new(),
        });
    let email = SignupForm::fields().email();

    handle.write_advanced(|core| {
        core.register_sync_field_validator(email, "required", |value, _context| {
            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        });
    });

    let submit = handle.managed_submit();
    let event = managed_submit_event();
    let called = Cell::new(false);

    let result = submit.on_submit(event.clone(), |_submitted| called.set(true));

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(!event.default_action_enabled());
    assert!(!event.propagates());
    assert!(!called.get());
    assert_eq!(handle.submit_attempt_count(), 1);
    assert!(!handle.can_submit());
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );
}

#[derive(Default)]
struct AsyncSubmitSuccessProbe {
    gate: AsyncGate<()>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    event: RefCell<Option<Event<()>>>,
    result: RefCell<Option<SubmitResult>>,
    submitted_snapshot: RefCell<Option<SignupForm>>,
    submitted_intent: RefCell<Option<SignupSubmitIntent>>,
}

fn async_submit_success_probe(probe: Rc<AsyncSubmitSuccessProbe>) -> Element {
    let form = use_form_handle(|| {
        let form: FormHandle<SignupForm, &'static str> =
            FormHandle::new_with_error_type(SignupForm {
                email: "ada@example.com".to_owned(),
            });
        let email = SignupForm::fields().email();

        form.write_advanced(|core| {
            core.register_sync_field_validator(email, "required", |value, _context| {
                if value.is_empty() {
                    vec!["required"]
                } else {
                    Vec::new()
                }
            });
        });

        assert_eq!(
            form.submit(|_submitted| SubmitError::form("try_later")),
            SubmitResult::Rejected
        );

        form
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let submit = form.managed_submit();
            let gate = probe.gate.clone();
            let submit_probe = Rc::clone(&probe);
            let event = managed_submit_event();
            let result = submit.intent(SignupSubmitIntent::Publish).on_submit_async(
                event.clone(),
                move |submitted| {
                    submit_probe
                        .submitted_intent
                        .borrow_mut()
                        .replace(*submitted.intent());
                    submit_probe
                        .submitted_snapshot
                        .borrow_mut()
                        .replace(submitted.value().clone());
                    gate.future()
                },
            );

            probe.event.borrow_mut().replace(event);
            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

#[derive(Default)]
struct FileAsyncSubmitProbe {
    gate: AsyncGate<()>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    result: RefCell<Option<SubmitResult>>,
    submitted_files: RefCell<Option<Vec<SelectedFile>>>,
}

#[derive(Default)]
struct PendingFileAsyncSubmitProbe {
    delay: AsyncGate<()>,
    validation: AsyncGate<Vec<FormValidationError<&'static str>>>,
    submit: AsyncGate<()>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validator_id: RefCell<Option<ValidatorId>>,
    result: RefCell<Option<SubmitResult>>,
    submitted_files: RefCell<Option<Vec<SelectedFile>>>,
    submit_calls: Cell<u32>,
}

#[derive(Default)]
struct InFlightFileSubmitErrorProbe {
    gate: AsyncGate<SubmitError<SignupForm, &'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    result: RefCell<Option<SubmitResult>>,
    submitted_files: RefCell<Option<Vec<SelectedFile>>>,
}

#[derive(Default)]
struct AsyncFileValidationSubmitProbe {
    validation: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    result: RefCell<Option<SubmitResult>>,
    captured_file_names: RefCell<Option<Vec<String>>>,
    submit_calls: Cell<u32>,
}

#[derive(Default)]
struct SyncSubmitAsyncFileValidationProbe {
    validation: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    result: RefCell<Option<SubmitResult>>,
    captured_file_names: RefCell<Option<Vec<String>>>,
    submit_calls: Cell<u32>,
}

#[derive(Default)]
struct AsyncFileUnrelatedChangeProbe {
    validation: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    captured_file_names: RefCell<Option<Vec<String>>>,
}

#[derive(Default)]
struct CompletedAsyncFileValidationProbe {
    validation: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    captured_file_names: RefCell<Option<Vec<String>>>,
}

#[derive(Default)]
struct ContextAwareAsyncFileValidationProbe {
    validation: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    captured_email: RefCell<Option<String>>,
}

#[derive(Default)]
struct FileSubmitStartedMutationProbe {
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    result: RefCell<Option<SubmitResult>>,
    submitted_files: RefCell<Option<Vec<SelectedFile>>>,
}

#[derive(Default)]
struct ManagedFileSubmitStartedMutationProbe {
    validation: AsyncGate<Vec<FormValidationError<&'static str>>>,
    submit: AsyncGate<()>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    result: RefCell<Option<SubmitResult>>,
    submitted_files: RefCell<Option<Vec<SelectedFile>>>,
}

#[derive(Default)]
struct FileCleanupProbe {
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
}

fn file_async_submit_probe(probe: Rc<FileAsyncSubmitProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let attachments_key = FileFieldKey::new("attachments");
            let attachments = form.file(attachments_key.clone());

            attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

            let gate = probe.gate.clone();
            let submit_probe = Rc::clone(&probe);
            let result = form.managed_submit().on_submit_async_with_files(
                managed_submit_event(),
                move |submitted, files| {
                    assert_eq!(submitted.value().email, "ada@example.com");
                    submit_probe
                        .submitted_files
                        .borrow_mut()
                        .replace(files.selected_files(&attachments_key));
                    attachments.select_files([SelectedFileMetadata::new("portfolio.zip", 4_096)]);
                    gate.future()
                },
            );

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn file_submit_started_mutation_probe(probe: Rc<FileSubmitStartedMutationProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        })
    });

    use_submit_listener(form.clone(), move |context| {
        if context.event() == SubmitListenerEvent::SubmissionStarted {
            context
                .form()
                .file(FileFieldKey::new("attachments"))
                .select_files([SelectedFileMetadata::new("listener.zip", 2_048)]);
        }
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let attachments_key = FileFieldKey::new("attachments");

            form.file(attachments_key.clone())
                .select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

            let submit_probe = Rc::clone(&probe);
            let result = form.submit_with_files(move |_submitted, files| {
                submit_probe
                    .submitted_files
                    .borrow_mut()
                    .replace(files.selected_files(&attachments_key));
            });

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn managed_file_submit_started_mutation_probe(
    probe: Rc<ManagedFileSubmitStartedMutationProbe>,
) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let validation = probe.validation.clone();

            form.async_validator("account")
                .on(ValidationTrigger::Submit)
                .check(move |_snapshot| validation.future());

            form
        }
    });

    use_submit_listener(form.clone(), move |context| {
        if context.event() == SubmitListenerEvent::SubmissionStarted {
            context
                .form()
                .file(FileFieldKey::new("attachments"))
                .select_files([SelectedFileMetadata::new("listener.zip", 2_048)]);
        }
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let attachments_key = FileFieldKey::new("attachments");

            form.file(attachments_key.clone())
                .select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

            let submit = probe.submit.clone();
            let submit_probe = Rc::clone(&probe);
            let result = form.managed_submit().on_submit_async_with_files(
                managed_submit_event(),
                move |_submitted, files| {
                    submit_probe
                        .submitted_files
                        .borrow_mut()
                        .replace(files.selected_files(&attachments_key));
                    submit.future()
                },
            );

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn file_cleanup_probe(probe: Rc<FileCleanupProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();

        move || {
            form.file(FileFieldKey::new("attachments"))
                .select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn pending_file_async_submit_probe(probe: Rc<PendingFileAsyncSubmitProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let validator_id = form.write_advanced(|core| {
                core.register_async_form_validator_for_triggers(
                    "account",
                    ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
                )
            });

            probe.validator_id.borrow_mut().replace(validator_id);
            form
        }
    });
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let delay = probe.delay.future();
            let validation = probe.validation.clone();

            form.validate_async_form_validator_with_debounce(
                validator_id,
                ValidationTrigger::Change,
                delay,
                move |_snapshot| validation.future(),
            );
        }
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let attachments_key = FileFieldKey::new("attachments");
            let attachments = form.file(attachments_key.clone());

            attachments.select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

            let submit = probe.submit.clone();
            let submit_probe = Rc::clone(&probe);
            let result = form.managed_submit().on_submit_async_with_files(
                managed_submit_event(),
                move |_submitted, files| {
                    submit_probe
                        .submit_calls
                        .set(submit_probe.submit_calls.get() + 1);
                    submit_probe
                        .submitted_files
                        .borrow_mut()
                        .replace(files.selected_files(&attachments_key));
                    submit.future()
                },
            );

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn in_flight_file_submit_error_probe(probe: Rc<InFlightFileSubmitErrorProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let attachments_key = FileFieldKey::new("attachments");

            form.file(attachments_key.clone())
                .select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

            let gate = probe.gate.clone();
            let submit_probe = Rc::clone(&probe);
            let result = form.managed_submit().on_submit_async_with_files(
                managed_submit_event(),
                move |_submitted, files| {
                    submit_probe
                        .submitted_files
                        .borrow_mut()
                        .replace(files.selected_files(&attachments_key));
                    gate.future()
                },
            );

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn async_file_validation_submit_probe(probe: Rc<AsyncFileValidationSubmitProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let attachments_key = FileFieldKey::new("attachments");
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.file(attachments_key.clone())
                .async_validator("virus_scan")
                .on(ValidationTrigger::Submit)
                .check(move |files| {
                    captured_probe.captured_file_names.borrow_mut().replace(
                        files
                            .selected_files(&attachments_key)
                            .into_iter()
                            .map(|file| file.name().to_owned())
                            .collect(),
                    );
                    validation.future()
                });

            form
        }
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let attachments_key = FileFieldKey::new("attachments");

            form.file(attachments_key)
                .select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

            let submit_probe = Rc::clone(&probe);
            let result = form.managed_submit().on_submit_async_with_files(
                managed_submit_event(),
                move |_submitted, _files| {
                    submit_probe
                        .submit_calls
                        .set(submit_probe.submit_calls.get() + 1);
                    async {}
                },
            );

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn sync_submit_async_file_validation_probe(
    probe: Rc<SyncSubmitAsyncFileValidationProbe>,
) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let attachments_key = FileFieldKey::new("attachments");
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.file(attachments_key.clone())
                .async_validator("virus_scan")
                .on(ValidationTrigger::Submit)
                .check(move |files| {
                    captured_probe.captured_file_names.borrow_mut().replace(
                        files
                            .selected_files(&attachments_key)
                            .into_iter()
                            .map(|file| file.name().to_owned())
                            .collect(),
                    );
                    validation.future()
                });

            form
        }
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let attachments_key = FileFieldKey::new("attachments");

            form.file(attachments_key)
                .select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);

            let submit_probe = Rc::clone(&probe);
            let result = form.submit_with_files(move |_submitted, _files| {
                submit_probe
                    .submit_calls
                    .set(submit_probe.submit_calls.get() + 1);
            });

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn async_file_unrelated_change_probe(probe: Rc<AsyncFileUnrelatedChangeProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> = FormHandle::from_config(
                FormConfig::new(SignupForm {
                    email: String::new(),
                })
                .validation_mode(ValidationMode::on_change()),
            );
            let attachments_key = FileFieldKey::new("attachments");
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.file(attachments_key.clone())
                .async_validator("virus_scan")
                .on(ValidationTrigger::Change)
                .check(move |files| {
                    captured_probe.captured_file_names.borrow_mut().replace(
                        files
                            .selected_files(&attachments_key)
                            .into_iter()
                            .map(|file| file.name().to_owned())
                            .collect(),
                    );
                    validation.future()
                });

            form
        }
    });

    use_hook({
        let form = form.clone();

        move || {
            form.text(SignupForm::fields().email())
                .on_input("ada@example.com");
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn completed_async_file_validation_probe(probe: Rc<CompletedAsyncFileValidationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> = FormHandle::from_config(
                FormConfig::new(SignupForm {
                    email: String::new(),
                })
                .validation_mode(ValidationMode::on_change()),
            );
            let attachments_key = FileFieldKey::new("attachments");
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.file(attachments_key.clone())
                .async_validator("virus_scan")
                .on(ValidationTrigger::Change)
                .check(move |files| {
                    captured_probe.captured_file_names.borrow_mut().replace(
                        files
                            .selected_files(&attachments_key)
                            .into_iter()
                            .map(|file| file.name().to_owned())
                            .collect(),
                    );
                    validation.future()
                });

            form
        }
    });

    use_hook({
        let form = form.clone();

        move || {
            form.file(FileFieldKey::new("attachments"))
                .select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

fn context_aware_async_file_validation_probe(
    probe: Rc<ContextAwareAsyncFileValidationProbe>,
) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> = FormHandle::from_config(
                FormConfig::new(SignupForm {
                    email: "before@example.com".to_owned(),
                })
                .validation_mode(ValidationMode::on_change()),
            );
            let attachments_key = FileFieldKey::new("attachments");
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.file(attachments_key)
                .async_validator("virus_scan")
                .on(ValidationTrigger::Change)
                .check_with_context(move |_files, context| {
                    captured_probe
                        .captured_email
                        .borrow_mut()
                        .replace(context.value().email.clone());
                    validation.future()
                });

            form
        }
    });

    use_hook({
        let form = form.clone();

        move || {
            form.file(FileFieldKey::new("attachments"))
                .select_files([SelectedFileMetadata::new("resume.pdf", 1_024)]);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

#[test]
fn dioxus_managed_async_submit_blocks_duplicates_allows_draft_edits_and_completes_success() {
    let probe = Rc::new(AsyncSubmitSuccessProbe::default());
    let mut dom = VirtualDom::new_with_props(async_submit_success_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    let event = probe.event.borrow();
    let event = event.as_ref().expect("probe should expose submit event");
    assert!(!event.default_action_enabled());
    assert!(!event.propagates());
    assert!(handle.is_submitting());
    assert_eq!(handle.submit_attempt_count(), 2);
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Rejected));
    assert!(handle.validation_errors().is_empty());

    assert_eq!(
        handle
            .managed_submit()
            .on_submit_async(managed_submit_event(), |_submitted| async {}),
        SubmitResult::Blocked(SubmitBlocker::InFlightSubmission)
    );
    assert_eq!(handle.submit_attempt_count(), 2);
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission))
    );

    handle.text(email.clone()).on_input("lin@example.com");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.submitted_snapshot.borrow().as_ref(),
        Some(&SignupForm {
            email: "ada@example.com".to_owned()
        })
    );
    assert_eq!(
        probe.submitted_intent.borrow().as_ref(),
        Some(&SignupSubmitIntent::Publish)
    );
    assert_eq!(handle.field_value(email.clone()), "lin@example.com");
    assert!(handle.is_submitting());

    probe.gate.complete(());
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Succeeded));
    assert!(handle.validation_errors().is_empty());
    assert_eq!(handle.field_value(email), "lin@example.com");
    assert_eq!(
        handle.read_core(|core| core.draft().baseline().clone()),
        SignupForm {
            email: "ada@example.com".to_owned()
        }
    );
    assert!(handle.is_dirty());
}

#[test]
fn dioxus_managed_async_submit_passes_a_frozen_file_selection_snapshot() {
    let probe = Rc::new(FileAsyncSubmitProbe::default());
    let mut dom = VirtualDom::new_with_props(file_async_submit_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let live_attachments = handle.file(FileFieldKey::new("attachments"));
    let submitted_files = probe.submitted_files.borrow();
    let submitted_files = submitted_files
        .as_ref()
        .expect("submit handler should receive the file snapshot");

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    assert_eq!(submitted_files.len(), 1);
    assert_eq!(submitted_files[0].name(), "resume.pdf");
    assert_eq!(live_attachments.selected_files()[0].name(), "portfolio.zip");
    assert!(handle.is_submitting());

    probe.gate.complete(());
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Succeeded));
}

#[test]
fn dioxus_sync_submit_with_files_freezes_snapshot_before_submit_started_listeners() {
    let probe = Rc::new(FileSubmitStartedMutationProbe::default());
    let mut dom = VirtualDom::new_with_props(file_submit_started_mutation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let live_attachments = handle.file(FileFieldKey::new("attachments"));
    let submitted_files = probe.submitted_files.borrow();
    let submitted_files = submitted_files
        .as_ref()
        .expect("submit handler should receive the file snapshot");

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Succeeded));
    assert_eq!(submitted_files.len(), 1);
    assert_eq!(submitted_files[0].name(), "resume.pdf");
    assert_eq!(live_attachments.selected_files()[0].name(), "listener.zip");
}

#[test]
fn dioxus_managed_async_submit_with_files_freezes_snapshot_before_submit_started_listeners() {
    let probe = Rc::new(ManagedFileSubmitStartedMutationProbe::default());
    let mut dom = VirtualDom::new_with_props(
        managed_file_submit_started_mutation_probe,
        Rc::clone(&probe),
    );

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    assert!(probe.submitted_files.borrow().is_none());

    probe.validation.complete(Vec::new());
    dom.render_immediate_to_vec();

    let submitted_files = probe.submitted_files.borrow();
    let submitted_files = submitted_files
        .as_ref()
        .expect("submit handler should receive the file snapshot");
    let live_attachments = handle.file(FileFieldKey::new("attachments"));

    assert_eq!(submitted_files.len(), 1);
    assert_eq!(submitted_files[0].name(), "resume.pdf");
    assert_eq!(live_attachments.selected_files()[0].name(), "listener.zip");
    assert!(handle.is_submitting());

    probe.submit.complete(());
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Succeeded));
}

#[test]
fn dioxus_managed_async_submit_does_not_submit_stale_files_after_selection_change() {
    let probe = Rc::new(PendingFileAsyncSubmitProbe::default());
    let mut dom = VirtualDom::new_with_props(pending_file_async_submit_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let attachments = handle.file(FileFieldKey::new("attachments"));

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    assert_eq!(probe.submit_calls.get(), 0);
    assert!(handle.is_submitting());

    attachments.select_files([SelectedFileMetadata::new("portfolio.zip", 4_096)]);
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
    assert!(!handle.is_submitting());
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
    );

    probe.validation.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
    assert!(probe.submitted_files.borrow().is_none());
    assert_eq!(attachments.selected_files()[0].name(), "portfolio.zip");
}

#[test]
fn dioxus_managed_async_submit_discards_stale_file_error_after_selection_change() {
    let probe = Rc::new(InFlightFileSubmitErrorProbe::default());
    let mut dom = VirtualDom::new_with_props(in_flight_file_submit_error_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let attachments_key = FileFieldKey::new("attachments");
    let attachments = handle.file(attachments_key.clone());

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());
    assert_eq!(
        probe
            .submitted_files
            .borrow()
            .as_ref()
            .expect("submit handler should receive files")[0]
            .name(),
        "resume.pdf"
    );

    attachments.select_files([SelectedFileMetadata::new("portfolio.zip", 4_096)]);
    dom.render_immediate_to_vec();

    probe.gate.complete(SubmitError::field_identity(
        attachments_key.identity(),
        "upload_failed",
    ));
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Rejected));
    assert!(attachments.validation_errors().is_empty());
    assert_eq!(attachments.selected_files()[0].name(), "portfolio.zip");
}

#[test]
fn dioxus_async_file_validator_receives_snapshot_and_blocks_submit_while_pending() {
    let probe = Rc::new(AsyncFileValidationSubmitProbe::default());
    let mut dom = VirtualDom::new_with_props(async_file_validation_submit_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let attachments = handle.file(FileFieldKey::new("attachments"));

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());
    assert_eq!(probe.submit_calls.get(), 0);

    dom.render_immediate_to_vec();

    assert_eq!(
        probe.captured_file_names.borrow().as_ref(),
        Some(&vec!["resume.pdf".to_owned()])
    );
    assert_eq!(probe.submit_calls.get(), 0);

    probe.validation.complete(vec!["file_rejected"]);
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert_eq!(probe.submit_calls.get(), 0);
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );
    assert_eq!(attachments.validation_errors()[0].error(), &"file_rejected");
}

#[test]
fn dioxus_sync_submit_with_files_blocks_pending_async_file_validation() {
    let probe = Rc::new(SyncSubmitAsyncFileValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(sync_submit_async_file_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(
        *probe.result.borrow(),
        Some(SubmitResult::Blocked(SubmitBlocker::PendingValidation))
    );
    assert_eq!(
        probe.captured_file_names.borrow().as_ref(),
        Some(&vec!["resume.pdf".to_owned()])
    );
    assert_eq!(probe.submit_calls.get(), 0);
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
    );
    assert_eq!(
        handle.submit_availability().blockers(),
        &[SubmitBlocker::PendingValidation]
    );
}

#[test]
fn dioxus_async_file_validator_does_not_run_on_unrelated_field_change() {
    let probe = Rc::new(AsyncFileUnrelatedChangeProbe::default());
    let mut dom = VirtualDom::new_with_props(async_file_unrelated_change_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert!(probe.captured_file_names.borrow().is_none());
    assert_eq!(handle.submit_availability().blockers(), &[]);
}

#[test]
fn dioxus_async_file_validator_keeps_completed_error_after_unrelated_field_change() {
    let probe = Rc::new(CompletedAsyncFileValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(completed_async_file_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let attachments = handle.file(FileFieldKey::new("attachments"));

    assert_eq!(
        probe.captured_file_names.borrow().as_ref(),
        Some(&vec!["resume.pdf".to_owned()])
    );

    probe.validation.complete(vec!["file_rejected"]);
    dom.render_immediate_to_vec();

    assert_eq!(attachments.validation_errors()[0].error(), &"file_rejected");
    assert_eq!(
        handle.submit_availability().blockers(),
        &[SubmitBlocker::ValidationErrors]
    );

    handle
        .text(SignupForm::fields().email())
        .on_input("ada@example.com");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.captured_file_names.borrow().as_ref(),
        Some(&vec!["resume.pdf".to_owned()])
    );
    assert_eq!(attachments.validation_errors()[0].error(), &"file_rejected");
    assert_eq!(
        handle.submit_availability().blockers(),
        &[SubmitBlocker::ValidationErrors]
    );
}

#[test]
fn dioxus_async_file_validator_completion_survives_unrelated_field_change() {
    let probe = Rc::new(CompletedAsyncFileValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(completed_async_file_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let attachments = handle.file(FileFieldKey::new("attachments"));

    assert_eq!(
        probe.captured_file_names.borrow().as_ref(),
        Some(&vec!["resume.pdf".to_owned()])
    );
    handle
        .text(SignupForm::fields().email())
        .on_input("ada@example.com");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.captured_file_names.borrow().as_ref(),
        Some(&vec!["resume.pdf".to_owned()])
    );
    probe.validation.complete(vec!["file_rejected"]);
    dom.render_immediate_to_vec();

    assert_eq!(attachments.validation_errors()[0].error(), &"file_rejected");
    assert_eq!(
        handle.submit_availability().blockers(),
        &[SubmitBlocker::ValidationErrors]
    );
}

#[test]
fn dioxus_context_aware_async_file_validator_discards_stale_result_after_model_change() {
    let probe = Rc::new(ContextAwareAsyncFileValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(context_aware_async_file_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let attachments = handle.file(FileFieldKey::new("attachments"));

    assert_eq!(
        probe.captured_email.borrow().as_deref(),
        Some("before@example.com")
    );

    handle
        .text(SignupForm::fields().email())
        .on_input("after@example.com");
    dom.render_immediate_to_vec();

    probe.validation.complete(vec!["file_rejected"]);
    dom.render_immediate_to_vec();

    assert!(attachments.validation_errors().is_empty());
    assert_eq!(handle.submit_availability().blockers(), &[]);
}

#[test]
fn dioxus_cleanup_clears_adapter_owned_file_selections() {
    let probe = Rc::new(FileCleanupProbe::default());
    let mut dom = VirtualDom::new_with_props(file_cleanup_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let attachments = handle.file(FileFieldKey::new("attachments"));

    assert_eq!(attachments.selected_files()[0].name(), "resume.pdf");

    drop(dom);

    assert!(attachments.selected_files().is_empty());
}

#[test]
fn dioxus_restored_form_ignores_pre_restore_async_submit_completion() {
    let probe = Rc::new(AsyncSubmitSuccessProbe::default());
    let mut dom = VirtualDom::new_with_props(async_submit_success_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let snapshot_handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: "restored@example.com".to_owned(),
        });
    let snapshot = snapshot_handle.state_snapshot();

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());

    handle
        .restore_state_snapshot(snapshot)
        .expect("compatible snapshot should restore");

    assert!(!handle.is_submitting());
    assert_eq!(
        handle.field_value(SignupForm::fields().email()),
        "restored@example.com"
    );

    assert!(matches!(
        handle.begin_submission(),
        SubmitAttempt::Started(_)
    ));
    assert!(handle.is_submitting());

    probe.gate.complete(());
    dom.render_immediate_to_vec();

    assert!(handle.is_submitting());
    assert_eq!(handle.last_submit_status(), None);
}

#[test]
fn dioxus_managed_async_submit_blocks_invalid_drafts_before_spawning_handler() {
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: String::new(),
        });
    let email = SignupForm::fields().email();

    handle.write_advanced(|core| {
        core.register_sync_field_validator(email.clone(), "required", |value, _context| {
            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        });
    });

    let submit = handle.managed_submit();
    let event = managed_submit_event();
    let called = Rc::new(Cell::new(false));
    let called_by_submit = Rc::clone(&called);

    let result = submit.on_submit_async(event.clone(), move |_submitted| {
        called_by_submit.set(true);
        async {}
    });

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(!event.default_action_enabled());
    assert!(!event.propagates());
    assert!(!called.get());
    assert_eq!(handle.submit_attempt_count(), 1);
    assert!(!handle.is_submitting());
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );
    assert_eq!(
        handle.visible_field_validation_errors(email)[0].error(),
        &"required"
    );
}

#[test]
fn dioxus_managed_async_submit_blocks_unmanaged_async_validation_without_waiting() {
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: "ada@example.com".to_owned(),
        });
    let email = SignupForm::fields().email();

    let availability = handle.write_advanced(|core| {
        core.register_async_field_validator_for_triggers(
            email.clone(),
            "availability",
            ValidationTrigger::Submit,
        )
    });

    let submit = handle.managed_submit();
    let event = managed_submit_event();
    let called = Rc::new(Cell::new(false));
    let called_by_submit = Rc::clone(&called);

    let result = submit.on_submit_async(event.clone(), move |_submitted| {
        called_by_submit.set(true);
        async {}
    });

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert!(!event.default_action_enabled());
    assert!(!event.propagates());
    assert!(!called.get());
    assert_eq!(handle.submit_attempt_count(), 1);
    assert!(!handle.is_submitting());
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
    );
    assert_eq!(
        handle.field_validation_status(email.clone(), availability),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(
        handle.submit_availability().blockers(),
        &[SubmitBlocker::PendingValidation]
    );
}

#[derive(Default)]
struct SyncSubmitStartsValidationProbe {
    validation: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    duplicate_submit: RefCell<Option<Box<dyn Fn() -> SubmitResult>>>,
    submit_result: RefCell<Option<SubmitResult>>,
    validation_snapshot: RefCell<Option<(String, String)>>,
    validation_calls: Cell<u32>,
    submit_calls: Cell<u32>,
    duplicate_submit_calls: Cell<u32>,
}

fn sync_submit_starts_validation_probe(probe: Rc<SyncSubmitStartsValidationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.field(email.clone())
                .async_validator("availability")
                .on(ValidationTrigger::Submit)
                .check(move |value, snapshot| {
                    captured_probe
                        .validation_calls
                        .set(captured_probe.validation_calls.get() + 1);
                    captured_probe
                        .validation_snapshot
                        .borrow_mut()
                        .replace((value, snapshot.value().email.clone()));
                    validation.future()
                });

            form
        }
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let submit_probe = Rc::clone(&probe);
            let result =
                form.managed_submit()
                    .on_submit(managed_submit_event(), move |_submitted| {
                        submit_probe
                            .submit_calls
                            .set(submit_probe.submit_calls.get() + 1);
                    });

            probe.submit_result.borrow_mut().replace(result);
        }
    });

    let runtime = dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let duplicate_submit = {
        let runtime = Rc::clone(&runtime);
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let submit_probe = Rc::clone(&probe);

            runtime.in_scope(scope, || {
                form.managed_submit()
                    .on_submit(managed_submit_event(), move |_submitted| {
                        submit_probe
                            .duplicate_submit_calls
                            .set(submit_probe.duplicate_submit_calls.get() + 1);
                    })
            })
        }
    };

    probe.handle.borrow_mut().replace(form);
    probe
        .duplicate_submit
        .borrow_mut()
        .replace(Box::new(duplicate_submit));
    VNode::empty()
}

#[test]
fn dioxus_sync_submit_starts_registered_async_validation_before_blocking() {
    let probe = Rc::new(SyncSubmitStartsValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(sync_submit_starts_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(
        *probe.submit_result.borrow(),
        Some(SubmitResult::Blocked(SubmitBlocker::PendingValidation))
    );
    assert_eq!(probe.validation_calls.get(), 1);
    assert_eq!(probe.submit_calls.get(), 0);
    assert_eq!(
        probe.validation_snapshot.borrow().as_ref(),
        Some(&("ada@example.com".to_owned(), "ada@example.com".to_owned()))
    );
    assert_eq!(
        handle.validation_status(email.clone(), "availability"),
        Some(ValidationStatus::Pending)
    );

    probe.validation.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(
        handle.validation_status(email, "availability"),
        Some(ValidationStatus::Valid)
    );

    let submit_calls = Rc::new(Cell::new(0));
    let submit_counter = Rc::clone(&submit_calls);
    let result = handle
        .managed_submit()
        .on_submit(managed_submit_event(), move |_submitted| {
            submit_counter.set(submit_counter.get() + 1);
        });

    assert_eq!(result, SubmitResult::Succeeded);
    assert_eq!(submit_calls.get(), 1);
}

#[test]
fn dioxus_sync_submit_does_not_restart_pending_submit_validation_on_duplicate_click() {
    let probe = Rc::new(SyncSubmitStartsValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(sync_submit_starts_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert_eq!(
        *probe.submit_result.borrow(),
        Some(SubmitResult::Blocked(SubmitBlocker::PendingValidation))
    );
    assert_eq!(probe.validation_calls.get(), 1);

    let duplicate_result = {
        let duplicate_submit = probe.duplicate_submit.borrow();
        let duplicate_submit = duplicate_submit
            .as_ref()
            .expect("probe should expose duplicate submit action");
        duplicate_submit()
    };

    dom.render_immediate_to_vec();

    assert_eq!(
        duplicate_result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert_eq!(probe.validation_calls.get(), 1);
    assert_eq!(probe.submit_calls.get(), 0);
    assert_eq!(probe.duplicate_submit_calls.get(), 0);
}

#[derive(Default)]
struct ManagedAsyncSubmitValidationProbe {
    delay: AsyncGate<()>,
    validation: AsyncGate<Vec<&'static str>>,
    submit: AsyncGate<()>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validator_id: RefCell<Option<ValidatorId>>,
    submit_result: RefCell<Option<SubmitResult>>,
    validation_snapshot: RefCell<Option<(String, String)>>,
    submitted_snapshot: RefCell<Option<SignupForm>>,
    submit_calls: Cell<u32>,
    events: RefCell<Vec<SubmitListenerEvent>>,
}

fn managed_async_submit_validation_probe(probe: Rc<ManagedAsyncSubmitValidationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();
            let validator_id = form.write_advanced(|core| {
                core.register_async_field_validator_for_triggers(
                    email,
                    "availability",
                    ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
                )
            });

            probe.validator_id.borrow_mut().replace(validator_id);
            form
        }
    });
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    let listener_probe = Rc::clone(&probe);
    use_submit_listener(form.clone(), move |context| {
        listener_probe.events.borrow_mut().push(context.event());
    });

    let validation_email = email.clone();
    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let delay = probe.delay.future();
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.validate_async_field_validator_with_debounce(
                validation_email.clone(),
                validator_id,
                ValidationTrigger::Change,
                delay,
                move |value, snapshot| {
                    captured_probe
                        .validation_snapshot
                        .borrow_mut()
                        .replace((value, snapshot.value().email.clone()));
                    validation.future()
                },
            );
        }
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let submit = probe.submit.clone();
            let submit_probe = Rc::clone(&probe);
            let result =
                form.managed_submit()
                    .on_submit_async(managed_submit_event(), move |submitted| {
                        submit_probe
                            .submit_calls
                            .set(submit_probe.submit_calls.get() + 1);
                        submit_probe
                            .submitted_snapshot
                            .borrow_mut()
                            .replace(submitted.value().clone());
                        submit.future()
                    });

            probe.submit_result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

#[cfg(feature = "serde")]
#[derive(Default)]
struct InvalidRestoreAtomicityProbe {
    delay: AsyncGate<()>,
    validation: AsyncGate<Vec<String>>,
    handle: RefCell<Option<FormHandle<SignupForm, String>>>,
    validator_id: RefCell<Option<ValidatorId>>,
    submit_result: RefCell<Option<SubmitResult>>,
}

#[cfg(feature = "serde")]
fn invalid_restore_atomicity_probe(probe: Rc<InvalidRestoreAtomicityProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, String> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();
            let validator_id = form.write_advanced(|core| {
                core.register_async_field_validator_for_triggers(
                    email,
                    "availability",
                    ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
                )
            });

            probe.validator_id.borrow_mut().replace(validator_id);
            form
        }
    });
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    let validation_email = email.clone();
    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let delay = probe.delay.future();
            let validation = probe.validation.clone();

            form.validate_async_field_validator_with_debounce(
                validation_email.clone(),
                validator_id,
                ValidationTrigger::Change,
                delay,
                move |_value, _snapshot| validation.future(),
            );
        }
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let result = form
                .managed_submit()
                .on_submit_async(managed_submit_event(), |_submitted| async {});

            probe.submit_result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

#[cfg(feature = "serde")]
fn snapshot_with_unsupported_form_state_version<Model, Error>(
    snapshot: dioform::advanced::FormStateSnapshot<Model, Error>,
) -> dioform::advanced::FormStateSnapshot<Model, Error>
where
    Model: serde::Serialize + for<'de> serde::Deserialize<'de>,
    Error: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    let mut value = serde_json::to_value(snapshot).expect("snapshot should serialize");

    value["version"] = serde_json::json!(u32::MAX);

    serde_json::from_value(value).expect("snapshot with unsupported version should deserialize")
}

#[cfg(feature = "serde")]
fn invoice_collection_snapshot_value() -> serde_json::Value {
    let handle: FormHandle<InvoiceCollectionForm, String> =
        FormHandle::new_with_error_type(invoice_collection_form());
    let lines = handle.collection(InvoiceCollectionForm::fields().lines());

    assert_eq!(lines.items().len(), 2);

    serde_json::to_value(handle.state_snapshot()).expect("snapshot should serialize to JSON value")
}

#[cfg(feature = "serde")]
fn invoice_collection_snapshot_from_value(
    value: serde_json::Value,
) -> dioform::advanced::FormStateSnapshot<InvoiceCollectionForm, String> {
    serde_json::from_value(value).expect("mutated snapshot should deserialize")
}

#[cfg(feature = "serde")]
fn restore_invoice_collection_snapshot_value(
    value: serde_json::Value,
) -> Result<(), FormStateRestoreError> {
    let restored: FormHandle<InvoiceCollectionForm, String> =
        FormHandle::new_with_error_type(InvoiceCollectionForm { lines: Vec::new() });

    restored.restore_state_snapshot(invoice_collection_snapshot_from_value(value))
}

#[cfg(feature = "serde")]
#[test]
fn dioform_state_snapshot_rejects_malformed_collection_identity_state() {
    let mut version_mismatch = invoice_collection_snapshot_value();
    version_mismatch["collection_identities"]["version"] = serde_json::json!(u32::MAX);
    assert!(matches!(
        restore_invoice_collection_snapshot_value(version_mismatch),
        Err(FormStateRestoreError::UnsupportedCollectionIdentityVersion { actual, .. })
            if actual == u32::MAX
    ));

    let mut duplicate_collection = invoice_collection_snapshot_value();
    let collections = duplicate_collection["collection_identities"]["collections"]
        .as_array_mut()
        .expect("collections should be an array");
    collections.push(collections[0].clone());
    assert!(matches!(
        restore_invoice_collection_snapshot_value(duplicate_collection),
        Err(FormStateRestoreError::DuplicateCollectionIdentity { .. })
    ));

    let mut duplicate_item = invoice_collection_snapshot_value();
    let current_items = duplicate_item["collection_identities"]["collections"][0]["current_items"]
        .as_array_mut()
        .expect("current items should be an array");
    current_items[1] = current_items[0].clone();
    assert!(matches!(
        restore_invoice_collection_snapshot_value(duplicate_item),
        Err(FormStateRestoreError::DuplicateCollectionItemIdentity { .. })
    ));

    let mut invalid_next_identity = invoice_collection_snapshot_value();
    invalid_next_identity["collection_identities"]["collections"][0]["next_item_identity"] =
        serde_json::json!(1);
    assert!(matches!(
        restore_invoice_collection_snapshot_value(invalid_next_identity),
        Err(FormStateRestoreError::InvalidNextCollectionItemIdentity { .. })
    ));
}

#[derive(Default)]
struct PlainSubmitFlushProbe {
    delay: AsyncGate<()>,
    validation: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validator_id: RefCell<Option<ValidatorId>>,
    validation_snapshot: RefCell<Option<(String, String)>>,
    submit_calls: Cell<u32>,
}

fn plain_submit_flush_probe(probe: Rc<PlainSubmitFlushProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();
            let validator_id = form.write_advanced(|core| {
                core.register_async_field_validator_for_triggers(
                    email,
                    "availability",
                    ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
                )
            });

            probe.validator_id.borrow_mut().replace(validator_id);

            form
        }
    });
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    let validation_email = email.clone();
    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let delay = probe.delay.future();
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.validate_async_field_validator_with_debounce(
                validation_email.clone(),
                validator_id,
                ValidationTrigger::Change,
                delay,
                move |value, snapshot| {
                    captured_probe
                        .validation_snapshot
                        .borrow_mut()
                        .replace((value, snapshot.value().email.clone()));
                    validation.future()
                },
            );
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

#[derive(Default)]
struct ManagedAsyncSubmitFormValidationProbe {
    delay: AsyncGate<()>,
    validation: AsyncGate<Vec<FormValidationError<&'static str>>>,
    submit: AsyncGate<()>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validator_id: RefCell<Option<ValidatorId>>,
    submit_result: RefCell<Option<SubmitResult>>,
    validation_snapshot: RefCell<Option<String>>,
    submitted_snapshot: RefCell<Option<SignupForm>>,
    submit_calls: Cell<u32>,
}

fn managed_async_submit_form_validation_probe(
    probe: Rc<ManagedAsyncSubmitFormValidationProbe>,
) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let validator_id = form.write_advanced(|core| {
                core.register_async_form_validator_for_triggers(
                    "account",
                    ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
                )
            });

            probe.validator_id.borrow_mut().replace(validator_id);

            form
        }
    });
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let delay = probe.delay.future();
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.validate_async_form_validator_with_debounce(
                validator_id,
                ValidationTrigger::Change,
                delay,
                move |snapshot| {
                    captured_probe
                        .validation_snapshot
                        .borrow_mut()
                        .replace(snapshot.value().email.clone());
                    validation.future()
                },
            );
        }
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let submit = probe.submit.clone();
            let submit_probe = Rc::clone(&probe);
            let result =
                form.managed_submit()
                    .on_submit_async(managed_submit_event(), move |submitted| {
                        submit_probe
                            .submit_calls
                            .set(submit_probe.submit_calls.get() + 1);
                        submit_probe
                            .submitted_snapshot
                            .borrow_mut()
                            .replace(submitted.value().clone());
                        submit.future()
                    });

            probe.submit_result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

#[derive(Default)]
struct PlainSubmitFormFlushProbe {
    delay: AsyncGate<()>,
    validation: AsyncGate<Vec<FormValidationError<&'static str>>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validator_id: RefCell<Option<ValidatorId>>,
    validation_snapshot: RefCell<Option<String>>,
    submit_calls: Cell<u32>,
}

fn plain_submit_form_flush_probe(probe: Rc<PlainSubmitFormFlushProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let validator_id = form.write_advanced(|core| {
                core.register_async_form_validator_for_triggers(
                    "account",
                    ValidationTriggers::new([ValidationTrigger::Change, ValidationTrigger::Submit]),
                )
            });

            probe.validator_id.borrow_mut().replace(validator_id);

            form
        }
    });
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let delay = probe.delay.future();
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.validate_async_form_validator_with_debounce(
                validator_id,
                ValidationTrigger::Change,
                delay,
                move |snapshot| {
                    captured_probe
                        .validation_snapshot
                        .borrow_mut()
                        .replace(snapshot.value().email.clone());
                    validation.future()
                },
            );
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

#[test]
fn dioxus_managed_async_submit_flushes_debounced_validation_before_submit_handler() {
    let probe = Rc::new(ManagedAsyncSubmitValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(managed_async_submit_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.submit_result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());
    assert_eq!(probe.submit_calls.get(), 0);
    assert_eq!(handle.submit_attempt_count(), 1);
    assert_eq!(
        handle
            .managed_submit()
            .on_submit_async(managed_submit_event(), |_submitted| async {}),
        SubmitResult::Blocked(SubmitBlocker::InFlightSubmission)
    );
    assert_eq!(handle.submit_attempt_count(), 1);

    dom.render_immediate_to_vec();

    assert_eq!(
        probe.validation_snapshot.borrow().as_ref(),
        Some(&("ada@example.com".to_owned(), "ada@example.com".to_owned()))
    );
    assert_eq!(probe.submit_calls.get(), 0);

    probe.validation.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 1);
    assert_eq!(
        probe.submitted_snapshot.borrow().as_ref(),
        Some(&SignupForm {
            email: "ada@example.com".to_owned()
        })
    );
    assert!(handle.is_submitting());
    assert_eq!(
        handle
            .managed_submit()
            .on_submit_async(managed_submit_event(), |_submitted| async {}),
        SubmitResult::Blocked(SubmitBlocker::InFlightSubmission)
    );

    probe.submit.complete(());
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert_eq!(handle.last_submit_status(), Some(SubmitStatus::Succeeded));
    assert!(handle.validation_errors().is_empty());
}

#[cfg(feature = "serde")]
#[test]
fn dioform_handle_rejected_state_snapshot_restore_preserves_adapter_runtime_state() {
    let probe = Rc::new(InvalidRestoreAtomicityProbe::default());
    let mut dom = VirtualDom::new_with_props(invalid_restore_atomicity_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.submit_result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());

    let valid_snapshot = FormHandle::<SignupForm, String>::new_with_error_type(SignupForm {
        email: "fresh@example.com".to_owned(),
    })
    .state_snapshot();
    let invalid_snapshot = snapshot_with_unsupported_form_state_version(valid_snapshot);

    assert!(matches!(
        handle.restore_state_snapshot(invalid_snapshot),
        Err(FormStateRestoreError::UnsupportedFormStateVersion { actual, .. })
            if actual == u32::MAX
    ));
    assert!(handle.is_submitting());
    assert_eq!(
        handle
            .managed_submit()
            .on_submit_async(managed_submit_event(), |_submitted| async {}),
        SubmitResult::Blocked(SubmitBlocker::InFlightSubmission)
    );
}

#[test]
fn dioxus_plain_submit_flushes_debounced_validation_before_blocking() {
    let probe = Rc::new(PlainSubmitFlushProbe::default());
    let mut dom = VirtualDom::new_with_props(plain_submit_flush_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    assert_eq!(
        handle.field_validation_status(email.clone(), validator_id),
        Some(ValidationStatus::Pending)
    );

    let submit_probe = Rc::clone(&probe);
    let result = handle.submit(move |_submitted| {
        submit_probe
            .submit_calls
            .set(submit_probe.submit_calls.get() + 1);
    });

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert_eq!(probe.submit_calls.get(), 0);
    assert!(probe.validation_snapshot.borrow().is_none());

    dom.render_immediate_to_vec();

    assert_eq!(
        probe.validation_snapshot.borrow().as_ref(),
        Some(&("ada@example.com".to_owned(), "ada@example.com".to_owned()))
    );
    assert_eq!(
        handle.field_validation_status(email.clone(), validator_id),
        Some(ValidationStatus::Pending)
    );
}

#[test]
fn dioxus_managed_async_submit_flushes_debounced_form_validation_before_submit_handler() {
    let probe = Rc::new(ManagedAsyncSubmitFormValidationProbe::default());
    let mut dom = VirtualDom::new_with_props(
        managed_async_submit_form_validation_probe,
        Rc::clone(&probe),
    );

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.submit_result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());
    assert_eq!(probe.submit_calls.get(), 0);

    dom.render_immediate_to_vec();

    assert_eq!(
        probe.validation_snapshot.borrow().as_deref(),
        Some("ada@example.com")
    );
    assert_eq!(probe.submit_calls.get(), 0);

    probe.validation.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 1);
    assert_eq!(
        probe.submitted_snapshot.borrow().as_ref(),
        Some(&SignupForm {
            email: "ada@example.com".to_owned()
        })
    );

    probe.submit.complete(());
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert!(handle.validation_errors().is_empty());
}

#[test]
fn dioxus_plain_submit_flushes_debounced_form_validation_before_blocking() {
    let probe = Rc::new(PlainSubmitFormFlushProbe::default());
    let mut dom = VirtualDom::new_with_props(plain_submit_form_flush_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    assert_eq!(
        handle.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Pending)
    );

    let submit_probe = Rc::clone(&probe);
    let result = handle.submit(move |_submitted| {
        submit_probe
            .submit_calls
            .set(submit_probe.submit_calls.get() + 1);
    });

    assert_eq!(
        result,
        SubmitResult::Blocked(SubmitBlocker::PendingValidation)
    );
    assert_eq!(probe.submit_calls.get(), 0);
    assert!(probe.validation_snapshot.borrow().is_none());

    dom.render_immediate_to_vec();

    assert_eq!(
        probe.validation_snapshot.borrow().as_deref(),
        Some("ada@example.com")
    );
    assert_eq!(
        handle.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Pending)
    );
}

#[test]
fn dioxus_managed_async_submit_blocks_when_flushed_form_validation_returns_errors() {
    let probe = Rc::new(ManagedAsyncSubmitFormValidationProbe::default());
    let mut dom = VirtualDom::new_with_props(
        managed_async_submit_form_validation_probe,
        Rc::clone(&probe),
    );

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    probe
        .validation
        .complete(vec![FormValidationError::form("account_unavailable")]);
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
    assert!(!handle.is_submitting());
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );
    let errors = handle.form_validation_errors();
    assert_eq!(errors[0].source().as_str(), "account");
    assert_eq!(errors[0].error(), &"account_unavailable");
}

#[test]
fn dioxus_managed_async_submit_blocks_when_flushed_validation_returns_errors() {
    let probe = Rc::new(ManagedAsyncSubmitValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(managed_async_submit_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    probe.validation.complete(vec!["email_unavailable"]);
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
    assert!(!handle.is_submitting());
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );
    let errors = handle.field_validation_errors(email);
    assert_eq!(errors[0].source().as_str(), "availability");
    assert_eq!(errors[0].error(), &"email_unavailable");
}

#[test]
fn dioxus_managed_async_submit_does_not_submit_stale_flushed_validation_after_draft_edit() {
    let probe = Rc::new(ManagedAsyncSubmitValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(managed_async_submit_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    handle.text(email.clone()).on_input("lin@example.com");
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
    assert!(!handle.is_submitting());
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
    );
    assert_eq!(handle.field_value(email), "lin@example.com");

    probe.validation.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
    assert!(!handle.is_submitting());
    assert!(handle.validation_errors().is_empty());
}

#[test]
fn submit_listener_reports_terminal_block_when_managed_validation_becomes_stale() {
    let probe = Rc::new(ManagedAsyncSubmitValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(managed_async_submit_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [SubmitListenerEvent::SubmitAttempted]
    );

    handle.text(email).on_input("lin@example.com");
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmitBlocked(SubmitBlocker::PendingValidation),
        ]
    );
}

#[test]
fn dioxus_managed_async_submit_does_not_submit_after_reset_while_validation_is_pending() {
    let probe = Rc::new(ManagedAsyncSubmitValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(managed_async_submit_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert!(handle.is_submitting());

    handle.reset();

    assert!(!handle.is_submitting());
    assert!(
        !handle
            .submit_availability()
            .contains(SubmitBlocker::InFlightSubmission)
    );

    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
    assert!(!handle.is_submitting());
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
    );

    probe.validation.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
}

#[test]
fn submit_listener_reports_terminal_block_when_managed_validation_is_cancelled() {
    let probe = Rc::new(ManagedAsyncSubmitValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(managed_async_submit_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(
        probe.events.borrow().as_slice(),
        [SubmitListenerEvent::SubmitAttempted]
    );

    handle.reset();
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
    assert_eq!(
        probe.events.borrow().as_slice(),
        [
            SubmitListenerEvent::SubmitAttempted,
            SubmitListenerEvent::SubmitBlocked(SubmitBlocker::PendingValidation),
        ]
    );
}

#[test]
fn dioxus_managed_async_submit_does_not_submit_after_reinitialize_while_validation_is_pending() {
    let probe = Rc::new(ManagedAsyncSubmitValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(managed_async_submit_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert!(handle.is_submitting());

    handle.reinitialize(SignupForm {
        email: "grace@example.com".to_owned(),
    });

    assert!(!handle.is_submitting());
    assert!(
        !handle
            .submit_availability()
            .contains(SubmitBlocker::InFlightSubmission)
    );

    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
    assert!(!handle.is_submitting());
    assert_eq!(
        handle.field_value(SignupForm::fields().email()),
        "grace@example.com"
    );
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::PendingValidation))
    );

    probe.validation.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
}

#[derive(Default)]
struct ManagedAsyncSubmitStartsValidationProbe {
    validation: AsyncGate<Vec<&'static str>>,
    submit: AsyncGate<()>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    submit_result: RefCell<Option<SubmitResult>>,
    validation_snapshot: RefCell<Option<(String, String)>>,
    submitted_snapshot: RefCell<Option<SignupForm>>,
    submit_calls: Cell<u32>,
}

fn managed_async_submit_starts_validation_probe(
    probe: Rc<ManagedAsyncSubmitStartsValidationProbe>,
) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.field(email.clone())
                .async_validator("availability")
                .on(ValidationTrigger::Submit)
                .check(move |value, snapshot| {
                    captured_probe
                        .validation_snapshot
                        .borrow_mut()
                        .replace((value, snapshot.value().email.clone()));
                    validation.future()
                });

            form
        }
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let submit = probe.submit.clone();
            let submit_probe = Rc::clone(&probe);
            let result =
                form.managed_submit()
                    .on_submit_async(managed_submit_event(), move |submitted| {
                        submit_probe
                            .submit_calls
                            .set(submit_probe.submit_calls.get() + 1);
                        submit_probe
                            .submitted_snapshot
                            .borrow_mut()
                            .replace(submitted.value().clone());
                        submit.future()
                    });

            probe.submit_result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

#[test]
fn dioxus_managed_async_submit_starts_required_async_validation_before_submit_handler() {
    let probe = Rc::new(ManagedAsyncSubmitStartsValidationProbe::default());
    let mut dom = VirtualDom::new_with_props(
        managed_async_submit_starts_validation_probe,
        Rc::clone(&probe),
    );

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.submit_result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());
    assert_eq!(probe.submit_calls.get(), 0);

    dom.render_immediate_to_vec();

    assert_eq!(
        probe.validation_snapshot.borrow().as_ref(),
        Some(&("ada@example.com".to_owned(), "ada@example.com".to_owned()))
    );
    assert_eq!(probe.submit_calls.get(), 0);

    probe.validation.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 1);
    assert_eq!(
        probe.submitted_snapshot.borrow().as_ref(),
        Some(&SignupForm {
            email: "ada@example.com".to_owned()
        })
    );
    assert!(handle.is_submitting());

    probe.submit.complete(());
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert!(handle.validation_errors().is_empty());
}

#[derive(Default)]
struct ManagedAsyncSubmitStartsFormValidationProbe {
    validation: AsyncGate<Vec<FormValidationError<&'static str>>>,
    submit: AsyncGate<()>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    submit_result: RefCell<Option<SubmitResult>>,
    validation_snapshot: RefCell<Option<String>>,
    submitted_snapshot: RefCell<Option<SignupForm>>,
    submit_calls: Cell<u32>,
}

fn managed_async_submit_starts_form_validation_probe(
    probe: Rc<ManagedAsyncSubmitStartsFormValidationProbe>,
) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                });
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);

            form.async_validator("account")
                .on(ValidationTrigger::Submit)
                .check(move |snapshot| {
                    captured_probe
                        .validation_snapshot
                        .borrow_mut()
                        .replace(snapshot.value().email.clone());
                    validation.future()
                });

            form
        }
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let submit = probe.submit.clone();
            let submit_probe = Rc::clone(&probe);
            let result =
                form.managed_submit()
                    .on_submit_async(managed_submit_event(), move |submitted| {
                        submit_probe
                            .submit_calls
                            .set(submit_probe.submit_calls.get() + 1);
                        submit_probe
                            .submitted_snapshot
                            .borrow_mut()
                            .replace(submitted.value().clone());
                        submit.future()
                    });

            probe.submit_result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);
    VNode::empty()
}

#[test]
fn dioxus_managed_async_submit_starts_required_async_form_validation_before_submit_handler() {
    let probe = Rc::new(ManagedAsyncSubmitStartsFormValidationProbe::default());
    let mut dom = VirtualDom::new_with_props(
        managed_async_submit_starts_form_validation_probe,
        Rc::clone(&probe),
    );

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(*probe.submit_result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());
    assert_eq!(probe.submit_calls.get(), 0);

    dom.render_immediate_to_vec();

    assert_eq!(
        probe.validation_snapshot.borrow().as_deref(),
        Some("ada@example.com")
    );
    assert_eq!(probe.submit_calls.get(), 0);

    probe.validation.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 1);
    assert_eq!(
        probe.submitted_snapshot.borrow().as_ref(),
        Some(&SignupForm {
            email: "ada@example.com".to_owned()
        })
    );

    probe.submit.complete(());
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert!(handle.validation_errors().is_empty());
}

#[derive(Default)]
struct AsyncSubmitErrorProbe {
    gate: AsyncGate<SubmitError<SignupForm, &'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    result: RefCell<Option<SubmitResult>>,
}

fn async_submit_error_probe(probe: Rc<AsyncSubmitErrorProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "taken@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let gate = probe.gate.clone();
            let result = form
                .managed_submit()
                .on_submit_async(managed_submit_event(), move |_submitted| gate.future());

            probe.result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

#[test]
fn dioxus_managed_async_submit_records_structured_submit_errors() {
    let probe = Rc::new(AsyncSubmitErrorProbe::default());
    let mut dom = VirtualDom::new_with_props(async_submit_error_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(*probe.result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());

    probe
        .gate
        .complete(SubmitError::field(email.clone(), "email_unavailable"));
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());

    let errors: Vec<_> = handle
        .field_validation_errors(email)
        .into_iter()
        .map(|error| {
            (
                error.field().unwrap().as_str().to_owned(),
                error.source().as_str().to_owned(),
                *error.error(),
            )
        })
        .collect();

    assert_eq!(
        errors,
        vec![("email".to_owned(), "submit".to_owned(), "email_unavailable")]
    );
}

#[derive(Default)]
struct AsyncSubmitStaleErrorProbe {
    gate: AsyncGate<SubmitError<SignupForm, &'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    submitted_snapshot: RefCell<Option<SignupForm>>,
}

fn async_submit_stale_error_probe(probe: Rc<AsyncSubmitStaleErrorProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "taken@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let gate = probe.gate.clone();
            let submit_probe = Rc::clone(&probe);

            form.managed_submit()
                .on_submit_async(managed_submit_event(), move |submitted| {
                    submit_probe
                        .submitted_snapshot
                        .borrow_mut()
                        .replace(submitted.value().clone());
                    gate.future()
                });
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

#[test]
fn dioxus_managed_async_submit_discards_stale_field_errors_after_draft_edit() {
    let probe = Rc::new(AsyncSubmitStaleErrorProbe::default());
    let mut dom = VirtualDom::new_with_props(async_submit_stale_error_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert!(handle.is_submitting());

    handle.text(email.clone()).on_input("new@example.com");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.submitted_snapshot.borrow().as_ref(),
        Some(&SignupForm {
            email: "taken@example.com".to_owned()
        })
    );
    assert_eq!(handle.field_value(email.clone()), "new@example.com");

    probe
        .gate
        .complete(SubmitError::field(email.clone(), "email_unavailable"));
    dom.render_immediate_to_vec();

    assert!(!handle.is_submitting());
    assert!(handle.field_validation_errors(email.clone()).is_empty());
    assert!(handle.validation_errors().is_empty());
    assert_eq!(handle.field_value(email), "new@example.com");
}

#[derive(Debug, Eq, PartialEq)]
struct AsyncFieldValidationSnapshot {
    status: ValidationStatus,
    error_count: usize,
    visible_error_count: usize,
    can_submit: bool,
    aria_invalid: bool,
}

#[derive(Default)]
struct AsyncFieldValidationProbe {
    gate: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validator_id: RefCell<Option<ValidatorId>>,
    start_status: RefCell<Option<ValidationStatus>>,
    captured: RefCell<Option<(String, String)>>,
    snapshots: RefCell<Vec<AsyncFieldValidationSnapshot>>,
}

fn reactive_async_field_validation_probe(probe: Rc<AsyncFieldValidationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "taken@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();
            let validator_id = form.write_advanced(|core| {
                core.register_async_field_validator_for_triggers(
                    email,
                    "availability",
                    ValidationTrigger::Manual,
                )
            });

            probe.validator_id.borrow_mut().replace(validator_id);

            form
        }
    });
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    let validation_email = email.clone();
    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let gate = probe.gate.clone();
            let captured_probe = Rc::clone(&probe);
            let non_send_marker = Rc::new(Cell::new(0));
            let start_status = form.validate_async_field_validator(
                validation_email.clone(),
                validator_id,
                ValidationTrigger::Manual,
                move |value, snapshot| {
                    non_send_marker.set(non_send_marker.get() + 1);
                    captured_probe
                        .captured
                        .borrow_mut()
                        .replace((value, snapshot.value().email.clone()));
                    gate.future()
                },
            );

            probe.start_status.borrow_mut().replace(
                start_status.expect("async validator should schedule through the adapter"),
            );
        }
    });

    let status = form
        .field_validation_status(email.clone(), validator_id)
        .expect("async validator status should be readable");
    let error_count = form.field_validation_errors(email.clone()).len();
    let visible_error_count = form.visible_field_validation_errors(email.clone()).len();
    let can_submit = form.can_submit();
    let aria_invalid = form.field_accessibility(email.clone()).aria_invalid();

    probe.handle.borrow_mut().replace(form);
    probe
        .snapshots
        .borrow_mut()
        .push(AsyncFieldValidationSnapshot {
            status,
            error_count,
            visible_error_count,
            can_submit,
            aria_invalid,
        });

    VNode::empty()
}

#[test]
fn dioxus_adapter_async_field_validation_updates_reactive_selectors() {
    let probe = Rc::new(AsyncFieldValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(reactive_async_field_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(
        *probe.start_status.borrow(),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(
        probe.captured.borrow().as_ref(),
        Some(&(
            "taken@example.com".to_owned(),
            "taken@example.com".to_owned()
        ))
    );
    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [AsyncFieldValidationSnapshot {
            status: ValidationStatus::Pending,
            error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        }]
    );

    handle.mark_field_blurred(email.clone());
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFieldValidationSnapshot {
            status: ValidationStatus::Pending,
            error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        })
    );

    probe.gate.complete(vec!["email_unavailable"]);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFieldValidationSnapshot {
            status: ValidationStatus::Invalid,
            error_count: 1,
            visible_error_count: 1,
            can_submit: false,
            aria_invalid: true,
        })
    );
    assert_eq!(
        handle.field_validation_errors(email)[0].error(),
        &"email_unavailable"
    );
}

#[derive(Default)]
struct DebouncedAsyncFieldValidationProbe {
    delay: AsyncGate<()>,
    validation: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validator_id: RefCell<Option<ValidatorId>>,
    start_status: RefCell<Option<ValidationStatus>>,
    captured: RefCell<Option<(String, String)>>,
    snapshots: RefCell<Vec<AsyncFieldValidationSnapshot>>,
}

fn debounced_async_field_validation_probe(
    probe: Rc<DebouncedAsyncFieldValidationProbe>,
) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "taken@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();
            let validator_id = form.write_advanced(|core| {
                core.register_async_field_validator_for_triggers(
                    email,
                    "availability",
                    ValidationTrigger::Change,
                )
            });

            probe.validator_id.borrow_mut().replace(validator_id);

            form
        }
    });
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    let validation_email = email.clone();
    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let delay = probe.delay.future();
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);
            let start_status = form.validate_async_field_validator_with_debounce(
                validation_email.clone(),
                validator_id,
                ValidationTrigger::Change,
                delay,
                move |value, snapshot| {
                    captured_probe
                        .captured
                        .borrow_mut()
                        .replace((value, snapshot.value().email.clone()));
                    validation.future()
                },
            );

            probe
                .start_status
                .borrow_mut()
                .replace(start_status.expect("debounced async validator should schedule"));
        }
    });

    let status = form
        .field_validation_status(email.clone(), validator_id)
        .expect("async validator status should be readable");
    let error_count = form.field_validation_errors(email.clone()).len();
    let visible_error_count = form.visible_field_validation_errors(email.clone()).len();
    let can_submit = form.can_submit();
    let aria_invalid = form.field_accessibility(email.clone()).aria_invalid();

    probe.handle.borrow_mut().replace(form);
    probe
        .snapshots
        .borrow_mut()
        .push(AsyncFieldValidationSnapshot {
            status,
            error_count,
            visible_error_count,
            can_submit,
            aria_invalid,
        });

    VNode::empty()
}

#[test]
fn dioxus_adapter_debounced_value_change_async_validation_updates_reactive_selectors() {
    let probe = Rc::new(DebouncedAsyncFieldValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(debounced_async_field_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(
        *probe.start_status.borrow(),
        Some(ValidationStatus::Pending)
    );
    assert!(probe.captured.borrow().is_none());
    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [AsyncFieldValidationSnapshot {
            status: ValidationStatus::Pending,
            error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        }]
    );

    handle.mark_field_blurred(email.clone());
    probe.delay.complete(());
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.captured.borrow().as_ref(),
        Some(&(
            "taken@example.com".to_owned(),
            "taken@example.com".to_owned()
        ))
    );

    probe.validation.complete(vec!["email_unavailable"]);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFieldValidationSnapshot {
            status: ValidationStatus::Invalid,
            error_count: 1,
            visible_error_count: 1,
            can_submit: false,
            aria_invalid: true,
        })
    );
    assert_eq!(
        handle.field_validation_errors(email)[0].error(),
        &"email_unavailable"
    );
}

#[derive(Default)]
struct DebouncedValidationCancellationProbe {
    delay_drops: Rc<Cell<usize>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    on_input: RefCell<Option<Box<InputHandler>>>,
}

fn debounced_validation_cancellation_probe(
    probe: Rc<DebouncedValidationCancellationProbe>,
) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                })
                .with_validation_mode(ValidationMode::on_change());
            let email = SignupForm::fields().email();
            let delay_drops = Rc::clone(&probe.delay_drops);

            form.field(email.clone())
                .async_validator("availability")
                .on(ValidationTrigger::Change)
                .debounce(move || DropCountingDelay {
                    drops: Rc::clone(&delay_drops),
                })
                .check(|_value, _snapshot| async { Vec::<&'static str>::new() });

            form
        }
    });

    let email = SignupForm::fields().email();
    let runtime = dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let on_input = {
        let runtime = Rc::clone(&runtime);
        let form = form.clone();

        move |value: String| runtime.in_scope(scope, || form.text(email.clone()).on_input(value))
    };

    probe.handle.borrow_mut().replace(form);
    probe.on_input.borrow_mut().replace(Box::new(on_input));

    VNode::empty()
}

#[test]
fn dioxus_debounced_value_change_cancels_superseded_delay() {
    let probe = Rc::new(DebouncedValidationCancellationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(debounced_validation_cancellation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("first@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.delay_drops.get(), 0);

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("second@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.delay_drops.get(), 1);
}

#[test]
fn dioxus_cleanup_cancels_pending_debounced_delay() {
    let probe = Rc::new(DebouncedValidationCancellationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(debounced_validation_cancellation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("first@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.delay_drops.get(), 0);

    drop(dom);

    assert_eq!(probe.delay_drops.get(), 1);
}

#[derive(Default)]
struct DebouncedFormValidationCancellationProbe {
    delay_drops: Rc<Cell<usize>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    on_input: RefCell<Option<Box<InputHandler>>>,
}

fn debounced_form_validation_cancellation_probe(
    probe: Rc<DebouncedFormValidationCancellationProbe>,
) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                })
                .with_validation_mode(ValidationMode::on_change());
            let delay_drops = Rc::clone(&probe.delay_drops);

            form.async_validator("account")
                .on(ValidationTrigger::Change)
                .debounce(move || DropCountingDelay {
                    drops: Rc::clone(&delay_drops),
                })
                .check(|_snapshot| async { Vec::<FormValidationError<&'static str>>::new() });

            form
        }
    });

    let email = SignupForm::fields().email();
    let runtime = dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let on_input = {
        let runtime = Rc::clone(&runtime);
        let form = form.clone();

        move |value: String| runtime.in_scope(scope, || form.text(email.clone()).on_input(value))
    };

    probe.handle.borrow_mut().replace(form);
    probe.on_input.borrow_mut().replace(Box::new(on_input));

    VNode::empty()
}

#[test]
fn dioxus_debounced_form_value_change_cancels_superseded_delay() {
    let probe = Rc::new(DebouncedFormValidationCancellationProbe::default());
    let mut dom = VirtualDom::new_with_props(
        debounced_form_validation_cancellation_probe,
        Rc::clone(&probe),
    );

    dom.rebuild_in_place();

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("first@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.delay_drops.get(), 0);

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("second@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.delay_drops.get(), 1);
}

#[test]
fn dioxus_cleanup_cancels_pending_form_debounced_delay() {
    let probe = Rc::new(DebouncedFormValidationCancellationProbe::default());
    let mut dom = VirtualDom::new_with_props(
        debounced_form_validation_cancellation_probe,
        Rc::clone(&probe),
    );

    dom.rebuild_in_place();

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("first@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.delay_drops.get(), 0);

    drop(dom);

    assert_eq!(probe.delay_drops.get(), 1);
}

#[derive(Default)]
struct AsyncValidationCancellationProbe {
    validation_drops: Rc<Cell<usize>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    on_input: RefCell<Option<Box<InputHandler>>>,
}

fn async_validation_cancellation_probe(probe: Rc<AsyncValidationCancellationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                })
                .with_validation_mode(ValidationMode::on_change());
            let email = SignupForm::fields().email();
            let validation_drops = Rc::clone(&probe.validation_drops);

            form.field(email.clone())
                .async_validator("availability")
                .on(ValidationTrigger::Change)
                .check(move |_value, _snapshot| DropCountingValidation {
                    drops: Rc::clone(&validation_drops),
                });

            form
        }
    });

    let email = SignupForm::fields().email();
    let runtime = dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let on_input = {
        let runtime = Rc::clone(&runtime);
        let form = form.clone();

        move |value: String| runtime.in_scope(scope, || form.text(email.clone()).on_input(value))
    };

    probe.handle.borrow_mut().replace(form);
    probe.on_input.borrow_mut().replace(Box::new(on_input));

    VNode::empty()
}

#[test]
fn dioxus_value_change_cancels_superseded_async_validation_task() {
    let probe = Rc::new(AsyncValidationCancellationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(async_validation_cancellation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("first@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.validation_drops.get(), 0);

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("second@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.validation_drops.get(), 1);
}

#[test]
fn dioxus_cleanup_cancels_pending_async_validation_task() {
    let probe = Rc::new(AsyncValidationCancellationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(async_validation_cancellation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("first@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.validation_drops.get(), 0);

    drop(dom);

    assert_eq!(probe.validation_drops.get(), 1);
}

#[derive(Default)]
struct AsyncFormValidationCancellationProbe {
    validation_drops: Rc<Cell<usize>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    on_input: RefCell<Option<Box<InputHandler>>>,
}

fn async_form_validation_cancellation_probe(
    probe: Rc<AsyncFormValidationCancellationProbe>,
) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "ada@example.com".to_owned(),
                })
                .with_validation_mode(ValidationMode::on_change());
            let validation_drops = Rc::clone(&probe.validation_drops);

            form.async_validator("account")
                .on(ValidationTrigger::Change)
                .check(move |_snapshot| DropCountingFormValidation {
                    drops: Rc::clone(&validation_drops),
                });

            form
        }
    });

    let email = SignupForm::fields().email();
    let runtime = dioxus_core::Runtime::current();
    let scope = runtime.current_scope_id();
    let on_input = {
        let runtime = Rc::clone(&runtime);
        let form = form.clone();

        move |value: String| runtime.in_scope(scope, || form.text(email.clone()).on_input(value))
    };

    probe.handle.borrow_mut().replace(form);
    probe.on_input.borrow_mut().replace(Box::new(on_input));

    VNode::empty()
}

#[test]
fn dioform_value_change_cancels_superseded_async_validation_task() {
    let probe = Rc::new(AsyncFormValidationCancellationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(async_form_validation_cancellation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("first@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.validation_drops.get(), 0);

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("second@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.validation_drops.get(), 1);
}

#[test]
fn dioxus_cleanup_cancels_pending_form_async_validation_task() {
    let probe = Rc::new(AsyncFormValidationCancellationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(async_form_validation_cancellation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    {
        let on_input = probe.on_input.borrow();
        let on_input = on_input
            .as_ref()
            .expect("probe should expose input handler");
        on_input("first@example.com".to_owned());
    }
    dom.render_immediate_to_vec();

    assert_eq!(probe.validation_drops.get(), 0);

    drop(dom);

    assert_eq!(probe.validation_drops.get(), 1);
}

#[derive(Default)]
struct AsyncFieldValidationStaleProbe {
    gate: AsyncGate<Vec<&'static str>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validator_id: RefCell<Option<ValidatorId>>,
    snapshots: RefCell<Vec<AsyncFieldValidationSnapshot>>,
}

fn stale_async_field_validation_probe(probe: Rc<AsyncFieldValidationStaleProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "taken@example.com".to_owned(),
                });
            let email = SignupForm::fields().email();
            let validator_id = form.write_advanced(|core| {
                core.register_async_field_validator_for_triggers(
                    email,
                    "availability",
                    ValidationTrigger::Manual,
                )
            });

            probe.validator_id.borrow_mut().replace(validator_id);

            form
        }
    });
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    let validation_email = email.clone();
    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let gate = probe.gate.clone();

            form.validate_async_field_validator(
                validation_email.clone(),
                validator_id,
                ValidationTrigger::Manual,
                move |_value, _snapshot| gate.future(),
            );
        }
    });

    let status = form
        .field_validation_status(email.clone(), validator_id)
        .expect("async validator status should be readable");
    let error_count = form.field_validation_errors(email.clone()).len();
    let visible_error_count = form.visible_field_validation_errors(email.clone()).len();
    let can_submit = form.can_submit();
    let aria_invalid = form.field_accessibility(email.clone()).aria_invalid();

    probe.handle.borrow_mut().replace(form);
    probe
        .snapshots
        .borrow_mut()
        .push(AsyncFieldValidationSnapshot {
            status,
            error_count,
            visible_error_count,
            can_submit,
            aria_invalid,
        });

    VNode::empty()
}

#[test]
fn dioxus_adapter_ignores_stale_async_field_validation_after_edit() {
    let probe = Rc::new(AsyncFieldValidationStaleProbe::default());
    let mut dom = VirtualDom::new_with_props(stale_async_field_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [AsyncFieldValidationSnapshot {
            status: ValidationStatus::Pending,
            error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        }]
    );

    handle.mark_field_blurred(email.clone());
    dom.render_immediate_to_vec();
    handle.text(email.clone()).on_input("fresh@example.com");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFieldValidationSnapshot {
            status: ValidationStatus::Stale,
            error_count: 0,
            visible_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        })
    );

    let snapshot_count_after_edit = probe.snapshots.borrow().len();

    probe.gate.complete(vec!["email_unavailable"]);
    dom.render_immediate_to_vec();

    assert_eq!(probe.snapshots.borrow().len(), snapshot_count_after_edit);
    assert_eq!(handle.field_value(email.clone()), "fresh@example.com");
    assert!(handle.field_validation_errors(email.clone()).is_empty());
    assert!(handle.visible_field_validation_errors(email).is_empty());
}

#[test]
fn dioxus_adapter_ignores_late_async_field_validation_success_after_form_cleanup() {
    let probe = Rc::new(AsyncFieldValidationStaleProbe::default());
    let mut dom = VirtualDom::new_with_props(stale_async_field_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    assert_eq!(
        handle.field_validation_status(email.clone(), validator_id),
        Some(ValidationStatus::Pending)
    );

    drop(dom);
    probe.gate.complete(Vec::new());

    assert_eq!(
        handle.field_validation_status(email.clone(), validator_id),
        Some(ValidationStatus::Pending)
    );
    assert!(handle.field_validation_errors(email).is_empty());
    assert!(handle.validation_errors().is_empty());
}

#[test]
fn dioxus_adapter_ignores_late_async_field_validation_errors_after_form_cleanup() {
    let probe = Rc::new(AsyncFieldValidationStaleProbe::default());
    let mut dom = VirtualDom::new_with_props(stale_async_field_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    drop(dom);
    probe.gate.complete(vec!["email_unavailable"]);

    assert_eq!(
        handle.field_validation_status(email.clone(), validator_id),
        Some(ValidationStatus::Pending)
    );
    assert!(handle.field_validation_errors(email).is_empty());
    assert!(handle.validation_errors().is_empty());
}

#[derive(Debug, Eq, PartialEq)]
struct AsyncFormValidationSnapshot {
    status: ValidationStatus,
    error_count: usize,
    form_error_count: usize,
    field_error_count: usize,
    can_submit: bool,
}

#[derive(Default)]
struct DebouncedAsyncFormValidationProbe {
    delay: AsyncGate<()>,
    validation: AsyncGate<Vec<FormValidationError<&'static str>>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validator_id: RefCell<Option<ValidatorId>>,
    start_status: RefCell<Option<ValidationStatus>>,
    captured_snapshot: RefCell<Option<String>>,
    snapshots: RefCell<Vec<AsyncFormValidationSnapshot>>,
}

fn debounced_async_form_validation_probe(probe: Rc<DebouncedAsyncFormValidationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "initial@example.com".to_owned(),
                })
                .with_validation_mode(ValidationMode::on_change());
            let delay = probe.delay.clone();
            let validation = probe.validation.clone();
            let captured_probe = Rc::clone(&probe);
            let validator_id = form
                .async_validator("account")
                .on(ValidationTrigger::Change)
                .debounce(move || delay.future())
                .check(move |snapshot| {
                    captured_probe
                        .captured_snapshot
                        .borrow_mut()
                        .replace(snapshot.value().email.clone());
                    validation.future()
                });

            probe.validator_id.borrow_mut().replace(validator_id);

            form
        }
    });
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    let input_email = email.clone();
    use_hook({
        let form = form.clone();

        move || {
            form.text(input_email.clone()).on_input("taken@example.com");
        }
    });

    let status = form
        .form_validation_status_by_id(validator_id)
        .expect("async form validator status should be readable");
    let error_count = form.validation_errors().len();
    let form_error_count = form.form_validation_errors().len();
    let field_error_count = form.field_validation_errors(email).len();
    let can_submit = form.can_submit();

    probe.start_status.borrow_mut().replace(status);
    probe.handle.borrow_mut().replace(form);
    probe
        .snapshots
        .borrow_mut()
        .push(AsyncFormValidationSnapshot {
            status,
            error_count,
            form_error_count,
            field_error_count,
            can_submit,
        });

    VNode::empty()
}

#[test]
fn dioxus_adapter_debounced_value_change_async_form_validation_updates_reactive_selectors() {
    let probe = Rc::new(DebouncedAsyncFormValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(debounced_async_form_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(
        *probe.start_status.borrow(),
        Some(ValidationStatus::Pending)
    );
    assert!(probe.captured_snapshot.borrow().is_none());
    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [AsyncFormValidationSnapshot {
            status: ValidationStatus::Pending,
            error_count: 0,
            form_error_count: 0,
            field_error_count: 0,
            can_submit: true,
        }]
    );

    probe.delay.complete(());
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.captured_snapshot.borrow().as_deref(),
        Some("taken@example.com")
    );

    probe.validation.complete(vec![
        FormValidationError::field(email.clone(), "email_unavailable"),
        FormValidationError::form("account_unavailable"),
    ]);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFormValidationSnapshot {
            status: ValidationStatus::Invalid,
            error_count: 2,
            form_error_count: 1,
            field_error_count: 1,
            can_submit: false,
        })
    );
    assert_eq!(
        handle.form_validation_errors()[0].error(),
        &"account_unavailable"
    );
    assert_eq!(
        handle.field_validation_errors(email)[0].error(),
        &"email_unavailable"
    );
}

#[derive(Default)]
struct AsyncFormValidationProbe {
    gate: AsyncGate<Vec<FormValidationError<&'static str>>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    validator_id: RefCell<Option<ValidatorId>>,
    start_status: RefCell<Option<ValidationStatus>>,
    captured_snapshot: RefCell<Option<String>>,
    snapshots: RefCell<Vec<AsyncFormValidationSnapshot>>,
}

fn reactive_async_form_validation_probe(probe: Rc<AsyncFormValidationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "taken@example.com".to_owned(),
                });
            let validator_id = form.write_advanced(|core| {
                core.register_async_form_validator_for_triggers(
                    "account",
                    ValidationTrigger::Manual,
                )
            });

            probe.validator_id.borrow_mut().replace(validator_id);

            form
        }
    });
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let gate = probe.gate.clone();
            let captured_probe = Rc::clone(&probe);
            let start_status = form.validate_async_form_validator(
                validator_id,
                ValidationTrigger::Manual,
                move |snapshot| {
                    captured_probe
                        .captured_snapshot
                        .borrow_mut()
                        .replace(snapshot.value().email.clone());
                    gate.future()
                },
            );

            probe.start_status.borrow_mut().replace(
                start_status.expect("async form validator should schedule through the adapter"),
            );
        }
    });

    let status = form
        .form_validation_status_by_id(validator_id)
        .expect("async form validator status should be readable");
    let error_count = form.validation_errors().len();
    let form_error_count = form.form_validation_errors().len();
    let field_error_count = form.field_validation_errors(email).len();
    let can_submit = form.can_submit();

    probe.handle.borrow_mut().replace(form);
    probe
        .snapshots
        .borrow_mut()
        .push(AsyncFormValidationSnapshot {
            status,
            error_count,
            form_error_count,
            field_error_count,
            can_submit,
        });

    VNode::empty()
}

#[derive(Default)]
struct FormErrorInvalidationProbe {
    gate: AsyncGate<Vec<FormValidationError<&'static str>>>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    form_error_counts: RefCell<Vec<usize>>,
}

fn form_error_invalidation_probe(probe: Rc<FormErrorInvalidationProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<SignupForm, &'static str> =
                FormHandle::new_with_error_type(SignupForm {
                    email: "stale@example.com".to_owned(),
                });
            let gate = probe.gate.clone();

            form.async_validator("account")
                .on(ValidationTrigger::Manual)
                .check(move |_snapshot| gate.future());

            form
        }
    });

    use_hook({
        let form = form.clone();

        move || form.validate_form(ValidationTrigger::Manual)
    });

    let form_error_count = form.form_validation_errors().len();

    probe.handle.borrow_mut().replace(form);
    probe.form_error_counts.borrow_mut().push(form_error_count);

    VNode::empty()
}

#[test]
fn dioxus_adapter_async_form_validation_updates_reactive_selectors() {
    let probe = Rc::new(AsyncFormValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(reactive_async_form_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(
        *probe.start_status.borrow(),
        Some(ValidationStatus::Pending)
    );
    assert_eq!(
        probe.captured_snapshot.borrow().as_deref(),
        Some("taken@example.com")
    );
    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [AsyncFormValidationSnapshot {
            status: ValidationStatus::Pending,
            error_count: 0,
            form_error_count: 0,
            field_error_count: 0,
            can_submit: true,
        }]
    );

    probe.gate.complete(vec![
        FormValidationError::field(email.clone(), "email_unavailable"),
        FormValidationError::form("account_unavailable"),
    ]);
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFormValidationSnapshot {
            status: ValidationStatus::Invalid,
            error_count: 2,
            form_error_count: 1,
            field_error_count: 1,
            can_submit: false,
        })
    );

    let errors: Vec<_> = handle
        .validation_errors()
        .into_iter()
        .map(|error| {
            (
                error.target(),
                error.source().as_str().to_owned(),
                *error.error(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        vec![
            (
                ValidationTarget::Field(email.identity()),
                "account".to_owned(),
                "email_unavailable",
            ),
            (
                ValidationTarget::Form,
                "account".to_owned(),
                "account_unavailable",
            ),
        ]
    );
}

#[test]
fn dioxus_async_form_validator_accepts_non_send_future() {
    let probe = Rc::new(AsyncFormValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(reactive_async_form_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFormValidationSnapshot {
            status: ValidationStatus::Pending,
            error_count: 0,
            form_error_count: 0,
            field_error_count: 0,
            can_submit: true,
        })
    );

    // AsyncGateFuture carries Rc<RefCell<_>>, so this exercises a non-Send validator future.
    probe.gate.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFormValidationSnapshot {
            status: ValidationStatus::Valid,
            error_count: 0,
            form_error_count: 0,
            field_error_count: 0,
            can_submit: true,
        })
    );
}

#[test]
fn dioform_error_selectors_rerender_when_field_edit_stales_async_form_errors() {
    let probe = Rc::new(FormErrorInvalidationProbe::default());
    let mut dom = VirtualDom::new_with_props(form_error_invalidation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert_eq!(probe.form_error_counts.borrow().as_slice(), [0]);

    probe
        .gate
        .complete(vec![FormValidationError::form("account_unavailable")]);
    dom.render_immediate_to_vec();

    assert_eq!(probe.form_error_counts.borrow().last(), Some(&1));

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    handle
        .text(SignupForm::fields().email())
        .on_input("fresh@example.com");
    dom.render_immediate_to_vec();

    assert_eq!(probe.form_error_counts.borrow().last(), Some(&0));
}

#[test]
fn dioxus_adapter_ignores_stale_async_form_validation_after_edit() {
    let probe = Rc::new(AsyncFormValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(reactive_async_form_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();

    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [AsyncFormValidationSnapshot {
            status: ValidationStatus::Pending,
            error_count: 0,
            form_error_count: 0,
            field_error_count: 0,
            can_submit: true,
        }]
    );

    handle.text(email.clone()).on_input("fresh@example.com");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&AsyncFormValidationSnapshot {
            status: ValidationStatus::Stale,
            error_count: 0,
            form_error_count: 0,
            field_error_count: 0,
            can_submit: true,
        })
    );

    let snapshot_count_after_edit = probe.snapshots.borrow().len();

    probe
        .gate
        .complete(vec![FormValidationError::form("account_unavailable")]);
    dom.render_immediate_to_vec();

    assert_eq!(probe.snapshots.borrow().len(), snapshot_count_after_edit);
    assert_eq!(handle.field_value(email), "fresh@example.com");
    assert!(handle.form_validation_errors().is_empty());
    assert!(handle.validation_errors().is_empty());
}

#[test]
fn dioxus_adapter_ignores_late_async_form_validation_success_after_form_cleanup() {
    let probe = Rc::new(AsyncFormValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(reactive_async_form_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    assert_eq!(
        handle.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Pending)
    );

    drop(dom);
    probe.gate.complete(Vec::new());

    assert_eq!(
        handle.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Pending)
    );
    assert!(handle.form_validation_errors().is_empty());
    assert!(handle.validation_errors().is_empty());
}

#[test]
fn dioxus_adapter_ignores_late_async_form_validation_errors_after_form_cleanup() {
    let probe = Rc::new(AsyncFormValidationProbe::default());
    let mut dom =
        VirtualDom::new_with_props(reactive_async_form_validation_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let email = SignupForm::fields().email();
    let validator_id = probe
        .validator_id
        .borrow()
        .expect("probe should store validator id");

    drop(dom);
    probe.gate.complete(vec![
        FormValidationError::field(email.clone(), "email_unavailable"),
        FormValidationError::form("account_unavailable"),
    ]);

    assert_eq!(
        handle.form_validation_status_by_id(validator_id),
        Some(ValidationStatus::Pending)
    );
    assert!(handle.field_validation_errors(email).is_empty());
    assert!(handle.form_validation_errors().is_empty());
    assert!(handle.validation_errors().is_empty());
}

#[test]
fn dioxus_adapter_field_updates_drop_non_comparable_stale_submit_errors() {
    let handle: FormHandle<UploadForm, &'static str> =
        FormHandle::new_with_error_type(UploadForm {
            token: UploadToken {
                token: "initial".to_owned(),
            },
        });
    let token = UploadForm::fields().token();

    let submitted = match handle.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    handle.set_user_field(
        token.clone(),
        UploadToken {
            token: "changed".to_owned(),
        },
    );

    assert_eq!(handle.field_value(token.clone()).token, "changed");
    assert!(handle.finish_submission_with_errors(
        submitted,
        SubmitError::field_identity(token.identity(), "upload_failed"),
    ));
    assert!(handle.field_validation_errors(token.clone()).is_empty());

    let submitted = match handle.begin_submission() {
        SubmitAttempt::Started(submitted) => submitted,
        other => panic!("expected submission to start, got {other:?}"),
    };

    assert!(handle.finish_submission_with_errors(
        submitted,
        SubmitError::field_identity(token.identity(), "upload_failed"),
    ));
    assert_eq!(
        handle.field_validation_errors(token)[0].error(),
        &"upload_failed"
    );
}

#[derive(Default)]
struct AsyncSubmitCleanupProbe {
    gate: AsyncGate<()>,
    handle: RefCell<Option<FormHandle<SignupForm, &'static str>>>,
    submitted: RefCell<Option<SubmissionSnapshot<SignupForm>>>,
}

fn async_submit_cleanup_probe(probe: Rc<AsyncSubmitCleanupProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new_with_error_type(SignupForm {
            email: "taken@example.com".to_owned(),
        })
    });

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let gate = probe.gate.clone();
            let submit_probe = Rc::clone(&probe);
            let result =
                form.managed_submit()
                    .on_submit_async(managed_submit_event(), move |submitted| {
                        submit_probe
                            .submitted
                            .borrow_mut()
                            .replace(submitted.clone());
                        gate.future()
                    });

            assert_eq!(result, SubmitResult::Started);
        }
    });

    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

#[test]
fn dioxus_managed_async_submit_ignores_late_success_after_form_cleanup() {
    let probe = Rc::new(AsyncSubmitCleanupProbe::default());
    let mut dom = VirtualDom::new_with_props(async_submit_cleanup_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert!(handle.is_submitting());

    drop(dom);
    probe.gate.complete(());

    assert!(!handle.finish_submission_success());
    assert!(handle.is_submitting());
    assert!(handle.validation_errors().is_empty());
}

#[test]
fn dioxus_managed_async_submit_ignores_late_structured_errors_after_form_cleanup() {
    let probe = Rc::new(AsyncSubmitCleanupProbe::default());
    let mut dom = VirtualDom::new_with_props(async_submit_cleanup_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let submitted = probe
        .submitted
        .borrow()
        .as_ref()
        .expect("async handler should receive submitted snapshot")
        .clone();
    let email = SignupForm::fields().email();

    handle.text(email.clone()).on_input("new@example.com");
    assert_eq!(handle.field_value(email.clone()), "new@example.com");

    drop(dom);

    assert!(!handle.finish_submission_with_errors(
        submitted,
        SubmitError::field(email.clone(), "email_unavailable"),
    ));
    assert!(handle.is_submitting());
    assert!(handle.field_validation_errors(email.clone()).is_empty());
    assert!(handle.validation_errors().is_empty());
    assert_eq!(handle.field_value(email), "new@example.com");
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct AccountForm {
    age: u8,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct DualNumberForm {
    age: u8,
    score: u8,
}

#[derive(Clone, Debug, Eq, Form, PartialEq)]
struct OptionalAccountForm {
    age: Option<u8>,
}

fn parse_upload_token(value: &str) -> Result<UploadToken, &'static str> {
    let token = value
        .strip_prefix("tok:")
        .ok_or("token must start with tok:")?;

    if token
        .chars()
        .all(|character| character.is_ascii_alphanumeric())
        && !token.is_empty()
    {
        Ok(UploadToken {
            token: token.to_owned(),
        })
    } else {
        Err("token must be alphanumeric")
    }
}

fn format_upload_token(value: &UploadToken) -> String {
    format!("tok:{}", value.token)
}

fn parse_date_ymd(value: &str) -> Result<DateYmd, &'static str> {
    if value.len() != 10 || &value[4..5] != "-" || &value[7..8] != "-" {
        return Err("date must use YYYY-MM-DD");
    }

    let year = value[0..4]
        .parse::<u16>()
        .map_err(|_| "date must use YYYY-MM-DD")?;
    let month = value[5..7]
        .parse::<u8>()
        .map_err(|_| "date must use YYYY-MM-DD")?;
    let day = value[8..10]
        .parse::<u8>()
        .map_err(|_| "date must use YYYY-MM-DD")?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err("date is outside the supported calendar range");
    }

    Ok(DateYmd { year, month, day })
}

fn format_date_ymd(value: &DateYmd) -> String {
    format!("{:04}-{:02}-{:02}", value.year, value.month, value.day)
}

impl std::fmt::Display for DateYmd {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&format_date_ymd(self))
    }
}

impl std::str::FromStr for DateYmd {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_date_ymd(value)
    }
}

fn controlled_upload_token(
    handle: &FormHandle<UploadForm, &'static str>,
) -> ParsedTextBinding<UploadForm, UploadToken, &'static str> {
    handle.parsed_text_with(
        UploadForm::fields().token(),
        parse_upload_token,
        format_upload_token,
    )
}

fn date(
    handle: &FormHandle<DateForm, &'static str>,
    path: dioform::FieldPath<DateForm, DateYmd>,
) -> ParsedTextBinding<DateForm, DateYmd, &'static str> {
    handle.date_with(path, parse_date_ymd, format_date_ymd)
}

#[derive(Debug, Eq, PartialEq)]
struct ParseSelectorSnapshot {
    rendered_value: String,
    parse_error_count: usize,
    can_submit: bool,
    aria_invalid: bool,
}

#[derive(Default)]
struct ParseSelectorProbe {
    handle: RefCell<Option<FormHandle<AccountForm>>>,
    age: RefCell<Option<ParsedTextBinding<AccountForm, u8>>>,
    snapshots: RefCell<Vec<ParseSelectorSnapshot>>,
}

fn reactive_parse_selector_probe(probe: Rc<ParseSelectorProbe>) -> Element {
    let form = use_form_handle(|| FormHandle::new(AccountForm { age: 42 }));
    let age_path = AccountForm::fields().age();
    let age = use_number(form.clone(), age_path.clone());

    let rendered_value = age.value();
    let parse_error_count = form.field_parse_errors(age_path).len();
    let can_submit = form.can_submit();
    let aria_invalid = age.accessibility().aria_invalid();

    probe.handle.borrow_mut().replace(form);
    probe.age.borrow_mut().replace(age);
    probe.snapshots.borrow_mut().push(ParseSelectorSnapshot {
        rendered_value,
        parse_error_count,
        can_submit,
        aria_invalid,
    });

    VNode::empty()
}

#[derive(Default)]
struct ManagedAsyncSubmitParseProbe {
    validation: AsyncGate<Vec<&'static str>>,
    submit: AsyncGate<()>,
    handle: RefCell<Option<FormHandle<AccountForm, &'static str>>>,
    age: RefCell<Option<ParsedTextBinding<AccountForm, u8, &'static str>>>,
    submit_result: RefCell<Option<SubmitResult>>,
    submit_calls: Cell<u32>,
}

fn managed_async_submit_parse_probe(probe: Rc<ManagedAsyncSubmitParseProbe>) -> Element {
    let form = use_form_handle({
        let probe = Rc::clone(&probe);

        move || {
            let form: FormHandle<AccountForm, &'static str> =
                FormHandle::new_with_error_type(AccountForm { age: 42 });
            let age = AccountForm::fields().age();
            let validation = probe.validation.clone();

            form.field(age)
                .async_validator("age_check")
                .on(ValidationTrigger::Submit)
                .check(move |_value, _snapshot| validation.future());

            form
        }
    });
    let age_path = AccountForm::fields().age();
    let age = use_parsed_text(form.clone(), age_path);

    use_hook({
        let form = form.clone();
        let probe = Rc::clone(&probe);

        move || {
            let submit = probe.submit.clone();
            let submit_probe = Rc::clone(&probe);
            let result =
                form.managed_submit()
                    .on_submit_async(managed_submit_event(), move |_submitted| {
                        submit_probe
                            .submit_calls
                            .set(submit_probe.submit_calls.get() + 1);
                        submit.future()
                    });

            probe.submit_result.borrow_mut().replace(result);
        }
    });

    probe.handle.borrow_mut().replace(form);
    probe.age.borrow_mut().replace(age);

    VNode::empty()
}

#[derive(Default)]
struct DateHookProbe {
    handle: RefCell<Option<FormHandle<DateForm>>>,
    rendered_value: RefCell<Option<String>>,
}

fn date_hook_probe(probe: Rc<DateHookProbe>) -> Element {
    let form = use_form_handle(|| {
        FormHandle::new(DateForm {
            check_in: DateYmd {
                year: 2026,
                month: 7,
                day: 12,
            },
            check_out: DateYmd {
                year: 2026,
                month: 7,
                day: 15,
            },
        })
    });
    let check_in = use_date(form.clone(), DateForm::fields().check_in());

    probe.rendered_value.borrow_mut().replace(check_in.value());
    probe.handle.borrow_mut().replace(form);

    VNode::empty()
}

#[test]
fn selector_reads_update_for_parse_blockers_and_reset() {
    let probe = Rc::new(ParseSelectorProbe::default());
    let mut dom = VirtualDom::new_with_props(reactive_parse_selector_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert_eq!(
        probe.snapshots.borrow().as_slice(),
        [ParseSelectorSnapshot {
            rendered_value: "42".to_owned(),
            parse_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        }]
    );

    let age = probe
        .age
        .borrow()
        .as_ref()
        .expect("probe should expose its parsed binding")
        .clone();

    age.on_input("not-a-number");
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&ParseSelectorSnapshot {
            rendered_value: "not-a-number".to_owned(),
            parse_error_count: 1,
            can_submit: false,
            aria_invalid: true,
        })
    );

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    handle.reset();
    dom.render_immediate_to_vec();

    assert_eq!(
        probe.snapshots.borrow().last(),
        Some(&ParseSelectorSnapshot {
            rendered_value: "42".to_owned(),
            parse_error_count: 0,
            can_submit: true,
            aria_invalid: false,
        })
    );
}

struct FieldParseSelectorProbe {
    form: FormHandle<DualNumberForm>,
    age: ParsedTextBinding<DualNumberForm, u8>,
    score: ParsedTextBinding<DualNumberForm, u8>,
    age_parse_error_counts: RefCell<Vec<usize>>,
    score_parse_error_counts: RefCell<Vec<usize>>,
}

impl FieldParseSelectorProbe {
    fn new() -> Self {
        let form = FormHandle::new(DualNumberForm { age: 42, score: 7 });
        let fields = DualNumberForm::fields();
        let age = form.number(fields.age());
        let score = form.number(fields.score());

        Self {
            form,
            age,
            score,
            age_parse_error_counts: RefCell::new(Vec::new()),
            score_parse_error_counts: RefCell::new(Vec::new()),
        }
    }
}

fn age_parse_selector_probe(probe: Rc<FieldParseSelectorProbe>) -> Element {
    let count = probe
        .form
        .field_parse_errors(DualNumberForm::fields().age())
        .len();

    probe.age_parse_error_counts.borrow_mut().push(count);

    VNode::empty()
}

fn score_parse_selector_probe(probe: Rc<FieldParseSelectorProbe>) -> Element {
    let count = probe
        .form
        .field_parse_errors(DualNumberForm::fields().score())
        .len();

    probe.score_parse_error_counts.borrow_mut().push(count);

    VNode::empty()
}

#[test]
fn field_parse_selectors_do_not_rerender_unrelated_field_readers() {
    let probe = Rc::new(FieldParseSelectorProbe::new());
    let mut age_dom = VirtualDom::new_with_props(age_parse_selector_probe, Rc::clone(&probe));
    let mut score_dom = VirtualDom::new_with_props(score_parse_selector_probe, Rc::clone(&probe));

    age_dom.rebuild_in_place();
    score_dom.rebuild_in_place();

    assert_eq!(probe.age_parse_error_counts.borrow().as_slice(), [0]);
    assert_eq!(probe.score_parse_error_counts.borrow().as_slice(), [0]);

    probe.age.on_input("not-a-number");
    age_dom.render_immediate_to_vec();
    score_dom.render_immediate_to_vec();

    assert_eq!(probe.age_parse_error_counts.borrow().as_slice(), [0, 1]);
    assert_eq!(probe.score_parse_error_counts.borrow().as_slice(), [0]);

    probe.score.on_input("not-a-number");
    age_dom.render_immediate_to_vec();
    score_dom.render_immediate_to_vec();

    assert_eq!(probe.age_parse_error_counts.borrow().len(), 2);
    assert_eq!(probe.score_parse_error_counts.borrow().as_slice(), [0, 1]);
}

#[test]
fn dioxus_parsed_text_binding_keeps_raw_input_and_parse_errors_separate() {
    let handle: FormHandle<AccountForm, &'static str> =
        FormHandle::new_with_error_type(AccountForm { age: 42 });
    let age_path = AccountForm::fields().age();
    let age = handle.parsed_text(age_path.clone());

    assert_eq!(age.name(), "age");
    assert_eq!(age.value(), "42");

    age.on_input("not-a-number");

    assert_eq!(age.value(), "not-a-number");
    assert_eq!(handle.field_value(age_path.clone()), 42);
    assert!(handle.is_field_touched(age_path.clone()));
    assert!(!handle.is_field_blurred(age_path.clone()));
    assert!(handle.validation_errors().is_empty());

    let parse_error = age.parse_error().expect("parse error should be exposed");
    assert_eq!(parse_error.field_identity().as_str(), "age");
    assert_eq!(parse_error.raw_value(), "not-a-number");
    assert!(!parse_error.message().is_empty());
    assert_eq!(
        handle.field_parse_errors(age_path.clone()),
        vec![parse_error.clone()]
    );
    assert_eq!(handle.parse_errors(), vec![parse_error]);

    age.on_blur();

    assert!(handle.is_field_blurred(age_path));
}

#[test]
fn dioxus_parsed_text_blur_with_parse_error_does_not_validate_stale_typed_value() {
    let validation_runs = Rc::new(Cell::new(0));
    let validation_runs_for_validator = Rc::clone(&validation_runs);
    let seen_values = Rc::new(RefCell::new(Vec::new()));
    let seen_values_for_validator = Rc::clone(&seen_values);
    let handle: FormHandle<AccountForm, &'static str> =
        FormHandle::new_with_error_type(AccountForm { age: 42 });
    let age_path = AccountForm::fields().age();
    let age = handle.parsed_text(age_path.clone());

    handle
        .field(age_path.clone())
        .validator("adult_on_blur")
        .on(ValidationTrigger::Blur)
        .check(move |value, context| {
            validation_runs_for_validator.set(validation_runs_for_validator.get() + 1);
            seen_values_for_validator.borrow_mut().push(*value);
            assert_eq!(context.trigger(), ValidationTrigger::Blur);

            if *value < 18 {
                vec!["must be adult"]
            } else {
                Vec::new()
            }
        });

    age.on_input("not-a-number");
    age.on_blur();

    assert!(handle.is_field_touched(age_path.clone()));
    assert!(handle.is_field_blurred(age_path.clone()));
    assert_eq!(handle.field_value(age_path.clone()), 42);
    assert!(age.parse_error().is_some());
    assert_eq!(validation_runs.get(), 0);
    assert!(seen_values.borrow().is_empty());
    assert!(handle.field_validation_errors(age_path.clone()).is_empty());

    age.on_input("17");
    age.on_blur();

    assert!(age.parse_error().is_none());
    assert_eq!(handle.field_value(age_path.clone()), 17);
    assert_eq!(validation_runs.get(), 1);
    assert_eq!(seen_values.borrow().as_slice(), &[17]);
    assert_eq!(
        handle.field_validation_errors(age_path)[0].error(),
        &"must be adult"
    );
}

#[test]
fn dioxus_parsed_text_binding_recovers_after_successful_parse() {
    let handle = FormHandle::new(AccountForm { age: 42 });
    let age_path = AccountForm::fields().age();
    let age = handle.parsed_text(age_path.clone());

    age.on_input("not-a-number");
    age.on_input("7");

    assert_eq!(age.value(), "7");
    assert_eq!(handle.field_value(age_path), 7);
    assert!(age.parse_error().is_none());
    assert!(handle.parse_errors().is_empty());
    assert!(handle.can_submit());
}

#[test]
fn dioxus_parsed_text_successful_parse_runs_value_change_validation() {
    let validation_runs = Rc::new(Cell::new(0));
    let validation_runs_for_validator = Rc::clone(&validation_runs);
    let handle: FormHandle<AccountForm, &'static str> =
        FormHandle::new_with_error_type(AccountForm { age: 42 })
            .with_validation_mode(ValidationMode::on_change());
    let age_path = AccountForm::fields().age();
    let age = handle.number(age_path.clone());

    handle
        .field(age_path.clone())
        .validator("adult")
        .on(ValidationTrigger::Change)
        .check(move |value, context| {
            validation_runs_for_validator.set(validation_runs_for_validator.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);

            if *value < 18 {
                vec!["must be adult"]
            } else {
                Vec::new()
            }
        });

    age.on_input("17");

    assert_eq!(validation_runs.get(), 1);
    assert_eq!(handle.field_value(age_path.clone()), 17);
    assert_eq!(
        handle.field_validation_errors(age_path.clone())[0].error(),
        &"must be adult"
    );

    age.on_input("18");

    assert_eq!(validation_runs.get(), 2);
    assert_eq!(handle.field_value(age_path.clone()), 18);
    assert!(handle.field_validation_errors(age_path).is_empty());
}

#[test]
fn dioxus_parse_blocked_submit_enters_submit_then_revalidate_phase() {
    let validation_runs = Rc::new(Cell::new(0));
    let validation_runs_for_validator = Rc::clone(&validation_runs);
    let handle: FormHandle<AccountForm, &'static str> =
        FormHandle::new_with_error_type(AccountForm { age: 42 })
            .with_validation_mode(ValidationMode::submit_then_revalidate());
    let age_path = AccountForm::fields().age();
    let age = handle.number(age_path.clone());

    handle
        .field(age_path.clone())
        .validator("adult")
        .on(ValidationTrigger::Change)
        .check(move |value, context| {
            validation_runs_for_validator.set(validation_runs_for_validator.get() + 1);
            assert_eq!(context.trigger(), ValidationTrigger::Change);

            if *value < 18 {
                vec!["must be adult"]
            } else {
                Vec::new()
            }
        });

    age.on_input("not-a-number");

    assert!(age.parse_error().is_some());
    assert_eq!(validation_runs.get(), 0);
    assert_eq!(handle.submit_attempt_count(), 0);

    assert_eq!(
        handle
            .managed_submit()
            .on_submit(managed_submit_event(), |_submitted| ()),
        SubmitResult::Blocked(SubmitBlocker::ParseErrors)
    );
    assert_eq!(handle.submit_attempt_count(), 1);

    age.on_input("17");

    assert!(age.parse_error().is_none());
    assert_eq!(handle.field_value(age_path.clone()), 17);
    assert_eq!(validation_runs.get(), 1);
    assert_eq!(
        handle.field_validation_errors(age_path)[0].error(),
        &"must be adult"
    );
}

#[test]
fn dioxus_number_binding_parses_success_and_empty_input_as_parse_error() {
    let handle: FormHandle<AccountForm, &'static str> =
        FormHandle::new_with_error_type(AccountForm { age: 42 });
    let age_path = AccountForm::fields().age();
    let age = handle.number(age_path.clone());

    age.on_input("7");

    assert_eq!(age.value(), "7");
    assert_eq!(handle.field_value(age_path.clone()), 7);
    assert!(handle.is_field_touched(age_path.clone()));
    assert!(age.parse_error().is_none());
    assert!(handle.validation_errors().is_empty());

    age.on_input("");

    assert_eq!(age.value(), "");
    assert_eq!(handle.field_value(age_path.clone()), 7);
    assert!(!handle.can_submit());
    assert!(handle.validation_errors().is_empty());

    let parse_error = age
        .parse_error()
        .expect("empty non-optional number input should be a parse error");
    assert_eq!(parse_error.field_identity().as_str(), "age");
    assert_eq!(parse_error.raw_value(), "");
    assert_eq!(handle.field_parse_errors(age_path), vec![parse_error]);
}

#[test]
fn dioxus_number_binding_uses_parsed_input_blockers_and_cleanup() {
    let handle = FormHandle::new(AccountForm { age: 42 });
    let age_path = AccountForm::fields().age();
    let age = handle.number(age_path.clone());

    age.on_input("not-a-number");

    assert_eq!(age.value(), "not-a-number");
    assert_eq!(handle.field_value(age_path.clone()), 42);
    assert_eq!(
        handle.submit_availability().blockers(),
        &[SubmitBlocker::ParseErrors]
    );

    handle.reset();

    assert_eq!(age.value(), "42");
    assert!(age.parse_error().is_none());
    assert!(handle.can_submit());

    age.on_input("not-a-number");
    handle.reinitialize(AccountForm { age: 9 });

    assert_eq!(age.value(), "9");
    assert_eq!(handle.field_value(age_path.clone()), 9);
    assert!(age.parse_error().is_none());
    assert!(handle.can_submit());

    {
        let mounted_age = handle.number(age_path.clone());
        mounted_age.on_input("not-a-number");

        assert_eq!(handle.field_value(age_path.clone()), 9);
        assert!(!handle.can_submit());
    }

    assert_eq!(handle.field_value(age_path), 9);
    assert!(handle.parse_errors().is_empty());
    assert!(handle.can_submit());
}

#[test]
fn dioform_handle_state_snapshot_does_not_transfer_adapter_parse_state() {
    let source = FormHandle::new(AccountForm { age: 42 });
    let age_path = AccountForm::fields().age();
    let age = source.number(age_path.clone());

    age.on_input("not-a-number");

    assert_eq!(age.value(), "not-a-number");
    assert_eq!(source.field_value(age_path.clone()), 42);
    assert!(age.parse_error().is_some());
    assert_eq!(
        source.submit_availability().blockers(),
        &[SubmitBlocker::ParseErrors]
    );

    let snapshot = source.state_snapshot();
    let restored = FormHandle::new(AccountForm { age: 0 });
    let restored_age = restored.number(age_path.clone());

    restored_age.on_input("still-not-a-number");

    assert_eq!(restored_age.value(), "still-not-a-number");
    assert_eq!(restored.field_value(age_path.clone()), 0);
    assert!(restored_age.parse_error().is_some());
    assert_eq!(
        restored.submit_availability().blockers(),
        &[SubmitBlocker::ParseErrors]
    );

    restored
        .restore_state_snapshot(snapshot)
        .expect("core form state snapshot should restore through the handle");

    assert_eq!(restored.field_value(age_path), 42);
    assert_eq!(restored_age.value(), "42");
    assert!(restored_age.parse_error().is_none());
    assert!(restored.parse_errors().is_empty());
    assert!(restored.can_submit());
}

#[test]
fn dioxus_number_binding_allows_application_defined_optional_empty_behavior() {
    let handle = FormHandle::new(OptionalAccountForm { age: Some(42) });
    let age_path = OptionalAccountForm::fields().age();
    let age = handle.number_with(
        age_path.clone(),
        |value| {
            if value.is_empty() {
                Ok(None)
            } else {
                value.parse::<u8>().map(Some)
            }
        },
        |value| value.map(|value| value.to_string()).unwrap_or_default(),
    );

    age.on_input("");

    assert_eq!(age.value(), "");
    assert_eq!(handle.field_value(age_path.clone()), None);
    assert!(age.parse_error().is_none());
    assert!(handle.can_submit());

    age.on_input("not-a-number");

    assert_eq!(age.value(), "not-a-number");
    assert_eq!(handle.field_value(age_path), None);
    assert!(age.parse_error().is_some());
    assert!(!handle.can_submit());
}

#[test]
fn dioxus_custom_parsed_text_binding_uses_explicit_parser_and_formatter() {
    let handle: FormHandle<UploadForm, &'static str> =
        FormHandle::new_with_error_type(UploadForm {
            token: UploadToken {
                token: "initial".to_owned(),
            },
        });
    let token_path = UploadForm::fields().token();
    let token = controlled_upload_token(&handle);

    assert_eq!(token.name(), "token");
    assert_eq!(token.value(), "tok:initial");

    token.on_input("tok:updated");

    assert_eq!(token.value(), "tok:updated");
    assert_eq!(
        handle.field_value(token_path.clone()),
        UploadToken {
            token: "updated".to_owned()
        }
    );
    assert!(handle.is_field_touched(token_path.clone()));
    assert!(token.parse_error().is_none());
    assert!(handle.validation_errors().is_empty());

    token.on_input("bad raw token");

    assert_eq!(token.value(), "bad raw token");
    assert_eq!(
        handle.field_value(token_path.clone()),
        UploadToken {
            token: "updated".to_owned()
        }
    );
    assert!(handle.validation_errors().is_empty());
    assert!(!handle.can_submit());

    let parse_error = token
        .parse_error()
        .expect("parse error should be exposed for custom parser failures");
    assert_eq!(parse_error.field_identity().as_str(), "token");
    assert_eq!(parse_error.raw_value(), "bad raw token");
    assert_eq!(parse_error.message(), "token must start with tok:");
    assert_eq!(handle.field_parse_errors(token_path), vec![parse_error]);
    assert!(token.accessibility().has_parse_errors());
}

#[test]
fn dioxus_custom_parsed_text_binding_recovers_and_blocks_submit() {
    let handle: FormHandle<UploadForm, &'static str> =
        FormHandle::new_with_error_type(UploadForm {
            token: UploadToken {
                token: "initial".to_owned(),
            },
        });
    let token_path = UploadForm::fields().token();
    let token = controlled_upload_token(&handle);
    let called = Cell::new(false);

    token.on_input("bad raw token");

    assert_eq!(
        handle
            .managed_submit()
            .on_submit(managed_submit_event(), |_submitted| called.set(true)),
        SubmitResult::Blocked(SubmitBlocker::ParseErrors)
    );
    assert!(!called.get());
    assert_eq!(handle.field_value(token_path.clone()).token, "initial");

    token.on_input("tok:recovered");

    assert!(token.parse_error().is_none());
    assert!(handle.can_submit());
    assert_eq!(handle.field_value(token_path).token, "recovered");
}

#[test]
fn dioxus_date_binding_uses_explicit_parser_and_formatter() {
    let observer_events = Rc::new(RefCell::new(Vec::new()));
    let captured_events = Rc::clone(&observer_events);
    let handle: FormHandle<DateForm, &'static str> = FormHandle::new_with_error_type(DateForm {
        check_in: DateYmd {
            year: 2026,
            month: 7,
            day: 12,
        },
        check_out: DateYmd {
            year: 2026,
            month: 7,
            day: 15,
        },
    });
    let check_in_path = DateForm::fields().check_in();
    let check_in = date(&handle, check_in_path.clone());

    handle.write_advanced(|core| {
        core.observe(move |event| captured_events.borrow_mut().push(event.clone()))
    });

    assert_eq!(check_in.name(), "check_in");
    assert_eq!(check_in.value(), "2026-07-12");

    check_in.on_input("2026-08-01");

    assert_eq!(check_in.value(), "2026-08-01");
    assert_eq!(
        handle.field_value(check_in_path),
        DateYmd {
            year: 2026,
            month: 8,
            day: 1,
        }
    );
    assert!(check_in.parse_error().is_none());
    assert!(handle.validation_errors().is_empty());
    assert!(observer_events
        .borrow()
        .iter()
        .any(|event| matches!(event, FormObserverEvent::FieldUpdated { field, origin: FieldUpdateOrigin::User, .. } if field.field_name() == "check_in")));
}

#[test]
fn dioxus_date_binding_uses_fromstr_and_tostring_convenience() {
    let handle: FormHandle<DateForm, &'static str> = FormHandle::new_with_error_type(DateForm {
        check_in: DateYmd {
            year: 2026,
            month: 7,
            day: 12,
        },
        check_out: DateYmd {
            year: 2026,
            month: 7,
            day: 15,
        },
    });
    let check_in_path = DateForm::fields().check_in();
    let check_in = handle.date(check_in_path.clone());

    assert_eq!(check_in.name(), "check_in");
    assert_eq!(check_in.value(), "2026-07-12");

    check_in.on_input("2026-08-01");

    assert_eq!(check_in.value(), "2026-08-01");
    assert_eq!(
        handle.field_value(check_in_path.clone()),
        DateYmd {
            year: 2026,
            month: 8,
            day: 1,
        }
    );
    assert!(check_in.parse_error().is_none());

    check_in.on_input("not-a-date");

    assert_eq!(check_in.value(), "not-a-date");
    assert_eq!(
        handle.field_value(check_in_path),
        DateYmd {
            year: 2026,
            month: 8,
            day: 1,
        }
    );
    assert_eq!(
        check_in
            .parse_error()
            .expect("invalid date should expose parse error")
            .message(),
        "date must use YYYY-MM-DD"
    );
}

#[test]
fn dioxus_date_hook_creates_fromstr_and_tostring_binding() {
    let probe = Rc::new(DateHookProbe::default());
    let mut dom = VirtualDom::new_with_props(date_hook_probe, Rc::clone(&probe));

    dom.rebuild_in_place();

    assert_eq!(probe.rendered_value.borrow().as_deref(), Some("2026-07-12"));
    assert_eq!(
        probe
            .handle
            .borrow()
            .as_ref()
            .expect("probe should expose its form handle")
            .field_value(DateForm::fields().check_in()),
        DateYmd {
            year: 2026,
            month: 7,
            day: 12,
        }
    );
}

#[test]
fn dioxus_date_binding_separates_parse_failures_from_validation() {
    let handle: FormHandle<DateForm, &'static str> = FormHandle::new_with_error_type(DateForm {
        check_in: DateYmd {
            year: 2026,
            month: 7,
            day: 12,
        },
        check_out: DateYmd {
            year: 2026,
            month: 7,
            day: 15,
        },
    });
    let fields = DateForm::fields();
    let check_in_path = fields.check_in();
    let check_out_path = fields.check_out();
    let check_in = date(&handle, check_in_path.clone());
    let check_out = date(&handle, check_out_path.clone());

    let date_order_field = check_out_path.clone();
    handle.validator("date_order").check(move |context| {
        let form = context.form();

        if form.check_out <= form.check_in {
            vec![FormValidationError::field(
                date_order_field.clone(),
                "date_order",
            )]
        } else {
            Vec::new()
        }
    });

    check_in.on_input("not-a-date");

    assert_eq!(check_in.value(), "not-a-date");
    assert_eq!(
        handle.field_value(check_in_path.clone()),
        DateYmd {
            year: 2026,
            month: 7,
            day: 12,
        }
    );
    assert!(handle.validation_errors().is_empty());
    assert_eq!(
        handle.submit_availability().blockers(),
        &[SubmitBlocker::ParseErrors]
    );

    let parse_error = check_in
        .parse_error()
        .expect("date parse error should be exposed");
    assert_eq!(parse_error.raw_value(), "not-a-date");
    assert_eq!(parse_error.message(), "date must use YYYY-MM-DD");
    assert_eq!(handle.field_parse_errors(check_in_path), vec![parse_error]);

    check_in.on_input("2026-07-12");
    check_out.on_input("2026-07-10");
    handle.validate_all(ValidationTrigger::Manual);

    assert!(handle.parse_errors().is_empty());
    assert_eq!(handle.validation_errors().len(), 1);
    assert_eq!(
        handle.validation_errors()[0].target(),
        ValidationTarget::Field(check_out_path.identity())
    );
}

#[test]
fn parsed_submit_availability_tracks_multiple_bindings_and_independent_recovery() {
    let handle: FormHandle<DateForm, &'static str> = FormHandle::new_with_error_type(DateForm {
        check_in: DateYmd {
            year: 2026,
            month: 7,
            day: 12,
        },
        check_out: DateYmd {
            year: 2026,
            month: 7,
            day: 15,
        },
    });
    let fields = DateForm::fields();
    let check_in_path = fields.check_in();
    let check_out_path = fields.check_out();
    let check_in = date(&handle, check_in_path.clone());
    let check_out = date(&handle, check_out_path.clone());

    check_in.on_input("bad check-in");

    assert_eq!(handle.parse_errors().len(), 1);
    assert_eq!(
        handle.submit_availability().blockers(),
        &[SubmitBlocker::ParseErrors]
    );
    assert!(!handle.can_submit());

    check_out.on_input("bad check-out");

    assert_eq!(handle.parse_errors().len(), 2);
    assert_eq!(handle.field_parse_errors(check_in_path.clone()).len(), 1);
    assert_eq!(handle.field_parse_errors(check_out_path.clone()).len(), 1);
    assert_eq!(
        handle.submit_availability().blockers(),
        &[SubmitBlocker::ParseErrors]
    );

    check_in.on_input("2026-08-01");

    assert!(check_in.parse_error().is_none());
    assert!(handle.field_parse_errors(check_in_path).is_empty());
    assert_eq!(handle.field_parse_errors(check_out_path).len(), 1);
    assert_eq!(handle.parse_errors().len(), 1);
    assert!(!handle.can_submit());

    check_out.on_input("2026-08-04");

    assert!(handle.parse_errors().is_empty());
    assert!(handle.can_submit());
}

#[test]
fn dioxus_date_binding_blocks_submit_and_cleans_up_like_parsed_input() {
    let handle: FormHandle<DateForm, &'static str> = FormHandle::new_with_error_type(DateForm {
        check_in: DateYmd {
            year: 2026,
            month: 7,
            day: 12,
        },
        check_out: DateYmd {
            year: 2026,
            month: 7,
            day: 15,
        },
    });
    let check_in_path = DateForm::fields().check_in();
    let check_in = date(&handle, check_in_path.clone());
    let called = Cell::new(false);

    check_in.on_input("bad raw date");

    assert_eq!(
        handle
            .managed_submit()
            .on_submit(managed_submit_event(), |_submitted| called.set(true)),
        SubmitResult::Blocked(SubmitBlocker::ParseErrors)
    );
    assert!(!called.get());

    handle.reset();

    assert_eq!(check_in.value(), "2026-07-12");
    assert!(check_in.parse_error().is_none());
    assert!(handle.can_submit());

    check_in.on_input("bad raw date");
    handle.reinitialize(DateForm {
        check_in: DateYmd {
            year: 2026,
            month: 8,
            day: 1,
        },
        check_out: DateYmd {
            year: 2026,
            month: 8,
            day: 4,
        },
    });

    assert_eq!(check_in.value(), "2026-08-01");
    assert!(check_in.parse_error().is_none());
    assert!(handle.can_submit());

    {
        let mounted_check_in = date(&handle, check_in_path.clone());
        mounted_check_in.on_input("bad raw date");

        assert!(!handle.can_submit());
        assert_eq!(handle.parse_errors().len(), 1);
        assert_eq!(
            handle.field_value(check_in_path),
            DateYmd {
                year: 2026,
                month: 8,
                day: 1,
            }
        );
    }

    assert!(handle.parse_errors().is_empty());
    assert!(handle.can_submit());
}

#[test]
fn custom_parsed_programmatic_setter_updates_typed_value_without_user_metadata() {
    let validation_runs = Rc::new(Cell::new(0));
    let validation_runs_for_validator = Rc::clone(&validation_runs);
    let handle: FormHandle<UploadForm, &'static str> =
        FormHandle::new_with_error_type(UploadForm {
            token: UploadToken {
                token: "initial".to_owned(),
            },
        });
    let token_path = UploadForm::fields().token();
    let token = controlled_upload_token(&handle);

    handle
        .field(token_path.clone())
        .validator("token_check")
        .on(ValidationTrigger::Change)
        .check(move |_value, _context| {
            validation_runs_for_validator.set(validation_runs_for_validator.get() + 1);
            Vec::new()
        });

    token.set_value(UploadToken {
        token: "programmatic".to_owned(),
    });

    assert_eq!(token.value(), "tok:programmatic");
    assert_eq!(handle.field_value(token_path.clone()).token, "programmatic");
    assert!(!handle.is_field_touched(token_path.clone()));
    assert!(!handle.is_field_blurred(token_path.clone()));
    assert_eq!(validation_runs.get(), 0);

    token.on_input("bad raw token");
    token.set_value(UploadToken {
        token: "cleared".to_owned(),
    });

    assert_eq!(token.value(), "tok:cleared");
    assert_eq!(handle.field_value(token_path).token, "cleared");
    assert!(token.parse_error().is_none());
    assert!(handle.can_submit());
    assert_eq!(validation_runs.get(), 0);
}

#[test]
fn reset_reinitialization_and_unmount_clear_custom_parse_blockers() {
    let handle: FormHandle<UploadForm, &'static str> =
        FormHandle::new_with_error_type(UploadForm {
            token: UploadToken {
                token: "initial".to_owned(),
            },
        });
    let token_path = UploadForm::fields().token();
    let token = controlled_upload_token(&handle);

    token.on_input("bad raw token");
    handle.reset();

    assert_eq!(token.value(), "tok:initial");
    assert!(token.parse_error().is_none());
    assert!(handle.can_submit());

    token.on_input("bad raw token");
    handle.reinitialize(UploadForm {
        token: UploadToken {
            token: "replacement".to_owned(),
        },
    });

    assert_eq!(token.value(), "tok:replacement");
    assert!(token.parse_error().is_none());
    assert!(handle.can_submit());

    {
        let mounted_token = controlled_upload_token(&handle);
        mounted_token.on_input("bad raw token");

        assert!(!handle.can_submit());
        assert_eq!(handle.parse_errors().len(), 1);
        assert_eq!(handle.field_value(token_path.clone()).token, "replacement");
    }

    assert!(handle.can_submit());
    assert!(handle.parse_errors().is_empty());
    assert_eq!(handle.field_value(token_path).token, "replacement");
}

#[test]
fn reset_reinitialization_and_unmount_clear_parse_blockers_across_helper_families() {
    let initial = ParsedLifecycleForm {
        age: 42,
        token: UploadToken {
            token: "initial".to_owned(),
        },
        check_in: DateYmd {
            year: 2026,
            month: 7,
            day: 12,
        },
    };
    let replacement = ParsedLifecycleForm {
        age: 9,
        token: UploadToken {
            token: "replacement".to_owned(),
        },
        check_in: DateYmd {
            year: 2026,
            month: 8,
            day: 1,
        },
    };
    let handle: FormHandle<ParsedLifecycleForm, &'static str> =
        FormHandle::new_with_error_type(initial.clone());
    let fields = ParsedLifecycleForm::fields();
    let age_path = fields.age();
    let token_path = fields.token();
    let check_in_path = fields.check_in();
    let age = handle.number(age_path.clone());
    let token =
        handle.parsed_text_with(token_path.clone(), parse_upload_token, format_upload_token);
    let check_in = handle.date_with(check_in_path.clone(), parse_date_ymd, format_date_ymd);

    age.on_input("not-a-number");
    token.on_input("bad raw token");
    check_in.on_input("bad raw date");

    assert_eq!(handle.parse_errors().len(), 3);
    assert!(!handle.can_submit());

    handle.reset();

    assert_eq!(age.value(), "42");
    assert_eq!(token.value(), "tok:initial");
    assert_eq!(check_in.value(), "2026-07-12");
    assert_eq!(handle.snapshot(), initial);
    assert!(handle.parse_errors().is_empty());
    assert!(handle.can_submit());

    age.on_input("not-a-number");
    token.on_input("bad raw token");
    check_in.on_input("bad raw date");
    handle.reinitialize(replacement.clone());

    assert_eq!(age.value(), "9");
    assert_eq!(token.value(), "tok:replacement");
    assert_eq!(check_in.value(), "2026-08-01");
    assert_eq!(handle.snapshot(), replacement);
    assert!(handle.parse_errors().is_empty());
    assert!(handle.can_submit());

    {
        let mounted_age = handle.number(age_path);
        let mounted_token =
            handle.parsed_text_with(token_path, parse_upload_token, format_upload_token);
        let mounted_check_in = handle.date_with(check_in_path, parse_date_ymd, format_date_ymd);

        mounted_age.on_input("not-a-number");
        mounted_token.on_input("bad raw token");
        mounted_check_in.on_input("bad raw date");

        assert_eq!(handle.parse_errors().len(), 3);
        assert_eq!(handle.snapshot(), replacement);
        assert!(!handle.can_submit());
    }

    assert_eq!(handle.snapshot(), replacement);
    assert!(handle.parse_errors().is_empty());
    assert!(handle.can_submit());
}

#[test]
fn dioxus_managed_submit_blocks_mounted_parse_errors() {
    let handle = FormHandle::new(AccountForm { age: 42 });
    let age = handle.parsed_text(AccountForm::fields().age());
    let submit = handle.managed_submit();
    let event = managed_submit_event();
    let called = Cell::new(false);

    age.on_input("not-a-number");

    assert!(!submit.can_submit());
    assert_eq!(
        submit.submit_availability().blockers(),
        &[SubmitBlocker::ParseErrors]
    );

    let result = submit.on_submit(event.clone(), |_submitted| called.set(true));

    assert_eq!(result, SubmitResult::Blocked(SubmitBlocker::ParseErrors));
    assert!(!event.default_action_enabled());
    assert!(!event.propagates());
    assert!(!called.get());
    assert_eq!(handle.submit_attempt_count(), 1);
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ParseErrors))
    );
}

#[test]
fn progressive_submit_prevents_native_submission_and_records_parse_blocked_attempt() {
    let handle = FormHandle::new(AccountForm { age: 42 });
    let age = handle.parsed_text(AccountForm::fields().age());
    let submit = handle.progressive_submit();
    let event = managed_submit_event();

    age.on_input("not-a-number");

    assert!(!submit.can_submit());
    assert_eq!(
        submit.submit_availability().blockers(),
        &[SubmitBlocker::ParseErrors]
    );

    let result = submit.on_submit(event.clone());

    assert_eq!(
        result,
        ProgressiveSubmitResult::Blocked(SubmitBlocker::ParseErrors)
    );
    assert!(!event.default_action_enabled());
    assert!(event.propagates());
    assert_eq!(handle.submit_attempt_count(), 1);
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ParseErrors))
    );
}

#[test]
fn progressive_submit_allows_browser_submission_without_recording_submission_state() {
    let handle = FormHandle::new(AccountForm { age: 42 });
    let submit = handle.progressive_submit();
    let event = managed_submit_event();

    let result = submit.on_submit(event.clone());

    assert_eq!(result, ProgressiveSubmitResult::Allowed);
    assert!(event.default_action_enabled());
    assert!(event.propagates());
    assert_eq!(handle.submit_attempt_count(), 0);
    assert_eq!(handle.last_submit_status(), None);
    assert!(!handle.is_submitting());
}

#[test]
fn progressive_submit_blocks_browser_submission_when_submit_validation_fails() {
    let handle: FormHandle<AccountForm, &'static str> =
        FormHandle::new_with_error_type(AccountForm { age: 17 });
    let age = AccountForm::fields().age();

    handle
        .field(age.clone())
        .validator("adult")
        .on(ValidationTrigger::Submit)
        .check(|value, _context| {
            if *value < 18 {
                vec!["adult_required"]
            } else {
                Vec::new()
            }
        });

    let submit = handle.progressive_submit();
    let event = managed_submit_event();

    let result = submit.on_submit(event.clone());

    assert_eq!(
        result,
        ProgressiveSubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(!event.default_action_enabled());
    assert!(event.propagates());
    assert_eq!(handle.submit_attempt_count(), 1);
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );
    assert_eq!(
        handle.field_validation_errors(age)[0].error(),
        &"adult_required"
    );
}

#[test]
fn progressive_submit_reports_validation_errors_before_unresolved_submit_async_validation() {
    let handle: FormHandle<AccountForm, &'static str> =
        FormHandle::new_with_error_type(AccountForm { age: 17 });
    let age = AccountForm::fields().age();
    let validation_calls = Rc::new(Cell::new(0));
    let validation_calls_for_validator = Rc::clone(&validation_calls);

    handle
        .field(age.clone())
        .validator("adult")
        .on(ValidationTrigger::Submit)
        .check(|value, _context| {
            if *value < 18 {
                vec!["adult_required"]
            } else {
                Vec::new()
            }
        });
    handle
        .async_validator("server_policy")
        .on(ValidationTrigger::Submit)
        .check(move |_snapshot| {
            validation_calls_for_validator.set(validation_calls_for_validator.get() + 1);
            async { Vec::<FormValidationError<&'static str>>::new() }
        });

    let submit = handle.progressive_submit();
    let event = managed_submit_event();

    let result = submit.on_submit(event.clone());

    assert_eq!(
        result,
        ProgressiveSubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );
    assert_eq!(validation_calls.get(), 0);
    assert!(!event.default_action_enabled());
    assert!(event.propagates());
}

#[test]
fn progressive_submit_uses_explicit_submit_intent_for_preflight_validation() {
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: String::new(),
        });
    let email = SignupForm::fields().email();
    let validator_email = email.clone();

    handle.write_advanced(|core| {
        core.register_sync_form_validator_for_triggers(
            "publish_email_required",
            ValidationTrigger::Submit,
            move |context| {
                if context.submit_intent::<SignupSubmitIntent>()
                    == Some(&SignupSubmitIntent::Publish)
                    && context.form().email.is_empty()
                {
                    vec![FormValidationError::field(
                        validator_email.clone(),
                        "email_required_for_publish",
                    )]
                } else {
                    Vec::new()
                }
            },
        );
    });

    let submit = handle.progressive_submit();
    let draft_event = managed_submit_event();
    let draft_result = submit
        .intent(SignupSubmitIntent::SaveDraft)
        .on_submit(draft_event.clone());

    assert_eq!(draft_result, ProgressiveSubmitResult::Allowed);
    assert!(draft_event.default_action_enabled());
    assert_eq!(handle.submit_attempt_count(), 0);

    let publish_event = managed_submit_event();
    let publish_result = submit
        .intent(SignupSubmitIntent::Publish)
        .on_submit(publish_event.clone());

    assert_eq!(
        publish_result,
        ProgressiveSubmitResult::Blocked(SubmitBlocker::ValidationErrors)
    );
    assert!(!publish_event.default_action_enabled());
    assert!(publish_event.propagates());
    assert_eq!(handle.submit_attempt_count(), 1);
    assert_eq!(
        handle.field_validation_errors(email)[0].error(),
        &"email_required_for_publish"
    );
    assert_eq!(
        handle.intent(SignupSubmitIntent::Publish).last_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ValidationErrors))
    );
    assert_eq!(
        handle.intent(SignupSubmitIntent::SaveDraft).last_status(),
        None
    );
}

#[test]
fn progressive_submit_allows_browser_submission_without_starting_submit_async_validation() {
    let handle: FormHandle<AccountForm, &'static str> =
        FormHandle::new_with_error_type(AccountForm { age: 42 });
    let validation_calls = Rc::new(Cell::new(0));
    let validation_calls_for_validator = Rc::clone(&validation_calls);

    handle
        .field(AccountForm::fields().age())
        .async_validator("age_check")
        .on(ValidationTrigger::Submit)
        .check(move |_value, _snapshot| {
            validation_calls_for_validator.set(validation_calls_for_validator.get() + 1);
            async { Vec::<&'static str>::new() }
        });

    let submit = handle.progressive_submit();
    let event = managed_submit_event();

    let result = submit.on_submit(event.clone());

    assert_eq!(result, ProgressiveSubmitResult::Allowed);
    assert_eq!(validation_calls.get(), 0);
    assert!(event.default_action_enabled());
    assert!(event.propagates());
    assert_eq!(handle.submit_attempt_count(), 0);
    assert_eq!(handle.submit_availability().blockers(), &[]);
}

#[test]
fn progressive_submit_blocks_browser_submission_while_managed_async_preflight_is_in_flight() {
    let probe = Rc::new(ManagedAsyncSubmitParseProbe::default());
    let mut dom = VirtualDom::new_with_props(managed_async_submit_parse_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let submit = handle.progressive_submit();
    let event = managed_submit_event();

    assert_eq!(*probe.submit_result.borrow(), Some(SubmitResult::Started));
    assert!(handle.is_submitting());
    assert!(
        submit
            .submit_availability()
            .contains(SubmitBlocker::InFlightSubmission)
    );

    let result = submit.on_submit(event.clone());

    assert_eq!(
        result,
        ProgressiveSubmitResult::Blocked(SubmitBlocker::InFlightSubmission)
    );
    assert!(!event.default_action_enabled());
    assert!(event.propagates());
    assert_eq!(handle.submit_attempt_count(), 1);
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::InFlightSubmission))
    );
    assert_eq!(probe.submit_calls.get(), 0);
}

#[test]
fn browser_submit_exposes_post_method_and_action_without_dioxus_event_handling() {
    let handle = FormHandle::new(SignupForm {
        email: "ada@example.com".to_owned(),
    });

    let submit = handle.browser_submit("/signup");

    assert_eq!(submit.method(), "post");
    assert_eq!(submit.action(), "/signup");
    assert_eq!(handle.submit_attempt_count(), 0);
    assert_eq!(handle.last_submit_status(), None);
}

#[test]
fn browser_submission_control_names_use_field_name_overrides_and_collection_indexes() {
    let handle = FormHandle::new(nested_invoice_collection_form());
    let submit = handle.browser_submit("/invoices");
    let lines_path = NestedInvoiceCollectionForm::fields()
        .invoice()
        .join(NestedInvoice::fields().lines());
    let product_name_path = NestedInvoiceLine::fields()
        .product()
        .join(NestedProduct::fields().name());
    let lines = handle.collection(lines_path);
    let product_name = lines.items()[0].text(product_name_path);

    assert_eq!(submit.method(), "post");
    assert_eq!(submit.action(), "/invoices");
    assert_eq!(
        product_name.name(),
        "invoice.invoice_lines[0].product.product-name"
    );
}

#[test]
fn dioxus_managed_async_submit_does_not_start_validation_when_parse_blocked() {
    let handle: FormHandle<AccountForm, &'static str> =
        FormHandle::new_with_error_type(AccountForm { age: 42 });
    let age_path = AccountForm::fields().age();
    let age = handle.parsed_text(age_path.clone());
    let validation_calls = Rc::new(Cell::new(0));
    let validation_calls_for_validator = Rc::clone(&validation_calls);

    handle
        .field(age_path)
        .async_validator("age_check")
        .on(ValidationTrigger::Submit)
        .check(move |_value, _snapshot| {
            validation_calls_for_validator.set(validation_calls_for_validator.get() + 1);
            async { Vec::<&'static str>::new() }
        });

    age.on_input("not-a-number");

    let submit_calls = Rc::new(Cell::new(0));
    let submit_calls_for_handler = Rc::clone(&submit_calls);
    let result =
        handle
            .managed_submit()
            .on_submit_async(managed_submit_event(), move |_submitted| {
                submit_calls_for_handler.set(submit_calls_for_handler.get() + 1);
                async {}
            });

    assert_eq!(result, SubmitResult::Blocked(SubmitBlocker::ParseErrors));
    assert_eq!(validation_calls.get(), 0);
    assert_eq!(submit_calls.get(), 0);
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ParseErrors))
    );
}

#[test]
fn dioxus_managed_async_submit_blocks_parse_errors_that_appear_while_validation_is_pending() {
    let probe = Rc::new(ManagedAsyncSubmitParseProbe::default());
    let mut dom = VirtualDom::new_with_props(managed_async_submit_parse_probe, Rc::clone(&probe));

    dom.rebuild_in_place();
    dom.render_immediate_to_vec();

    let handle = probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();
    let age = probe
        .age
        .borrow()
        .as_ref()
        .expect("probe should expose its parsed binding")
        .clone();

    assert_eq!(*probe.submit_result.borrow(), Some(SubmitResult::Started));
    assert_eq!(handle.submit_attempt_count(), 1);
    assert!(handle.is_submitting());
    assert_eq!(probe.submit_calls.get(), 0);

    age.on_input("not-a-number");
    dom.render_immediate_to_vec();

    assert_eq!(handle.field_value(AccountForm::fields().age()), 42);
    assert_eq!(handle.parse_errors().len(), 1);
    assert_eq!(
        handle.submit_availability().blockers(),
        &[
            SubmitBlocker::ParseErrors,
            SubmitBlocker::PendingValidation,
            SubmitBlocker::InFlightSubmission,
        ]
    );

    probe.validation.complete(Vec::new());
    dom.render_immediate_to_vec();

    assert_eq!(probe.submit_calls.get(), 0);
    assert!(!handle.is_submitting());
    assert_eq!(handle.submit_attempt_count(), 1);
    assert_eq!(
        handle.last_submit_status(),
        Some(SubmitStatus::Blocked(SubmitBlocker::ParseErrors))
    );
    assert_eq!(handle.field_value(AccountForm::fields().age()), 42);
    assert_eq!(handle.parse_errors().len(), 1);
}

#[test]
fn dioxus_reset_clears_mounted_raw_input_state_and_parse_errors() {
    let handle = FormHandle::new(AccountForm { age: 42 });
    let age_path = AccountForm::fields().age();
    let age = handle.parsed_text(age_path.clone());

    age.on_input("not-a-number");

    assert!(!handle.can_submit());

    handle.reset();

    assert_eq!(age.value(), "42");
    assert_eq!(handle.field_value(age_path), 42);
    assert!(age.parse_error().is_none());
    assert!(handle.parse_errors().is_empty());
    assert!(handle.can_submit());
}

#[test]
fn dropping_parsed_text_binding_unregisters_parse_blocker_without_mutating_draft() {
    let handle = FormHandle::new(AccountForm { age: 42 });
    let age_path = AccountForm::fields().age();

    {
        let age = handle.parsed_text(age_path.clone());
        age.on_input("not-a-number");

        assert_eq!(handle.field_value(age_path.clone()), 42);
        assert!(!handle.can_submit());
        assert_eq!(handle.parse_errors().len(), 1);
    }

    assert_eq!(handle.field_value(age_path), 42);
    assert!(handle.parse_errors().is_empty());
    assert!(handle.can_submit());
}

#[test]
fn dioxus_accessibility_helpers_derive_stable_ids_from_namespace_and_field_name() {
    let handle = FormHandle::new_with_id_namespace(
        SignupForm {
            email: String::new(),
        },
        "signup",
    );
    let email_path = SignupForm::fields().email();
    let accessibility = handle.field_accessibility(email_path.clone());

    assert_eq!(handle.id_namespace().as_str(), "signup");
    assert_eq!(accessibility.input_id(), "signup-email-input");
    assert_eq!(accessibility.help_id(), "signup-email-help");
    assert_eq!(accessibility.error_id(), "signup-email-error");
    assert!(!accessibility.aria_invalid());
    assert_eq!(
        accessibility.aria_describedby().as_deref(),
        Some("signup-email-help")
    );

    let email = handle.text(email_path);

    assert_eq!(email.accessibility(), accessibility);
}

#[test]
fn dioxus_accessibility_helpers_derive_ids_from_overridden_field_names() {
    let handle = FormHandle::new_with_id_namespace(
        ProfileForm {
            email: String::new(),
            accepts_terms: false,
        },
        "profile",
    );
    let email = handle.text(ProfileForm::fields().email());
    let accessibility = email.accessibility();

    assert_eq!(email.name(), "contact-email");
    assert_eq!(accessibility.input_id(), "profile-contact%2demail-input");
    assert_eq!(accessibility.help_id(), "profile-contact%2demail-help");
    assert_eq!(accessibility.error_id(), "profile-contact%2demail-error");
}

#[test]
fn dioxus_accessibility_helpers_avoid_collisions_between_form_namespaces() {
    let first = FormHandle::new_with_id_namespace(
        SignupForm {
            email: String::new(),
        },
        "signup-primary",
    );
    let second = FormHandle::new_with_id_namespace(
        SignupForm {
            email: String::new(),
        },
        "signup-secondary",
    );
    let email = SignupForm::fields().email();

    assert_ne!(
        first.field_accessibility(email.clone()).input_id(),
        second.field_accessibility(email).input_id()
    );
    assert_ne!(
        FormIdNamespace::new("account-email").input_id("main"),
        FormIdNamespace::new("account").input_id("email-main")
    );
}

#[derive(Clone)]
struct HydrationFormConfiguration {
    initial_email: String,
    initial_accepts_terms: bool,
    id_namespace: &'static str,
}

impl HydrationFormConfiguration {
    fn build_config(
        &self,
        validation_runs: Rc<Cell<usize>>,
    ) -> FormConfig<ProfileForm, &'static str> {
        let email_path = ProfileForm::fields().email();

        FormConfig::new(ProfileForm {
            email: self.initial_email.clone(),
            accepts_terms: self.initial_accepts_terms,
        })
        .id_namespace(self.id_namespace)
        .field_validator(email_path, "required")
        .check(move |value, _context| {
            validation_runs.set(validation_runs.get() + 1);

            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HydrationRenderSnapshot {
    email_value: String,
    accepts_terms_checked: bool,
    email_input_id: String,
    email_help_id: String,
    email_error_id: String,
    terms_input_id: String,
    terms_help_id: String,
    terms_error_id: String,
    validation_error_count: usize,
    can_submit: bool,
}

struct HydrationProbe {
    configuration: HydrationFormConfiguration,
    validation_runs: Rc<Cell<usize>>,
    handle: RefCell<Option<FormHandle<ProfileForm, &'static str>>>,
    snapshots: RefCell<Vec<HydrationRenderSnapshot>>,
}

impl HydrationProbe {
    fn new(configuration: HydrationFormConfiguration) -> Self {
        Self {
            configuration,
            validation_runs: Rc::new(Cell::new(0)),
            handle: RefCell::new(None),
            snapshots: RefCell::new(Vec::new()),
        }
    }

    fn latest_snapshot(&self) -> HydrationRenderSnapshot {
        self.snapshots
            .borrow()
            .last()
            .expect("probe should have rendered")
            .clone()
    }
}

fn hydration_oriented_probe(probe: Rc<HydrationProbe>) -> Element {
    let configuration = probe.configuration.clone();
    let validation_runs = Rc::clone(&probe.validation_runs);
    let form = use_form_config(configuration.build_config(validation_runs));
    let email_path = ProfileForm::fields().email();
    let terms_path = ProfileForm::fields().accepts_terms();
    let email = form.text(email_path);
    let terms = form.checkbox(terms_path);
    let email_accessibility = email.accessibility();
    let terms_accessibility = terms.accessibility();

    probe.handle.borrow_mut().replace(form.clone());
    probe.snapshots.borrow_mut().push(HydrationRenderSnapshot {
        email_value: email.value(),
        accepts_terms_checked: terms.checked(),
        email_input_id: email_accessibility.input_id().to_owned(),
        email_help_id: email_accessibility.help_id().to_owned(),
        email_error_id: email_accessibility.error_id().to_owned(),
        terms_input_id: terms_accessibility.input_id().to_owned(),
        terms_help_id: terms_accessibility.help_id().to_owned(),
        terms_error_id: terms_accessibility.error_id().to_owned(),
        validation_error_count: form.validation_errors().len(),
        can_submit: form.can_submit(),
    });

    VNode::empty()
}

#[test]
fn hydration_oriented_renders_are_deterministic_without_serialized_form_state() {
    let configuration = HydrationFormConfiguration {
        initial_email: String::new(),
        initial_accepts_terms: true,
        id_namespace: "signup hydrate",
    };
    let server_probe = Rc::new(HydrationProbe::new(configuration.clone()));
    let client_probe = Rc::new(HydrationProbe::new(configuration));
    let mut server_dom =
        VirtualDom::new_with_props(hydration_oriented_probe, Rc::clone(&server_probe));
    let mut client_dom =
        VirtualDom::new_with_props(hydration_oriented_probe, Rc::clone(&client_probe));

    server_dom.rebuild_in_place();
    client_dom.rebuild_in_place();

    let server_snapshot = server_probe.latest_snapshot();
    let client_snapshot = client_probe.latest_snapshot();

    assert_eq!(server_snapshot, client_snapshot);
    assert_eq!(server_snapshot.email_value, "");
    assert!(server_snapshot.accepts_terms_checked);
    assert_eq!(
        server_snapshot.email_input_id,
        "signup%20hydrate-contact%2demail-input"
    );
    assert_eq!(
        server_snapshot.email_help_id,
        "signup%20hydrate-contact%2demail-help"
    );
    assert_eq!(
        server_snapshot.email_error_id,
        "signup%20hydrate-contact%2demail-error"
    );
    assert_eq!(
        server_snapshot.terms_input_id,
        "signup%20hydrate-accepted_terms-input"
    );
    assert_eq!(
        server_snapshot.terms_help_id,
        "signup%20hydrate-accepted_terms-help"
    );
    assert_eq!(
        server_snapshot.terms_error_id,
        "signup%20hydrate-accepted_terms-error"
    );
    assert_eq!(server_snapshot.validation_error_count, 0);
    assert!(server_snapshot.can_submit);
    assert_eq!(server_probe.validation_runs.get(), 0);
    assert_eq!(client_probe.validation_runs.get(), 0);

    let server_handle = server_probe
        .handle
        .borrow()
        .as_ref()
        .expect("probe should expose its form handle")
        .clone();

    assert_eq!(
        server_handle.read_core(|core| {
            core.validation_status(ProfileForm::fields().email(), "required")
        }),
        Some(ValidationStatus::Unknown)
    );

    server_handle.validate_field(ProfileForm::fields().email(), ValidationTrigger::Manual);

    assert_eq!(server_probe.validation_runs.get(), 1);
    assert_eq!(server_handle.validation_errors().len(), 1);
    assert_eq!(
        server_handle.read_core(|core| {
            core.validation_status(ProfileForm::fields().email(), "required")
        }),
        Some(ValidationStatus::Invalid)
    );
}

#[test]
fn dioxus_accessibility_helpers_mark_visible_validation_errors_invalid() {
    let handle: FormHandle<SignupForm, &'static str> =
        FormHandle::new_with_error_type(SignupForm {
            email: String::new(),
        })
        .with_id_namespace("signup");
    let email_path = SignupForm::fields().email();

    handle.write_advanced(|core| {
        core.register_sync_field_validator(email_path.clone(), "required", |value, _context| {
            if value.is_empty() {
                vec!["required"]
            } else {
                Vec::new()
            }
        });
        core.validate_field(email_path.clone(), dioform::ValidationTrigger::Manual);
    });

    let hidden_error_accessibility = handle.field_accessibility(email_path.clone());

    assert!(!hidden_error_accessibility.has_visible_validation_errors());
    assert!(!hidden_error_accessibility.aria_invalid());
    assert_eq!(
        hidden_error_accessibility.aria_describedby().as_deref(),
        Some("signup-email-help")
    );

    handle.mark_field_blurred(email_path.clone());

    let visible_error_accessibility = handle.field_accessibility(email_path);

    assert!(visible_error_accessibility.has_visible_validation_errors());
    assert!(visible_error_accessibility.aria_invalid());
    assert_eq!(
        visible_error_accessibility.aria_describedby().as_deref(),
        Some("signup-email-help signup-email-error")
    );
    assert_eq!(
        visible_error_accessibility
            .aria_describedby_with_help(false)
            .as_deref(),
        Some("signup-email-error")
    );
}

#[test]
fn dioxus_accessibility_helpers_compose_describedby_with_parse_errors() {
    let handle = FormHandle::new_with_id_namespace(AccountForm { age: 42 }, "account");
    let age_path = AccountForm::fields().age();
    let age = handle.parsed_text(age_path);

    assert_eq!(
        age.accessibility().aria_describedby().as_deref(),
        Some("account-age-help")
    );

    age.on_input("not-a-number");

    let accessibility = age.accessibility();

    assert!(accessibility.has_parse_errors());
    assert!(accessibility.aria_invalid());
    assert_eq!(
        accessibility.aria_describedby().as_deref(),
        Some("account-age-help account-age-error")
    );
    assert_eq!(
        accessibility.aria_describedby_with_help(false).as_deref(),
        Some("account-age-error")
    );
}

#[test]
fn dioxus_accessibility_helpers_combine_validation_and_parse_error_state() {
    let handle: FormHandle<AccountForm, &'static str> =
        FormHandle::new_with_error_type(AccountForm { age: 17 }).with_id_namespace("account");
    let age_path = AccountForm::fields().age();
    let age = handle.number(age_path.clone());

    handle
        .field(age_path.clone())
        .validator("adult")
        .check(|value, _context| {
            if *value < 18 {
                vec!["must be adult"]
            } else {
                Vec::new()
            }
        });
    handle.validate_field(age_path.clone(), ValidationTrigger::Manual);
    handle.mark_field_blurred(age_path);

    let validation_only = age.accessibility();

    assert!(validation_only.has_visible_validation_errors());
    assert!(!validation_only.has_parse_errors());
    assert!(validation_only.aria_invalid());
    assert_eq!(
        validation_only.aria_describedby().as_deref(),
        Some("account-age-help account-age-error")
    );

    age.on_input("not-a-number");

    let combined = age.accessibility();

    assert!(combined.has_visible_validation_errors());
    assert!(combined.has_parse_errors());
    assert!(combined.aria_invalid());
    assert_eq!(
        combined.aria_describedby().as_deref(),
        Some("account-age-help account-age-error")
    );

    handle.reset();
    age.on_input("not-a-number");

    let parse_only = age.accessibility();

    assert!(!parse_only.has_visible_validation_errors());
    assert!(parse_only.has_parse_errors());
    assert!(parse_only.aria_invalid());
    assert_eq!(
        parse_only.aria_describedby_with_help(false).as_deref(),
        Some("account-age-error")
    );
}
