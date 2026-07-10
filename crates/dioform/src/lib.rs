//! Dioxus-facing facade for Dioform.
//!
//! This crate provides explicit [`FormHandle`] access, optional typed Dioxus context access,
//! Dioxus hooks, controlled input bindings, parse blockers, managed submission, and the first
//! runtime integration for async and debounced validation.
//!
//! Async validators are registered through field or form builders and run on Dioxus-spawned tasks
//! from owned [`FormSnapshot`] values. Debounced validators use caller-supplied delay futures;
//! [`debounce_duration`] is the default runtime-neutral helper and can be replaced with an
//! application-specific Dioxus or browser timer future when needed.

use std::{
    any::Any,
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::BTreeMap,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    str::FromStr,
    task::{Context, Poll},
    time::Duration,
};

use dioxus_core::{Event, ReactiveContext, Subscribers, provide_context, try_consume_context};
use dioxus_html::FocusData;
pub use dioxus_html::{FileData, FormData, SerializedFileData};

mod adapter_input_state;
mod adapter_runtime;
mod managed_submission;
mod selector_notifications;

#[doc(hidden)]
pub mod __private {
    pub use dioform_core::FieldIdentity;
}

use dioform_core::{
    __private::CollectionItemFieldAddress, AsyncValidatorContext, CollectionIdentityState,
    CollectionItem, FormCore, FormStateRestoreError, FormStateSnapshot, SubmitAttempt,
    SubmitValidationSnapshot, ValidationStatusView, ValidatorId,
};
pub use dioform_core::{
    CollectionItemIdentity, ErrorVisibilityPolicy, FieldGroup, FieldIdentity, FieldMetadata,
    FieldPath, FieldUpdateOrigin, Form, FormSnapshot, FormValidationError, FormValidatorContext,
    LastSubmitStatus, SubmissionSnapshot, SubmitAvailability, SubmitBlocker, SubmitError,
    SubmitErrors, SubmitResult, SubmitStatus, ValidationErrorSnapshot, ValidationErrorView,
    ValidationMode, ValidationStatus, ValidationTarget, ValidationTrigger, ValidationTriggers,
    ValidatorContext, ValidatorSource,
};
pub use dioform_derive::{FieldGroup, Form};

/// Common imports for ordinary Dioxus form usage.
///
/// This prelude keeps the first-contact API focused on the facade, typed field paths, bindings,
/// validation, and submission. Lower-level core, observer, serialization, and manual runtime types
/// remain available from the crate root for adapter authors and advanced integrations.
pub mod prelude {
    pub use crate::{
        AsyncFileSelectionValidatorBuilder, BrowserSubmitBinding, CheckboxBinding,
        CollectionBinding, CollectionItemBinding, CollectionItemIdentity, ErrorVisibilityPolicy,
        FieldAccessibility, FieldBindingLifecycle, FieldBindingListenerContext,
        FieldBlurListenerContext, FieldGroup, FieldHandle, FieldListenerContext, FieldMetadata,
        FieldPath, FieldUpdateOrigin, FileData, FileFieldKey, FileSelectionBinding,
        FileSelectionCardinality, FileSubmissionSnapshot, Form, FormBlurListenerContext,
        FormConfig, FormContext, FormData, FormHandle, FormIdNamespace, FormListenerContext,
        FormListenerEvent, FormSnapshot, FormValidationError, FormValidatorContext,
        IntentFormHandle, IntentProgressiveSubmitBinding, IntentSubmitBinding, LastSubmitStatus,
        MultiSelectBinding, MultiSelectItem, MultiSelectOptionBinding, NumericInputValue,
        ParseError, ParsedTextBinding, ProgressiveSubmitBinding, ProgressiveSubmitResult,
        RadioGroupBinding, RenderedSelectBinding, SelectBinding, SelectedFile,
        SelectedFileMetadata, SerializedFileData, SubmissionSnapshot, SubmitAvailability,
        SubmitBlocker, SubmitError, SubmitErrors, SubmitListenerContext, SubmitListenerEvent,
        SubmitResult, SubmitStatus, SyncCollectionItemFieldValidatorBuilder,
        SyncFieldValidatorBuilder, SyncFileSelectionValidatorBuilder, SyncFormValidatorBuilder,
        TextBinding, TextareaBinding, ValidationErrorSnapshot, ValidationErrorView, ValidationMode,
        ValidationStatus, ValidationTarget, ValidationTrigger, ValidationTriggers,
        ValidatorContext, debounce_duration, provide_form_context, try_use_form_context,
        use_collection_item_checkbox, use_collection_item_date, use_collection_item_date_with,
        use_collection_item_number, use_collection_item_number_with,
        use_collection_item_parsed_text, use_collection_item_parsed_text_with,
        use_collection_item_radio_group, use_collection_item_select,
        use_collection_item_select_with, use_date, use_date_with, use_debounced_field_listener,
        use_debounced_field_listener_for_origin, use_debounced_form_listener,
        use_debounced_form_listener_for_origin, use_field_binding_listener,
        use_field_blur_listener, use_field_listener, use_field_listener_for_origin, use_form,
        use_form_blur_listener, use_form_config, use_form_context, use_form_handle,
        use_form_listener, use_form_listener_for_origin, use_multi_select, use_number,
        use_number_with, use_parsed_text, use_parsed_text_with, use_radio_group, use_select,
        use_select_with, use_submit_listener,
    };
}

/// Lower-level imports for adapters, diagnostics, serialization, and manual runtime integration.
pub mod advanced {
    pub use dioform_core::{
        AsyncFieldValidation, AsyncFormValidation, AsyncValidatorContext,
        COLLECTION_IDENTITY_SERIALIZATION_VERSION, CollectionIdentitySequence,
        CollectionIdentitySnapshot, CollectionIdentityState, CollectionItem,
        CollectionItemIdentity, DebouncedAsyncFieldValidation, DebouncedAsyncFormValidation,
        FORM_STATE_SERIALIZATION_VERSION, FieldIdentity, FieldUpdateOrigin, FormCore, FormDraft,
        FormObserverEvent, FormObserverField, FormObserverValue, FormStateRestoreError,
        FormStateSnapshot, SubmitAttempt, SubmitValidationSnapshot, ValidationStatusView,
        ValidatorId,
    };

    pub use crate::{
        CollectionBinding, CollectionCheckboxBinding, CollectionItemBinding,
        CollectionParsedTextBinding, CollectionRadioGroupBinding, CollectionRenderedSelectBinding,
        CollectionSelectBinding, CollectionTextBinding, DebounceDelay,
    };
}

use adapter_input_state::ParseBindingId;
use adapter_runtime::{
    AdapterRuntime, DebounceWake, RuntimeAsyncFieldValidator, RuntimeAsyncFormValidator,
    ValidationRuntime,
};
use managed_submission::ManagedSubmission;
use selector_notifications::SelectorTransition;

/// Creates a Dioxus-managed explicit form handle for a component instance.
///
/// This is the canonical hook for configured forms. The `create` closure runs once for the
/// component instance, registers cleanup for unmount, and is form initialization rather than prop
/// synchronization. Initial values are captured when the hook initializes the form; later parent
/// data changes must use [`FormHandle::reinitialize`] when they intentionally replace the baseline
/// and draft. Creating a form does not run validation automatically.
pub fn use_form_handle<Model: 'static, Error: 'static>(
    create: impl FnOnce() -> FormHandle<Model, Error> + 'static,
) -> FormHandle<Model, Error> {
    dioxus_core::use_hook_with_cleanup(create, |handle| handle.cleanup())
}

/// Registers a field-scoped listener for semantic form events during this component's lifetime.
pub fn use_field_listener<Model, Value, Error, Listener>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    listener: Listener,
) where
    Model: 'static,
    Value: 'static,
    Error: 'static,
    Listener: FnMut(FieldListenerContext<Model, Error>) + 'static,
{
    let _registration = dioxus_core::use_hook_with_cleanup(
        move || handle.register_field_listener(path, None, listener),
        drop,
    );
}

/// Registers a field-scoped listener for one update origin during this component's lifetime.
pub fn use_field_listener_for_origin<Model, Value, Error, Listener>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    origin: FieldUpdateOrigin,
    listener: Listener,
) where
    Model: 'static,
    Value: 'static,
    Error: 'static,
    Listener: FnMut(FieldListenerContext<Model, Error>) + 'static,
{
    let _registration = dioxus_core::use_hook_with_cleanup(
        move || handle.register_field_listener(path, Some(origin), listener),
        drop,
    );
}

/// Registers a debounced field-scoped listener for semantic form events during this component's lifetime.
pub fn use_debounced_field_listener<Model, Value, Error, DelayFactory, Delay, Listener>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    delay: DelayFactory,
    listener: Listener,
) where
    Model: 'static,
    Value: 'static,
    Error: 'static,
    DelayFactory: Fn() -> Delay + 'static,
    Delay: Future<Output = ()> + 'static,
    Listener: FnMut(FieldListenerContext<Model, Error>) + 'static,
{
    let delay = Rc::new(move || Box::pin(delay()) as Pin<Box<dyn Future<Output = ()>>>);
    let _registration = dioxus_core::use_hook_with_cleanup(
        move || handle.register_debounced_field_listener(path, None, delay, listener),
        drop,
    );
}

/// Registers a debounced field-scoped listener for one update origin during this component's lifetime.
pub fn use_debounced_field_listener_for_origin<Model, Value, Error, DelayFactory, Delay, Listener>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    origin: FieldUpdateOrigin,
    delay: DelayFactory,
    listener: Listener,
) where
    Model: 'static,
    Value: 'static,
    Error: 'static,
    DelayFactory: Fn() -> Delay + 'static,
    Delay: Future<Output = ()> + 'static,
    Listener: FnMut(FieldListenerContext<Model, Error>) + 'static,
{
    let delay = Rc::new(move || Box::pin(delay()) as Pin<Box<dyn Future<Output = ()>>>);
    let _registration = dioxus_core::use_hook_with_cleanup(
        move || handle.register_debounced_field_listener(path, Some(origin), delay, listener),
        drop,
    );
}

/// Registers a field-scoped listener for blur events during this component's lifetime.
pub fn use_field_blur_listener<Model, Value, Error, Listener>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    listener: Listener,
) where
    Model: 'static,
    Value: 'static,
    Error: 'static,
    Listener: FnMut(FieldBlurListenerContext<Model, Error>) + 'static,
{
    let _registration = dioxus_core::use_hook_with_cleanup(
        move || handle.register_field_blur_listener(path, listener),
        drop,
    );
}

/// Registers a listener for hook-owned field binding mount and unmount events.
pub fn use_field_binding_listener<Model, Value, Error, Listener>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    listener: Listener,
) where
    Model: 'static,
    Value: 'static,
    Error: 'static,
    Listener: FnMut(FieldBindingListenerContext<Model, Error>) + 'static,
{
    let _registration = dioxus_core::use_hook_with_cleanup(
        move || handle.register_field_binding_listener(path, listener),
        drop,
    );
}

/// Registers a form-level listener for semantic form events during this component's lifetime.
pub fn use_form_listener<Model, Error, Listener>(
    handle: FormHandle<Model, Error>,
    listener: Listener,
) where
    Model: 'static,
    Error: 'static,
    Listener: FnMut(FormListenerContext<Model, Error>) + 'static,
{
    let _registration = dioxus_core::use_hook_with_cleanup(
        move || handle.register_form_listener(None, listener),
        drop,
    );
}

/// Registers a form-level listener for one update origin during this component's lifetime.
pub fn use_form_listener_for_origin<Model, Error, Listener>(
    handle: FormHandle<Model, Error>,
    origin: FieldUpdateOrigin,
    listener: Listener,
) where
    Model: 'static,
    Error: 'static,
    Listener: FnMut(FormListenerContext<Model, Error>) + 'static,
{
    let _registration = dioxus_core::use_hook_with_cleanup(
        move || handle.register_form_listener(Some(origin), listener),
        drop,
    );
}

/// Registers a debounced form-level listener for semantic form events during this component's lifetime.
pub fn use_debounced_form_listener<Model, Error, DelayFactory, Delay, Listener>(
    handle: FormHandle<Model, Error>,
    delay: DelayFactory,
    listener: Listener,
) where
    Model: 'static,
    Error: 'static,
    DelayFactory: Fn() -> Delay + 'static,
    Delay: Future<Output = ()> + 'static,
    Listener: FnMut(FormListenerContext<Model, Error>) + 'static,
{
    let delay = Rc::new(move || Box::pin(delay()) as Pin<Box<dyn Future<Output = ()>>>);
    let _registration = dioxus_core::use_hook_with_cleanup(
        move || handle.register_debounced_form_listener(None, delay, listener),
        drop,
    );
}

/// Registers a debounced form-level listener for one update origin during this component's lifetime.
pub fn use_debounced_form_listener_for_origin<Model, Error, DelayFactory, Delay, Listener>(
    handle: FormHandle<Model, Error>,
    origin: FieldUpdateOrigin,
    delay: DelayFactory,
    listener: Listener,
) where
    Model: 'static,
    Error: 'static,
    DelayFactory: Fn() -> Delay + 'static,
    Delay: Future<Output = ()> + 'static,
    Listener: FnMut(FormListenerContext<Model, Error>) + 'static,
{
    let delay = Rc::new(move || Box::pin(delay()) as Pin<Box<dyn Future<Output = ()>>>);
    let _registration = dioxus_core::use_hook_with_cleanup(
        move || handle.register_debounced_form_listener(Some(origin), delay, listener),
        drop,
    );
}

/// Registers a form-level listener for blur events during this component's lifetime.
pub fn use_form_blur_listener<Model, Error, Listener>(
    handle: FormHandle<Model, Error>,
    listener: Listener,
) where
    Model: 'static,
    Error: 'static,
    Listener: FnMut(FormBlurListenerContext<Model, Error>) + 'static,
{
    let _registration = dioxus_core::use_hook_with_cleanup(
        move || handle.register_form_blur_listener(listener),
        drop,
    );
}

/// Registers a listener for submit lifecycle events during this component's lifetime.
pub fn use_submit_listener<Model, Error, Listener>(
    handle: FormHandle<Model, Error>,
    listener: Listener,
) where
    Model: 'static,
    Error: 'static,
    Listener: FnMut(SubmitListenerContext<Model, Error>) + 'static,
{
    let _registration =
        dioxus_core::use_hook_with_cleanup(move || handle.register_submit_listener(listener), drop);
}

fn use_field_binding_hook<Model, Value, Error, Binding, Create>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    create: Create,
) -> Binding
where
    Model: 'static,
    Value: 'static,
    Error: 'static,
    Binding: Clone + 'static,
    Create: FnOnce(FormHandle<Model, Error>, FieldPath<Model, Value>) -> Binding + 'static,
{
    let cleanup_handle = handle.clone();
    let cleanup_field = path.identity();

    dioxus_core::use_hook_with_cleanup(
        move || {
            let field = path.identity();
            let binding = create(handle.clone(), path);
            handle.dispatch_field_binding_listeners(field, FieldBindingLifecycle::Mounted);
            binding
        },
        move |_binding| {
            cleanup_handle
                .dispatch_field_binding_listeners(cleanup_field, FieldBindingLifecycle::Unmounted);
        },
    )
}

/// Creates a Dioxus-managed form-owned draft initialized from `initial`.
pub fn use_form<Model>(initial: Model) -> FormHandle<Model>
where
    Model: Clone + 'static,
{
    use_form_handle(move || FormHandle::new(initial))
}

/// Creates a Dioxus-managed form from durable configuration.
pub fn use_form_config<Model, Error>(config: FormConfig<Model, Error>) -> FormHandle<Model, Error>
where
    Model: Clone + 'static,
    Error: 'static,
{
    use_form_handle(move || FormHandle::from_config(config))
}

/// Creates a Dioxus-managed form-owned draft with an explicit form ID namespace.
pub fn use_form_with_id_namespace<Model, Namespace>(
    initial: Model,
    id_namespace: Namespace,
) -> FormHandle<Model>
where
    Model: Clone + 'static,
    Namespace: Into<FormIdNamespace> + 'static,
{
    use_form_handle(move || FormHandle::new_with_id_namespace(initial, id_namespace))
}

/// Creates a Dioxus-managed form handle from renderer-agnostic form state.
pub fn use_form_from_core<Model: 'static, Error: 'static>(
    core: FormCore<Model, Error>,
) -> FormHandle<Model, Error> {
    use_form_handle(move || FormHandle::from_core(core))
}

/// Creates a Dioxus-managed form handle from form state with an explicit form ID namespace.
pub fn use_form_from_core_with_id_namespace<Model: 'static, Error: 'static, Namespace>(
    core: FormCore<Model, Error>,
    id_namespace: Namespace,
) -> FormHandle<Model, Error>
where
    Namespace: Into<FormIdNamespace> + 'static,
{
    use_form_handle(move || FormHandle::from_core_with_id_namespace(core, id_namespace))
}

/// Typed Dioxus context value for one scoped form handle.
pub struct FormContext<Scope, Model, Error = String> {
    handle: FormHandle<Model, Error>,
    scope: PhantomData<Scope>,
}

impl<Scope, Model, Error> Clone for FormContext<Scope, Model, Error> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            scope: PhantomData,
        }
    }
}

impl<Scope, Model, Error> FormContext<Scope, Model, Error> {
    fn new(handle: FormHandle<Model, Error>) -> Self {
        Self {
            handle,
            scope: PhantomData,
        }
    }
}

/// Provides an existing form handle to descendants through a typed Dioxus context scope.
pub fn provide_form_context<Scope, Model, Error>(
    handle: FormHandle<Model, Error>,
) -> FormHandle<Model, Error>
where
    Scope: 'static,
    Model: 'static,
    Error: 'static,
{
    provide_context(FormContext::<Scope, Model, Error>::new(handle)).handle
}

/// Reads a scoped form handle from Dioxus context when one is available.
pub fn try_use_form_context<Scope, Model, Error>() -> Option<FormHandle<Model, Error>>
where
    Scope: 'static,
    Model: 'static,
    Error: 'static,
{
    try_consume_context::<FormContext<Scope, Model, Error>>().map(|context| context.handle)
}

/// Reads a scoped form handle from Dioxus context, panicking when no provider is available.
pub fn use_form_context<Scope, Model, Error>() -> FormHandle<Model, Error>
where
    Scope: 'static,
    Model: 'static,
    Error: 'static,
{
    try_use_form_context::<Scope, Model, Error>().unwrap_or_else(|| {
        panic!(
            "missing Dioform context for scope `{}` and form handle `{}`",
            std::any::type_name::<Scope>(),
            std::any::type_name::<FormHandle<Model, Error>>()
        )
    })
}

/// Creates a stable controlled select binding for a component instance.
pub fn use_select<Model, Value, Error>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
) -> SelectBinding<Model, Value, Error>
where
    Model: 'static,
    Value: 'static,
    Error: 'static,
{
    use_field_binding_hook(handle, path, |handle, path| handle.select(path))
}

/// Creates a stable controlled select binding with rendered string conversion.
///
/// Use this for native select controls whose Dioxus events expose rendered string values while the
/// form field remains an enum or other typed Rust value.
pub fn use_select_with<Model, Value, Error, Parser, ParserError, Formatter>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    parser: Parser,
    formatter: Formatter,
) -> RenderedSelectBinding<Model, Value, Error>
where
    Model: 'static,
    Value: 'static,
    Error: 'static,
    Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
    ParserError: fmt::Display + 'static,
    Formatter: Fn(&Value) -> String + 'static,
{
    use_field_binding_hook(handle, path, |handle, path| {
        handle.select_with(path, parser, formatter)
    })
}

/// Creates a stable controlled radio group binding for a component instance.
pub fn use_radio_group<Model, Value, Error>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
) -> RadioGroupBinding<Model, Value, Error>
where
    Model: 'static,
    Value: 'static,
    Error: 'static,
{
    use_field_binding_hook(handle, path, |handle, path| handle.radio_group(path))
}

/// Creates a stable true multi-select binding for a direct `Vec<Value>` field.
pub fn use_multi_select<Model, Value, Error>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Vec<Value>>,
) -> MultiSelectBinding<Model, Value, Error>
where
    Model: 'static,
    Value: 'static,
    Error: 'static,
{
    use_field_binding_hook(handle, path, |handle, path| handle.multi_select(path))
}

/// Creates a controlled checkbox binding for a collection item child field.
pub fn use_collection_item_checkbox<Model, Item, Error>(
    item: CollectionItemBinding<Model, Item, Error>,
    path: FieldPath<Item, bool>,
) -> CollectionCheckboxBinding<Model, Item, Error> {
    item.checkbox(path)
}

/// Creates a headless controlled select binding for a collection item child field.
pub fn use_collection_item_select<Model, Item, Value, Error>(
    item: CollectionItemBinding<Model, Item, Error>,
    path: FieldPath<Item, Value>,
) -> CollectionSelectBinding<Model, Item, Value, Error> {
    item.select(path)
}

/// Creates a headless controlled select binding with rendered string conversion for a collection
/// item child field.
pub fn use_collection_item_select_with<Model, Item, Value, Error, Parser, ParserError, Formatter>(
    item: CollectionItemBinding<Model, Item, Error>,
    path: FieldPath<Item, Value>,
    parser: Parser,
    formatter: Formatter,
) -> CollectionRenderedSelectBinding<Model, Item, Value, Error>
where
    Value: 'static,
    Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
    ParserError: fmt::Display + 'static,
    Formatter: Fn(&Value) -> String + 'static,
{
    item.select_with(path, parser, formatter)
}

/// Creates a controlled radio group binding for a collection item child field.
pub fn use_collection_item_radio_group<Model, Item, Value, Error>(
    item: CollectionItemBinding<Model, Item, Error>,
    path: FieldPath<Item, Value>,
) -> CollectionRadioGroupBinding<Model, Item, Value, Error> {
    item.radio_group(path)
}

/// Creates a stable parsed text binding for a component instance.
///
/// Parsed bindings own mounted parse-error state. Creating one directly during every render can
/// unregister the previous parse blocker, so components should use this hook when binding parsed
/// inputs.
pub fn use_parsed_text<Model, Value, Error>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
) -> ParsedTextBinding<Model, Value, Error>
where
    Model: 'static,
    Value: FromStr + fmt::Display + 'static,
    Value::Err: fmt::Display + 'static,
    Error: 'static,
{
    use_parsed_text_with(
        handle,
        path,
        |value| value.parse::<Value>(),
        |value| value.to_string(),
    )
}

/// Creates a stable parsed text binding with explicit parser and formatter behavior.
///
/// Parsed bindings own mounted parse-error state. Creating one directly during every render can
/// unregister the previous parse blocker, so components should use this hook when binding parsed
/// inputs.
pub fn use_parsed_text_with<Model, Value, Error, Parser, ParserError, Formatter>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    parser: Parser,
    formatter: Formatter,
) -> ParsedTextBinding<Model, Value, Error>
where
    Model: 'static,
    Value: 'static,
    Error: 'static,
    Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
    ParserError: fmt::Display + 'static,
    Formatter: Fn(&Value) -> String + 'static,
{
    let cleanup_handle = handle.clone();
    let cleanup_field = path.identity();

    dioxus_core::use_hook_with_cleanup(
        move || {
            let field = path.identity();
            let binding = handle.parsed_text_with(path, parser, formatter);
            handle.dispatch_field_binding_listeners(field, FieldBindingLifecycle::Mounted);
            binding
        },
        move |_binding| {
            cleanup_handle
                .dispatch_field_binding_listeners(cleanup_field, FieldBindingLifecycle::Unmounted);
        },
    )
}

/// Creates a stable numeric input binding for a component instance.
///
/// Numeric bindings are a convenience over parsed text bindings. They parse rendered number input
/// into typed numeric field values while keeping range, step, precision, and business rules in
/// validation. Empty input for non-optional numeric fields is a parse error from the field type's
/// [`FromStr`] implementation.
pub fn use_number<Model, Value, Error>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
) -> ParsedTextBinding<Model, Value, Error>
where
    Model: 'static,
    Value: NumericInputValue,
    Value::Err: fmt::Display + 'static,
    Error: 'static,
{
    use_parsed_text(handle, path)
}

/// Creates a stable numeric input binding with explicit parser and formatter behavior.
///
/// Use this when the application needs behavior such as optional numeric fields where empty input
/// has domain-specific meaning.
pub fn use_number_with<Model, Value, Error, Parser, ParserError, Formatter>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    parser: Parser,
    formatter: Formatter,
) -> ParsedTextBinding<Model, Value, Error>
where
    Model: 'static,
    Value: 'static,
    Error: 'static,
    Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
    ParserError: fmt::Display + 'static,
    Formatter: Fn(&Value) -> String + 'static,
{
    use_parsed_text_with(handle, path, parser, formatter)
}

/// Creates a stable parsed text binding for a collection item child field.
///
/// The hook-owned parse registration is keyed by the logical collection item identity and child
/// field identity. The returned binding still uses the current rendered item index for
/// [`CollectionParsedTextBinding::name`], so names update after reordering without remounting the
/// parse blocker.
pub fn use_collection_item_parsed_text<Model, Item, Value, Error>(
    item: CollectionItemBinding<Model, Item, Error>,
    path: FieldPath<Item, Value>,
) -> CollectionParsedTextBinding<Model, Item, Value, Error>
where
    Model: 'static,
    Item: 'static,
    Value: FromStr + fmt::Display + 'static,
    Value::Err: fmt::Display + 'static,
    Error: 'static,
{
    use_collection_item_parsed_text_with(
        item,
        path,
        |value| value.parse::<Value>(),
        |value| value.to_string(),
    )
}

/// Creates a stable parsed text binding with explicit parser and formatter behavior for a
/// collection item child field.
///
/// Prefer this hook inside Dioxus row components that render parsed collection item inputs. It
/// keeps mounted parse state stable across rerenders while deriving the rendered field name from
/// the latest item index.
pub fn use_collection_item_parsed_text_with<
    Model,
    Item,
    Value,
    Error,
    Parser,
    ParserError,
    Formatter,
>(
    item: CollectionItemBinding<Model, Item, Error>,
    path: FieldPath<Item, Value>,
    parser: Parser,
    formatter: Formatter,
) -> CollectionParsedTextBinding<Model, Item, Value, Error>
where
    Model: 'static,
    Item: 'static,
    Value: 'static,
    Error: 'static,
    Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
    ParserError: fmt::Display + 'static,
    Formatter: Fn(&Value) -> String + 'static,
{
    let hook_item = item.clone();
    let hook_path = path.clone();
    let state = dioxus_core::use_hook(move || {
        CollectionParsedTextHookState::new(hook_item, hook_path, parser, formatter)
    });
    let CollectionParsedTextHookState {
        registration,
        parser,
        formatter,
    } = state;

    CollectionParsedTextBinding {
        base: CollectionFieldBindingCore::new(
            item.handle.clone(),
            item.collection_path.clone(),
            item.item,
            path,
        ),
        registration,
        parser,
        formatter,
    }
}

/// Creates a stable numeric input binding for a collection item child field.
pub fn use_collection_item_number<Model, Item, Value, Error>(
    item: CollectionItemBinding<Model, Item, Error>,
    path: FieldPath<Item, Value>,
) -> CollectionParsedTextBinding<Model, Item, Value, Error>
where
    Model: 'static,
    Item: 'static,
    Value: NumericInputValue,
    Value::Err: fmt::Display + 'static,
    Error: 'static,
{
    use_collection_item_parsed_text(item, path)
}

/// Creates a stable numeric input binding with explicit parser and formatter behavior for a
/// collection item child field.
pub fn use_collection_item_number_with<Model, Item, Value, Error, Parser, ParserError, Formatter>(
    item: CollectionItemBinding<Model, Item, Error>,
    path: FieldPath<Item, Value>,
    parser: Parser,
    formatter: Formatter,
) -> CollectionParsedTextBinding<Model, Item, Value, Error>
where
    Model: 'static,
    Item: 'static,
    Value: 'static,
    Error: 'static,
    Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
    ParserError: fmt::Display + 'static,
    Formatter: Fn(&Value) -> String + 'static,
{
    use_collection_item_parsed_text_with(item, path, parser, formatter)
}

/// Creates a stable date-oriented input binding for a collection item child field.
pub fn use_collection_item_date<Model, Item, Value, Error>(
    item: CollectionItemBinding<Model, Item, Error>,
    path: FieldPath<Item, Value>,
) -> CollectionParsedTextBinding<Model, Item, Value, Error>
where
    Model: 'static,
    Item: 'static,
    Value: FromStr + fmt::Display + 'static,
    Value::Err: fmt::Display + 'static,
    Error: 'static,
{
    use_collection_item_parsed_text(item, path)
}

/// Creates a stable date-oriented input binding with explicit parser and formatter behavior for a
/// collection item child field.
pub fn use_collection_item_date_with<Model, Item, Value, Error, Parser, ParserError, Formatter>(
    item: CollectionItemBinding<Model, Item, Error>,
    path: FieldPath<Item, Value>,
    parser: Parser,
    formatter: Formatter,
) -> CollectionParsedTextBinding<Model, Item, Value, Error>
where
    Model: 'static,
    Item: 'static,
    Value: 'static,
    Error: 'static,
    Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
    ParserError: fmt::Display + 'static,
    Formatter: Fn(&Value) -> String + 'static,
{
    use_collection_item_parsed_text_with(item, path, parser, formatter)
}

/// Creates a stable date-oriented input binding for values that implement [`FromStr`] and [`fmt::Display`].
///
/// Date bindings are a convenience over parsed text bindings. The library does not own date,
/// timezone, localization, or calendar semantics; applications choose the typed value and rendered
/// format. Date relationship rules such as check-out after check-in belong in field or form
/// validation.
pub fn use_date<Model, Value, Error>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
) -> ParsedTextBinding<Model, Value, Error>
where
    Model: 'static,
    Value: FromStr + fmt::Display + 'static,
    Value::Err: fmt::Display + 'static,
    Error: 'static,
{
    use_parsed_text(handle, path)
}

/// Creates a stable date-oriented input binding with explicit parser and formatter behavior.
///
/// Date bindings are a convenience over parsed text bindings. The library does not own date,
/// timezone, localization, or calendar semantics; applications provide the rendered format and the
/// typed value parser. Date relationship rules such as check-out after check-in belong in field or
/// form validation.
pub fn use_date_with<Model, Value, Error, Parser, ParserError, Formatter>(
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    parser: Parser,
    formatter: Formatter,
) -> ParsedTextBinding<Model, Value, Error>
where
    Model: 'static,
    Value: 'static,
    Error: 'static,
    Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
    ParserError: fmt::Display + 'static,
    Formatter: Fn(&Value) -> String + 'static,
{
    use_parsed_text_with(handle, path, parser, formatter)
}

/// Creates a reusable debounce delay factory from a duration.
///
/// This helper uses a runtime-neutral [`futures_timer::Delay`] future that is polled inside the
/// Dioxus-spawned validation task. Applications can still pass any other `Future<Output = ()>`
/// factory to [`AsyncFieldValidatorBuilder::debounce`],
/// [`AsyncFileSelectionValidatorBuilder::debounce`], or [`AsyncFormValidatorBuilder::debounce`]
/// when they need a Dioxus-specific, browser-specific, or test-controlled timer source.
pub fn debounce_duration(duration: Duration) -> impl Fn() -> DebounceDelay + Clone + 'static {
    move || DebounceDelay::new(duration)
}

/// A delay future used by [`debounce_duration`].
pub struct DebounceDelay {
    inner: futures_timer::Delay,
}

impl DebounceDelay {
    /// Creates a delay future that completes after `duration`.
    pub fn new(duration: Duration) -> Self {
        Self {
            inner: futures_timer::Delay::new(duration),
        }
    }
}

impl Future for DebounceDelay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner).poll(context)
    }
}

/// A per-form namespace used to derive stable element IDs for field accessibility helpers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormIdNamespace {
    value: String,
}

impl FormIdNamespace {
    /// Creates a form ID namespace.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    /// Returns the namespace value before ID-segment escaping.
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Derives the stable input ID for a field name.
    pub fn input_id(&self, field_name: &str) -> String {
        derive_field_id(&self.value, field_name, "input")
    }

    /// Derives the stable help-text ID for a field name.
    pub fn help_id(&self, field_name: &str) -> String {
        derive_field_id(&self.value, field_name, "help")
    }

    /// Derives the stable error ID for a field name.
    pub fn error_id(&self, field_name: &str) -> String {
        derive_field_id(&self.value, field_name, "error")
    }
}

impl AsRef<str> for FormIdNamespace {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for FormIdNamespace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Default for FormIdNamespace {
    fn default() -> Self {
        Self::new("form")
    }
}

impl From<&str> for FormIdNamespace {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for FormIdNamespace {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<Cow<'_, str>> for FormIdNamespace {
    fn from(value: Cow<'_, str>) -> Self {
        Self::new(value.into_owned())
    }
}

impl From<FormIdNamespace> for String {
    fn from(value: FormIdNamespace) -> Self {
        value.value
    }
}

type FormConfigRegistration<Model, Error> = dyn Fn(&FormHandle<Model, Error>) + 'static;

/// Durable setup used to create a form handle.
pub struct FormConfig<Model, Error = String> {
    initial: Model,
    id_namespace: Option<FormIdNamespace>,
    validation_mode: ValidationMode,
    error_visibility_policy: ErrorVisibilityPolicy,
    registrations: Vec<Rc<FormConfigRegistration<Model, Error>>>,
    _marker: PhantomData<fn() -> Error>,
}

/// Builder for configuring a durable synchronous field validator on [`FormConfig`].
pub struct ConfiguredSyncFieldValidatorBuilder<Model, Value, Error = String> {
    config: FormConfig<Model, Error>,
    path: FieldPath<Model, Value>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
}

/// Builder for configuring a durable asynchronous field validator on [`FormConfig`].
pub struct ConfiguredAsyncFieldValidatorBuilder<Model, Value, Error = String> {
    config: FormConfig<Model, Error>,
    path: FieldPath<Model, Value>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
    debounce: Option<Rc<DelayFactoryFn>>,
}

/// Builder for configuring a durable synchronous collection item child-field validator on [`FormConfig`].
pub struct ConfiguredSyncCollectionItemFieldValidatorBuilder<Model, Item, Value, Error = String> {
    config: FormConfig<Model, Error>,
    collection: FieldPath<Model, Vec<Item>>,
    field: FieldPath<Item, Value>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
}

/// Builder for configuring a durable synchronous form validator on [`FormConfig`].
pub struct ConfiguredSyncFormValidatorBuilder<Model, Error = String> {
    config: FormConfig<Model, Error>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
}

/// Builder for configuring a durable asynchronous form validator on [`FormConfig`].
pub struct ConfiguredAsyncFormValidatorBuilder<Model, Error = String> {
    config: FormConfig<Model, Error>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
    debounce: Option<Rc<DelayFactoryFn>>,
}

impl<Model: Clone, Error> Clone for FormConfig<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            initial: self.initial.clone(),
            id_namespace: self.id_namespace.clone(),
            validation_mode: self.validation_mode,
            error_visibility_policy: self.error_visibility_policy,
            registrations: self.registrations.clone(),
            _marker: PhantomData,
        }
    }
}

impl<Model: fmt::Debug, Error> fmt::Debug for FormConfig<Model, Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FormConfig")
            .field("initial", &self.initial)
            .field("id_namespace", &self.id_namespace)
            .field("validation_mode", &self.validation_mode)
            .field("error_visibility_policy", &self.error_visibility_policy)
            .field("registrations", &self.registrations.len())
            .finish()
    }
}

impl<Model, Error> FormConfig<Model, Error> {
    /// Creates form configuration from initial values.
    pub fn new(initial: Model) -> Self {
        Self {
            initial,
            id_namespace: None,
            validation_mode: ValidationMode::default(),
            error_visibility_policy: ErrorVisibilityPolicy::default(),
            registrations: Vec::new(),
            _marker: PhantomData,
        }
    }

    fn push_registration(&mut self, registration: impl Fn(&FormHandle<Model, Error>) + 'static) {
        self.registrations.push(Rc::new(registration));
    }

    /// Starts configuring a durable synchronous field validator.
    pub fn field_validator<Value, Source>(
        self,
        path: FieldPath<Model, Value>,
        source: Source,
    ) -> ConfiguredSyncFieldValidatorBuilder<Model, Value, Error>
    where
        Source: Into<ValidatorSource>,
    {
        ConfiguredSyncFieldValidatorBuilder {
            config: self,
            path,
            source: source.into(),
            triggers: ValidationTriggers::all(),
        }
    }

    /// Starts configuring a durable asynchronous field validator.
    pub fn async_field_validator<Value, Source>(
        self,
        path: FieldPath<Model, Value>,
        source: Source,
    ) -> ConfiguredAsyncFieldValidatorBuilder<Model, Value, Error>
    where
        Source: Into<ValidatorSource>,
    {
        ConfiguredAsyncFieldValidatorBuilder {
            config: self,
            path,
            source: source.into(),
            triggers: ValidationTriggers::all(),
            debounce: None,
        }
    }

    /// Starts configuring a durable synchronous validator template for one child field on every collection item.
    pub fn collection_item_field_validator<Item, Value, Source>(
        self,
        collection: FieldPath<Model, Vec<Item>>,
        field: FieldPath<Item, Value>,
        source: Source,
    ) -> ConfiguredSyncCollectionItemFieldValidatorBuilder<Model, Item, Value, Error>
    where
        Source: Into<ValidatorSource>,
    {
        ConfiguredSyncCollectionItemFieldValidatorBuilder {
            config: self,
            collection,
            field,
            source: source.into(),
            triggers: ValidationTriggers::all(),
        }
    }

    /// Starts configuring a durable synchronous validator template for each selected item value in a collection field.
    pub fn collection_item_validator<Value, Source>(
        self,
        collection: FieldPath<Model, Vec<Value>>,
        source: Source,
    ) -> ConfiguredSyncCollectionItemFieldValidatorBuilder<Model, Value, Value, Error>
    where
        Value: 'static,
        Source: Into<ValidatorSource>,
    {
        self.collection_item_field_validator(collection, multi_select_item_value_path(), source)
    }

