# Validation Adapters

Dioform keeps external validation libraries outside the **Form Core** and the Dioxus **Facade Crate**. A **Validation Adapter Crate** maps an external library's diagnostics into the form's shared **Validation Error** type through normal **Form Validation** APIs.

There are two first-party adapter crates:

- `dioform-garde` for the [`garde`](https://docs.rs/garde) crate.
- `dioform-validator` for the [`validator`](https://docs.rs/validator) crate.

Both are renderer-agnostic, depend only on `dioform-core` plus their external library, and register a synchronous form validator on `FormCore`. Neither adds its external library to `dioform-core` or `dioform`. They are intentionally separate but similar; the structure they share (the external-path map and the borrowed diagnostic view) moves into a `dioform-validation-adapter` support crate, while each adapter keeps its own library-specific diagnostic iteration and builder (see [ADR-0012](adr/0012-use-a-shared-validation-adapter-support-crate.md)). The sections below document the `garde` adapter first, then the `validator` adapter, and end with a comparison of both against deferred schema-oriented libraries.

## Dependencies

Applications own their `garde` feature choices. The adapter depends on `garde` with `default-features = false` and does not enable `garde/full` or `garde/derive` by default.

Choose the derive and rule features your application needs:

```toml
[dependencies]
dioform-core = "0.1"
dioform-garde = "0.1"
garde = { version = "0.23", default-features = false, features = ["derive", "email"] }
```

Use `garde/full` only when your application wants that larger dependency set. The adapter does not require it.

## String Convenience

Simple forms whose shared validation error type is `String` can register the adapter without writing a mapper closure:

```rust
use dioform_core::{FormCore, ValidationTrigger};
use dioform_garde::{GardePathMap, GardeValidationExt};

let mut form = FormCore::new(SignupForm::default());

form.garde_validation()
    .triggers(ValidationTrigger::Submit)
    .path_map(GardePathMap::new().with_field("email", email_path()))
    .register_string_errors();
```

`register_string_errors` stores `garde::Error::to_string()` as the validation error value. It still uses the path map for field or form attachment, but the `String` itself does not preserve the original external path or selected target.

Use this path for small forms where display text is enough. Use a custom enum or struct for richer applications.

## Custom Error Mapping

Every validator in one form uses the same **Validation Error** type. Applications usually map `garde` diagnostics into their own enum or struct so native validators, submit errors, and adapter errors can coexist while preserving useful external details.

```rust
use dioform_core::{FormCore, ValidationTarget, ValidationTrigger};
use dioform_garde::{GardeDiagnostic, GardePathMap, GardeValidationExt};

#[derive(Clone, Debug, Eq, PartialEq)]
enum ValidationError {
    Native(&'static str),
    Garde {
        external_path: String,
        message: String,
        target: ValidationTarget,
    },
}

fn map_garde(diagnostic: GardeDiagnostic<'_>) -> ValidationError {
    ValidationError::Garde {
        external_path: diagnostic.path().to_string(),
        message: diagnostic.error().to_string(),
        target: diagnostic.target(),
    }
}

let mut form: FormCore<SignupForm, ValidationError> =
    FormCore::new_with_error_type(SignupForm::default());

form.garde_validation()
    .source("garde-model")
    .triggers(ValidationTrigger::Submit)
    .path_map(GardePathMap::new().with_field("email", email_path()))
    .register(map_garde);
```

The mapper receives a `GardeDiagnostic` containing the original `garde::Path`, the original `garde::Error`, and the final Dioform `ValidationTarget`. Path attachment and error conversion are separate decisions.

## Explicit Path Mapping

`garde` reports **External Diagnostic Paths**. Dioform renders and stores errors through typed **Field Paths**. The adapter never treats rendered **Field Names**, serde names, or Rust field names as implicit validation addresses.

Map external paths explicitly:

```rust
let path_map = GardePathMap::new()
    .with_field("email", email_path())
    .with_field("password", password_path());

form.garde_validation()
    .path_map(path_map)
    .register(map_garde);
```

Path matching is exact and uses the canonical `garde::Path::to_string()` representation. If `garde` reports a path with no **Explicit Path Mapping**, the adapter preserves that **Unmapped Diagnostic** by attaching it to the form with `ValidationTarget::form()` instead of dropping it. This can happen with a partially populated map as well as an empty one. Under the default **Error Visibility** policy, the form-scoped error is invisible before a submit attempt but still blocks **Submit Availability**.

The mapper can inspect the original external path through `diagnostic.path()`. Because a `GardePathMap` contains only field targets, a non-empty path whose `diagnostic.target().is_form()` is an **Unmapped Diagnostic**. The empty-path case is different: `garde` uses an empty `garde::Path` for a genuinely whole-model diagnostic, which also resolves to the form.

### Reporting Unmapped Diagnostics

Use `on_unmapped_path` when the application should log, test, or otherwise diagnose incomplete path mapping:

```rust
use std::{cell::RefCell, rc::Rc};

let unmapped_paths = Rc::new(RefCell::new(Vec::new()));
let reported_paths = Rc::clone(&unmapped_paths);

form.garde_validation()
    .path_map(GardePathMap::new().with_field("email", email_path()))
    .on_unmapped_path(move |path| {
        reported_paths.borrow_mut().push(path.to_string());
    })
    .register_string_errors();
```

The reporter runs once per **Unmapped Diagnostic**, in report order, on every validation run against the current **Form Draft**. It does not require `Send`, change diagnostic routing, or report `garde`'s genuinely whole-model empty path. Without a reporter, the adapter remains silent.

## Trigger Choices

The builder defaults to the `garde` source, `ValidationTriggers::all()`, and an empty `GardePathMap`. It therefore behaves like a native synchronous form validator and routes every non-empty diagnostic path to the form until mappings are added. Whole-model external validation can be more expensive than field-local validation, so choose triggers deliberately.

Submit-only validation is often the right first choice:

```rust
form.garde_validation()
    .triggers(ValidationTrigger::Submit)
    .path_map(path_map)
    .register(map_garde);
```

Use `ValidationTrigger::Change` only when the form's **Validation Mode** and the adapter triggers intentionally opt into live or post-submit revalidation. Use `ValidationTrigger::Initial` only when initial invalid drafts should be checked explicitly through normal initialization validation.

## Context-Aware Garde Validation

`garde::Validate::Context` is external `garde` validation context. Dioform's `FormValidatorContext` is lifecycle context for the current validation run. They are related only through the context-provider closure you supply.

```rust
use dioform_core::ValidationTrigger;

struct SignupLimits {
    minimum_password_length: usize,
}

#[derive(garde::Validate)]
#[garde(context(SignupLimits as limits))]
struct SignupForm {
    #[garde(length(min = limits.minimum_password_length))]
    password: String,
}

form.garde_validation()
    .triggers([ValidationTrigger::Manual, ValidationTrigger::Submit])
    .path_map(GardePathMap::new().with_field("password", password_path()))
    .register_with_context(
        |context| SignupLimits {
            minimum_password_length: match context.trigger() {
                ValidationTrigger::Submit => 12,
                _ => 8,
            },
        },
        map_garde,
    );
```

The provider runs for each validation run. It can derive external `garde` context from the current form draft, the Dioform validation trigger, the adapter source label, or field metadata exposed through `FormValidatorContext`.

String-error forms can use `register_string_errors_with_context` with the same provider shape.

## Coexisting With Native Validators

The adapter registers a normal source-aware form validator. Its errors coexist with native Dioform **Field Validation**, native **Form Validation**, submit errors, and other validator sources as long as they all return the same shared **Validation Error** type.

```rust
form.register_sync_field_validator_for_triggers(
    email_path(),
    "native-email",
    ValidationTrigger::Submit,
    |email, _context| {
        if email.ends_with("@example.invalid") {
            vec![ValidationError::Native("reserved email domain")]
        } else {
            Vec::new()
        }
    },
);

form.garde_validation()
    .source("garde-model")
    .triggers(ValidationTrigger::Submit)
    .path_map(GardePathMap::new().with_field("email", email_path()))
    .register(map_garde);
```

Rerunning the `garde` adapter replaces only errors from that adapter source. A successful `garde` validation clears previous `garde` adapter errors without clearing native validator errors or submit errors from other sources. The default adapter source label is `garde`; use `.source("...")` when multiple adapter registrations need distinct labels.

# Validator Adapter

`dioform-validator` maps [`validator`](https://docs.rs/validator) diagnostics into the form's shared **Validation Error** type. It mirrors the `garde` adapter UX: a builder on `FormCore`, **Explicit Path Mapping**, **Unmapped Diagnostic** preservation and reporting, source-aware replacement, and a string convenience for simple forms.

```rust
use dioform_core::{FormCore, ValidationTrigger};
use dioform_validator::{ValidatorPathMap, ValidatorValidationExt};

let mut form = FormCore::new(SignupForm::default());

form.validator_validation()
    .source("validator")
    .triggers(ValidationTrigger::Submit)
    .path_map(ValidatorPathMap::new().with_field("email", email_path()))
    .register_string_errors();
```

## Dependencies

Applications own their `validator` feature choices. The adapter depends on `validator` with `default-features = false` and does not enable `validator/derive`. Add the derive and rule features your application needs:

```toml
[dependencies]
dioform-core = "0.2"
dioform-validator = "0.2"
validator = { version = "0.20", features = ["derive"] }
```

## Flattened Diagnostic Paths

`validator::ValidationErrors` is a nested tree of struct, list, and field diagnostics. The adapter flattens it into stable diagnostic records before mapping. Each diagnostic carries a canonical **External Diagnostic Path**:

- Nested structs join with a dot: `address.street`.
- List items use bracketed indices: `lines[0].quantity`.

Ordering is deterministic: `validator` stores fields in a `HashMap`, so the adapter sorts field keys, iterates list indices in ascending order, and preserves each field's error-vector order. This adapter does not translate list indices into Dioform **Collection Item Identity**: a `lines[0]` diagnostic maps to whatever field path you register for that literal path, or to the form if unmapped.

## Custom Error Mapping

The mapper receives a `ValidatorDiagnostic` exposing the canonical flattened path, the original `validator::ValidationError` (with its `code`, `message`, and `params`), and the selected `ValidationTarget`. Map those into your own error type to preserve external details across sources:

```rust
use dioform_core::{FormCore, ValidationTarget, ValidationTrigger};
use dioform_validator::{ValidatorDiagnostic, ValidatorPathMap, ValidatorValidationExt};

#[derive(Clone, Debug, Eq, PartialEq)]
enum ValidationError {
    Native(&'static str),
    Validator {
        external_path: String,
        code: String,
        message: Option<String>,
        target: ValidationTarget,
    },
}

fn map_validator(diagnostic: ValidatorDiagnostic<'_>) -> ValidationError {
    ValidationError::Validator {
        external_path: diagnostic.path().to_owned(),
        code: diagnostic.error().code.to_string(),
        message: diagnostic.error().message.as_ref().map(|m| m.to_string()),
        target: diagnostic.target(),
    }
}

form.validator_validation()
    .source("validator-model")
    .triggers(ValidationTrigger::Submit)
    .path_map(ValidatorPathMap::new().with_field("email", email_path()))
    .register(map_validator);
```

`register_string_errors` is the lossy convenience: it stores the diagnostic message when present, otherwise the diagnostic code. It does not preserve the external path, params, or selected target inside the `String`. Use a custom enum or struct when those matter.

## Explicit Path Mapping

Path matching is exact against the canonical flattened path. A path with no **Explicit Path Mapping** produces an **Unmapped Diagnostic** that attaches to the form with `ValidationTarget::form()` instead of being dropped. This can happen with a partially populated map as well as an empty one. Under the default **Error Visibility** policy, the form-scoped error is invisible before a submit attempt but still blocks **Submit Availability**. The adapter never treats rendered **Field Names**, serde names, `validator` field keys, or Rust field names as implicit validation addresses.

The mapper can inspect the external path through `diagnostic.path()`. Because a `ValidatorPathMap` contains only field targets, `diagnostic.target().is_form()` inside the mapper means that path was unregistered.

```rust
let path_map = ValidatorPathMap::new()
    .with_field("email", email_path())
    .with_field("address.street", street_path())
    .with_field("lines[0].quantity", first_line_quantity_path());

form.validator_validation().path_map(path_map).register(map_validator);
```

This example registers only `lines[0].quantity`. A diagnostic for any other row, such as `lines[1].quantity`, is unmapped and therefore routes to the form. See the collection identity limitation described under [Flattened Diagnostic Paths](#flattened-diagnostic-paths).

### Reporting Unmapped Diagnostics

The same opt-in configuration step is available on the `validator` builder, including before both string-convenience terminal methods:

```rust
use std::{cell::RefCell, rc::Rc};

let unmapped_paths = Rc::new(RefCell::new(Vec::new()));
let reported_paths = Rc::clone(&unmapped_paths);

form.validator_validation()
    .path_map(ValidatorPathMap::new().with_field("email", email_path()))
    .on_unmapped_path(move |path| {
        reported_paths.borrow_mut().push(path.to_owned());
    })
    .register_string_errors();
```

The reporter runs once per **Unmapped Diagnostic**, in flattened diagnostic order, on every validation run against the current **Form Draft**. Duplicate diagnostics at one path produce duplicate reports. Reporting does not require `Send` and does not change diagnostic routing. Without a reporter, the adapter remains silent.

## Context-Aware Validator Validation

Models validated through `validator::ValidateArgs` (derived with `#[validate(context = ...)]`) use `register_with_context`. The provider receives Dioform's `FormValidatorContext` for the current run and returns the owned external context value; the adapter passes a reference to it as the model's `ValidateArgs::Args`.

```rust
struct SignupLimits {
    minimum_password_length: usize,
}

form.validator_validation()
    .triggers([ValidationTrigger::Manual, ValidationTrigger::Submit])
    .path_map(ValidatorPathMap::new().with_field("password", password_path()))
    .register_with_context(
        |context| SignupLimits {
            minimum_password_length: match context.trigger() {
                ValidationTrigger::Submit => 12,
                _ => 8,
            },
        },
        map_validator,
    );
```

The provider runs for each validation run. String-error forms can use `register_string_errors_with_context` with the same provider shape.

## Trigger Choices And Coexistence

The builder defaults to the `validator` source, `ValidationTriggers::all()`, and an empty `ValidatorPathMap`. It therefore routes every diagnostic to the form until mappings are added. Whole-model validation can be more expensive than field-local validation, so submit-only validation is often the right first choice. Adapter errors coexist with native **Field Validation**, native **Form Validation**, submit errors, and the `garde` adapter as long as they share the same **Validation Error** type. Rerunning the adapter replaces only errors from its own source; a successful run clears its own prior errors without touching other sources.

# Choosing An Adapter

| Library | Style | Adapter | Notes |
| --- | --- | --- | --- |
| Native validators | Dioform closures returning your error type | built in | No external dependency; field- or form-local; full control over targets and triggers. |
| `garde` | Derive-based, typed model, non-mutating | `dioform-garde` | Reports a flat `garde::Path` per diagnostic; first-class context via `garde::Validate::Context`. |
| `validator` | Derive-based, typed model, non-mutating | `dioform-validator` | Nested struct/list diagnostics flattened to canonical paths; context via `validator::ValidateArgs`. |
| `validify` | Derive-based, mutates then validates | deferred | `Validify` mutation conflicts with **Form Draft** semantics; a future adapter could support only its non-mutating `Validate` trait. |
| `serde_valid`, `jsonschema` | serde / JSON-schema oriented | deferred | Center of gravity is transport or **Dynamic Form** validation rather than the compile-time **Form Model** path. |

Native validators and one or both adapters can run in the same form. Prefer native validators for field-local rules, and reach for a `garde` or `validator` adapter to reuse an existing whole-model validation definition.

# Writing A Third-Party Adapter

A new external validation library integrates as its own adapter crate, reusing the shared
`dioform-validation-adapter` support crate rather than a per-adapter trait. There is deliberately
no public, implementable adapter trait: a Dioform trait could only be implemented by Dioform's
own adapters (Rust has no cross-library "Standard Schema" interface to converge on), and the
per-library `register` bounds are irreducible (`garde::Validate<Context = ()>` versus
`validator::Validate`, and an associated-type context versus a higher-ranked generic context), so a
unifying trait would leak those bounds into every implementor. See
[ADR-0018](adr/0018-decline-public-validation-adapter-trait.md) and
[ADR-0012](adr/0012-use-a-shared-validation-adapter-support-crate.md).

The supported seam is the support crate's two data types plus the **Form Core** registration APIs. A
third-party adapter:

- depends on `dioform-core`, `dioform-validation-adapter`, and the external library only:
  never adding the external library to `dioform-core` or the `dioform` facade (ADR-0003);
- builds a `PathMap<Model>` from explicit `External Diagnostic Path` → typed **Field Path** entries,
  and resolves each diagnostic with `PathMap::target_for_path` so an **Unmapped Diagnostic** attaches
  to the form instead of being dropped;
- runs its library's validator inside a registered synchronous form validator so validation re-runs on
  each configured **Validation Trigger** against the current **Form Draft**;
- hands each diagnostic to the application mapper as a `DiagnosticView<'_, Path, Err>` (original path,
  original error, resolved `ValidationTarget`) and constructs the shared **Validation Error** through
  `FormValidationError::for_target`;
- optionally exposes an adapter-local reporter for paths whose resolved target is the form, accounting
  for any external-library convention that also uses a form target for genuine whole-model errors;
- exposes its own thin builder (`source`, `triggers`, `path_map`, `on_unmapped_path`, `register` /
  `register_string_errors` / `register_with_context` / `register_string_errors_with_context`) with
  the bounds its library requires.

A `#[derive(Form)]`-derived path map plugs into this seam directly, because a derived map
is just a `PathMap` passed to the existing `path_map(...)` builder step; it needs no adapter trait.
