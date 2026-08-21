# Validation Adapters

Dioform keeps external validation libraries outside the **Form Core** and the Dioxus **Facade Crate**. A **Validation Adapter Crate** maps an external library's diagnostics into the form's shared **Validation Error** type through normal **Form Validation** APIs.

There are two first-party adapter crates:

- `dioform-garde` for the [`garde`](https://docs.rs/garde) crate.
- `dioform-validator` for the [`validator`](https://docs.rs/validator) crate.

Both are renderer-agnostic, depend only on `dioform-core` plus their external library, and register a synchronous form validator on `FormCore`. Neither adds its external library to `dioform-core` or `dioform`. They are intentionally separate but share exact-path mapping, classified route outcomes, mapper-facing diagnostic views, and configuration issue types through `dioform-validation-adapter`; **Form Core** supplies the typed live collection rule and rules-aware registration APIs. Each adapter keeps its library-specific path matching, diagnostic iteration, and builder (see [ADR-0012](adr/0012-use-a-shared-validation-adapter-support-crate.md) and [ADR-0043](adr/0043-resolve-collection-diagnostic-targets-against-current-identities.md)).

## Dependencies

Applications own their `garde` feature choices. The adapter depends on `garde` with `default-features = false` and does not enable `garde/full` or `garde/derive` by default.

Choose the derive and rule features your application needs:

```toml
[dependencies]
dioform-core = "0.2"
dioform-garde = "0.2"
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

Path matching is exact and uses the canonical `garde::Path::to_string()` representation. Exact mappings are for structurally static targets. A `FieldPath` that captures one **Collection Item Identity** is ineligible as an exact target: the adapter ignores it for routing, reports it through `configuration_issues()`, and never attaches a new diagnostic to the captured identity.

Use a live rule for collection rows. `GardeCollectionRowMatcher::new` takes the named path components before and after exactly one numeric row index; matching reconstructs a public `garde::Path` and compares it structurally, so an index is not confused with a string key containing digits or brackets.

```rust
use dioform_garde::{GardeCollectionRowMatcher, GardePathMap, GardeValidationExt};

form.garde_validation()
    .path_map(GardePathMap::new().with_field("customer.email", customer_email_path()))
    .collection_row_descendant(
        GardeCollectionRowMatcher::new(["lines"], ["description"]),
        lines_path(),
        line_description_path(),
    )
    .expect("the collection and descendant paths must have static identities")
    .register(map_garde);
```

This rule maps `lines[0].description`, `lines[1].description`, and later rows to the current logical items at those indices on each validation run. For a diagnostic attached to the item value itself, use
`collection_row_item(matcher, collection)`, for example with
`GardeCollectionRowMatcher::new(["tags"], std::iter::empty::<&str>())`. The resulting exact
item-value error is available from the matching ordinary
[`CollectionItemBinding`](collection-fields.md#item-root-validation-errors) through
`validation_errors()` and its visible-error variants; descendant errors remain on their own field
bindings.

### Routing And Reporting

Routing has this precedence for every diagnostic:

1. One eligible exact static mapping wins, even if a collection rule also matches.
2. A captured collection-item exact mapping is ineligible and cannot win. Exactly one matching collection rule resolves its emitted row index against the current **Collection Item Identity** order.
3. More than one matching rule produces `CollectionValidationTargetResolutionFailure::AmbiguousMatchingRules { match_count }`. One matching rule whose row is absent produces `CollectionValidationTargetResolutionFailure::MissingRow`.
4. No eligible exact mapping and no matching collection rule is an **Unmapped Diagnostic**.

An ineligible exact mapping can therefore fall through to a live rule, or become an **Unmapped Diagnostic** if no rule matches. Ambiguity, a missing row, and a true miss all preserve the diagnostic at form scope with `ValidationTarget::form()`; none drops it, guesses correspondence, or targets a retired identity. Under the default **Error Visibility** policy, a form-scoped error is invisible before a submit attempt but still blocks **Submit Availability**.

The mapper can inspect `diagnostic.route_provenance()`, which first-party adapters populate with one of `DiagnosticRouteProvenance::ExactStaticTarget`, `CollectionValidationTargetRule`, `CollectionValidationTargetResolutionFailure(...)`, or `UnmappedDiagnostic`. This **Diagnostic Route Provenance** exists only in the `GardeDiagnostic` passed to the mapper. Copy it into the application's **Validation Error** if it must survive mapping; it is not stored in `ValidationTarget` or core validation state. `diagnostic.target().is_form()` alone does not distinguish a collection resolution failure, an unmapped path, or a genuinely whole-model diagnostic.

Use `on_unmapped_path` for true misses and `on_collection_resolution_failure` for matched collection paths that could not select one current target:

```rust
use std::{cell::RefCell, rc::Rc};

let unmapped_paths = Rc::new(RefCell::new(Vec::new()));
let reported_paths = Rc::clone(&unmapped_paths);

form.garde_validation()
    .collection_row_descendant(
        GardeCollectionRowMatcher::new(["lines"], ["description"]),
        lines_path(),
        line_description_path(),
    )
    .expect("static collection paths")
    .on_unmapped_path(move |path| {
        reported_paths.borrow_mut().push(path.to_string());
    })
    .on_collection_resolution_failure(|path, failure| {
        eprintln!("could not route {path}: {failure:?}");
    })
    .register_string_errors();
```

Each configured reporter runs once per applicable diagnostic, in `garde` report order, on every validation run against the current **Form Draft**. `on_unmapped_path` does not report `garde`'s genuinely whole-model empty path; `on_collection_resolution_failure` receives `&CollectionValidationTargetResolutionFailure`. Both callbacks are optional, require no `Send`, have no validation or routing side effects, and leave the same form-scoped fallback in place. Without them, the adapter remains silent.

Before a terminal registration method consumes the builder, `configuration_issues()` returns all statically detectable `ValidationAdapterConfigurationIssue`s: every `IneligibleExactTarget` from the path map and every `DuplicateCollectionRule` detected from equal adapter matchers. Inspection has no validation side effects, and issues do not make registration fallible; runtime ambiguity still follows the classified form fallback.

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

Ordering is deterministic: `validator` stores fields in a `HashMap`, so the adapter sorts field keys, iterates list indices in ascending order, and preserves each field's error-vector order. `ValidatorCollectionPath` retains the structural list index while the tree is traversed, and `ValidatorCollectionTargetRule` resolves that index to the current Dioform **Collection Item Identity**. The adapter does not parse a wildcard string from the flattened display path.

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

Path matching is exact against the canonical flattened path and uses the same precedence and fallback classification described for `garde`: eligible static exact mapping, one live collection rule, collection resolution failure, then true miss. The adapter never treats rendered **Field Names**, serde names, `validator` field keys, or Rust field names as implicit validation addresses.

```rust
use dioform_validator::{
    ValidatorCollectionPath, ValidatorCollectionTargetRule, ValidatorPathMap,
    ValidatorValidationExt,
};

let quantity_rule = ValidatorCollectionTargetRule::descendant(
    ValidatorCollectionPath::new(["lines"], ["quantity"]),
    lines_path(),
    line_quantity_path(),
)
.expect("the collection and descendant paths must have static identities");

form.validator_validation()
    .path_map(
        ValidatorPathMap::new()
            .with_field("email", email_path())
            .with_field("address.street", street_path()),
    )
    .collection_target_rule(quantity_rule)
    .register(map_validator);
```

`ValidatorCollectionPath::new(before_index, after_index)` matches exactly one structural `ValidationErrorsKind::List` index between the named components. The rule above therefore resolves every current `lines[index].quantity`, including rows appended after registration. To attach a matched row diagnostic to the item value rather than a descendant, pass the same structural matcher to `ValidatorCollectionTargetRule::item`.

### Routing And Reporting

`ValidatorDiagnostic::route_provenance()` exposes the same ephemeral `DiagnosticRouteProvenance` during a custom mapper call. The reporters use the flattened `&str` path:

```rust
use std::{cell::RefCell, rc::Rc};

let unmapped_paths = Rc::new(RefCell::new(Vec::new()));
let reported_paths = Rc::clone(&unmapped_paths);

form.validator_validation()
    .collection_target_rule(quantity_rule)
    .on_unmapped_path(move |path| {
        reported_paths.borrow_mut().push(path.to_owned());
    })
    .on_collection_resolution_failure(|path, failure| {
        eprintln!("could not route {path}: {failure:?}");
    })
    .register_string_errors();
```

`on_unmapped_path` runs only for **Unmapped Diagnostics**. `on_collection_resolution_failure` runs only for `AmbiguousMatchingRules` or `MissingRow`; it receives `&CollectionValidationTargetResolutionFailure`. Each optional callback runs once per applicable diagnostic in flattened order, so duplicate diagnostics at one path produce duplicate reports. Neither requires `Send` or changes validation and routing. `configuration_issues()` aggregates ineligible exact targets and duplicate `ValidatorCollectionPath` matchers before registration, just as on the `garde` builder.

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

# Live Collection Rule Lifecycle

Both adapters register their core `CollectionValidationTargetRule<Model>` values through
`FormCore::register_sync_form_validator_for_triggers_with_collection_target_rules` (or the all-trigger
`register_sync_form_validator_with_collection_target_rules`). Registration prepares each addressed collection's identity state before validation; a run resolves against the **Form Draft** and identity order paired in its `FormValidatorContext`, not against an index-to-identity snapshot captured when the adapter was configured.

The prepared order follows all coordinated collection transitions:

- append and insert mint identities; remove retires the removed identity;
- move and swap reorder existing identities, while `replace_collection_item` preserves the replaced logical item's identity;
- clear leaves no current rows;
- reset and containing-field reset restore baseline values and baseline identities;
- reinitialization mints a fresh baseline and current identity sequence;
- cardinality-valid `restore_state_snapshot` restores the snapshot's full draft and identity order atomically;
- generic `set_field` replacement of the collection or a containing field treats every reached current item as displaced, clears its item-scoped state, and mints a fresh current sequence without positional or value matching.

The per-collection counter never rewinds, so generic replacement does not reuse retired identities; baseline identities remain reserved so reset can restore them. These semantics, including submit-validation currency, are specified in [ADR-0043](adr/0043-resolve-collection-diagnostic-targets-against-current-identities.md).

The shipped rules are synchronous-only and support direct or named-struct-composed `Vec<Item>` collection paths. A rule can target the item value (`CollectionValidationTargetRule::item`) or one static descendant (`CollectionValidationTargetRule::descendant`). Collections nested inside collection items are not representable by the current identity model and are unsupported; future async routing must capture identity sequences with its owned **Form Snapshot**.

`FieldPath::direct(identity, field_name, get, get_mut)` is a semantic trust boundary. Rule constructors reject identities that visibly capture a runtime **Collection Item Identity**, and `PathMap` marks such exact mappings ineligible, but Dioform cannot prove that manually supplied accessors truthfully implement the structurally static identity supplied by the application. Derived or honestly composed static paths preserve the routing guarantees; a lying `FieldPath::direct` can violate them.

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

The supported seam is public data and routing functions rather than a public adapter trait. It includes
`PathMap<Model>`, `ExactPathLookup`, `DiagnosticRoute`, `DiagnosticRouteProvenance`,
`CollectionValidationTargetResolutionFailure`, `DiagnosticView`, `route_diagnostic`, and the
configuration issue types from `dioform-validation-adapter`; `CollectionValidationTargetRule<Model>`
and `register_sync_form_validator_with_collection_target_rules` /
`register_sync_form_validator_for_triggers_with_collection_target_rules` come from **Form Core**. A
third-party adapter:

- depends on `dioform-core`, `dioform-validation-adapter`, and the external library only:
  never adding the external library to `dioform-core` or the `dioform` facade (ADR-0003);
- builds a `PathMap<Model>` for structurally static exact entries, calls
  `PathMap::exact_target_for_path`, and owns the external syntax that extracts one row index for each
  adapter matcher paired with a core `CollectionValidationTargetRule<Model>`;
- registers those durable rules with
  `register_sync_form_validator_for_triggers_with_collection_target_rules`, resolves them through
  `CollectionValidationTargetRule::resolve`, and passes the resulting candidates to
  `route_diagnostic` so precedence and fallback classification match first-party adapters;
- hands each diagnostic to the application mapper as a
  `DiagnosticView::from_route(path, error, route)` and constructs the shared **Validation Error**
  through `FormValidationError::for_target`;
- optionally exposes separate reporters selected from `DiagnosticRouteProvenance` for true misses and
  `CollectionValidationTargetResolutionFailure`, without inferring either from a form target;
- aggregates `PathMap::configuration_issues()` with duplicate issues detected by its own matcher
  grammar;
- exposes its own thin builder (`source`, `triggers`, `path_map`, collection-rule registration,
  `on_unmapped_path`, `on_collection_resolution_failure`, `configuration_issues`, `register` /
  `register_string_errors` / `register_with_context` / `register_string_errors_with_context`) with
  the bounds its library requires.

A `#[derive(Form)]`-derived path map plugs into this seam directly, because a derived map
is just a `PathMap` passed to the existing `path_map(...)` builder step; it needs no adapter trait.