    /// Starts configuring a durable synchronous form validator.
    pub fn form_validator<Source>(
        self,
        source: Source,
    ) -> ConfiguredSyncFormValidatorBuilder<Model, Error>
    where
        Source: Into<ValidatorSource>,
    {
        ConfiguredSyncFormValidatorBuilder {
            config: self,
            source: source.into(),
            triggers: ValidationTriggers::all(),
        }
    }

    /// Starts configuring a durable asynchronous form validator.
    pub fn async_form_validator<Source>(
        self,
        source: Source,
    ) -> ConfiguredAsyncFormValidatorBuilder<Model, Error>
    where
        Source: Into<ValidatorSource>,
    {
        ConfiguredAsyncFormValidatorBuilder {
            config: self,
            source: source.into(),
            triggers: ValidationTriggers::all(),
            debounce: None,
        }
    }

    /// Sets the namespace used to derive field IDs for accessibility helpers.
    pub fn id_namespace(mut self, id_namespace: impl Into<FormIdNamespace>) -> Self {
        self.id_namespace = Some(id_namespace.into());
        self
    }

    /// Sets when automatic validation runs.
    pub const fn validation_mode(mut self, validation_mode: ValidationMode) -> Self {
        self.validation_mode = validation_mode;
        self
    }

    /// Sets when stored validation errors are exposed by visible-error selectors.
    pub const fn error_visibility_policy(mut self, policy: ErrorVisibilityPolicy) -> Self {
        self.error_visibility_policy = policy;
        self
    }
}

impl<Model, Value, Error> ConfiguredSyncFieldValidatorBuilder<Model, Value, Error> {
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Adds this synchronous field validator to the form configuration.
    pub fn check<Validator>(mut self, validator: Validator) -> FormConfig<Model, Error>
    where
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Vec<Error> + 'static,
        Model: 'static,
        Value: 'static,
        Error: 'static,
    {
        let path = self.path;
        let source = self.source;
        let triggers = self.triggers;
        let validator = Rc::new(validator);

        self.config.push_registration(move |handle| {
            let validator = Rc::clone(&validator);

            handle.register_sync_field_validator_for_triggers(
                path.clone(),
                source.clone(),
                triggers.clone(),
                move |value, context| validator(value, context),
            );
        });

        self.config
    }

    /// Adds this synchronous field validator when it returns zero or one error.
    pub fn check_optional<Validator>(self, validator: Validator) -> FormConfig<Model, Error>
    where
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Option<Error> + 'static,
        Model: 'static,
        Value: 'static,
        Error: 'static,
    {
        self.check(move |value, context| validator(value, context).into_iter().collect())
    }
}

impl<Model, Value, Error> ConfiguredAsyncFieldValidatorBuilder<Model, Value, Error> {
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Debounces value-change validation with a fresh delay future for each run.
    pub fn debounce<DelayFactory, Delay>(mut self, delay: DelayFactory) -> Self
    where
        DelayFactory: Fn() -> Delay + 'static,
        Delay: Future<Output = ()> + 'static,
    {
        self.debounce = Some(Rc::new(move || Box::pin(delay())));
        self
    }

    /// Adds this asynchronous field validator to the form configuration.
    pub fn check<Validator, Fut, Errors>(self, validator: Validator) -> FormConfig<Model, Error>
    where
        Model: Clone + 'static,
        Value: Clone + 'static,
        Error: 'static,
        Validator: Fn(Value, FormSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
    {
        self.check_with_context(move |value, context| {
            validator(value, context.into_form_snapshot())
        })
    }

    /// Adds this asynchronous field validator with access to validation context metadata.
    pub fn check_with_context<Validator, Fut, Errors>(
        mut self,
        validator: Validator,
    ) -> FormConfig<Model, Error>
    where
        Model: Clone + 'static,
        Value: Clone + 'static,
        Error: 'static,
        Validator: Fn(Value, AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
    {
        let path = self.path;
        let source = self.source;
        let triggers = self.triggers;
        let debounce = self.debounce;
        let validator: Rc<FieldAsyncValidatorFn<Model, Value, Error>> =
            Rc::new(move |value, context| {
                let future = validator(value, context);

                Box::pin(async move { future.await.into_iter().collect() })
            });

        self.config.push_registration(move |handle| {
            let validator = Rc::clone(&validator);

            handle.register_runtime_async_field_validator(
                path.clone(),
                source.clone(),
                triggers.clone(),
                debounce.clone(),
                move |value, context| validator(value, context),
            );
        });

        self.config
    }
}

impl<Model, Item, Value, Error>
    ConfiguredSyncCollectionItemFieldValidatorBuilder<Model, Item, Value, Error>
{
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Adds this collection item child-field validator template to the form configuration.
    pub fn check<Validator>(mut self, validator: Validator) -> FormConfig<Model, Error>
    where
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Vec<Error> + 'static,
        Model: 'static,
        Item: 'static,
        Value: 'static,
        Error: 'static,
    {
        let collection = self.collection;
        let field = self.field;
        let source = self.source;
        let triggers = self.triggers;
        let validator = Rc::new(validator);

        self.config.push_registration(move |handle| {
            let validator = Rc::clone(&validator);

            handle.register_sync_collection_item_field_validator_for_triggers(
                collection.clone(),
                field.clone(),
                source.clone(),
                triggers.clone(),
                move |value, context| validator(value, context),
            );
        });

        self.config
    }

    /// Adds this collection item validator template when it returns zero or one error.
    pub fn check_optional<Validator>(self, validator: Validator) -> FormConfig<Model, Error>
    where
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Option<Error> + 'static,
        Model: 'static,
        Item: 'static,
        Value: 'static,
        Error: 'static,
    {
        self.check(move |value, context| validator(value, context).into_iter().collect())
    }
}

impl<Model, Error> ConfiguredAsyncFormValidatorBuilder<Model, Error> {
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Debounces value-change validation with a fresh delay future for each run.
    pub fn debounce<DelayFactory, Delay>(mut self, delay: DelayFactory) -> Self
    where
        DelayFactory: Fn() -> Delay + 'static,
        Delay: Future<Output = ()> + 'static,
    {
        self.debounce = Some(Rc::new(move || Box::pin(delay())));
        self
    }

    /// Adds this asynchronous form validator to the form configuration.
    pub fn check<Validator, Fut, Errors>(self, validator: Validator) -> FormConfig<Model, Error>
    where
        Model: Clone + 'static,
        Error: 'static,
        Validator: Fn(FormSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = FormValidationError<Error>> + 'static,
    {
        self.check_with_context(move |context| validator(context.into_form_snapshot()))
    }

    /// Adds this asynchronous form validator with access to validation context metadata.
    pub fn check_with_context<Validator, Fut, Errors>(
        mut self,
        validator: Validator,
    ) -> FormConfig<Model, Error>
    where
        Model: Clone + 'static,
        Error: 'static,
        Validator: Fn(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = FormValidationError<Error>> + 'static,
    {
        let source = self.source;
        let triggers = self.triggers;
        let debounce = self.debounce;
        let validator: Rc<FormAsyncValidatorFn<Model, Error>> = Rc::new(move |context| {
            let future = validator(context);

            Box::pin(async move { future.await.into_iter().collect() })
        });

        self.config.push_registration(move |handle| {
            let validator = Rc::clone(&validator);

            handle.register_runtime_async_form_validator(
                source.clone(),
                triggers.clone(),
                debounce.clone(),
                move |context| validator(context),
            );
        });

        self.config
    }
}

impl<Model, Error> ConfiguredSyncFormValidatorBuilder<Model, Error> {
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Adds this synchronous form validator to the form configuration.
    pub fn check<Validator>(mut self, validator: Validator) -> FormConfig<Model, Error>
    where
        Validator: for<'a> Fn(FormValidatorContext<'a, Model>) -> Vec<FormValidationError<Error>>
            + 'static,
        Model: 'static,
        Error: 'static,
    {
        let source = self.source;
        let triggers = self.triggers;
        let validator = Rc::new(validator);

        self.config.push_registration(move |handle| {
            let validator = Rc::clone(&validator);

            handle.register_sync_form_validator_for_triggers(
                source.clone(),
                triggers.clone(),
                move |context| validator(context),
            );
        });

        self.config
    }

    /// Adds this synchronous form validator when it returns zero or one error.
    pub fn check_optional<Validator>(self, validator: Validator) -> FormConfig<Model, Error>
    where
        Validator: for<'a> Fn(FormValidatorContext<'a, Model>) -> Option<FormValidationError<Error>>
            + 'static,
        Model: 'static,
        Error: 'static,
    {
        self.check(move |context| validator(context).into_iter().collect())
    }
}

/// Headless accessibility metadata for a field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldAccessibility {
    input_id: String,
    help_id: String,
    error_id: String,
    has_visible_validation_errors: bool,
    has_parse_errors: bool,
}

impl FieldAccessibility {
    fn new(
        namespace: &FormIdNamespace,
        field_name: &str,
        has_visible_validation_errors: bool,
        has_parse_errors: bool,
    ) -> Self {
        Self {
            input_id: namespace.input_id(field_name),
            help_id: namespace.help_id(field_name),
            error_id: namespace.error_id(field_name),
            has_visible_validation_errors,
            has_parse_errors,
        }
    }

    /// Returns the ID intended for the rendered input control.
    pub fn input_id(&self) -> &str {
        &self.input_id
    }

    /// Returns the ID intended for optional help text.
    pub fn help_id(&self) -> &str {
        &self.help_id
    }

    /// Returns the ID intended for visible validation or parse errors.
    pub fn error_id(&self) -> &str {
        &self.error_id
    }

    /// Returns whether this field currently has visible validation errors.
    pub const fn has_visible_validation_errors(&self) -> bool {
        self.has_visible_validation_errors
    }

    /// Returns whether this field currently has mounted binding parse errors.
    pub const fn has_parse_errors(&self) -> bool {
        self.has_parse_errors
    }

    /// Returns whether this field should be marked invalid for ARIA.
    pub const fn aria_invalid(&self) -> bool {
        self.has_visible_validation_errors || self.has_parse_errors
    }

    /// Returns `aria-describedby` IDs, including help text and current error text when present.
    pub fn aria_describedby(&self) -> Option<String> {
        self.aria_describedby_with_help(true)
    }

    /// Returns `aria-describedby` IDs, optionally including help text.
    pub fn aria_describedby_with_help(&self, include_help: bool) -> Option<String> {
        let mut described_by = String::new();

        if include_help {
            described_by.push_str(&self.help_id);
        }

        if self.aria_invalid() {
            if !described_by.is_empty() {
                described_by.push(' ');
            }

            described_by.push_str(&self.error_id);
        }

        if described_by.is_empty() {
            None
        } else {
            Some(described_by)
        }
    }
}

fn derive_field_id(namespace: &str, field_name: &str, kind: &str) -> String {
    let mut id = String::new();

    push_id_segment(&mut id, namespace);

    if !id.is_empty() {
        id.push('-');
    }

    push_id_segment(&mut id, field_name);

    if !id.is_empty() {
        id.push('-');
    }

    id.push_str(kind);
    id
}

fn push_id_segment(id: &mut String, value: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'_' {
            id.push(byte as char);
        } else {
            id.push('%');
            id.push(HEX[(byte >> 4) as usize] as char);
            id.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
}

/// A binding-level parse error produced while converting rendered input into a typed field value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    field: FieldIdentity,
    raw_value: String,
    message: String,
}

impl ParseError {
    /// Returns the field whose rendered input failed to parse.
    pub fn field_identity(&self) -> FieldIdentity {
        self.field.clone()
    }

    /// Returns the raw rendered input that could not be converted.
    pub fn raw_value(&self) -> &str {
        &self.raw_value
    }

    /// Returns the parser-provided error message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Cardinality policy for the ordered files retained by a file selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileSelectionCardinality {
    /// Retain at most one selected file.
    Single,
    /// Retain all selected files in their input order.
    Multiple,
}

impl FileSelectionCardinality {
    /// Returns whether this policy allows more than one selected file.
    pub const fn allows_multiple(self) -> bool {
        matches!(self, Self::Multiple)
    }
}

/// A typed key for a file selection attached to a form without being part of its form draft.
pub struct FileFieldKey<Model> {
    identity: FieldIdentity,
    field_name: Rc<str>,
    cardinality: FileSelectionCardinality,
    _marker: PhantomData<fn() -> Model>,
}

impl<Model> Clone for FileFieldKey<Model> {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity.clone(),
            field_name: Rc::clone(&self.field_name),
            cardinality: self.cardinality,
            _marker: PhantomData,
        }
    }
}

impl<Model> fmt::Debug for FileFieldKey<Model> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileFieldKey")
            .field("identity", &self.identity)
            .field("field_name", &self.field_name)
            .field("cardinality", &self.cardinality)
            .finish()
    }
}

impl<Model> FileFieldKey<Model> {
    /// Creates a single-file field key using the same value for identity and rendered name.
    pub fn new(name: impl Into<Rc<str>>) -> Self {
        Self::single(name)
    }

    /// Creates a single-file field key using the same value for identity and rendered name.
    pub fn single(name: impl Into<Rc<str>>) -> Self {
        Self::with_cardinality(name, FileSelectionCardinality::Single)
    }

    /// Creates a multi-file field key using the same value for identity and rendered name.
    pub fn multiple(name: impl Into<Rc<str>>) -> Self {
        Self::with_cardinality(name, FileSelectionCardinality::Multiple)
    }

    fn with_cardinality(name: impl Into<Rc<str>>, cardinality: FileSelectionCardinality) -> Self {
        let name = name.into();

        Self {
            identity: FieldIdentity::file(Rc::clone(&name)),
            field_name: name,
            cardinality,
            _marker: PhantomData,
        }
    }

    /// Returns the internal identity used for file-selection metadata and errors.
    pub fn identity(&self) -> FieldIdentity {
        self.identity.clone()
    }

    /// Returns the rendered field name for HTML interoperability.
    pub fn field_name(&self) -> &str {
        &self.field_name
    }

    /// Returns the file selection cardinality policy for this key.
    pub const fn cardinality(&self) -> FileSelectionCardinality {
        self.cardinality
    }

    /// Returns whether this key represents a multi-file field.
    pub const fn allows_multiple(&self) -> bool {
        self.cardinality.allows_multiple()
    }

    fn normalize_selection<Files, File>(&self, files: Files) -> Vec<SelectedFile>
    where
        Files: IntoIterator<Item = File>,
        File: Into<SelectedFile>,
    {
        match self.cardinality {
            FileSelectionCardinality::Single => files.into_iter().take(1).map(Into::into).collect(),
            FileSelectionCardinality::Multiple => files.into_iter().map(Into::into).collect(),
        }
    }
}

/// Cloneable metadata for one user-selected file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedFileMetadata {
    name: String,
    size_bytes: u64,
    media_type: Option<String>,
}

impl SelectedFileMetadata {
    /// Creates selected-file metadata without a media type.
    pub fn new(name: impl Into<String>, size_bytes: u64) -> Self {
        Self {
            name: name.into(),
            size_bytes,
            media_type: None,
        }
    }

    /// Returns this selected-file metadata with a browser-provided media type.
    pub fn with_media_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }

    /// Returns the selected file's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the selected file's byte size.
    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// Returns the selected file's media type, when the platform supplied one.
    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }
}

impl From<FileData> for SelectedFileMetadata {
    fn from(file: FileData) -> Self {
        let mut metadata = Self::new(file.name(), file.size());

        if let Some(media_type) = file.content_type() {
            metadata = metadata.with_media_type(media_type);
        }

        metadata
    }
}

/// One selected file, including cloneable metadata and an optional platform file handle.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectedFile {
    metadata: SelectedFileMetadata,
    file_data: Option<FileData>,
}

impl SelectedFile {
    /// Creates a selected file from metadata only.
    pub fn from_metadata(metadata: SelectedFileMetadata) -> Self {
        Self {
            metadata,
            file_data: None,
        }
    }

    /// Creates a selected file from Dioxus platform file data.
    pub fn from_file_data(file_data: FileData) -> Self {
        Self {
            metadata: SelectedFileMetadata::from(file_data.clone()),
            file_data: Some(file_data),
        }
    }

    /// Returns the selected file metadata.
    pub const fn metadata(&self) -> &SelectedFileMetadata {
        &self.metadata
    }

    /// Returns the Dioxus platform file handle, when this selection came from a file input event.
    pub fn file_data(&self) -> Option<FileData> {
        self.file_data.clone()
    }

    /// Returns the selected file's name.
    pub fn name(&self) -> &str {
        self.metadata.name()
    }

    /// Returns the selected file's byte size.
    pub const fn size_bytes(&self) -> u64 {
        self.metadata.size_bytes()
    }

    /// Returns the selected file's media type, when the platform supplied one.
    pub fn media_type(&self) -> Option<&str> {
        self.metadata.media_type()
    }
}

impl From<SelectedFileMetadata> for SelectedFile {
    fn from(metadata: SelectedFileMetadata) -> Self {
        Self::from_metadata(metadata)
    }
}

impl From<FileData> for SelectedFile {
    fn from(file_data: FileData) -> Self {
        Self::from_file_data(file_data)
    }
}

/// A submit-time snapshot of selected files captured outside the form draft.
#[derive(Clone, Debug, PartialEq)]
pub struct FileSubmissionSnapshot<Model> {
    selections: BTreeMap<FieldIdentity, Vec<SelectedFile>>,
    _marker: PhantomData<fn() -> Model>,
}

impl<Model> FileSubmissionSnapshot<Model> {
    fn new(selections: BTreeMap<FieldIdentity, Vec<SelectedFile>>) -> Self {
        Self {
            selections,
            _marker: PhantomData,
        }
    }

    /// Returns selected files captured for a file field key at submit time.
    pub fn selected_files(&self, key: &FileFieldKey<Model>) -> Vec<SelectedFile> {
        key.normalize_selection(
            self.selections
                .get(&key.identity())
                .cloned()
                .unwrap_or_default(),
        )
    }
}

#[derive(Default)]
struct ReactiveSubscribers {
    subscribers: Subscribers,
}

impl ReactiveSubscribers {
    fn track_read(&self) {
        if let Some(context) = ReactiveContext::current() {
            context.subscribe(self.subscribers.clone());
        }
    }

    fn notify_changed(&self) {
        let mut subscribers = Vec::new();
        self.subscribers
            .visit(|subscriber| subscribers.push(*subscriber));
        let mut dropped = Vec::new();

        for subscriber in subscribers {
            if !subscriber.mark_dirty() {
                dropped.push(subscriber);
            }
        }

        if dropped.is_empty() {
            return;
        }

        for subscriber in dropped {
            self.subscribers.remove(&subscriber);
        }
    }
}

#[derive(Default)]
struct FieldReactivity {
    value: ReactiveSubscribers,
    metadata: ReactiveSubscribers,
    validation_errors: ReactiveSubscribers,
    visible_validation_errors: ReactiveSubscribers,
    parse_errors: ReactiveSubscribers,
}

impl FieldReactivity {
    fn notify_all(&self) {
        self.value.notify_changed();
        self.metadata.notify_changed();
        self.validation_errors.notify_changed();
        self.visible_validation_errors.notify_changed();
        self.parse_errors.notify_changed();
    }
}

#[derive(Default)]
struct FormReactivity {
    whole: ReactiveSubscribers,
    snapshot: ReactiveSubscribers,
    submit: ReactiveSubscribers,
    validation_errors: ReactiveSubscribers,
    visible_validation_errors: ReactiveSubscribers,
    form_validation_errors: ReactiveSubscribers,
    visible_form_validation_errors: ReactiveSubscribers,
    parse_errors: ReactiveSubscribers,
    fields: RefCell<BTreeMap<FieldIdentity, Rc<FieldReactivity>>>,
}

impl FormReactivity {
    fn track_read(&self) {
        self.whole.track_read();
    }

    fn track_snapshot(&self) {
        self.snapshot.track_read();
    }

    fn track_submit(&self) {
        self.submit.track_read();
    }

    fn track_validation_errors(&self) {
        self.validation_errors.track_read();
    }

    fn track_visible_validation_errors(&self) {
        self.visible_validation_errors.track_read();
    }

    fn track_form_validation_errors(&self) {
        self.form_validation_errors.track_read();
    }

    fn track_visible_form_validation_errors(&self) {
        self.visible_form_validation_errors.track_read();
    }

    fn track_parse_errors(&self) {
        self.parse_errors.track_read();
    }

    fn track_field_value(&self, field: &FieldIdentity) {
        self.field(field).value.track_read();
    }

    fn track_field_metadata(&self, field: &FieldIdentity) {
        self.field(field).metadata.track_read();
    }

    fn track_field_validation_errors(&self, field: &FieldIdentity) {
        self.field(field).validation_errors.track_read();
    }

    fn track_visible_field_validation_errors(&self, field: &FieldIdentity) {
        self.field(field).visible_validation_errors.track_read();
    }

    fn track_field_parse_errors(&self, field: &FieldIdentity) {
        self.field(field).parse_errors.track_read();
    }

    fn field(&self, field: &FieldIdentity) -> Rc<FieldReactivity> {
        Rc::clone(
            self.fields
                .borrow_mut()
                .entry(field.clone())
                .or_insert_with(|| Rc::new(FieldReactivity::default())),
        )
    }

    fn tracked_field_identities(&self) -> Vec<FieldIdentity> {
        self.fields.borrow().keys().cloned().collect()
    }
}

type FieldAsyncValidatorFn<Model, Value, Error> = dyn Fn(Value, AsyncValidatorContext<Model>) -> Pin<Box<dyn Future<Output = Vec<Error>>>>
    + 'static;

type FieldIdentityAsyncValidatorFn<Model, Error> =
    dyn Fn(AsyncValidatorContext<Model>) -> Pin<Box<dyn Future<Output = Vec<Error>>>> + 'static;

type FormAsyncValidatorFn<Model, Error> = dyn Fn(
        AsyncValidatorContext<Model>,
    ) -> Pin<Box<dyn Future<Output = Vec<FormValidationError<Error>>>>>
    + 'static;

type DelayFactoryFn = dyn Fn() -> Pin<Box<dyn Future<Output = ()>>> + 'static;

type TextParserFn<Value> = dyn Fn(&str) -> Result<Value, String> + 'static;

type TextFormatterFn<Value> = dyn Fn(&Value) -> String + 'static;

/// A field value type supported by the default numeric input helpers.
pub trait NumericInputValue: FromStr + fmt::Display + 'static
where
    Self::Err: fmt::Display + 'static,
{
}

macro_rules! impl_numeric_input_value {
    ($($value:ty),+ $(,)?) => {
        $(
            impl NumericInputValue for $value {}
        )+
    };
}

impl_numeric_input_value!(u8, u16, u32, u64, u128, usize);
impl_numeric_input_value!(i8, i16, i32, i64, i128, isize);
impl_numeric_input_value!(f32, f64);

mod controlled_choice {
    use super::field_binding::FieldBindingCore;

    pub(super) fn is_selected<Model, Value, Error>(
        binding: &FieldBindingCore<Model, Value, Error>,
        value: &Value,
    ) -> bool
    where
        Value: PartialEq,
    {
        binding.is_current(value)
    }

    pub(super) fn set_value<Model, Value, Error>(
        binding: &FieldBindingCore<Model, Value, Error>,
        value: Value,
    ) {
        binding.set_programmatic(value);
    }

    pub(super) fn select<Model, Value, Error>(
        binding: &FieldBindingCore<Model, Value, Error>,
        value: Value,
    ) {
        binding.set_user(value);
    }
}

mod field_binding {
    use super::*;

    /// Internal typed field behavior shared by direct and collection item bindings.
    ///
    /// This module keeps typed field reads, updates, metadata, validation, accessibility, and
    /// selector tracking together. Parsed-input bindings layer parser state on top through the
    /// narrow [`TypedFieldBinding`] interface below.
    pub(super) trait TypedFieldBinding<Value> {
        fn read_value_or<Result, Read>(&self, read: Read, stale: Result) -> Result
        where
            Read: FnOnce(&Value) -> Result;

        fn set_programmatic(&self, value: Value);

        fn set_user(&self, value: Value);

        fn mark_touched(&self);

        fn blur(&self);

        fn blur_without_validation(&self);
    }

    pub(super) struct CollectionFieldBindingCore<Model, Item, Value, Error = String> {
        handle: FormHandle<Model, Error>,
        collection_path: FieldPath<Model, Vec<Item>>,
        item: CollectionItem,
        path: FieldPath<Item, Value>,
    }

    impl<Model, Item, Value, Error> Clone for CollectionFieldBindingCore<Model, Item, Value, Error> {
        fn clone(&self) -> Self {
            Self {
                handle: self.handle.clone(),
                collection_path: self.collection_path.clone(),
                item: self.item,
                path: self.path.clone(),
            }
        }
    }

    impl<Model, Item, Value, Error> CollectionFieldBindingCore<Model, Item, Value, Error> {
        pub(super) fn new(
            handle: FormHandle<Model, Error>,
            collection_path: FieldPath<Model, Vec<Item>>,
            item: CollectionItem,
            path: FieldPath<Item, Value>,
        ) -> Self {
            Self {
                handle,
                collection_path,
                item,
                path,
            }
        }

        fn address(&self) -> CollectionItemFieldAddress {
            CollectionItemFieldAddress::new(
                &self.collection_path,
                self.item.identity(),
                self.item.index(),
                &self.path,
            )
        }

        fn identity(&self) -> FieldIdentity {
            self.address().identity()
        }

        pub(super) fn name(&self) -> String {
            self.address().field_name().to_owned()
        }

        pub(super) fn accessibility(&self) -> FieldAccessibility {
            let address = self.address();

            self.handle
                .field_accessibility_by_identity(address.identity(), address.accessibility_name())
        }

        pub(super) fn metadata(&self) -> FieldMetadata {
            self.handle
                .reactivity
                .track_field_metadata(&self.identity());
            self.handle.core.borrow().collection_item_field_metadata(
                self.collection_path.clone(),
                self.item.identity(),
                self.path.clone(),
            )
        }

        pub(super) fn is_touched(&self) -> bool {
            self.metadata().is_touched()
        }

        pub(super) fn is_blurred(&self) -> bool {
            self.metadata().is_blurred()
        }

        pub(super) fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
        where
            Error: Clone,
        {
            self.handle
                .field_validation_errors_by_identity(&self.identity())
        }

        pub(super) fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
        where
            Error: Clone,
        {
            self.handle
                .visible_field_validation_errors_by_identity(&self.identity())
        }

        pub(super) fn read_value<Result>(
            &self,
            read: impl FnOnce(&Value) -> Result,
            stale: Result,
        ) -> Result {
            let field = self.identity();
            self.handle.reactivity.track_field_value(&field);
            let core = self.handle.core.borrow();

            core.collection_item_field_value(
                self.collection_path.clone(),
                self.item.identity(),
                self.path.clone(),
            )
            .map(read)
            .unwrap_or(stale)
        }

        pub(super) fn value(&self) -> Option<Value>
        where
            Value: Clone,
        {
            self.handle.reactivity.track_field_value(&self.identity());
            let core = self.handle.core.borrow();

            core.collection_item_field_value(
                self.collection_path.clone(),
                self.item.identity(),
                self.path.clone(),
            )
            .cloned()
        }

        pub(super) fn expect_value(&self) -> Value
        where
            Value: Clone,
        {
            self.value()
                .expect("collection item child field should exist while its binding is rendered")
        }

        pub(super) fn is_current(&self, value: &Value) -> bool
        where
            Value: PartialEq,
        {
            self.read_value(|current| current == value, false)
        }

        pub(super) fn set_programmatic(&self, value: Value) {
            self.handle.set_collection_item_field(
                self.collection_path.clone(),
                self.item.identity(),
                self.path.clone(),
                value,
            );
        }

        pub(super) fn set_user(&self, value: Value) {
            self.handle.set_user_collection_item_field(
                self.collection_path.clone(),
                self.item.identity(),
                self.path.clone(),
                value,
            );
        }

        pub(super) fn mark_touched(&self) {
            self.handle.mark_collection_item_field_touched(
                self.collection_path.clone(),
                self.item.identity(),
                self.path.clone(),
            );
        }

        pub(super) fn blur(&self) {
            self.handle.mark_collection_item_field_blurred(
                self.collection_path.clone(),
                self.item.identity(),
                self.path.clone(),
            );
        }

        pub(super) fn blur_without_validation(&self) {
            self.handle
                .mark_collection_item_field_blurred_without_validation(
                    self.collection_path.clone(),
                    self.item.identity(),
                    self.path.clone(),
                );
        }
    }

    impl<Model, Item, Value, Error> TypedFieldBinding<Value>
        for CollectionFieldBindingCore<Model, Item, Value, Error>
    {
        fn read_value_or<Result, Read>(&self, read: Read, stale: Result) -> Result
        where
            Read: FnOnce(&Value) -> Result,
        {
            self.read_value(read, stale)
        }

        fn set_programmatic(&self, value: Value) {
            CollectionFieldBindingCore::set_programmatic(self, value);
        }

        fn set_user(&self, value: Value) {
            CollectionFieldBindingCore::set_user(self, value);
        }

        fn mark_touched(&self) {
            CollectionFieldBindingCore::mark_touched(self);
        }

        fn blur(&self) {
            CollectionFieldBindingCore::blur(self);
        }

        fn blur_without_validation(&self) {
            CollectionFieldBindingCore::blur_without_validation(self);
        }
    }

    pub(super) struct FieldBindingCore<Model, Value, Error = String> {
        handle: FormHandle<Model, Error>,
        path: FieldPath<Model, Value>,
    }

    impl<Model, Value, Error> Clone for FieldBindingCore<Model, Value, Error> {
        fn clone(&self) -> Self {
            Self {
                handle: self.handle.clone(),
                path: self.path.clone(),
            }
        }
    }

    impl<Model, Value, Error> FieldBindingCore<Model, Value, Error> {
        pub(super) fn new(handle: FormHandle<Model, Error>, path: FieldPath<Model, Value>) -> Self {
            Self { handle, path }
        }

        pub(super) fn name(&self) -> &str {
            self.path.field_name()
        }

        pub(super) fn accessibility(&self) -> FieldAccessibility {
            self.handle.field_accessibility(self.path.clone())
        }

        pub(super) fn metadata(&self) -> FieldMetadata {
            self.handle.field_metadata(self.path.clone())
        }

        pub(super) fn is_touched(&self) -> bool {
            self.metadata().is_touched()
        }

        pub(super) fn is_blurred(&self) -> bool {
            self.metadata().is_blurred()
        }

        pub(super) fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
        where
            Error: Clone,
        {
            self.handle.field_validation_errors(self.path.clone())
        }

        pub(super) fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
        where
            Error: Clone,
        {
            self.handle
                .visible_field_validation_errors(self.path.clone())
        }

        pub(super) fn read_value<Result>(&self, read: impl FnOnce(&Value) -> Result) -> Result {
            self.handle
                .reactivity
                .track_field_value(&self.path.identity());
            let core = self.handle.core.borrow();
            read(core.field_value(self.path.clone()))
        }

        pub(super) fn value(&self) -> Value
        where
            Value: Clone,
        {
            self.read_value(Clone::clone)
        }

        pub(super) fn is_current(&self, value: &Value) -> bool
        where
            Value: PartialEq,
        {
            self.read_value(|current| current == value)
        }

        pub(super) fn set_programmatic(&self, value: Value) {
            self.handle.set_field(self.path.clone(), value);
        }

        pub(super) fn set_user(&self, value: Value) {
            self.handle.set_user_field(self.path.clone(), value);
        }

        pub(super) fn mark_touched(&self) {
            self.handle.mark_field_touched(self.path.clone());
        }

        pub(super) fn blur(&self) {
            self.handle.mark_field_blurred(self.path.clone());
        }

        pub(super) fn blur_without_validation(&self) {
            self.handle
                .mark_field_blurred_without_validation(self.path.clone());
        }
    }

    impl<Model, Value, Error> TypedFieldBinding<Value> for FieldBindingCore<Model, Value, Error> {
        fn read_value_or<Result, Read>(&self, read: Read, stale: Result) -> Result
        where
            Read: FnOnce(&Value) -> Result,
        {
            let _ = stale;
            self.read_value(read)
        }

        fn set_programmatic(&self, value: Value) {
            FieldBindingCore::set_programmatic(self, value);
        }

        fn set_user(&self, value: Value) {
            FieldBindingCore::set_user(self, value);
        }

        fn mark_touched(&self) {
            FieldBindingCore::mark_touched(self);
        }

        fn blur(&self) {
            FieldBindingCore::blur(self);
        }

        fn blur_without_validation(&self) {
            FieldBindingCore::blur_without_validation(self);
        }
    }
}

use field_binding::{CollectionFieldBindingCore, FieldBindingCore};

mod parsed_input {
    use super::field_binding::TypedFieldBinding;
    use super::*;

    pub(super) fn value<Binding, Value>(
        binding: &Binding,
        registration: &ParseBindingRegistration,
        formatter: &Rc<TextFormatterFn<Value>>,
    ) -> String
    where
        Binding: TypedFieldBinding<Value>,
    {
        match parse_error(registration) {
            Some(error) => error.raw_value,
            None => binding.read_value_or(|value| formatter.as_ref()(value), String::new()),
        }
    }

    pub(super) fn set_value<Binding, Value>(
        binding: &Binding,
        registration: &ParseBindingRegistration,
        value: Value,
    ) where
        Binding: TypedFieldBinding<Value>,
    {
        registration.clear_error();
        binding.set_programmatic(value);
    }

    pub(super) fn on_input<Binding, Value>(
        binding: &Binding,
        registration: &ParseBindingRegistration,
        parser: &Rc<TextParserFn<Value>>,
        value: impl Into<String>,
    ) where
        Binding: TypedFieldBinding<Value>,
    {
        let raw_value = value.into();

        match parser.as_ref()(&raw_value) {
            Ok(value) => {
                registration.clear_error();
                binding.set_user(value);
            }
            Err(error) => {
                binding.mark_touched();
                registration.set_error(raw_value, error);
            }
        }
    }

    pub(super) fn on_blur<Binding, Value>(
        binding: &Binding,
        registration: &ParseBindingRegistration,
    ) where
        Binding: TypedFieldBinding<Value>,
    {
        if parse_error(registration).is_some() {
            binding.blur_without_validation();
        } else {
            binding.blur();
        }
    }

    pub(super) fn parse_error(registration: &ParseBindingRegistration) -> Option<ParseError> {
        registration.parse_error()
    }
}

struct ParseBindingRegistrationInner {
    adapter: AdapterRuntime,
    reactivity: Rc<FormReactivity>,
    id: ParseBindingId,
    field: FieldIdentity,
}

impl Drop for ParseBindingRegistrationInner {
    fn drop(&mut self) {
        if self.adapter.unregister_parse_binding(self.id) {
            self.reactivity
                .notify_selector_transition(SelectorTransition::ParseChanged(self.field.clone()));
        }
    }
}

struct ParseBindingRegistration {
    inner: Rc<ParseBindingRegistrationInner>,
}

impl Clone for ParseBindingRegistration {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl ParseBindingRegistration {
    fn new(
        adapter: AdapterRuntime,
        reactivity: Rc<FormReactivity>,
        id: ParseBindingId,
        field: FieldIdentity,
    ) -> Self {
        Self {
            inner: Rc::new(ParseBindingRegistrationInner {
                adapter,
                reactivity,
                id,
                field,
            }),
        }
    }

    fn parse_error(&self) -> Option<ParseError> {
        self.inner
            .reactivity
            .track_field_parse_errors(&self.inner.field);
        self.inner.adapter.parse_error(self.inner.id)
    }

    fn set_error(&self, raw_value: String, message: String) {
        let field = self.inner.field.clone();

        self.inner
            .adapter
            .set_parse_error(self.inner.id, raw_value, message);
        self.inner
            .reactivity
            .notify_selector_transition(SelectorTransition::ParseChanged(field));
    }

