# Collection Fields

The first **Collection Field** slice supports direct `Vec<Item>` fields on a named **Form Model** and nested direct collection paths reached through named-struct **Field Path** composition. The form owns the vector value and assigns each current item a library-owned opaque **Collection Item Identity** so metadata, errors, parse blockers, and row keys follow the logical item across insertion, removal, and reordering. Use binding `key()` helpers or `CollectionItemIdentity::key()` for rendering keys instead of inspecting numeric identity internals.

Use `FormHandle::collection(path)` to create a `CollectionBinding` for a direct `Vec<Item>` field path. Render each row as its own component, keyed by the item's **Collection Item Identity**:

```rust
#[component]
fn InvoiceEditor() -> Element {
    let form = use_form(initial());
    let lines = form.collection(InvoiceForm::fields().lines());

    rsx! {
        for item in lines.items() {
            InvoiceLineRow {
                key: "{item.key()}",
                form: form.clone(),
                item: item.identity(),
            }
        }
    }
}

#[component]
fn InvoiceLineRow(form: FormHandle<InvoiceForm>, item: CollectionItemIdentity) -> Element {
    let Some(item) = form.collection(InvoiceForm::fields().lines()).item(item) else {
        return rsx! {};
    };

    let description = item.text(InvoiceLine::fields().description());
    let quantity = use_collection_item_number(
        item.clone(),
        InvoiceLine::fields().quantity(),
    );

    rsx! {
        input {
            name: description.name(),
            value: description.value(),
            oninput: description.oninput(),
        }
        input {
            r#type: "number",
            name: quantity.name(),
            value: quantity.value(),
            oninput: quantity.oninput(),
        }
    }
}
```

`name()` on a collection item child binding is `Option<String>` (see below). Passing it straight to an rsx attribute is the intended use: Dioxus omits the attribute entirely for `None`, so a row caught mid-removal renders no `name=` rather than one that could collide with a live row.

A row that calls a collection-item hook must be a component keyed by the item identity, because the hook's parse state lives in the scope that calls it:

- A plain `fn` row helper called from the page's `for` loop runs its `use_` calls in the **page's** hook slots, indexed by loop order. Removing or reordering a row silently re-keys later rows' **Parse Blockers** to the wrong item.
- A row component keyed by index has the same problem one level down: Dioxus reuses the scope that sits at that index, hook state and all.
- `key: "{item.key()}"` (or `CollectionItemIdentity::key()`) ties the row's scope to the logical item, so its parse state moves with the item and is released with it.

Row components take the **Form Handle** and the item identity as props. A handle compares by observable identity ([ADR-0024](adr/0024-compare-form-handles-by-observable-identity.md)), so it is an ordinary `#[component]` prop, and a page that renders collection rows needs no **Form Context Scope**. Context access stays available for a subtree of renderless helpers deep enough that threading the handle is the greater cost (see [Form Context Access](form-context.md)); a keyed row is not that case.

Resolve the item from the collection inside the row rather than accepting a resolved row value from the parent. That lookup is also the row's subscription to the collection's value, and **Field Ancestry** is strict between a collection and its own items, so nothing else wakes the row when the collection's structure changes. A row that stopped reading the collection would never re-render after a reorder or a removal: its accessors would still *answer* correctly when called, because the index resolves live, but the row would keep painting the values and the sibling count from its last render — leaving, for example, an enabled "move down" control on a row that is now last.

