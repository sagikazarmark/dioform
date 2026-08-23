//! A form-library-agnostic field convention for Dioxus.
//!
//! [`Binding`] is the upper-level value binding contract. Widget registries that do not depend on
//! this crate can instead accept separate `value`, `on_change`, and `on_commit` props matching the
//! lower-level contract carried by [`BindingPropTrio`].

use std::{any::Any, rc::Rc};

use dioxus_core::{Callback, provide_context, try_consume_context};
use dioxus_hooks::use_signal;
use dioxus_signals::{ReadSignal, Signal, WritableExt};

/// Describes whether a value write came from user interaction or application code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeOrigin {
    /// The user changed the value through a widget.
    User,
    /// Application code changed the value.
    Programmatic,
}

/// A reactive, two-way binding to one field-shaped value.
///
/// Equality compares the binding's producer-defined identity. Equal bindings are guaranteed to
/// represent the same read and write behavior; producers may conservatively return unequal
/// bindings when they cannot prove that interchangeability.
pub struct Binding<T: 'static> {
    /// The binding's reactive value.
    pub read: ReadSignal<T>,
    write: Callback<(T, ChangeOrigin)>,
    commit: Callback<()>,
    identity: BindingIdentity,
}

impl<T: 'static> Binding<T> {
    /// Creates a binding identified by its exact read, write, and commit handles.
    pub fn new(
        read: ReadSignal<T>,
        write: Callback<(T, ChangeOrigin)>,
        commit: Callback<()>,
    ) -> Self {
        Self::with_identity(read, write, commit, (read, write, commit))
    }

    fn with_identity<I>(
        read: ReadSignal<T>,
        write: Callback<(T, ChangeOrigin)>,
        commit: Callback<()>,
        identity: I,
    ) -> Self
    where
        I: PartialEq + 'static,
    {
        Self {
            read,
            write,
            commit,
            identity: BindingIdentity::new(identity),
        }
    }

    /// Writes a value and preserves where the change originated.
    pub fn write(&self, value: T, origin: ChangeOrigin) {
        self.write.call((value, origin));
    }

    /// Reports the widget-defined end of one interaction unit.
    pub fn commit(&self) {
        self.commit.call(());
    }

    /// Decomposes this binding into the dependency-free widget prop contract.
    ///
    /// The lower-level `on_change` callback has no origin parameter, so its writes are user writes.
    pub fn into_trio(self) -> BindingPropTrio<T> {
        let value = self.read;
        let on_commit = self.commit;
        let on_change = Callback::new(move |value| self.write(value, ChangeOrigin::User));

        BindingPropTrio {
            value,
            on_change,
            on_commit,
        }
    }
}

impl<T: 'static> Clone for Binding<T> {
    fn clone(&self) -> Self {
        Self {
            read: self.read,
            write: self.write,
            commit: self.commit,
            identity: self.identity.clone(),
        }
    }
}

impl<T: 'static> PartialEq for Binding<T> {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl<T: 'static> From<Signal<T>> for Binding<T> {
    fn from(signal: Signal<T>) -> Self {
        let read = ReadSignal::from(signal);
        let mut writer = signal;
        let write = Callback::new(move |(value, _origin)| writer.set(value));
        let commit = Callback::new(|()| {});

        Self::with_identity(read, write, commit, signal)
    }
}

impl<T: 'static> From<(ReadSignal<T>, Callback<T>)> for Binding<T> {
    fn from((read, on_change): (ReadSignal<T>, Callback<T>)) -> Self {
        let write = Callback::new(move |(value, _origin)| on_change.call(value));
        let commit = Callback::new(|()| {});

        Self::with_identity(read, write, commit, (read, on_change))
    }
}

impl<T: 'static> From<T> for Binding<T> {
    fn from(value: T) -> Self {
        Signal::new(value).into()
    }
}

/// A carrier for the lower-level prop contract implemented by field-shaped widgets.
///
/// Decompose this carrier into three separate props to keep a widget independent from this crate.
/// Since `on_change` does not carry a [`ChangeOrigin`], calling it represents a user change.
pub struct BindingPropTrio<T: 'static> {
    /// The reactive value read by the widget.
    pub value: ReadSignal<T>,
    /// The callback invoked when user interaction changes the value.
    pub on_change: Callback<T>,
    /// The callback invoked at the widget-defined end of an interaction unit.
    pub on_commit: Callback<()>,
}

impl<T: 'static> From<Binding<T>> for BindingPropTrio<T> {
    fn from(binding: Binding<T>) -> Self {
        binding.into_trio()
    }
}

/// Type-erased context for one field's binding.
///
/// The context itself is intentionally not generic. This lets [`use_binding`] distinguish an
/// absent context from a present context containing the wrong value type.
#[derive(Clone)]
pub struct FieldContext {
    binding: Rc<dyn Any>,
    value_type_name: &'static str,
}

impl FieldContext {
    /// Creates context for a binding.
    pub fn new<T: 'static>(binding: Binding<T>) -> Self {
        Self {
            binding: Rc::new(binding),
            value_type_name: std::any::type_name::<T>(),
        }
    }

    /// Resolves the context binding for `T`.
    ///
    /// # Panics
    ///
    /// Panics when the field context contains a binding for a different value type.
    pub fn resolve<T: 'static>(&self) -> Binding<T> {
        self.binding
            .downcast_ref::<Binding<T>>()
            .unwrap_or_else(|| {
                panic!(
                    "Field Context contains a binding for {}, but a binding for {} was requested",
                    self.value_type_name,
                    std::any::type_name::<T>()
                )
            })
            .clone()
    }
}

/// Provides a binding as the current scope's [`FieldContext`].
pub fn provide_field_context<T: 'static>(binding: Binding<T>) -> FieldContext {
    provide_context(FieldContext::new(binding))
}

/// Resolves a binding using explicit prop, [`FieldContext`], then uncontrolled-state precedence.
///
/// The internal signal hook is called regardless of which source wins so the resolution order can
/// change between renders without violating Dioxus's hook ordering rules.
pub fn use_binding<T: 'static>(explicit: Option<Binding<T>>, default: T) -> Binding<T> {
    let internal = use_signal(|| default);

    if let Some(binding) = explicit {
        return binding;
    }

    if let Some(context) = try_consume_context::<FieldContext>() {
        return context.resolve();
    }

    internal.into()
}

#[derive(Clone)]
struct BindingIdentity(Rc<dyn ComparableIdentity>);

impl BindingIdentity {
    fn new<I: PartialEq + 'static>(identity: I) -> Self {
        Self(Rc::new(identity))
    }
}

impl PartialEq for BindingIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.0.equals(other.0.as_ref())
    }
}

trait ComparableIdentity: Any {
    fn equals(&self, other: &dyn ComparableIdentity) -> bool;
}

impl<I: PartialEq + 'static> ComparableIdentity for I {
    fn equals(&self, other: &dyn ComparableIdentity) -> bool {
        let other = other as &dyn Any;
        other.downcast_ref::<I>().is_some_and(|other| self == other)
    }
}