    fn clear_error(&self) {
        let field = self.inner.field.clone();

        self.inner.adapter.clear_parse_error(self.inner.id);
        self.inner
            .reactivity
            .notify_selector_transition(SelectorTransition::ParseChanged(field));
    }
}

struct CollectionParsedTextHookState<Value> {
    registration: ParseBindingRegistration,
    parser: Rc<TextParserFn<Value>>,
    formatter: Rc<TextFormatterFn<Value>>,
}

impl<Value> Clone for CollectionParsedTextHookState<Value> {
    fn clone(&self) -> Self {
        Self {
            registration: self.registration.clone(),
            parser: Rc::clone(&self.parser),
            formatter: Rc::clone(&self.formatter),
        }
    }
}

impl<Value> CollectionParsedTextHookState<Value> {
    fn new<Model, Item, Error, Parser, ParserError, Formatter>(
        item: CollectionItemBinding<Model, Item, Error>,
        path: FieldPath<Item, Value>,
        parser: Parser,
        formatter: Formatter,
    ) -> Self
    where
        Value: 'static,
        Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
        ParserError: fmt::Display + 'static,
        Formatter: Fn(&Value) -> String + 'static,
    {
        let field = CollectionItemFieldAddress::identity_for(
            &item.collection_path,
            item.item.identity(),
            &path,
        );
        let registration = item.handle.register_parse_binding(field);
        let parser = Rc::new(move |value: &str| parser(value).map_err(|error| error.to_string()));

        Self {
            registration,
            parser,
            formatter: Rc::new(formatter),
        }
    }
}

type FieldListenerId = u64;
type FieldListenerCallback<Model, Error> =
    Box<dyn FnMut(FieldListenerContext<Model, Error>) + 'static>;
type DebouncedFieldListenerSchedule<Model, Error> =
    dyn Fn(FormHandle<Model, Error>, FieldIdentity, FieldUpdateOrigin) + 'static;
type FieldBlurListenerId = u64;
type FieldBlurListenerCallback<Model, Error> =
    Box<dyn FnMut(FieldBlurListenerContext<Model, Error>) + 'static>;
type FieldBindingListenerId = u64;
type FieldBindingListenerCallback<Model, Error> =
    Box<dyn FnMut(FieldBindingListenerContext<Model, Error>) + 'static>;
type FormListenerId = u64;
type FormListenerCallback<Model, Error> =
    Box<dyn FnMut(FormListenerContext<Model, Error>) + 'static>;
type DebouncedFormListenerSchedule<Model, Error> = dyn Fn(FormHandle<Model, Error>, FieldIdentity, String, FieldUpdateOrigin, FormListenerEvent)
    + 'static;
type FormBlurListenerId = u64;
type FormBlurListenerCallback<Model, Error> =
    Box<dyn FnMut(FormBlurListenerContext<Model, Error>) + 'static>;
type SubmitListenerId = u64;
type SubmitListenerCallback<Model, Error> =
    Box<dyn FnMut(SubmitListenerContext<Model, Error>) + 'static>;

struct FieldListenerEntry<Model, Error> {
    id: FieldListenerId,
    field: FieldIdentity,
    origin: Option<FieldUpdateOrigin>,
    callback: Rc<RefCell<FieldListenerCallback<Model, Error>>>,
}

struct DebouncedFieldListenerEntry<Model, Error> {
    id: FieldListenerId,
    field: FieldIdentity,
    origin: Option<FieldUpdateOrigin>,
    generation: Rc<Cell<u64>>,
    active: Rc<Cell<bool>>,
    schedule: Rc<DebouncedFieldListenerSchedule<Model, Error>>,
}

struct DebouncedFieldListenerDispatch<Model, Error> {
    schedule: Rc<DebouncedFieldListenerSchedule<Model, Error>>,
}

struct FieldBlurListenerEntry<Model, Error> {
    id: FieldBlurListenerId,
    field: FieldIdentity,
    callback: Rc<RefCell<FieldBlurListenerCallback<Model, Error>>>,
}

struct FieldBindingListenerEntry<Model, Error> {
    id: FieldBindingListenerId,
    field: FieldIdentity,
    callback: Rc<RefCell<FieldBindingListenerCallback<Model, Error>>>,
}

struct FieldBindingListenerUnregistration<Model, Error> {
    field: FieldIdentity,
    callback: Rc<RefCell<FieldBindingListenerCallback<Model, Error>>>,
    mounted_count: usize,
}

struct FormListenerEntry<Model, Error> {
    id: FormListenerId,
    origin: Option<FieldUpdateOrigin>,
    callback: Rc<RefCell<FormListenerCallback<Model, Error>>>,
}

struct DebouncedFormListenerEntry<Model, Error> {
    id: FormListenerId,
    origin: Option<FieldUpdateOrigin>,
    generation: Rc<Cell<u64>>,
    active: Rc<Cell<bool>>,
    schedule: Rc<DebouncedFormListenerSchedule<Model, Error>>,
}

struct DebouncedFormListenerDispatch<Model, Error> {
    schedule: Rc<DebouncedFormListenerSchedule<Model, Error>>,
}

struct FormBlurListenerEntry<Model, Error> {
    id: FormBlurListenerId,
    callback: Rc<RefCell<FormBlurListenerCallback<Model, Error>>>,
}

struct SubmitListenerEntry<Model, Error> {
    id: SubmitListenerId,
    callback: Rc<RefCell<SubmitListenerCallback<Model, Error>>>,
}

struct FormListeners<Model, Error> {
    next_id: FieldListenerId,
    field_listeners: Vec<FieldListenerEntry<Model, Error>>,
    debounced_field_listeners: Vec<DebouncedFieldListenerEntry<Model, Error>>,
    field_blur_listeners: Vec<FieldBlurListenerEntry<Model, Error>>,
    field_binding_listeners: Vec<FieldBindingListenerEntry<Model, Error>>,
    mounted_field_bindings: BTreeMap<FieldIdentity, usize>,
    form_listeners: Vec<FormListenerEntry<Model, Error>>,
    debounced_form_listeners: Vec<DebouncedFormListenerEntry<Model, Error>>,
    form_blur_listeners: Vec<FormBlurListenerEntry<Model, Error>>,
    submit_listeners: Vec<SubmitListenerEntry<Model, Error>>,
}

impl<Model, Error> Default for FormListeners<Model, Error> {
    fn default() -> Self {
        Self {
            next_id: 0,
            field_listeners: Vec::new(),
            debounced_field_listeners: Vec::new(),
            field_blur_listeners: Vec::new(),
            field_binding_listeners: Vec::new(),
            mounted_field_bindings: BTreeMap::new(),
            form_listeners: Vec::new(),
            debounced_form_listeners: Vec::new(),
            form_blur_listeners: Vec::new(),
            submit_listeners: Vec::new(),
        }
    }
}

fn bump_debounced_generation(generation: &Cell<u64>) -> u64 {
    let next = generation
        .get()
        .checked_add(1)
        .expect("debounced listener generation exhausted");
    generation.set(next);
    next
}

impl<Model, Error> FormListeners<Model, Error> {
    fn register_field_listener<Listener>(
        &mut self,
        field: FieldIdentity,
        origin: Option<FieldUpdateOrigin>,
        listener: Listener,
    ) -> FieldListenerId
    where
        Listener: FnMut(FieldListenerContext<Model, Error>) + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.field_listeners.push(FieldListenerEntry {
            id,
            field,
            origin,
            callback: Rc::new(RefCell::new(Box::new(listener))),
        });

        id
    }

    fn unregister_field_listener(&mut self, id: FieldListenerId) {
        self.field_listeners.retain(|listener| listener.id != id);
    }

    fn register_debounced_field_listener<Listener>(
        &mut self,
        field: FieldIdentity,
        origin: Option<FieldUpdateOrigin>,
        delay: Rc<DelayFactoryFn>,
        listener: Listener,
    ) -> FieldListenerId
    where
        Model: 'static,
        Error: 'static,
        Listener: FnMut(FieldListenerContext<Model, Error>) + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        let callback: Rc<RefCell<FieldListenerCallback<Model, Error>>> =
            Rc::new(RefCell::new(Box::new(listener)));
        let generation = Rc::new(Cell::new(0));
        let active = Rc::new(Cell::new(true));
        let schedule_delay = Rc::clone(&delay);
        let schedule_callback = Rc::clone(&callback);
        let schedule_generation = Rc::clone(&generation);
        let schedule_active = Rc::clone(&active);
        let schedule_runtime = dioxus_core::Runtime::current();
        let schedule_scope = schedule_runtime.current_scope_id();
        let schedule = Rc::new(
            move |handle: FormHandle<Model, Error>,
                  field: FieldIdentity,
                  origin: FieldUpdateOrigin| {
                let generation = bump_debounced_generation(&schedule_generation);
                let delay = (schedule_delay)();
                let listener_generation = Rc::clone(&schedule_generation);
                let active = Rc::clone(&schedule_active);
                let listener_callback = Rc::clone(&schedule_callback);

                schedule_runtime.in_scope(schedule_scope, || {
                    dioxus_core::spawn(async move {
                        delay.await;

                        if !active.get() || listener_generation.get() != generation {
                            return;
                        }

                        let context = FieldListenerContext {
                            form: handle,
                            field,
                            origin,
                        };
                        let Ok(mut callback) = listener_callback.try_borrow_mut() else {
                            panic!(
                                "debounced field listener re-entered while it was already running; \
                             avoid listener-caused debounced cycles"
                            );
                        };

                        (callback.as_mut())(context);
                    });
                });
            },
        );
        self.debounced_field_listeners
            .push(DebouncedFieldListenerEntry {
                id,
                field,
                origin,
                generation,
                active,
                schedule,
            });

        id
    }

    fn unregister_debounced_field_listener(&mut self, id: FieldListenerId) {
        for listener in &self.debounced_field_listeners {
            if listener.id == id {
                listener.active.set(false);
                bump_debounced_generation(&listener.generation);
            }
        }

        self.debounced_field_listeners
            .retain(|listener| listener.id != id);
    }

    fn register_field_blur_listener<Listener>(
        &mut self,
        field: FieldIdentity,
        listener: Listener,
    ) -> FieldBlurListenerId
    where
        Listener: FnMut(FieldBlurListenerContext<Model, Error>) + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.field_blur_listeners.push(FieldBlurListenerEntry {
            id,
            field,
            callback: Rc::new(RefCell::new(Box::new(listener))),
        });

        id
    }

    fn unregister_field_blur_listener(&mut self, id: FieldBlurListenerId) {
        self.field_blur_listeners
            .retain(|listener| listener.id != id);
    }

    fn register_field_binding_listener<Listener>(
        &mut self,
        field: FieldIdentity,
        listener: Listener,
    ) -> (
        FieldBindingListenerId,
        Rc<RefCell<FieldBindingListenerCallback<Model, Error>>>,
        usize,
    )
    where
        Listener: FnMut(FieldBindingListenerContext<Model, Error>) + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        let mounted_count = self
            .mounted_field_bindings
            .get(&field)
            .copied()
            .unwrap_or_default();
        let callback = Rc::new(RefCell::new(
            Box::new(listener) as FieldBindingListenerCallback<Model, Error>
        ));
        self.field_binding_listeners
            .push(FieldBindingListenerEntry {
                id,
                field,
                callback: Rc::clone(&callback),
            });

        (id, callback, mounted_count)
    }

    fn unregister_field_binding_listener(
        &mut self,
        id: FieldBindingListenerId,
    ) -> Option<FieldBindingListenerUnregistration<Model, Error>> {
        let index = self
            .field_binding_listeners
            .iter()
            .position(|listener| listener.id == id)?;
        let listener = self.field_binding_listeners.remove(index);
        let mounted_count = self
            .mounted_field_bindings
            .get(&listener.field)
            .copied()
            .unwrap_or_default();

        Some(FieldBindingListenerUnregistration {
            field: listener.field,
            callback: listener.callback,
            mounted_count,
        })
    }

    fn record_field_binding_lifecycle(
        &mut self,
        field: &FieldIdentity,
        lifecycle: FieldBindingLifecycle,
    ) {
        match lifecycle {
            FieldBindingLifecycle::Mounted => {
                *self
                    .mounted_field_bindings
                    .entry(field.clone())
                    .or_default() += 1;
            }
            FieldBindingLifecycle::Unmounted => {
                if let Some(count) = self.mounted_field_bindings.get_mut(field) {
                    *count -= 1;

                    if *count == 0 {
                        self.mounted_field_bindings.remove(field);
                    }
                }
            }
        }
    }

    fn register_form_listener<Listener>(
        &mut self,
        origin: Option<FieldUpdateOrigin>,
        listener: Listener,
    ) -> FormListenerId
    where
        Listener: FnMut(FormListenerContext<Model, Error>) + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.form_listeners.push(FormListenerEntry {
            id,
            origin,
            callback: Rc::new(RefCell::new(Box::new(listener))),
        });

        id
    }

    fn unregister_form_listener(&mut self, id: FormListenerId) {
        self.form_listeners.retain(|listener| listener.id != id);
    }

    fn register_debounced_form_listener<Listener>(
        &mut self,
        origin: Option<FieldUpdateOrigin>,
        delay: Rc<DelayFactoryFn>,
        listener: Listener,
    ) -> FormListenerId
    where
        Model: 'static,
        Error: 'static,
        Listener: FnMut(FormListenerContext<Model, Error>) + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        let callback: Rc<RefCell<FormListenerCallback<Model, Error>>> =
            Rc::new(RefCell::new(Box::new(listener)));
        let generation = Rc::new(Cell::new(0));
        let active = Rc::new(Cell::new(true));
        let schedule_delay = Rc::clone(&delay);
        let schedule_callback = Rc::clone(&callback);
        let schedule_generation = Rc::clone(&generation);
        let schedule_active = Rc::clone(&active);
        let schedule_runtime = dioxus_core::Runtime::current();
        let schedule_scope = schedule_runtime.current_scope_id();
        let schedule = Rc::new(
            move |handle: FormHandle<Model, Error>,
                  field: FieldIdentity,
                  field_name: String,
                  origin: FieldUpdateOrigin,
                  event: FormListenerEvent| {
                let generation = bump_debounced_generation(&schedule_generation);
                let delay = (schedule_delay)();
                let listener_generation = Rc::clone(&schedule_generation);
                let active = Rc::clone(&schedule_active);
                let listener_callback = Rc::clone(&schedule_callback);

                schedule_runtime.in_scope(schedule_scope, || {
                    dioxus_core::spawn(async move {
                        delay.await;

                        if !active.get() || listener_generation.get() != generation {
                            return;
                        }

                        let context = FormListenerContext {
                            form: handle,
                            field,
                            field_name,
                            event,
                            origin,
                        };
                        let Ok(mut callback) = listener_callback.try_borrow_mut() else {
                            panic!(
                                "debounced form listener re-entered while it was already running; \
                                 avoid listener-caused debounced cycles"
                            );
                        };

                        (callback.as_mut())(context);
                    });
                });
            },
        );
        self.debounced_form_listeners
            .push(DebouncedFormListenerEntry {
                id,
                origin,
                generation,
                active,
                schedule,
            });

        id
    }

    fn unregister_debounced_form_listener(&mut self, id: FormListenerId) {
        for listener in &self.debounced_form_listeners {
            if listener.id == id {
                listener.active.set(false);
                bump_debounced_generation(&listener.generation);
            }
        }

        self.debounced_form_listeners
            .retain(|listener| listener.id != id);
    }

    fn register_form_blur_listener<Listener>(&mut self, listener: Listener) -> FormBlurListenerId
    where
        Listener: FnMut(FormBlurListenerContext<Model, Error>) + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.form_blur_listeners.push(FormBlurListenerEntry {
            id,
            callback: Rc::new(RefCell::new(Box::new(listener))),
        });

        id
    }

    fn unregister_form_blur_listener(&mut self, id: FormBlurListenerId) {
        self.form_blur_listeners
            .retain(|listener| listener.id != id);
    }

    fn register_submit_listener<Listener>(&mut self, listener: Listener) -> SubmitListenerId
    where
        Listener: FnMut(SubmitListenerContext<Model, Error>) + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.submit_listeners.push(SubmitListenerEntry {
            id,
            callback: Rc::new(RefCell::new(Box::new(listener))),
        });

        id
    }

    fn unregister_submit_listener(&mut self, id: SubmitListenerId) {
        self.submit_listeners.retain(|listener| listener.id != id);
    }

    fn field_callbacks(
        &self,
        field: &FieldIdentity,
        origin: FieldUpdateOrigin,
    ) -> Vec<Rc<RefCell<FieldListenerCallback<Model, Error>>>> {
        self.field_listeners
            .iter()
            .filter(|listener| {
                &listener.field == field
                    && match listener.origin {
                        Some(listener_origin) => listener_origin == origin,
                        None => true,
                    }
            })
            .map(|listener| Rc::clone(&listener.callback))
            .collect()
    }

    fn debounced_field_callbacks(
        &self,
        field: &FieldIdentity,
        origin: FieldUpdateOrigin,
    ) -> Vec<DebouncedFieldListenerDispatch<Model, Error>> {
        self.debounced_field_listeners
            .iter()
            .filter(|listener| {
                &listener.field == field
                    && match listener.origin {
                        Some(listener_origin) => listener_origin == origin,
                        None => true,
                    }
            })
            .map(|listener| DebouncedFieldListenerDispatch {
                schedule: Rc::clone(&listener.schedule),
            })
            .collect()
    }

    fn field_blur_callbacks(
        &self,
        field: &FieldIdentity,
    ) -> Vec<Rc<RefCell<FieldBlurListenerCallback<Model, Error>>>> {
        self.field_blur_listeners
            .iter()
            .filter(|listener| &listener.field == field)
            .map(|listener| Rc::clone(&listener.callback))
            .collect()
    }

    fn field_binding_callbacks(
        &self,
        field: &FieldIdentity,
    ) -> Vec<Rc<RefCell<FieldBindingListenerCallback<Model, Error>>>> {
        self.field_binding_listeners
            .iter()
            .filter(|listener| &listener.field == field)
            .map(|listener| Rc::clone(&listener.callback))
            .collect()
    }

    fn form_callbacks(
        &self,
        origin: FieldUpdateOrigin,
    ) -> Vec<Rc<RefCell<FormListenerCallback<Model, Error>>>> {
        self.form_listeners
            .iter()
            .filter(|listener| match listener.origin {
                Some(listener_origin) => listener_origin == origin,
                None => true,
            })
            .map(|listener| Rc::clone(&listener.callback))
            .collect()
    }

    fn debounced_form_callbacks(
        &self,
        origin: FieldUpdateOrigin,
    ) -> Vec<DebouncedFormListenerDispatch<Model, Error>> {
        self.debounced_form_listeners
            .iter()
            .filter(|listener| match listener.origin {
                Some(listener_origin) => listener_origin == origin,
                None => true,
            })
            .map(|listener| DebouncedFormListenerDispatch {
                schedule: Rc::clone(&listener.schedule),
            })
            .collect()
    }

    fn form_blur_callbacks(&self) -> Vec<Rc<RefCell<FormBlurListenerCallback<Model, Error>>>> {
        self.form_blur_listeners
            .iter()
            .map(|listener| Rc::clone(&listener.callback))
            .collect()
    }

    fn submit_callbacks(&self) -> Vec<Rc<RefCell<SubmitListenerCallback<Model, Error>>>> {
        self.submit_listeners
            .iter()
            .map(|listener| Rc::clone(&listener.callback))
            .collect()
    }
}

struct FieldListenerRegistrationInner<Model, Error> {
    listeners: Rc<RefCell<FormListeners<Model, Error>>>,
    id: FieldListenerId,
}

impl<Model, Error> Drop for FieldListenerRegistrationInner<Model, Error> {
    fn drop(&mut self) {
        self.listeners
            .borrow_mut()
            .unregister_field_listener(self.id);
    }
}

struct FieldListenerRegistration<Model, Error> {
    inner: Rc<FieldListenerRegistrationInner<Model, Error>>,
}

impl<Model, Error> Clone for FieldListenerRegistration<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<Model, Error> FieldListenerRegistration<Model, Error> {
    fn new(listeners: Rc<RefCell<FormListeners<Model, Error>>>, id: FieldListenerId) -> Self {
        Self {
            inner: Rc::new(FieldListenerRegistrationInner { listeners, id }),
        }
    }
}

struct DebouncedFieldListenerRegistrationInner<Model, Error> {
    listeners: Rc<RefCell<FormListeners<Model, Error>>>,
    id: FieldListenerId,
}

impl<Model, Error> Drop for DebouncedFieldListenerRegistrationInner<Model, Error> {
    fn drop(&mut self) {
        self.listeners
            .borrow_mut()
            .unregister_debounced_field_listener(self.id);
    }
}

struct DebouncedFieldListenerRegistration<Model, Error> {
    inner: Rc<DebouncedFieldListenerRegistrationInner<Model, Error>>,
}

impl<Model, Error> Clone for DebouncedFieldListenerRegistration<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<Model, Error> DebouncedFieldListenerRegistration<Model, Error> {
    fn new(listeners: Rc<RefCell<FormListeners<Model, Error>>>, id: FieldListenerId) -> Self {
        Self {
            inner: Rc::new(DebouncedFieldListenerRegistrationInner { listeners, id }),
        }
    }
}

struct FieldBlurListenerRegistrationInner<Model, Error> {
    listeners: Rc<RefCell<FormListeners<Model, Error>>>,
    id: FieldBlurListenerId,
}

impl<Model, Error> Drop for FieldBlurListenerRegistrationInner<Model, Error> {
    fn drop(&mut self) {
        self.listeners
            .borrow_mut()
            .unregister_field_blur_listener(self.id);
    }
}

struct FieldBlurListenerRegistration<Model, Error> {
    inner: Rc<FieldBlurListenerRegistrationInner<Model, Error>>,
}

impl<Model, Error> Clone for FieldBlurListenerRegistration<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<Model, Error> FieldBlurListenerRegistration<Model, Error> {
    fn new(listeners: Rc<RefCell<FormListeners<Model, Error>>>, id: FieldBlurListenerId) -> Self {
        Self {
            inner: Rc::new(FieldBlurListenerRegistrationInner { listeners, id }),
        }
    }
}

struct FieldBindingListenerRegistrationInner<Model, Error> {
    handle: FormHandle<Model, Error>,
    id: FieldBindingListenerId,
}

impl<Model, Error> Drop for FieldBindingListenerRegistrationInner<Model, Error> {
    fn drop(&mut self) {
        let Some(unregistration) = self
            .handle
            .listeners
            .borrow_mut()
            .unregister_field_binding_listener(self.id)
        else {
            return;
        };

        for _ in 0..unregistration.mounted_count {
            self.handle.dispatch_field_binding_callback(
                Rc::clone(&unregistration.callback),
                unregistration.field.clone(),
                FieldBindingLifecycle::Unmounted,
            );
        }
    }
}

struct FieldBindingListenerRegistration<Model, Error> {
    inner: Rc<FieldBindingListenerRegistrationInner<Model, Error>>,
}

impl<Model, Error> Clone for FieldBindingListenerRegistration<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<Model, Error> FieldBindingListenerRegistration<Model, Error> {
    fn new(handle: FormHandle<Model, Error>, id: FieldBindingListenerId) -> Self {
        Self {
            inner: Rc::new(FieldBindingListenerRegistrationInner { handle, id }),
        }
    }
}

struct FormListenerRegistrationInner<Model, Error> {
    listeners: Rc<RefCell<FormListeners<Model, Error>>>,
    id: FormListenerId,
}

impl<Model, Error> Drop for FormListenerRegistrationInner<Model, Error> {
    fn drop(&mut self) {
        self.listeners
            .borrow_mut()
            .unregister_form_listener(self.id);
    }
}

struct FormListenerRegistration<Model, Error> {
    inner: Rc<FormListenerRegistrationInner<Model, Error>>,
}

impl<Model, Error> Clone for FormListenerRegistration<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<Model, Error> FormListenerRegistration<Model, Error> {
    fn new(listeners: Rc<RefCell<FormListeners<Model, Error>>>, id: FormListenerId) -> Self {
        Self {
            inner: Rc::new(FormListenerRegistrationInner { listeners, id }),
        }
    }
}

struct DebouncedFormListenerRegistrationInner<Model, Error> {
    listeners: Rc<RefCell<FormListeners<Model, Error>>>,
    id: FormListenerId,
}

impl<Model, Error> Drop for DebouncedFormListenerRegistrationInner<Model, Error> {
    fn drop(&mut self) {
        self.listeners
            .borrow_mut()
            .unregister_debounced_form_listener(self.id);
    }
}

struct DebouncedFormListenerRegistration<Model, Error> {
    inner: Rc<DebouncedFormListenerRegistrationInner<Model, Error>>,
}

impl<Model, Error> Clone for DebouncedFormListenerRegistration<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<Model, Error> DebouncedFormListenerRegistration<Model, Error> {
    fn new(listeners: Rc<RefCell<FormListeners<Model, Error>>>, id: FormListenerId) -> Self {
        Self {
            inner: Rc::new(DebouncedFormListenerRegistrationInner { listeners, id }),
        }
    }
}

struct FormBlurListenerRegistrationInner<Model, Error> {
    listeners: Rc<RefCell<FormListeners<Model, Error>>>,
    id: FormBlurListenerId,
}

impl<Model, Error> Drop for FormBlurListenerRegistrationInner<Model, Error> {
    fn drop(&mut self) {
        self.listeners
            .borrow_mut()
            .unregister_form_blur_listener(self.id);
    }
}

struct FormBlurListenerRegistration<Model, Error> {
    inner: Rc<FormBlurListenerRegistrationInner<Model, Error>>,
}

impl<Model, Error> Clone for FormBlurListenerRegistration<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<Model, Error> FormBlurListenerRegistration<Model, Error> {
    fn new(listeners: Rc<RefCell<FormListeners<Model, Error>>>, id: FormBlurListenerId) -> Self {
        Self {
            inner: Rc::new(FormBlurListenerRegistrationInner { listeners, id }),
        }
    }
}

struct SubmitListenerRegistrationInner<Model, Error> {
    listeners: Rc<RefCell<FormListeners<Model, Error>>>,
    id: SubmitListenerId,
}

impl<Model, Error> Drop for SubmitListenerRegistrationInner<Model, Error> {
    fn drop(&mut self) {
        self.listeners
            .borrow_mut()
            .unregister_submit_listener(self.id);
    }
}

struct SubmitListenerRegistration<Model, Error> {
    inner: Rc<SubmitListenerRegistrationInner<Model, Error>>,
}

impl<Model, Error> Clone for SubmitListenerRegistration<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<Model, Error> SubmitListenerRegistration<Model, Error> {
    fn new(listeners: Rc<RefCell<FormListeners<Model, Error>>>, id: SubmitListenerId) -> Self {
        Self {
            inner: Rc::new(SubmitListenerRegistrationInner { listeners, id }),
        }
    }
}

/// Context supplied to a field listener callback.
pub struct FieldListenerContext<Model, Error = String> {
    form: FormHandle<Model, Error>,
    field: FieldIdentity,
    origin: FieldUpdateOrigin,
}

impl<Model, Error> FieldListenerContext<Model, Error> {
    /// Returns the form handle that emitted the listener event.
    pub fn form(&self) -> FormHandle<Model, Error> {
        self.form.clone()
    }

    /// Returns the field whose semantic event triggered this listener.
    pub fn field_identity(&self) -> FieldIdentity {
        self.field.clone()
    }

    /// Returns whether the field update came from user interaction or application code.
    pub const fn origin(&self) -> FieldUpdateOrigin {
        self.origin
    }
}

/// Context supplied to a field blur listener callback.
pub struct FieldBlurListenerContext<Model, Error = String> {
    form: FormHandle<Model, Error>,
    field: FieldIdentity,
}

impl<Model, Error> FieldBlurListenerContext<Model, Error> {
    /// Returns the form handle that emitted the listener event.
    pub fn form(&self) -> FormHandle<Model, Error> {
        self.form.clone()
    }

    /// Returns the field whose blur event triggered this listener.
    pub fn field_identity(&self) -> FieldIdentity {
        self.field.clone()
    }
}

/// A hook-owned field binding lifecycle event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldBindingLifecycle {
    /// The binding was mounted for a Dioxus component instance.
    Mounted,
    /// The binding was unmounted for a Dioxus component instance.
    Unmounted,
}

/// Context supplied to a field binding lifecycle listener callback.
pub struct FieldBindingListenerContext<Model, Error = String> {
    form: FormHandle<Model, Error>,
    field: FieldIdentity,
    lifecycle: FieldBindingLifecycle,
}

impl<Model, Error> FieldBindingListenerContext<Model, Error> {
    /// Returns the form handle that emitted the listener event.
    pub fn form(&self) -> FormHandle<Model, Error> {
        self.form.clone()
    }

    /// Returns the field whose hook-owned binding changed lifecycle state.
    pub fn field_identity(&self) -> FieldIdentity {
        self.field.clone()
    }

    /// Returns whether the binding mounted or unmounted.
    pub const fn lifecycle(&self) -> FieldBindingLifecycle {
        self.lifecycle
    }
}

/// A form-level listener event.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormListenerEvent {
    /// A field value was replaced through the form API.
    FieldReplaced,
}

/// Context supplied to a form-level listener callback.
pub struct FormListenerContext<Model, Error = String> {
    form: FormHandle<Model, Error>,
    field: FieldIdentity,
    field_name: String,
    event: FormListenerEvent,
    origin: FieldUpdateOrigin,
}

impl<Model, Error> FormListenerContext<Model, Error> {
    /// Returns the form handle that emitted the listener event.
    pub fn form(&self) -> FormHandle<Model, Error> {
        self.form.clone()
    }

    /// Returns the field whose semantic event triggered this listener.
    pub fn field_identity(&self) -> FieldIdentity {
        self.field.clone()
    }

    /// Returns the rendered field name for the field whose event triggered this listener.
    pub fn field_name(&self) -> &str {
        &self.field_name
    }

    /// Returns the semantic event that triggered this listener.
    pub const fn event(&self) -> FormListenerEvent {
        self.event
    }

    /// Returns whether the field update came from user interaction or application code.
    pub const fn origin(&self) -> FieldUpdateOrigin {
        self.origin
    }
}

/// Context supplied to a form-level blur listener callback.
pub struct FormBlurListenerContext<Model, Error = String> {
    form: FormHandle<Model, Error>,
    field: FieldIdentity,
    field_name: String,
}

impl<Model, Error> FormBlurListenerContext<Model, Error> {
    /// Returns the form handle that emitted the listener event.
    pub fn form(&self) -> FormHandle<Model, Error> {
        self.form.clone()
    }

    /// Returns the field whose blur event triggered this listener.
    pub fn field_identity(&self) -> FieldIdentity {
        self.field.clone()
    }

    /// Returns the rendered field name for the field whose blur event triggered this listener.
    pub fn field_name(&self) -> &str {
        &self.field_name
    }
}

/// A submit lifecycle event delivered to submit listeners.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitListenerEvent {
    /// A submit attempt was accepted for validation and lifecycle processing.
    SubmitAttempted,
    /// Submit validation passed and application submit behavior started.
    SubmissionStarted,
    /// Submission did not start because of a known blocker.
    SubmitBlocked(SubmitBlocker),
    /// Application submit behavior returned structured submit errors.
    SubmissionRejected,
    /// Application submit behavior completed successfully.
    SubmissionSucceeded,
}

/// Context supplied to a submit listener callback.
pub struct SubmitListenerContext<Model, Error = String> {
    form: FormHandle<Model, Error>,
    event: SubmitListenerEvent,
    intent: Rc<dyn Any>,
}

impl<Model, Error> SubmitListenerContext<Model, Error> {
    /// Returns the form handle that emitted the listener event.
    pub fn form(&self) -> FormHandle<Model, Error> {
        self.form.clone()
    }

    /// Returns the submit lifecycle event.
    pub const fn event(&self) -> SubmitListenerEvent {
        self.event
    }

    /// Returns the typed submit intent that triggered this listener event when the requested type matches.
    pub fn submit_intent<Intent: 'static>(&self) -> Option<&Intent> {
        self.intent.as_ref().downcast_ref()
    }
}

/// A cheap Dioxus-facing handle to form state and behavior.
pub struct FormHandle<Model, Error = String> {
    core: Rc<RefCell<FormCore<Model, Error>>>,
    adapter: AdapterRuntime,
    runtime: Rc<RefCell<ValidationRuntime<Model, Error>>>,
    reactivity: Rc<FormReactivity>,
    listeners: Rc<RefCell<FormListeners<Model, Error>>>,
    active_submit_intent: Rc<RefCell<Option<Rc<dyn Any>>>>,
    submit_generation: Rc<Cell<u64>>,
    id_namespace: FormIdNamespace,
}

/// Submit-related form behavior scoped to one explicit submit intent.
pub struct IntentFormHandle<Model, Intent, Error = String> {
    handle: FormHandle<Model, Error>,
    intent: Intent,
}

/// Which listeners a field mutation dispatches after its selector and validation side effects.
enum FieldMutationDispatch {
    ValueReplacement(FieldUpdateOrigin),
    Blur,
}

/// The reactivity, validation, and listener side effects that follow a field write.
///
/// Every mutating field operation runs the same fixed sequence (notify selectors, kick off
/// gated async validation, notify validation subscribers, dispatch listeners), so the ordering
/// and completeness live in [`FormHandle::apply_field_mutation`] rather than being hand-copied
/// into each method. Callers declare only what varies.
struct FieldMutation {
    field: FieldIdentity,
    field_name: String,
    selectors: Vec<SelectorTransition>,
    trigger: ValidationTrigger,
    dispatch: FieldMutationDispatch,
}

impl<Model, Error> Clone for FormHandle<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            core: Rc::clone(&self.core),
            adapter: self.adapter.clone(),
            runtime: Rc::clone(&self.runtime),
            reactivity: Rc::clone(&self.reactivity),
            listeners: Rc::clone(&self.listeners),
            active_submit_intent: Rc::clone(&self.active_submit_intent),
            submit_generation: Rc::clone(&self.submit_generation),
            id_namespace: self.id_namespace.clone(),
        }
    }
}

impl<Model, Intent, Error> Clone for IntentFormHandle<Model, Intent, Error>
where
    Intent: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            intent: self.intent.clone(),
        }
    }
}

/// Field-scoped behavior for a typed field path.
pub struct FieldHandle<Model, Value, Error = String> {
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
}

impl<Model, Value, Error> Clone for FieldHandle<Model, Value, Error> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            path: self.path.clone(),
        }
    }
}

/// Headless access to one file selection attached to a form.
pub struct FileSelectionBinding<Model, Error = String> {
    handle: FormHandle<Model, Error>,
    key: FileFieldKey<Model>,
}

impl<Model, Error> Clone for FileSelectionBinding<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            key: self.key.clone(),
        }
    }
}

/// Headless access to one direct `Vec<Item>` collection field.
pub struct CollectionBinding<Model, Item, Error = String> {
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Vec<Item>>,
}

impl<Model, Item, Error> Clone for CollectionBinding<Model, Item, Error> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            path: self.path.clone(),
        }
    }
}

/// Headless access to one logical item inside a collection field.
pub struct CollectionItemBinding<Model, Item, Error = String> {
    handle: FormHandle<Model, Error>,
    collection_path: FieldPath<Model, Vec<Item>>,
    item: CollectionItem,
}

impl<Model, Item, Error> Clone for CollectionItemBinding<Model, Item, Error> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            collection_path: self.collection_path.clone(),
            item: self.item,
        }
    }
}

/// Headless true multi-select behavior for one direct `Vec<Value>` field.
pub struct MultiSelectBinding<Model, Value, Error = String> {
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Vec<Value>>,
}

impl<Model, Value, Error> Clone for MultiSelectBinding<Model, Value, Error> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            path: self.path.clone(),
        }
    }
}

/// One currently selected value in a true multi-select field.
pub struct MultiSelectItem<Model, Value, Error = String> {
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Vec<Value>>,
    item: CollectionItem,
}

impl<Model, Value, Error> Clone for MultiSelectItem<Model, Value, Error> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            path: self.path.clone(),
            item: self.item,
        }
    }
}

/// Headless checkbox-like behavior for one application-rendered multi-select option.
pub struct MultiSelectOptionBinding<Model, Value, Error = String> {
    multi_select: MultiSelectBinding<Model, Value, Error>,
    value: Value,
}

impl<Model, Value, Error> Clone for MultiSelectOptionBinding<Model, Value, Error>
where
    Value: Clone,
{
    fn clone(&self) -> Self {
        Self {
            multi_select: self.multi_select.clone(),
            value: self.value.clone(),
        }
    }
}

fn multi_select_item_value_ref<Value>(value: &Value) -> &Value {
    value
}

fn multi_select_item_value_mut<Value>(value: &mut Value) -> &mut Value {
    value
}

fn multi_select_item_value_path<Value: 'static>() -> FieldPath<Value, Value> {
    FieldPath::direct(
        FieldIdentity::new(""),
        "",
        multi_select_item_value_ref::<Value>,
        multi_select_item_value_mut::<Value>,
    )
}

/// Builder for registering a synchronous field validator.
pub struct SyncFieldValidatorBuilder<Model, Value, Error = String> {
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
}

/// Builder for registering a synchronous form validator.
pub struct SyncFormValidatorBuilder<Model, Error = String> {
    handle: FormHandle<Model, Error>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
}

/// Builder for registering a synchronous file-selection validator.
pub struct SyncFileSelectionValidatorBuilder<Model, Error = String> {
    handle: FormHandle<Model, Error>,
    key: FileFieldKey<Model>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
}

/// Builder for registering a synchronous validator template for collection item child fields.
pub struct SyncCollectionItemFieldValidatorBuilder<Model, Item, Value, Error = String> {
    handle: FormHandle<Model, Error>,
    collection: FieldPath<Model, Vec<Item>>,
    field: FieldPath<Item, Value>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
}

/// Builder for registering an asynchronous field validator.
pub struct AsyncFieldValidatorBuilder<Model, Value, Error = String> {
    handle: FormHandle<Model, Error>,
    path: FieldPath<Model, Value>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
    debounce: Option<Rc<DelayFactoryFn>>,
}

/// Builder for registering an asynchronous form validator.
pub struct AsyncFormValidatorBuilder<Model, Error = String> {
    handle: FormHandle<Model, Error>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
    debounce: Option<Rc<DelayFactoryFn>>,
}

/// Builder for registering an asynchronous file-selection validator.
pub struct AsyncFileSelectionValidatorBuilder<Model, Error = String> {
    handle: FormHandle<Model, Error>,
    key: FileFieldKey<Model>,
    source: ValidatorSource,
    triggers: ValidationTriggers,
    debounce: Option<Rc<DelayFactoryFn>>,
}

impl<Model, Value, Error> FieldHandle<Model, Value, Error> {
    /// Starts configuring a synchronous validator for this field.
    pub fn validator<Source>(self, source: Source) -> SyncFieldValidatorBuilder<Model, Value, Error>
    where
        Source: Into<ValidatorSource>,
    {
        SyncFieldValidatorBuilder {
            handle: self.handle,
            path: self.path,
            source: source.into(),
            triggers: ValidationTriggers::all(),
        }
    }

    /// Starts configuring an asynchronous validator for this field.
    pub fn async_validator<Source>(
        self,
        source: Source,
    ) -> AsyncFieldValidatorBuilder<Model, Value, Error>
    where
        Source: Into<ValidatorSource>,
    {
        AsyncFieldValidatorBuilder {
            handle: self.handle,
            path: self.path,
            source: source.into(),
            triggers: ValidationTriggers::all(),
            debounce: None,
        }
    }
}

impl<Model, Value, Error> SyncFieldValidatorBuilder<Model, Value, Error> {
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Registers this synchronous field validator.
    pub fn check<Validator>(self, validator: Validator) -> ValidatorId
    where
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Vec<Error> + 'static,
        Model: 'static,
        Value: 'static,
    {
        self.handle.register_sync_field_validator_for_triggers(
            self.path,
            self.source,
            self.triggers,
            validator,
        )
    }

    /// Registers this synchronous field validator when it returns zero or one error.
    pub fn check_optional<Validator>(self, validator: Validator) -> ValidatorId
    where
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Option<Error> + 'static,
        Model: 'static,
        Value: 'static,
    {
        self.handle
            .register_sync_field_validator_optional_for_triggers(
                self.path,
                self.source,
                self.triggers,
                validator,
            )
    }
}

