# Dioxus Adapter Input Helpers

The Dioxus adapter provides headless controlled binding helpers. They expose names, values,
event-oriented methods, parse state where needed, and accessibility metadata, but applications own
all markup, option rendering, labels, styling, and layout.

## Event Handlers

Dioxus event handlers are `'static` closures, so each one must own what it captures. A binding is a
cheap `Rc`-backed handle, but it is `Clone` rather than `Copy`, so wiring a field into several
handlers by hand means one `binding.clone()` per handler.

To avoid that, every controlled binding exposes ready-made handler constructors that each own their
own clone: `oninput()` / `onchange()` (whichever the control uses) and `onblur()`. They take
`&self`, so the binding stays usable for `name()`, `value()`, and `is_selected(...)` reads in the
same `rsx!`:

```rust
let email = form.text(fields.email());

rsx! {
    input {
        name: email.name(),
        value: email.value(),
        oninput: email.oninput(),
        onblur: email.onblur(),
    }
}
```

Checkboxes use `onchange()` (reads `checked`), text/textarea/parsed inputs use `oninput()`, and
selects use `onchange()`. Radio groups and multi-select options render one control per value, so the
radio binding offers `onselect(value)` to wire each option without a per-option clone:

```rust
for plan_option in ["starter", "pro"] {
    input {
        r#type: "radio",
        name: plan.name(),
        checked: plan.is_selected(&plan_option.to_string()),
        onclick: plan.onselect(plan_option.to_string()),
    }
}
```

These cover the common cases. `onblur()` reports **Commit** and then **Focus Exit**; it does not
write the current value again. Custom widgets can report the two events independently through
`on_commit()` and `on_focus_exit()`. When a handler needs extra logic, fall back to a plain
`move |event| { ...; binding.on_input(event.value()) }` closure with an explicit `binding.clone()`.

## Tri-State Checkboxes

Use `FormHandle::tri_state_checkbox(path)` for an `Option<bool>` Field. Its `state()` and
`on_change(...)` methods preserve `Some(false)`, `Some(true)`, and `None` exactly. The binding does
not choose a state cycle or expose a native `onchange()` helper because a browser checkbox event
only reports `checked: bool`; the application or widget owns the cycle and indeterminate rendering.

## Choice Helpers

Use `FormHandle::select(path)` when the application can pass typed values directly, such
as from custom controls or typed option handlers. The binding exposes `value()`, `is_selected(...)`,
`on_change(value)`, `select(value)`, `on_commit()`, `on_focus_exit()`, `name()`, and
`accessibility()`.

Native select elements usually emit rendered string option values. Use
`FormHandle::select_with(path, parser, formatter)` for enum-like or custom typed fields:

```rust
let plan = form.select_with(plan_path, parse_plan, format_plan);

rsx! {
    select {
        name: plan.name(),
        value: plan.value(),
        onchange: plan.onchange(),
        option { value: "starter", selected: plan.is_rendered_selected("starter"), "Starter" }
        option { value: "pro", selected: plan.is_rendered_selected("pro"), "Pro" }
    }
}
```

The parser maps rendered option values into the typed field value. The formatter maps the current
typed value back into the rendered option value. Invalid rendered option values do not mutate the
typed draft; `try_on_change(...)` returns the parser error when the application wants to observe that
case. Select conversion failures do not register Parse Blockers because select options are
application-owned committed choices rather than free-form Raw Input State.

Use `FormHandle::radio_group(path)` for one typed field rendered as a radio group or any
radio-like custom UI. The application renders every option and calls `is_selected(...)` and
`select(value)` for each candidate. Radio helpers do not own option lists or visual components.

Hook variants are available for component code: `use_select`,
`use_select_with`, and `use_radio_group`.

## Optional Scalar Text

Use `FormHandle::optional_text(path)` or `use_optional_text(&form, path)` for an
`Option<String>` scalar field. This is a plain controlled binding, not a parsed binding: `""`
writes `None`, and every non-empty rendered value writes `Some(value)` unchanged. Whitespace-only
input is therefore present and is not trimmed. Normalization remains an application step or a Form
Listener concern.

