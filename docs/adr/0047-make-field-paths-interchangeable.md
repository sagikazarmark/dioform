# Make field paths interchangeable

Dioform will implement `PartialEq` for `FieldPath` as the core capability for **Field Path
Interchangeability**. Directly derived paths compare structurally, so two independent derivations of
the same field compare equal:

```rust
Model::fields().street() == Model::fields().street()
```

Composed paths compare by clone-of accessor identity. Equality never lies: equal paths are genuinely
interchangeable, while false inequality costs only a missed memoization. This decision supersedes
[ADR-0030](0030-decline-partial-eq-for-field-paths-and-bindings.md). The Dioxus integration initiative
is the reported case that satisfies that ADR's revisit clause.

## Direct paths retain structural accessors

`FieldPathAccessor` remains private. It distinguishes direct accessors, which retain their
`get: fn` and `get_mut: fn` pointers, from composed accessors, which retain the existing
`Rc<dyn Fn>` closures. Core's public `PartialEq` implementation is the named interchangeability
capability; no accessor representation or additional public trait crosses the **Form Core** boundary.

Two direct paths compare their **Field Identity**, rendered **Field Name**, getter pointer, and mutable
getter pointer. The derive macro emits the same non-capturing functions for independent derivations of
one static field, making the ordinary derived-path comparison structural and true. Identity or name
alone remains insufficient because `FieldPath::direct` is public and optional traversal deliberately
permits distinct accessors to share both.

Function-pointer comparison is conservative under code generation. Linker identical-code folding can
merge only accessors whose callable behavior is interchangeable; the identity and name checks still
distinguish different declared fields. Duplication across codegen units can instead give one accessor
more than one address, but that produces only false inequality. It cannot make unequal behavior compare
equal or leave a memoized child holding the wrong accessor. The implementation may therefore carry a
reasoned allowance for `unpredictable_function_pointer_comparisons`: the platform variability is a
performance-only loss of memoization, not a correctness risk.

## Composed paths retain clone-of equality

`join`, `.or`, and mounted **Field Group Maps** compose paths through capturing closures. Their accessor
behavior has no stable structural representation, so their closures compare with `Rc::ptr_eq`. A path
and its clone compare equal; two independently composed paths may compare unequal even when they
address the same value. In particular, `.or(&a)` and `.or(&b)` remain unequal when their accessors
differ despite sharing identity and name.

This asymmetry is part of the public contract. Equality means interchangeability, never merely matching
metadata. False inequality for an independently rebuilt composed path is safe and asks callers to hoist
or clone such a path when memoization matters.

Derived **Field Group Maps** will compare through their paths. Maps containing only direct derived paths
gain structural equality, while composed or mounted maps inherit the clone-only limitation of the paths
they contain. Scalar **Field Bindings** will compare as **Form Handle** equality conjoined with path
equality.

## Collection bindings remain excluded

`CollectionBinding` and `CollectionItemBinding` do not gain equality. Collection rows continue to take
a **Form Handle** and **Collection Item Identity** as props. Prop equality cannot replace the reactive
collection-structure subscription that an `items()` read creates.

The hazard identified by ADR-0030 remains: after removing the last row, surviving rows can retain both
identity and index while collection-derived sibling counts and move controls need to change. A memoized
row handed only an equal collection binding could keep an enabled move control whose destination is now
out of range. Structural path equality does not wake that row, so collection-row subscription is
deliberately out of scope rather than approximated with binding equality.