impl<Model, Error> SyncFileSelectionValidatorBuilder<Model, Error> {
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Registers this synchronous file-selection validator.
    ///
    /// The validator receives a submit-style snapshot of selected-file metadata and returned errors
    /// are attached to this file selection's identity.
    pub fn check<Validator, Errors>(self, validator: Validator) -> ValidatorId
    where
        Validator: Fn(FileSubmissionSnapshot<Model>) -> Errors + 'static,
        Errors: IntoIterator<Item = Error>,
        Model: 'static,
    {
        self.check_with_context(move |files, _context| validator(files))
    }

    /// Registers this synchronous file-selection validator with validation context metadata.
    pub fn check_with_context<Validator, Errors>(self, validator: Validator) -> ValidatorId
    where
        Validator: for<'a> Fn(FileSubmissionSnapshot<Model>, ValidatorContext<'a, Model>) -> Errors
            + 'static,
        Errors: IntoIterator<Item = Error>,
        Model: 'static,
    {
        let adapter = self.handle.adapter.clone();
        let field = self.key.identity();

        let id = self.handle.write_core(|core| {
            core.register_sync_field_identity_validator_for_triggers(
                field,
                self.source,
                self.triggers,
                move |_model, context| {
                    let files = FileSubmissionSnapshot::new(adapter.file_selection_snapshot());

                    validator(files, context).into_iter().collect()
                },
            )
        });

        self.handle
            .notify_selectors(SelectorTransition::ValidationChanged);

        id
    }

    /// Registers this synchronous file-selection validator when it returns zero or one error.
    pub fn check_optional<Validator>(self, validator: Validator) -> ValidatorId
    where
        Validator: Fn(FileSubmissionSnapshot<Model>) -> Option<Error> + 'static,
        Model: 'static,
    {
        self.check(move |files| validator(files).into_iter().collect::<Vec<_>>())
    }

    /// Registers this synchronous file-selection validator with context when it returns zero or one error.
    pub fn check_optional_with_context<Validator>(self, validator: Validator) -> ValidatorId
    where
        Validator: for<'a> Fn(FileSubmissionSnapshot<Model>, ValidatorContext<'a, Model>) -> Option<Error>
            + 'static,
        Model: 'static,
    {
        self.check_with_context(move |files, context| {
            validator(files, context).into_iter().collect::<Vec<_>>()
        })
    }
}

impl<Model, Error> AsyncFileSelectionValidatorBuilder<Model, Error> {
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Debounces value-change validation with a fresh delay future for each run.
    pub fn debounce<DelayFactory, Delay>(mut self, delay: DelayFactory) -> Self
    where
        DelayFactory: Fn() -> Delay + 'static,
        Delay: Future<Output = ()> + 'static,
    {
        self.debounce = Some(Rc::new(move || Box::pin(delay())));
        self
    }

    /// Registers this asynchronous file-selection validator.
    ///
    /// The validator receives a submit-style snapshot of selected files and may return a non-`Send`
    /// future. Returned errors are attached to this file selection's identity. Late stale results
    /// and results after Dioxus cleanup are ignored by the runtime integration before they can
    /// mutate form state.
    pub fn check<Validator, Fut, Errors>(self, validator: Validator) -> ValidatorId
    where
        Model: Clone + 'static,
        Error: 'static,
        Validator: Fn(FileSubmissionSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
    {
        let adapter = self.handle.adapter.clone();
        let field = self.key.identity();

        self.handle.register_runtime_async_field_identity_validator(
            field,
            self.source,
            self.triggers,
            self.debounce,
            false,
            move |_context| {
                let files = FileSubmissionSnapshot::new(adapter.file_selection_snapshot());
                validator(files)
            },
        )
    }

    /// Registers this asynchronous file-selection validator with validation context metadata.
    pub fn check_with_context<Validator, Fut, Errors>(self, validator: Validator) -> ValidatorId
    where
        Model: Clone + 'static,
        Error: 'static,
        Validator: Fn(FileSubmissionSnapshot<Model>, AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
    {
        let adapter = self.handle.adapter.clone();
        let field = self.key.identity();

        self.handle.register_runtime_async_field_identity_validator(
            field,
            self.source,
            self.triggers,
            self.debounce,
            true,
            move |context| {
                let files = FileSubmissionSnapshot::new(adapter.file_selection_snapshot());
                validator(files, context)
            },
        )
    }

    /// Registers this asynchronous validator when it resolves to zero or one error.
    pub fn check_optional<Validator, Fut>(self, validator: Validator) -> ValidatorId
    where
        Model: Clone + 'static,
        Error: 'static,
        Validator: Fn(FileSubmissionSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Option<Error>> + 'static,
    {
        self.check(validator)
    }
}

impl<Model, Error> SyncFormValidatorBuilder<Model, Error> {
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Registers this synchronous form validator.
    pub fn check<Validator>(self, validator: Validator) -> ValidatorId
    where
        Validator: for<'a> Fn(FormValidatorContext<'a, Model>) -> Vec<FormValidationError<Error>>
            + 'static,
        Model: 'static,
    {
        self.handle
            .register_sync_form_validator_for_triggers(self.source, self.triggers, validator)
    }

    /// Registers this synchronous form validator when it returns zero or one error.
    pub fn check_optional<Validator>(self, validator: Validator) -> ValidatorId
    where
        Validator: for<'a> Fn(FormValidatorContext<'a, Model>) -> Option<FormValidationError<Error>>
            + 'static,
        Model: 'static,
    {
        self.handle
            .register_sync_form_validator_optional_for_triggers(
                self.source,
                self.triggers,
                validator,
            )
    }
}

impl<Model, Item, Value, Error> SyncCollectionItemFieldValidatorBuilder<Model, Item, Value, Error> {
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Registers this synchronous collection item child-field validator.
    pub fn check<Validator>(self, validator: Validator) -> ValidatorId
    where
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Vec<Error> + 'static,
        Model: 'static,
        Item: 'static,
        Value: 'static,
    {
        self.handle
            .register_sync_collection_item_field_validator_for_triggers(
                self.collection,
                self.field,
                self.source,
                self.triggers,
                validator,
            )
    }

    /// Registers this validator when it returns zero or one error.
    pub fn check_optional<Validator>(self, validator: Validator) -> ValidatorId
    where
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Option<Error> + 'static,
        Model: 'static,
        Item: 'static,
        Value: 'static,
    {
        self.handle
            .register_sync_collection_item_field_validator_for_triggers(
                self.collection,
                self.field,
                self.source,
                self.triggers,
                move |value, context| validator(value, context).into_iter().collect(),
            )
    }
}

impl<Model, Item, Error> CollectionBinding<Model, Item, Error> {
    /// Starts configuring a synchronous validator template for one child field on every item.
    pub fn item_field_validator<Value, Source>(
        &self,
        field: FieldPath<Item, Value>,
        source: Source,
    ) -> SyncCollectionItemFieldValidatorBuilder<Model, Item, Value, Error>
    where
        Source: Into<ValidatorSource>,
    {
        SyncCollectionItemFieldValidatorBuilder {
            handle: self.handle.clone(),
            collection: self.path.clone(),
            field,
            source: source.into(),
            triggers: ValidationTriggers::all(),
        }
    }

    /// Returns the rendered field name for this collection field.
    pub fn name(&self) -> &str {
        self.path.field_name()
    }

    /// Returns headless accessibility IDs and ARIA state for this collection field.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.handle.field_accessibility(self.path.clone())
    }

    /// Returns tracked user interaction metadata for this collection field.
    pub fn metadata(&self) -> FieldMetadata {
        self.handle.field_metadata(self.path.clone())
    }

    /// Returns whether this collection field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.metadata().is_touched()
    }

    /// Returns whether this collection field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.metadata().is_blurred()
    }

    /// Returns the current collection value.
    pub fn value(&self) -> Vec<Item>
    where
        Item: Clone,
    {
        self.handle.field_value(self.path.clone())
    }

    /// Returns whether this collection differs from its baseline.
    pub fn is_dirty(&self) -> bool
    where
        Item: PartialEq,
    {
        self.handle.is_field_dirty(self.path.clone())
    }

    /// Returns validation errors attached to this collection field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.handle.field_validation_errors(self.path.clone())
    }

    /// Returns visible validation errors attached to this collection field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.handle
            .visible_field_validation_errors(self.path.clone())
    }

    /// Returns the current logical items in rendered order.
    pub fn items(&self) -> Vec<CollectionItemBinding<Model, Item, Error>> {
        self.handle.collection_items(self.path.clone())
    }

    /// Appends an item programmatically and returns its logical identity.
    pub fn append_programmatic(&self, item: Item) -> CollectionItemIdentity {
        self.handle.push_collection_item(self.path.clone(), item)
    }

    /// Appends an item because of user interaction and returns its logical identity.
    pub fn append(&self, item: Item) -> CollectionItemIdentity {
        self.handle
            .push_user_collection_item(self.path.clone(), item)
    }

    /// Inserts an item programmatically.
    pub fn insert_programmatic(&self, index: usize, item: Item) -> Option<CollectionItemIdentity> {
        self.handle
            .insert_collection_item(self.path.clone(), index, item)
    }

    /// Inserts an item because of user interaction.
    pub fn insert(&self, index: usize, item: Item) -> Option<CollectionItemIdentity> {
        self.handle
            .insert_user_collection_item(self.path.clone(), index, item)
    }

    /// Removes an item by logical identity programmatically.
    pub fn remove_programmatic(&self, item: CollectionItemIdentity) -> Option<Item> {
        self.handle.remove_collection_item(self.path.clone(), item)
    }

    /// Removes an item by logical identity because of user interaction.
    pub fn remove(&self, item: CollectionItemIdentity) -> Option<Item> {
        self.handle
            .remove_user_collection_item(self.path.clone(), item)
    }

    /// Moves an item by logical identity programmatically.
    pub fn move_to_index_programmatic(&self, item: CollectionItemIdentity, index: usize) -> bool {
        self.handle
            .move_collection_item_to_index(self.path.clone(), item, index)
    }

    /// Moves an item by logical identity because of user interaction.
    pub fn move_to_index(&self, item: CollectionItemIdentity, index: usize) -> bool {
        self.handle
            .move_user_collection_item_to_index(self.path.clone(), item, index)
    }

    /// Swaps two items by position programmatically; each item keeps its logical identity.
    ///
    /// Item-scoped metadata, validation, parse state, and dirty tracking follow the swapped items.
    /// Returns `false` if either index is out of bounds or the two indices are equal (a no-op).
    pub fn swap_programmatic(&self, a: usize, b: usize) -> bool {
        self.handle.swap_collection_items(self.path.clone(), a, b)
    }

    /// Swaps two items by position because of user interaction; each item keeps its logical identity.
    pub fn swap(&self, a: usize, b: usize) -> bool {
        self.handle
            .swap_user_collection_items(self.path.clone(), a, b)
    }

    /// Replaces the item value at one position programmatically, keeping that item's logical identity.
    ///
    /// This is an in-place value replacement: the existing **Collection Item Identity** and its
    /// item-scoped metadata and validation attachment are retained. Returns `false` if the index is
    /// out of bounds.
    pub fn replace_programmatic(&self, index: usize, item: Item) -> bool {
        self.handle
            .replace_collection_item(self.path.clone(), index, item)
    }

    /// Replaces the item value at one position because of user interaction, keeping its identity.
    pub fn replace(&self, index: usize, item: Item) -> bool {
        self.handle
            .replace_user_collection_item(self.path.clone(), index, item)
    }

    /// Removes all items programmatically, releasing item-scoped state for each removed item.
    ///
    /// Form-level state and sibling collections are untouched. Returns whether any item was removed.
    pub fn clear_programmatic(&self) -> bool {
        self.handle.clear_collection_items(self.path.clone())
    }

    /// Removes all items because of user interaction, releasing item-scoped state for each item.
    pub fn clear(&self) -> bool {
        self.handle.clear_user_collection_items(self.path.clone())
    }
}

impl<Model, Value: 'static, Error> MultiSelectBinding<Model, Value, Error> {
    /// Starts configuring a synchronous validator template for every current and future selected value.
    pub fn item_validator<Source>(
        &self,
        source: Source,
    ) -> SyncCollectionItemFieldValidatorBuilder<Model, Value, Value, Error>
    where
        Source: Into<ValidatorSource>,
    {
        SyncCollectionItemFieldValidatorBuilder {
            handle: self.handle.clone(),
            collection: self.path.clone(),
            field: multi_select_item_value_path(),
            source: source.into(),
            triggers: ValidationTriggers::all(),
        }
    }

    /// Returns the rendered field name for this multi-select field.
    pub fn name(&self) -> &str {
        self.path.field_name()
    }

    /// Returns headless accessibility IDs and ARIA state for the multi-select field.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.handle.field_accessibility(self.path.clone())
    }

    /// Returns tracked user interaction metadata for this multi-select field.
    pub fn metadata(&self) -> FieldMetadata {
        self.handle.field_metadata(self.path.clone())
    }

    /// Returns whether this multi-select field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.metadata().is_touched()
    }

    /// Returns whether this multi-select field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.metadata().is_blurred()
    }

    /// Returns the current selected values.
    pub fn selected_values(&self) -> Vec<Value>
    where
        Value: Clone,
    {
        self.handle.field_value(self.path.clone())
    }

    /// Returns the current selected values.
    pub fn value(&self) -> Vec<Value>
    where
        Value: Clone,
    {
        self.selected_values()
    }

    /// Returns the selected values as logical collection items in rendered order.
    pub fn items(&self) -> Vec<MultiSelectItem<Model, Value, Error>> {
        self.handle
            .collection_items(self.path.clone())
            .into_iter()
            .map(|binding| MultiSelectItem {
                handle: self.handle.clone(),
                path: self.path.clone(),
                item: binding.item,
            })
            .collect()
    }

    /// Creates checkbox-like behavior for one application-rendered option.
    pub fn option(&self, value: Value) -> MultiSelectOptionBinding<Model, Value, Error> {
        MultiSelectOptionBinding {
            multi_select: self.clone(),
            value,
        }
    }

    /// Returns whether `value` is currently selected.
    pub fn is_selected(&self, value: &Value) -> bool
    where
        Value: PartialEq,
    {
        self.handle
            .reactivity
            .track_field_value(&self.path.identity());
        self.handle
            .core
            .borrow()
            .field_value(self.path.clone())
            .iter()
            .any(|selected| selected == value)
    }

    /// Returns the current selected item for `value`, if it is selected.
    pub fn selected_item(&self, value: &Value) -> Option<MultiSelectItem<Model, Value, Error>>
    where
        Value: PartialEq,
    {
        self.items().into_iter().find(|item| {
            self.handle
                .reactivity
                .track_field_value(&item.field_identity());
            self.handle.core.borrow().collection_item_field_value(
                self.path.clone(),
                item.identity(),
                multi_select_item_value_path(),
            ) == Some(value)
        })
    }

    /// Returns the logical identity for `value`, if it is selected.
    pub fn selected_identity(&self, value: &Value) -> Option<CollectionItemIdentity>
    where
        Value: PartialEq,
    {
        self.selected_item(value).map(|item| item.identity())
    }

    /// Returns whether the selected value collection differs from its collection baseline.
    pub fn is_dirty(&self) -> bool
    where
        Value: PartialEq,
    {
        self.handle
            .reactivity
            .track_field_value(&self.path.identity());
        self.handle
            .core
            .borrow()
            .is_collection_dirty(self.path.clone())
    }

    /// Selects `value` programmatically without marking the field touched.
    pub fn set_selected(&self, value: Value, selected: bool)
    where
        Value: PartialEq,
    {
        if selected {
            self.select_programmatic(value);
        } else {
            self.deselect_programmatic(&value);
        }
    }

    /// Selects `value` because of user interaction.
    pub fn select(&self, value: Value) -> CollectionItemIdentity
    where
        Value: PartialEq,
    {
        self.select_user(value)
    }

    /// Deselects `value` because of user interaction.
    pub fn deselect(&self, value: &Value) -> Option<Value>
    where
        Value: PartialEq,
    {
        self.deselect_user(value)
    }

    /// Applies a committed user selection state for one option value.
    pub fn on_change(&self, value: Value, selected: bool)
    where
        Value: PartialEq,
    {
        if selected {
            self.select_user(value);
        } else {
            self.deselect_user(&value);
        }
    }

    /// Toggles one option because of user interaction and returns the new selected state.
    pub fn toggle(&self, value: Value) -> bool
    where
        Value: PartialEq,
    {
        if self.is_selected(&value) {
            self.deselect_user(&value);
            false
        } else {
            self.select_user(value);
            true
        }
    }

    /// Marks the multi-select field and current selected values as blurred.
    pub fn on_blur(&self) {
        self.handle.mark_field_blurred(self.path.clone());

        for item in self.items() {
            item.on_blur();
        }
    }

    /// Returns validation errors attached to the multi-select field itself.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.handle.field_validation_errors(self.path.clone())
    }

    /// Returns visible validation errors attached to the multi-select field itself.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.handle
            .visible_field_validation_errors(self.path.clone())
    }

    fn select_programmatic(&self, value: Value) -> CollectionItemIdentity
    where
        Value: PartialEq,
    {
        self.selected_identity(&value)
            .unwrap_or_else(|| self.handle.push_collection_item(self.path.clone(), value))
    }

    fn select_user(&self, value: Value) -> CollectionItemIdentity
    where
        Value: PartialEq,
    {
        let identity = self.selected_identity(&value).unwrap_or_else(|| {
            self.handle
                .push_user_collection_item(self.path.clone(), value)
        });
        self.handle
            .mark_multi_select_item_touched(self.path.clone(), identity);
        identity
    }

    fn deselect_programmatic(&self, value: &Value) -> Option<Value>
    where
        Value: PartialEq,
    {
        let item = self.selected_item(value)?;
        self.handle
            .remove_collection_item(self.path.clone(), item.identity())
    }

    fn deselect_user(&self, value: &Value) -> Option<Value>
    where
        Value: PartialEq,
    {
        let item = self.selected_item(value)?;
        self.handle
            .remove_user_collection_item(self.path.clone(), item.identity())
    }
}

impl<Model, Value: 'static, Error> MultiSelectItem<Model, Value, Error> {
    /// Returns this selected value's logical collection item identity.
    pub const fn identity(&self) -> CollectionItemIdentity {
        self.item.identity()
    }

    /// Returns this selected value's current rendered index.
    pub const fn index(&self) -> usize {
        self.item.index()
    }

    /// Returns a stable key suitable for keyed selected-value rendering.
    pub fn key(&self) -> String {
        format!("multi-select-{}", self.identity().key())
    }

    /// Returns the selected value's item-level field identity.
    pub fn field_identity(&self) -> FieldIdentity {
        CollectionItemFieldAddress::identity_for(
            &self.path,
            self.identity(),
            &multi_select_item_value_path(),
        )
    }

    /// Returns the rendered item name derived from current selected-value order.
    pub fn name(&self) -> String {
        CollectionItemFieldAddress::field_name_for(
            &self.path,
            self.index(),
            &multi_select_item_value_path(),
        )
    }

    /// Returns headless accessibility IDs and ARIA state for this selected value.
    pub fn accessibility(&self) -> FieldAccessibility {
        let address = CollectionItemFieldAddress::new(
            &self.path,
            self.identity(),
            self.index(),
            &multi_select_item_value_path(),
        );

        self.handle
            .field_accessibility_by_identity(address.identity(), address.accessibility_name())
    }

    /// Returns the current selected value.
    pub fn value(&self) -> Value
    where
        Value: Clone,
    {
        self.handle
            .reactivity
            .track_field_value(&self.field_identity());
        self.handle
            .core
            .borrow()
            .collection_item_field_value(
                self.path.clone(),
                self.identity(),
                multi_select_item_value_path(),
            )
            .cloned()
            .expect("multi-select item should exist while its binding is rendered")
    }

    /// Returns tracked user interaction metadata for this selected value.
    pub fn metadata(&self) -> FieldMetadata {
        self.handle
            .reactivity
            .track_field_metadata(&self.field_identity());
        self.handle.core.borrow().collection_item_field_metadata(
            self.path.clone(),
            self.identity(),
            multi_select_item_value_path(),
        )
    }

    /// Returns whether this selected value has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.metadata().is_touched()
    }

    /// Returns whether this selected value has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.metadata().is_blurred()
    }

    /// Returns whether this selected value differs from its collection item baseline.
    pub fn is_dirty(&self) -> bool
    where
        Value: PartialEq,
    {
        self.handle
            .reactivity
            .track_field_value(&self.field_identity());
        self.handle.core.borrow().is_collection_item_field_dirty(
            self.path.clone(),
            self.identity(),
            multi_select_item_value_path(),
        )
    }

    /// Marks this selected value as blurred and touched.
    pub fn on_blur(&self) {
        self.handle
            .mark_multi_select_item_blurred(self.path.clone(), self.identity());
    }

    /// Returns validation errors attached to this selected value.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.handle
            .field_validation_errors_by_identity(&self.field_identity())
    }

    /// Returns visible validation errors attached to this selected value.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.handle
            .visible_field_validation_errors_by_identity(&self.field_identity())
    }
}

impl<Model, Value: 'static, Error> MultiSelectOptionBinding<Model, Value, Error> {
    /// Returns the rendered field name shared by options in this multi-select field.
    pub fn name(&self) -> &str {
        self.multi_select.name()
    }

    /// Returns the application-provided option value.
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Returns headless accessibility IDs and ARIA state for the multi-select field.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.multi_select.accessibility()
    }

    /// Returns whether this option is currently selected.
    pub fn checked(&self) -> bool
    where
        Value: PartialEq,
    {
        self.multi_select.is_selected(&self.value)
    }

    /// Returns whether this option is currently selected.
    pub fn is_selected(&self) -> bool
    where
        Value: PartialEq,
    {
        self.checked()
    }

    /// Returns this option's selected item, if it is currently selected.
    pub fn selected_item(&self) -> Option<MultiSelectItem<Model, Value, Error>>
    where
        Value: PartialEq,
    {
        self.multi_select.selected_item(&self.value)
    }

    /// Replaces this option's selected state programmatically.
    pub fn set_checked(&self, checked: bool)
    where
        Value: Clone + PartialEq,
    {
        self.multi_select.set_selected(self.value.clone(), checked);
    }

    /// Applies a Dioxus checkbox-like `onchange` checked-state update.
    pub fn on_change(&self, checked: bool)
    where
        Value: Clone + PartialEq,
    {
        self.multi_select.on_change(self.value.clone(), checked);
    }

    /// Selects this option because of user interaction.
    pub fn select(&self) -> CollectionItemIdentity
    where
        Value: Clone + PartialEq,
    {
        self.multi_select.select(self.value.clone())
    }

    /// Deselects this option because of user interaction.
    pub fn deselect(&self) -> Option<Value>
    where
        Value: PartialEq,
    {
        self.multi_select.deselect(&self.value)
    }

    /// Toggles this option because of user interaction and returns the new selected state.
    pub fn toggle(&self) -> bool
    where
        Value: Clone + PartialEq,
    {
        self.multi_select.toggle(self.value.clone())
    }

    /// Marks the multi-select field and current selected values as blurred.
    pub fn on_blur(&self) {
        self.multi_select.on_blur();
    }

    /// Returns a ready-made checkbox change handler that reads `checked` from the event.
    ///
    /// The handler owns its own clone, so `oninput: option.onchange()` needs no separate
    /// `option.clone()` and the option binding stays usable for `checked()`/`name()`.
    pub fn onchange(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Value: Clone + PartialEq + 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_change(event.checked())
    }

    /// Returns a ready-made `onblur` handler for this multi-select option.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Value: Clone + 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }
}

impl<Model, Item, Error> CollectionItemBinding<Model, Item, Error> {
    /// Returns this item's logical identity.
    pub const fn identity(&self) -> CollectionItemIdentity {
        self.item.identity()
    }

    /// Returns this item's current rendered index.
    pub const fn index(&self) -> usize {
        self.item.index()
    }

    /// Returns a stable key suitable for keyed row rendering.
    pub fn key(&self) -> String {
        format!("collection-{}", self.identity().key())
    }

    /// Creates a controlled text binding for a `String` child field.
    pub fn text(&self, path: FieldPath<Item, String>) -> CollectionTextBinding<Model, Item, Error> {
        CollectionTextBinding {
            base: CollectionFieldBindingCore::new(
                self.handle.clone(),
                self.collection_path.clone(),
                self.item,
                path,
            ),
        }
    }

    /// Creates a controlled checkbox binding for a `bool` child field.
    pub fn checkbox(
        &self,
        path: FieldPath<Item, bool>,
    ) -> CollectionCheckboxBinding<Model, Item, Error> {
        CollectionCheckboxBinding {
            base: CollectionFieldBindingCore::new(
                self.handle.clone(),
                self.collection_path.clone(),
                self.item,
                path,
            ),
        }
    }

    /// Creates a headless controlled select binding for a typed child field.
    pub fn select<Value>(
        &self,
        path: FieldPath<Item, Value>,
    ) -> CollectionSelectBinding<Model, Item, Value, Error> {
        CollectionSelectBinding {
            base: CollectionFieldBindingCore::new(
                self.handle.clone(),
                self.collection_path.clone(),
                self.item,
                path,
            ),
        }
    }

    /// Creates a headless controlled select binding with rendered string conversion.
    pub fn select_with<Value, Parser, ParserError, Formatter>(
        &self,
        path: FieldPath<Item, Value>,
        parser: Parser,
        formatter: Formatter,
    ) -> CollectionRenderedSelectBinding<Model, Item, Value, Error>
    where
        Value: 'static,
        Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
        ParserError: fmt::Display + 'static,
        Formatter: Fn(&Value) -> String + 'static,
    {
        let parser = Rc::new(move |value: &str| parser(value).map_err(|error| error.to_string()));

        CollectionRenderedSelectBinding {
            base: CollectionFieldBindingCore::new(
                self.handle.clone(),
                self.collection_path.clone(),
                self.item,
                path,
            ),
            parser,
            formatter: Rc::new(formatter),
        }
    }

    /// Creates a headless controlled radio group binding for a typed child field.
    pub fn radio_group<Value>(
        &self,
        path: FieldPath<Item, Value>,
    ) -> CollectionRadioGroupBinding<Model, Item, Value, Error> {
        CollectionRadioGroupBinding {
            base: CollectionFieldBindingCore::new(
                self.handle.clone(),
                self.collection_path.clone(),
                self.item,
                path,
            ),
        }
    }

    /// Creates a parsed text binding for a child field.
    pub fn parsed_text<Value>(
        &self,
        path: FieldPath<Item, Value>,
    ) -> CollectionParsedTextBinding<Model, Item, Value, Error>
    where
        Value: FromStr + fmt::Display + 'static,
        Value::Err: fmt::Display + 'static,
    {
        self.parsed_text_with(
            path,
            |value| value.parse::<Value>(),
            |value| value.to_string(),
        )
    }

    /// Creates a parsed text binding for a child field.
    pub fn parsed_text_with<Value, Parser, ParserError, Formatter>(
        &self,
        path: FieldPath<Item, Value>,
        parser: Parser,
        formatter: Formatter,
    ) -> CollectionParsedTextBinding<Model, Item, Value, Error>
    where
        Value: 'static,
        Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
        ParserError: fmt::Display + 'static,
        Formatter: Fn(&Value) -> String + 'static,
    {
        let field = CollectionItemFieldAddress::identity_for(
            &self.collection_path,
            self.item.identity(),
            &path,
        );
        let registration = self.handle.register_parse_binding(field);
        let parser = Rc::new(move |value: &str| parser(value).map_err(|error| error.to_string()));

        CollectionParsedTextBinding {
            base: CollectionFieldBindingCore::new(
                self.handle.clone(),
                self.collection_path.clone(),
                self.item,
                path,
            ),
            registration,
            parser,
            formatter: Rc::new(formatter),
        }
    }

    /// Creates a numeric input binding for a child field.
    pub fn number<Value>(
        &self,
        path: FieldPath<Item, Value>,
    ) -> CollectionParsedTextBinding<Model, Item, Value, Error>
    where
        Value: NumericInputValue,
        Value::Err: fmt::Display + 'static,
    {
        self.parsed_text(path)
    }

    /// Creates a numeric input binding with explicit parser and formatter behavior.
    pub fn number_with<Value, Parser, ParserError, Formatter>(
        &self,
        path: FieldPath<Item, Value>,
        parser: Parser,
        formatter: Formatter,
    ) -> CollectionParsedTextBinding<Model, Item, Value, Error>
    where
        Value: 'static,
        Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
        ParserError: fmt::Display + 'static,
        Formatter: Fn(&Value) -> String + 'static,
    {
        self.parsed_text_with(path, parser, formatter)
    }

    /// Creates a date-oriented input binding for a child field.
    pub fn date<Value>(
        &self,
        path: FieldPath<Item, Value>,
    ) -> CollectionParsedTextBinding<Model, Item, Value, Error>
    where
        Value: FromStr + fmt::Display + 'static,
        Value::Err: fmt::Display + 'static,
    {
        self.parsed_text(path)
    }

    /// Creates a date-oriented input binding with explicit parser and formatter behavior.
    pub fn date_with<Value, Parser, ParserError, Formatter>(
        &self,
        path: FieldPath<Item, Value>,
        parser: Parser,
        formatter: Formatter,
    ) -> CollectionParsedTextBinding<Model, Item, Value, Error>
    where
        Value: 'static,
        Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
        ParserError: fmt::Display + 'static,
        Formatter: Fn(&Value) -> String + 'static,
    {
        self.parsed_text_with(path, parser, formatter)
    }
}

impl<Model, Value, Error> AsyncFieldValidatorBuilder<Model, Value, Error> {
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Debounces value-change validation with a fresh delay future for each run.
    pub fn debounce<DelayFactory, Delay>(mut self, delay: DelayFactory) -> Self
    where
        DelayFactory: Fn() -> Delay + 'static,
        Delay: Future<Output = ()> + 'static,
    {
        self.debounce = Some(Rc::new(move || Box::pin(delay())));
        self
    }

