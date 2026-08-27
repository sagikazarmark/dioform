# Widen Error Visibility outward from the Field an error is attached to

**Error Visibility** for an error attached to a **Field** `F` will consider whether `F`, or any
**Field** contained by `F`, has been blurred — or touched, under the touched-scoped policy. The
widening is directional: a blur above `F` never makes `F`'s error visible. Blurred and touched
**metadata** stay exact, and the serialized snapshot is unchanged by this decision. The committed
arm introduced later by [ADR-0051](0051-reveal-field-errors-after-commit-without-marking-fields-blurred.md)
uses the same directional reach while keeping committed metadata exact.

## The defect is a contradiction between two rules about one event

[ADR-0020](0020-derive-field-ancestry-from-identity-paths.md) widened validator *selection* to span
**Field Ancestry**: `sync_field_keys_for_chain` filters on `FieldAncestry::relates`, so committing
`invoice.customer.name` runs a validator registered on `invoice.customer` and stores its verdict.
That verdict is load-bearing — `has_validation_errors` reads stored errors directly, so
`submit_availability` reports `SubmitBlocker::ValidationErrors`.

`should_show_validation_errors` still reads exact interaction metadata for the **Field** the error is
attached to, and the container was never committed. One event is therefore simultaneously sufficient to
produce a verdict that blocks submit and insufficient to display it. That is not a gap in coverage;
it is two rules about the same event giving contradictory answers, and the library ships both.

Three repairs exist. Two are closed.

**Narrowing selection back to exact identity** is closed. ADR-0020 commits to the widened filter, and
ADR-0028 narrows only the listener half — "the predicate itself is untouched". The consequence is also
unacceptable: Commit enters through leaf bindings, and a leaf-input UI never writes the containing
object's path, so under exact-identity selection a validator registered on a container would never run
from leaf-driven user interaction at all. (Programmatic container writes and
`MultiSelectBinding::on_commit` still would, but neither goes through the ancestry relation.)

**Aggregating the blurred flag upward** is closed by
[ADR-0028](0028-match-listener-reach-to-what-each-event-asserts.md), which measured it and recorded
three grounds: it makes a child's stored error visible on an input the user never focused and starts
announcing `aria-invalid` for it; it forks the meaning of serialized metadata; and it redefines
**Blurred Field** into "focus has been inside this subtree".

**Widening the visibility predicate alone** is the remaining door. That it is the only one left is
also the answer to a question the fix would otherwise have to argue separately: the change belongs in
the predicate, not in metadata. `is_field_blurred` and `is_field_touched` keep answering for exactly
the **Field** that blurred, and `FORM_STATE_SERIALIZATION_VERSION` is not bumped —
[ADR-0031](0031-blur-the-multi-select-option-the-user-left.md) has just paid that cost and this
decision does not pay it again.

## The direction comes from ADR-0028's accessibility constraint, not from its criterion

ADR-0028 disclaims this question by name: "That is a visibility-policy question and has its own
issue." So its *criterion* — match reach to what the event asserts — is not the warrant for this
decision, and deriving the rule from it would reverse that ADR's own scope statement.

Its *rejection of upward flag aggregation* is a different passage and is not disclaimed. It rules out
a mechanism, and the deferral of a criterion does not retract a constraint on the solution space. The
harm it measured is the reason this widening is downward-only: the symmetric predicate would make a
leaf's stored error visible because a **Field** containing it was blurred, which is the same
observable outcome the flag aggregation was rejected for, and which ADR-0031 has just finished
removing from the multi-select path. Downward-only is the largest widening that avoids it.

Two earlier justifications were considered and are recorded because both are tempting and both are
false:

- **"Visibility follows opportunity to fix."** `ValidatorContext::form` hands *field* validators the
  whole draft, so a validator registered on `invoice.customer` may check `invoice.total`, and its
  error is not fixable from inside `invoice.customer` at all. Even within the subtree, blurring
  `customer.name` reveals an error about `customer.tax_id`. The premise is false, and it is the
  premise `CONTEXT.md` currently gives for the behaviour this decision changes.
- **"`BlurOrSubmit` is a predicate about a rendered control, which a container does not have."**
`commit_field` is unbounded over `Value`, and `use_select`, `use_radio_group`,
  `use_parsed_text_with` and the multi-select bindings all commit composite paths, so the premise holds
  for exactly two container shapes and fails for seven hook families. It also imports vocabulary
  `CONTEXT.md` bans for **Field** — "*Avoid*: Input, control, widget" — into a `dioform-core` rule
  that [ADR-0001](0001-renderer-agnostic-core-dioxus-adapter-and-derive-macro.md) keeps
  renderer-agnostic, and it would equally condemn `ValidationTarget::Form`, whose identical
  stored-blocking-invisible shape is deliberate.

The **Blurred Field** dialogue in `CONTEXT.md` still ends "none of them starts showing **Validation
Errors** the user never had a chance to fix". That sentence states the first of the two rejected
premises as domain law, and it is superseded by this decision. Amending it is outstanding.

