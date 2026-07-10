# Whole-Form Error Summaries

Dioform exposes every stored **Validation Error** across the whole form in one call, so
applications can build an accessible error-summary panel (the common "N errors, jump to field"
pattern) without walking fields by hand. This is the source-aware analog of TanStack Form's
`getAllErrors`.

## The aggregate accessors

`FormHandle::validation_errors()` returns every stored **Validation Error** across the whole form as
`Vec<ValidationErrorSnapshot>`, in deterministic flattened order. It spans:

- every direct **Field**,
- every **Collection Field** item child field,
- the **Form** itself (form-level validators), and
- stored **Submit Errors**.

`FormHandle::visible_validation_errors()` is the same aggregate filtered by the current **Error
Visibility** policy. Use `validation_errors()` when you want *all* stored errors regardless of
visibility (for example, a summary shown only after a submit attempt that should still list
everything), and `visible_validation_errors()` when the summary should track exactly what the inline
field messages show.

Both are pure derived reads over existing source-aware storage; they add no new state and change no
semantics. Scoped accessors remain available when you do not need the whole form:
`field_validation_errors(path)`, `form_validation_errors()`, and the `visible_*` variants.

## What each entry carries

Each `ValidationErrorSnapshot` keeps full **Validation Source** detail rather than flattening multiple
sources on one field into a single slot:

- `target()`: the `ValidationTarget`, either the form or a field.
- `field_identity()`: the typed `FieldIdentity` for field-level errors (`None` for form-level).
- `source()`: the `ValidatorSource` that produced the error (native label, an adapter source such
  as `garde`, or `submit` for submit errors).
- `validator_id()`: the per-validator identity for validator-sourced errors (`None` for submit
  errors).
- `error()`: the typed **Validation Error** value.

## Rendered field names and accessibility

Targets are reported as typed **Field Identity**, not as bare rendered **Field Names**. This keeps the
aggregate stable and framework-neutral: **Field Identity** is the durable join key that does not depend
on rendered names, serde names, or Rust field names.

To build the accessible "jump to field" summary, pair each error's `field_identity()` with your own
typed `FieldPath` to recover the rendered **Field Name**, then link the summary entry to the input the
same way you would elsewhere, through the **Accessibility Helpers** (`FormHandle::field_accessibility`)
that already produce the field's id and ARIA relationships. Because the application owns its typed
field paths, it owns the identity → name mapping; the library does not embed a name registry.

```rust
// Sketch: turn the aggregate into summary rows that link to inputs.
let email = SignupForm::fields().email();
for snapshot in form.validation_errors() {
    let (name, id) = match snapshot.field_identity() {
        Some(identity) if identity == email.identity() => (
            email.field_name().to_owned(),
            form.field_accessibility(email.clone()).input_id().to_owned(),
        ),
        Some(_) => continue, // match the rest of your known field paths
        None => ("form".to_owned(), String::new()), // form-level error
    };
    // render a summary entry for (name, id, snapshot.error())
}
```

## Related

- Per-field grouping by trigger/source (one field's errors, not the whole form) is tracked separately
  in issue #136; this page is the form-wide aggregate across every field plus the form.
- Error Visibility policy: `FormHandle::set_error_visibility_policy`.
- Accessibility Helpers: `FormHandle::field_accessibility`.