    /// Registers this asynchronous field validator.
    ///
    /// The validator receives owned field and form snapshot values and may return a non-`Send`
    /// future. Late stale results and results after Dioxus cleanup are ignored by the runtime
    /// integration before they can mutate form state.
    pub fn check<Validator, Fut, Errors>(self, validator: Validator) -> ValidatorId
    where
        Model: Clone + 'static,
        Value: Clone + 'static,
        Error: 'static,
        Validator: Fn(Value, FormSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
    {
        self.check_with_context(move |value, context| {
            validator(value, context.into_form_snapshot())
        })
    }

    /// Registers this asynchronous field validator with access to validation context metadata.
    pub fn check_with_context<Validator, Fut, Errors>(self, validator: Validator) -> ValidatorId
    where
        Model: Clone + 'static,
        Value: Clone + 'static,
        Error: 'static,
        Validator: Fn(Value, AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
    {
        self.handle.register_runtime_async_field_validator(
            self.path,
            self.source,
            self.triggers,
            self.debounce,
            validator,
        )
    }
}

impl<Model, Error> AsyncFormValidatorBuilder<Model, Error> {
    /// Configures which semantic validation triggers should run this validator.
    pub fn on<Triggers>(mut self, triggers: Triggers) -> Self
    where
        Triggers: Into<ValidationTriggers>,
    {
        self.triggers = triggers.into();
        self
    }

    /// Debounces value-change validation with a fresh delay future for each run.
    pub fn debounce<DelayFactory, Delay>(mut self, delay: DelayFactory) -> Self
    where
        DelayFactory: Fn() -> Delay + 'static,
        Delay: Future<Output = ()> + 'static,
    {
        self.debounce = Some(Rc::new(move || Box::pin(delay())));
        self
    }

    /// Registers this asynchronous form validator.
    ///
    /// The validator receives an owned form snapshot and may return a non-`Send` future. Late stale
    /// results and results after Dioxus cleanup are ignored by the runtime integration before they
    /// can mutate form state.
    pub fn check<Validator, Fut, Errors>(self, validator: Validator) -> ValidatorId
    where
        Model: Clone + 'static,
        Error: 'static,
        Validator: Fn(FormSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = FormValidationError<Error>> + 'static,
    {
        self.check_with_context(move |context| validator(context.into_form_snapshot()))
    }

    /// Registers this asynchronous form validator with access to validation context metadata.
    pub fn check_with_context<Validator, Fut, Errors>(self, validator: Validator) -> ValidatorId
    where
        Model: Clone + 'static,
        Error: 'static,
        Validator: Fn(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = FormValidationError<Error>> + 'static,
    {
        self.handle.register_runtime_async_form_validator(
            self.source,
            self.triggers,
            self.debounce,
            validator,
        )
    }
}

impl<Model: Clone> FormHandle<Model> {
    /// Creates a handle for a form-owned draft initialized from `initial`.
    pub fn new(initial: Model) -> Self {
        Self::new_with_error_type(initial)
    }

    /// Creates a handle with an explicit validation error type.
    pub fn new_with_error_type<Error>(initial: Model) -> FormHandle<Model, Error> {
        FormHandle::from_core(FormCore::new_with_error_type::<Error>(initial))
    }

    /// Creates a handle with an explicit form ID namespace for deterministic derived IDs.
    pub fn new_with_id_namespace(initial: Model, id_namespace: impl Into<FormIdNamespace>) -> Self {
        Self::new(initial).with_id_namespace(id_namespace)
    }
}

impl<Model: Clone, Error> FormHandle<Model, Error> {
    /// Creates a handle from durable form configuration.
    pub fn from_config(config: FormConfig<Model, Error>) -> Self {
        let FormConfig {
            initial,
            id_namespace,
            validation_mode,
            error_visibility_policy,
            registrations,
            _marker: _,
        } = config;
        let core = FormCore::new_with_error_type::<Error>(initial)
            .with_validation_mode(validation_mode)
            .with_error_visibility_policy(error_visibility_policy);
        let handle = Self::from_core(core);
        let handle = match id_namespace {
            Some(id_namespace) => handle.with_id_namespace(id_namespace),
            None => handle,
        };

        for registration in registrations {
            registration(&handle);
        }

        handle
    }

    /// Restores the form to its baseline value and clears interaction and validation state.
    pub fn reset(&self) {
        self.adapter.cancel_validation_tasks();
        self.adapter.finish_managed_async_submission();
        self.clear_active_submit_intent();
        self.advance_submit_generation();
        self.write_core(FormCore::reset);
        self.adapter.clear_parse_errors();
        self.adapter.clear_file_selections();
        self.notify_changed();
    }

    /// Restores one field to its current baseline value and clears that field's field-scoped state.
    ///
    /// The single-field analog of [`Self::reset`]: it restores this field to the current
    /// **Baseline Value** (honoring any [`Self::reinitialize`], not the original config value),
    /// clears the field's touched, blurred, and dirty metadata and its field-scoped **Validation
    /// Errors** and pending validation, and clears any mounted **Parse Error** / **Raw Input State**
    /// for the field. Other fields, form-level validators, and the submit lifecycle are left
    /// untouched. In-flight async validation for the field is superseded by the field's new version
    /// and ignored on completion, so no explicit cancellation is needed.
    ///
    /// This slice targets direct **Field Paths**. Resetting a whole **Collection Field** or an
    /// individual collection-item child field is out of scope; use whole-form [`Self::reset`] for
    /// collections, since a single-field reset does not reconcile collection-item identities or
    /// item-scoped validator state.
    pub fn reset_field<Value>(&self, path: FieldPath<Model, Value>)
    where
        Value: Clone,
    {
        let field = path.identity();
        self.write_core(|core| core.reset_field(path));
        let cleared_parse = self.adapter.clear_field_parse_errors(&field);

        self.notify_selectors(SelectorTransition::FieldValueChanged(field.clone()));
        self.notify_selectors(SelectorTransition::FieldMetadataChanged(field.clone()));
        if cleared_parse {
            self.notify_selectors(SelectorTransition::ParseChanged(field));
        }
    }

    /// Explicitly replaces the form baseline and current draft, clearing interaction and validation state.
    pub fn reinitialize(&self, initial: Model) {
        self.adapter.cancel_validation_tasks();
        self.adapter.finish_managed_async_submission();
        self.clear_active_submit_intent();
        self.advance_submit_generation();
        self.write_core(|core| core.reinitialize(initial));
        self.adapter.clear_parse_errors();
        self.adapter.clear_file_selections();
        self.notify_changed();
    }

    /// Starts a submission by blocking duplicates, validating, and returning an owned snapshot.
    pub fn begin_submission(&self) -> SubmitAttempt<Model> {
        self.begin_intent_submission(())
    }

    fn begin_intent_submission<Intent>(&self, intent: Intent) -> SubmitAttempt<Model, Intent>
    where
        Intent: Clone + PartialEq + 'static,
    {
        self.begin_intent_submission_with_started_payload(intent, |_| ())
            .0
    }

    fn begin_intent_submission_with_started_payload<Intent, Payload, CapturePayload>(
        &self,
        intent: Intent,
        capture_payload: CapturePayload,
    ) -> (SubmitAttempt<Model, Intent>, Option<Payload>)
    where
        Intent: Clone + PartialEq + 'static,
        CapturePayload: FnOnce(&FormHandle<Model, Error>) -> Payload,
    {
        let listener_intent = intent.clone();
        let mut capture_payload = Some(capture_payload);

        if self.has_parse_blockers() {
            let blocker = self
                .write_core(|core| core.intent(intent).block_submission_with_parse_errors())
                .expect_blocker();
            self.notify_and_dispatch_submit_blocked(blocker, listener_intent);
            return (SubmitAttempt::Blocked(blocker), None);
        }

        {
            let core = self.core.borrow();
            self.adapter
                .flush_submit_relevant_debounced_validations(&core);
        }

        let result = self.write_core(|core| core.intent(intent).begin_submission());

        if matches!(
            result,
            SubmitAttempt::Blocked(SubmitBlocker::PendingValidation)
        ) && dioxus_core::Runtime::try_current().is_some()
        {
            self.start_runtime_async_validators(ValidationTrigger::Submit);
        }

        match &result {
            SubmitAttempt::Blocked(SubmitBlocker::InFlightSubmission) => {
                self.notify_selectors(SelectorTransition::SubmitChanged);
            }
            SubmitAttempt::Started(_) | SubmitAttempt::Blocked(_) => {
                self.notify_selectors(SelectorTransition::ValidationChanged);
            }
        }

        let payload = match &result {
            SubmitAttempt::Started(_) => {
                let capture_payload = capture_payload
                    .take()
                    .expect("submit-start payload was captured more than once");
                let payload = capture_payload(self);
                self.remember_active_submit_intent(listener_intent.clone());
                self.dispatch_submit_listeners(
                    SubmitListenerEvent::SubmitAttempted,
                    listener_intent.clone(),
                );
                self.dispatch_submit_listeners(
                    SubmitListenerEvent::SubmissionStarted,
                    listener_intent.clone(),
                );

                Some(payload)
            }
            SubmitAttempt::Blocked(SubmitBlocker::InFlightSubmission) => {
                self.dispatch_submit_listeners(
                    SubmitListenerEvent::SubmitAttempted,
                    listener_intent.clone(),
                );
                self.dispatch_submit_listeners(
                    SubmitListenerEvent::SubmitBlocked(SubmitBlocker::InFlightSubmission),
                    listener_intent.clone(),
                );

                None
            }
            SubmitAttempt::Blocked(blocker) => {
                self.dispatch_submit_listeners(
                    SubmitListenerEvent::SubmitAttempted,
                    listener_intent.clone(),
                );
                self.dispatch_submit_listeners(
                    SubmitListenerEvent::SubmitBlocked(*blocker),
                    listener_intent.clone(),
                );

                None
            }
        };

        (result, payload)
    }

    /// Runs a synchronous submit handler when submit validation passes.
    pub fn submit<Submit, Outcome>(&self, submit: Submit) -> SubmitResult
    where
        Submit: FnOnce(SubmissionSnapshot<Model>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        self.submit_intent((), submit)
    }

    /// Runs a synchronous submit handler with a submit-time file-selection snapshot.
    pub fn submit_with_files<Submit, Outcome>(&self, submit: Submit) -> SubmitResult
    where
        Submit: FnOnce(SubmissionSnapshot<Model>, FileSubmissionSnapshot<Model>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        self.submit_intent_with_files((), submit)
    }

    fn submit_intent_with_files<Intent, Submit, Outcome>(
        &self,
        intent: Intent,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>, FileSubmissionSnapshot<Model>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        let (attempt, file_snapshot) = self
            .begin_intent_submission_with_started_payload(intent, |handle| {
                handle.file_submission_snapshot()
            });

        match attempt {
            SubmitAttempt::Started(submitted) => {
                let submitted_for_result = submitted.clone();
                let file_snapshot =
                    file_snapshot.expect("started file-aware submit should capture file snapshot");
                let submit_errors = submit(submitted, file_snapshot).into();

                if submit_errors.is_empty() {
                    self.finish_submission_success_for_intent(
                        submitted_for_result.intent().clone(),
                    );
                    SubmitResult::Succeeded
                } else {
                    self.finish_submission_with_errors(submitted_for_result, submit_errors);
                    SubmitResult::Rejected
                }
            }
            SubmitAttempt::Blocked(blocker) => SubmitResult::Blocked(blocker),
        }
    }

    fn submit_intent<Intent, Submit, Outcome>(&self, intent: Intent, submit: Submit) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        match self.begin_intent_submission(intent) {
            SubmitAttempt::Started(submitted) => {
                let submitted_for_result = submitted.clone();
                let submit_errors = submit(submitted).into();

                if submit_errors.is_empty() {
                    self.finish_submission_success_for_intent(
                        submitted_for_result.intent().clone(),
                    );
                    SubmitResult::Succeeded
                } else {
                    self.finish_submission_with_errors(submitted_for_result, submit_errors);
                    SubmitResult::Rejected
                }
            }
            SubmitAttempt::Blocked(blocker) => SubmitResult::Blocked(blocker),
        }
    }

    /// Starts an asynchronous submit handler on the Dioxus task runtime when submit validation passes.
    ///
    /// Returns [`SubmitResult::Started`] when the async handler was started. Structured submit
    /// errors are stored when the spawned future completes.
    ///
    /// This is the fire-and-return async path: submit-relevant async validation that is already
    /// pending blocks this attempt instead of being awaited. Prefer [`Self::submit_async_managed`]
    /// for Dioxus UI submit handlers that should flush and wait for submit-relevant async
    /// validation before running application submit behavior.
    pub fn submit_async<Submit, Fut, Outcome>(&self, submit: Submit) -> SubmitResult
    where
        Submit: FnOnce(SubmissionSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        self.submit_async_unmanaged(submit)
    }

    /// Starts an asynchronous submit handler without waiting for pending async validation.
    ///
    /// This is an explicit alias for [`Self::submit_async`]. Prefer
    /// [`Self::submit_async_managed`] for ordinary Dioxus UI submit handlers.
    pub fn submit_async_unmanaged<Submit, Fut, Outcome>(&self, submit: Submit) -> SubmitResult
    where
        Submit: FnOnce(SubmissionSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        self.submit_async_unmanaged_intent((), submit)
    }

    fn submit_async_unmanaged_intent<Intent, Submit, Fut, Outcome>(
        &self,
        intent: Intent,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        match self.begin_intent_submission(intent) {
            SubmitAttempt::Started(submitted) => {
                let handle = self.clone();
                let submit_generation = self.submit_generation();
                let submitted_for_result = submitted.clone();

                self.spawn_detached(async move {
                    let submit_errors: SubmitErrors<Model, Error> = submit(submitted).await.into();

                    if !handle.submit_generation_matches(submit_generation) {
                        return;
                    }

                    if submit_errors.is_empty() {
                        handle.finish_submission_success_for_intent(
                            submitted_for_result.intent().clone(),
                        );
                    } else {
                        handle.finish_submission_with_errors(submitted_for_result, submit_errors);
                    }
                });

                SubmitResult::Started
            }
            SubmitAttempt::Blocked(blocker) => SubmitResult::Blocked(blocker),
        }
    }

    /// Starts an asynchronous managed submit.
    ///
    /// Submit-relevant debounced validation is flushed immediately, required async validators are
    /// started for the submit snapshot, and the application submit future starts only after pending
    /// submit validation settles successfully.
    pub fn submit_async_managed<Submit, Fut, Outcome>(&self, submit: Submit) -> SubmitResult
    where
        Submit: FnOnce(SubmissionSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        self.submit_async_managed_intent((), submit)
    }

    /// Starts an asynchronous managed submit with a submit-time file-selection snapshot.
    ///
    /// The selected-file metadata is captured when submission starts and is handed
    /// to the application submit future alongside the validated form snapshot.
    pub fn submit_async_managed_with_files<Submit, Fut, Outcome>(
        &self,
        submit: Submit,
    ) -> SubmitResult
    where
        Submit: FnOnce(SubmissionSnapshot<Model>, FileSubmissionSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        self.submit_async_managed_intent_with_files((), submit)
    }

    fn submit_async_managed_intent<Intent, Submit, Fut, Outcome>(
        &self,
        intent: Intent,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        ManagedSubmission::new(self.clone()).submit_async(intent, submit)
    }

    fn submit_async_managed_intent_with_files<Intent, Submit, Fut, Outcome>(
        &self,
        intent: Intent,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>, FileSubmissionSnapshot<Model>) -> Fut
            + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        ManagedSubmission::new(self.clone()).submit_async_with_files(intent, submit)
    }

    /// Creates a Dioxus-managed submit binding.
    ///
    /// The binding's Dioxus `onsubmit` methods prevent native browser submission and stop event
    /// propagation before running the form submission lifecycle.
    pub fn managed_submit(&self) -> SubmitBinding<Model, Error> {
        SubmitBinding {
            handle: self.clone(),
        }
    }

    /// Creates a progressive browser submit binding.
    ///
    /// The binding's Dioxus `onsubmit` method allows browser submission to continue unless the
    /// hydrated form currently has known client-side blockers.
    pub fn progressive_submit(&self) -> ProgressiveSubmitBinding<Model, Error> {
        ProgressiveSubmitBinding {
            handle: self.clone(),
        }
    }
}

impl<Model, Intent, Error> IntentFormHandle<Model, Intent, Error> {
    /// Returns the underlying form handle.
    pub const fn handle(&self) -> &FormHandle<Model, Error> {
        &self.handle
    }

    /// Returns the submit intent this scope uses.
    pub const fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Returns current UI-oriented submit availability for this submit intent.
    pub fn availability(&self) -> SubmitAvailability
    where
        Intent: PartialEq + 'static,
    {
        self.handle.intent_availability(&self.intent)
    }

    /// Returns whether this submit intent has no current known blockers.
    pub fn can_submit(&self) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.availability().is_available()
    }

    /// Returns the latest outcome when this submit intent produced the latest status.
    pub fn last_status(&self) -> Option<SubmitStatus>
    where
        Intent: PartialEq + 'static,
    {
        self.handle.intent_last_status(&self.intent)
    }

    /// Returns whether this submit intent produced the latest status and it was a success.
    ///
    /// A pure derived read over [`Self::last_status`]: `false` when the latest recorded outcome
    /// belongs to a different intent, has not succeeded, or does not exist yet.
    pub fn is_submit_successful(&self) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.last_status().is_some_and(SubmitStatus::is_succeeded)
    }

    /// Returns accessibility state for one field, filtering validation errors by this submit intent.
    pub fn field_accessibility<Value>(&self, path: FieldPath<Model, Value>) -> FieldAccessibility
    where
        Intent: PartialEq + 'static,
    {
        self.handle.intent_field_accessibility(path, &self.intent)
    }
}

impl<Model: Clone, Intent, Error> IntentFormHandle<Model, Intent, Error> {
    /// Starts a submission with this submit intent.
    pub fn begin_submission(&self) -> SubmitAttempt<Model, Intent>
    where
        Intent: Clone + PartialEq + 'static,
    {
        self.handle.begin_intent_submission(self.intent.clone())
    }

    /// Runs a synchronous submit handler with this submit intent.
    pub fn submit<Submit, Outcome>(&self, submit: Submit) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        self.handle.submit_intent(self.intent.clone(), submit)
    }

    /// Runs a synchronous file-aware submit handler with this submit intent.
    pub fn submit_with_files<Submit, Outcome>(&self, submit: Submit) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>, FileSubmissionSnapshot<Model>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        self.handle
            .submit_intent_with_files(self.intent.clone(), submit)
    }

    /// Starts an asynchronous submit handler without waiting for pending async validation.
    pub fn submit_async<Submit, Fut, Outcome>(&self, submit: Submit) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        self.handle
            .submit_async_unmanaged_intent(self.intent.clone(), submit)
    }

    /// Starts a managed asynchronous submit for this submit intent.
    pub fn submit_async_managed<Submit, Fut, Outcome>(&self, submit: Submit) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        self.handle
            .submit_async_managed_intent(self.intent.clone(), submit)
    }

    /// Starts a file-aware managed asynchronous submit for this submit intent.
    pub fn submit_async_managed_with_files<Submit, Fut, Outcome>(
        &self,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>, FileSubmissionSnapshot<Model>) -> Fut
            + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        self.handle
            .submit_async_managed_intent_with_files(self.intent.clone(), submit)
    }

    /// Records a submit attempt and runs submit-triggered validators for this intent.
    pub fn validate_for_submit(&self) -> bool
    where
        Intent: Clone + PartialEq + 'static,
    {
        self.handle.validate_intent_for_submit(self.intent.clone())
    }
}

impl<Model, Error> FormHandle<Model, Error> {
    /// Wraps renderer-agnostic form state in a Dioxus-facing handle.
    pub fn from_core(core: FormCore<Model, Error>) -> Self {
        Self {
            core: Rc::new(RefCell::new(core)),
            adapter: AdapterRuntime::default(),
            runtime: Rc::new(RefCell::new(ValidationRuntime::default())),
            reactivity: Rc::new(FormReactivity::default()),
            listeners: Rc::new(RefCell::new(FormListeners::default())),
            active_submit_intent: Rc::new(RefCell::new(None)),
            submit_generation: Rc::new(Cell::new(0)),
            id_namespace: FormIdNamespace::default(),
        }
    }

    /// Wraps renderer-agnostic form state in a Dioxus-facing handle with an explicit ID namespace.
    pub fn from_core_with_id_namespace(
        core: FormCore<Model, Error>,
        id_namespace: impl Into<FormIdNamespace>,
    ) -> Self {
        Self::from_core(core).with_id_namespace(id_namespace)
    }

    /// Returns a copy of this handle that derives field IDs from `id_namespace`.
    pub fn with_id_namespace(mut self, id_namespace: impl Into<FormIdNamespace>) -> Self {
        self.id_namespace = id_namespace.into();
        self
    }

    /// Returns this handle with a mode for automatic validation execution.
    pub fn with_validation_mode(self, mode: ValidationMode) -> Self {
        self.write_core(|core| core.set_validation_mode(mode));
        self
    }

    /// Returns this handle with a policy for visible validation errors.
    pub fn with_error_visibility_policy(self, policy: ErrorVisibilityPolicy) -> Self {
        self.write_core(|core| core.set_error_visibility_policy(policy));
        self
    }

    /// Returns the namespace used for derived accessibility IDs.
    pub fn id_namespace(&self) -> &FormIdNamespace {
        &self.id_namespace
    }

    /// Creates a browser-owned POST submit helper for a form action.
    pub fn browser_submit(&self, action: impl Into<String>) -> BrowserSubmitBinding {
        BrowserSubmitBinding {
            action: action.into(),
        }
    }

    fn register_field_listener<Value, Listener>(
        &self,
        path: FieldPath<Model, Value>,
        origin: Option<FieldUpdateOrigin>,
        listener: Listener,
    ) -> FieldListenerRegistration<Model, Error>
    where
        Listener: FnMut(FieldListenerContext<Model, Error>) + 'static,
    {
        let id =
            self.listeners
                .borrow_mut()
                .register_field_listener(path.identity(), origin, listener);

        FieldListenerRegistration::new(Rc::clone(&self.listeners), id)
    }

    fn register_field_blur_listener<Value, Listener>(
        &self,
        path: FieldPath<Model, Value>,
        listener: Listener,
    ) -> FieldBlurListenerRegistration<Model, Error>
    where
        Listener: FnMut(FieldBlurListenerContext<Model, Error>) + 'static,
    {
        let id = self
            .listeners
            .borrow_mut()
            .register_field_blur_listener(path.identity(), listener);

        FieldBlurListenerRegistration::new(Rc::clone(&self.listeners), id)
    }

    fn register_debounced_field_listener<Value, Listener>(
        &self,
        path: FieldPath<Model, Value>,
        origin: Option<FieldUpdateOrigin>,
        delay: Rc<DelayFactoryFn>,
        listener: Listener,
    ) -> DebouncedFieldListenerRegistration<Model, Error>
    where
        Model: 'static,
        Error: 'static,
        Listener: FnMut(FieldListenerContext<Model, Error>) + 'static,
    {
        let id = self
            .listeners
            .borrow_mut()
            .register_debounced_field_listener(path.identity(), origin, delay, listener);

        DebouncedFieldListenerRegistration::new(Rc::clone(&self.listeners), id)
    }

    fn register_field_binding_listener<Value, Listener>(
        &self,
        path: FieldPath<Model, Value>,
        listener: Listener,
    ) -> FieldBindingListenerRegistration<Model, Error>
    where
        Listener: FnMut(FieldBindingListenerContext<Model, Error>) + 'static,
    {
        let field = path.identity();
        let (id, callback, mounted_count) = self
            .listeners
            .borrow_mut()
            .register_field_binding_listener(field.clone(), listener);

        for _ in 0..mounted_count {
            self.dispatch_field_binding_callback(
                Rc::clone(&callback),
                field.clone(),
                FieldBindingLifecycle::Mounted,
            );
        }

        FieldBindingListenerRegistration::new(self.clone(), id)
    }

    fn register_form_listener<Listener>(
        &self,
        origin: Option<FieldUpdateOrigin>,
        listener: Listener,
    ) -> FormListenerRegistration<Model, Error>
    where
        Listener: FnMut(FormListenerContext<Model, Error>) + 'static,
    {
        let id = self
            .listeners
            .borrow_mut()
            .register_form_listener(origin, listener);

        FormListenerRegistration::new(Rc::clone(&self.listeners), id)
    }

    fn register_form_blur_listener<Listener>(
        &self,
        listener: Listener,
    ) -> FormBlurListenerRegistration<Model, Error>
    where
        Listener: FnMut(FormBlurListenerContext<Model, Error>) + 'static,
    {
        let id = self
            .listeners
            .borrow_mut()
            .register_form_blur_listener(listener);

        FormBlurListenerRegistration::new(Rc::clone(&self.listeners), id)
    }

    fn register_debounced_form_listener<Listener>(
        &self,
        origin: Option<FieldUpdateOrigin>,
        delay: Rc<DelayFactoryFn>,
        listener: Listener,
    ) -> DebouncedFormListenerRegistration<Model, Error>
    where
        Model: 'static,
        Error: 'static,
        Listener: FnMut(FormListenerContext<Model, Error>) + 'static,
    {
        let id = self
            .listeners
            .borrow_mut()
            .register_debounced_form_listener(origin, delay, listener);

        DebouncedFormListenerRegistration::new(Rc::clone(&self.listeners), id)
    }

    fn register_submit_listener<Listener>(
        &self,
        listener: Listener,
    ) -> SubmitListenerRegistration<Model, Error>
    where
        Listener: FnMut(SubmitListenerContext<Model, Error>) + 'static,
    {
        let id = self
            .listeners
            .borrow_mut()
            .register_submit_listener(listener);

        SubmitListenerRegistration::new(Rc::clone(&self.listeners), id)
    }

    fn dispatch_form_listeners(
        &self,
        field: FieldIdentity,
        field_name: String,
        origin: FieldUpdateOrigin,
        event: FormListenerEvent,
    ) {
        let callbacks = self.listeners.borrow().form_callbacks(origin);

        for callback in callbacks {
            let context = FormListenerContext {
                form: self.clone(),
                field: field.clone(),
                field_name: field_name.clone(),
                event,
                origin,
            };
            let Ok(mut callback) = callback.try_borrow_mut() else {
                panic!(
                    "form listener re-entered while it was already running; use \
                     use_form_listener_for_origin to avoid listener-caused programmatic reentry"
                );
            };

            (callback.as_mut())(context);
        }
    }

    fn dispatch_debounced_form_listeners(
        &self,
        field: FieldIdentity,
        field_name: String,
        origin: FieldUpdateOrigin,
        event: FormListenerEvent,
    ) {
        let callbacks = self.listeners.borrow().debounced_form_callbacks(origin);

        for callback in callbacks {
            (callback.schedule)(
                self.clone(),
                field.clone(),
                field_name.clone(),
                origin,
                event,
            );
        }
    }

    fn dispatch_value_replacement_listeners(
        &self,
        field: FieldIdentity,
        field_name: String,
        origin: FieldUpdateOrigin,
    ) {
        self.dispatch_form_listeners(
            field.clone(),
            field_name.clone(),
            origin,
            FormListenerEvent::FieldReplaced,
        );
        self.dispatch_debounced_form_listeners(
            field.clone(),
            field_name,
            origin,
            FormListenerEvent::FieldReplaced,
        );
        self.dispatch_field_listeners(field.clone(), origin);
        self.dispatch_debounced_field_listeners(field, origin);
    }

    fn dispatch_field_listeners(&self, field: FieldIdentity, origin: FieldUpdateOrigin) {
        let callbacks = self.listeners.borrow().field_callbacks(&field, origin);

        for callback in callbacks {
            let context = FieldListenerContext {
                form: self.clone(),
                field: field.clone(),
                origin,
            };
            let Ok(mut callback) = callback.try_borrow_mut() else {
                panic!(
                    "field listener re-entered while it was already running; use \
                     use_field_listener_for_origin to avoid listener-caused programmatic reentry"
                );
            };

            (callback.as_mut())(context);
        }
    }

    fn dispatch_debounced_field_listeners(&self, field: FieldIdentity, origin: FieldUpdateOrigin) {
        let callbacks = self
            .listeners
            .borrow()
            .debounced_field_callbacks(&field, origin);

        for callback in callbacks {
            (callback.schedule)(self.clone(), field.clone(), origin);
        }
    }

    fn dispatch_field_blur_listeners(&self, field: FieldIdentity) {
        let callbacks = self.listeners.borrow().field_blur_callbacks(&field);

        for callback in callbacks {
            let context = FieldBlurListenerContext {
                form: self.clone(),
                field: field.clone(),
            };
            let Ok(mut callback) = callback.try_borrow_mut() else {
                panic!(
                    "field blur listener re-entered while it was already running; avoid \
                     listener-caused blur cycles"
                );
            };

            (callback.as_mut())(context);
        }
    }

    fn dispatch_form_blur_listeners(&self, field: FieldIdentity, field_name: String) {
        let callbacks = self.listeners.borrow().form_blur_callbacks();

        for callback in callbacks {
            let context = FormBlurListenerContext {
                form: self.clone(),
                field: field.clone(),
                field_name: field_name.clone(),
            };
            let Ok(mut callback) = callback.try_borrow_mut() else {
                panic!(
                    "form blur listener re-entered while it was already running; avoid \
                     listener-caused blur cycles"
                );
            };

            (callback.as_mut())(context);
        }
    }

    fn dispatch_field_binding_listeners(
        &self,
        field: FieldIdentity,
        lifecycle: FieldBindingLifecycle,
    ) {
        let callbacks = {
            let mut listeners = self.listeners.borrow_mut();
            listeners.record_field_binding_lifecycle(&field, lifecycle);
            listeners.field_binding_callbacks(&field)
        };

        for callback in callbacks {
            self.dispatch_field_binding_callback(callback, field.clone(), lifecycle);
        }
    }

    fn dispatch_field_binding_callback(
        &self,
        callback: Rc<RefCell<FieldBindingListenerCallback<Model, Error>>>,
        field: FieldIdentity,
        lifecycle: FieldBindingLifecycle,
    ) {
        let context = FieldBindingListenerContext {
            form: self.clone(),
            field,
            lifecycle,
        };
        let Ok(mut callback) = callback.try_borrow_mut() else {
            panic!(
                "field binding listener re-entered while it was already running; avoid \
                 listener-caused binding lifecycle cycles"
            );
        };

        (callback.as_mut())(context);
    }

    fn remember_active_submit_intent<Intent>(&self, intent: Intent)
    where
        Intent: 'static,
    {
        self.active_submit_intent
            .borrow_mut()
            .replace(Rc::new(intent));
    }

    fn take_active_submit_intent(&self) -> Option<Rc<dyn Any>> {
        self.active_submit_intent.borrow_mut().take()
    }

    fn clear_active_submit_intent(&self) {
        self.active_submit_intent.borrow_mut().take();
    }

    pub(crate) fn submit_generation(&self) -> u64 {
        self.submit_generation.get()
    }

    fn advance_submit_generation(&self) {
        let next = self
            .submit_generation
            .get()
            .checked_add(1)
            .expect("submit generation counter exhausted");

        self.submit_generation.set(next);
    }

    pub(crate) fn submit_generation_matches(&self, generation: u64) -> bool {
        self.submit_generation.get() == generation
    }

    fn dispatch_submit_listeners<Intent>(&self, event: SubmitListenerEvent, intent: Intent)
    where
        Intent: 'static,
    {
        self.dispatch_submit_listeners_with_intent(event, Rc::new(intent));
    }

    /// Runs the notify-and-dispatch protocol for a submission blocked before it could start.
    ///
    /// Every submit surface (synchronous, Dioxus-managed, and progressive) shares this sequence
    /// when a parse-error preflight or a duplicate-submission check blocks the attempt: notify the
    /// selectors for the blocker, then dispatch `SubmitAttempted` followed by `SubmitBlocked`.
    /// Concentrating it here keeps the selector transition and listener ordering from drifting
    /// across the three surfaces. Only `ParseErrors` and `InFlightSubmission` reach this path; other
    /// blockers are resolved after validation, not by a pre-submission guard.
    fn notify_and_dispatch_submit_blocked<Intent>(&self, blocker: SubmitBlocker, intent: Intent)
    where
        Intent: Clone + 'static,
    {
        match blocker {
            SubmitBlocker::ParseErrors => {
                self.notify_selectors(SelectorTransition::SubmitAttempted);
            }
            SubmitBlocker::InFlightSubmission => self.notify_submit_changed(),
            other => {
                unreachable!("a pre-submission guard produced an unexpected blocker: {other:?}")
            }
        }
        self.dispatch_submit_listeners(SubmitListenerEvent::SubmitAttempted, intent.clone());
        self.dispatch_submit_listeners(SubmitListenerEvent::SubmitBlocked(blocker), intent);
    }

    fn dispatch_submit_listeners_with_intent(
        &self,
        event: SubmitListenerEvent,
        intent: Rc<dyn Any>,
    ) {
        let callbacks = self.listeners.borrow().submit_callbacks();

        for callback in callbacks {
            let context = SubmitListenerContext {
                form: self.clone(),
                event,
                intent: Rc::clone(&intent),
            };
            let Ok(mut callback) = callback.try_borrow_mut() else {
                panic!(
                    "submit listener re-entered while it was already running; avoid \
                     listener-caused submit lifecycle cycles"
                );
            };

            (callback.as_mut())(context);
        }
    }

    /// Returns a field-scoped handle for configuring behavior around a typed field path.
    pub fn field<Value>(&self, path: FieldPath<Model, Value>) -> FieldHandle<Model, Value, Error> {
        FieldHandle {
            handle: self.clone(),
            path,
        }
    }

    /// Scopes submit-related operations to one explicit submit intent.
    pub fn intent<Intent>(&self, intent: Intent) -> IntentFormHandle<Model, Intent, Error> {
        IntentFormHandle {
            handle: self.clone(),
            intent,
        }
    }

    /// Starts configuring a synchronous validator for the whole form.
    pub fn validator<Source>(&self, source: Source) -> SyncFormValidatorBuilder<Model, Error>
    where
        Source: Into<ValidatorSource>,
    {
        SyncFormValidatorBuilder {
            handle: self.clone(),
            source: source.into(),
            triggers: ValidationTriggers::all(),
        }
    }

    /// Starts configuring an asynchronous validator for the whole form.
    pub fn async_validator<Source>(&self, source: Source) -> AsyncFormValidatorBuilder<Model, Error>
    where
        Source: Into<ValidatorSource>,
    {
        AsyncFormValidatorBuilder {
            handle: self.clone(),
            source: source.into(),
            triggers: ValidationTriggers::all(),
            debounce: None,
        }
    }

    /// Returns the mode that controls automatic validation execution.
    pub fn validation_mode(&self) -> ValidationMode {
        self.track_read();
        self.core.borrow().validation_mode()
    }

    /// Replaces the mode that controls automatic validation execution.
    pub fn set_validation_mode(&self, mode: ValidationMode) {
        self.write_core(|core| core.set_validation_mode(mode));
        self.notify_changed();
    }

    /// Returns the policy that controls visible validation errors.
    pub fn error_visibility_policy(&self) -> ErrorVisibilityPolicy {
        self.track_read();
        self.core.borrow().error_visibility_policy()
    }

    /// Replaces the policy that controls visible validation errors.
    pub fn set_error_visibility_policy(&self, policy: ErrorVisibilityPolicy) {
        self.write_core(|core| core.set_error_visibility_policy(policy));
        self.notify_changed();
    }

    /// Captures opt-in form state for explicit serialization or transfer.
    pub fn state_snapshot(&self) -> FormStateSnapshot<Model, Error>
    where
        Model: Clone,
        Error: Clone,
    {
        self.track_read();
        self.core.borrow().state_snapshot()
    }

    /// Restores an opt-in form-state snapshot, then clears adapter parse state, fences old async
    /// submit completions, and invalidates all Dioxus selectors if the core snapshot is compatible.
    pub fn restore_state_snapshot(
        &self,
        snapshot: FormStateSnapshot<Model, Error>,
    ) -> Result<(), FormStateRestoreError> {
        let result = self.write_core(|core| core.restore_state_snapshot(snapshot));

        if result.is_ok() {
            self.adapter.cancel_validation_tasks();
            self.adapter.finish_managed_async_submission();
            self.clear_active_submit_intent();
            self.advance_submit_generation();
            self.adapter.clear_parse_errors();
            self.adapter.clear_file_selections();
            self.notify_changed();
        }

        result
    }

    /// Returns serializable runtime identity state for all tracked collection fields.
    pub fn collection_identity_state(&self) -> CollectionIdentityState {
        self.track_read();
        self.core.borrow().collection_identity_state()
    }

    /// Restores runtime collection identity state and invalidates all Dioxus selectors.
    pub fn restore_collection_identity_state(
        &self,
        state: CollectionIdentityState,
    ) -> Result<(), FormStateRestoreError> {
        let result = self.write_core(|core| core.restore_collection_identity_state(state));

        if result.is_ok() {
            self.notify_changed();
        }

        result
    }

    /// Reads the underlying form core through a short scoped borrow.
    pub fn read_core<Result>(
        &self,
        read: impl FnOnce(&FormCore<Model, Error>) -> Result,
    ) -> Result {
        self.track_read();
        let core = self.core.borrow();
        read(&core)
    }

    /// Advanced escape hatch for mutating the underlying form core through a short scoped borrow.
    ///
    /// This intentionally uses coarse selector invalidation because arbitrary core mutations do not
    /// expose a more specific semantic transition to the Dioxus adapter.
    pub fn write_advanced<Result>(
        &self,
        write: impl FnOnce(&mut FormCore<Model, Error>) -> Result,
    ) -> Result {
        let result = self.write_core(write);
        self.notify_changed();
        result
    }

    fn write_core<Result>(
        &self,
        write: impl FnOnce(&mut FormCore<Model, Error>) -> Result,
    ) -> Result {
        let mut core = self.core.borrow_mut();
        write(&mut core)
    }

    fn track_read(&self) {
        self.reactivity.track_read();
    }

    fn notify_changed(&self) {
        self.notify_selectors(SelectorTransition::UnknownMutation);
    }

    fn notify_validation_changed(&self) {
        self.notify_selectors(SelectorTransition::ValidationChanged);
    }

    fn notify_submit_changed(&self) {
        self.notify_selectors(SelectorTransition::SubmitChanged);
    }

    fn file_submission_snapshot(&self) -> FileSubmissionSnapshot<Model> {
        FileSubmissionSnapshot::new(self.adapter.file_selection_snapshot())
    }

    fn notify_selectors(&self, transition: SelectorTransition) {
        let wake_validation_waiters = transition.wakes_validation_waiters();
        self.reactivity.notify_selector_transition(transition);
        if wake_validation_waiters {
            self.adapter.wake_validation_waiters();
        }
    }

    fn cleanup(&self) {
        self.adapter.deactivate();
        self.clear_active_submit_intent();
    }

    fn is_active(&self) -> bool {
        self.adapter.is_active()
    }

    fn spawn_validation_task(
        &self,
        target: ValidationTarget,
        id: ValidatorId,
        future: impl Future<Output = ()> + 'static,
    ) {
        self.adapter.spawn_validation_task(target, id, future);
    }

    /// Spawns a fire-and-forget submission task through the spawner seam (ADR-0015).
    fn spawn_detached(&self, future: impl Future<Output = ()> + 'static) {
        self.adapter.spawn_detached(future);
    }

