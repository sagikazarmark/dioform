# Blur the multi-select option the user left

`MultiSelectOptionBinding::on_focus_exit` marks the **Collection Field** and *its own* selected value.
It does not delegate to `MultiSelectBinding::on_focus_exit`, which would fan one gesture out across every
selected value. The dispatch count falls from `1 + N` to at most `1 + 1` as a consequence of marking
the right **Fields**, not as a separate concession.

## The binding already knew which value blurred, and threw it away

`MultiSelectOptionBinding` holds the option's `value` and exposes `selected_item()`. It is also the
binding an application wires a checkbox to, through `onblur()`. Every fact needed to name the value
whose control lost focus was in hand at the call site:

The former option-level path delegated to the group after discarding its value.
The group then marked the **Collection Field** and looped over the whole selection, because by that
point the value was gone and every selected value looked
equally likely. One `onblur` on one checkbox produced `1 + N` blur dispatches, `N` of them naming
values the user never focused.

The amplification was measurable on `use_form_blur_listener` today. It was not reachable from
`use_field_blur_listener`, whose filter is exact and whose registrations are always `Static` — that
surface's dispatch at the item site could match nothing. The listener count was the reported symptom;
it was not the defect.

## The defect is metadata, and it was visible in the UI

`ErrorVisibilityPolicy::BlurOrSubmit` was the default and read the **Field**'s own blurred flag.
Marking every selected value blurred therefore made every selected value's stored error visible, and
`aria-invalid` true, on controls the user had never focused. Selecting three topics, one of which
fails an `item_validator`, and leaving a *different* checkbox announced an error on the invalid one.
The current `CommitOrSubmit` default added by
[ADR-0051](0051-reveal-field-errors-after-commit-without-marking-fields-blurred.md) preserves that
exact blurred meaning while separately considering exact committed metadata.

That is the harm [ADR-0028](0028-match-listener-reach-to-what-each-event-asserts.md) measured when
it refused to aggregate the blurred flag upward — "a child's stored error **visible on an input the
user never focused**" — arriving from the other direction. `CONTEXT.md` states the rule without a
direction: a **Blurred Field** is one that lost focus, "and only the **Field** the user left did".
The fan-out marked `N + 1`.

A second symptom followed from the same marking. Item metadata is keyed by **Collection Item
Identity**, so deselecting and reselecting a value mints a fresh one
([ADR-0025](0025-mint-collection-item-identities-from-a-never-rewinding-counter.md)) with an unblurred
flag. The same invalid value oscillated between showing its error and hiding it, driven by selection
history rather than by anything the user did with focus.

## Commit preserves exact per-value validation

The former fan-out did two jobs: it marked fields blurred and ran validation inline. Commit and Focus
Exit are now separate. `MultiSelectOptionBinding::on_commit` explicitly names the collection and its
selected item for validation, while `on_focus_exit` marks those same exact fields. The strict
collection clause means a collection Commit alone does not reach an item:
`FieldAncestry` relates a static identity to a `CollectionItem` only when it is a *strict* ancestor
of the collection component, so `topics` does not relate to `topics[item]`. That strictness is
deliberate and load-bearing
([ADR-0020](0020-derive-field-ancestry-from-identity-paths.md)), and it means a collection-level Commit
selects no item validators at all.

Measured, with `ValidationMode::on_commit()` and one failing `item_validator`:

```
collection Commit only        -> stored=0, visible=0 on every item   (validator never ran)
Commit the selected item      -> that item stored=1, visible=1; siblings untouched
```

Naming the one selected value on Commit keeps its validator running exactly where the user can act
on the result. Naming it separately on Focus Exit keeps blurred metadata and listeners exact without
running validation again.

## One control, two addresses — and why that is not upward aggregation

A multi-select option control is the rendered control for the **Collection Field** and for one
selected value at the same time. It carries the collection's **Field Name** — every option in the
group renders `name = "topics"`, which is what makes the group one HTML form control — while
representing the value `topics[i]` that the library stores an identity, metadata, and validation
errors for. Both addresses are exact for that element.

Marking both is therefore not the subtree aggregation ADR-0028 rejected. Nothing is being inferred
about a **Field** from an event that happened somewhere else inside it: one DOM element has two
**Field Identities**, and the blur is a fact about both. The rule in `CONTEXT.md` survives unamended
— the user left one control, and both **Fields** it addresses lost focus.

The two other readings were considered:

- **Item only.** The most literal reading, and it leaves a rendering that binds one listbox to the
  whole **Field** — `MultiSelectBinding::on_focus_exit`, which names no value — marking nothing, so a
  collection-scoped validator's error never becomes visible before a submit attempt. That is a
  regression for an entry point that is correct today.
- **Collection only.** The radio-group analogue: one **Field**, many inputs, blur marks the
  **Field**. It is the tidier domain story and it is what the strict collection clause would have to
  be breached to make workable, because per-item validators would stop running. Breaching it to fix
  a blur bug would relate every static descendant of a collection path to its items, which is what
  `a_static_descendant_of_a_collection_path_does_not_relate_to_its_items` forbids.

## What each Focus Exit entry point marks

- `MultiSelectOptionBinding::on_focus_exit` — the **Collection Field**, plus its own selected value when
  the option is selected. An unselected option has no item, so it marks the collection alone. This is
  the DOM-driven path.
- `MultiSelectBinding::on_focus_exit` — the **Collection Field** alone. It names no value, so it marks no
  value. This is the entry point for a rendering that binds one control to the whole **Field**.
- `MultiSelectItem::on_focus_exit` — that value alone. It is the per-chip entry point
  for the chip, listbox, and command-palette renderings `docs/collection-fields.md` promises.

Each is exact about what its caller named, which is the property the fan-out lacked.

## The empty identity segment is a consequence, not a defect

`FieldIdentity::as_str()` returns an empty segment for a collection item value identity, which is
documented on the method and is load-bearing: within an item the empty segment is the item root, and
so an ancestor of every non-empty sibling segment, which is what makes writing a whole item value
reach its child fields. Three of the four dispatches reporting `""` was the fan-out's arithmetic, not
a broken identity.

After this change at most one item identity is dispatched per gesture, and it is the one whose value
the caller named, so the caller can always recover it. The general question — that a listener's
`field_identity()` may be item-relative, and that `as_str()` is the wrong accessor to reach for — is
[#38](https://github.com/sagikazarmark/dioform/issues/38)'s residue, already live on the value
surface, and already answered by its convention: assert through the identity's collection and item
accessors, never through its string form.

## Serialized metadata changes meaning

Per-item blurred flags in a `FormStateSnapshot` currently mean "the group was blurred". After this
change they mean "this value's control was left". The wire format is unchanged and the meaning is
not, which is exactly what `FORM_STATE_SERIALIZATION_VERSION` exists to reject, so it is bumped.
`docs/form-state-serialization.md` already frames snapshots as same-deployment transfer, and multi-
select item metadata is in any case the least durable state in the snapshot, since it does not
survive a value being deselected and reselected.

## What this narrows

ADR-0028 decided listener **reach** — which registered listeners a dispatched event reaches. This
decides dispatch **count** — how many events one gesture produces. They are different axes, and this
is not the per-surface exception ADR-0028's "one criterion, applied to all four" refused: no listener
filter changes here. ADR-0028's "Blur metadata stays exact" is narrowed only in that its argument is
restated without a direction, which is how `CONTEXT.md` already had it.
