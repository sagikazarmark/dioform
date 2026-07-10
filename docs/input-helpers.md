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

These cover the common cases. When a handler needs extra logic, fall back to a plain
`move |event| { ...; binding.on_input(event.value()) }` closure with an explicit `binding.clone()`;
the `on_input` / `on_change` / `on_blur` methods used above remain available for that.

## Choice Helpers

Use `FormHandle::select(path)` when the application can pass typed values directly, such
as from custom controls or typed option handlers. The binding exposes `value()`, `is_selected(...)`,
`on_change(value)`, `select(value)`, `on_blur()`, `name()`, and `accessibility()`.

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
- `date(path)` for date-like values that implement `FromStr` and `ToString`.
- `date_with(path, parser, formatter)` for date-like domain values without requiring
  `chrono`, `time`, or any other date dependency.

In Dioxus components, prefer the hook variants for parsed helpers, such as
`use_number(...)`, `use_date(...)`, and `use_date_with(...)`.
Parsed bindings own mounted parse state, so the hook keeps the Parse Blocker lifecycle stable across
rerenders.

For collection item child fields, use `use_collection_item_parsed_text(...)`,
`use_collection_item_parsed_text_with(...)`, `use_collection_item_number(...)`,
or `use_collection_item_number_with(...)` in row components. Those hooks keep Parse
Blockers keyed by the logical collection item and child field while rendered input names update after
reordering.

## Parsing Versus Validation

Input Parsing converts rendered input into a typed Field value. Field Validation and Form Validation
check typed values. A failed parse:

- Preserves the rendered raw input so the user can correct it.
- Leaves the Form Draft at the last valid typed value.
- Exposes a binding-level Parse Error separately from Validation Errors.
- Registers a mounted Parse Blocker so Dioxus-Managed Submission cannot submit stale typed values.
- Marks the field touched without running typed validation for a value that does not exist.
- On blur, marks the field blurred but does not run typed blur validation while the Parse Error is
  active.

A successful parse updates the typed field through the user update path, clears the binding's Parse
Error and Parse Blocker, and participates in configured value-change validation.

Reset, reinitialization, and unmounting parsed bindings clear mounted parse state. Unmounting a
parsed binding unregisters its Parse Blocker without mutating the Form Draft.

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

In Dioxus components, `use_multi_select(form, path)` provides the same stable binding pattern as the
other choice-helper hooks.
