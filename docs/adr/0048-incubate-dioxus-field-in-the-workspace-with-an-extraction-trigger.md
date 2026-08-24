# Incubate dioxus-field in the workspace with an extraction trigger

> **Completed 2026-08-24.** The extraction trigger fired as specified: the crate was extracted with
> history to [sagikazarmark/dioxus-field](https://github.com/sagikazarmark/dioxus-field) and published
> to crates.io as 0.1.0 before any public-facing event. Dioform now consumes the published crate as an
> ordinary dependency, and the independence guardrails below are enforced by the standalone repository
> rather than by this workspace.

The form-library-agnostic **Field Convention** crate will incubate at `crates/dioxus-field` as a Dioform
workspace member. Workspace hosting keeps early changes reviewable beside their first producer and
consumer, but it is temporary incubation rather than permanent Dioform ownership.

## Keep the crate independent while incubating

`dioxus-field` has zero dependencies on Dioform crates, including dev-dependencies. Its own tests must
compile and pass without Dioform; tests that exercise the integration between the two belong in
`dioform-integration-tests`. This keeps the crate usable by a bare Dioxus signal and prevents its test
architecture from quietly introducing a form-library dependency.

The crate owns its version, `README.md`, and `CHANGELOG.md` rather than inheriting Dioform's release
identity. Its public API and public documentation use only form-library-agnostic **Field Convention**
language. They contain no Dioform vocabulary, model assumptions, submission behavior, or knowledge of
all fields in a form.

Donatability to `dioxus-primitives` is a standing design constraint. APIs, documentation, tests, and
repository-local tooling must remain suitable for an upstream home rather than depending on Dioform's
internal layout or release process.

## Extract before either public-facing event

Incubation ends before the crate is public-facing. The full crate history will be extracted to a
standalone repository with `git subtree split` or an equivalent history-preserving mechanism before
either of these events:

- publishing `dioxus-field` 0.1 to crates.io;
- opening the formal upstream proposal.

The order is therefore fixed: incubate in this workspace, extract with history, then publish or propose
upstream. Neither public event may make the temporary workspace location the crate's de facto permanent
home.
