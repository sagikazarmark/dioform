# Decline whole-model schema coercion

Dioform will not add a whole-model value-coercion step: the
[`parseValuesWithSchema`](https://tanstack.com/form/latest/docs/reference/classes/formapi#parsevalueswithschema)
analog evaluated in [issue #150](https://github.com/sagikazarmark/dioform/issues/150).
Coercion stays a per-field, per-binding concern owned by the **Dioxus Adapter**; the **Form Core**
continues to store only typed values. This records the deliberate divergence so the TanStack Form
parity surface has a decision rather than an open gap.

## Why TanStack needs it and Dioform does not

TanStack Form's values are a loosely typed JavaScript object, and a Standard Schema both validates
*and* transforms, so `parseValuesWithSchema` produces a coerced, reshaped **output** value distinct
from the raw input values (trimmed strings, `string -> number` coercion, defaulted fields). The
raw-versus-output split is real there because the input object is untyped.

A Dioform **Form Model** is a compile-time Rust type. Fields already hold typed values, so there
is no untyped input object to coerce into a typed output: the output type *is* the model. The single
concern TanStack folds into one schema pass, Dioform has already separated: **Input Parsing**
converts rendered input into the typed value (Raw Input State + Parse Error, adapter-owned), and
**Field / Form Validation** checks the typed value. See `CONTEXT.md` ("Input Parsing determines
whether rendered input can become a typed field value; validation determines whether a typed value is
acceptable") and `docs/input-helpers.md`.

## Why adding it would be a regression, not a feature

A whole-model coerce step would reintroduce a *second* parsing mechanism overlapping the per-field one
already owned by parsed bindings. **Parse Errors** are binding-level and distinct from **Validation
Errors**; **Parse Blockers** are mount-scoped and lifecycle-bound to their bindings. A model-wide
coerce pass would sit outside that lifecycle and compete with it for ownership of "turn input into a
typed value," splitting one clear boundary into two overlapping ones. This is the same failure mode
[ADR-0011](0011-do-not-extract-a-chain-executor-module.md) rejects: a construct that relocates or
duplicates a mechanism without concentrating it is not a real seam. It also runs against the decision
that Standard Schema is deliberately not the primary API
([issue #148](https://github.com/sagikazarmark/dioform/issues/148)), and value
*transformation* is the part of Standard Schema least aligned with a statically typed model.

## The one legitimate slice, and where it already lives

The plausible non-declined case is **normalizing an already-typed value**: trimming or canonicalizing
a `String` field before submit. That needs no schema and no whole-model pass: it is expressible today
as an ordinary application step or a value-replacement **Form Listener** (`docs/form-listeners.md`),
operating on the typed value that already exists. Per-field parsed bindings plus an optional
normalization listener cover the real cases.

If a typed-normalization *convenience* ever earns its keep, it should be scoped as a minimal,
explicitly typed surface over existing field values, not a schema-driven coerce mechanism, and not a
reintroduction of a raw-versus-output type. Recording that boundary here keeps a future ergonomic
helper from being mistaken for adopting `parseValuesWithSchema`.
