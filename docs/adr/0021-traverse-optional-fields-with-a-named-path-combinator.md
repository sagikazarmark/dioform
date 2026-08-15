# Traverse optional fields with a named path combinator

A field inside an **Optional Field** is unreachable today: `FieldPath` carries an infallible accessor
pair, and no function produces `&T` from `&None`. Applications that need to edit one flatten the
record into scalar fields and keep a DTO plus a side-channel `Signal` beside the `FormHandle` —
measured at roughly 114 lines for one form, and exactly the state the library exists to own.

Dioform will close this with a small, explicitly-named combinator on `FieldPath<Model, Option<Inner>>`
that returns an ordinary `FieldPath<Model, Inner>`, reading a caller-supplied fallback when the value
is absent and materialising a clone of that same fallback on write. It will **not** add an
`optional_group` addressing scope, and it will **not** make the accessor pair fallible.

The investigation is recorded in `docs/research/optional-field-traversal.md` (primary-source survey of
form and optics libraries), two throwaway build spikes, and eight independent reviews. This ADR records
what those settled. It rests on two foundations that had to land first:
[ADR-0020](0020-derive-field-ancestry-from-identity-paths.md), because presence toggling is a parent
write read back through child bindings and the reverse is the ordinary typing path, and the
`reset_field` fix, because a derived path is exactly the unlawful-path shape that mutated the model.

## The capability already exists; the library only has to name it

An application can build a total path through an `Option` today, on 0.2.0, using only public API:
`FieldPath::direct` with a getter that falls back to a `'static` default and a setter that uses
`get_or_insert_with`. This was verified against a real **Form Core** — `join` produces the correct
**Field Identity** and rendered **Field Name** (`counterparty.name`), editing an inner field preserves
sibling fields of the picked value, and `is_dirty`, `is_field_dirty`, `reset` and `state_snapshot` all
behave. Every binding, validator and listener works unchanged. `crates/dioform-core/tests/form_core.rs`
has been hand-building exactly this shape for `Option<Party>` all along.

The side-channel `Signal<Option<Party>>` that motivated the request is therefore removable **without
any library change** — roughly 7 lines per optional record against the measured 114. What the pattern
is not is discoverable, hard to get wrong, or free of a `'static` default the caller has to conjure.

That reframes the library's job. It is not to add an addressing axis; it is to make one already-reachable
pattern safe, correct and named. A combinator is the smallest construct that does that, and its result
is an ordinary `FieldPath`, so it composes with `join`, **Field Groups**, bindings, validators,
listeners, snapshots and submission with no new method anywhere.

## The Optional Field entry stands; no reversal is required

`CONTEXT.md` defines an **Optional Field** as one "whose nested values are *not implicitly created by
field traversal*", and the domain dialogue is explicit that the form "should not invent missing nested
values while traversing fields". That decision is not reopened here.

A bare `FieldPath<Model, Option<T>>` still refuses traversal — nothing is implicitly created *by
traversal*. The combinator produces a derived total path that the caller asked for **by name**,
supplying the default at the call site. Naming it is the whole point: opt-in materialisation the
application requested is a different act from traversal silently inventing a value, and the research
records `non` as the optics ecosystem's sanctioned form of exactly this.

This also resolves by decision, not by capability, which glossary entry governs `Option<T>`: **Optional
Field** does. **Variant Field** is unchanged, and
`crates/dioform/tests/derive_contract/fail/variant_inner_traversal.rs` stays a compile-fail contract.
The earlier claim that this generalises to enum traversal does not hold — neither spike touched an enum.

## The rejected alternatives

**Materialise-on-write inside an `optional_group` scope** — the shape the original request proposed —
does not type-check under its own stated semantics. Materialising fixes the *write*; the *read* still
has to produce `&'a Inner` from `&Model` with no `&mut` available, so
`form.text(counterparty.field(Party::fields().name()))` cannot compile against `FormHandle::text`.

**Fallible accessors in the core** (`Fn(&Model) -> Option<&Value>`) build, and pass 137/137 core tests.
The green build was bought with roughly thirteen unflagged semantic decisions — an absent text field
renders `""`, an absent checkbox `false`, an absent field validates as **Valid**,
`push_collection_item` panics — none of which produce a compile error downstream. `demo/` is not a
workspace member, so nothing checked the only real consumer. Realistic cost is about two weeks, not the
headline hunk count.