Both `None` and an externally supplied `Some("")` render as `""`. The binding's `typed_value()`
accessor preserves that distinction until the next input event; entering the rendered empty value
always writes `None`, so `Some("")` is deliberately unreachable through this binding.

Because scalar presence conversion is infallible, `OptionalTextBinding` performs Input Parsing
without owning Raw Input State, Parse Error, or Parse Blocker.
In the narrower API sense it is not a parsed binding: presence has no fallible parse to retain.
[ADR-0046](adr/0046-bind-optional-text-as-controlled-scalar-presence.md) records this choice.

With the `dioxus-field` feature, an optional-text binding converts into a
`dioxus_field::Binding<String>` or a `dioxus_field::FieldContext` over its *rendered text*, so a
Widget Registry text input binds an `Option<String>` field with no per-field wiring:

```rust
let nickname = use_optional_text(&form, fields.nickname());

rsx! {
    TextField { context: nickname, label: "Nickname" }
}
```

The convention read renders `None` as `""`, and a convention write applies the same presence rule
as `on_input`: exactly empty writes `None`, anything else writes `Some(value)`, for user and
programmatic writes alike. A typed
`Binding<Option<String>>` conversion also exists for controls that consume the presence type
directly; with both conversions available, a `let binding: Binding<_> = nickname.into()` site
needs an explicit type annotation. A control that resolves `Option<String>` *through the context*
— a generic `Select<String>` over presence, say — gets a `BindingTypeMismatch` instead of the
rendered-text binding; hand it the presence type through the select helpers or the typed
conversion. Prop-position `context:` sites compile unchanged under the rendered-text behavior, so
code migrating from dioform 0.4 finds them by grep rather than by compile error. See
[ADR-0053](adr/0053-bind-optional-text-to-the-field-convention-as-rendered-text.md).

## Parsed Helpers

Parsed helpers are for rendered text-like input that may temporarily fail conversion into the typed
field value. They keep Raw Input State in the Dioxus adapter while the Form Core keeps the last valid
typed value.

Use these helpers when the rendered input is text-like:

- `parsed_text(path)` for `FromStr + ToString` values.
- `parsed_text_with(path, parser, formatter)` for custom typed values.
- `number(path)` for built-in numeric field types.
- `number_with(path, parser, formatter)` for custom numeric behavior, including optional
  fields where empty input maps to `None`.
- `use_optional_number(&form, path)` for optional built-in numeric scalar fields, where `""`
  maps to `None` and non-empty invalid input remains a Parse Error.
- `date(path)` for date-like values that implement `FromStr` and `ToString`.
- `date_with(path, parser, formatter)` for date-like domain values without requiring
  `chrono`, `time`, or any other date dependency.
- `use_optional_date(&form, path)` for optional date-like scalar fields, where `""` maps to
  `None` and non-empty invalid input remains a Parse Error.

In Dioxus components, prefer the hook variants for parsed helpers, such as `use_number(...)`,
`use_optional_number(...)`, `use_date(...)`, `use_optional_date(...)`, and `use_date_with(...)`.
Parsed bindings own mounted parse state, so the hook keeps the Parse Blocker lifecycle stable across
rerenders.

For collection item child fields, use `use_collection_item_parsed_text(...)`,
`use_collection_item_parsed_text_with(...)`, `use_collection_item_number(...)`,
or `use_collection_item_number_with(...)` in row components keyed by **Collection Item Identity**.
Those hooks keep Parse Blockers keyed by the logical collection item and child field while rendered
input names update after reordering. The row's hook state lives in the scope that calls the hook, so
a plain `fn` row helper or an index key hands that state to the wrong item after a removal or a
reorder; see [Collection Fields](collection-fields.md).

A scope that is not a row may bind a collection item too, and a scope that renders a different item
than it did last render re-addresses its Parse Blocker to the item it renders now. That drops the
in-flight raw text and parse error held for the previous item, and the input renders the new item's
formatted value; see [Collection Fields](collection-fields.md).

## Parsed Helpers and the Field Convention

With the `dioxus-field` feature, a parsed binding converts into a `dioxus_field::Binding<String>` or
a `dioxus_field::FieldContext`, so a Widget Registry text input renders a number or date field with
no per-field wiring:

