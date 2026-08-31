use dioxus::prelude::*;
use dioxus_field::{
    Binding, ChangeOrigin, FieldContext, FieldControlOptions, FieldMeta, FieldSurface,
    merge_attributes, use_binding, use_field_meta, use_focus_registration,
};
use dioxus_primitives::dioxus_attributes::attributes;
use std::rc::Rc;

use crate::components::field::{
    Field, FieldAppearance, FieldDescription, FieldDescriptionAppearance, FieldError,
    FieldErrorAppearance, FieldLabel,
};

/// daisyUI's colour axis for an input.
///
/// [`InputColor::Default`] emits no class, which is daisyUI's uncoloured input
/// rather than a synonym for [`InputColor::Neutral`].
#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub enum InputColor {
    #[default]
    Default,
    Neutral,
    Primary,
    Secondary,
    Accent,
    Info,
    Success,
    Warning,
    Error,
}

impl InputColor {
    /// Every value of this axis, in the order the preview renders them.
    pub const ALL: &'static [Self] = &[
        Self::Default,
        Self::Neutral,
        Self::Primary,
        Self::Secondary,
        Self::Accent,
        Self::Info,
        Self::Success,
        Self::Warning,
        Self::Error,
    ];

    /// The daisyUI class name for this value, as a complete string literal so
    /// Tailwind's scanner can see it.
    pub const fn class(self) -> &'static str {
        match self {
            Self::Default => "",
            Self::Neutral => "input-neutral",
            Self::Primary => "input-primary",
            Self::Secondary => "input-secondary",
            Self::Accent => "input-accent",
            Self::Info => "input-info",
            Self::Success => "input-success",
            Self::Warning => "input-warning",
            Self::Error => "input-error",
        }
    }
}

/// daisyUI's size axis for an input.
///
/// [`InputSize::Default`] emits no class and renders at the same size as
/// daisyUI's explicit `input-md`.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub enum InputSize {
    Xs,
    Sm,
    #[default]
    Default,
    Lg,
    Xl,
}

impl InputSize {
    /// Every value of this axis, from smallest to largest.
    pub const ALL: &'static [Self] = &[Self::Xs, Self::Sm, Self::Default, Self::Lg, Self::Xl];

    /// The daisyUI class name for this value, as a complete string literal so
    /// Tailwind's scanner can see it.
    pub const fn class(self) -> &'static str {
        match self {
            Self::Xs => "input-xs",
            Self::Sm => "input-sm",
            Self::Default => "",
            Self::Lg => "input-lg",
            Self::Xl => "input-xl",
        }
    }
}

/// daisyUI's appearance axis for an input.
///
/// [`InputAppearance::Default`] emits no class, which is daisyUI's bordered
/// input rather than a named style.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub enum InputAppearance {
    #[default]
    Default,
    Ghost,
}

impl InputAppearance {
    /// Every value of this axis, in the order the preview renders them.
    pub const ALL: &'static [Self] = &[Self::Default, Self::Ghost];

    /// The daisyUI class name for this value, as a complete string literal so
    /// Tailwind's scanner can see it.
    pub const fn class(self) -> &'static str {
        match self {
            Self::Default => "",
            Self::Ghost => "input-ghost",
        }
    }
}