[`item(identity)`](#resolving-one-item-by-identity) is that lookup: it answers `None` once the item no longer belongs to the collection, and it registers the same collection-value read `items()` does, so the subscription above is intact either way. A row that renders one item wants `item()`; `items()` remains the parent's call, and the row's own call when it needs the sibling count as well. The early return above is a guard rather than a reachable path — Dioxus flushes the parent first, so a removed row unmounts before it renders again — but it must come **before the row's first hook**. Returning ahead of every `use_` call leaves the scope's hook slots untouched; skipping one hook while claiming a later one mis-indexes them and panics.

Bindings that own no hook state — `item.text(...)`, `item.select(...)`, `item.checkbox(...)` — are safe to build anywhere, but keeping the whole row in one keyed component keeps the rule simple.

That rule is about *rows*. A scope that is not a row may bind a collection item freely: a page-level control bound to the first line, a detail panel bound to the selected row, a virtualized slot. Such a scope renders a different item when the collection moves under it, and the mounted **Parse Blocker** is **re-addressed** to the item the scope renders now ([ADR-0026](adr/0026-re-address-a-parse-blocker-to-the-item-its-mount-renders.md)). Re-addressing drops the in-flight raw text and the parse error held for the previous item — text typed for one logical item is not input for another — so the input renders the new item's formatted value. Reordering changes nothing, because reordering does not change an item's identity. A binding clone retained past the change, such as one parked in an event handler, keeps addressing its own item: its value writes still land there, while its parse reads and writes go inert rather than driving the mount that has moved on.

For a collection under a nested named struct, compose the generated direct field paths with `FieldPath::join`:

```rust
let lines = InvoicePage::fields()
    .invoice()
    .join(Invoice::fields().lines());

let product_name = InvoiceLine::fields()
    .product()
    .join(Product::fields().name());

for item in form.collection(lines).items() {
    let name = item.text(product_name.clone());

    assert_eq!(name.name().as_deref(), Some("invoice.lines[0].product.name"));
}
```

The composed collection **Field Identity** remains static, such as `invoice.lines`, while item child field identities combine that static collection path, the logical **Collection Item Identity**, and the static child path, such as `product.name`. Rendered **Field Names** remain HTML-compatible and index-based, such as `invoice.lines[0].product.name`, so names update after reordering while metadata remains attached to the logical item.

Collection item child bindings use the current rendered index for their HTML-compatible **Field Name**, such as `lines[0].description`. Accessibility helpers and row keys use **Collection Item Identity** so they stay stable when the item moves.

The rendered index is resolved **live** against the collection every time it is asked for, not captured when the binding is built. `name()` and `index()` therefore stay correct on a binding retained across a sibling removal, insertion, `move_to_index` or `swap`, and no two resolved bindings in one collection ever render the same **Field Name**. Because a live index has no honest total answer, both return `Option`: `index()` is `Option<usize>` and `name()` is `Option<String>` on the item binding, on every leaf collection binding, and on `MultiSelectItem`. See [ADR-0023](adr/0023-resolve-the-rendered-collection-item-index-live.md).

When a collection item child input owns parse state, prefer `use_collection_item_parsed_text`, `use_collection_item_parsed_text_with`, `use_collection_item_number`, or `use_collection_item_number_with` inside identity-keyed row components, as in the example above. These hooks keep the mounted Parse Blocker keyed by **Collection Item Identity** and child field identity while `name()` continues to reflect the latest rendered index.

Supported in this slice:

- Direct `Vec<Item>` collection fields and nested direct `Vec<Item>` collection paths composed through named struct fields.
- Form-owned append, insert, remove, move, swap, replace, and clear operations, with programmatic and user-originated variants.
- Typed item child bindings for text and parsed numeric/text inputs.
- Metadata, submit errors, validation errors, parse blockers, and dirty state keyed by logical item identity.
- Reusable synchronous item-child validator templates for every current and future item, such as every `lines[].quantity`.
- True multi-select helpers for direct `Vec<Value>` fields, where each selected value is a logical collection item.
- Opt-in form-state snapshots that include current and baseline collection item identity sequences for tracked collections.
- Reset and reinitialization through the existing form lifecycle.

Register reusable item-child validators on the collection binding when a rule applies to every current and future item:

```rust
let lines = form.collection(InvoiceForm::fields().lines());

lines
    .item_field_validator(InvoiceLine::fields().quantity(), "quantity")
    .check(|quantity, _context| {
        if *quantity == 0 {
            vec!["Quantity must be at least 1."]
        } else {
            Vec::new()
        }
    });
```

The validator result attaches to the logical item child field. If the item moves, the error moves with it. If the item is removed, the item-scoped validator state is cleared without affecting sibling items.

## Unresolved Bindings

A binding is created for one logical **Collection Item Identity**. Nothing stops the application from
holding that binding across a mutation that removes the item — event handlers, spawned futures,
`use_effect` and `use_memo` all keep their own clone, and `oninput()` / `onchange()` hand the handler a
clone precisely so the binding stays usable for `value()`. Once the item is gone the binding is an
**Unresolved Binding**, and every accessor on it answers by surface:

- The **rendered surface is total and neutral**: `value()` on text, parsed text and rendered select
  returns `""`, `checked()` returns `false`, and every `is_selected(...)` / `is_rendered_selected(...)`
  returns `false`. A UI must not crash on a removal race.
- The **typed surface reports absence in the return type**: `CollectionSelectBinding::value`,
  `CollectionRenderedSelectBinding::typed_value`, `CollectionRadioGroupBinding::value` and
  `MultiSelectItem::value` return `Option<Value>` and read `None`. They do not invent a
  `Default::default()`, which would assert a selection the model does not hold.
- **The rendered name and index report absence too**: `name()` returns `None` and `index()` returns
  `None`. An **Unresolved Binding** renders no name, so it can never collide with the name of a row
  that is still live. It is not `""`: `name=""` is the value the HTML entry-set algorithm branches on
  to exclude a control from submission, and it is already a legal rendered name in this crate.
- **Metadata and validation errors report nothing**: `is_touched()` and `is_blurred()` are `false` and
  `validation_errors()` is empty. Removal releases the item's scoped state, so this is a true statement
  about state that no longer exists.
- **Writes are silent no-ops**: `set_value`, `on_input`, `on_change`, `select` and `on_blur` leave the
  collection and its metadata unchanged. Setters do not report whether the write landed, because the
  ready-made handlers are `impl FnMut(Event<..>)` and cannot propagate a result.

No accessor panics.

`is_resolved()` is the guard. It is available on `CollectionItemBinding`, on every leaf collection
binding (text, checkbox, select, rendered select, radio group, parsed text) and on `MultiSelectItem`, so
a handler that only retained a leaf can check without a route back to its `CollectionItemBinding`:

```rust
let description = item.text(InvoiceLine::fields().description());

spawn(async move {
    save(draft).await;

    if description.is_resolved() {
        description.set_value("saved");
    }
});
```

For one binding, `name()`, `index()`, `value()` and `is_resolved()` answer in **exact lockstep**. An
unresolved binding never renders a name, and a binding that renders a name always has a value.

`identity()` and `accessibility()` stay infallible. Both are derived from the **Collection Item
Identity** rather than the index, so they keep answering for an **Unresolved Binding** — the
accessibility ID of a row is unchanged by a sibling removal.

The same rule holds for **Optional Fields** reached through `FieldPath::or` (see
[Optional Fields](optional-fields.md)): a total, neutral editing surface with an honest accessor
alongside. See [ADR-0022](adr/0022-represent-an-absent-binding-target-in-the-return-type.md).

## Identity Lifetime

A **Collection Item Identity** is minted once, from a counter that never moves backward, and denotes
one logical item for as long as any binding holds it. No number is ever issued twice within one
form's lifetime, so a retained binding can go unresolved but can never come back addressing a
different row ([ADR-0025](adr/0025-mint-collection-item-identities-from-a-never-rewinding-counter.md)):

- `reset()` restores the collection to its baseline rows, which are the same logical items they were
  before. Each keeps its identity, so a binding for a baseline row still denotes that row and its
  `key()` is unchanged, which keeps the row mounted. Rows added since the baseline are dropped and
  their identities retired.
- `reinitialize(model)` installs a different model, whose rows are new logical items. Every identity
  the collection has issued is retired and the new rows are numbered above all of them, so a binding
  retained across the call is unresolved for good. Every row is rendered under a fresh `key()` and is
  therefore **remounted**, which releases the scope state of each row — focus, scroll position, and
  the parse state held by that row's hooks. Prefer per-item operations when a form's rows should
  survive an update.
- `restore_state_snapshot(snapshot)` adopts the identities the snapshot carries, so a same-form
  round trip returns each row to the binding that held it. The counter never drops to the snapshot's
  value: it stays at the higher of the live and restored one, and a live collection the snapshot
  says nothing about has its identities retired rather than renumbered from zero.
- `reset_field(path)` on the collection's own `Vec<Item>` path behaves like `reset()` for that one
  collection.

A snapshot minted by a *different* form instance or process is a documented limitation. Its
identities come from an unrelated allocator history, so they can collide with live ones and a
retained binding can alias onto a restored row. Restoring a foreign snapshot — hydration and
cross-process restore — targets a form that has just mounted and holds no retained bindings.

## Array Mutations

`CollectionBinding` exposes the array mutations directly, each with a user-originated method (the plain
name, which marks the collection touched) and a `_programmatic` variant (an application update that does
not mark it touched), matching `append` / `insert` / `remove` / `move_to_index`:

- `append` / `insert(index, item)`: add an item; a fresh **Collection Item Identity** is allocated.
- `remove(item)`: remove one item by its logical identity, releasing that item's scoped state.
- `move_to_index(item, index)`: reorder one item by its logical identity.
- `swap(a, b)`: exchange two items **by position**. Each item keeps its own **Collection Item
  Identity**, so item-scoped metadata, validation errors, parse blockers, and dirty state follow the
  moved items rather than the index. Returns `false` if either index is out of bounds or the two
  indices are equal (a no-op).
- `replace(index, item)`: replace one item's value **in place**. This is deliberately identity
  *preserving*: the existing item keeps its **Collection Item Identity** and its item-scoped metadata
  and validation attachment, so a replace reads as "this same row now holds a new value," not as a
  remove-then-insert. Returns `false` if the index is out of bounds. Insert a fresh identity by
  `remove` + `insert` when you intend a genuinely new row.
- `clear`: remove every item, releasing item-scoped state for each, without touching form-level state
  or sibling collections. Returns whether any item was removed (clearing an empty collection is a
  no-op).

```rust
let lines = form.collection(InvoiceForm::fields().lines());

lines.swap(0, 1); // exchange the first two rows, identities intact
// in-place value replacement, keeping this row's Collection Item Identity
lines.replace(0, InvoiceLine { description: "Deploy".to_owned(), quantity: 5 });
lines.clear(); // remove every row
```

Rendered index-based **Field Names** update after each operation, while **Field Identity** /
**Collection Item Identity** keyed state stays attached to the logical item. Each operation emits the
matching **Form Observer** transition (`CollectionItemsSwapped`, `CollectionItemReplaced`,
`CollectionCleared`) and integrates with **Reset** and **Reinitialization** through the existing form
lifecycle.

## Resolving One Item by Identity

`item(identity)` is the read side of that identity-keyed family. Pass it an identity already in hand
— the one `append` or `insert` just returned, one captured in an event handler or in application
state, one delivered by a **Form Observer** transition — and it resolves to that item's binding, or
to `None` once the item no longer belongs to the collection:

```rust
let lines = form.collection(InvoiceForm::fields().lines());
let added = lines.append(InvoiceLine {
    description: String::new(),
    quantity: 1,
});

if let Some(item) = lines.item(added) {
    item.text(InvoiceLine::fields().description())
        .set_value("Deploy");
}
```

The binding behaves exactly as the one `items()` yields for that item: it reports the live rendered
index, so it keeps answering correctly after a reorder. Answering the absence at the lookup is the
alternative to holding an [**Unresolved Binding**](#unresolved-bindings) and asking it afterwards —
useful when the caller can act on the item being gone, as a row that returns early can.

`MultiSelectBinding` has no such lookup; see [Multi-Select Fields](#multi-select-fields).

## Multi-Select Fields

True multi-select helpers use the same direct `Vec<Item>` collection machinery, but the selected
value itself is the logical collection item. Use `FormHandle::multi_select(path)` for a single typed
Field such as `topics: Vec<Topic>`:

```rust
let topics = form.multi_select(ProfileForm::fields().topics());
let rust = topics.option(Topic::Rust);

rsx! {
    input {
        r#type: "checkbox",
        name: rust.name(),
        checked: rust.checked(),
        oninput: move |event| rust.on_change(event.checked()),
    }
}
```

The helper does not own or render option lists. It exposes selection behavior while the application
chooses native checkboxes, a custom listbox, command palette rows, chips, or any other UI. Selected
values can be inspected through `selected_values()` or `items()`. Each `MultiSelectItem` exposes its
opaque `CollectionItemIdentity`, item-level metadata, dirty state, accessibility helper, and
validation errors.

A multi-select is keyed by **value** rather than by identity: `is_selected(value)`,
`selected_item(value)` and `selected_identity(value)` are the lookups, and `select` / `deselect` /
`toggle` take a value too. It therefore has no `item(identity)` counterpart — a caller holding one of
its identities also holds the value that produced it
([ADR-0027](adr/0027-add-an-identity-keyed-collection-item-accessor.md)).

Use `item_validator(...).check(...)` when a rule applies to every selected value. Validation errors
attach to the selected value identity, so removing that selected value clears its item-scoped state
without affecting other selected values.

Serialization:

`FormCore::state_snapshot()` and `FormHandle::state_snapshot()` include **Collection Item Identity** state for every tracked collection. The serialized identity state records each collection field, its baseline item identity sequence, its current rendered item identity sequence, and the next identity counter used for future insertions. Restoring that snapshot preserves item-scoped metadata and non-submit validation errors that are keyed by **Field Identity**.

This is different from deterministic initialization. A deterministic client render can recreate the same `Vec<Item>` values, but it cannot infer which logical item survived an insertion, removal, or reorder. Snapshot restore is explicit and opt-in; see [Form State Serialization](form-state-serialization.md).

Still deferred:

- Maps, sets, arrays, collection traversal through collection item hierarchies, and enum-variant collections.
- Asynchronous collection item validator templates.
- Adapter-owned parse-state serialization.