**A parallel `OptionalFieldPath` type** works and is fully green, but its duplication is permanent
rather than one-time.

**A no-op write** was rejected earlier on the grounds that the discard is unobservable. That turned out
to rest on a fixable detail (the version bump), so it is not the reason. The reason is simpler: the
combinator makes writes total, so the case does not arise on the happy path.

## One value serves both the absent read and the materialising write

The bound is `Inner: Clone`, **not** `Inner: Default`. The same value must answer the absent read and
the materialising write, or the two can silently diverge — a form that reads one default and writes
another is worse than one that cannot traverse at all. Taking the value at the call site also makes the
combinator usable for types that have no `Default`, and it keeps `#[derive(Form)]` bound-free: the bound
belongs where the caller opts in, not on every model that happens to contain an `Option`.

An honest read accessor ships alongside, yielding `Option<&Inner>`, because the combinator erases the
distinction between absent and present-holding-the-default by construction. Erasing it for the editing
path is the trade; leaving callers no way to recover it would not be.

## The derived path reuses the parent's identity

The derived path keeps the parent's **Field Identity** and rendered **Field Name**. Giving it a distinct
identity produces wrong `name=` attributes on joined paths — verified, as `counterparty_value.name`.

The aliasing this creates between the `Option`-typed path and the derived path is intended. They are two
views of one field and share touched, blurred, version and submit-error state. Each validator captures
its own typed path, so the correct closure still runs against the correct value.

This settles a question left open on the request: **Field Identity** for inner fields is stable across a
clear/ensure cycle, because presence never enters the identity at all. Under
[ADR-0020](0020-derive-field-ancestry-from-identity-paths.md) that stable identity earns **Field
Ancestry** for free — the derived path and its inner fields stand in the relation without any special
case, so presence toggling notifies the inner bindings and inner edits notify the presence reader.

## Known limits, recorded deliberately

Two consequences are inherent to `non`-shaped materialisation. They are not implementation defects; they
are the price of this design, and they are why presence work is deferred rather than cancelled.

**Validators on inner paths fire while the parent is absent.** A `required` rule on `counterparty.name`
reports `Invalid` and blocks submit for a section the user never opened. Today that is reachable only by
consulting `ValidatorContext::form()`, which reaches around the abstraction.

**Type-then-backspace leaves a phantom.** Typing one character into an inner field of an absent parent
and deleting it leaves `Some(Inner::default())`. Every field the UI renders reports clean while the form
reports dirty, and the submitted payload changes from omitted to a defaulted record.

Two mitigations were tested. Normalising at submit fixes the payload but not `is_dirty`, `reset_field`,
or observer bookkeeping. A listener that collapses to `None` on default-equality is **unsound** and must
be neither used nor documented: it cannot distinguish "the user cleared the last field" from "this
section is legitimately present and empty", so it deletes sections that were present in the baseline.

Both share one root cause — a total path erases presence, and no combinator wrapped around
`FieldPath::direct` can restore it. Documentation therefore has to state the ratchet outright:
materialisation is one-way, clearing an inner value does not un-materialise the parent, and the payload
will carry a defaulted record where the user may have expected omission.

## What is deliberately not in this decision

**Presence as a first-class, metadata-carrying concept** — modelled the way **Collection Fields** model
item existence, carrying its own identity, validation and dirty state. This is the answer to both known
limits above, and it is deferred, not cancelled. It is a separate design needing its own evidence, and
nothing here forecloses it.

**An `or_default()` form** requiring a `StaticDefault` trait plus derive support. Evaluated and ranked
second: it covers only const-constructible types, so a `HashMap` field defeats it, and it introduces a
second notion of "default" that can diverge from a hand-written `Default` impl. Revisit only if
supplying the value at the call site proves onerous in practice.

**A thread-local, `TypeId`-keyed map of leaked defaults**, so no `'static` argument is needed. It
deduplicates per *thread*, which is the wrong axis — wasm is single-threaded and SSR pays threads ×
types — it has two reachable panic paths in well-typed programs, and it hides a process-lifetime
allocation inside an innocuous-looking combinator.

**Any change to `FieldPath`'s accessor pair, and any parallel optional path type**, per the rejections
above.

**Enum and Variant Field traversal**, which stays deferred under ADR-0002's slice.

This adds no second parsing mechanism, so [ADR-0017](0017-decline-whole-model-schema-coercion.md) stays
intact: the combinator adds addressing, not coercion.
