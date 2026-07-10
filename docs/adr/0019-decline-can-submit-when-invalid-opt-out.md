# Decline a can-submit-when-invalid opt-out

Dioform will not add an opt-out that lets a **Dioxus-Managed Submission** proceed despite
validation-error blockers: the `canSubmitWhenInvalid` analog evaluated in
[issue #151](https://github.com/sagikazarmark/dioform/issues/151). **Submit Availability**
stays an unconditional "no known blockers" signal, and managed submission keeps running required
validation before application submit behavior. The motivating flows are already expressible through the
typed **Submit Intent** model without weakening that invariant.

## Why the invariant is worth keeping

Submit Availability's value is that it is unconditional: for a given **Submit Intent**, available means
there are no known blockers: validation errors, parse blockers, required pending validation, or an
in-flight submission (`CONTEXT.md`). Button enablement, progressive submit preflight
([ADR-0007](0007-use-browser-submit-preflight-without-async-waiting.md)), and the blocker enumeration
all lean on that reading. A per-intent opt-out that suppresses the validation-error blocker turns a
clear invariant into a conditional one, and every consumer of availability then has to ask "available,
or available-except-for-the-bypass?" TanStack needs `canSubmitWhenInvalid` because its `canSubmit` is a
single global boolean with no intent dimension; Dioform's per-intent availability already provides
the granularity that flag approximates, so importing it would add a second mechanism overlapping one the
design already has.

## The motivating flows are already expressible

**Save-draft / save-partial.** Submit-triggered validation is intent-scoped: a validator reads the
**Submit Intent** from its context and can be a no-op for `SaveDraft` while required for `Publish` (the
`publish-title-required` pattern in `docs/submit-intent.md`). A value that is invalid *for Publish* then
carries no blocker *for SaveDraft*, so `form.intent(SaveDraft).availability()` is correctly available
and the managed submit proceeds: no bypass, invariant intact.

This works because the design separates universal invariants from purpose-specific requirements.
Non-submit **Validation Errors** (change/blur/field rules) are conservative known blockers across all
intents (`CONTEXT.md`); submit-triggered rules are intent-scoped. So a requirement you want a draft save
to skip belongs at submit scope keyed on intent, not as a universal field rule. Modeling it at the right
scope is what keeps "non-submit errors block every intent" meaningful: those are the things that are
invalid regardless of purpose (and unparsable *input* is a **Parse Error**/**Parse Blocker**, which the
opt-out leaves non-bypassable anyway). The opt-out's appeal is that it lets a universal blocker be
declared and then bypassed; modeling the rule at submit+intent scope reaches the same behavior honestly.

**Server-authoritative.** What the client refuses to submit belongs in client validators; what the
server decides belongs in **Submit Errors** via `dioform-fullstack` rejection mapping
([#41](https://github.com/sagikazarmark/dioform/issues/41)) and manual **Validation Source**
injection ([#139](https://github.com/sagikazarmark/dioform/issues/139)). If the client enforces
nothing for that path, its submit-triggered validators produce no error and there is no blocker to
bypass; the server's verdict arrives as a submit-scoped error afterward. The issue itself notes this flow
is already expressible with ordinary application events plus fullstack rejection mapping.

## When to revisit

Reopen if a concrete flow emerges that intent-scoped submit validation plus server-side **Submit Errors**
genuinely cannot express: for example, the *same* intent must sometimes block and sometimes allow an
*identical* value based on runtime state a validator cannot observe through **Validator Context**. That
has not been demonstrated; the context already exposes the form snapshot, trigger, intent, and field
metadata, which cover the known cases. Parse blockers and in-flight submission must remain
non-bypassable regardless, so even the blanket TanStack flag would not translate directly.
