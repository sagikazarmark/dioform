# Decline a public, implementable validation-adapter trait

Dioform will not promote the `dioform-validation-adapter` support crate into a public,
implementable adapter *trait*: the "Standard Schema analog" evaluated in
[issue #148](https://github.com/sagikazarmark/dioform/issues/148). The support crate stays a
pair of shared **data types**, and that pair (plus the **Form Core** registration APIs) is the
public extension point for third-party adapters. This extends
[ADR-0012](0012-use-a-shared-validation-adapter-support-crate.md) to the "make the seam public"
question rather than reversing it.

## What a trait could and could not unify

The per-adapter pipeline has three parts. Two are already shared and already public in the 142-line
support crate: `PathMap<Model>` (external-path → `ValidationTarget` resolution, unmapped-attaches-to-form)
and `DiagnosticView<'a, Path, Err>` (the borrowed path/error/target view handed to the mapper), joined
by `FormValidationError::for_target` in the core. The third (running the external validator and
iterating its diagnostics) is library-specific by construction, and it is the part a trait would have
to absorb to deliver "implement one trait, get an adapter."

It cannot absorb it without leaking. The two first-party `register` surfaces do not share a bound:

- Non-context: `dioform-garde` requires `Model: garde::Validate<Context = ()>`;
  `dioform-validator` requires `Model: validator::Validate`: different traits, not one trait
  parameterized differently.
- Context: garde's context is an **associated type** (`Model::Context`, so `register_with_context`
  takes two type parameters); validator's is a **generic method parameter** under a higher-ranked
  bound (`Model: for<'args> validator::ValidateArgs<'args, Args = &'args Context>`, so it takes three).

A single trait method spanning both would have to expose both bound styles in its own signature, so
the bound leaks into every implementor and every caller: the exact "more indirection than the
duplication it removes" outcome ADR-0012 predicted, now confirmed at the signature level. The validator
also runs *inside* the registered form-validator closure on each trigger against the current draft, so
it cannot be pre-run and handed to a generic registration step; the bound-carrying closure is
irreducibly per-library.

## Why the payoff is internal-only

TanStack Form's Standard Schema support is valuable because Zod, Valibot, ArkType, and Effect Schema
all implement one *community* interface, so a single integration reaches the whole ecosystem. Rust has
no such interface. A Dioform adapter trait would be implemented only by Dioform's own adapters,
so its entire payoff is internal code-dedup plus a "cleaner extension point", not ecosystem reach. The
dedup on offer is small: the shared bridge is already extracted, and what remains duplicated is the
thin builder scaffolding (`.source()`, `.triggers()`, `.path_map()`), not the bound-specific `register`
bodies a trait cannot fold together anyway.

## What we do instead

Keep the support crate's two data types as the supported, public "build your own adapter" seam, and
document it as such (`docs/validation-adapters.md`): a third-party adapter depends on
`dioform-core` + `dioform-validation-adapter` + its library, reuses `PathMap` and
`DiagnosticView`, runs its library's validator inside a registered sync form validator, resolves each
diagnostic's target through `PathMap`, and maps it via `FormValidationError::for_target`: exactly what
the two first-party adapters do. `#[derive(Form)]`-derived path maps
([#138](https://github.com/sagikazarmark/dioform/issues/138)) plug into this seam without a
trait, because a derived map is just a `PathMap` fed to the existing `.path_map(...)` builder step.

## When to revisit

Reopen if either condition changes: (1) a Rust community schema interface analogous to Standard Schema
emerges, making external ecosystem reach real rather than internal-only; or (2) enough adapters exist
(three or more) that the *builder scaffolding* duplication (not the bound-specific register bodies)
demonstrably dominates and a trait over just that scaffolding earns its keep. Neither holds today.
