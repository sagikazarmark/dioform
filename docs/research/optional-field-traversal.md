# Addressing Fields Inside Optional Parents: Prior Art

Research input for [#25](https://github.com/sagikazarmark/dioform/issues/25) ("is supporting
optional fields feasible at all, and if so what is the best solution?").

**This file is input to a decision, not the decision.** It records what other form libraries and
optics libraries actually do, with citations. It does not recommend a design, and nothing here is
recorded in `CONTEXT.md` or an ADR.

## The question

> How do existing form libraries and optics libraries let a user address a field *nested inside an
> optional/nullable container*, and what semantics do they choose for reading and writing through an
> absent parent?

Broken into six sub-questions:

1. **Addressing** — can you name a path that reaches through an optional parent? Is it type-checked?
2. **Read semantics** — what does reading `parent.child` yield when `parent` is absent?
3. **Write semantics** — does writing auto-create the parent? Deliberate, or emergent from a
   lodash-`set`-shaped implementation?
4. **Explicit presence** — is there a way to set/clear the whole optional parent?
5. **Metadata survival** — what happens to validation state, errors, touched/dirty for inner fields
   across a clear/re-create cycle? Is field identity stable?
6. **Absent vs present-and-default** — distinguished, or collapsed?

## Method and evidence rules

Every factual claim below links to a primary source: an official docs page, or a source file at a
pinned commit. Where a behaviour is *not documented* and was read out of the implementation, the
claim is marked **[from source]**. Where neither docs nor source settle it, the text says
**not established** rather than guessing.

Permalinks are pinned to these commits, current at time of writing (2026-08-15):

| Repo | Commit |
| --- | --- |
| `TanStack/form` | [`7b8fc1d`](https://github.com/TanStack/form/tree/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd) |
| `react-hook-form/react-hook-form` | [`9b7af71`](https://github.com/react-hook-form/react-hook-form/tree/9b7af71b4d25da143fd2522c40d20466c7433f00) |
| `react-hook-form/documentation` | [`da85181`](https://github.com/react-hook-form/documentation/tree/da8518113d739026189285af39d79a5abdcbe755) |
| `ekmett/lens` | [`a7c08e1`](https://github.com/ekmett/lens/tree/a7c08e1cbeb2a4a937854dbbbe7b21cfab16eff2) |
| `well-typed/optics` | [`5eb61a8`](https://github.com/well-typed/optics/tree/5eb61a85180dd5177ebe11d38b47d1d7a7caadce) |

Formik, Final Form, the Rust crates and the typed non-JS libraries are cited against their own
default branches inline (those move; the quoted excerpts are what was read on 2026-08-15).

Runtime behaviour attributed to Formik and Final Form marked *verified* was produced by executing
the published packages (`formik@2.4.9`, `final-form@5.0.1`), not inferred.

**Verification depth.** Every code excerpt quoted in the TanStack, React Hook Form, optics and Rust
optics sections was fetched and read directly. In the typed non-JS section, the following were
re-fetched and confirmed line by line: `reactive_stores` (`subfield.rs`, `option.rs`, `path.rs`,
`store_field.rs`, `lib.rs`), `composable-form` (`Form.elm`, `Form/Base.elm`, `Form/Base/FormList.elm`),
Play `Form.scala`, Reflex `Dynamic.hs`, `etaque/elm-form` (`Form.elm`, `Form/Tree.elm`, `example/src/View.elm`),
`digestive-functors` (`Form.hs`, `Form/Internal.hs`), `leptos_form` (`form_component/mod.rs`),
`validator` (`traits.rs`), `yew_form` (`model.rs`), Formless issue #62, and `bevy_reflect`'s `GetPath`
docs. The remaining short entries under "The rest, briefly" (`dillonkearns/elm-form`, `keypath`,
Druid's `widget::Maybe`, `leptos_form_tool`, `yewdux`, `egui`, `garde`) are cited but were **not**
independently re-read at this level; they are colour rather than load-bearing, and no conclusion in
this file rests on them alone.

---

## Comparison table

| | 1. Addressing | 2. Read through absent parent | 3. Write through absent parent | 4. Explicit presence | 5. Metadata survival | 6. Absent vs present-default |
| --- | --- | --- | --- | --- | --- | --- |
| **TanStack Form** | Yes. `DeepKeys<T>` recurses through `NonNullable<T[K]>`, so `'party.name'` exists even when `party?: Party`. Type-checked. | `undefined` (or `null` if a link is `null`). At the *type* level the parent's nullability is pushed onto the child: `DeepValue<…,'party.name'> = string \| undefined`. | **Auto-creates a plain object.** Not documented; emergent from `setBy`. | Yes — the parent path is itself a `DeepKey`; `setFieldValue('party', …)` / `deleteField('party')`. | Meta is a flat map keyed by path string, so identity is stable — **except** `deleteField(parent)` explicitly deletes every `parent.*` meta entry. | Collapsed at the type level (both become `\| undefined`); distinguishable at runtime only by inspecting the parent yourself. |
| **React Hook Form** | Yes. `Path<T>` maps over `keyof T` with `-?`, so optional parents still yield child paths. Type-checked. | `undefined`. `PathValue` widens the child with `\| undefined` when the parent is optional. | **Auto-creates**; object vs array chosen by a lookahead numeric test on the *next* segment. Not documented as such; docs only say `setValue` on an unregistered input "works". | Yes — `setValue('parent', {...})`, `unregister('parent')`, `resetField`. | `unregister` drops value/error/touched/dirty by default, with explicit `keepValue`/`keepError`/`keepDirty`/`keepTouched` opt-outs. Identity is the path string. | Collapsed. `unset` prunes emptied parents recursively, so present-and-empty decays to absent. |
| **`@hookform/lenses`** (typed lens layer on RHF) | Yes, `lens.focus('profile.email')`; type-checked via RHF's `PathValue`. | Inherits RHF: `Lens<string \| undefined>`. | Inherits RHF `setValue`. | Inherits RHF. | Inherits RHF. | Collapsed. Its answer to optionality is `defined()` / `narrow()` / `assert()` — documented as **type-level only, no runtime check**. |
| **Formik** | Yes, lodash dot/bracket strings. **Untyped `string`** — the type-level question does not arise. | `undefined`, or an explicit `def` argument to `getIn`. | **Auto-creates**; array vs object by lookahead numeric test. **Undocumented — emergent.** A primitive parent is silently destroyed. | Yes — `setFieldValue('parent', …)`; writing `undefined` `delete`s the key. | `values`/`errors`/`touched` are three independent trees keyed by the same path string; identity stable, stale metadata survives. | Collapsed on read. Does **not** prune, so `{a:{}}` husks persist and reach the submit payload. |
| **Final Form** | Yes, dot/bracket, with a dedicated docs page. Untyped in the React binding; `FormApi` types `name` as `keyof FormValues`, which *breaks* nested paths if you parameterise it. | `undefined`. No `def` parameter. React binding maps `undefined` → `''` for the input. | **Auto-creates — and this is documented deliberately**, with a rule list and worked table. | Yes — `form.change('parent', undefined \| {...})`; plus `destroyOnUnregister`. | Field records live in a flat `state.fields` map keyed by path; `touched`/`visited`/`modified` survive a clear/re-create cycle. Errors are pruned with values. | Collapsed hardest: setting the last child to `undefined` **deletes the parent**, so present-and-empty is unrepresentable for objects (arrays exempted by design). |
| **Haskell `lens` / `optics`** | Yes, by composing a `Lens` with a `Prism`; the composite is an **`AffineTraversal`**, a distinct kind. Fully type-checked. | `preview` returns `Maybe a` → `Nothing`. Absence is in the result type. | **No-op.** `set`/`over` through a non-matching prism returns the structure unchanged. Documented by doctest. | Yes, and it is a *separate optic*: `at k :: Lens' m (Maybe v)` reads/writes/deletes presence; `ix k :: AffineTraversal' m v` traverses only if present. Law: `ix k ≡ at k % _Just`. | N/A (optics carry no metadata). | Distinguished by construction — `Maybe a` is the focus of `at`. Collapsing them is an explicit opt-in combinator, `non`, which requires a caller-supplied default. |
| **`reactive_stores`** (Leptos) | Yes, type-checked, **with the infallible `fn(&Prev) -> &T` / `fn(&mut Prev) -> &mut T` pair unchanged**. The `Option` hop is an ordinary path segment with `unwrap`-based accessors. | Panics via `.unwrap()`; the documented safe route `map()` returns `None`. | No auto-create; panics. Undocumented. | Yes — the `Option<T>` field is itself a path; `store.name().set(None)`. | **Path identity is structural (`Vec<StorePathSegment>`)**, so a subfield's trigger is found at the same path regardless of ancestor presence; their `patch` test corroborates this rather than proving it. | Distinguished — it is a real `Option` in the model. |
| **`composable-form`** (Elm) / Play `OptionalMapping` | No optional parents exist. Editing state is total; `Maybe`/`Option` appears only in the *parsed output*. Nesting is a total lens. | n/a | n/a | No — presence is **derived** from emptiness of the inner fields. | Nothing persisted: the form is a pure `values -> FilledForm`. | **Collapsed** — present-and-default is unrepresentable. |
| **Reflex `maybeDyn`** (Haskell) | You enter a scope that exists only while present, rather than naming a path through absence. | Scope does not exist. | n/a | Yes, structurally. | **New identity per presence episode — documented in the doc comment.** | Distinguished. |
| **`bevy_reflect`** | Runtime strings, untyped. `Option` stepped through as an enum with `.0`. | `Err(ReflectPathError)`; **[from source]** absence surfaces as `IncompatibleEnumVariantTypes`, indistinguishable from a type mismatch. | Errors, never creates. | No — docs say paths "assume the variant is already known ahead of time". | n/a | n/a |
| **Rust optics crates** | Thin and mostly abandoned. `lens-rs` and `enso-optics` type-check a path through `Option`; `serde_json`'s pointer is untyped. Maintained crates (`frunk`, `pathmod`, `xilem_core`, `pl-lens`) are lens-only, i.e. dioform's exact signature. | `Option<&T>` / `Option<&mut T>` in the three designs that model it; owned `Result<A, E>` in `optics`. | Split. `lens-rs`, `enso-optics`, `serde_json` no-op. `optics` and `druid-widget-nursery` **auto-construct** — undocumented, from source. `smart_access` makes construction a different index type (`Ensure`). | Where present, yes — construction is a separate trait (`Review`) or a separate index type, never a flag on the traversal. | N/A. | Distinguished where `Option<&T>` is the read type. |

---

## TanStack Form

dioform's explicit parity reference (see [`docs/archive/tanstack-parity.md`](../archive/tanstack-parity.md)).

### 1. Addressing — yes, and the type system deliberately erases the parent's optionality into the child's value

`DeepKeys<T>` is built from `DeepKeysAndValues`, which recurses into `NonNullable<T[K]>` and widens
the value with the parent's nullability:

```ts
export type Nullable<T> = T & (undefined | null)

export type ObjectValue<TParent extends AnyDeepKeyAndValue, T, TKey extends AllObjectKeys<T>> =
  T[TKey] | Nullable<TParent['value']>

export type DeepKeyAndValueObject<TParent, T, TAcc, TAllKeys extends AllObjectKeys<T> = AllObjectKeys<T>> =
  TAllKeys extends any
    ? DeepKeysAndValuesImpl<
        NonNullable<T[TAllKeys]>,
        ObjectDeepKeyAndValue<TParent, T, TAllKeys>,
        TAcc | ObjectDeepKeyAndValue<TParent, T, TAllKeys>
      >
    : never
```

— [`packages/form-core/src/util-types.ts` L106–L134](https://github.com/TanStack/form/blob/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd/packages/form-core/src/util-types.ts#L106-L134).

Because the recursion strips `NonNullable`, a key through an optional parent **is** generated;
because `ObjectValue` re-adds `Nullable<TParent['value']>`, the *child's* type absorbs the parent's
absence. `Nullable<Party | undefined>` reduces to `undefined`; `Nullable<Party>` reduces to `never`,
so non-optional parents cost nothing.

This is deliberate, not accidental — it has dedicated type tests:

```ts
type NestedNullableObjectCase = {
  null: { mainUser: 'name' } | null
  undefined: { mainUser: 'name' } | undefined
  optional?: { mainUser: 'name' }
  mixed: { mainUser: 'name' } | null | undefined
}
// DeepValue<NestedNullableObjectCase, 'null.mainUser'>      === 'name' | null
// DeepValue<NestedNullableObjectCase, 'undefined.mainUser'> === 'name' | undefined
// DeepValue<DoubleNestedNullableObjectCase, 'mixed.mainUser.name'> === 'name' | null | undefined
```

— [`packages/form-core/tests/util-types.test-d.ts` L176–L228](https://github.com/TanStack/form/blob/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd/packages/form-core/tests/util-types.test-d.ts#L176-L228).

**This is the load-bearing observation for #25.** TanStack's answer to "what type does a field under
an optional parent have?" is: *the inner type, unioned with the parent's absence*. It does not
introduce an optional-group construct at all. In Rust terms, `FieldPath<Model, Party>` under an
`Option<Party>` parent becomes something whose read is `Option<&String>` — i.e. the type-level shape
of candidate 3 (fallible traversal), not of candidate 1 or 2.

### 2. Read semantics — `undefined` propagates

```ts
export function getBy(obj: unknown, path: string | (string | number)[]): any {
  const pathObj = makePathArray(path)
  return pathObj.reduce((current: any, pathPart) => {
    if (current === null) return null
    if (typeof current !== 'undefined') { return current[pathPart] }
    return undefined
  }, obj)
}
```

— [`packages/form-core/src/utils.ts` L36–L45](https://github.com/TanStack/form/blob/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd/packages/form-core/src/utils.ts#L36-L45).
`FormApi.getFieldValue` is exactly `getBy(this.state.values, field)` —
[`FormApi.ts` L2570–L2572](https://github.com/TanStack/form/blob/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd/packages/form-core/src/FormApi.ts#L2570-L2572).

No error, no per-field default at the read site. A `null` link short-circuits to `null`; an
`undefined` link short-circuits to `undefined`.

### 3. Write semantics — auto-creates a plain object; **[from source]**, not documented

```ts
export function setBy(obj: any, _path: any, updater: Updater<any>) {
  const path = makePathArray(_path)
  function doSet(parent?: any): any {
    if (!path.length) { return functionalUpdate(updater, parent) }
    const key = path.shift()
    if (typeof key === 'string' || (typeof key === 'number' && !Array.isArray(parent))) {
      if (typeof parent === 'object') {
        if (parent === null) { parent = {} }
        return { ...parent, [key]: doSet(parent[key]) }
      }
      return { [key]: doSet() }          // <-- parent was undefined: a fresh object is invented
    }
    …
  }
  return doSet(obj)
}
```

— [`packages/form-core/src/utils.ts` L51–L91](https://github.com/TanStack/form/blob/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd/packages/form-core/src/utils.ts#L51-L91).
`setFieldValue` writes through it —
[`FormApi.ts` L2651–L2689](https://github.com/TanStack/form/blob/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd/packages/form-core/src/FormApi.ts#L2651-L2689).

Two consequences worth recording:

- `setFieldValue` also marks the field `isTouched: true, isDirty: true` in the same batch, so
  materialisation and interaction-state are entangled by construction (same lines).
- **[from source]** Merely *mounting* a field with a `defaultValue` materialises an absent parent:
  `FieldApi.mount()` calls `setFieldValue(this.name, this.options.defaultValue, { dontUpdateMeta: true })`
  when a default is present and the field is untouched —
  [`FieldApi.ts` L859–L864](https://github.com/TanStack/form/blob/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd/packages/form-core/src/FieldApi.ts#L859-L864).
  So rendering an inner field can create the parent before the user types anything.

**Documentation status:** I searched the React guides (`basic-concepts.md`, `form-groups.md`,
`arrays.md`) and the generated API reference. **TanStack Form's prose docs never mention optional or
nullable parents, and never state that writing materialises them.** The auto-creation is emergent
from `setBy` in the same way lodash `set` is emergent for the other libraries. The generated
reference documents `deleteField()`'s *signature* only, with no prose about semantics —
[`docs/reference/classes/FormApi.md` L266–L294](https://github.com/TanStack/form/blob/main/docs/reference/classes/FormApi.md).

### 4. Explicit presence — yes, but it is just another path

Because the optional parent is itself a `DeepKey`, `setFieldValue('party', undefined)` and
`deleteField('party')` both work. `deleteField` uses `deleteBy`, which removes the key —
[`FormApi.ts` L2691–L2710](https://github.com/TanStack/form/blob/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd/packages/form-core/src/FormApi.ts#L2691-L2710),
[`utils.ts` L97–L143](https://github.com/TanStack/form/blob/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd/packages/form-core/src/utils.ts#L97-L143).

There is no dedicated presence API. Both the whole-value binding and the inner-field bindings write
to the same store with no arbitration, which is exactly the ambiguity raised as open question 5 in
the issue triage.

### 5. Metadata survival — path-keyed, so stable, except `deleteField` wipes descendants

Field meta is a **flat map keyed by the deep key string**:

```ts
fieldMetaBase: Partial<Record<DeepKeys<TFormData>, AnyFieldLikeMetaBase>>
```

— [`FormApi.ts` L652](https://github.com/TanStack/form/blob/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd/packages/form-core/src/FormApi.ts#L652).

So "field identity" is the path string, entirely decoupled from whether the value exists.
`setFieldValue('party', undefined)` leaves all `party.*` meta in place **[from source]**.
`deleteField('party')` does not:

```ts
const subFieldsToDelete = Object.keys(this.fieldInfo).filter((f) => {
  const fieldStr = field.toString()
  return f.startsWith(`${fieldStr}.`) || f.startsWith(`${fieldStr}[`)
})
```

— [`FormApi.ts` L2691–L2710](https://github.com/TanStack/form/blob/7b8fc1d17fdfc3dbdb81077820f900bdc4bb28fd/packages/form-core/src/FormApi.ts#L2691-L2710).

**Two different "clear the parent" operations therefore have two different metadata outcomes**, and
neither is documented. That asymmetry is directly relevant to issue open questions 3 and 4.

### 6. Absent vs present-and-default — collapsed at the type level

`DeepValue<Form, 'party.name'>` is `string | undefined` whether `party` is absent or `party.name` is
absent. The value types cannot tell the two apart; only reading `getFieldValue('party')` can. The
type tests above make this explicit and intentional.

---

## React Hook Form

### 1. Addressing — yes, type-checked, with the same collapse

`Path<T>` maps over `keyof T` with the `-?` modifier and recurses on `T[K]`:

```ts
type PathInternal<T, TraversedTypes = T, D extends number = 9> = …
  : { [K in keyof T]-?: PathImpl<K & string, T[K], TraversedTypes, D> }[keyof T];

export type Path<T> = T extends any ? PathInternal<T> : never;
export type FieldPath<TFieldValues extends FieldValues> = Path<TFieldValues>;
```

— [`src/types/path/eager.ts` L42–L72](https://github.com/react-hook-form/react-hook-form/blob/9b7af71b4d25da143fd2522c40d20466c7433f00/src/types/path/eager.ts#L42-L72).

The type tests confirm paths are generated through optional parents — `ArrayPath` of
`{ bar?: { baz?: 1; fooArr?: Foo[] } } | Record<string, never>` is `'bar.fooArr'` —
[`src/__typetest__/path/eager.test-d.ts` L95–L106](https://github.com/react-hook-form/react-hook-form/blob/9b7af71b4d25da143fd2522c40d20466c7433f00/src/__typetest__/path/eager.test-d.ts#L95-L106).

The exported `PathValue` widens the child with `| undefined` when the parent is optional:

```ts
type PathValueImpl<T, P extends string> = T extends any
  ? P extends `${infer K}.${infer R}`
    ? K extends keyof T
      ? undefined extends T[K]
        ? PathValueImpl<T[K], R> | undefined
        : PathValueImpl<T[K], R>
      : …
```

— [`src/types/path/eager.ts` L160–L185](https://github.com/react-hook-form/react-hook-form/blob/9b7af71b4d25da143fd2522c40d20466c7433f00/src/types/path/eager.ts#L160-L185).

The internal `EvaluateKey` machinery is documented with the same rule and directly tested:

```ts
/** it should add null if the type may be null */
const actual = _ as EvaluateKey<null | { foo: string }, 'foo'>;   // string | null
/** it should add undefined if the type may be undefined */
const actual = _ as EvaluateKey<undefined | { foo: string }, 'foo'>;  // string | undefined
```

— [`src/__typetest__/path/common.test-d.ts` L489–L500](https://github.com/react-hook-form/react-hook-form/blob/9b7af71b4d25da143fd2522c40d20466c7433f00/src/__typetest__/path/common.test-d.ts#L489-L500),
and the doc comment on `TryAccess` states the rule verbatim
([`src/types/path/common.ts` L170–L188](https://github.com/react-hook-form/react-hook-form/blob/9b7af71b4d25da143fd2522c40d20466c7433f00/src/types/path/common.ts#L170-L188)).

**RHF and TanStack independently converged on the same type-level answer.** That is the strongest
signal in the JS prior art: the industry answer to "field under an optional parent" is *not* a new
addressing construct — it is `Option`-widening the leaf value type.

### 2. Read semantics — `undefined`

```ts
const result = paths.reduce<any>((result, key) => {
  return isNullOrUndefined(result) ? undefined : result[key];
}, object);
return isUndefined(result) || result === object
  ? isUndefined(object[path as keyof T]) ? defaultValue : object[path as keyof T]
  : result;
```

— [`src/utils/get.ts`](https://github.com/react-hook-form/react-hook-form/blob/9b7af71b4d25da143fd2522c40d20466c7433f00/src/utils/get.ts).

`getValues` is documented only as "Gets the value at path of the form values", with one rule: "It
will return `defaultValues` from `useForm` before the **initial** render" —
[getValues docs](https://react-hook-form.com/docs/useform/getvalues)
([source](https://github.com/react-hook-form/documentation/blob/da8518113d739026189285af39d79a5abdcbe755/src/content/docs/useform/getvalues.mdx)).
Absent parents are not discussed.

### 3. Write semantics — lodash-`set`-shaped auto-creation, **[from source]**

```ts
if (index !== lastIndex) {
  const objValue = object[key];
  newValue =
    isObject(objValue) || Array.isArray(objValue)
      ? objValue
      : !isNaN(+tempPath[index + 1])
        ? []
        : {};
}
object[key] = newValue;
object = object[key];
```

— [`src/utils/set.ts`](https://github.com/react-hook-form/react-hook-form/blob/9b7af71b4d25da143fd2522c40d20466c7433f00/src/utils/set.ts).

Note this is a **mutating** write, and the array-vs-object decision is a lookahead numeric test on
the *next* segment — the classic lodash heuristic.

**Documentation status:** the [`setValue` docs](https://react-hook-form.com/docs/useform/setvalue)
([source](https://github.com/react-hook-form/documentation/blob/da8518113d739026189285af39d79a5abdcbe755/src/content/docs/useform/setvalue.mdx))
never say that missing parents are created. What they *do* document is adjacent and revealing:

> ```js
> // you can use `setValue` on an unregistered input
> setValue("notRegisteredInput", "value") // ✅ prefer it to be registered
>
> // the following will implicitly register a single input (without register being invoked)
> setValue("resultSingleNestedField", { test: "1", test2: "2" }) // ⚠️ works, but registers a field you never called register() on — prefer registering it explicitly
> ```

and a documented *failure* of path-based writing:

> ```js
> register("nestedValue", { value: { test: "data" } }) // register a nested value input
> setValue("nestedValue.test", "updatedData") // ❌ failed to find the relevant field
> setValue("nestedValue", { test: "updatedData" }) // ✅ setValue finds the input and updates it
> ```

So: writing through a path is supported and the docs hedge it with warnings, but the specific
question "does the parent get invented?" is **undocumented**. It is emergent from `set`.

### 4. Explicit presence — `unregister`, with a documented table

`unregister` is the closest thing to a presence control, and it is documented with a worked example:

| Type | Input Name | Value |
| --- | --- | --- |
| `string` | `unregister("yourDetails")` | `{}` |
| `string` | `unregister("yourDetails.firstName")` | `{ lastName: '' }` |

— [unregister docs](https://react-hook-form.com/docs/useform/unregister)
([source](https://github.com/react-hook-form/documentation/blob/da8518113d739026189285af39d79a5abdcbe755/src/content/docs/useform/unregister.mdx)).

**Evidence gap:** that table's "Value" column is ambiguous about whether it shows the whole form
values or the parent object, and the two rows are not obviously consistent with each other. The
implementation is unambiguous, and it *prunes*:

```ts
if (
  index !== 0 &&
  ((isObject(childObject) && isEmptyObject(childObject)) ||
   (Array.isArray(childObject) && isEmptyArray(childObject)))
) {
  unset(object, paths.slice(0, -1));
}
```

— [`src/utils/unset.ts`](https://github.com/react-hook-form/react-hook-form/blob/9b7af71b4d25da143fd2522c40d20466c7433f00/src/utils/unset.ts). **[from source]** removing the last child of a
parent removes the parent too, recursively.

### 5. Metadata survival — documented, and opt-out-able

`unregister` clears value, errors, dirty, touched and validating state by default, with explicit
`keepValue`, `keepError`, `keepDirty`, `keepTouched`, `keepIsValid`, `keepDefaultValue`,
`keepIsValidating` opt-outs — [unregister docs, Options table](https://react-hook-form.com/docs/useform/unregister).
Two rules are documented and relevant:

> - This method will remove input reference and its value, which means **built-in validation** rules
>   will be removed as well.
> - By `unregister` an input, it will not affect the schema validation.

RHF is the only surveyed library that treats "what happens to metadata when presence changes" as a
**first-class, per-flag, caller-controlled decision** rather than an emergent consequence. That is
directly applicable to issue open question 4.

Identity itself is the path string, so it is stable across a clear/re-create cycle **[from source]**
(both `set` and `unset` key off `stringToPath(path)`).

### 6. Absent vs present-and-default — collapsed

`PathValue` cannot distinguish them (both `| undefined`), and `unset`'s pruning actively converts
present-and-empty into absent.

### `@hookform/lenses` — the officially documented typed lens layer

RHF ships a documented `useLens` API backed by
[`@hookform/lenses`](https://react-hook-form.com/docs/uselens)
([source](https://github.com/react-hook-form/documentation/blob/da8518113d739026189285af39d79a5abdcbe755/src/content/docs/uselens.mdx)).
It is the closest JS analogue to dioform's `FieldPath`: a composable, type-safe focus with
`focus` / `reflect` / `map` / `interop`.

**Its answer to optionality is a type-level escape hatch, not an optic.** The documented methods are:

| Method | Description | Returns |
| --- | --- | --- |
| `narrow` | Type-safe narrowing of union types | `Lens<SubType>` |
| `assert` | Runtime type assertion for type narrowing | `void` |
| `defined` | Exclude null and undefined from lens type | `Lens<NonNullable>` |
| `cast` | Force type change (unsafe) | `Lens<NewType>` |

with the docs stating plainly:

> The `narrow` method performs type-level operations only. It doesn't validate the runtime value —
> use it when you have external guarantees about the value's type.

> `assert` is a type-only operation that doesn't perform runtime validation.

and framing the whole group as temporary:

> The `narrow`, `assert`, `defined`, and `cast` methods serve as escape hatches for current
> TypeScript limitations with lens type compatibility.

So the one JS library that genuinely calls its abstraction a *lens* handles optional parents by
**asserting the problem away at the type level**. A Rust library cannot take that route: `&Party`
from `&None` is not merely unsound, it is unconstructible.

---

## Formik

Formik's paths are lodash dot/bracket **strings with no type-level constraint at all**, so
sub-question 1's type-level half does not arise. `setFieldValue: (field: string, value: any, …)` —
[`packages/formik/src/types.tsx` L96–L100](https://github.com/jaredpalmer/formik/blob/main/packages/formik/src/types.tsx#L96-L100).
Path parsing is delegated to `lodash/toPath` —
[`utils.ts` L2](https://github.com/jaredpalmer/formik/blob/main/packages/formik/src/utils.ts#L2).
Documented: "To access nested objects or arrays, name can also accept lodash-like dot path like
`social.facebook` or `friends[0].firstName`" —
[`docs/api/field.md`](https://github.com/jaredpalmer/formik/blob/main/docs/api/field.md#L208).

**Read** — `getIn` walks and bails, returning an optional `def`:

```ts
export function getIn(obj: any, key: string | string[], def?: any, p: number = 0) {
  const path = toPath(key);
  while (obj && p < path.length) { obj = obj[path[p++]]; }
  if (p !== path.length && !obj) { return def; }
  return obj === undefined ? def : obj;
}
```

— [`utils.ts` L69–L86](https://github.com/jaredpalmer/formik/blob/main/packages/formik/src/utils.ts#L69-L86).
The consumer-facing read passes no `def`: `value: getIn(state.values, name)` —
[`Formik.tsx` L877–L885](https://github.com/jaredpalmer/formik/blob/main/packages/formik/src/Formik.tsx#L877-L885).

**Write** — `setIn` auto-creates, choosing array vs object by lookahead:

```ts
const nextPath: string = pathArray[i + 1];
resVal = resVal[currentPath] =
  isInteger(nextPath) && Number(nextPath) >= 0 ? [] : {};
```

— [`utils.ts` L118–L129](https://github.com/jaredpalmer/formik/blob/main/packages/formik/src/utils.ts#L118-L129),
with `isInteger = (obj) => String(Math.floor(Number(obj))) === obj` —
[`utils.ts` L20–L21](https://github.com/jaredpalmer/formik/blob/main/packages/formik/src/utils.ts#L20-L21).
The `else` branch fires for a *primitive* parent too, so an existing string at `a` is silently
replaced by an object.

**Documentation status: emergent, not deliberate.** Formik's utils page is a bare list of type
signatures ([`docs/api/utils.md`](https://github.com/jaredpalmer/formik/blob/main/docs/api/utils.md)),
and `setFieldValue`'s docs say only "`field` should match the key of `values` you wish to update" —
[`docs/api/formik.md` L196–L201](https://github.com/jaredpalmer/formik/blob/main/docs/api/formik.md#L196-L201) —
which if anything implies the key should already exist. **No Formik doc mentions absent parents,
auto-creation, or the numeric heuristic.** The closest acknowledgement that the tree is sparse is
about *errors*: "For the nested field errors, you should assume that no part of the object is
defined unless you've checked for it" —
[`docs/api/fieldarray.md` L149](https://github.com/jaredpalmer/formik/blob/main/docs/api/fieldarray.md#L149).

**Explicit presence** — `setFieldValue('a', undefined)` deletes the key
([`utils.ts` L136–L146](https://github.com/jaredpalmer/formik/blob/main/packages/formik/src/utils.ts#L136-L146)),
but Formik does **not** prune empty parents, so clearing the last child leaves `{a: {}}` behind,
which reaches the submit payload.

**Metadata survival** — `values`, `errors` and `touched` are three independent trees written by the
same `setIn` with the same string path
([`Formik.tsx` L81–L91](https://github.com/jaredpalmer/formik/blob/main/packages/formik/src/Formik.tsx#L81-L91)),
and the field registry is a flat `Record<string, {validate}>`
([`Formik.tsx` L536–L544](https://github.com/jaredpalmer/formik/blob/main/packages/formik/src/Formik.tsx#L536-L544)).
Identity is stable; stale `touched`/`errors` survive a clear and are still present on re-create.
The only place Formik deliberately keeps the three trees in sync is `FieldArray`, which explicitly
rewrites the matching slices — "We need to make sure we also remove relevant pieces of `touched` and
`errors`" —
[`FieldArray.tsx` L171–L216](https://github.com/jaredpalmer/formik/blob/main/packages/formik/src/FieldArray.tsx#L171-L216).
**No equivalent exists for plain optional objects.**

---

## Final Form / React Final Form

Final Form is the only JS library surveyed that **documents the auto-creation semantics
deliberately**, on a dedicated concept page.

[final-form.org/docs/final-form/field-names](https://final-form.org/docs/final-form/field-names)
states four rules verbatim:

> 1. `.` and `[` are treated the same.
> 2. `]` is ignored.
> 3. `Number` keys will result in array structures.
> 4. Setting `undefined` to a field value deletes any empty object – but not array! – structures.

with a worked table (`bar.frog` on `{}` → `{bar: {frog: 'foo'}}`; `bar[1]` on `{}` →
`{bar: [null, 'foo']}`; `bar.frog = undefined` on `{bar:{frog:'foo'}, other:42}` → `{other: 42}`),
and frames the whole thing as "very similar to Lodash's `_.set()`" plus the empty-object cleanup.

**Type level:** paths are untyped in the React binding (`FieldProps.name: string` —
[`react-final-form/src/types.ts` L22](https://github.com/final-form/react-final-form/blob/main/src/types.ts#L22)).
The `FormApi` surface types names as `keyof FormValues` —
[`final-form/src/types.ts` L232–L255](https://github.com/final-form/final-form/blob/main/src/types.ts#L232-L255) —
a **flat** key, not a path, so parameterising `FormApi<MyValues>` makes `form.change('a.b', …)` a
type error even though it is the documented supported syntax. The type system models flat keys, not
paths, and never expresses parent optionality.

**Read** — `undefined`, with no `def` parameter:

```ts
if (current === undefined || current === null || typeof current !== "object" ||
    (Array.isArray(current) && isNaN(Number(key)))) { return undefined; }
```

— [`src/structure/getIn.ts` L4–L21](https://github.com/final-form/final-form/blob/main/src/structure/getIn.ts#L4-L21),
covered by tests named "should return undefined when state is undefined" etc. —
[`getIn.test.ts`](https://github.com/final-form/final-form/blob/main/src/structure/getIn.test.ts).
The React binding then maps `undefined` → `''` before it reaches the input
(`defaultFormat` / `defaultParse` —
[`react-final-form/src/useField.ts` L25–L28](https://github.com/final-form/react-final-form/blob/main/src/useField.ts#L25-L28)),
documented under [FieldProps](https://final-form.org/docs/react-final-form/types/FieldProps).

**Write** — auto-creates, with a stricter numeric predicate than Formik's, applied to the *current*
segment:

```ts
const isValidArrayIndex = (key: string): boolean => {
  const num = Number(key);
  return !isNaN(num) && Number.isInteger(num) && num >= 0 && String(num) === key;
};
```

— [`src/structure/setIn.ts` L6–L18](https://github.com/final-form/final-form/blob/main/src/structure/setIn.ts#L6-L18).
Unlike Formik it *throws* on a type mismatch rather than silently replacing:
`throw new Error("Cannot set a non-numeric property on an array")` —
[`setIn.ts` L49–L51](https://github.com/final-form/final-form/blob/main/src/structure/setIn.ts#L49-L51).

**Pruning** — the headline difference from Formik. Setting the last child to `undefined` collapses
the parent, recursively:

```ts
if (result === undefined) {
  const numKeys = Object.keys(current).length;
  if ((current as any)[key] === undefined && numKeys === 0) { return undefined; }
  if ((current as any)[key] !== undefined && numKeys <= 1) {
    if (isValidArrayIndex(path[index - 1]) && !destroyArrays) { return {}; }
    else { return undefined; }
  }
  …
}
```

— [`setIn.ts` L60–L82](https://github.com/final-form/final-form/blob/main/src/structure/setIn.ts#L60-L82),
tested as "should delete structure when setting undefined" —
[`setIn.test.ts` L135–L290](https://github.com/final-form/final-form/blob/main/src/structure/setIn.test.ts#L135-L290).
The form absorbs the total-prune case with `(setIn(...) || {})` —
[`FinalForm.ts` L247–L251](https://github.com/final-form/final-form/blob/main/src/FinalForm.ts#L247-L251).

Combined with `defaultParse` mapping `''` → `undefined`, **a user backspacing a text input can
delete the optional parent object entirely.** That is the mirror image of materialise-on-write:
*dematerialise-on-clear*, and it is the same class of "the library invents a presence decision the
user did not make".

**Metadata survival** — field records live in a flat `state.fields` map keyed by the full path
([`FinalForm.ts` L920–L976](https://github.com/final-form/final-form/blob/main/src/FinalForm.ts#L920-L976)),
deleted only on unregister teardown
([`FinalForm.ts` L1075–L1085](https://github.com/final-form/final-form/blob/main/src/FinalForm.ts#L1075-L1085)),
never as a consequence of a value change. Verified by executing `final-form@5.0.1`: after
`change('a.b', undefined)` the parent is pruned from `values` but `touched: true`, `visited: true`,
`modified: true` and `error: 'required'` all persist, `getRegisteredFields()` still reports
`['a.b']`, and re-creating the value leaves `touched` still `true`. Resetting requires the explicit
`resetFieldState` — [FormApi docs](https://final-form.org/docs/final-form/types/FormApi).

**Asymmetry worth flagging:** errors *are* pruned with values on unregister
([`FinalForm.ts` L1083–L1085](https://github.com/final-form/final-form/blob/main/src/FinalForm.ts#L1083-L1085)),
but `touched`/`visited` are not, because they live on a different structure. Two kinds of per-field
metadata get two different presence policies in the same library.

---

## The optics framing

This is the sharpest available vocabulary for the choice in #25, and it is worth stating precisely
because the three kinds correspond one-to-one to the candidate designs.

### The three kinds

**Lens** — focuses *exactly one* value. From the `optics` overview:

> A `Lens' S A` captures the structure of `A` being a field of `S`, with the projection function
> given by `view` and the update function by `set`.

— [`optics/src/Optics.hs` L278–L297](https://github.com/well-typed/optics/blob/5eb61a85180dd5177ebe11d38b47d1d7a7caadce/optics/src/Optics.hs#L278-L297).

This is exactly what dioform's `FieldPath` is: `for<'a> fn(&'a Model) -> &'a Value` plus the `&mut`
equivalent is a total getter/setter pair, i.e. a lens. That is why it cannot traverse an `Option`:
a lens is *by definition* total.

**Prism** — focuses *zero or one*, **and can construct the outer value from the inner**:

> projecting out `A` from `S` (pattern-matching on the constructor) may fail, so it has type
> `S -> Maybe A`. In the reverse direction we have a function of type `A -> S` representing the
> constructor itself.
>
> ```
>         _Left  :: Prism' (Either X Y) X
> preview _Left  :: Either X Y -> Maybe X
> review  _Right :: Y -> Either X Y
> ```

— [`optics/src/Optics.hs` L306–L326](https://github.com/well-typed/optics/blob/5eb61a85180dd5177ebe11d38b47d1d7a7caadce/optics/src/Optics.hs#L306-L326).

In `lens`, the constructive direction is documented by doctest: `_Just # 5` ≡ `Just 5` —
[`src/Control/Lens/Prism.hs` L300–L325](https://github.com/ekmett/lens/blob/a7c08e1cbeb2a4a937854dbbbe7b21cfab16eff2/src/Control/Lens/Prism.hs#L300-L325).
`_Just` itself is `prism Just $ maybe (Left Nothing) Right` (same lines).

**Affine Traversal** — focuses *zero or one*, **cannot construct**:

> An `AffineTraversal` is a `Traversal` that applies to at most one element. These arise most
> frequently as the composition of a `Lens` with a `Prism`.
>
> ```
> preview :: AffineTraversal s t a b -> s -> Maybe a
> over    :: AffineTraversal s t a b -> (a -> b) -> s -> t
> set     :: AffineTraversal s t a b ->       b  -> s -> t
> ```

— [`optics-core/src/Optics/AffineTraversal.hs` L1–L45](https://github.com/well-typed/optics/blob/5eb61a85180dd5177ebe11d38b47d1d7a7caadce/optics-core/src/Optics/AffineTraversal.hs#L1-L45).

Its constructor is a matcher plus an updater, and the updater is only reached on a match:

```haskell
atraversal :: (s -> Either t a) -> (s -> b -> t) -> AffineTraversal s t a b
atraversal match update = Optic $
  dimap (\s -> (match s, update s))
        (\(etb, f) -> either id f etb)
  . first' . right'
```

— [`Optics/AffineTraversal.hs` L104–L116](https://github.com/well-typed/optics/blob/5eb61a85180dd5177ebe11d38b47d1d7a7caadce/optics-core/src/Optics/AffineTraversal.hs#L104-L116).
**[from source]** on a `Left t` the result is `either id f (Left t)` = `t`, i.e. the original
structure unchanged — **set through an absent focus is a no-op**.

### The composition rule — and why it matters here

> the constraint `JoinKinds A_Lens A_Prism k` makes GHC infer that `k` must be `An_AffineTraversal`.

— [`optics/src/Optics.hs` L520–L545](https://github.com/well-typed/optics/blob/5eb61a85180dd5177ebe11d38b47d1d7a7caadce/optics/src/Optics.hs#L520-L545).

**Lens ∘ Prism = AffineTraversal, not Prism.** Composing "field of the model" with "the `Just` case"
produces something that can *read zero-or-one* and *write-if-present*, and has **lost** the ability
to construct. That loss is not a design choice anyone made; it is forced. You cannot rebuild
`Model` from a `String` just because you know how to rebuild `Option<Party>` from a `Party`.

This is the precise formal reason materialise-on-write cannot be derived from composition. Any
library that materialises must be supplied with the missing information — the parent's default —
from outside the optic.

### Set through an absent focus is a documented no-op

`lens` documents this by doctest on the prisms themselves:

```
>>> over _Left (+1) (Left 2)
Left 3
>>> over _Left (+1) (Right 2)
Right 2

>>> over _Right (+1) (Left 2)
Left 2
>>> over _Right (+1) (Right 2)
Right 3
```

— [`src/Control/Lens/Prism.hs` L250–L296](https://github.com/ekmett/lens/blob/a7c08e1cbeb2a4a937854dbbbe7b21cfab16eff2/src/Control/Lens/Prism.hs#L250-L296).

And reading is documented as `Maybe`: `Nothing ^? _Just` ≡ `Nothing` —
[`Prism.hs` L316–L322](https://github.com/ekmett/lens/blob/a7c08e1cbeb2a4a937854dbbbe7b21cfab16eff2/src/Control/Lens/Prism.hs#L316-L322).

**[from source]** There is no doctest literally showing `over _Just f Nothing`; the claim for `_Just`
specifically follows from `_Just = prism Just $ maybe (Left Nothing) Right` composed with
`prism bt seta = dimap seta (either pure (fmap bt)) . right'`
([`Prism.hs` L124–L135, L322–L325](https://github.com/ekmett/lens/blob/a7c08e1cbeb2a4a937854dbbbe7b21cfab16eff2/src/Control/Lens/Prism.hs#L124-L135)),
which returns `pure t` — the original — on the non-matching branch. The `_Left`/`_Right` doctests
above exercise the identical code path.

### The `at` / `ix` split — the closest prior art to #25's actual question

This is the single most directly transferable finding. The optics ecosystem faced exactly the
"address a value that may not be there, and decide whether writing creates it" problem for
`Map`-like containers, and **answered it with two separate optics rather than one optic with a
policy.**

```haskell
-- | Provides a simple 'AffineTraversal' lets you traverse the value at a given key …
class Ixed m where
  -- | /NB:/ Setting the value of this 'AffineTraversal' will only set the value
  -- in 'at' if it is already present.
  --
  -- If you want to be able to insert /missing/ values, you want 'at'.
  ix :: Index m -> Optic' (IxKind m) NoIx m (IxValue m)
```

— [`optics-core/src/Optics/At/Core.hs` L134–L160](https://github.com/well-typed/optics/blob/5eb61a85180dd5177ebe11d38b47d1d7a7caadce/optics-core/src/Optics/At/Core.hs#L134-L160)
(identical wording in `lens`:
[`src/Control/Lens/At.hs` L214–L237](https://github.com/ekmett/lens/blob/a7c08e1cbeb2a4a937854dbbbe7b21cfab16eff2/src/Control/Lens/At.hs#L214-L237)).

```haskell
-- | 'At' provides a 'Lens' that can be used to read, write or delete the value
-- associated with a key in a 'Map'-like container on an ad hoc basis.
--
-- An instance of 'At' should satisfy:
--
-- @
-- 'ix' k ≡ 'at' k '%' '_Just'
-- @
class (Ixed m, IxKind m ~ An_AffineTraversal) => At m where
  at :: Index m -> Lens' m (Maybe (IxValue m))
```

— [`Optics/At/Core.hs` L379–L401](https://github.com/well-typed/optics/blob/5eb61a85180dd5177ebe11d38b47d1d7a7caadce/optics-core/src/Optics/At/Core.hs#L379-L401)
(`lens` states the same law as `ix k ≡ at k . traverse` —
[`Control/Lens/At.hs` L458–L476](https://github.com/ekmett/lens/blob/a7c08e1cbeb2a4a937854dbbbe7b21cfab16eff2/src/Control/Lens/At.hs#L458-L476)),
with the default `ix` being literally `ixAt = \i -> at i % _Just` —
[`Optics/At/Core.hs` L165–L169](https://github.com/well-typed/optics/blob/5eb61a85180dd5177ebe11d38b47d1d7a7caadce/optics-core/src/Optics/At/Core.hs#L165-L169).

Read this against dioform's current surface:

| optics | dioform today |
| --- | --- |
| `at k :: Lens' m (Maybe v)` — explicit presence: read, write, delete | **already exists**: `FieldPath<Model, Option<Party>>`, bindable as a whole value |
| `ix k :: AffineTraversal' m v` — traverse the inner value only if present | **missing** — this is the entire gap in #25 |
| `ix k ≡ at k % _Just` | the composition dioform cannot express, because `join` needs a total child accessor |

The ecosystem's answer is: keep the presence lens and the inner-field affine traversal as **two
distinct things with different types**, and relate them by a law. Do not give one optic a
materialise-or-not policy flag.

### `non` — materialise-on-write exists, as an *opt-in combinator that demands a default*

The optics libraries do support "treat absent as a default value and materialise on write" — but as
a separate, explicitly-invoked `Iso`, not as a property of traversal:

```
>>> Map.fromList [] ^. at "hello" . non 0
0
>>> Map.fromList [("hello",1)] & at "hello" . non 0 -~ 1
fromList []
>>> Map.empty & at "hello" . non Map.empty . at "world" ?~ "!!!"
fromList [("hello",fromList [("world","!!!")])]
>>> Map.fromList [("hello",Map.fromList [("world","!!!")])] & at "hello" . non Map.empty . at "world" .~ Nothing
fromList []

non :: Eq a => a -> Iso' (Maybe a) a
```

— [`src/Control/Lens/Iso.hs` L288–L322](https://github.com/ekmett/lens/blob/a7c08e1cbeb2a4a937854dbbbe7b21cfab16eff2/src/Control/Lens/Iso.hs#L288-L322)
(same in [`optics-core/src/Optics/Iso.hs`](https://github.com/well-typed/optics/blob/5eb61a85180dd5177ebe11d38b47d1d7a7caadce/optics-core/src/Optics/Iso.hs)).

Three properties of `non` are directly relevant to #25:

1. **It requires a caller-supplied default and an `Eq` bound.** The information needed to
   materialise cannot come from the optic; it is a call-site argument. This matches the issue's
   criterion that "any `T: Default`-style bound stays opt-in at the call site".
2. **It is an `Iso' (Maybe a) a` — a bijection.** That is the formal statement that
   *absent and present-and-default are deliberately identified*. You get materialise-on-write only
   by explicitly accepting the collapse.
3. **The collapse is symmetric.** Writing the default value *removes* the parent
   (`& at "hello" . non 0 -~ 1` → `fromList []`). Materialise-on-write without
   dematerialise-on-default-write would be a strictly weaker, non-isomorphic thing that the optics
   ecosystem does not offer.

`non'` and `anon` generalise the "which value counts as empty" predicate —
[`Control/Lens/Iso.hs` L324–L349](https://github.com/ekmett/lens/blob/a7c08e1cbeb2a4a937854dbbbe7b21cfab16eff2/src/Control/Lens/Iso.hs#L324-L349).

---

## Rust optics crates

**Reporting honestly: the ecosystem is thin and largely abandoned.** [crates.io keyword
`optics`](https://crates.io/keywords/optics) returns 23 crates in total and [keyword
`lens`](https://crates.io/keywords/lens) 26, most of which are physical optics (lasers, ray tracing)
or camera-lens correction. Counts fetched from the crates.io API 2026-08-15.

### Crates that give a `&mut` affine traversal — three, all unusable in practice

**`lens-rs`** — [crates.io](https://crates.io/crates/lens-rs) ·
[`lens-rs/src/traits.rs`](https://github.com/TOETOE55/lens-rs/blob/master/lens-rs/src/traits.rs).
0.3.2, released **2021-05-07**; 22,952 all-time downloads. It reproduces the Haskell hierarchy with
`&mut` accessors and no `Clone` bound:

```rust
pub trait PrismMut<Optics, Image: ?Sized>: PrismRef<Optics, Image> + TraversalMut<Optics, Image> {
    fn preview_mut(&mut self, optics: Optics) -> Option<&mut Image>;
}
pub trait LensMut<Optics, Image: ?Sized>: LensRef<Optics, Image> + PrismMut<Optics, Image> {
    fn view_mut(&mut self, optics: Optics) -> &mut Image;
}
```

`LensRef: PrismRef` encodes the subtyping directly, so composing a lens with a prism is consumed
through the `Prism*` API and yields `Option<&mut T>`. Construction is a **separate** `Review` trait,
not a property of the setter — the same split as Haskell's `at` / `ix`
([`lens-rs/src/lib.rs`](https://github.com/TOETOE55/lens-rs/blob/master/lens-rs/src/lib.rs)).

The architectural catch matters for dioform: in `lens-rs` the optic is a **type-level path**
(`Optics![_1.Ok._1]`) whose impls are generated onto the *data* type, so there is no first-class
runtime optic value to store in a `FieldPath<Model, Value>` struct. Membership is expressed as a
trait bound. It also needs build-script codegen, and its own docs list limits: "can't derive `Lens`
for enum", "can't derive `Prism` and `Review` for the variant has more than one argument or has
named field" (same `lib.rs`).

**`smart_access`** — [docs.rs](https://docs.rs/smart_access). 0.7.0, released **2020-07-13**. It
self-describes as "a minimalistic 'lens' (more precisely, affine traversal) library using an
opinionated imperative approach", and uses the CPS dual of `Option<&mut T>`:

```rust
pub trait At<Index> {
    type View: ?Sized;
    fn access_at<R, F>(&mut self, i: Index, f: F) -> Option<R>
    where F: FnOnce(&mut Self::View) -> R;
}
```

The absent-focus contract is documented on the trait: *"Otherwise `None` **must** be returned and
`self` must stay unchanged. In essence `access_at` returns `None` if and only if `self` has not been
touched."* The docs also flag the cost of the CPS shape: *"The following two cases are
indistinguishable: a view couldn't be obtained (and thus `f` had not been called); `f` had been
called but failed to mutate the view in a meaningful way."*

Notably, `smart_access` separates no-op traversal from construct-if-missing **at the index type**:
`Ensure { key, value }` is a different accessor that inserts a default. A third structural option
alongside "policy flag" and "two optics".

**`enso-optics`** — [crates.io](https://crates.io/crates/enso-optics), 0.2.0, released
**2021-05-12**. Its repository field points at a GitHub repo that 404s. Read from the published
tarball, `src/lib.rs` contains exactly the pair dioform would need:

```rust
trait OptGetter<T>: HasField<T> {
    fn get     (&    self) -> Option <&    Field<Self, T>>;
    fn get_mut (&mut self) -> Option <&mut Field<Self, T>>;
}
```

composed by `and_then` over an HList path, with total lenses lifted by wrapping in `Some(...)` and
`Option` traversal falling out of `mk_lenses_for!(Option<T>::Some{val: T})`. Writes are no-ops when
absent (`let r = self.get(); r.map(|s| *s = val);`), and its own test walks
`lens_mut!(foo.bar.baz.Some.qux.Some.quxx).set(...)` through two `Option`s.

**[from source]** Every one of those items is private and `mk_lenses_for` is not `#[macro_export]`ed;
the only `pub` item at module scope is `pub fn main()`, which is why
[docs.rs](https://docs.rs/enso-optics/0.2.0/enso_optics/) renders "doesn't have any documentation".
**The crate cannot be used as a dependency.** It is a design reference only.

### The maintained crates either drop optionality or auto-construct

**`optics`** ([crates.io](https://crates.io/crates/optics), 0.3.0, released 2025-06-05, 24 recent
downloads) is the only recent general optics crate. Its README says *"This is a **pre-release**, and
the code is **unfinished**"* and *"This is a **layman's implementation** of optics"* —
[README](https://github.com/axos88/optics-rs/blob/master/README.md). It is **owned/clone-based**, not
`&mut`: `fn try_get(&self, source: &S) -> Result<A, Self::GetterError>`
([`src/base/getter.rs`](https://github.com/axos88/optics-rs/blob/master/src/base/getter.rs)), and the
getter doc admits *"you will likely need to Clone or Copy the result in order to extract it from the
source."* Its traversal module is one line: `//TODO`
([`src/optics/traversal/mod.rs`](https://github.com/axos88/optics-rs/blob/master/src/optics/traversal/mod.rs)).

Most importantly, **its composed write auto-constructs**:

```rust
fn set(&self, source: &mut S, value: A) {
    if let Ok(mut i) = self.optic1.try_get(source).map_err(self.error_fn_1) {
        self.optic2.set(&mut i, value);
        self.optic1.set(source, i);
    }
}
```

— [`src/optics/prism/composed.rs`](https://github.com/axos88/optics-rs/blob/master/src/optics/prism/composed.rs).
**[from source]** Combined with its own `Option` prism example whose setter is
`*source = Some(value)`
([`examples/extend_with_some_prism.rs`](https://github.com/axos88/optics-rs/blob/master/examples/extend_with_some_prism.rs)),
composing `lens(Model → Option<Party>)` with that prism and writing while the field is `None`
succeeds and materialises the `Some`. That is prism semantics substituted for affine-traversal
semantics — the exact conflation Haskell's `at`/`ix` split exists to prevent.

**Druid** is worth reading and officially dead — its
[README](https://github.com/linebender/druid/blob/master/README.md) states *"**UNMAINTAINED** — The
Druid project has been discontinued."* (last release 0.8.3, 2023-02-28). `druid::Lens` is CPS and
**total**:

```rust
pub trait Lens<T: ?Sized, U: ?Sized> {
    fn with<V, F: FnOnce(&U) -> V>(&self, data: &T, f: F) -> V;
    fn with_mut<V, F: FnOnce(&mut U) -> V>(&self, data: &mut T, f: F) -> V;
}
```

— [`druid/src/lens/lens.rs`](https://github.com/linebender/druid/blob/master/druid/src/lens/lens.rs)
/ [docs.rs](https://docs.rs/druid/0.8.3/druid/lens/trait.Lens.html). There is no `Option` anywhere in
the trait or its combinators; the CPS shape exists for copy-on-write (`InArc`), not optionality.

`druid_widget_nursery::Prism` is `get(&self, &T) -> Option<U>` / `put(&self, &mut T, U)` with owned
`U`, and its doc comment says *"This is just a simple prototype for me to work with until [#1136] is
merged"* — [`src/prism.rs`](https://github.com/linebender/druid-widget-nursery/blob/master/src/prism.rs),
referencing [druid#1136](https://github.com/linebender/druid/pull/1136) and
[druid#1135](https://github.com/linebender/druid/issues/1135), which never landed. **There is no
composition operator at all** — no way to compose `Lens<A,B>` with `Prism<B,C>`; callers do
read-modify-write by hand. And `put` auto-constructs (`*data = Some(inner)`).

### Crates whose lens is dioform's exact signature — and stopped there

The most useful negative result: several independent Rust designs converged on the same infallible
accessor pair dioform has, and **none added an optional variant.**

| crate | shape | status |
| --- | --- | --- |
| [`pl-lens`](https://crates.io/crates/pl-lens) ([source](https://github.com/plausiblelabs/lens-rs/blob/master/src/lens.rs)) | `get_ref(&self, &'a Source) -> &'a Target` + `get_mut_ref(&self, &'a mut Source) -> &'a mut Target` | 1.0.1, **2020-09-04**. No `Option` or `Prism` in `lens.rs`. |
| [`xilem_core::lens`](https://github.com/linebender/xilem/blob/main/xilem_core/src/views/lens.rs) | `for<'a> Fn(&'a mut ParentState) -> &'a mut ChildState` | xilem 0.4.0, 2025-10-29. Druid's successor: a plain closure, **no optics abstraction**, no optional counterpart. |
| [`pathmod`](https://crates.io/crates/pathmod) | `Accessor::from_fns(get_ref: fn(&T) -> &F, get_mut: fn(&mut T) -> &mut F)`, stored as a byte offset, composed by offset addition | 2025-09-02. Closest published analogue of `FieldPath<Model, Value>`. No `Option` variant. |
| [`frunk`](https://github.com/lloydmeta/frunk/blob/master/core/src/path.rs) ([docs](https://docs.rs/frunk_core/latest/frunk_core/path/index.html)) | `trait PathTraverser<Path, Indices> { type TargetValue; fn get(self) -> Self::TargetValue; }` | 0.5.0, 2026-07-04 — the only actively maintained one. But `get(self)` **consumes**, there is **no `get_mut`**, and `path.rs` has no `Option`/`Coproduct` handling. Not usable for mutation. |
| [`photonix`](https://crates.io/crates/photonix) | has `GetOption`/`SetOption`/`ModifyOption`, but consuming-self and owned (`get_option(self) -> Option<Value>`) | 0.1.1, **2019-02-21**. |
| [`lenses`](https://crates.io/crates/lenses) | `&Fn(&S) -> A` getters, separate `ModLens` with `&'a mut S -> &'a mut A` | 0.1.0, **2018-10-24**, no repository field. |

[`partial_ref`](https://crates.io/crates/partial_ref) is sometimes suggested here but is orthogonal:
it type-checks *disjoint simultaneous* borrows of several fields, not optionality.

Two newer, tiny crates — [`karpal-optics`](https://crates.io/crates/karpal-optics) (0.8.0,
2026-07-22) and [`id_effect_optics`](https://crates.io/crates/id_effect_optics) (0.4.0, 2026-07-12) —
both expose owned `preview(&self, &S) -> Option<A>` and neither exposes `Option<&mut T>`.

### The untyped point of comparison: `serde_json::Value::pointer_mut`

```rust
pub fn pointer(&self, pointer: &str) -> Option<&Value>
pub fn pointer_mut(&mut self, pointer: &str) -> Option<&mut Value>
```

— [docs.rs](https://docs.rs/serde_json/latest/serde_json/enum.Value.html#method.pointer_mut).
**[from source]** `pointer_mut` does **not** create missing parents; it is a `try_fold` that bails to
`None` on the first missing or wrong-shaped segment
([`src/value/mod.rs`](https://github.com/serde-rs/json/blob/master/src/value/mod.rs)):

```rust
pointer.split('/').skip(1).map(|x| x.replace("~1", "/").replace("~0", "~"))
    .try_fold(self, |target, token| match target {
        Value::Object(map) => map.get_mut(&token),
        Value::Array(list) => parse_index(&token).and_then(move |x| list.get_mut(x)),
        _ => None,
    })
```

The prose docs say only "The addressed value is returned and if there is no such value `None` is
returned" — the no-auto-create behaviour is not stated, so it is cited from source. This is the
single highest-adoption Rust example of an affine traversal, and it composes by `and_then` returning
`Option<&mut T>`.

### What the Rust survey establishes

- **No maintained crate implements a `&mut` affine traversal.** The three that do last shipped in
  2020, 2021, and 2021-but-exports-nothing.
- **The three working designs independently chose the same shape:** `Option<&mut T>` composed by
  `and_then` (`lens-rs::preview_mut`, `enso-optics::OptResolver::resolve_mut`,
  `serde_json::pointer_mut`). A total lens lifts into that shape trivially (`|m| Some(f(m))`), which
  is precisely the `LensRef: PrismRef` subtyping in `lens-rs`.
- **The CPS form loses information.** `smart_access` documents that "focus absent" and "closure ran
  but changed nothing" are indistinguishable under `access_at`; returning `Option<&'a mut Value>`
  keeps them apart.
- **The crates that conflate prism and affine traversal auto-construct.** `optics` and
  `druid-widget-nursery` both materialise on write, and neither documents it as a decision.
- **The crates that store a first-class runtime path value** (`pathmod`, and dioform itself) are
  exactly the ones that could not express optionality; the ones that could express optionality
  (`lens-rs`, `enso-optics`) encode the path at the type level with no runtime value. **Nothing on
  crates.io stores a first-class fn-pointer pair of the form
  `(for<'a> fn(&'a Model) -> Option<&'a Value>, for<'a> fn(&'a mut Model) -> Option<&'a mut Value>)`.**
  That combination is unattested, not disproven.

## Typed non-JS form libraries

Where the type system forces the question instead of letting `undefined` paper over it. Two findings
carry most of the weight; the rest is surveyed briefly because it is mostly negative.

### `reactive_stores` (Leptos) — literally dioform's struct, and it addresses through `Option`

This is the closest analogue found anywhere. [`reactive_stores/src/subfield.rs` L21–L29](https://github.com/leptos-rs/leptos/blob/main/reactive_stores/src/subfield.rs):

```rust
pub struct Subfield<Inner, Prev, T> {
    path_segment: StorePathSegment,
    inner: Inner,
    read: fn(&Prev) -> &T,
    write: fn(&mut Prev) -> &mut T,
    ...
}
```

The same infallible fn-pointer pair as `FieldPath<Model, Value>`, plus two things dioform's does not
carry: an `inner` so paths compose structurally, and a `path_segment` so paths have identity.

**Addressing (1)** — yes, type-checked, and **the accessor pair is not changed**. The `Option` hop is
made an ordinary path segment with panicking accessors, exposed through an extension trait
([`reactive_stores/src/option.rs` L53–L60](https://github.com/leptos-rs/leptos/blob/main/reactive_stores/src/option.rs)):

```rust
fn unwrap(self) -> Subfield<Self, Option<Self::Output>, Self::Output> {
    Subfield::new(self, 0.into(), |t| t.as_ref().unwrap(), |t| t.as_mut().unwrap())
}
```

so `store.name().unwrap().first_name()` type-checks end to end (their own test
`substores_reachable_through_option`, same file).

**Read (2) / Write (3)** — **[from source]** `.unwrap()` panics on `None`, in both directions; there
is no auto-create anywhere in `option.rs`. Neither the panic nor the absence of auto-create is
documented. The *safe* route is documented, and it is the affine-traversal shape: `map()` "returns
`None` if the subfield is currently `None`… will cause it to re-run if the field toggles between
`None` and `Some(_)`" (same file).

**Presence (4)** — free, and by the same mechanism dioform already has: the `Option<Name>` field is
itself a `Subfield<_, User, Option<Name>>`, so `store.name().set(None)` works directly.

**Metadata survival (5) — the most decision-relevant single data point in this section.** `StorePath`
is `Vec<StorePathSegment>` where `StorePathSegment(usize)`
([`reactive_stores/src/path.rs`](https://github.com/leptos-rs/leptos/blob/main/reactive_stores/src/path.rs)),
and the reactive trigger registry is keyed **by that structural path**, not by an allocated node id:
`struct TriggerMap(FxHashMap<StorePath, StoreFieldTrigger>)`
([`reactive_stores/src/lib.rs`](https://github.com/leptos-rs/leptos/blob/main/reactive_stores/src/lib.rs)),
reached through `fn get_trigger(&self, path: StorePath) -> StoreFieldTrigger`
([`store_field.rs`](https://github.com/leptos-rs/leptos/blob/main/reactive_stores/src/store_field.rs)).
So a subfield's identity is a pure function of its position in the model, entirely independent of
whether any ancestor currently holds a value.

Their `patch` test ([`option.rs`](https://github.com/leptos-rs/leptos/blob/main/reactive_stores/src/option.rs))
drives `Some(Inner{first:"A",second:"C"})` → `None` → `Some(Inner{first:"A",second:"B"})` and asserts
`inner_first_count` is `2` both before and after the final restore — **the `first` subscriber does
not re-run when the parent comes back, because its trigger was found at the same path and its value
was unchanged.** Being precise about what this does and does not show: the count *does* increment on
the transition *into* `None` (1 → 2), and the test's stated purpose is that `patch` limits
notifications to genuinely changed fields, not identity stability as such. It uses
`map_untracked` for exactly that reason, with an inline comment noting that real code should use
`map()` so presence toggles *are* tracked. The identity claim rests on `TriggerMap` being
path-keyed; the test is corroboration, not the proof.

Caveat: segments are declaration indices, so paths survive value mutation but not a struct field
reorder.

**Absent vs default (6)** — fully distinguished; it is a real `Option` in the model.

Two further observations. The derive already generalises past `Option` to arbitrary enums by
returning `Option<Subfield<..>>` after a `matches!` check, with the honest panic message
`"accessed an enum field that is no longer matched"`
([`reactive_stores_macro/src/lib.rs`](https://github.com/leptos-rs/leptos/blob/main/reactive_stores_macro/src/lib.rs))
— an admission that a path can go stale between construction and use. And a path through an optional
parent inherently has **two** subscriptions (parent presence, leaf value); `map()` vs
`map_untracked()` exists so callers choose whether reading the leaf subscribes to the parent's
presence.

### `hecrj/composable-form` (Elm) — make the problem not exist

The other instructive answer, and the opposite strategy. Its field primitive is dioform's accessor
pair in setter form ([`src/Form/Base.elm`](https://github.com/hecrj/composable-form/blob/master/src/Form/Base.elm)):

```elm
type alias FieldConfig attrs input values output =
    { parser : input -> Result String output
    , value : values -> input
    , update : input -> values -> values
    , error : values -> Maybe String
    , attributes : attrs
    }
```

and nesting is a **total lens**
([`src/Form.elm`](https://github.com/hecrj/composable-form/blob/master/src/Form.elm)):

```elm
mapValues : { value : a -> b, update : b -> a -> a } -> Form b output -> Form a output
```

whose documented example has `SignupValues` containing `address : AddressValues` — **not
`Maybe AddressValues`**. The editing-state type is always total and always present (typically all
`String`s); `Maybe` lives only in the *parsed output*. A total lens is therefore always sufficient
and the optional-parent question never arises.

Presence is **derived, not stored**. From the `optional` docs (same file):

> Make a form optional. An optional form succeeds when: all of its fields are **empty**, producing
> `Nothing`; all of its fields are **correct**, producing `Just` the `output`. … This `websiteForm`
> will only be valid if **both** fields are blank, or **both** fields are filled correctly.

Implementation ([`Form/Base.elm`](https://github.com/hecrj/composable-form/blob/master/src/Form/Base.elm)):
on `Err`, if the filled form is empty it wipes every child field's error and returns `Ok Nothing`;
otherwise it propagates. So absent and present-and-default are **collapsed**, and a partially filled
optional section is one opaque group error with no "clear this section" affordance.

Metadata: there is nothing to survive. `Form values output field` is a *pure function*
`values -> FilledForm output field`; errors are recomputed on every fill. The only persistent
metadata in the library is `showFieldError : Set String` in `Form.View`, keyed by the field's
human-readable label, insert-only on blur, never removed
([`src/Form/View.elm`](https://github.com/hecrj/composable-form/blob/master/src/Form/View.elm)).
There is no `touched` and no `dirty` anywhere in the codebase.

The discriminated-union escape hatch is also instructive: `andThen`'s documented example branches a
`selectField` into two sub-forms while `Values` stays **one flat record with fields shared across
both branches**. Switching the discriminator does not clear the other branch's state; it stops
including those fields, and re-selecting restores the text. **The union is stored as a flattened
superset.** That is a concrete blueprint for modelling `Option<T>` as `(present: bool, T)`. Relatedly,
`Form.list`'s config carries `default : elementValues`
([`src/Form/Base/FormList.elm`](https://github.com/hecrj/composable-form/blob/master/src/Form/Base/FormList.elm))
— "create the parent" is made total by demanding a default up front at form-definition time.

The design ported unchanged to F#:
[`Fable.Form/Base.fs`](https://github.com/MangelMaxime/Fable.Form/blob/master/packages/Fable.Form/Base.fs)
has the same `mapValues` and the same emptiness machinery.

### Play Framework (Scala) `OptionalMapping` — the same lift, with a better emptiness rule

[`core/play/src/main/scala/play/api/data/Form.scala`](https://github.com/playframework/playframework/blob/main/core/play/src/main/scala/play/api/data/Form.scala):

```scala
case class OptionalMapping[T](wrapped: Mapping[T], ...) extends Mapping[Option[T]] {
  def bind(data: Map[String, String]): Either[Seq[FormError], Option[T]] = {
    data.keys
      .filter(p => p == key || p.startsWith(s"$key.") || p.startsWith(s"$key["))
      .map(k => data.get(k).filterNot(_.isEmpty))
      .collectFirst { case Some(v) => v }
      .map(_ => wrapped.bind(data).map(Some(_)))
      .getOrElse(Right(None))
      .flatMap(applyConstraints)
  }
  def unbind(value: Option[T]): Map[String, String] =
    value.map(wrapped.unbind).getOrElse(Map.empty)
}
```

Same shape as composable-form — a combinator lifting a *total* inner mapping to `Mapping[Option[T]]`
— but the presence rule is **"any non-empty key under the prefix ⇒ present"** rather than "all empty
or all valid". A partially filled optional address therefore yields `Some` plus the inner mapping's
own per-field required-errors, which is materially better error attribution. `unbind(None)` emits no
keys at all. Absent vs present-and-default is still collapsed (`filterNot(_.isEmpty)` makes an
all-blank `Some` unrepresentable), and metadata survival is moot because `Form` is immutable and
re-derived per request.

### Reflex (Haskell) `maybeDyn` — the opposite metadata choice, precisely specified

[`src/Reflex/Dynamic.hs`](https://github.com/reflex-frp/reflex/blob/develop/src/Reflex/Dynamic.hs):

```haskell
-- | Factor a Dynamic t (Maybe a) into a Dynamic t (Maybe (Dynamic t a)),
-- such that the outer Dynamic is updated only when the "Maybe"'s constructor
-- chages from 'Nothing' to 'Just' or vice-versa. …
maybeDyn :: (Reflex t, MonadFix m, MonadHold t m)
         => Dynamic t (Maybe a) -> m (Dynamic t (Maybe (Dynamic t a)))
```

You never name a path through an absent parent; you **enter a scope that only exists while present**,
and any per-field metadata allocated there lives in that scope. Presence changes and content changes
are separate event streams. Re-entering `Just` mints a **new** inner `Dynamic`, so metadata does
*not* survive a clear/re-create cycle — and that is stated in the doc comment rather than left
emergent. Generalised as `eitherDyn` / `factorDyn` in the same file.

Together with `reactive_stores`, this is the useful pair: **both coherent answers to issue open
question 3 are taken by somebody, and both are written down by the library that takes them.**

### The rest, briefly

- **Haskell `digestive-functors`** — paths are `Text` refs (`(.:) :: Text -> Form v m a -> Form v m a`,
  `type Path = [Text]`), so untyped. Reading a missing path is a runtime `error` crash in
  `queryField`, while `errors`/`childErrors` silently return `[]` and `subView` returns a View whose
  form is a deferred bottom
  ([`Form/Internal.hs`](https://github.com/jaspervdj/digestive-functors/blob/master/digestive-functors/src/Text/Digestive/Form/Internal.hs),
  [`View.hs`](https://github.com/jaspervdj/digestive-functors/blob/master/digestive-functors/src/Text/Digestive/View.hs)).
  **There is no generic `optional` combinator** — the "Optional forms" export section is leaf-level
  only (`optionalText`, `optionalString`, …), and `FormTree` has `Functor`/`Applicative` but **no
  `Monad`**, so you cannot branch a sub-form on a checkbox. The forced idiom makes every inner field
  independently `Form v m (Maybe a)` combined through `Maybe`'s applicative, so a partially filled
  composite silently yields `Nothing` **with no error at all**
  ([`Form.hs`](https://github.com/jaspervdj/digestive-functors/blob/master/digestive-functors/src/Text/Digestive/Form.hs)).
- **PureScript Formless** — flat by construction. State is
  `FieldState input error output = { initialValue, value, result :: Maybe (Either error output) }`
  over a single row mapped once by `MkFieldStates`; depth is not expressible, and the runtime store
  is a flat string-keyed `Object` with types erased
  ([`src/Formless.purs`](https://github.com/thomashoneyman/purescript-halogen-formless/blob/main/src/Formless.purs)).
  Nesting is a known gap, closed without a feature
  ([issue #62](https://github.com/thomashoneyman/purescript-halogen-formless/issues/62); see
  Candidate 5 below for the maintainer's answer). Two ideas worth noting regardless:
  `result :: Maybe (Either error output)`
  distinguishes not-yet-validated from validated, and storing `initialValue` per field makes `dirty`
  derivable rather than a flag to maintain.
- **`etaque/elm-form`** — the cautionary tale, and the only surveyed library with a real runtime path
  tree. Dotted strings, `getAtPath -> Maybe (Tree value)` collapsing "absent" with "path runs through
  a leaf", and **undocumented auto-creation** on write
  (`getAtName name tree |> Maybe.withDefault (Group Dict.empty)` in
  [`src/Form/Tree.elm`](https://github.com/etaque/elm-form/blob/master/src/Form/Tree.elm)).
  Metadata lives in flat string-keyed side tables (`dirtyFields`, `changedFields`, `originalValues`)
  with hand-written prefix cleanup on item removal
  ([`src/Form.elm`](https://github.com/etaque/elm-form/blob/master/src/Form.elm)). **[from source]**
  that cleanup builds the pattern `listName ++ String.fromInt index` (`"todos0"`) while its own
  example generates paths like `"todos.0.label"`
  ([`example/src/View.elm`](https://github.com/etaque/elm-form/blob/master/example/src/View.elm)), so
  the `String.startsWith` guard never matches and the cleanup never fires; `dirtyFields` is not
  filtered even in intent; prefix matching would also catch `todos.10.*`; and indices shift on
  removal so metadata is positional rather than stable. This is precisely the failure mode
  `reactive_stores` avoids with structural paths and composable-form avoids by persisting nothing.
- **`dillonkearns/elm-form`** — a nice *leaf-level* type-level idea: fields parse to `Maybe parsed`
  by default and `required` is a combinator that strips the `Maybe`, guarded by a phantom
  `constraints` row so you cannot apply it twice or after a map
  ([`src/Form/Field.elm`](https://github.com/dillonkearns/elm-form/blob/main/src/Form/Field.elm)).
  Structurally it is composable-form again: string field names, `Dict String (List error)`, flat
  form-encoded state, `Maybe` only in the parsed output.
- **`bevy_reflect`** — a runtime, *untyped* path system that had to answer this question. Signatures
  are `fn reflect_path<'p>(&self, path: impl ReflectPath<'p>) -> Result<&dyn PartialReflect, ReflectPathError<'p>>`
  and `reflect_path_mut` ([docs.rs `GetPath`](https://docs.rs/bevy_reflect/latest/bevy_reflect/trait.GetPath.html)).
  `Option<T>` is reflected as an enum and stepped through with `.0`. **[from source]** on `None` you
  get `AccessErrorKind::IncompatibleEnumVariantTypes { expected: Tuple, actual: Unit }`
  ([`crates/bevy_reflect/src/path/access.rs`](https://github.com/bevyengine/bevy/blob/main/crates/bevy_reflect/src/path/access.rs)),
  i.e. **absence is indistinguishable from a type mismatch** and the rendered message says nothing
  about the parent being `None`. Writes never auto-create. The documented admission is worth quoting
  ([`crates/bevy_reflect/src/path/mod.rs`](https://github.com/bevyengine/bevy/blob/main/crates/bevy_reflect/src/path/mod.rs)):

  > Paths used by this trait do not have any pattern matching capabilities; instead, they **assume
  > the variant is already known ahead of time**. … If the variant cannot be known ahead of time, the
  > path will need to be split up and proper enum pattern matching will need to be handled manually.

- **`keypath` (cmyr)** — `/// A non-fallible keypath. pub struct KeyPath<Root, Value>`
  ([`keypath/src/lib.rs`](https://github.com/cmyr/keypath/blob/master/keypath/src/lib.rs)),
  compile-time validated by proc macro, reads panic on failure. **Its README's first TODO is
  optional chaining** (`People.friends[10]?.age`) and it never shipped
  ([README](https://github.com/cmyr/keypath/blob/master/README.md)).
- **Druid's answer was at the widget layer, not the lens layer.** `druid::widget::Maybe` lifts
  `Widget<T>` + `Widget<()>` into `Widget<Option<T>>`
  ([`druid/src/widget/maybe.rs`](https://github.com/linebender/druid/blob/master/druid/src/widget/maybe.rs))
  — the same "lift a total inner thing with a combinator" pattern as composable-form and Play — at
  the cost of **rebuilding the child subtree** on `Some`↔`None` transitions
  (`ctx.children_changed()`), i.e. no metadata survival.
- **Rust form crates are mostly not prior art.** `leptos_forms` / `leptos-forms` do not exist on
  crates.io. `leptos_form` (abandoned, 0.2.0-rc1) is the instructive *opposite* of
  `reactive_stores`: it erases `Option` from the state graph entirely — `type Signal = T::Signal`,
  `None => T::default_signal(..)`, and `try_from_signal` returns `None` iff the value is the default
  ([`core/src/form_component/mod.rs`](https://github.com/leptos-form/leptos_form/blob/main/core/src/form_component/mod.rs)).
  So `Option<Inner>` allocates the full inner signal tree, the parent becomes `Some` iff any
  descendant leaf is non-default, reads always succeed, metadata trivially survives, absent and
  present-and-default are **fully collapsed**, and there is no presence API at all. Undocumented —
  **[from source]**. `leptos_form_tool` swaps the accessor pair for getter/setter *closures*
  (`Fn(&FD) -> FDT` by value, `Fn(&mut FD, FDT)`), which pushes the whole question onto the caller
  (`d.parent.get_or_insert_with(Default::default).child = v`) at the cost of having no path object
  and cloning on every read
  ([`src/controls/mod.rs`](https://github.com/MitchellMarinoDev/leptos_form_tool/blob/main/src/controls/mod.rs)).
  `yew_form` (abandoned 2020) uses dotted strings over a flat `Vec<FormField>` fixed at construction
  and **`Option<T>` cannot compile** — the only leaf impl is
  `impl<T: ToString + FromStr> FormValue for T`
  ([`yew_form/src/model.rs`](https://github.com/jfbilodeau/yew_form/blob/master/yew_form/src/model.rs)),
  which `Option<T>` does not satisfy, and the derive has no `Option` arm
  ([`yew_form_derive/src/lib.rs`](https://github.com/jfbilodeau/yew_form/blob/master/yew_form_derive/src/lib.rs)).
  `yewdux`'s `use_selector` is a read-only projection with no write-back half
  ([docs.rs](https://docs.rs/yewdux/latest/yewdux/functional/fn.use_selector.html)). Neither
  Dioxus-ecosystem form crate (`dioxus-forms`, `dioxus-form`) has a model-struct path concept at all.
- **iced / egui** — no path concept. iced is Elm-architecture (`text_input(placeholder, value)`);
  egui's "path" is a `&mut` borrow at the call site
  (`TextEdit::singleline(text: &'t mut dyn TextBuffer)`), so absence is answered by
  `if let Some(x) = &mut opt`. egui *does* keep persistent per-widget metadata, but keyed by a
  **hash** `Id` rather than a path
  ([`egui::Id`](https://docs.rs/egui/latest/egui/struct.Id.html),
  [`IdTypeMap`](https://docs.rs/egui/latest/egui/util/id_type_map/struct.IdTypeMap.html)) with no
  per-frame GC — metadata leaks for the session and is silently re-adopted when a widget with the
  same `Id` reappears. Tolerable only because egui declares that data unimportant; a form library
  storing touched/dirty/errors would inherit stale-state resurrection.
- **Rust validation crates confirm the "no addressable slot" problem.** `validator` has **no
  `Validate` impl for `Option<T>`**; only `ValidateArgs` handles it, by short-circuiting
  `if let Some(nested) = self { .. } else { Ok(()) }`
  ([`validator/src/traits.rs`](https://github.com/Keats/validator/blob/master/validator/src/traits.rs)),
  so when the parent is `None` **no key is emitted at all** and an absent parent is indistinguishable
  from a valid one. `garde`'s `Path::join<C: PathComponentKind>` is generic over component kind, not
  typed against the model
  ([docs.rs](https://docs.rs/garde/latest/garde/error/struct.Path.html)).

### What the typed survey establishes

1. **Nobody makes a total path silently tolerate an absent parent.** Every system either errors,
   panics, or changes the accessor's *type* to admit absence. Quietly widening an infallible
   accessor pair is not on anyone's menu.
2. **Where a clean encoding exists, it is a subtyping relation rather than a parallel type.**
   `lens-rs`' `LensRef: PrismRef` and `optics-rs`' `Lens<S,A>: HasGetter<S, A, GetterError = Infallible>`
   ([`src/optics/lens/mod.rs`](https://github.com/axos88/optics-rs/blob/master/src/optics/lens/mod.rs))
   are two spellings of the same idea, with the infallible pair as the `Infallible` specialisation.
3. **Write-through-absent is the genuinely contested decision.** Shipped answers are auto-create
   (`druid-widget-nursery`, `optics-rs`, `etaque/elm-form`) or error/panic (`bevy_reflect`,
   `reactive_stores`). Silent no-op is what the Haskell optics choose but is rare outside them; note
   that line 1 of `optics-rs`' setter module is
   `//TODO: Consider returning a bool here, or adding a SetterError associated type`
   ([`src/base/setter.rs`](https://github.com/axos88/optics-rs/blob/master/src/base/setter.rs)) —
   its authors flagged "a write through a non-matching variant cannot report failure" as a mistake.
4. **The two structural alternatives to a fallible path are both attested.** Either keep the total
   pair and make the `Option` hop a path segment with a safe wrapper in front of it
   (`reactive_stores`), or make the *editing state* total and let `Option` exist only in the parsed
   output (`composable-form`, Play, `dillonkearns/elm-form`, and — degenerately — `leptos_form`).

---

## What the evidence indicates for #25

Framed against the five candidate directions in the issue's second comment. This is what the prior
art shows, not a recommendation.

### On feasibility (the issue's first question)

Feasibility is settled affirmatively, and the strongest evidence is not from optics but from
**`reactive_stores` in Leptos**, which ships dioform's exact accessor pair
(`read: fn(&Prev) -> &T, write: fn(&mut Prev) -> &mut T`) and *still* addresses through `Option`
without changing that pair — by making the `Option` hop an ordinary path segment whose accessors
`unwrap`, and putting a `map()` combinator in front that does the presence check. That is an
existence proof that candidate 3's premise ("the infallible accessor pair is the real constraint") is
avoidable, not just satisfiable.

On the optics side the Rust picture is thinner but also affirmative. `lens-rs` (2021) ships
`fn preview_mut(&mut self, optics: Optics) -> Option<&mut Image>` with no `Clone` bound, generated by
a derive macro, and composes lens-through-prism-through-lens; `enso-optics` (2021) ships
`fn get_mut(&mut self) -> Option<&mut Field<Self, T>>` composed by `and_then` and walks two nested
`Option`s in its own test. Both are unmaintained and neither is depend-able (see
[Rust optics crates](#rust-optics-crates)), but they are existence proofs that the borrow checker
does not obstruct the shape. What is genuinely unattested is combining that shape with a
*first-class runtime path value* — which is what `FieldPath<Model, Value>` is.

### Candidate 1 — Optional group scope, materialise-on-write

**Prior art is real but thin, and every instance of it is either undocumented or explicitly paid for.**

- The JS form libraries all do it (TanStack `setBy`, RHF `set`, Formik `setIn`, Final Form `setIn`),
  but in three of four it is **emergent from a lodash-`set`-shaped helper and undocumented**
  (TanStack, RHF, Formik). Only Final Form documents it as a deliberate rule set. So "everyone does
  it" is mostly "everyone inherited it from lodash".
- Two Rust crates do it silently: `optics`' composed `set` reads the outer optic, mutates, and
  writes back, so composing a lens with an `Option` prism and writing while `None` **materialises the
  `Some`**; `druid-widget-nursery`'s `Prism::put` is `*data = Some(inner)`. Neither documents this as
  a decision, and `optics`' own README calls itself unfinished. This is the same emergent-not-chosen
  pattern as lodash `set`, transplanted into Rust.
- In optics terms it corresponds to **`at k . non default`**, and the ecosystem's version of it has
  two properties dioform's proposal does not currently claim: it demands an explicit default at the
  call site, and it is **symmetric** — writing the default deletes the parent. Asymmetric
  materialisation (create on write, never remove) is not something the optics ecosystem offers.
- Final Form is the cautionary data point in the opposite direction: it prunes so eagerly that a
  user backspacing a text field deletes the optional parent object. Once the library owns presence
  implicitly, it owns it in both directions, and both directions surprise someone.
- The one typed Rust form crate that materialises, `leptos_form`, does it by going all the way:
  `None` is *represented* as the all-defaults inner tree, so writing an inner field materialises the
  parent because the parent was never absent in the state graph to begin with. That buys total reads
  and free metadata survival at the price of collapsing absent and present-and-default entirely, and
  of having no presence API at all. **[from source]**, undocumented.
- It contradicts the `CONTEXT.md` **Optional Field** definition, as the triage already recorded.

### Candidate 2 — Optional group scope, explicit presence only

**This is what the optics ecosystem actually converged on, and it is the only candidate with a
documented law behind it.** `at` (presence, a `Lens'` onto `Maybe v`) and `ix` (inner traversal, an
`AffineTraversal'`) are two separate optics related by `ix k ≡ at k % _Just`, with the docs stating
outright: *"Setting the value of this AffineTraversal will only set the value in `at` if it is
already present. If you want to be able to insert missing values, you want `at`."*

Note that dioform **already has the `at` half** — `FieldPath<Model, Option<Party>>` bindable as a
whole value. What is missing is the `ix` half. That reframes candidate 2 as "add the missing
optic", not "add a scope construct".

The cost the issue identifies (a UI round trip to commit presence first) is real and the optics
ecosystem accepts it; `non` exists precisely so callers who don't want it can opt out per call site.

`reactive_stores` is the working Rust instance of this split: the panicking `.unwrap()` path exists,
but the *documented* API is `map()`, which "returns `None` if the subfield is currently `None`" and
re-runs when the field toggles between `None` and `Some(_)`. Presence is committed by writing the
`Option<T>` field itself, which is an ordinary path.

Rust corroborates the shape of the split. `lens-rs` puts construction in a separate `Review` trait
rather than in the setter; `smart_access` puts it in a separate *index type* (`Ensure { key, value }`)
rather than in the optic. Both are variations on "presence is committed by a different operation
than field editing" — the third being Haskell's "a different optic". None of them is a policy flag on
a single traversal.

### Candidate 3 — Fallible traversal in the path core

**This is the affine traversal, and it is what both typed JS form libraries converged on at the type
level.** TanStack's `DeepValue<T, 'a.b'>` = `B | undefined` when `a` is optional, and RHF's
`PathValue` does the same — independently, with type tests in both repos. Neither introduced an
optional-group construct; both made the *leaf value type* absorb the parent's absence.

The optics framing says the same thing more precisely: a path whose read yields `Option<&Value>` and
whose write is a no-op when absent **is** an affine traversal, and it is exactly what
`Lens % Prism` composes to. It also generalises to `Variant Field` / enums for free, since a prism
is the general "one constructor of a sum type" optic (`_Left`, `_Right`, `_Just` are all the same
shape) — which is the broader gap the issue names.

The Rust survey adds a concrete shape: the three Rust designs that actually model this
(`lens-rs::preview_mut`, `enso-optics::OptResolver::resolve_mut`, `serde_json::pointer_mut`)
independently chose `Option<&mut T>` composed by `and_then`, and a total lens lifts into that shape
trivially. Nothing on crates.io stores that as a first-class runtime fn-pointer pair the way
`FieldPath` stores its infallible pair — so this direction is unattested in Rust rather than
disproven.

The counter-evidence is cost, not correctness: it changes `FieldPath`'s accessor pair and everything
built on it. Note also that the optics libraries keep `Lens` and `AffineTraversal` as **distinct
kinds with a subtyping relation**, not one type with a fallible getter — a total lens is still a
lens, and only compositions that pass through a prism get downgraded. A design that made *every*
dioform path fallible would be strictly weaker than what optics does; `lens-rs` shows the subtyping
version working in Rust (`LensRef: PrismRef`, `LensMut: PrismMut`).

A sixth direction appears in the prior art that the issue's list does not name, and it is the one
several typed libraries actually chose: **keep the model's `Option` but make the *form state* total**
— `Option<T>` in the domain model becomes `(present: bool, T)` in the editing state, with presence
either stored explicitly or derived from emptiness. `composable-form` and Play both do the derived
version; `leptos_form` does the degenerate version (`None ≡ all-defaults`). It preserves
`FieldPath`'s infallible pair completely and moves the whole question into the model↔state mapping.
Its cost is exactly the criterion the issue lists last: it **collapses absent and present-and-default**
in every instance found, and Play's variant additionally requires an emptiness predicate per type.
Note that `composable-form`'s `andThen` example shows the same technique applied to general sum
types by storing a **flattened superset** of both branches — which is the `Variant Field` question in
`CONTEXT.md` answered by the same mechanism.

### Candidate 4 — Presence as a first-class field

**No direct prior art in the surveyed libraries** — none of them model presence as a field with its
own identity, validation state and dirty tracking. The closest analogues:

- optics' `at k` is a *lens onto `Maybe v`*, which is presence-as-an-addressable-value but carries no
  metadata (optics carry no metadata at all).
- RHF's `unregister` options (`keepValue` / `keepError` / `keepDirty` / `keepTouched` /
  `keepIsValid` / `keepIsValidating`) are the only place in the survey where "what happens to
  metadata when presence changes" is treated as a **first-class, per-flag, caller-controlled
  decision**. That is evidence the question is real and that a single global policy tends not to
  satisfy everyone.

- Reflex's `maybeDyn` is presence-as-a-scope rather than presence-as-a-field: the inner `Dynamic`
  exists only while `Just`, so per-field metadata allocated inside it is scoped to the presence
  episode and a new episode mints new identity.

So candidate 4 is not contradicted by prior art; it is simply unattested. The precedent it would be
following is dioform's own `Collection Item Identity`, not another library's.

### Candidate 5 — Status quo, documented

**The prior art is mixed, and weaker against the baseline than it first looks.**

Every *JavaScript* library surveyed lets you name a path through an optional parent, and so does
`reactive_stores`. But several typed libraries genuinely do not, and say so:

- PureScript Formless's field row is flat. Better ergonomics for nested / array-of-field forms was
  requested in [issue #62](https://github.com/thomashoneyman/purescript-halogen-formless/issues/62)
  and **closed without a library feature**; the maintainer's answer was to coordinate it yourself —
  "render the browser fields so each one tracks its index in the array and updates its value there
  appropriately" — with the repo's `example/nested-array` (a child component owning its own form
  state) as the pattern. That is the DTO-plus-side-channel shape, endorsed.
- `digestive-functors` has no generic `optional` combinator, only leaf-level ones, and its form tree
  has no `Monad` instance so you cannot branch a sub-form on presence at all.
- `bevy_reflect` documents that its paths "assume the variant is already known ahead of time" and
  tells you to split the path and pattern-match manually.
- `yew_form` structurally cannot compile an `Option<T>` field.

So "you cannot address it" is not an outlier position among typed libraries — it is common, and
usually accompanied by an explicit statement that the user should handle presence outside the form
abstraction. What *is* an outlier is doing that while also having a composable typed path type;
`reactive_stores` shows the combination is achievable.

### On field identity and metadata (issue open questions 3 and 4)

Every form library surveyed keys field metadata by the **path string in a flat map**, entirely
decoupled from whether the value exists:

- TanStack: `fieldMetaBase: Partial<Record<DeepKeys<TFormData>, …>>`
- Formik: three parallel trees (`values`/`errors`/`touched`) plus a flat `fieldRegistry`
- Final Form: flat `state.fields[name]`
- RHF: `_fields` / `_formState` keyed by path

Consequence: **identity is trivially stable across a clear/re-create cycle, and metadata survives by
default** — sometimes to users' surprise (Final Form keeps `touched: true` and the stale `error`
after the parent is pruned). Where libraries *do* discard metadata, it is because an explicit
lifecycle call did it (`unregister`, `deleteField`), not because the value went absent.

Outside JS, **both answers are taken and both are written down by the library that takes them**:
`reactive_stores` keys reactive triggers by a *structural* `Vec<StorePathSegment>`, so an inner
subscriber's trigger is found at the same path after a `Some → None → Some` round trip regardless of
ancestor presence (their `patch` test corroborates this but does not prove it — see
[the section above](#reactive_stores-leptos--literally-dioforms-struct-and-it-addresses-through-option));
Reflex's `maybeDyn` documents the opposite — re-entering `Just` mints a **new** inner
`Dynamic`, so metadata does not survive. The cautionary case is `etaque/elm-form`, which left it
emergent: metadata lives in flat string-keyed side tables with hand-written prefix cleanup, and
**[from source]** the cleanup builds `"todos0"` while paths are `"todos.0.label"`, so it has never
fired.

Also worth noting for open question 4: **the same library can apply different presence policies to
different metadata**. Final Form prunes errors with values but keeps `touched`/`visited`; TanStack's
`setFieldValue(parent, undefined)` keeps all descendant meta while `deleteField(parent)` deletes it.
Neither documents the asymmetry.

### On absent vs present-and-default (issue open question 2)

**Every JS library collapses them, and the typed ones collapse them in the type system.** TanStack
and RHF both union the parent's absence into the child's value type; Formik and Final Form collapse
on read via `undefined`; Final Form additionally makes present-and-empty unrepresentable by pruning.

**The optics ecosystem and `reactive_stores` are the surveyed prior art that keeps them apart** —
`at k` focuses `Maybe v` and `reactive_stores`' `Option<T>` field is a real path, so absence is a
value you can pattern-match — and optics offers the collapse as an explicit, named, default-carrying
opt-in (`non`). The typed form libraries that make the editing state total (`composable-form`, Play,
`leptos_form`) all collapse the two, and in `composable-form` and Play that collapse is *the
mechanism* by which presence is derived, not an incidental cost. If dioform wants to preserve the
distinction (one of the issue's stated criteria), that rules against the total-editing-state family
regardless of how ergonomic it is.

One further data point on why the distinction matters downstream: `validator` has no `Validate` impl
for `Option<T>` and its `ValidateArgs` path short-circuits on `None`, emitting **no error key at
all** — so an absent parent and a valid parent are indistinguishable in the error map, and there is
no addressable slot for a field inside an absent parent
([`validator/src/traits.rs`](https://github.com/Keats/validator/blob/master/validator/src/traits.rs)).

---

## Where the evidence is missing

Recorded plainly rather than filled with inference:

- **TanStack Form's prose documentation never discusses optional or nullable parents.** The type
  behaviour is pinned by type tests; the *runtime* behaviour (auto-creation in `setBy`, meta
  retention on `setFieldValue(parent, undefined)` vs deletion on `deleteField(parent)`) is
  undocumented and was read from source.
- **React Hook Form never documents that `setValue` creates missing intermediate objects.** The
  docs discuss registering and unregistered inputs, not parent materialisation. The `unregister`
  docs table showing `unregister("yourDetails")` → `{}` is ambiguous about what the "Value" column
  refers to and is not obviously self-consistent across its two rows.
- **Formik documents nothing about absent parents** — no prose on `getIn`/`setIn` semantics, no
  mention of auto-creation or the array-vs-object heuristic.
- **No surveyed form library documents what happens to inner-field metadata when an optional parent
  is cleared and re-created.** Every claim in this file about that cycle is either read from source
  or verified by executing the package, and is labelled as such.
- **No surveyed form library models presence as a metadata-carrying entity** (candidate 4), so
  there is no prior art for or against it.
- **The Rust crates that auto-construct on write (`optics`, `druid-widget-nursery`) do not document
  that they do.** Both claims are read from source.
- **`serde_json::pointer_mut`'s no-auto-create behaviour is not stated in its prose docs**, only
  observable in the implementation.
- **`enso-optics` has no documentation at all** (docs.rs renders empty because every item is
  private); everything reported about it was read from the published tarball.
- **`reactive_stores` does not document that `.unwrap()` panics on `None`, nor that writes never
  auto-create.** Its doc comment mentions neither; both were read from source. Its *metadata*
  behaviour is only established by its own test, not by prose.
- **`etaque/elm-form` documents its `setIn` equivalent as "Set node in tree at given path"** and
  nothing more; the auto-creation and the broken prefix cleanup were both read from source.
- **`leptos_form`'s `None ≡ all-defaults` collapse is undocumented**, read from
  `core/src/form_component/mod.rs`.
- **`bevy_reflect` documents the enum-path syntax but not what a `None` parent yields**; the
  `IncompatibleEnumVariantTypes` outcome came from `path/access.rs` and their own test.