## The collection component takes ancestor-or-equal

The strict static-to-collection clause is load-bearing for writes and stays so. Inherited unchanged by
this predicate it produces an inversion: committing `invoice.lines[2].product` makes an error attached to
`invoice` visible and one attached to `invoice.lines` invisible — the nearer registration receiving
strictly less than the further one, which ADR-0028 called unshippable.

It is not vacuous. `validate_collection_item_field_commit` also runs the form chain, and
`sync_form_ids_for_chain` applies no field filter, so a form validator or an adapter **Path Map** can
attach an error to `invoice.lines` from a row-leaf Commit. Measured, that error is stored, invisible,
and submit-blocking. `CollectionBinding` has no `on_commit`, so a repeater's collection field may
never be committed and the error is invisible until a submit attempt — permanently, for the
container shape where collection-level verdicts ("at least one line", "no duplicates") are most
idiomatic. Keeping it strict would also reward attaching such errors to `invoice` instead.

The widening is upward-only and cannot leak downward: the clause matches the collection component
exactly, so a static descendant of a collection path still does not relate to its items, and no item's
error becomes visible on another row's blur.

This clause belongs inside the directional companion predicate that
[#50](https://github.com/sagikazarmark/dioform/issues/50) introduces, not at the call sites. The
adapter already hand-copies the same disjunct once in `value_replacement_reaches`; deciding it
per-caller would make three spellings of one relation. `FieldAncestry::relates` is not modified.

## The touched-scoped policy widens too

Both arms are widened. `TouchedOrSubmit` reaches the same dead end on its own merits — a leaf Commit
stores the container's verdict and the container is never touched — so the change is not justified by
comparison with `BlurOrSubmit`.

The alternative of widening the touched arm on *blur* — "touched here, blurred inside" — was measured
and declined. A container with no control of its own is never touched directly, so that arm collapses
onto the widened blur arm and `TouchedOrSubmit` stops existing for exactly the errors this decision is
about. It also leaves an unbounded dead end under `ValidationMode::on_change`, where the container
error is guaranteed fresh and the user may never blur anything.

The cost is that a touched or Focus Exit reveal is not validation-coincident. Commit runs the chain;
Focus Exit only marks touched and blurred metadata. The native `onblur()` convenience composes Commit
before Focus Exit, but custom widgets can report them independently. `CONTEXT.md` already states the
invariant: **Error Visibility** is separate from whether validation has run. The window is bounded by
the next Commit, never indefinite.

## The predicate must not scan the whole metadata map

The widened predicate answers a subtree question on the hottest read path in the library: it is the
`include` closure for every visible-error selector and feeds `FieldAccessibility`, which components
read on every render. A naive scan of the metadata map is O(**Fields** the user has interacted with)
per call — metadata is materialized lazily, so the worst case is a fully filled-out form, which is
also when an error summary is most likely rendered.

Measured, the naive scan is not acceptable. At 1000 registered validators a whole-form
`visible_validation_errors` read goes from 42µs to 2.83ms, and a form with no error-summary component
at all still pays +5.9ms per render distributed across row components, because `field_accessibility`
builds a full error Vec to answer a boolean. The shape that settles it is the wizard, paginated or
virtualized form — every **Field** registered and interacted, few rendered — which is comfortably fast
today and becomes 4–12× slower.

The metadata map is a `BTreeMap` and `FieldIdentity`'s ordering makes a container's descendants
contiguous within each identity kind, so a two-range scan answers the question in O(descendants) and
restores parity at roughly 1.1× of today's cost. [ADR-0029](0029-create-selector-registrations-only-from-reactive-reads.md)
declined range queries over the *reactivity* map, where the bounds were not expressible as identities
and the scan ran once per write behind a dominant dedup; neither holds here, and that declination is
scoped to that map. The residual risk is that correctness depends on the declaration order of
`FieldIdentityKind` and its `CollectionItem` fields, with no compile error on a reorder — carried by a
randomized test asserting agreement with the naive scan.

The whole cost disappears once a submit has been attempted, so the regression window is
pre-first-submit only.

## What this does not fix

**The mirror direction was a separate selection defect.** This decision correctly declined to make
descendant errors visible after a container event. [ADR-0035](0035-select-commit-validators-outward-from-the-field-that-committed.md)
subsequently made Commit validator selection outward-only, so committing a container no longer runs
validators on the **Fields** it contains.

**Form scope is unchanged.** `ValidationTarget::Form` stays invisible until a submit attempt. The
reason is not that no principled test exists — form validators are selected by any **Field**'s Commit, so
mirroring selection would be one — but that a form-scoped rule has no point at which the user has
demonstrably finished, and the designed reveal is the submit attempt.

**Container errors now become visible earlier than form-scoped ones**, while `CONTEXT.md` advises
moving verdicts that must survive edits to form scope. That cliff is accepted here and named so it is
not discovered as a surprise.
