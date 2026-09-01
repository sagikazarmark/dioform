# Bind optional text to the Field Convention as rendered text

An `OptionalTextBinding` provides the **Field Convention** a `Binding<String>` over the text a
control renders, not a `Binding<Option<String>>` over the `Option` it stores, and its
`FieldContext` carries that rendered-text binding. This mirrors
[ADR-0052](0052-bind-parsed-fields-to-the-field-convention-as-rendered-text.md): the convention
binding is over what the control shows, and under
[ADR-0046](0046-bind-optional-text-as-controlled-scalar-presence.md) the rendered-text form *is*
this binding's own language — it renders `""` for `None`, and only exact empty input means absent,
so the `Option` expresses nothing on this surface that a `String` cannot. The previous conversion
exposed the binding's internal representation instead, so a **Widget Registry** text control
resolving `Binding<String>` hit a `BindingTypeMismatch` panic at runtime on the simplest optional
field shape, while `Option<f64>` and `Option<chrono::NaiveDate>` worked in one line through
ADR-0052.

The read is the binding's rendered text: `""` for `None`, the string itself for `Some`. A write
applies the ADR-0046 presence rule — exactly empty writes `None`, anything else writes `Some`, no
trim — with the **Change Origin** mapped to the same user or programmatic typed write the direct
`on_input` and `set_value` paths use. The presence rule travels with the rendered-text write
regardless of origin, which also closes an existing breach: the macro-generated convention write
passed the raw `Option` through, so a Programmatic convention write of `Some(String::new())`
could land in the **Form Draft** even though ADR-0046 makes `Some("")` deliberately unreachable
through the binding. One asymmetry remains and is blessed by ADR-0046: a Programmatic convention
write of `""` produces `None`, while the typed `set_value(Some(String::new()))` still produces
`Some("")` — the two render identically but differ in dirty state.

The rendered text is derived rather than stored, so a `Binding<String>` read needs storage to hand
out a reference into. Unlike ADR-0052's mount-scoped storage, this storage is **form-owned and
shared across all mounts of a field**, keyed and looked up like every other field signal slot,
including [ADR-0047](0047-make-field-paths-interchangeable.md) path interchangeability. The
divergence is deliberate and sound: a parsed binding's rendered text depends on per-mount **Raw
Input State**, while an optional-text binding's rendered text is a pure function of the **Field**
value, so every mount showing one field shows one text. Form-owned allocation is also what keeps
the non-hook, per-render `FormHandle::optional_text` constructor from accumulating storage. The
sharing profile — form-owned per-field slots read through the form's selector tracking — is
exactly the shipped `FieldHandle` read path. Binding identity is the field handle alone: with no
parser or formatter to vary, two bindings over one field are always interchangeable.

The typed `From<OptionalTextBinding> for Binding<Option<String>>` stays. Presence-typed
conversions are a live pattern where the `Option` expresses a real state the control renders — a
`TriStateCheckboxBinding`'s `Option<bool>` is a third checkbox state, and a select over
`Binding<Option<T>>` renders absence as its own option — so those `FieldContext` conversions also
stay presence-typed. Optional text is different precisely because ADR-0046 collapses the
distinction: the control has one absence sentinel, the empty string. Only the `FieldContext`
conversion switches; `FieldContext` holds exactly one erased binding, so the conversion is
necessarily either/or, and it must pick what text controls resolve. A multi-binding `FieldContext`
in dioxus-field would dissolve the either/or, but it is not on that crate's roadmap and this
decision does not wait for it.

The alternative — leaving the context presence-typed and having dioxus-field text controls
fallback-chain `String` then `Option<String>`, as `try_resolve`'s own documentation blesses for
multi-type controls, or adding a `try_resolve_text` shim there — was rejected on write-side
ownership. Resolving the read is easy; the write is where the ADR-0046 presence rule lives
(exact-empty only, no trim, the `Some("")` collapse), and pushing it into every text control and
**Widget Registry** would ask a deliberately form-library-agnostic crate
([ADR-0048](0048-incubate-dioxus-field-in-the-workspace-with-an-extraction-trigger.md)) to
reimplement Dioform presence semantics correctly, everywhere, forever.

This ships as a behavior-breaking change in one release rather than through a deprecation step: a
consumer resolving `Option<String>` from an optional-text context gets the inverse mismatch at
runtime, and a two-step release would not soften that — adding the second `From` impl is itself
breaking, because `let b: Binding<_> = binding.into()` sites lose inference the moment both impls
exist.

**Collection Field** item bindings remain outside the **Field Convention**; the collection family
is a separate decision, exactly as ADR-0052 records.