```rust
let quantity = use_number(&form, fields.quantity());

rsx! {
    TextField { context: quantity, label: "Quantity", r#type: "number" }
}
```

The convention binding is over the *rendered text*, not the typed value: it reads Raw Input State
while a Parse Blocker stands and the formatted field value otherwise, and a write parses before it
reaches the field. A user write parses, or marks the field touched and raises a Parse Blocker; a
programmatic write parses and writes programmatically, reporting no interaction.

An unresolved Parse Error leads the reported Field Meta errors and marks the field invalid, so the
registry's own error region renders it. It does not become a Validation Error, and unlike a
Validation Error it does not wait for a Commit to become visible: it clears on the keystroke that
makes the text parse. See
[ADR-0052](adr/0052-bind-parsed-fields-to-the-field-convention-as-rendered-text.md).

`use_optional_text` reaches text controls the same way — as rendered text, though with no parse
state to carry; see [Optional Scalar Text](#optional-scalar-text) above and
[ADR-0053](adr/0053-bind-optional-text-to-the-field-convention-as-rendered-text.md).

Collection item bindings, including the parsed ones, stay outside the Field Convention.

## Parsing Versus Validation

Input Parsing converts rendered input into a typed Field value. Field Validation and Form Validation
check typed values. A failed parse:

- Preserves the rendered raw input so the user can correct it.
- Leaves the Form Draft at the last valid typed value.
- Exposes a binding-level Parse Error separately from Validation Errors.
- Registers a mounted Parse Blocker so Dioxus-Managed Submission cannot submit stale typed values.
- Marks the field touched without running typed validation for a value that does not exist.
- On Commit, marks the field committed but does not run typed Commit validation while the Parse
  Error is active.
- On Focus Exit, marks the field touched and blurred and dispatches blur listeners without running
  validation.

A successful parse updates the typed field through the user update path, clears the binding's Parse
Error and Parse Blocker, and participates in configured value-change validation.

Reset, reinitialization, and unmounting parsed bindings clear mounted parse state. Resetting one
Collection Field clears the parse state of its retained rows and unregisters parsed bindings for
rows the reset drops, without touching other collections. Unmounting a parsed binding unregisters
its Parse Blocker without mutating the Form Draft.

## Manual Typed Setters

The built-in helpers are not the only way to update fields. For unusual controls, applications can
use manual typed setters as an escape hatch:

- `FormHandle::set_user_field(path, value)` applies a user-originated typed update, marking the
  field touched and participating in value-change validation.
- `FormHandle::set_field(path, value)` applies a programmatic typed update without marking the field
  touched.
- Binding-level setters such as `set_value(...)` and `set_checked(...)` wrap programmatic updates for
  the corresponding controlled helper and clear parse state where relevant.

## Multi-Select Boundary

Use independent boolean checkbox Fields when each checkbox represents a separate durable domain
answer, such as `accepts_terms`, `wants_email`, and `wants_sms`. Each checkbox has its own typed
Field value, metadata, validation, and rendered name.

Use `FormHandle::multi_select(path)` when one typed Field contains many selected values, such as
`topics: Vec<Topic>`. The multi-select helper is headless: applications render the options, labels,
layout, and copy, then call `option(value).on_change(checked)` or the typed `select`, `deselect`, and
`toggle` methods.

```rust
let topics = form.multi_select(ProfileForm::fields().topics());
let rust = topics.option(Topic::Rust);

rsx! {
    input {
        r#type: "checkbox",
        name: rust.name(),
        checked: rust.checked(),
        oninput: rust.onchange(),
    }
}
```

The selected values are stored in the single `Vec<Topic>` Field, but each selected value is also a
logical collection item with library-owned **Collection Item Identity**. That means selected-value
metadata, item-level validation attachment, dirty tracking, reset, reinitialization, submission, and
future reordering compatibility follow existing **Collection Field** semantics rather than ad hoc
adapter-only state.

In Dioxus components, `use_multi_select(&form, path)` provides the same stable binding pattern as the
other choice-helper hooks.