/// A native text field styled with daisyUI's `input` class.
///
/// Producer-defined invalidity emits `input-error` when no colour is passed.
/// Binding, metadata, and focus resolve from explicit props, Field Context,
/// then standalone state.
///
/// Classes passed by the caller concatenate with the input's own; every other
/// attribute the caller passes overrides the input's. Event handlers are
/// explicit because Dioxus' extended attributes do not include them.
#[component]
pub fn Input(
    /// An explicit colour, or `None` to derive error colour from Field metadata.
    #[props(default)]
    color: Option<InputColor>,
    /// daisyUI's size axis.
    #[props(default)]
    size: InputSize,
    /// daisyUI's appearance axis.
    #[props(default)]
    appearance: InputAppearance,
    /// An explicit Field binding, which wins over Field Context.
    binding: Option<Binding<String>>,
    /// Explicit Field metadata, which wins over Field Context.
    meta: Option<FieldMeta>,
    /// The value rendered by the input.
    #[props(default)]
    value: Option<ReadSignal<String>>,
    /// Called with the input's value after user input.
    on_change: Option<EventHandler<String>>,
    /// Called when the native `change` event ends the interaction unit.
    on_commit: Option<EventHandler<()>>,
    /// Called after focus leaves the native input.
    on_focus_exit: Option<EventHandler<()>>,
    /// Whether the native input is required.
    #[props(default)]
    required: Option<bool>,
    /// Whether the native input is disabled.
    #[props(default)]
    disabled: Option<bool>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = input)]
    attributes: Vec<Attribute>,
) -> Element {
    let binding = use_binding(binding, String::new());
    let meta = use_field_meta(meta);
    let color = color.map_or_else(
        || {
            if meta.invalid() { "input-error" } else { "" }
        },
        InputColor::class,
    );
    let size = size.class();
    let appearance = appearance.class();
    let binding_value = binding.read;
    let resolved_value = value.unwrap_or(binding_value);
    let mut focus_exit_reported = use_signal(|| false);
    let mut control: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let focus_control = use_callback(move |()| {
        if let Some(control) = control() {
            spawn(async move {
                let _ = control.set_focus(true).await;
            });
        }
    });
    use_focus_registration(focus_control);

    let base = attributes!(input {
        class: "input {color} {size} {appearance}",
    });
    let meta_attributes = meta.attributes_for(
        &FieldControlOptions::new()
            .disabled(disabled)
            .required(required)
            .surface(FieldSurface::NATIVE),
    );
    let merged = merge_attributes(vec![meta_attributes, base, attributes]);
    let change_binding = binding.clone();
    let commit_binding = binding.clone();
    let focus_exit_binding = binding;

    rsx! {
        input {
            value: resolved_value,
            onmounted: move |event: MountedEvent| control.set(Some(event.data())),
            onfocusin: move |_| focus_exit_reported.set(false),
            oninput: move |event| {
                let next = event.value();
                change_binding.write(next.clone(), ChangeOrigin::User);
                if let Some(handler) = &on_change {
                    handler.call(next);
                }
            },
            onchange: move |_| {
                commit_binding.commit();
                if let Some(handler) = &on_commit {
                    handler.call(());
                }
            },
            onfocusout: move |_| {
                if focus_exit_reported() {
                    return;
                }
                focus_exit_reported.set(true);
                focus_exit_binding.focus_exit();
                if let Some(handler) = &on_focus_exit {
                    handler.call(());
                }
            },
            ..merged,
        }
    }
}

/// The common Field composition for a native text input.
///
/// This Composition sugar intentionally has no children. Use [`Field`] and its
/// Compound parts when content or attributes must land between the parts. Native
/// input attributes and caller classes are forwarded to [`Input`].
#[component]
pub fn InputField(
    /// The context supplied to the input and every Field part.
    #[props(into)]
    context: FieldContext,
    /// The input's visible label.
    label: String,
    /// Supporting text rendered between the input and its error region.
    #[props(default)]
    description: Option<String>,
    /// An explicit colour, or `None` to derive error colour from Field metadata.
    #[props(default)]
    color: Option<InputColor>,
    /// daisyUI's size axis.
    #[props(default)]
    size: InputSize,
    /// daisyUI's appearance axis.
    #[props(default)]
    appearance: InputAppearance,
    /// Whether the surrounding Field emits its default layout utilities.
    #[props(default)]
    field_appearance: FieldAppearance,
    /// Whether supporting text emits its default wrapping utilities.
    #[props(default)]
    description_appearance: FieldDescriptionAppearance,
    /// Whether the error region emits its default semantic colour.
    #[props(default)]
    error_appearance: FieldErrorAppearance,
    /// An explicit Field binding, which wins over `context` for the input.
    binding: Option<Binding<String>>,
    /// Explicit Field metadata, which wins over `context` for the input.
    meta: Option<FieldMeta>,
    /// The value rendered by the input.
    #[props(default)]
    value: Option<ReadSignal<String>>,
    /// Called with the input's value after user input.
    on_change: Option<EventHandler<String>>,
    /// Called when the native `change` event ends the interaction unit.
    on_commit: Option<EventHandler<()>>,
    /// Called after focus leaves the native input.
    on_focus_exit: Option<EventHandler<()>>,
    /// Whether the native input is required.
    #[props(default)]
    required: Option<bool>,
    /// Whether the native input is disabled.
    #[props(default)]
    disabled: Option<bool>,
    #[props(extends = GlobalAttributes)]
    #[props(extends = input)]
    attributes: Vec<Attribute>,
) -> Element {
    rsx! {
        Field { context, appearance: field_appearance,
            FieldLabel { {label} }
            Input {
                color,
                size,
                appearance,
                binding,
                meta,
                value,
                on_change,
                on_commit,
                on_focus_exit,
                required,
                disabled,
                attributes,
            }
            if let Some(description) = description {
                FieldDescription { appearance: description_appearance, {description} }
            }
            FieldError { appearance: error_appearance }
        }
    }
}
