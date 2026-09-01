# Bind select bindings to the Field Convention as optional selection

A `SelectBinding` and a `RenderedSelectBinding` provide the **Field Convention** a
`Binding<Option<Value>>` over the selection a widget shows, not a `Binding<Value>` over the value
the **Field** stores, and their `FieldContext` carries that wrapped binding. **Field Convention**
selection widgets resolve `Binding<Option<T>>` because a selection widget can be on nothing, while
a required-valued select **Field** always has a value; the previous conversion exposed the field's
value type instead, so a **Widget Registry** select resolving `Option<Value>` hit a
`BindingTypeMismatch` at runtime on every non-`Option` select field. This closes for scalar
selects what [ADR-0053](0053-bind-optional-text-to-the-field-convention-as-rendered-text.md)
closed for optional text — but unlike ADR-0053 it is not a pure correction: the two sides
genuinely disagree about whether "unselected" is representable, and the conversion takes a
position on it.

The read is total: the field always has a value, so the convention read is `Some` of it. A write
of `Some(next)` is the ordinary selection write with its **Change Origin** preserved. A write of
`None` is refused: the **Form Draft** stays untouched and the widget's subsequent **Commit**
proceeds normally, which violates no recorded contract — committing an unchanged value is already
dioform-normal (`SelectBinding::onblur` commits unconditionally, and
[ADR-0051](0051-reveal-field-errors-after-commit-without-marking-fields-blurred.md)'s sticky
committed metadata records only that an interaction unit ended, idempotent under repeated
commits). The residual hazard is silent drift: at the pinned upstream revision no shipped
selection widget emits `None` from user interaction, so any future hit means upstream behavior
changed. The refusal is therefore loud — `tracing::error!`, the level dioxus-field's own
resolution diagnostics ship at — and not a `debug_assert!`: the branch fires mid-interaction on a
user event, where debug/release behavioral divergence for a policy branch is wrong. This adds
dioform's only `tracing` dependency, gated with the `dioxus-field` feature.

Two alternatives for the `None` write were rejected. Suppressing the subsequent commit's
side-effects is mechanically incoherent: write and commit are separate widget callbacks
(write-then-commit, unconditional), so suppression would need hidden cross-callback state and
would falsify what committed metadata means under ADR-0051. Leaving the behavior
producer-defined or documented-unspecified abdicates the decision the conversion exists to make.

The policy travels with the *conversion*, not the widget: any `Option`-resolving widget that
resolves the produced `Binding<Option<Value>>` — a combobox as much as a select — hits the same
refusal, and there is no mechanical seam for a per-widget exemption. A future genuinely-clearable
affordance therefore needs a different *producer*, not a widget-scoped policy override: the
optional-select path below over an `Option`-valued **Field**. As a companion, not a substitute,
the wrapped conversion reports `required: true` in its **Field Meta** — overriding the hardcoded
`required: false` of the plain field conversion, per the `ParsedTextBinding` meta-override
precedent — truthful clearability metadata a clearable widget can honor by suppressing its clear
affordance. No shipped widget acts on it yet, so refusal remains the primary policy. One caveat
travels with it: a widget may forward its widget-level `None` to the application's own `on_change`
handler even though the model refuses the write, so applications cannot read `on_change(None)` as
a model clear.

**Fields that are themselves `Option<Value>` ship a direct mapping in the same release.** The
blanket wrapped conversion would double-wrap them to `Binding<Option<Option<Value>>>` and break
the one select shape that previously reached convention widgets with zero adaptation, and the
generic impl cannot exclude `Value = Option<_>` (no negative bounds in Rust). So
`FormHandle::optional_select` / `use_optional_select` produce an `OptionalSelectBinding`, with
`optional_select_with` / `use_optional_select_with` as the rendered twin, both converting
`=> Option<Value>` directly: the widget's `None` *is* the field's `None`, a `None` write is a real
clear, and `required` stays false. The rendered twin reserves the empty rendered value `""` for
the unselected state, so its parser and formatter only ever see the inner `Value`.

Serving a `Binding<Option<Value>>` read over a `Value`-typed field needs form-owned storage to
hand a reference into, exactly as ADR-0053's rendered text did. That storage generalizes from
ADR-0053's monomorphic rendered-text slot to a per-`Value` derived slot, keyed and shared like
every other field signal slot, including
[ADR-0047](0047-make-field-paths-interchangeable.md) path interchangeability. Slot lookup
downcasts by the `(source, derived)` type pair and compares paths, so the derivation must remain a
pure function fully determined by that pair — which it is: `Some(value)` here, `""`-for-`None`
there.

The typed `From<…> for Binding<Value>` conversions stay, per the ADR-0053 shape: only the
`FieldContext` conversion switches, because `FieldContext` carries exactly one erased binding and
must carry what selection widgets resolve. The typed binding is the policy-free direct-wiring
interface — a consumer taking a required select as `Binding<Value>` has a total read and no `None`
branch. Honestly recorded: it currently has no in-tree consumer; the retention is symmetry with
`OptionalTextBinding` and optionality, not a live dependency. Dropping it now only to re-add it
later would be a second inference-breaking release, and the inference break is sunk regardless:
ADR-0053 records that merely adding the second `From` impl breaks `.into()` inference at
annotation-free call sites, so this ships as a one-step breaking 0.x release.

`RadioGroupBinding` stays `=> Value`: the radio widget shape resolves plain `Binding<String>`, so
the `Option` gap does not exist there — though radio is "already correct" only for
`String`-valued fields, and typed radio has its own separate convention gap, unfiled.
`MultiSelectBinding` and the **Collection Field** select bindings remain outside the **Field
Convention**; this decision closes the last scalar-select convention-boundary gap, not the last
gap overall.

The alternative home — resolving `Option<T>` with a fallback to `T` inside dioxus-field's
`try_resolve`, fixing every producer at once — was rejected because it pushes the `None`-write
question into every widget of a deliberately form-library-agnostic crate
([ADR-0048](0048-incubate-dioxus-field-in-the-workspace-with-an-extraction-trigger.md)), while the
producer-side conversion gets identity and metadata free through the existing conversion macro and
decides the policy once. What that producer-side erasure costs — the type-level clearability
information — is exactly what the `required: true` metadata above preserves.