    fn register_runtime_async_field_validator<Value, Validator, Fut, Errors>(
        &self,
        path: FieldPath<Model, Value>,
        source: ValidatorSource,
        triggers: ValidationTriggers,
        debounce: Option<Rc<DelayFactoryFn>>,
        validator: Validator,
    ) -> ValidatorId
    where
        Model: Clone + 'static,
        Value: Clone + 'static,
        Error: 'static,
        Validator: Fn(Value, AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
    {
        let field = path.identity();
        let id = self.write_core(|core| {
            core.register_async_field_validator_for_triggers(path.clone(), source, triggers)
        });
        let validator: Rc<FieldAsyncValidatorFn<Model, Value, Error>> =
            Rc::new(move |value, context| {
                let future = validator(value, context);
                Box::pin(async move { future.await.into_iter().collect() })
            });

        self.runtime.borrow_mut().field_validators.insert(
            (field, id),
            RuntimeAsyncFieldValidator {
                start: Rc::new(move |handle, trigger, sync_already_ran| {
                    let validator = Rc::clone(&validator);

                    match debounce.as_ref() {
                        Some(delay) if trigger == ValidationTrigger::Change => {
                            if sync_already_ran {
                                handle.validate_async_field_validator_with_debounce_after_sync(
                                    path.clone(),
                                    id,
                                    trigger,
                                    delay(),
                                    move |value, snapshot| validator(value, snapshot),
                                )
                            } else {
                                handle.validate_async_field_validator_with_debounce(
                                    path.clone(),
                                    id,
                                    trigger,
                                    delay(),
                                    move |value, snapshot| validator(value, snapshot),
                                )
                            }
                        }
                        Some(_) | None => {
                            if sync_already_ran {
                                handle.validate_async_field_validator_after_sync(
                                    path.clone(),
                                    id,
                                    trigger,
                                    move |value, snapshot| validator(value, snapshot),
                                )
                            } else {
                                handle.validate_async_field_validator(
                                    path.clone(),
                                    id,
                                    trigger,
                                    move |value, snapshot| validator(value, snapshot),
                                )
                            }
                        }
                    }
                }),
            },
        );
        self.notify_validation_changed();

        id
    }

    fn register_runtime_async_field_identity_validator<Validator, Fut, Errors>(
        &self,
        field: FieldIdentity,
        source: ValidatorSource,
        triggers: ValidationTriggers,
        debounce: Option<Rc<DelayFactoryFn>>,
        model_dependent: bool,
        validator: Validator,
    ) -> ValidatorId
    where
        Model: Clone + 'static,
        Error: 'static,
        Validator: Fn(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
    {
        let id = self.write_core(|core| {
            core.register_async_field_identity_validator_for_triggers_with_model_dependency(
                field.clone(),
                source,
                triggers,
                model_dependent,
            )
        });
        let validator: Rc<FieldIdentityAsyncValidatorFn<Model, Error>> = Rc::new(move |context| {
            let future = validator(context);
            Box::pin(async move { future.await.into_iter().collect() })
        });

        self.runtime.borrow_mut().field_validators.insert(
            (field.clone(), id),
            RuntimeAsyncFieldValidator {
                start: Rc::new(move |handle, trigger, sync_already_ran| {
                    let validator = Rc::clone(&validator);

                    match debounce.as_ref() {
                        Some(delay) if trigger == ValidationTrigger::Change => {
                            if sync_already_ran {
                                handle.validate_async_field_identity_validator_with_debounce_after_sync(
                                    field.clone(),
                                    id,
                                    trigger,
                                    delay(),
                                    move |context| validator(context),
                                )
                            } else {
                                handle.validate_async_field_identity_validator_with_debounce(
                                    field.clone(),
                                    id,
                                    trigger,
                                    delay(),
                                    move |context| validator(context),
                                )
                            }
                        }
                        Some(_) | None => {
                            if sync_already_ran {
                                handle.validate_async_field_identity_validator_after_sync(
                                    field.clone(),
                                    id,
                                    trigger,
                                    move |context| validator(context),
                                )
                            } else {
                                handle.validate_async_field_identity_validator(
                                    field.clone(),
                                    id,
                                    trigger,
                                    move |context| validator(context),
                                )
                            }
                        }
                    }
                }),
            },
        );
        self.notify_validation_changed();

        id
    }

    fn register_runtime_async_form_validator<Validator, Fut, Errors>(
        &self,
        source: ValidatorSource,
        triggers: ValidationTriggers,
        debounce: Option<Rc<DelayFactoryFn>>,
        validator: Validator,
    ) -> ValidatorId
    where
        Model: Clone + 'static,
        Error: 'static,
        Validator: Fn(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = FormValidationError<Error>> + 'static,
    {
        let id = self
            .write_core(|core| core.register_async_form_validator_for_triggers(source, triggers));
        let validator: Rc<FormAsyncValidatorFn<Model, Error>> = Rc::new(move |context| {
            let future = validator(context);
            Box::pin(async move { future.await.into_iter().collect() })
        });

        self.runtime.borrow_mut().form_validators.insert(
            id,
            RuntimeAsyncFormValidator {
                start: Rc::new(move |handle, trigger, sync_already_ran| {
                    let validator = Rc::clone(&validator);

                    match debounce.as_ref() {
                        Some(delay) if trigger == ValidationTrigger::Change => {
                            if sync_already_ran {
                                handle.validate_async_form_validator_with_debounce_after_sync(
                                    id,
                                    trigger,
                                    delay(),
                                    move |snapshot| validator(snapshot),
                                )
                            } else {
                                handle.validate_async_form_validator_with_debounce(
                                    id,
                                    trigger,
                                    delay(),
                                    move |snapshot| validator(snapshot),
                                )
                            }
                        }
                        Some(_) | None => {
                            if sync_already_ran {
                                handle.validate_async_form_validator_after_sync(
                                    id,
                                    trigger,
                                    move |snapshot| validator(snapshot),
                                )
                            } else {
                                handle.validate_async_form_validator(id, trigger, move |snapshot| {
                                    validator(snapshot)
                                })
                            }
                        }
                    }
                }),
            },
        );
        self.notify_validation_changed();

        id
    }

    fn start_runtime_async_validators(&self, trigger: ValidationTrigger) -> bool {
        let starts: Vec<_> = self
            .runtime
            .borrow()
            .field_validators
            .iter()
            .map(|((field, id), validator)| {
                (
                    ValidationTarget::Field(field.clone()),
                    *id,
                    Rc::clone(&validator.start),
                )
            })
            .collect();
        let form_starts: Vec<_> = self
            .runtime
            .borrow()
            .form_validators
            .iter()
            .map(|(id, validator)| (ValidationTarget::Form, *id, Rc::clone(&validator.start)))
            .collect();
        let mut started = false;

        for (target, id, start) in starts {
            if self
                .should_skip_runtime_async_start_for_pending_debounced_submit(target, id, trigger)
            {
                continue;
            }

            if start(self.clone(), trigger, true).is_some() {
                started = true;
            }
        }

        for (target, id, start) in form_starts {
            if self
                .should_skip_runtime_async_start_for_pending_debounced_submit(target, id, trigger)
            {
                continue;
            }

            if start(self.clone(), trigger, true).is_some() {
                started = true;
            }
        }

        started
    }

    fn start_runtime_async_field_validators(
        &self,
        field: FieldIdentity,
        trigger: ValidationTrigger,
    ) -> bool {
        let starts: Vec<_> = self
            .runtime
            .borrow()
            .field_validators
            .iter()
            .filter(|((validator_field, _), _)| *validator_field == field)
            .map(|((_, id), validator)| (*id, Rc::clone(&validator.start)))
            .collect();
        let mut started = false;

        for (id, start) in starts {
            if self.should_skip_runtime_async_start_for_pending_debounced_submit(
                ValidationTarget::Field(field.clone()),
                id,
                trigger,
            ) {
                continue;
            }

            if start(self.clone(), trigger, true).is_some() {
                started = true;
            }
        }

        started
    }

    fn start_runtime_async_form_validators(&self, trigger: ValidationTrigger) -> bool {
        let starts: Vec<_> = self
            .runtime
            .borrow()
            .form_validators
            .iter()
            .map(|(id, validator)| (*id, Rc::clone(&validator.start)))
            .collect();
        let mut started = false;

        for (id, start) in starts {
            if self.should_skip_runtime_async_start_for_pending_debounced_submit(
                ValidationTarget::Form,
                id,
                trigger,
            ) {
                continue;
            }

            if start(self.clone(), trigger, true).is_some() {
                started = true;
            }
        }

        started
    }

    fn should_skip_runtime_async_start_for_pending_debounced_submit(
        &self,
        target: ValidationTarget,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> bool {
        trigger == ValidationTrigger::Submit
            && self
                .adapter
                .has_active_debounced_validation(target.clone(), id)
            && self
                .core
                .borrow()
                .should_flush_debounced_validation_for_submit(&target, id)
    }

    fn start_runtime_async_field_validator(
        &self,
        field: FieldIdentity,
        id: ValidatorId,
        trigger: ValidationTrigger,
        sync_already_ran: bool,
    ) -> Option<ValidationStatus> {
        let start = self
            .runtime
            .borrow()
            .field_validators
            .get(&(field, id))
            .map(|validator| Rc::clone(&validator.start))?;

        start(self.clone(), trigger, sync_already_ran)
    }

    fn start_runtime_async_form_validator(
        &self,
        id: ValidatorId,
        trigger: ValidationTrigger,
        sync_already_ran: bool,
    ) -> Option<ValidationStatus> {
        let start = self
            .runtime
            .borrow()
            .form_validators
            .get(&id)
            .map(|validator| Rc::clone(&validator.start))?;

        start(self.clone(), trigger, sync_already_ran)
    }

    fn runtime_field_validator_id_for_source<Value>(
        &self,
        path: FieldPath<Model, Value>,
        source: &ValidatorSource,
    ) -> Option<ValidatorId> {
        self.core
            .borrow()
            .field_validation_statuses(path)
            .into_iter()
            .find(|status| status.source() == source)
            .map(|status| status.validator_id())
    }

    fn runtime_form_validator_id_for_source(
        &self,
        source: &ValidatorSource,
    ) -> Option<ValidatorId> {
        self.core
            .borrow()
            .form_validation_statuses()
            .into_iter()
            .find(|status| status.source() == source)
            .map(|status| status.validator_id())
    }

    /// Reads and clones the current form draft through a selector subscription.
    pub fn snapshot(&self) -> Model
    where
        Model: Clone,
    {
        self.reactivity.track_snapshot();
        self.core.borrow().snapshot()
    }

    /// Reads and clones one typed field value through a field-scoped selector subscription.
    pub fn field_value<Value: Clone>(&self, path: FieldPath<Model, Value>) -> Value {
        self.reactivity.track_field_value(&path.identity());
        self.core.borrow().field_value(path).clone()
    }

    /// Returns whether a submission has started and not completed yet.
    pub fn is_submitting(&self) -> bool {
        self.reactivity.track_submit();
        self.core.borrow().is_submitting() || self.adapter.has_managed_async_submission()
    }

    /// Returns current UI-oriented submit availability through a selector subscription.
    ///
    /// This is a conservative UI signal. Stored validation errors from non-submit triggers can make
    /// the form unavailable here even when a later submit-triggered validation pass would be allowed
    /// to submit.
    pub fn submit_availability(&self) -> SubmitAvailability {
        self.reactivity.track_submit();

        let core_availability = self.core.borrow().submit_availability();
        let has_parse_blockers = self.has_parse_blockers();
        let mut blockers = Vec::new();

        if core_availability.contains(SubmitBlocker::ValidationErrors) {
            blockers.push(SubmitBlocker::ValidationErrors);
        }

        if has_parse_blockers {
            blockers.push(SubmitBlocker::ParseErrors);
        }

        if core_availability.contains(SubmitBlocker::PendingValidation) {
            blockers.push(SubmitBlocker::PendingValidation);
        }

        if core_availability.contains(SubmitBlocker::InFlightSubmission)
            || self.adapter.has_managed_async_submission()
        {
            blockers.push(SubmitBlocker::InFlightSubmission);
        }

        SubmitAvailability::blocked_by(blockers)
    }

    fn intent_availability<Intent>(&self, intent: &Intent) -> SubmitAvailability
    where
        Intent: PartialEq + 'static,
    {
        self.reactivity.track_submit();

        let core_availability = self.core.borrow().intent_availability(intent);
        let has_parse_blockers = self.has_parse_blockers();
        let mut blockers = Vec::new();

        if core_availability.contains(SubmitBlocker::ValidationErrors) {
            blockers.push(SubmitBlocker::ValidationErrors);
        }

        if has_parse_blockers {
            blockers.push(SubmitBlocker::ParseErrors);
        }

        if core_availability.contains(SubmitBlocker::PendingValidation) {
            blockers.push(SubmitBlocker::PendingValidation);
        }

        if core_availability.contains(SubmitBlocker::InFlightSubmission)
            || self.adapter.has_managed_async_submission()
        {
            blockers.push(SubmitBlocker::InFlightSubmission);
        }

        SubmitAvailability::blocked_by(blockers)
    }

    /// Returns whether there are no current known submit blockers.
    pub fn can_submit(&self) -> bool {
        self.submit_availability().is_available()
    }

    /// Returns how many submit attempts have been recorded.
    pub fn submit_attempt_count(&self) -> u64 {
        self.reactivity.track_submit();
        self.core.borrow().submit_attempt_count()
    }

    /// Returns the number of managed submit attempts recorded, including attempts blocked by known
    /// blockers.
    ///
    /// The submit-side companion to the derived-state readers; a convenience name over the same
    /// count as [`Self::submit_attempt_count`]. In-flight duplicate attempts are not counted as new
    /// attempts. Cleared on [`Reset`](Self::reset).
    pub fn submission_attempts(&self) -> u64 {
        self.reactivity.track_submit();
        self.core.borrow().submit_attempt_count()
    }

    /// Returns whether the latest recorded submit outcome was a successful submission.
    ///
    /// A pure derived read over [`Last Submit Status`](Self::last_submit_status); `false` before any
    /// submit completes and after a rejected or blocked attempt. Cleared on [`Reset`](Self::reset).
    pub fn is_submit_successful(&self) -> bool {
        self.reactivity.track_submit();
        self.core.borrow().is_submit_successful()
    }

    /// Returns the latest meaningful submission outcome, if one has been recorded.
    pub fn last_submit_status(&self) -> Option<SubmitStatus> {
        self.reactivity.track_submit();
        self.core.borrow().last_submit_status()
    }

    /// Returns the latest meaningful submission outcome with its typed submit intent.
    pub fn last_submit_status_as<Intent>(&self) -> Option<LastSubmitStatus<Intent>>
    where
        Intent: Clone + 'static,
    {
        self.reactivity.track_submit();
        self.core.borrow().last_submit_status_as()
    }

    fn intent_last_status<Intent>(&self, intent: &Intent) -> Option<SubmitStatus>
    where
        Intent: PartialEq + 'static,
    {
        self.reactivity.track_submit();
        self.core.borrow().intent_last_status(intent)
    }

    /// Completes an in-flight submission without changing values or the baseline.
    pub fn finish_submission(&self) -> bool {
        if !self.is_active() {
            return false;
        }

        let finished = self.write_core(FormCore::finish_submission);

        if finished {
            self.clear_active_submit_intent();
            self.notify_selectors(SelectorTransition::SubmitChanged);
        }

        finished
    }

    /// Completes a successful submission without resetting or changing the baseline.
    pub fn finish_submission_success(&self) -> bool {
        self.finish_submission_success_for_active_intent()
    }

    fn finish_submission_success_for_active_intent(&self) -> bool {
        if !self.is_active() {
            return false;
        }

        let finished = self.write_core(FormCore::finish_submission_success);

        if finished {
            let intent = self
                .take_active_submit_intent()
                .unwrap_or_else(|| Rc::new(()) as Rc<dyn Any>);
            self.notify_selectors(SelectorTransition::ValidationChanged);
            self.dispatch_submit_listeners_with_intent(
                SubmitListenerEvent::SubmissionSucceeded,
                intent,
            );
        }

        finished
    }

    fn finish_submission_success_for_intent<Intent>(&self, intent: Intent) -> bool
    where
        Intent: 'static,
    {
        if !self.is_active() {
            return false;
        }

        let finished = self.write_core(FormCore::finish_submission_success);

        if finished {
            self.clear_active_submit_intent();
            self.notify_selectors(SelectorTransition::ValidationChanged);
            self.dispatch_submit_listeners(SubmitListenerEvent::SubmissionSucceeded, intent);
        }

        finished
    }

    /// Completes an in-flight submission with structured submit errors.
    pub fn finish_submission_with_errors<Intent, Outcome>(
        &self,
        submitted: SubmissionSnapshot<Model, Intent>,
        errors: Outcome,
    ) -> bool
    where
        Intent: Clone + 'static,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        if !self.is_active() {
            return false;
        }

        let intent = submitted.intent().clone();
        let finished =
            self.write_core(|core| core.finish_submission_with_errors(submitted, errors));

        if finished {
            self.clear_active_submit_intent();
            self.notify_selectors(SelectorTransition::ValidationChanged);
            self.dispatch_submit_listeners(SubmitListenerEvent::SubmissionRejected, intent);
        }

        finished
    }

    /// Returns whether any form value differs from the baseline value.
    pub fn is_dirty(&self) -> bool
    where
        Model: PartialEq,
    {
        self.reactivity.track_snapshot();
        self.core.borrow().is_dirty()
    }

    /// Returns whether one typed field value differs from its baseline value.
    pub fn is_field_dirty<Value>(&self, path: FieldPath<Model, Value>) -> bool
    where
        Value: PartialEq,
    {
        self.reactivity.track_field_value(&path.identity());
        self.core.borrow().is_field_dirty(path)
    }

    /// Returns whether every form value equals the baseline value.
    ///
    /// The inverse of [`Self::is_dirty`], provided as a convenience for pristine-form UI reads.
    pub fn is_pristine(&self) -> bool
    where
        Model: PartialEq,
    {
        self.reactivity.track_snapshot();
        self.core.borrow().is_pristine()
    }

    /// Returns whether one typed field currently equals its baseline value.
    ///
    /// Dirty Fields are non-sticky, so this is the inverse of [`Self::is_field_dirty`].
    pub fn is_default_value<Value>(&self, path: FieldPath<Model, Value>) -> bool
    where
        Value: PartialEq,
    {
        self.reactivity.track_field_value(&path.identity());
        self.core.borrow().is_default_value(path)
    }

    /// Creates headless access to one direct `Vec<Item>` collection field.
    pub fn collection<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
    ) -> CollectionBinding<Model, Item, Error> {
        CollectionBinding {
            handle: self.clone(),
            path,
        }
    }

    /// Creates headless true multi-select access to one direct `Vec<Value>` field.
    pub fn multi_select<Value>(
        &self,
        path: FieldPath<Model, Vec<Value>>,
    ) -> MultiSelectBinding<Model, Value, Error> {
        MultiSelectBinding {
            handle: self.clone(),
            path,
        }
    }

    fn collection_items<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
    ) -> Vec<CollectionItemBinding<Model, Item, Error>> {
        self.reactivity.track_field_value(&path.identity());
        self.write_core(|core| core.collection_items(path.clone()))
            .into_iter()
            .map(|item| CollectionItemBinding {
                handle: self.clone(),
                collection_path: path.clone(),
                item,
            })
            .collect()
    }

