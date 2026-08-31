# Bind parsed Fields to the Field Convention as rendered text

A parsed **Field Binding** produces a `Binding<String>` over the text a control renders, not a
`Binding<Value>` over the typed **Field** it writes. The control edits text; the **Form Draft** holds
the parsed value; the binding is the seam between them. Every parsed helper — `use_parsed_text`,
`use_number`, `use_date`, and their optional and `_with` variants — therefore reaches a **Widget
Registry** through the same text-shaped **Value Binding** an ordinary text input uses, and needs no
registry support beyond the one it already has.

The binding reads what `ParsedTextBinding::value` already returns: **Raw Input State** while a
**Parse Blocker** stands, and the formatted **Field** value otherwise. That text is derived from two
reactive sources rather than stored, so each read recomputes it into storage owned by the mount and
hands out a reference to that. The storage is created on the first such read, inside the render that
reads it, so a mount that never produces the convention never allocates one.

A write parses before it reaches the **Field**, so the **Change Origin** travels with the text: a
user write parses and writes as the user, or marks the **Field** touched and raises a **Parse
Blocker**; a programmatic write parses and writes programmatically, and neither marks the **Field**
touched nor reports interaction, exactly as an already-typed programmatic write does. Commit and
**Focus Exit** map as they do for every other Dioform-produced binding, including the existing rule
that a Commit holding an unresolved **Parse Error** ends the interaction unit without validating.

An unresolved **Parse Error** leads the **Field Meta** errors and marks the **Field** invalid. This
does not make it a **Validation Error**: the core keeps the two apart, and the projection is one-way
into pre-rendered presentation text that the convention models as `Rc<str>` rather than as the form's
error type. It leads the list because it describes the text the reader is looking at, and it does not
wait for a Commit the way [ADR-0051](0051-reveal-field-errors-after-commit-without-marking-fields-blurred.md)
makes a **Validation Error** wait: a blocker exists only while the rendered text cannot become a
value at all, and it clears on the keystroke that makes it parse, so there is no judgment about a
value to defer.

Leaving the **Parse Blocker** out of **Field Meta** was declined. A registry control renders errors
from metadata alone, so the blocker would be invisible in exactly the composition this binding
exists to reach, and an application would be back to rendering `parse_error()` beside the widget by
hand. Mapping it into the form's **Validation Error** type was also declined: it would demand a
conversion from every application error type, and it would put a binding-level fact into
validation-error selectors, summaries, and adapters that ADR-boundary work has kept clear of it.

Binding identity combines the **Field Path** with the mount's own identity, because two parsed
bindings on one **Field Path** with different parsers or formatters are not interchangeable, while
two produced from one mount always are.

**Collection Field** item bindings, including `CollectionParsedTextBinding`, remain outside the
**Field Convention**. Nothing here changes that; the collection family is a separate decision.
