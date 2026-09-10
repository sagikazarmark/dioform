# dioform-core

[![crates.io](https://img.shields.io/crates/v/dioform-core?style=flat-square)](https://crates.io/crates/dioform-core)
[![docs.rs](https://img.shields.io/docsrs/dioform-core?style=flat-square)](https://docs.rs/dioform-core)

**Renderer-agnostic typed form state core for [Dioform](https://github.com/sagikazarmark/dioform).**

The core owns form drafts, typed field paths, validation state, submission state,
and reset and reinitialization semantics, plus value-redacted observer events,
without depending on Dioxus or a concrete async runtime.

Async and debounced validation cross the runtime boundary through explicit
work-token APIs. The core decides when a validator is pending, skipped, stale,
valid, or invalid; adapters execute the returned work from owned `FormSnapshot`
values and complete it back into the core.

Most applications should depend on the [`dioform`](https://crates.io/crates/dioform)
facade instead of using this crate directly. Use `dioform-core` when building
a renderer other than Dioxus, a validation adapter, or server-side validation.

## Install

```toml
[dependencies]
dioform-core = "0.6"
dioform-derive = "0.6"
```

`dioform-core` exports the `Form` trait; `dioform-derive` supplies the derive macro.
Import both as shown below: traits and macros occupy separate namespaces. The
macro generates paths through `::dioform` by default, so core-only models need
`#[form(crate = "::dioform_core")]`. The same attribute works for
`#[derive(FieldGroup)]`. It selects the generated code's crate path, independently
of rendered field names. A shared model derived against core can also be used by
the Dioxus facade in a client application.

## Validating on the Server with `dioform-core`

For a one-shot synchronous check, construct a fresh `FormCore`, register validators,
and explicitly run `validate_all(ValidationTrigger::Submit)`. Construction and
registration alone do not run validation. This example returns application-owned
diagnostics ready for a template or response payload:

```rust
use dioform_core::{Form, FormCore, ValidationTrigger};
use dioform_derive::Form;

#[derive(Clone, Form)]
#[form(crate = "::dioform_core")]
struct CreateLinkForm {
    #[form(name = "link-title")]
    title: String,
}

#[derive(Debug, PartialEq)]
struct Diagnostic {
    // None means a form-level diagnostic, displayed in the form summary.
    field_name: Option<String>,
    message: String,
}

fn validate_link(model: CreateLinkForm) -> Vec<Diagnostic> {
    let title = CreateLinkForm::fields().title();
    let mut form = FormCore::new(model);
    form.register_sync_field_validator_for_triggers(
        title.clone(),
        "required-title",
        ValidationTrigger::Submit,
        |value, _context| {
            if value.trim().is_empty() {
                vec!["Enter a title".to_owned()]
            } else {
                Vec::new()
            }
        },
    );
    form.validate_all(ValidationTrigger::Submit);

    form.validation_errors()
        .into_iter()
        .map(|error| Diagnostic {
            field_name: match error.field_identity() {
                Some(identity) if identity == title.identity() => {
                    Some(title.field_name().to_owned())
                }
                // Keep form-level or unrecognized targets in the summary.
                _ => None,
            },
            message: error.error().clone(),
        })
        .collect()
}

assert_eq!(
    validate_link(CreateLinkForm { title: " ".into() }),
    vec![Diagnostic {
        field_name: Some("link-title".into()),
        message: "Enter a title".into(),
    }],
);
assert!(validate_link(CreateLinkForm { title: "Project invite".into() }).is_empty());
```

### Choosing the Validation Entry Point

- `validate_all(ValidationTrigger::Submit)` runs synchronous field and form
  validators registered for that trigger. On a fresh core it does not establish a
  **Submit Intent**, record a submit attempt, or perform submit-lifecycle
  bookkeeping. Use it for a simple synchronous pass whose rules do not inspect
  submit intent, as above.
- `validate_for_submit()` records an attempt (including the `SubmitAttempted`
  observer event), clears previous submit errors, establishes the unit submit
  intent, and runs the submit-validation cycle. It also marks unresolved
  submit-relevant async validators pending and checks submit blockers. Use it
  when participating in Dioform's submission lifecycle; for typed purposes use
  `form.intent(intent).validate_for_submit()` so validators receive that intent.
  This validates for submission; it does not itself run application submit behavior.

Neither call executes async validators automatically. This walkthrough registers
only synchronous validators. Async validation requires a runtime adapter to start
and complete the core's work tokens; an empty error list alone does not prove that
pending or unexecuted async checks passed.

### Reading and Rendering Errors

`validation_errors()` returns borrowed error views regardless of **Error Visibility**.
Use it for server responses instead of `visible_validation_errors()`, whose UI
policy may hide errors before interaction or a submit attempt. Clone or convert
the error values before the core is dropped, as the helper does above.

`field_identity()` returns `None` for a form-level error. For field errors, compare
the structured **Field Identity** with a known typed path's `identity()`, then use
that path's `field_name()` for rendered output. In this example the identity is
`title` while the rendered name is `link-title`; do not treat the identity string
as an HTML name. Extend this explicit mapping for additional fields and preserve
unrecognized targets in a form summary rather than dropping their errors.

### Async Server Handlers

`FormCore` holds `Rc`-backed behavior and is **not `Send`**. Keeping it live across
an `.await` makes that future non-`Send`, which is incompatible with server APIs
that require `Send` futures. A local executor may allow this; `.await` itself is
not prohibited. Prefer a synchronous helper such as `validate_link` that drops
the core and returns owned diagnostics before async transport or database work.
The `Vec<Diagnostic>` above contains only owned strings and can cross a `Send`
boundary; custom diagnostic types must also satisfy the server's bounds.

For external validation libraries, see the complete core-only
[validation adapter example](https://github.com/sagikazarmark/dioform/blob/main/docs/validation-adapters.md#string-convenience).

## Feature Flags

- `serde`: enables serialization support for form-state snapshots.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