    fn push_collection_item<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        item: Item,
    ) -> CollectionItemIdentity {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let identity = self.write_core(|core| core.push_collection_item(path, item));
        self.notify_collection_changed(collection.clone());
        self.dispatch_value_replacement_listeners(
            collection,
            field_name,
            FieldUpdateOrigin::Programmatic,
        );
        identity
    }

    fn push_user_collection_item<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        item: Item,
    ) -> CollectionItemIdentity {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let identity = self.write_core(|core| core.push_user_collection_item(path, item));
        self.notify_collection_user_changed(collection.clone());
        self.dispatch_value_replacement_listeners(collection, field_name, FieldUpdateOrigin::User);
        identity
    }

    fn insert_collection_item<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        index: usize,
        item: Item,
    ) -> Option<CollectionItemIdentity> {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let identity = self.write_core(|core| core.insert_collection_item(path, index, item));
        if identity.is_some() {
            self.notify_collection_changed(collection.clone());
            self.dispatch_value_replacement_listeners(
                collection,
                field_name,
                FieldUpdateOrigin::Programmatic,
            );
        }
        identity
    }

    fn insert_user_collection_item<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        index: usize,
        item: Item,
    ) -> Option<CollectionItemIdentity> {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let identity = self.write_core(|core| core.insert_user_collection_item(path, index, item));
        if identity.is_some() {
            self.notify_collection_user_changed(collection.clone());
            self.dispatch_value_replacement_listeners(
                collection,
                field_name,
                FieldUpdateOrigin::User,
            );
        }
        identity
    }

    fn remove_collection_item<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
    ) -> Option<Item> {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let removed = self.write_core(|core| core.remove_collection_item(path, item));
        if removed.is_some() {
            self.unregister_collection_item_parse_bindings(collection.clone(), item);
            self.notify_collection_changed(collection.clone());
            self.dispatch_value_replacement_listeners(
                collection,
                field_name,
                FieldUpdateOrigin::Programmatic,
            );
        }
        removed
    }

    fn remove_user_collection_item<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
    ) -> Option<Item> {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let removed = self.write_core(|core| core.remove_user_collection_item(path, item));
        if removed.is_some() {
            self.unregister_collection_item_parse_bindings(collection.clone(), item);
            self.notify_collection_user_changed(collection.clone());
            self.dispatch_value_replacement_listeners(
                collection,
                field_name,
                FieldUpdateOrigin::User,
            );
        }
        removed
    }

    fn move_collection_item_to_index<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        index: usize,
    ) -> bool {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let moved = self.write_core(|core| core.move_collection_item_to_index(path, item, index));
        if moved {
            self.notify_collection_changed(collection.clone());
            self.dispatch_value_replacement_listeners(
                collection,
                field_name,
                FieldUpdateOrigin::Programmatic,
            );
        }
        moved
    }

    fn move_user_collection_item_to_index<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        index: usize,
    ) -> bool {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let moved =
            self.write_core(|core| core.move_user_collection_item_to_index(path, item, index));
        if moved {
            self.notify_collection_user_changed(collection.clone());
            self.dispatch_value_replacement_listeners(
                collection,
                field_name,
                FieldUpdateOrigin::User,
            );
        }
        moved
    }

    fn swap_collection_items<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        a: usize,
        b: usize,
    ) -> bool {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let swapped = self.write_core(|core| core.swap_collection_items(path, a, b));
        if swapped {
            self.notify_collection_changed(collection.clone());
            self.dispatch_value_replacement_listeners(
                collection,
                field_name,
                FieldUpdateOrigin::Programmatic,
            );
        }
        swapped
    }

    fn swap_user_collection_items<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        a: usize,
        b: usize,
    ) -> bool {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let swapped = self.write_core(|core| core.swap_user_collection_items(path, a, b));
        if swapped {
            self.notify_collection_user_changed(collection.clone());
            self.dispatch_value_replacement_listeners(
                collection,
                field_name,
                FieldUpdateOrigin::User,
            );
        }
        swapped
    }

    fn replace_collection_item<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        index: usize,
        item: Item,
    ) -> bool {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let replaced = self.write_core(|core| core.replace_collection_item(path, index, item));
        if replaced {
            self.notify_collection_changed(collection.clone());
            self.dispatch_value_replacement_listeners(
                collection,
                field_name,
                FieldUpdateOrigin::Programmatic,
            );
        }
        replaced
    }

    fn replace_user_collection_item<Item>(
        &self,
        path: FieldPath<Model, Vec<Item>>,
        index: usize,
        item: Item,
    ) -> bool {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let replaced = self.write_core(|core| core.replace_user_collection_item(path, index, item));
        if replaced {
            self.notify_collection_user_changed(collection.clone());
            self.dispatch_value_replacement_listeners(
                collection,
                field_name,
                FieldUpdateOrigin::User,
            );
        }
        replaced
    }

    fn clear_collection_items<Item>(&self, path: FieldPath<Model, Vec<Item>>) -> bool {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let cleared = self.write_core(|core| core.clear_collection_items(path));
        if cleared.is_empty() {
            return false;
        }
        for item in cleared {
            self.unregister_collection_item_parse_bindings(collection.clone(), item);
        }
        self.notify_collection_changed(collection.clone());
        self.dispatch_value_replacement_listeners(
            collection,
            field_name,
            FieldUpdateOrigin::Programmatic,
        );
        true
    }

    fn clear_user_collection_items<Item>(&self, path: FieldPath<Model, Vec<Item>>) -> bool {
        let collection = path.identity();
        let field_name = path.field_name().to_owned();
        let cleared = self.write_core(|core| core.clear_user_collection_items(path));
        if cleared.is_empty() {
            return false;
        }
        for item in cleared {
            self.unregister_collection_item_parse_bindings(collection.clone(), item);
        }
        self.notify_collection_user_changed(collection.clone());
        self.dispatch_value_replacement_listeners(collection, field_name, FieldUpdateOrigin::User);
        true
    }

    fn collection_item_field_name<Item, Value>(
        &self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
    ) -> Option<String> {
        self.write_core(|core| {
            core.collection_items(collection.clone())
                .into_iter()
                .find(|candidate| candidate.identity() == item)
                .map(|candidate| {
                    CollectionItemFieldAddress::field_name_for(
                        &collection,
                        candidate.index(),
                        &field,
                    )
                })
        })
    }

    fn set_collection_item_field<Item, Value>(
        &self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
        value: Value,
    ) -> bool {
        let collection_identity = collection.identity();
        let identity = CollectionItemFieldAddress::identity_for(&collection, item, &field);
        let field_name = self.collection_item_field_name(collection.clone(), item, field.clone());
        let updated =
            self.write_core(|core| core.set_collection_item_field(collection, item, field, value));
        if updated {
            self.notify_collection_item_field_changed(collection_identity, identity.clone());
            self.dispatch_value_replacement_listeners(
                identity,
                field_name
                    .expect("updated collection item field should have a rendered field name"),
                FieldUpdateOrigin::Programmatic,
            );
        }
        updated
    }

    fn set_user_collection_item_field<Item, Value>(
        &self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
        value: Value,
    ) -> bool {
        let collection_identity = collection.identity();
        let identity = CollectionItemFieldAddress::identity_for(&collection, item, &field);
        let field_name = self.collection_item_field_name(collection.clone(), item, field.clone());
        let updated = self
            .write_core(|core| core.set_user_collection_item_field(collection, item, field, value));
        if updated {
            self.notify_collection_item_field_user_changed(collection_identity, identity.clone());
            self.dispatch_value_replacement_listeners(
                identity,
                field_name
                    .expect("updated collection item field should have a rendered field name"),
                FieldUpdateOrigin::User,
            );
        }
        updated
    }

    fn mark_collection_item_field_touched<Item, Value>(
        &self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
    ) -> bool {
        let identity = CollectionItemFieldAddress::identity_for(&collection, item, &field);
        let touched = self
            .write_core(|core| core.mark_collection_item_field_touched(collection, item, field));
        if touched {
            self.notify_selectors(SelectorTransition::FieldMetadataChanged(identity));
        }
        touched
    }

    fn mark_collection_item_field_blurred<Item, Value>(
        &self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
    ) -> bool {
        let identity = CollectionItemFieldAddress::identity_for(&collection, item, &field);
        let field_name = self.collection_item_field_name(collection.clone(), item, field.clone());
        let (blurred, validates_on_blur) = self.write_core(|core| {
            (
                core.mark_collection_item_field_blurred(collection, item, field),
                core.validation_mode()
                    .should_validate_on_blur(core.submit_attempt_count()),
            )
        });
        if blurred {
            self.notify_selectors(SelectorTransition::FieldMetadataChanged(identity.clone()));
            if validates_on_blur {
                self.notify_validation_changed();
            }
            self.dispatch_form_blur_listeners(
                identity.clone(),
                field_name
                    .expect("blurred collection item field should have a rendered field name"),
            );
            self.dispatch_field_blur_listeners(identity);
        }
        blurred
    }

    fn mark_collection_item_field_blurred_without_validation<Item, Value>(
        &self,
        collection: FieldPath<Model, Vec<Item>>,
        item: CollectionItemIdentity,
        field: FieldPath<Item, Value>,
    ) -> bool {
        let identity = CollectionItemFieldAddress::identity_for(&collection, item, &field);
        let field_name = self.collection_item_field_name(collection.clone(), item, field.clone());
        let blurred = self.write_core(|core| {
            core.mark_collection_item_field_blurred_without_validation(collection, item, field)
        });
        if blurred {
            self.notify_selectors(SelectorTransition::FieldMetadataChanged(identity.clone()));
            self.dispatch_form_blur_listeners(
                identity.clone(),
                field_name
                    .expect("blurred collection item field should have a rendered field name"),
            );
            self.dispatch_field_blur_listeners(identity);
        }
        blurred
    }

    fn mark_multi_select_item_touched<Value: 'static>(
        &self,
        collection: FieldPath<Model, Vec<Value>>,
        item: CollectionItemIdentity,
    ) -> bool {
        let identity = CollectionItemFieldAddress::identity_for(
            &collection,
            item,
            &multi_select_item_value_path(),
        );
        let touched = self.write_core(|core| {
            core.mark_collection_item_field_touched(
                collection,
                item,
                multi_select_item_value_path(),
            )
        });
        if touched {
            self.notify_selectors(SelectorTransition::FieldMetadataChanged(identity));
        }
        touched
    }

    fn mark_multi_select_item_blurred<Value: 'static>(
        &self,
        collection: FieldPath<Model, Vec<Value>>,
        item: CollectionItemIdentity,
    ) -> bool {
        let field = multi_select_item_value_path();
        let identity = CollectionItemFieldAddress::identity_for(&collection, item, &field);
        let field_name = self.collection_item_field_name(collection.clone(), item, field.clone());
        let blurred = self
            .write_core(|core| core.mark_collection_item_field_blurred(collection, item, field));
        if blurred {
            self.notify_selectors(SelectorTransition::FieldMetadataChanged(identity.clone()));
            self.notify_validation_changed();
            self.dispatch_form_blur_listeners(
                identity.clone(),
                field_name.expect("blurred multi-select item should have a rendered field name"),
            );
            self.dispatch_field_blur_listeners(identity);
        }
        blurred
    }

    fn notify_collection_changed(&self, collection: FieldIdentity) {
        self.notify_selectors(SelectorTransition::CollectionStructureChanged(collection));
    }

    fn notify_collection_user_changed(&self, collection: FieldIdentity) {
        self.notify_selectors(SelectorTransition::CollectionStructureUserChanged(
            collection,
        ));
    }

    fn notify_collection_item_field_changed(
        &self,
        collection: FieldIdentity,
        field: FieldIdentity,
    ) {
        self.notify_selectors(SelectorTransition::CollectionItemFieldValueChanged {
            collection,
            field,
        });
    }

    fn notify_collection_item_field_user_changed(
        &self,
        collection: FieldIdentity,
        field: FieldIdentity,
    ) {
        self.notify_selectors(SelectorTransition::CollectionItemFieldUserValueChanged {
            collection,
            field,
        });
    }

    /// Returns tracked user interaction metadata for one typed field through a selector subscription.
    pub fn field_metadata<Value>(&self, path: FieldPath<Model, Value>) -> FieldMetadata {
        self.reactivity.track_field_metadata(&path.identity());
        self.core.borrow().field_metadata(path)
    }

    /// Returns whether one typed field has received user interaction.
    pub fn is_field_touched<Value>(&self, path: FieldPath<Model, Value>) -> bool {
        self.field_metadata(path).is_touched()
    }

    /// Returns whether one typed field has lost focus at least once.
    pub fn is_field_blurred<Value>(&self, path: FieldPath<Model, Value>) -> bool {
        self.field_metadata(path).is_blurred()
    }

    /// Returns every stored validation error across the whole form in one call.
    ///
    /// This is the whole-form aggregate (the source-aware analog of TanStack Form's
    /// `getAllErrors`): it spans every field, collection-item child field, and the form itself,
    /// plus stored submit errors, in deterministic flattened order. Each entry keeps its
    /// `ValidationTarget` (field identity or form) and `ValidatorSource` rather than flattening
    /// multiple sources on one field into a single slot. Field targets are reported as typed
    /// `FieldIdentity`; pair each with your own `FieldPath` to recover the rendered field name for
    /// an accessible error-summary that links to the input.
    ///
    /// This returns all stored errors regardless of visibility; use
    /// [`Self::visible_validation_errors`] for the Error Visibility-honoring variant. See
    /// `docs/error-summary.md`.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.reactivity.track_validation_errors();
        let core = self.core.borrow();

        core.validation_errors()
            .into_iter()
            .map(ValidationErrorSnapshot::from)
            .collect()
    }

    /// Returns validation errors for one typed field through a field-scoped selector subscription.
    pub fn field_validation_errors<Value>(
        &self,
        path: FieldPath<Model, Value>,
    ) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.reactivity
            .track_field_validation_errors(&path.identity());
        let core = self.core.borrow();

        core.field_validation_errors(path)
            .into_iter()
            .map(ValidationErrorSnapshot::from)
            .collect()
    }

    fn field_validation_errors_by_identity(
        &self,
        field: &FieldIdentity,
    ) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.reactivity.track_field_validation_errors(field);
        let core = self.core.borrow();

        core.field_validation_errors_by_identity(field)
            .into_iter()
            .map(ValidationErrorSnapshot::from)
            .collect()
    }

    /// Returns form-level validation errors through a form-error selector subscription.
    pub fn form_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.reactivity.track_form_validation_errors();
        let core = self.core.borrow();

        core.form_validation_errors()
            .into_iter()
            .map(ValidationErrorSnapshot::from)
            .collect()
    }

    /// Returns visible validation errors through a form-wide visibility selector subscription.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.reactivity.track_visible_validation_errors();
        let core = self.core.borrow();

        core.visible_validation_errors()
            .into_iter()
            .map(ValidationErrorSnapshot::from)
            .collect()
    }

    /// Returns visible validation errors relevant to one submit intent.
    pub fn visible_validation_errors_for_intent<Intent>(
        &self,
        intent: &Intent,
    ) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Intent: PartialEq + 'static,
        Error: Clone,
    {
        self.reactivity.track_visible_validation_errors();
        let core = self.core.borrow();

        core.visible_validation_errors_for_intent(intent)
            .into_iter()
            .map(ValidationErrorSnapshot::from)
            .collect()
    }

    /// Returns visible validation errors for one field through a field-scoped selector subscription.
    pub fn visible_field_validation_errors<Value>(
        &self,
        path: FieldPath<Model, Value>,
    ) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.reactivity
            .track_visible_field_validation_errors(&path.identity());
        let core = self.core.borrow();

        core.visible_field_validation_errors(path)
            .into_iter()
            .map(ValidationErrorSnapshot::from)
            .collect()
    }

    /// Returns visible validation errors relevant to one submit intent for one field.
    pub fn visible_field_validation_errors_for_intent<Value, Intent>(
        &self,
        path: FieldPath<Model, Value>,
        intent: &Intent,
    ) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Intent: PartialEq + 'static,
        Error: Clone,
    {
        self.reactivity
            .track_visible_field_validation_errors(&path.identity());
        let core = self.core.borrow();

        core.visible_field_validation_errors_for_intent(path, intent)
            .into_iter()
            .map(ValidationErrorSnapshot::from)
            .collect()
    }

    fn visible_field_validation_errors_by_identity(
        &self,
        field: &FieldIdentity,
    ) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.reactivity.track_visible_field_validation_errors(field);
        let core = self.core.borrow();

        core.visible_field_validation_errors_by_identity(field)
            .into_iter()
            .map(ValidationErrorSnapshot::from)
            .collect()
    }

    /// Returns visible form-level validation errors through a form-error selector subscription.
    pub fn visible_form_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.reactivity.track_visible_form_validation_errors();
        let core = self.core.borrow();

        core.visible_form_validation_errors()
            .into_iter()
            .map(ValidationErrorSnapshot::from)
            .collect()
    }

    /// Returns visible form-level validation errors relevant to one submit intent.
    pub fn visible_form_validation_errors_for_intent<Intent>(
        &self,
        intent: &Intent,
    ) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Intent: PartialEq + 'static,
        Error: Clone,
    {
        self.reactivity.track_visible_form_validation_errors();
        let core = self.core.borrow();

        core.visible_form_validation_errors_for_intent(intent)
            .into_iter()
            .map(ValidationErrorSnapshot::from)
            .collect()
    }

    /// Registers a synchronous validator for one direct field and trigger set.
    fn register_sync_field_validator_for_triggers<Value, Source, Triggers, Validator>(
        &self,
        path: FieldPath<Model, Value>,
        source: Source,
        triggers: Triggers,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Vec<Error> + 'static,
        Model: 'static,
        Value: 'static,
    {
        let id = self.write_core(|core| {
            core.register_sync_field_validator_for_triggers(path, source, triggers, validator)
        });
        self.notify_selectors(SelectorTransition::ValidationChanged);
        id
    }

    /// Registers a zero-or-one-error synchronous validator for one direct field and trigger set.
    fn register_sync_field_validator_optional_for_triggers<Value, Source, Triggers, Validator>(
        &self,
        path: FieldPath<Model, Value>,
        source: Source,
        triggers: Triggers,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Option<Error> + 'static,
        Model: 'static,
        Value: 'static,
    {
        let id = self.write_core(|core| {
            core.register_sync_field_validator_optional_for_triggers(
                path, source, triggers, validator,
            )
        });
        self.notify_selectors(SelectorTransition::ValidationChanged);
        id
    }

    /// Starts a registered async field validator on the Dioxus task runtime.
    ///
    /// The validator runs from owned snapshot values. Completion is ignored when the result is stale
    /// or the Dioxus component has been cleaned up.
    pub fn validate_async_field_validator<Value, Validator, Fut, Errors>(
        &self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Value: Clone + 'static,
        Validator: FnOnce(Value, AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        self.validate_async_field_validator_with_sync_mode(path, id, trigger, false, validator)
    }

    fn validate_async_field_validator_after_sync<Value, Validator, Fut, Errors>(
        &self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Value: Clone + 'static,
        Validator: FnOnce(Value, AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        self.validate_async_field_validator_with_sync_mode(path, id, trigger, true, validator)
    }

    fn validate_async_field_validator_with_sync_mode<Value, Validator, Fut, Errors>(
        &self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
        sync_already_ran: bool,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Value: Clone + 'static,
        Validator: FnOnce(Value, AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        let target = ValidationTarget::Field(path.identity());
        let run = if sync_already_ran {
            self.write_core(|core| {
                core.begin_async_field_validation_after_sync(path.clone(), id, trigger)
            })
        } else {
            self.write_core(|core| core.begin_async_field_validation(path.clone(), id, trigger))
        };
        self.notify_validation_changed();
        let run = run?;

        self.adapter.cancel_validation_task(target.clone(), id);
        let handle = self.clone();
        let context = run.validator_context();
        let field_value = run.field_value().clone();
        let future = validator(field_value, context);

        self.spawn_validation_task(target, id, async move {
            let errors = future.await;

            if !handle.is_active() {
                return;
            }

            let status = handle
                .write_core(|core| core.complete_async_field_validation(path, id, &run, errors));

            if status.is_some() {
                handle.notify_validation_changed();
            }
        });

        Some(ValidationStatus::Pending)
    }

    /// Starts a registered async field validator, debouncing value-change validation only.
    pub fn validate_async_field_validator_with_debounce<Value, Delay, Validator, Fut, Errors>(
        &self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
        delay: Delay,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Value: Clone + 'static,
        Delay: Future<Output = ()> + 'static,
        Validator: FnOnce(Value, AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        self.validate_async_field_validator_with_debounce_sync_mode(
            path, id, trigger, delay, false, validator,
        )
    }

    fn validate_async_field_validator_with_debounce_after_sync<
        Value,
        Delay,
        Validator,
        Fut,
        Errors,
    >(
        &self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
        delay: Delay,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Value: Clone + 'static,
        Delay: Future<Output = ()> + 'static,
        Validator: FnOnce(Value, AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        self.validate_async_field_validator_with_debounce_sync_mode(
            path, id, trigger, delay, true, validator,
        )
    }

    fn validate_async_field_validator_with_debounce_sync_mode<
        Value,
        Delay,
        Validator,
        Fut,
        Errors,
    >(
        &self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
        delay: Delay,
        sync_already_ran: bool,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Value: Clone + 'static,
        Delay: Future<Output = ()> + 'static,
        Validator: FnOnce(Value, AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        if trigger != ValidationTrigger::Change {
            return self.validate_async_field_validator_with_sync_mode(
                path,
                id,
                trigger,
                sync_already_ran,
                validator,
            );
        }

        let target = ValidationTarget::Field(path.identity());
        self.adapter.cancel_debounced_validation(target.clone(), id);

        let scheduled = if sync_already_ran {
            self.write_core(|core| {
                core.schedule_debounced_async_field_validation_after_sync(path.clone(), id, trigger)
            })
        } else {
            self.write_core(|core| {
                core.schedule_debounced_async_field_validation(path.clone(), id, trigger)
            })
        };
        self.notify_validation_changed();
        let scheduled = scheduled?;

        self.adapter.cancel_validation_task(target.clone(), id);
        let handle = self.clone();
        let delay = self
            .adapter
            .register_debounced_validation(target.clone(), id)
            .delay(delay);

        self.spawn_validation_task(target, id, async move {
            let wake = delay.await;

            if !handle.is_active() {
                return;
            }

            let run = handle.write_core(|core| match wake {
                DebounceWake::TimerElapsed => {
                    core.begin_debounced_async_field_validation(path.clone(), id, &scheduled)
                }
                DebounceWake::Flushed(trigger) => core
                    .flush_debounced_async_field_validation_for_trigger(
                        path.clone(),
                        id,
                        &scheduled,
                        trigger,
                    ),
                DebounceWake::Cancelled => None,
            });
            let Some(run) = run else {
                return;
            };
            handle.notify_validation_changed();

            let context = run.validator_context();
            let field_value = run.field_value().clone();
            let errors = validator(field_value, context).await;

            if !handle.is_active() {
                return;
            }

            let status = handle
                .write_core(|core| core.complete_async_field_validation(path, id, &run, errors));

            if status.is_some() {
                handle.notify_validation_changed();
            }
        });

        Some(ValidationStatus::Pending)
    }

    fn validate_async_field_identity_validator<Validator, Fut, Errors>(
        &self,
        field: FieldIdentity,
        id: ValidatorId,
        trigger: ValidationTrigger,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        self.validate_async_field_identity_validator_with_sync_mode(
            field, id, trigger, false, validator,
        )
    }

    fn validate_async_field_identity_validator_after_sync<Validator, Fut, Errors>(
        &self,
        field: FieldIdentity,
        id: ValidatorId,
        trigger: ValidationTrigger,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        self.validate_async_field_identity_validator_with_sync_mode(
            field, id, trigger, true, validator,
        )
    }

    fn validate_async_field_identity_validator_with_sync_mode<Validator, Fut, Errors>(
        &self,
        field: FieldIdentity,
        id: ValidatorId,
        trigger: ValidationTrigger,
        sync_already_ran: bool,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        let target = ValidationTarget::Field(field.clone());
        let run = if sync_already_ran {
            self.write_core(|core| {
                core.begin_async_field_identity_validation_after_sync(field.clone(), id, trigger)
            })
        } else {
            self.write_core(|core| {
                core.begin_async_field_identity_validation(field.clone(), id, trigger)
            })
        };
        self.notify_validation_changed();
        let run = run?;

        self.adapter.cancel_validation_task(target.clone(), id);
        let handle = self.clone();
        let context = run.validator_context();
        let future = validator(context);

        self.spawn_validation_task(target, id, async move {
            let errors = future.await;

            if !handle.is_active() {
                return;
            }

            let status = handle.write_core(|core| {
                core.complete_async_field_identity_validation(field, id, &run, errors)
            });

            if status.is_some() {
                handle.notify_validation_changed();
            }
        });

        Some(ValidationStatus::Pending)
    }

    fn validate_async_field_identity_validator_with_debounce<Delay, Validator, Fut, Errors>(
        &self,
        field: FieldIdentity,
        id: ValidatorId,
        trigger: ValidationTrigger,
        delay: Delay,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Delay: Future<Output = ()> + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        self.validate_async_field_identity_validator_with_debounce_sync_mode(
            field, id, trigger, delay, false, validator,
        )
    }

    fn validate_async_field_identity_validator_with_debounce_after_sync<
        Delay,
        Validator,
        Fut,
        Errors,
    >(
        &self,
        field: FieldIdentity,
        id: ValidatorId,
        trigger: ValidationTrigger,
        delay: Delay,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Delay: Future<Output = ()> + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        self.validate_async_field_identity_validator_with_debounce_sync_mode(
            field, id, trigger, delay, true, validator,
        )
    }

    fn validate_async_field_identity_validator_with_debounce_sync_mode<
        Delay,
        Validator,
        Fut,
        Errors,
    >(
        &self,
        field: FieldIdentity,
        id: ValidatorId,
        trigger: ValidationTrigger,
        delay: Delay,
        sync_already_ran: bool,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Delay: Future<Output = ()> + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = Error> + 'static,
        Error: 'static,
    {
        if trigger != ValidationTrigger::Change {
            return self.validate_async_field_identity_validator_with_sync_mode(
                field,
                id,
                trigger,
                sync_already_ran,
                validator,
            );
        }

        let target = ValidationTarget::Field(field.clone());
        self.adapter.cancel_debounced_validation(target.clone(), id);

        let scheduled = if sync_already_ran {
            self.write_core(|core| {
                core.schedule_debounced_async_field_identity_validation_after_sync(
                    field.clone(),
                    id,
                    trigger,
                )
            })
        } else {
            self.write_core(|core| {
                core.schedule_debounced_async_field_identity_validation(field.clone(), id, trigger)
            })
        };
        self.notify_validation_changed();
        let scheduled = scheduled?;

        self.adapter.cancel_validation_task(target.clone(), id);
        let handle = self.clone();
        let delay = self
            .adapter
            .register_debounced_validation(target.clone(), id)
            .delay(delay);

        self.spawn_validation_task(target, id, async move {
            let wake = delay.await;

            if !handle.is_active() {
                return;
            }

            let run = handle.write_core(|core| match wake {
                DebounceWake::TimerElapsed => core.begin_debounced_async_field_identity_validation(
                    field.clone(),
                    id,
                    &scheduled,
                ),
                DebounceWake::Flushed(trigger) => core
                    .flush_debounced_async_field_identity_validation_for_trigger(
                        field.clone(),
                        id,
                        &scheduled,
                        trigger,
                    ),
                DebounceWake::Cancelled => None,
            });
            let Some(run) = run else {
                return;
            };
            handle.notify_validation_changed();

            let context = run.validator_context();
            let errors = validator(context).await;

            if !handle.is_active() {
                return;
            }

            let status = handle.write_core(|core| {
                core.complete_async_field_identity_validation(field, id, &run, errors)
            });

            if status.is_some() {
                handle.notify_validation_changed();
            }
        });

        Some(ValidationStatus::Pending)
    }

    /// Starts a registered async form validator on the Dioxus task runtime.
    ///
    /// The validator runs from an owned snapshot value. Completion is ignored when the result is
    /// stale or the Dioxus component has been cleaned up.
    pub fn validate_async_form_validator<Validator, Fut, Errors>(
        &self,
        id: ValidatorId,
        trigger: ValidationTrigger,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = FormValidationError<Error>> + 'static,
        Error: 'static,
    {
        self.validate_async_form_validator_with_sync_mode(id, trigger, false, validator)
    }

    fn validate_async_form_validator_after_sync<Validator, Fut, Errors>(
        &self,
        id: ValidatorId,
        trigger: ValidationTrigger,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = FormValidationError<Error>> + 'static,
        Error: 'static,
    {
        self.validate_async_form_validator_with_sync_mode(id, trigger, true, validator)
    }

    fn validate_async_form_validator_with_sync_mode<Validator, Fut, Errors>(
        &self,
        id: ValidatorId,
        trigger: ValidationTrigger,
        sync_already_ran: bool,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = FormValidationError<Error>> + 'static,
        Error: 'static,
    {
        let run = if sync_already_ran {
            self.write_core(|core| core.begin_async_form_validation_after_sync(id, trigger))
        } else {
            self.write_core(|core| core.begin_async_form_validation(id, trigger))
        };
        self.notify_validation_changed();
        let run = run?;

        self.adapter
            .cancel_validation_task(ValidationTarget::Form, id);
        let handle = self.clone();
        let context = run.validator_context();
        let future = validator(context);

        self.spawn_validation_task(ValidationTarget::Form, id, async move {
            let errors = future.await;

            if !handle.is_active() {
                return;
            }

            let status =
                handle.write_core(|core| core.complete_async_form_validation(id, &run, errors));

            if status.is_some() {
                handle.notify_validation_changed();
            }
        });

        Some(ValidationStatus::Pending)
    }

    /// Starts a registered async form validator, debouncing value-change validation only.
    pub fn validate_async_form_validator_with_debounce<Delay, Validator, Fut, Errors>(
        &self,
        id: ValidatorId,
        trigger: ValidationTrigger,
        delay: Delay,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Delay: Future<Output = ()> + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = FormValidationError<Error>> + 'static,
        Error: 'static,
    {
        self.validate_async_form_validator_with_debounce_sync_mode(
            id, trigger, delay, false, validator,
        )
    }

    fn validate_async_form_validator_with_debounce_after_sync<Delay, Validator, Fut, Errors>(
        &self,
        id: ValidatorId,
        trigger: ValidationTrigger,
        delay: Delay,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Delay: Future<Output = ()> + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = FormValidationError<Error>> + 'static,
        Error: 'static,
    {
        self.validate_async_form_validator_with_debounce_sync_mode(
            id, trigger, delay, true, validator,
        )
    }

    fn validate_async_form_validator_with_debounce_sync_mode<Delay, Validator, Fut, Errors>(
        &self,
        id: ValidatorId,
        trigger: ValidationTrigger,
        delay: Delay,
        sync_already_ran: bool,
        validator: Validator,
    ) -> Option<ValidationStatus>
    where
        Model: Clone + 'static,
        Delay: Future<Output = ()> + 'static,
        Validator: FnOnce(AsyncValidatorContext<Model>) -> Fut + 'static,
        Fut: Future<Output = Errors> + 'static,
        Errors: IntoIterator<Item = FormValidationError<Error>> + 'static,
        Error: 'static,
    {
        if trigger != ValidationTrigger::Change {
            return self.validate_async_form_validator_with_sync_mode(
                id,
                trigger,
                sync_already_ran,
                validator,
            );
        }

        self.adapter
            .cancel_debounced_validation(ValidationTarget::Form, id);

        let scheduled = if sync_already_ran {
            self.write_core(|core| {
                core.schedule_debounced_async_form_validation_after_sync(id, trigger)
            })
        } else {
            self.write_core(|core| core.schedule_debounced_async_form_validation(id, trigger))
        };
        self.notify_validation_changed();
        let scheduled = scheduled?;

        self.adapter
            .cancel_validation_task(ValidationTarget::Form, id);
        let handle = self.clone();
        let delay = self
            .adapter
            .register_debounced_validation(ValidationTarget::Form, id)
            .delay(delay);

        self.spawn_validation_task(ValidationTarget::Form, id, async move {
            let wake = delay.await;

            if !handle.is_active() {
                return;
            }

            let run = handle.write_core(|core| match wake {
                DebounceWake::TimerElapsed => {
                    core.begin_debounced_async_form_validation(id, &scheduled)
                }
                DebounceWake::Flushed(trigger) => {
                    core.flush_debounced_async_form_validation_for_trigger(id, &scheduled, trigger)
                }
                DebounceWake::Cancelled => None,
            });
            let Some(run) = run else {
                return;
            };
            handle.notify_validation_changed();

            let context = run.validator_context();
            let errors = validator(context).await;

            if !handle.is_active() {
                return;
            }

            let status =
                handle.write_core(|core| core.complete_async_form_validation(id, &run, errors));

            if status.is_some() {
                handle.notify_validation_changed();
            }
        });

        Some(ValidationStatus::Pending)
    }

    /// Registers a synchronous validator for the whole form and trigger set.
    fn register_sync_form_validator_for_triggers<Source, Triggers, Validator>(
        &self,
        source: Source,
        triggers: Triggers,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
        Validator: for<'a> Fn(FormValidatorContext<'a, Model>) -> Vec<FormValidationError<Error>>
            + 'static,
        Model: 'static,
    {
        let id = self.write_core(|core| {
            core.register_sync_form_validator_for_triggers(source, triggers, validator)
        });
        self.notify_selectors(SelectorTransition::ValidationChanged);
        id
    }

    /// Registers a zero-or-one-error synchronous validator for the whole form and trigger set.
    fn register_sync_form_validator_optional_for_triggers<Source, Triggers, Validator>(
        &self,
        source: Source,
        triggers: Triggers,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
        Validator: for<'a> Fn(FormValidatorContext<'a, Model>) -> Option<FormValidationError<Error>>
            + 'static,
        Model: 'static,
    {
        let id = self.write_core(|core| {
            core.register_sync_form_validator_optional_for_triggers(source, triggers, validator)
        });
        self.notify_selectors(SelectorTransition::ValidationChanged);
        id
    }

    /// Registers a synchronous validator template for one collection item child field and trigger set.
    fn register_sync_collection_item_field_validator_for_triggers<
        Item,
        Value,
        Source,
        Triggers,
        Validator,
    >(
        &self,
        collection: FieldPath<Model, Vec<Item>>,
        field: FieldPath<Item, Value>,
        source: Source,
        triggers: Triggers,
        validator: Validator,
    ) -> ValidatorId
    where
        Source: Into<ValidatorSource>,
        Triggers: Into<ValidationTriggers>,
        Validator: for<'a> Fn(&'a Value, ValidatorContext<'a, Model>) -> Vec<Error> + 'static,
        Model: 'static,
        Item: 'static,
        Value: 'static,
    {
        let id = self.write_core(|core| {
            core.register_sync_collection_item_field_validator_for_triggers(
                collection, field, source, triggers, validator,
            )
        });
        self.notify_selectors(SelectorTransition::ValidationChanged);
        id
    }

    /// Runs one validator registered for one field and trigger.
    pub fn validate_field_validator<Value>(
        &self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<ValidationStatus> {
        let field = path.identity();
        let status = self
            .write_core(|core| core.validate_field_validator(path, id, trigger))
            .or_else(|| {
                self.start_runtime_async_field_validator(field.clone(), id, trigger, false)
            });
        self.notify_selectors(SelectorTransition::FieldValidationChanged(field));
        status
    }

    /// Runs the first validator source label registered for one field and trigger.
    pub fn validate_field_source<Value, Source>(
        &self,
        path: FieldPath<Model, Value>,
        source: Source,
        trigger: ValidationTrigger,
    ) -> Option<ValidationStatus>
    where
        Source: Into<ValidatorSource>,
    {
        let source = source.into();
        let field = path.identity();
        let status = self
            .write_core(|core| core.validate_field_source(path.clone(), source.clone(), trigger))
            .or_else(|| {
                let id = self.runtime_field_validator_id_for_source(path, &source)?;
                self.start_runtime_async_field_validator(field.clone(), id, trigger, false)
            });
        self.notify_selectors(SelectorTransition::FieldValidationChanged(field));
        status
    }

    /// Runs one form validator by stable ID and trigger.
    pub fn validate_form_validator(
        &self,
        id: ValidatorId,
        trigger: ValidationTrigger,
    ) -> Option<ValidationStatus> {
        let status = self
            .write_core(|core| core.validate_form_validator(id, trigger))
            .or_else(|| self.start_runtime_async_form_validator(id, trigger, false));
        self.notify_validation_changed();
        status
    }

    /// Runs the first form validator source label registered for the whole form and trigger.
    pub fn validate_form_source<Source>(
        &self,
        source: Source,
        trigger: ValidationTrigger,
    ) -> Option<ValidationStatus>
    where
        Source: Into<ValidatorSource>,
    {
        let source = source.into();
        let status = self
            .write_core(|core| core.validate_form_source(source.clone(), trigger))
            .or_else(|| {
                let id = self.runtime_form_validator_id_for_source(&source)?;
                self.start_runtime_async_form_validator(id, trigger, false)
            });
        self.notify_validation_changed();
        status
    }

    /// Returns the current status for one registered field validator.
    pub fn field_validation_status<Value>(
        &self,
        path: FieldPath<Model, Value>,
        id: ValidatorId,
    ) -> Option<ValidationStatus> {
        self.reactivity.track_validation_errors();
        self.core.borrow().field_validation_status(path, id)
    }

    /// Returns the current status for the first registered field validator with this source label.
    pub fn validation_status<Value, Source>(
        &self,
        path: FieldPath<Model, Value>,
        source: Source,
    ) -> Option<ValidationStatus>
    where
        Source: Into<ValidatorSource>,
    {
        self.reactivity.track_validation_errors();
        self.core.borrow().validation_status(path, source)
    }

    /// Returns the current status for one registered form validator.
    pub fn form_validation_status_by_id(&self, id: ValidatorId) -> Option<ValidationStatus> {
        self.reactivity.track_validation_errors();
        self.core.borrow().form_validation_status_by_id(id)
    }

    /// Returns the current status for the first registered form validator with this source label.
    pub fn form_validation_status<Source>(&self, source: Source) -> Option<ValidationStatus>
    where
        Source: Into<ValidatorSource>,
    {
        self.reactivity.track_validation_errors();
        self.core.borrow().form_validation_status(source)
    }

    /// Returns source-level validation statuses in deterministic flattened order.
    pub fn validation_statuses(&self) -> Vec<ValidationStatusView> {
        self.reactivity.track_validation_errors();
        self.core.borrow().validation_statuses()
    }

    /// Returns source-level validation statuses for one field.
    pub fn field_validation_statuses<Value>(
        &self,
        path: FieldPath<Model, Value>,
    ) -> Vec<ValidationStatusView> {
        self.reactivity.track_validation_errors();
        self.core.borrow().field_validation_statuses(path)
    }

    /// Returns source-level validation statuses for form validators.
    pub fn form_validation_statuses(&self) -> Vec<ValidationStatusView> {
        self.reactivity.track_validation_errors();
        self.core.borrow().form_validation_statuses()
    }

    /// Returns whether any registered validator currently has a pending status.
    ///
    /// A convenience read over existing **Validation Status** state; it stores nothing new.
    pub fn is_validating(&self) -> bool {
        self.reactivity.track_validation_errors();
        self.core.borrow().is_validating()
    }

    /// Returns whether any validator attached to one field currently has a pending status.
    pub fn is_field_validating<Value>(&self, path: FieldPath<Model, Value>) -> bool {
        self.reactivity.track_validation_errors();
        self.core.borrow().is_field_validating(path)
    }

    /// Runs validators registered for one field and trigger, then form validators for the same trigger.
    pub fn validate_field<Value>(&self, path: FieldPath<Model, Value>, trigger: ValidationTrigger) {
        let field = path.identity();
        self.write_core(|core| core.validate_field(path, trigger));
        self.start_runtime_async_field_validators(field, trigger);
        self.start_runtime_async_form_validators(trigger);
        self.notify_selectors(SelectorTransition::ValidationChanged);
    }

    /// Runs all form validators registered for one trigger.
    pub fn validate_form(&self, trigger: ValidationTrigger) {
        self.write_core(|core| core.validate_form(trigger));
        self.start_runtime_async_form_validators(trigger);
        self.notify_selectors(SelectorTransition::ValidationChanged);
    }

    /// Runs all validators registered for one trigger.
    pub fn validate_all(&self, trigger: ValidationTrigger) {
        self.write_core(|core| core.validate_all(trigger));
        self.start_runtime_async_validators(trigger);
        self.notify_selectors(SelectorTransition::ValidationChanged);
    }

    /// Explicitly runs validators registered for form initialization.
    ///
    /// Creating a handle and registering validators never call this automatically.
    /// The returned boolean reflects immediate synchronous initialization validation only; async
    /// initialization validators may be `Pending` after this method returns.
    pub fn validate_initialization(&self) -> bool {
        let valid = self.write_core(FormCore::validate_initialization);
        self.start_runtime_async_validators(ValidationTrigger::Initial);
        self.notify_selectors(SelectorTransition::ValidationChanged);
        valid
    }

    /// Records a submit attempt and runs submit-triggered validators.
    pub fn validate_for_submit(&self) -> bool {
        self.validate_intent_for_submit(())
    }

    fn validate_intent_for_submit<Intent>(&self, intent: Intent) -> bool
    where
        Intent: Clone + PartialEq + 'static,
    {
        let availability_intent = intent.clone();
        let listener_intent = intent.clone();
        let valid = self.write_core(|core| core.intent(intent).validate_for_submit());
        self.start_runtime_async_validators(ValidationTrigger::Submit);
        self.notify_selectors(SelectorTransition::ValidationChanged);
        let blocker = self
            .intent_availability(&availability_intent)
            .blockers()
            .first()
            .copied()
            .or_else(|| (!valid).then_some(SubmitBlocker::ValidationErrors));

        if let Some(blocker) = blocker {
            self.record_validate_for_submit_blocker(blocker, listener_intent.clone());
        }

        self.dispatch_submit_listeners(
            SubmitListenerEvent::SubmitAttempted,
            listener_intent.clone(),
        );

        if let Some(blocker) = blocker {
            self.dispatch_submit_listeners(
                SubmitListenerEvent::SubmitBlocked(blocker),
                listener_intent,
            );
        }

        valid
    }

    fn record_validate_for_submit_blocker<Intent>(&self, blocker: SubmitBlocker, intent: Intent)
    where
        Intent: 'static,
    {
        self.write_core(|core| core.record_submit_blocker_after_attempt(blocker, intent));
        self.notify_submit_changed();
    }

    /// Replaces one typed field value through the form core.
    /// Runs the fixed reactivity, validation, and listener sequence that follows a field write.
    ///
    /// `validates` is the write's own decision (from the validation mode) about whether this
    /// mutation triggers validation. The ordering here is the invariant every mutating field
    /// method shares; callers supply only the per-mutation `FieldMutation` deltas.
    fn apply_field_mutation(&self, mutation: FieldMutation, validates: bool) {
        let FieldMutation {
            field,
            field_name,
            selectors,
            trigger,
            dispatch,
        } = mutation;

        for transition in selectors {
            self.notify_selectors(transition);
        }

        if validates {
            self.start_runtime_async_field_validators(field.clone(), trigger);
            self.start_runtime_async_form_validators(trigger);
        }

        // Every field mutation notifies validation subscribers, whether or not it ran validation:
        // value writes clear submit errors and invalidate async validators, and blur flips the
        // blurred/touched metadata that gates blur- and touch-scoped error visibility, so any
        // mutation can change what a validation subscriber should see. See issue #129.
        self.notify_validation_changed();

        match dispatch {
            FieldMutationDispatch::ValueReplacement(origin) => {
                self.dispatch_value_replacement_listeners(field, field_name, origin);
            }
            FieldMutationDispatch::Blur => {
                self.dispatch_form_blur_listeners(field.clone(), field_name);
                self.dispatch_field_blur_listeners(field);
            }
        }
    }

    pub fn set_field<Value>(&self, path: FieldPath<Model, Value>, value: Value) {
        let field = path.identity();
        let field_name = path.field_name().to_owned();
        let validates_on_change = self.write_core(|core| {
            core.set_field(path, value);
            core.validation_mode()
                .should_validate_on_change(core.submit_attempt_count())
        });

        self.apply_field_mutation(
            FieldMutation {
                field: field.clone(),
                field_name,
                selectors: vec![SelectorTransition::FieldValueChanged(field)],
                trigger: ValidationTrigger::Change,
                dispatch: FieldMutationDispatch::ValueReplacement(FieldUpdateOrigin::Programmatic),
            },
            validates_on_change,
        );
    }

    /// Replaces one typed field value because of user input.
    pub fn set_user_field<Value>(&self, path: FieldPath<Model, Value>, value: Value) {
        let field = path.identity();
        let field_name = path.field_name().to_owned();
        let validates_on_change = self.write_core(|core| {
            core.set_user_field(path, value);
            core.validation_mode()
                .should_validate_on_change(core.submit_attempt_count())
        });

        self.apply_field_mutation(
            FieldMutation {
                field: field.clone(),
                field_name,
                selectors: vec![
                    SelectorTransition::FieldValueChanged(field.clone()),
                    SelectorTransition::FieldMetadataChanged(field),
                ],
                trigger: ValidationTrigger::Change,
                dispatch: FieldMutationDispatch::ValueReplacement(FieldUpdateOrigin::User),
            },
            validates_on_change,
        );
    }

    /// Marks a field as touched by user interaction.
    pub fn mark_field_touched<Value>(&self, path: FieldPath<Model, Value>) {
        let field = path.identity();
        self.write_core(|core| core.mark_field_touched(path));
        self.notify_selectors(SelectorTransition::FieldMetadataChanged(field));
    }

    fn mark_field_blurred_without_validation<Value>(&self, path: FieldPath<Model, Value>) {
        let field = path.identity();
        let field_name = path.field_name().to_owned();
        self.write_core(|core| core.mark_field_blurred_without_validation(path));

        self.apply_field_mutation(
            FieldMutation {
                field: field.clone(),
                field_name,
                selectors: vec![SelectorTransition::FieldMetadataChanged(field)],
                trigger: ValidationTrigger::Blur,
                dispatch: FieldMutationDispatch::Blur,
            },
            false,
        );
    }

    /// Marks a field as blurred and touched by user interaction.
    pub fn mark_field_blurred<Value>(&self, path: FieldPath<Model, Value>) {
        let field = path.identity();
        let field_name = path.field_name().to_owned();
        let validates_on_blur = self.write_core(|core| {
            let validates_on_blur = core
                .validation_mode()
                .should_validate_on_blur(core.submit_attempt_count());
            core.mark_field_blurred(path.clone());
            validates_on_blur
        });

        self.apply_field_mutation(
            FieldMutation {
                field: field.clone(),
                field_name,
                selectors: vec![SelectorTransition::FieldMetadataChanged(field)],
                trigger: ValidationTrigger::Blur,
                dispatch: FieldMutationDispatch::Blur,
            },
            validates_on_blur,
        );
    }

    /// Returns all mounted binding parse errors separately from validation errors through a selector subscription.
    pub fn parse_errors(&self) -> Vec<ParseError> {
        self.reactivity.track_parse_errors();
        self.adapter.parse_errors()
    }

    /// Returns mounted binding parse errors for one typed field path through a field-scoped selector subscription.
    pub fn field_parse_errors<Value>(&self, path: FieldPath<Model, Value>) -> Vec<ParseError> {
        let field = path.identity();
        self.reactivity.track_field_parse_errors(&field);
        self.adapter.field_parse_errors(field)
    }

    /// Returns headless accessibility IDs and ARIA state for one typed field path through selector subscriptions.
    pub fn field_accessibility<Value>(&self, path: FieldPath<Model, Value>) -> FieldAccessibility {
        let field = path.identity();
        self.reactivity
            .track_visible_field_validation_errors(&field);
        self.reactivity.track_field_parse_errors(&field);

        let has_visible_validation_errors = !self
            .core
            .borrow()
            .visible_field_validation_errors(path.clone())
            .is_empty();
        let has_parse_errors = self.adapter.has_field_parse_errors(field);

        FieldAccessibility::new(
            &self.id_namespace,
            path.field_name(),
            has_visible_validation_errors,
            has_parse_errors,
        )
    }

    fn intent_field_accessibility<Value, Intent>(
        &self,
        path: FieldPath<Model, Value>,
        intent: &Intent,
    ) -> FieldAccessibility
    where
        Intent: PartialEq + 'static,
    {
        let field = path.identity();
        self.reactivity
            .track_visible_field_validation_errors(&field);
        self.reactivity.track_field_parse_errors(&field);

        let has_visible_validation_errors = !self
            .core
            .borrow()
            .visible_field_validation_errors_for_intent(path.clone(), intent)
            .is_empty();
        let has_parse_errors = self.adapter.has_field_parse_errors(field);

        FieldAccessibility::new(
            &self.id_namespace,
            path.field_name(),
            has_visible_validation_errors,
            has_parse_errors,
        )
    }

    fn field_accessibility_by_identity(
        &self,
        field: FieldIdentity,
        accessibility_name: &str,
    ) -> FieldAccessibility {
        self.reactivity
            .track_visible_field_validation_errors(&field);
        self.reactivity.track_field_parse_errors(&field);

        let has_visible_validation_errors = !self
            .core
            .borrow()
            .visible_field_validation_errors_by_identity(&field)
            .is_empty();
        let has_parse_errors = self.adapter.has_field_parse_errors(field);

        FieldAccessibility::new(
            &self.id_namespace,
            accessibility_name,
            has_visible_validation_errors,
            has_parse_errors,
        )
    }

    fn has_parse_blockers(&self) -> bool {
        self.adapter.has_parse_blockers()
    }

    fn register_parse_binding(&self, field: FieldIdentity) -> ParseBindingRegistration {
        let id = self.adapter.register_parse_binding(field.clone());
        ParseBindingRegistration::new(self.adapter.clone(), Rc::clone(&self.reactivity), id, field)
    }

    fn unregister_collection_item_parse_bindings(
        &self,
        collection: FieldIdentity,
        item: CollectionItemIdentity,
    ) {
        for field in self
            .adapter
            .unregister_collection_item_parse_bindings(collection, item)
        {
            self.notify_selectors(SelectorTransition::ParseChanged(field));
        }
    }

    /// Creates a controlled text binding for a `String` field.
    pub fn text(&self, path: FieldPath<Model, String>) -> TextBinding<Model, Error> {
        TextBinding {
            base: FieldBindingCore::new(self.clone(), path),
        }
    }

    /// Creates a controlled textarea binding for a `String` field.
    pub fn textarea(&self, path: FieldPath<Model, String>) -> TextareaBinding<Model, Error> {
        TextareaBinding {
            base: FieldBindingCore::new(self.clone(), path),
        }
    }

    /// Creates a controlled checkbox binding for a `bool` field.
    pub fn checkbox(&self, path: FieldPath<Model, bool>) -> CheckboxBinding<Model, Error> {
        CheckboxBinding {
            base: FieldBindingCore::new(self.clone(), path),
        }
    }

    /// Creates a headless file-selection binding outside the typed form draft.
    pub fn file(&self, key: FileFieldKey<Model>) -> FileSelectionBinding<Model, Error> {
        FileSelectionBinding {
            handle: self.clone(),
            key,
        }
    }

    /// Creates a headless controlled select binding for a typed field.
    pub fn select<Value>(
        &self,
        path: FieldPath<Model, Value>,
    ) -> SelectBinding<Model, Value, Error> {
        SelectBinding {
            base: FieldBindingCore::new(self.clone(), path),
        }
    }

    /// Creates a headless controlled select binding with rendered string conversion.
    ///
    /// Native select events expose rendered option values as strings. This binding keeps the form
    /// draft typed by parsing those rendered values into the field value type, while applications
    /// still own option labels, ordering, disabled state, grouping, styling, and markup.
    pub fn select_with<Value, Parser, ParserError, Formatter>(
        &self,
        path: FieldPath<Model, Value>,
        parser: Parser,
        formatter: Formatter,
    ) -> RenderedSelectBinding<Model, Value, Error>
    where
        Value: 'static,
        Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
        ParserError: fmt::Display + 'static,
        Formatter: Fn(&Value) -> String + 'static,
    {
        let parser = Rc::new(move |value: &str| parser(value).map_err(|error| error.to_string()));

        RenderedSelectBinding {
            base: FieldBindingCore::new(self.clone(), path),
            parser,
            formatter: Rc::new(formatter),
        }
    }

    /// Creates a headless controlled radio group binding for a typed field.
    pub fn radio_group<Value>(
        &self,
        path: FieldPath<Model, Value>,
    ) -> RadioGroupBinding<Model, Value, Error> {
        RadioGroupBinding {
            base: FieldBindingCore::new(self.clone(), path),
        }
    }

    /// Creates a controlled text binding that parses rendered input into a typed field.
    ///
    /// In Dioxus components, prefer [`use_parsed_text`] so the parse binding and its
    /// mounted parse blocker remain stable across rerenders.
    pub fn parsed_text<Value>(
        &self,
        path: FieldPath<Model, Value>,
    ) -> ParsedTextBinding<Model, Value, Error>
    where
        Value: FromStr + fmt::Display + 'static,
        Value::Err: fmt::Display + 'static,
    {
        self.parsed_text_with(
            path,
            |value| value.parse::<Value>(),
            |value| value.to_string(),
        )
    }

    /// Creates a controlled text binding with explicit parser and formatter behavior.
    ///
    /// This is intended for custom field values that should remain typed in the form draft but need
    /// application-supplied rendered text conversion.
    pub fn parsed_text_with<Value, Parser, ParserError, Formatter>(
        &self,
        path: FieldPath<Model, Value>,
        parser: Parser,
        formatter: Formatter,
    ) -> ParsedTextBinding<Model, Value, Error>
    where
        Value: 'static,
        Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
        ParserError: fmt::Display + 'static,
        Formatter: Fn(&Value) -> String + 'static,
    {
        let registration = self.register_parse_binding(path.identity());
        let parser = Rc::new(move |value: &str| parser(value).map_err(|error| error.to_string()));

        ParsedTextBinding {
            base: FieldBindingCore::new(self.clone(), path),
            registration,
            parser,
            formatter: Rc::new(formatter),
        }
    }

    /// Creates a numeric input binding backed by parsed text state.
    ///
    /// This helper intentionally only parses the rendered value into the typed numeric field. Range,
    /// step, precision, and business validation belong in field or form validators.
    pub fn number<Value>(
        &self,
        path: FieldPath<Model, Value>,
    ) -> ParsedTextBinding<Model, Value, Error>
    where
        Value: NumericInputValue,
        Value::Err: fmt::Display + 'static,
    {
        self.parsed_text(path)
    }

    /// Creates a numeric input binding with explicit parser and formatter behavior.
    ///
    /// Use this for application-specific numeric semantics such as optional numeric fields where
    /// empty input should parse as `None`.
    pub fn number_with<Value, Parser, ParserError, Formatter>(
        &self,
        path: FieldPath<Model, Value>,
        parser: Parser,
        formatter: Formatter,
    ) -> ParsedTextBinding<Model, Value, Error>
    where
        Value: 'static,
        Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
        ParserError: fmt::Display + 'static,
        Formatter: Fn(&Value) -> String + 'static,
    {
        self.parsed_text_with(path, parser, formatter)
    }

    /// Creates a date-oriented input binding for values that implement [`FromStr`] and [`fmt::Display`].
    ///
    /// This helper intentionally only parses the rendered date-like value into the typed field.
    /// Date type choice, browser-rendered format, timezone, localization, calendar rules, and date
    /// relationship validation remain application-owned.
    pub fn date<Value>(
        &self,
        path: FieldPath<Model, Value>,
    ) -> ParsedTextBinding<Model, Value, Error>
    where
        Value: FromStr + fmt::Display + 'static,
        Value::Err: fmt::Display + 'static,
    {
        self.parsed_text(path)
    }

    /// Creates a date-oriented input binding with explicit parser and formatter behavior.
    ///
    /// This helper intentionally only parses the rendered date-like value into the typed field.
    /// Date type choice, browser-rendered format, timezone, localization, calendar rules, and date
    /// relationship validation remain application-owned.
    pub fn date_with<Value, Parser, ParserError, Formatter>(
        &self,
        path: FieldPath<Model, Value>,
        parser: Parser,
        formatter: Formatter,
    ) -> ParsedTextBinding<Model, Value, Error>
    where
        Value: 'static,
        Parser: Fn(&str) -> Result<Value, ParserError> + 'static,
        ParserError: fmt::Display + 'static,
        Formatter: Fn(&Value) -> String + 'static,
    {
        self.parsed_text_with(path, parser, formatter)
    }
}

impl<Model, Error> FileSelectionBinding<Model, Error> {
    /// Returns the rendered input name for this file selection.
    pub fn name(&self) -> &str {
        self.key.field_name()
    }

    /// Returns the internal identity for this file selection.
    pub fn identity(&self) -> FieldIdentity {
        self.key.identity()
    }

    /// Returns the file selection cardinality policy for this binding.
    pub const fn cardinality(&self) -> FileSelectionCardinality {
        self.key.cardinality()
    }

    /// Returns whether this binding represents a multi-file field.
    pub const fn allows_multiple(&self) -> bool {
        self.key.allows_multiple()
    }

    /// Starts configuring a synchronous validator for this file selection.
    pub fn validator<Source>(
        &self,
        source: Source,
    ) -> SyncFileSelectionValidatorBuilder<Model, Error>
    where
        Source: Into<ValidatorSource>,
    {
        SyncFileSelectionValidatorBuilder {
            handle: self.handle.clone(),
            key: self.key.clone(),
            source: source.into(),
            triggers: ValidationTriggers::all(),
        }
    }

    /// Starts configuring an asynchronous validator for this file selection.
    pub fn async_validator<Source>(
        &self,
        source: Source,
    ) -> AsyncFileSelectionValidatorBuilder<Model, Error>
    where
        Source: Into<ValidatorSource>,
    {
        AsyncFileSelectionValidatorBuilder {
            handle: self.handle.clone(),
            key: self.key.clone(),
            source: source.into(),
            triggers: ValidationTriggers::all(),
            debounce: None,
        }
    }

    /// Returns headless accessibility IDs and ARIA state for this file selection.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.handle
            .field_accessibility_by_identity(self.key.identity(), self.key.field_name())
    }

    /// Replaces the current selected files for this file selection.
    ///
    /// Single-file bindings retain at most the first selected file. Multi-file bindings retain all
    /// selected files in iterator order.
    pub fn select_files<Files, File>(&self, files: Files)
    where
        Files: IntoIterator<Item = File>,
        File: Into<SelectedFile>,
    {
        let field = self.key.identity();
        let field_name = self.key.field_name().to_owned();
        let files = self.key.normalize_selection(files);

        self.handle.adapter.set_file_selection(field.clone(), files);
        let validates_on_change = self.handle.write_core(|core| {
            core.record_field_identity_user_change(&field);
            core.validation_mode()
                .should_validate_on_change(core.submit_attempt_count())
        });

        self.handle.apply_field_mutation(
            FieldMutation {
                field: field.clone(),
                field_name,
                selectors: vec![
                    SelectorTransition::FieldValueChanged(field.clone()),
                    SelectorTransition::FieldMetadataChanged(field),
                ],
                trigger: ValidationTrigger::Change,
                dispatch: FieldMutationDispatch::ValueReplacement(FieldUpdateOrigin::User),
            },
            validates_on_change,
        );
    }

    /// Replaces the selected files from a Dioxus file-input event.
    pub fn on_change(&self, event: Event<FormData>) {
        self.select_files(event.data().files());
    }

    /// Clears the current selected-file metadata for this file selection.
    pub fn clear(&self) {
        self.select_files(Vec::<SelectedFile>::new());
    }

    /// Marks this file selection as blurred by user interaction.
    pub fn on_blur(&self) {
        let field = self.key.identity();
        let field_name = self.key.field_name().to_owned();

        let validates_on_blur = self.handle.write_core(|core| {
            let validates_on_blur = core
                .validation_mode()
                .should_validate_on_blur(core.submit_attempt_count());
            core.mark_field_identity_blurred(&field);
            validates_on_blur
        });
        self.handle
            .notify_selectors(SelectorTransition::FieldMetadataChanged(field.clone()));

        if validates_on_blur {
            self.handle
                .start_runtime_async_field_validators(field.clone(), ValidationTrigger::Blur);
            self.handle
                .start_runtime_async_form_validators(ValidationTrigger::Blur);
            self.handle.notify_validation_changed();
        }

        self.handle
            .dispatch_form_blur_listeners(field.clone(), field_name);
        self.handle.dispatch_field_blur_listeners(field);
    }

    /// Returns the selected-file metadata for this file selection.
    pub fn selected_files(&self) -> Vec<SelectedFile> {
        let field = self.key.identity();

        self.handle.reactivity.track_field_value(&field);
        self.key
            .normalize_selection(self.handle.adapter.file_selection(field))
    }

    /// Returns tracked user interaction metadata for this file selection.
    pub fn metadata(&self) -> FieldMetadata {
        let field = self.key.identity();

        self.handle.reactivity.track_field_metadata(&field);
        self.handle.core.borrow().field_metadata_by_identity(&field)
    }

    /// Returns whether this file selection has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.metadata().is_touched()
    }

    /// Returns whether this file selection has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.metadata().is_blurred()
    }

    /// Returns validation errors attached to this file selection.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.handle
            .field_validation_errors_by_identity(&self.key.identity())
    }

    /// Returns currently visible validation errors attached to this file selection.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.handle
            .visible_field_validation_errors_by_identity(&self.key.identity())
    }
}

/// Controlled text input behavior for a `String` child field inside a collection item.
pub struct CollectionTextBinding<Model, Item, Error = String> {
    base: CollectionFieldBindingCore<Model, Item, String, Error>,
}

impl<Model, Item, Error> Clone for CollectionTextBinding<Model, Item, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
        }
    }
}

impl<Model, Item, Error> CollectionTextBinding<Model, Item, Error> {
    /// Returns the rendered input name derived from current collection order.
    pub fn name(&self) -> String {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this input.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this item child field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this item child field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this item child field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the current controlled input value.
    pub fn value(&self) -> String {
        self.base.value().unwrap_or_default()
    }

    /// Replaces the controlled input value.
    pub fn set_value(&self, value: impl Into<String>) {
        self.base.set_programmatic(value.into());
    }

    /// Applies a Dioxus text-like `oninput` value update.
    pub fn on_input(&self, value: impl Into<String>) {
        self.base.set_user(value.into());
    }

    /// Applies a Dioxus text-like `onblur` interaction update.
    pub fn on_blur(&self) {
        self.base.blur();
    }

    /// Returns validation errors for this item child field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this item child field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }

    /// Returns a ready-made `oninput` handler for this item child field. See
    /// [`TextBinding::oninput`] for the ergonomics this enables.
    pub fn oninput(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Item: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_input(event.value())
    }

    /// Returns a ready-made `onblur` handler for this item child field.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Item: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }
}

/// Controlled checkbox behavior for a `bool` child field inside a collection item.
pub struct CollectionCheckboxBinding<Model, Item, Error = String> {
    base: CollectionFieldBindingCore<Model, Item, bool, Error>,
}

impl<Model, Item, Error> Clone for CollectionCheckboxBinding<Model, Item, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
        }
    }
}

impl<Model, Item, Error> CollectionCheckboxBinding<Model, Item, Error> {
    /// Returns the rendered checkbox name derived from current collection order.
    pub fn name(&self) -> String {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this checkbox.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this item child field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this item child field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this item child field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the current controlled checkbox checked state.
    pub fn checked(&self) -> bool {
        self.base.value().unwrap_or(false)
    }

    /// Replaces the controlled checkbox checked state.
    pub fn set_checked(&self, checked: bool) {
        self.base.set_programmatic(checked);
    }

    /// Applies a Dioxus checkbox `onchange` checked-state update.
    pub fn on_change(&self, checked: bool) {
        self.base.set_user(checked);
    }

    /// Applies a Dioxus checkbox `onblur` interaction update.
    pub fn on_blur(&self) {
        self.base.blur();
    }

    /// Returns validation errors for this item child field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this item child field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }

    /// Returns a ready-made checkbox change handler that reads `checked` from the event.
    ///
    /// Wire it to `oninput`/`onchange`. The handler owns its own clone, so the binding stays usable
    /// for `checked()`/`name()`.
    pub fn onchange(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Item: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_change(event.checked())
    }

    /// Returns a ready-made `onblur` handler for this item child field.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Item: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }
}

/// Headless controlled select behavior for a typed child field inside a collection item.
pub struct CollectionSelectBinding<Model, Item, Value, Error = String> {
    base: CollectionFieldBindingCore<Model, Item, Value, Error>,
}

impl<Model, Item, Value, Error> Clone for CollectionSelectBinding<Model, Item, Value, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
        }
    }
}

impl<Model, Item, Value, Error> CollectionSelectBinding<Model, Item, Value, Error> {
    /// Returns the rendered select name derived from current collection order.
    pub fn name(&self) -> String {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this select.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this item child field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this item child field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this item child field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the current controlled select value.
    pub fn value(&self) -> Value
    where
        Value: Clone,
    {
        self.base.expect_value()
    }

    /// Returns whether an option value should be rendered as selected.
    pub fn is_selected(&self, value: &Value) -> bool
    where
        Value: PartialEq,
    {
        self.base.is_current(value)
    }

    /// Replaces the controlled select value.
    pub fn set_value(&self, value: Value) {
        self.base.set_programmatic(value);
    }

    /// Applies a committed user select choice.
    pub fn on_change(&self, value: Value) {
        self.base.set_user(value);
    }

    /// Applies a committed user select choice.
    pub fn select(&self, value: Value) {
        self.on_change(value);
    }

    /// Applies a Dioxus select `onblur` interaction update.
    pub fn on_blur(&self) {
        self.base.blur();
    }

    /// Returns validation errors for this item child field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this item child field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }
}

impl<Model, Item, Error> CollectionSelectBinding<Model, Item, String, Error> {
    /// Returns a ready-made `onchange` handler for a native `String`-valued row `<select>`.
    ///
    /// The handler owns its own clone, so the binding stays usable for `value()`/`is_selected(...)`.
    /// Typed (non-`String`) row selects continue to use `on_change`/`select`.
    pub fn onchange(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Item: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_change(event.value())
    }

    /// Returns a ready-made `onblur` handler for this row select.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Item: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }
}

/// Headless controlled select behavior for a typed collection item child field rendered through
/// string option values.
pub struct CollectionRenderedSelectBinding<Model, Item, Value, Error = String> {
    base: CollectionFieldBindingCore<Model, Item, Value, Error>,
    parser: Rc<TextParserFn<Value>>,
    formatter: Rc<TextFormatterFn<Value>>,
}

impl<Model, Item, Value, Error> Clone
    for CollectionRenderedSelectBinding<Model, Item, Value, Error>
{
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            parser: Rc::clone(&self.parser),
            formatter: Rc::clone(&self.formatter),
        }
    }
}

impl<Model, Item, Value, Error> CollectionRenderedSelectBinding<Model, Item, Value, Error> {
    /// Returns the rendered select name derived from current collection order.
    pub fn name(&self) -> String {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this select.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this item child field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this item child field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this item child field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the current controlled select value as the rendered option value.
    pub fn value(&self) -> String {
        self.base
            .read_value(|value| (self.formatter)(value), String::new())
    }

    /// Returns the current typed field value.
    pub fn typed_value(&self) -> Value
    where
        Value: Clone,
    {
        self.base.expect_value()
    }

    /// Returns whether an option value should be rendered as selected.
    pub fn is_selected(&self, value: &Value) -> bool
    where
        Value: PartialEq,
    {
        self.base.is_current(value)
    }

    /// Returns whether a rendered option value should be rendered as selected.
    pub fn is_rendered_selected(&self, rendered_value: &str) -> bool {
        self.value() == rendered_value
    }

    /// Replaces the controlled select value programmatically.
    pub fn set_value(&self, value: Value) {
        self.base.set_programmatic(value);
    }

    /// Applies a committed user select choice as a typed value.
    pub fn select(&self, value: Value) {
        self.base.set_user(value);
    }

    /// Applies a committed user select choice from its rendered string value.
    ///
    /// Invalid rendered values do not mutate the typed draft. Use [`Self::try_on_change`] when the
    /// application wants to observe conversion failures.
    pub fn on_change(&self, value: impl AsRef<str>) {
        let _ = self.try_on_change(value);
    }

    /// Tries to apply a committed user select choice from its rendered string value.
    ///
    /// Select options are application-owned, so conversion failures are returned to the caller
    /// instead of registering adapter parse blockers.
    pub fn try_on_change(&self, value: impl AsRef<str>) -> Result<(), String> {
        match (self.parser)(value.as_ref()) {
            Ok(value) => {
                self.base.set_user(value);
                Ok(())
            }
            Err(error) => {
                self.base.mark_touched();
                Err(error)
            }
        }
    }

    /// Applies a Dioxus select `onblur` interaction update.
    pub fn on_blur(&self) {
        self.base.blur();
    }

    /// Returns validation errors for this item child field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this item child field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }

    /// Returns a ready-made `onchange` handler that parses the selected rendered option value.
    ///
    /// The handler owns its own clone, so the binding stays usable for
    /// `value()`/`is_rendered_selected(...)`.
    pub fn onchange(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Item: 'static,
        Value: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_change(event.value())
    }

    /// Returns a ready-made `onblur` handler for this row select.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Item: 'static,
        Value: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }
}

/// Headless controlled radio group behavior for a typed child field inside a collection item.
pub struct CollectionRadioGroupBinding<Model, Item, Value, Error = String> {
    base: CollectionFieldBindingCore<Model, Item, Value, Error>,
}

impl<Model, Item, Value, Error> Clone for CollectionRadioGroupBinding<Model, Item, Value, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
        }
    }
}

