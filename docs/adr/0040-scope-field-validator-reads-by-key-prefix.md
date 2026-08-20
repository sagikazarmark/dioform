# Scope field-validator reads by key prefix

A field-scoped **Validation Error** read traverses only validator states whose key names that
**Field**. The field-validator table and collection-item validator-state table keep their existing
`(field, id)` key order. A read starts at `(field, ValidatorId(0))` and stops when the key's field
changes. Whole-form reads keep their existing global sort, and form validators keep a full scan
because one form validator may attach errors to any field
([issue #62](https://github.com/sagikazarmark/dioform/issues/62)).

## The existing key order is the field index

Every validator key is unique and all keys for one field are contiguous in a `BTreeMap` ordered by
`(field, id)`. A lower-bounded range therefore visits the field's entries in ascending validator ID
without inspecting validators registered on other fields. The same property holds for instantiated
collection-item validator states, which use the same key type.

This changes a field read from traversing and sorting every registered field validator to a range
lookup plus traversal of that field's own validators. The remaining work is proportional to errors
from form validators and submission because those stores are not keyed by target field.

## Restricting the old order is exactly the range order

Whole-form field-validator output is ordered by `(id, field, source)`. Restricting that total order
to one field leaves ascending `id`. The source tie-breaker is unreachable because a validator ID is
allocated once and each `(field, id)` key is unique. The `(field, id)` map range also yields
ascending `id` after fixing `field`, so no observable ordering changes.

Field-scoped output retains its category order: direct field-validator errors, collection-item
validator errors, form-validator errors, then submit errors. Each source retains its own error
order. Whole-form status and error reads continue to sort by `(id, field)` across fields.

## Visibility and boolean reads do only the work they assert

Every error in a field-scoped read has the same target. `should_show_validation_errors` is therefore
evaluated once for that target before traversing validators, rather than once per stored entry. This
avoids repeated metadata subtree scans under blur- and touch-scoped **Error Visibility**.

Accessibility needs only to know whether a visible error exists. Its core query uses `any` over the
same scoped ranges and then over form and submit errors, stopping at the first match rather than
materializing a `Vec<ValidationErrorView>`. The **Dioxus Adapter** keeps the same visible-validation
and parse-error selector registrations around that query.

## Re-keying by validator ID is declined

Ordering the maps as `(id, field)` would make whole-form iteration naturally match its output order,
but it would interleave fields and remove the prefix needed by the more frequent field-scoped read.
The triage measurements at 1,030 validators made the trade-off explicit:

| implementation | field read | scaling in total validators |
| --- | ---: | --- |
| previous clone and sort | 25.0 us | linear |
| `(id, field)` key and no sort | 4.1 us | linear |
| `(field, id)` scoped range | 0.215 us | flat |

The public-read benchmark can be reproduced with:

```console
cargo run --release -p dioform-core --example field_error_reads
```

It measures 10, 100, and 1,030 registered validators while reading one field with one error. The
benchmark is intentionally a runnable example rather than a timing assertion in the test suite;
wall-clock thresholds would make correctness tests machine-dependent.

A release run in the implementation environment on 2026-08-20 produced:

| registered validators | nanoseconds per visible field read |
| ---: | ---: |
| 10 | 70.7 |
| 100 | 79.5 |
| 1,030 | 77.0 |

The absolute numbers are machine-specific; the relevant result is that adding unrelated validators
does not produce the previous linear growth.

## What this does not fix

Whole-form reads still collect and sort field-validator and collection-item entries. Form-validator
errors and submit errors still require target filtering because their stores are not keyed by target
field. Validator selection on writes still traverses the full tables because its **Validator
Selection Reach** includes related fields rather than one exact identity. No error views are cached
or memoized. Those costs have different semantics and require separate decisions.
