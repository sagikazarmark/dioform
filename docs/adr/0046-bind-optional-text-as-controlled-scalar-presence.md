# Bind optional text as controlled scalar presence

`Option<String>` input will use a dedicated controlled `OptionalTextBinding`, not an infallible
`ParsedTextBinding`. Empty rendered input writes `None`; every non-empty string, including a
whitespace-only string, is preserved unchanged and writes `Some(value)`. `use_optional_date` and
`use_optional_number` remain parsed bindings because their non-empty input can genuinely fail typed
conversion.

## Presence does not need a parsed binding

An optional text control has total Input Parsing for every rendered string. Registering parsed
binding state for it would make an infallible presence decision pay the Raw Input State, Parse
Error, Parse Blocker, reset, reinitialization, and unmount lifecycle of a conversion that cannot
fail. It would also publish `parse_error()` on a surface where no parse error is possible.

The terminology has two levels: converting rendered input into a typed Field value is **Input
Parsing** in Dioform's domain vocabulary, while this presence decision is not a fallible parse and
does not make the control a `ParsedTextBinding`. The latter is the API distinction this decision
pins down.

The dedicated controlled binding instead writes the `Option<String>` Field Path through the normal
user update path. It keeps touched state, value-change validation, listeners, selectors, and blur
behavior without creating a second kind of presence state.

## Empty is the absence sentinel

The helper does not trim. Trimming during controlled input can eat characters while the user types,
and typed normalization remains an application step or Form Listener concern under
[ADR-0017](0017-decline-whole-model-schema-coercion.md). Only the exact empty string means absence.

`None` and `Some("")` therefore both render as `""`. `typed_value()` honestly reports the current
`Option<String>`, but the next empty input always writes `None`; `Some("")` is deliberately
unreachable through this binding. This collapse gives the rendered control one unambiguous absence
sentinel instead of inventing a second UI state for an empty present string.

## This does not reverse the optional-record ratchet

`FieldPath::or` addresses inner Fields of an optional record by materializing a caller-supplied
fallback, so [ADR-0021](0021-traverse-optional-fields-with-a-named-path-combinator.md) correctly
describes that operation as a one-way ratchet. Optional scalar helpers do not traverse or
materialize an inner value. They write the `Option`-typed scalar path directly, which
`docs/optional-fields.md` already identifies as the sanctioned way to set and clear presence.
Consequently `""` to `None` and non-empty input to `Some(value)` is total and reversible.

Rendered reads remain neutral while `typed_value()` returns `Option<String>`, following
[ADR-0022](0022-represent-an-absent-binding-target-in-the-return-type.md). No accessor panics.