impl<Model, Item, Value, Error> CollectionRadioGroupBinding<Model, Item, Value, Error> {
    /// Returns the rendered radio group name derived from current collection order.
    pub fn name(&self) -> String {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this radio group.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this item child field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this item child field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this item child field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the current controlled radio group value.
    pub fn value(&self) -> Value
    where
        Value: Clone,
    {
        self.base.expect_value()
    }

    /// Returns whether an option value should be rendered as checked or selected.
    pub fn is_selected(&self, value: &Value) -> bool
    where
        Value: PartialEq,
    {
        self.base.is_current(value)
    }

    /// Replaces the controlled radio group value.
    pub fn set_value(&self, value: Value) {
        self.base.set_programmatic(value);
    }

    /// Applies a committed user radio choice.
    pub fn select(&self, value: Value) {
        self.base.set_user(value);
    }

    /// Applies a committed user radio choice.
    pub fn on_change(&self, value: Value) {
        self.select(value);
    }

    /// Applies a Dioxus radio group `onblur` interaction update.
    pub fn on_blur(&self) {
        self.base.blur();
    }

    /// Returns validation errors for this item child field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this item child field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }

    /// Returns a ready-made handler that selects `value` when the option's control fires.
    ///
    /// Wire it per option, e.g. `onclick: role.onselect(value)`. The handler owns its own clone,
    /// so one binding serves every option's reads and handler without a per-iteration `clone()`.
    pub fn onselect<Data>(&self, value: Value) -> impl FnMut(Event<Data>) + 'static
    where
        Data: ?Sized + 'static,
        Model: 'static,
        Item: 'static,
        Value: Clone + 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<Data>| binding.select(value.clone())
    }

    /// Returns a ready-made `onblur` handler for this row radio group.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Item: 'static,
        Value: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }
}

/// Controlled text input behavior for a collection item child field parsed from rendered text.
pub struct CollectionParsedTextBinding<Model, Item, Value, Error = String> {
    base: CollectionFieldBindingCore<Model, Item, Value, Error>,
    registration: ParseBindingRegistration,
    parser: Rc<TextParserFn<Value>>,
    formatter: Rc<TextFormatterFn<Value>>,
}

impl<Model, Item, Value, Error> Clone for CollectionParsedTextBinding<Model, Item, Value, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            registration: self.registration.clone(),
            parser: Rc::clone(&self.parser),
            formatter: Rc::clone(&self.formatter),
        }
    }
}

impl<Model, Item, Value, Error> CollectionParsedTextBinding<Model, Item, Value, Error> {
    /// Returns the rendered input name derived from current collection order.
    pub fn name(&self) -> String {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this input.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this item child field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this item child field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this item child field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the controlled rendered value, preferring raw input while parsing is failing.
    pub fn value(&self) -> String {
        parsed_input::value(&self.base, &self.registration, &self.formatter)
    }

    /// Replaces the typed field value programmatically and clears binding parse state.
    pub fn set_value(&self, value: Value) {
        parsed_input::set_value(&self.base, &self.registration, value);
    }

    /// Applies a Dioxus text-like `oninput` value update by parsing rendered input.
    pub fn on_input(&self, value: impl Into<String>) {
        parsed_input::on_input(&self.base, &self.registration, &self.parser, value);
    }

    /// Applies a Dioxus text-like `onblur` interaction update.
    pub fn on_blur(&self) {
        parsed_input::on_blur(&self.base, &self.registration);
    }

    /// Returns this mounted binding's parse error, if rendered input is currently unparsable.
    pub fn parse_error(&self) -> Option<ParseError> {
        parsed_input::parse_error(&self.registration)
    }

    /// Returns validation errors for this item child field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this item child field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }

    /// Returns a ready-made `oninput` handler that parses rendered input for this row field.
    ///
    /// The handler owns its own clone, so the binding stays usable for `value()`/`parse_error()`.
    pub fn oninput(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Item: 'static,
        Value: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_input(event.value())
    }

    /// Returns a ready-made `onblur` handler for this row parsed field.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Item: 'static,
        Value: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }
}

/// Headless controlled select behavior for a typed field rendered through string option values.
pub struct RenderedSelectBinding<Model, Value, Error = String> {
    base: FieldBindingCore<Model, Value, Error>,
    parser: Rc<TextParserFn<Value>>,
    formatter: Rc<TextFormatterFn<Value>>,
}

impl<Model, Value, Error> Clone for RenderedSelectBinding<Model, Value, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            parser: Rc::clone(&self.parser),
            formatter: Rc::clone(&self.formatter),
        }
    }
}

impl<Model, Value, Error> RenderedSelectBinding<Model, Value, Error> {
    /// Returns the rendered select name derived from the typed field path.
    pub fn name(&self) -> &str {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this select.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the current controlled select value as the rendered option value.
    pub fn value(&self) -> String {
        self.base.read_value(|value| (self.formatter)(value))
    }

    /// Returns the current typed field value.
    pub fn typed_value(&self) -> Value
    where
        Value: Clone,
    {
        self.base.value()
    }

    /// Returns whether an option value should be rendered as selected.
    pub fn is_selected(&self, value: &Value) -> bool
    where
        Value: PartialEq,
    {
        controlled_choice::is_selected(&self.base, value)
    }

    /// Returns whether a rendered option value should be rendered as selected.
    pub fn is_rendered_selected(&self, rendered_value: &str) -> bool {
        self.value() == rendered_value
    }

    /// Replaces the controlled select value programmatically.
    pub fn set_value(&self, value: Value) {
        controlled_choice::set_value(&self.base, value);
    }

    /// Applies a committed user select choice as a typed value.
    pub fn select(&self, value: Value) {
        controlled_choice::select(&self.base, value);
    }

    /// Applies a committed user select choice from its rendered string value.
    ///
    /// Invalid rendered values do not mutate the typed draft. Use [`Self::try_on_change`] when the
    /// application wants to observe conversion failures.
    pub fn on_change(&self, value: impl AsRef<str>) {
        let _ = self.try_on_change(value);
    }

    /// Tries to apply a committed user select choice from its rendered string value.
    ///
    /// Select options are application-owned, so conversion failures are returned to the caller
    /// instead of registering adapter parse blockers.
    pub fn try_on_change(&self, value: impl AsRef<str>) -> Result<(), String> {
        match (self.parser)(value.as_ref()) {
            Ok(value) => {
                controlled_choice::select(&self.base, value);
                Ok(())
            }
            Err(error) => {
                self.base.mark_touched();
                Err(error)
            }
        }
    }

    /// Applies a Dioxus select `onblur` interaction update.
    pub fn on_blur(&self) {
        self.base.blur();
    }

    /// Returns validation errors for this field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }

    /// Returns a ready-made `onchange` handler that parses the selected rendered option value.
    ///
    /// The handler owns its own clone, so `onchange: field.onchange()` needs no separate
    /// `field.clone()` and the binding stays usable for `value()`/`is_rendered_selected(...)`.
    pub fn onchange(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Value: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_change(event.value())
    }

    /// Returns a ready-made `onblur` handler for this select.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Value: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }
}

/// Controlled text input behavior for a typed `String` field.
pub struct TextBinding<Model, Error = String> {
    base: FieldBindingCore<Model, String, Error>,
}

impl<Model, Error> Clone for TextBinding<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
        }
    }
}

impl<Model, Error> TextBinding<Model, Error> {
    /// Returns the rendered input name derived from the typed field path.
    pub fn name(&self) -> &str {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this input.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the current controlled input value.
    pub fn value(&self) -> String {
        self.base.value()
    }

    /// Replaces the controlled input value.
    pub fn set_value(&self, value: impl Into<String>) {
        self.base.set_programmatic(value.into());
    }

    /// Applies a Dioxus text-like `oninput` value update.
    pub fn on_input(&self, value: impl Into<String>) {
        self.base.set_user(value.into());
    }

    /// Applies a Dioxus text-like `onblur` interaction update.
    pub fn on_blur(&self) {
        self.base.blur();
    }

    /// Returns a ready-made `oninput` event handler for this field.
    ///
    /// The handler owns its own clone of the binding, so the binding stays usable afterwards for
    /// `name()`, `value()`, and other reads. This removes the `let field_oninput = field.clone();`
    /// step: wire `oninput: field.oninput()` directly and keep using `field` in the same `rsx!`.
    pub fn oninput(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_input(event.value())
    }

    /// Returns a ready-made `onblur` event handler for this field.
    ///
    /// Like [`oninput`](Self::oninput), the handler owns its own clone, so `onblur: field.onblur()`
    /// needs no separate `field.clone()`.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }

    /// Returns validation errors for this field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }
}

/// Controlled textarea behavior for a typed `String` field.
pub struct TextareaBinding<Model, Error = String> {
    base: FieldBindingCore<Model, String, Error>,
}

impl<Model, Error> Clone for TextareaBinding<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
        }
    }
}

impl<Model, Error> TextareaBinding<Model, Error> {
    /// Returns the rendered textarea name derived from the typed field path.
    pub fn name(&self) -> &str {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this textarea.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the current controlled textarea value.
    pub fn value(&self) -> String {
        self.base.value()
    }

    /// Replaces the controlled textarea value.
    pub fn set_value(&self, value: impl Into<String>) {
        self.base.set_programmatic(value.into());
    }

    /// Applies a Dioxus textarea `oninput` value update.
    pub fn on_input(&self, value: impl Into<String>) {
        self.base.set_user(value.into());
    }

    /// Applies a Dioxus textarea `onblur` interaction update.
    pub fn on_blur(&self) {
        self.base.blur();
    }

    /// Returns a ready-made `oninput` event handler for this textarea. See
    /// [`TextBinding::oninput`] for the ergonomics this enables.
    pub fn oninput(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_input(event.value())
    }

    /// Returns a ready-made `onblur` event handler for this textarea.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }

    /// Returns validation errors for this field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }
}

/// Controlled checkbox behavior for a typed `bool` field.
pub struct CheckboxBinding<Model, Error = String> {
    base: FieldBindingCore<Model, bool, Error>,
}

impl<Model, Error> Clone for CheckboxBinding<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
        }
    }
}

impl<Model, Error> CheckboxBinding<Model, Error> {
    /// Returns the rendered checkbox name derived from the typed field path.
    pub fn name(&self) -> &str {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this checkbox.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the current controlled checkbox checked state.
    pub fn checked(&self) -> bool {
        self.base.value()
    }

    /// Replaces the controlled checkbox checked state.
    pub fn set_checked(&self, checked: bool) {
        self.base.set_programmatic(checked);
    }

    /// Applies a Dioxus checkbox `onchange` checked-state update.
    pub fn on_change(&self, checked: bool) {
        self.base.set_user(checked);
    }

    /// Applies a Dioxus checkbox `onblur` interaction update.
    pub fn on_blur(&self) {
        self.base.blur();
    }

    /// Returns a ready-made checkbox change handler that reads `checked` from the event.
    ///
    /// Wire it to the element's `oninput` (or `onchange`): `oninput: subscribe.onchange()`. The
    /// handler owns its own clone, so the binding stays usable for `checked()`/`name()`.
    pub fn onchange(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_change(event.checked())
    }

    /// Returns a ready-made `onblur` event handler for this checkbox.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }

    /// Returns validation errors for this field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }
}

/// Headless controlled select behavior for a typed field.
pub struct SelectBinding<Model, Value, Error = String> {
    base: FieldBindingCore<Model, Value, Error>,
}

impl<Model, Value, Error> Clone for SelectBinding<Model, Value, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
        }
    }
}

impl<Model, Value, Error> SelectBinding<Model, Value, Error> {
    /// Returns the rendered select name derived from the typed field path.
    pub fn name(&self) -> &str {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this select.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the current controlled select value.
    pub fn value(&self) -> Value
    where
        Value: Clone,
    {
        self.base.value()
    }

    /// Returns whether an option value should be rendered as selected.
    pub fn is_selected(&self, value: &Value) -> bool
    where
        Value: PartialEq,
    {
        controlled_choice::is_selected(&self.base, value)
    }

    /// Replaces the controlled select value.
    pub fn set_value(&self, value: Value) {
        controlled_choice::set_value(&self.base, value);
    }

    /// Applies a committed user select choice.
    pub fn on_change(&self, value: Value) {
        controlled_choice::select(&self.base, value);
    }

    /// Applies a committed user select choice.
    pub fn select(&self, value: Value) {
        self.on_change(value);
    }

    /// Applies a Dioxus select `onblur` interaction update.
    pub fn on_blur(&self) {
        self.base.blur();
    }

    /// Returns validation errors for this field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }
}

impl<Model, Error> SelectBinding<Model, String, Error> {
    /// Returns a ready-made `onchange` handler for a native `String`-valued `<select>`.
    ///
    /// Native selects emit their chosen option as a string, so this is the common case. The handler
    /// owns its own clone, so `onchange: role.onchange()` leaves `role` usable for
    /// `value()`/`is_selected(...)` in the same `rsx!`. Typed (non-`String`) selects continue to use
    /// `on_change`/`select` explicitly, or `select_with` for parsed rendering.
    pub fn onchange(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_change(event.value())
    }

    /// Returns a ready-made `onblur` handler for this select.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }
}

/// Headless controlled radio group behavior for a typed field.
pub struct RadioGroupBinding<Model, Value, Error = String> {
    base: FieldBindingCore<Model, Value, Error>,
}

impl<Model, Value, Error> Clone for RadioGroupBinding<Model, Value, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
        }
    }
}

impl<Model, Value, Error> RadioGroupBinding<Model, Value, Error> {
    /// Returns the rendered radio group name derived from the typed field path.
    pub fn name(&self) -> &str {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this radio group.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the current controlled radio group value.
    pub fn value(&self) -> Value
    where
        Value: Clone,
    {
        self.base.value()
    }

    /// Returns whether an option value should be rendered as checked or selected.
    pub fn is_selected(&self, value: &Value) -> bool
    where
        Value: PartialEq,
    {
        controlled_choice::is_selected(&self.base, value)
    }

    /// Replaces the controlled radio group value.
    pub fn set_value(&self, value: Value) {
        controlled_choice::set_value(&self.base, value);
    }

    /// Applies a committed user radio choice.
    pub fn select(&self, value: Value) {
        controlled_choice::select(&self.base, value);
    }

    /// Applies a committed user radio choice.
    pub fn on_change(&self, value: Value) {
        self.select(value);
    }

    /// Applies a Dioxus radio group `onblur` interaction update.
    pub fn on_blur(&self) {
        self.base.blur();
    }

    /// Returns validation errors for this field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }

    /// Returns a ready-made handler that selects `value` when the option's control fires.
    ///
    /// Radio options are rendered one per value, so wire this per option, e.g.
    /// `onclick: plan.onselect(value)`. The handler owns its own clone, so a single `plan` binding
    /// serves every option's `name()`/`is_selected(...)` reads and handler without a per-iteration
    /// `plan.clone()`. The event is ignored, so it works on `onclick`, `onchange`, or `oninput`.
    pub fn onselect<Data>(&self, value: Value) -> impl FnMut(Event<Data>) + 'static
    where
        Data: ?Sized + 'static,
        Model: 'static,
        Value: Clone + 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<Data>| binding.select(value.clone())
    }

    /// Returns a ready-made `onblur` handler for this radio group.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Value: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }
}

/// Controlled text input behavior for a field parsed from rendered text.
pub struct ParsedTextBinding<Model, Value, Error = String> {
    base: FieldBindingCore<Model, Value, Error>,
    registration: ParseBindingRegistration,
    parser: Rc<TextParserFn<Value>>,
    formatter: Rc<TextFormatterFn<Value>>,
}

impl<Model, Value, Error> Clone for ParsedTextBinding<Model, Value, Error> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            registration: self.registration.clone(),
            parser: Rc::clone(&self.parser),
            formatter: Rc::clone(&self.formatter),
        }
    }
}

impl<Model, Value, Error> ParsedTextBinding<Model, Value, Error> {
    /// Returns the rendered input name derived from the typed field path.
    pub fn name(&self) -> &str {
        self.base.name()
    }

    /// Returns headless accessibility IDs and ARIA state for this input.
    pub fn accessibility(&self) -> FieldAccessibility {
        self.base.accessibility()
    }

    /// Returns tracked user interaction metadata for this field.
    pub fn metadata(&self) -> FieldMetadata {
        self.base.metadata()
    }

    /// Returns whether this field has received user interaction.
    pub fn is_touched(&self) -> bool {
        self.base.is_touched()
    }

    /// Returns whether this field has lost focus at least once.
    pub fn is_blurred(&self) -> bool {
        self.base.is_blurred()
    }

    /// Returns the controlled rendered value, preferring raw input while parsing is failing.
    pub fn value(&self) -> String {
        parsed_input::value(&self.base, &self.registration, &self.formatter)
    }

    /// Replaces the typed field value programmatically and clears binding parse state.
    pub fn set_value(&self, value: Value) {
        parsed_input::set_value(&self.base, &self.registration, value);
    }

    /// Applies a Dioxus text-like `oninput` value update by parsing rendered input.
    pub fn on_input(&self, value: impl Into<String>) {
        parsed_input::on_input(&self.base, &self.registration, &self.parser, value);
    }

    /// Applies a Dioxus text-like `onblur` interaction update.
    pub fn on_blur(&self) {
        parsed_input::on_blur(&self.base, &self.registration);
    }

    /// Returns this mounted binding's parse error, if rendered input is currently unparsable.
    pub fn parse_error(&self) -> Option<ParseError> {
        parsed_input::parse_error(&self.registration)
    }

    /// Returns validation errors for this field.
    pub fn validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.validation_errors()
    }

    /// Returns visible validation errors for this field.
    pub fn visible_validation_errors(&self) -> Vec<ValidationErrorSnapshot<Error>>
    where
        Error: Clone,
    {
        self.base.visible_validation_errors()
    }

    /// Returns a ready-made `oninput` handler that parses rendered input for this field.
    ///
    /// The handler owns its own clone, so `oninput: field.oninput()` needs no separate
    /// `field.clone()` and the binding stays usable for `value()`/`parse_error()`.
    pub fn oninput(&self) -> impl FnMut(Event<FormData>) + 'static
    where
        Model: 'static,
        Value: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |event: Event<FormData>| binding.on_input(event.value())
    }

    /// Returns a ready-made `onblur` handler for this parsed field.
    pub fn onblur(&self) -> impl FnMut(Event<FocusData>) + 'static
    where
        Model: 'static,
        Value: 'static,
        Error: 'static,
    {
        let binding = self.clone();
        move |_event: Event<FocusData>| binding.on_blur()
    }
}

/// Dioxus-managed submit behavior for a form.
pub struct SubmitBinding<Model, Error = String> {
    handle: FormHandle<Model, Error>,
}

/// Browser-owned POST submit attributes for a form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserSubmitBinding {
    action: String,
}

/// The result of a progressive browser submit preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressiveSubmitResult {
    /// The browser submit event was allowed to continue.
    Allowed,
    /// The browser submit event was blocked by a known submit blocker.
    Blocked(SubmitBlocker),
}

/// Progressive browser submit behavior for a form.
pub struct ProgressiveSubmitBinding<Model, Error = String> {
    handle: FormHandle<Model, Error>,
}

/// Progressive browser submit behavior scoped to one explicit submit intent.
pub struct IntentProgressiveSubmitBinding<Model, Intent, Error = String> {
    submit: ProgressiveSubmitBinding<Model, Error>,
    intent: Intent,
}

/// Dioxus-managed submit behavior scoped to one explicit submit intent.
pub struct IntentSubmitBinding<Model, Intent, Error = String> {
    submit: SubmitBinding<Model, Error>,
    intent: Intent,
}

impl<Model, Error> Clone for SubmitBinding<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
        }
    }
}

impl BrowserSubmitBinding {
    /// Returns the HTML form method for browser-owned submission.
    pub const fn method(&self) -> &'static str {
        "post"
    }

    /// Returns the HTML form action for browser-owned submission.
    pub fn action(&self) -> &str {
        &self.action
    }
}

impl<Model, Error> Clone for ProgressiveSubmitBinding<Model, Error> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
        }
    }
}

impl<Model, Intent, Error> Clone for IntentProgressiveSubmitBinding<Model, Intent, Error>
where
    Intent: Clone,
{
    fn clone(&self) -> Self {
        Self {
            submit: self.submit.clone(),
            intent: self.intent.clone(),
        }
    }
}

impl<Model, Intent, Error> Clone for IntentSubmitBinding<Model, Intent, Error>
where
    Intent: Clone,
{
    fn clone(&self) -> Self {
        Self {
            submit: self.submit.clone(),
            intent: self.intent.clone(),
        }
    }
}

fn manage_submit_event<EventData: ?Sized + 'static>(event: &Event<EventData>) {
    event.prevent_default();
    event.stop_propagation();
}

fn block_browser_submit_event<EventData: ?Sized + 'static>(event: &Event<EventData>) {
    event.prevent_default();
}

impl<Model, Error> SubmitBinding<Model, Error> {
    /// Scopes this submit binding to one explicit submit intent.
    pub fn intent<Intent>(&self, intent: Intent) -> IntentSubmitBinding<Model, Intent, Error> {
        IntentSubmitBinding {
            submit: self.clone(),
            intent,
        }
    }
}

impl<Model: Clone, Error> SubmitBinding<Model, Error> {
    /// Applies a Dioxus `onsubmit` event and submits the form synchronously.
    ///
    /// Pass the event Dioxus supplies to a form `onsubmit` handler. This method synchronously calls
    /// [`Event::prevent_default`] and [`Event::stop_propagation`] before running the existing submit
    /// lifecycle. Stopping propagation is intentional: once this binding claims the event as
    /// Dioxus-managed, parent submit handlers should not receive it unexpectedly. Use
    /// [`FormHandle::browser_submit`] or [`FormHandle::progressive_submit`] for browser-owned POST.
    pub fn on_submit<EventData: ?Sized + 'static, Submit, Outcome>(
        &self,
        event: Event<EventData>,
        submit: Submit,
    ) -> SubmitResult
    where
        Submit: FnOnce(SubmissionSnapshot<Model>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        manage_submit_event(&event);
        self.handle.submit(submit)
    }

    /// Applies a Dioxus `onsubmit` event and submits the form synchronously with selected files.
    pub fn on_submit_with_files<EventData: ?Sized + 'static, Submit, Outcome>(
        &self,
        event: Event<EventData>,
        submit: Submit,
    ) -> SubmitResult
    where
        Submit: FnOnce(SubmissionSnapshot<Model>, FileSubmissionSnapshot<Model>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        manage_submit_event(&event);
        self.handle.submit_with_files(submit)
    }

    /// Applies a Dioxus `onsubmit` event and starts a managed async submit lifecycle.
    ///
    /// Pass the event Dioxus supplies to a form `onsubmit` handler. This method synchronously calls
    /// [`Event::prevent_default`] and [`Event::stop_propagation`] before starting the async submit
    /// lifecycle. Stopping propagation is intentional: once this binding claims the event as
    /// Dioxus-managed, parent submit handlers should not receive it unexpectedly. Use
    /// [`FormHandle::browser_submit`] or [`FormHandle::progressive_submit`] for browser-owned POST.
    ///
    /// Returns [`SubmitResult::Started`] when the managed async submit lifecycle was accepted.
    /// Submit-relevant debounced validation is flushed before waiting. If submit-relevant async
    /// validation is pending, the application submit handler starts only after that validation
    /// settles successfully. Structured submit errors are stored when the spawned handler future
    /// completes.
    pub fn on_submit_async<EventData: ?Sized + 'static, Submit, Fut, Outcome>(
        &self,
        event: Event<EventData>,
        submit: Submit,
    ) -> SubmitResult
    where
        Submit: FnOnce(SubmissionSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        manage_submit_event(&event);
        self.handle.submit_async_managed(submit)
    }

    /// Applies a Dioxus `onsubmit` event and starts a file-aware managed async submit lifecycle.
    ///
    /// The submit handler receives both the validated form snapshot and a submit-time snapshot of
    /// selected-file metadata.
    pub fn on_submit_async_with_files<EventData: ?Sized + 'static, Submit, Fut, Outcome>(
        &self,
        event: Event<EventData>,
        submit: Submit,
    ) -> SubmitResult
    where
        Submit: FnOnce(SubmissionSnapshot<Model>, FileSubmissionSnapshot<Model>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        manage_submit_event(&event);
        self.handle.submit_async_managed_with_files(submit)
    }

    /// Returns whether there are no current known submit blockers.
    pub fn can_submit(&self) -> bool {
        self.handle.can_submit()
    }

    /// Returns current UI-oriented submit availability.
    pub fn submit_availability(&self) -> SubmitAvailability {
        self.handle.submit_availability()
    }

    /// Returns the latest meaningful submission outcome, if one has been recorded.
    pub fn last_submit_status(&self) -> Option<SubmitStatus> {
        self.handle.last_submit_status()
    }

    /// Returns the latest meaningful submission outcome with its typed submit intent.
    pub fn last_submit_status_as<Intent>(&self) -> Option<LastSubmitStatus<Intent>>
    where
        Intent: Clone + 'static,
    {
        self.handle.last_submit_status_as()
    }
}

impl<Model, Error> ProgressiveSubmitBinding<Model, Error> {
    /// Scopes this progressive submit binding to one explicit submit intent.
    pub fn intent<Intent>(
        &self,
        intent: Intent,
    ) -> IntentProgressiveSubmitBinding<Model, Intent, Error> {
        IntentProgressiveSubmitBinding {
            submit: self.clone(),
            intent,
        }
    }
}

impl<Model: Clone, Error> ProgressiveSubmitBinding<Model, Error> {
    /// Applies a Dioxus `onsubmit` event and blocks native browser submission only when preflight
    /// finds a current known submit blocker.
    pub fn on_submit<EventData: ?Sized + 'static>(
        &self,
        event: Event<EventData>,
    ) -> ProgressiveSubmitResult {
        self.on_submit_with_intent(event, ())
    }

    fn on_submit_with_intent<EventData, Intent>(
        &self,
        event: Event<EventData>,
        intent: Intent,
    ) -> ProgressiveSubmitResult
    where
        EventData: ?Sized + 'static,
        Intent: Clone + PartialEq + 'static,
    {
        let listener_intent = intent.clone();

        if self.handle.adapter.has_managed_async_submission() {
            let blocker = self
                .handle
                .write_core(|core| core.intent(intent).block_duplicate_submission())
                .expect_blocker();
            self.handle
                .notify_and_dispatch_submit_blocked(blocker, listener_intent);
            block_browser_submit_event(&event);
            return ProgressiveSubmitResult::Blocked(blocker);
        }

        if self.handle.has_parse_blockers() {
            let blocker = self
                .handle
                .write_core(|core| core.intent(intent).block_submission_with_parse_errors())
                .expect_blocker();
            self.handle
                .notify_and_dispatch_submit_blocked(blocker, listener_intent);
            block_browser_submit_event(&event);
            return ProgressiveSubmitResult::Blocked(blocker);
        }

        if let Some(blocker) = self
            .handle
            .write_core(|core| core.intent(intent).validate_for_submit_preflight())
        {
            self.handle
                .notify_selectors(SelectorTransition::ValidationChanged);
            self.handle.dispatch_submit_listeners(
                SubmitListenerEvent::SubmitAttempted,
                listener_intent.clone(),
            );
            self.handle.dispatch_submit_listeners(
                SubmitListenerEvent::SubmitBlocked(blocker),
                listener_intent,
            );
            block_browser_submit_event(&event);
            ProgressiveSubmitResult::Blocked(blocker)
        } else {
            self.handle
                .notify_selectors(SelectorTransition::ValidationChanged);
            ProgressiveSubmitResult::Allowed
        }
    }

    /// Returns whether there are no current known submit blockers.
    pub fn can_submit(&self) -> bool {
        self.handle.can_submit()
    }

    /// Returns current UI-oriented submit availability.
    pub fn submit_availability(&self) -> SubmitAvailability {
        self.handle.submit_availability()
    }
}

impl<Model, Intent, Error> IntentProgressiveSubmitBinding<Model, Intent, Error> {
    /// Returns the underlying progressive submit binding.
    pub const fn submit_binding(&self) -> &ProgressiveSubmitBinding<Model, Error> {
        &self.submit
    }

    /// Returns the submit intent this binding uses.
    pub const fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Returns current UI-oriented submit availability for this submit intent.
    pub fn availability(&self) -> SubmitAvailability
    where
        Intent: PartialEq + 'static,
    {
        self.submit.handle.intent_availability(&self.intent)
    }

    /// Returns whether this submit intent has no current known blockers.
    pub fn can_submit(&self) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.availability().is_available()
    }

    /// Returns the latest outcome when this submit intent produced the latest status.
    pub fn last_status(&self) -> Option<SubmitStatus>
    where
        Intent: PartialEq + 'static,
    {
        self.submit.handle.intent_last_status(&self.intent)
    }
}

impl<Model: Clone, Intent, Error> IntentProgressiveSubmitBinding<Model, Intent, Error> {
    /// Applies a Dioxus `onsubmit` event and blocks browser submission only when this intent's
    /// preflight finds a current known submit blocker.
    pub fn on_submit<EventData: ?Sized + 'static>(
        &self,
        event: Event<EventData>,
    ) -> ProgressiveSubmitResult
    where
        Intent: Clone + PartialEq + 'static,
    {
        self.submit
            .on_submit_with_intent(event, self.intent.clone())
    }
}

impl<Model, Intent, Error> IntentSubmitBinding<Model, Intent, Error> {
    /// Returns the underlying submit binding.
    pub const fn submit_binding(&self) -> &SubmitBinding<Model, Error> {
        &self.submit
    }

    /// Returns the submit intent this binding uses.
    pub const fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Returns current UI-oriented submit availability for this submit intent.
    pub fn availability(&self) -> SubmitAvailability
    where
        Intent: PartialEq + 'static,
    {
        self.submit.handle.intent_availability(&self.intent)
    }

    /// Returns whether this submit intent has no current known blockers.
    pub fn can_submit(&self) -> bool
    where
        Intent: PartialEq + 'static,
    {
        self.availability().is_available()
    }

    /// Returns the latest outcome when this submit intent produced the latest status.
    pub fn last_status(&self) -> Option<SubmitStatus>
    where
        Intent: PartialEq + 'static,
    {
        self.submit.handle.intent_last_status(&self.intent)
    }
}

impl<Model: Clone, Intent, Error> IntentSubmitBinding<Model, Intent, Error> {
    /// Applies a Dioxus `onsubmit` event and submits the form synchronously with this intent.
    pub fn on_submit<EventData: ?Sized + 'static, Submit, Outcome>(
        &self,
        event: Event<EventData>,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        manage_submit_event(&event);
        self.submit
            .handle
            .submit_intent(self.intent.clone(), submit)
    }

    /// Applies a Dioxus `onsubmit` event and submits the form synchronously with this intent and files.
    pub fn on_submit_with_files<EventData: ?Sized + 'static, Submit, Outcome>(
        &self,
        event: Event<EventData>,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>, FileSubmissionSnapshot<Model>) -> Outcome,
        Outcome: Into<SubmitErrors<Model, Error>>,
    {
        manage_submit_event(&event);
        self.submit
            .handle
            .submit_intent_with_files(self.intent.clone(), submit)
    }

    /// Applies a Dioxus `onsubmit` event and starts a managed async submit with this intent.
    pub fn on_submit_async<EventData: ?Sized + 'static, Submit, Fut, Outcome>(
        &self,
        event: Event<EventData>,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>) -> Fut + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        manage_submit_event(&event);
        self.submit
            .handle
            .submit_async_managed_intent(self.intent.clone(), submit)
    }

    /// Applies a Dioxus `onsubmit` event and starts a file-aware managed async submit with this intent.
    pub fn on_submit_async_with_files<EventData: ?Sized + 'static, Submit, Fut, Outcome>(
        &self,
        event: Event<EventData>,
        submit: Submit,
    ) -> SubmitResult
    where
        Intent: Clone + PartialEq + 'static,
        Submit: FnOnce(SubmissionSnapshot<Model, Intent>, FileSubmissionSnapshot<Model>) -> Fut
            + 'static,
        Fut: Future<Output = Outcome> + 'static,
        Outcome: Into<SubmitErrors<Model, Error>> + 'static,
        Model: 'static,
        Error: 'static,
    {
        manage_submit_event(&event);
        self.submit
            .handle
            .submit_async_managed_intent_with_files(self.intent.clone(), submit)
    }
}
