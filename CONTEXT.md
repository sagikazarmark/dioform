# Dioform

Dioform is a headless form library for Rust applications that need typed form state, validation, hydration support, and Dioxus integration without prescribing visual form components.

The MVP scope boundaries and post-MVP extension points are documented in `docs/archive/mvp-boundaries.md`.

## Language

**Headless Form Library**:
A library that manages form state and behavior while leaving all rendering, styling, and input components to the application.
_Avoid_: Form component library, UI kit

**Form Core**:
The renderer-agnostic part of the library that defines form state, field state, validation, submission, and metadata concepts without depending on Dioxus.
_Avoid_: Dioxus internals, form component layer

**Dioxus Adapter**:
The first-party integration layer that connects the **Form Core** to Dioxus reactivity, hooks, context, events, and hydration behavior.
_Avoid_: Core, component library

**Form Model**:
A Rust type known at compile time that represents the values a form edits.
_Avoid_: Runtime schema, JSON shape

**Dynamic Form**:
A form whose fields and value shape are discovered at runtime rather than represented by a compile-time **Form Model**.
_Avoid_: Form model

**Form Draft**:
The editable current value owned by a form while the user is interacting with it, compared against initial values for reset and dirty state.
_Avoid_: Application state, submitted value

**Application-Owned Draft**:
An alternative form integration style where the application owns the editable value and the form library tracks behavior around it.
_Avoid_: Form draft

**Field Path**:
A typed reference to a value inside a **Form Model**, carrying both the root model type and the field value type.
_Avoid_: String field name, selector string

**Field Group**:
A reusable grouping of addressable **Fields** expressed through typed **Field Paths** so shared field behavior or rendering can be applied across forms without changing **Form Draft** ownership.
_Avoid_: UI component, dynamic schema, form model

**Field Group Map**:
A typed mapping from each logical **Field** in a **Field Group** to a concrete **Field Path** in one **Form Model**.
_Avoid_: String path map, provider context

**Field Group Mount**:
The placement of a **Field Group** in a host **Form Model** by composing a typed parent **Field Path** or by supplying an explicit **Field Group Map**.
_Avoid_: Component nesting, dynamic prefix

**Field Identity**:
Structured internal metadata that identifies a **Field** for state, errors, selectors, and observers independently from its rendered **Field Name**.
_Avoid_: HTML name, function pointer identity

**Field**:
Any addressable value inside a **Form Model**, including leaf values, nested objects, and collections.
_Avoid_: Input, control, widget

**Optional Field**:
A **Field** whose value may be absent and whose nested values are not implicitly created by field traversal.
_Avoid_: Auto-created nested field

**Collection Field**:
A **Field** whose value contains ordered repeated items and whose item metadata follows items across insertions, removals, and reordering.
_Avoid_: Array input, repeated component

**Collection Item Identity**:
Structured internal metadata that identifies one logical item inside a **Collection Field** so item metadata follows the item independently from its rendered index.
_Avoid_: Array index, row ID, application key

**Input Parsing**:
The conversion of user input from a rendered control into the typed value expected by a **Field**.
_Avoid_: Validation

**Field Validation**:
Validation that evaluates one **Field** value in isolation.
_Avoid_: Form validation

**Form Validation**:
Validation that evaluates the **Form Draft** as a whole and may produce form-level errors or errors attached to specific **Field Paths**.
_Avoid_: Field validation

**Validation Error**:
A typed value describing why **Field Validation** or **Form Validation** rejected a typed value; validators within one form share the same validation error type.
_Avoid_: Display message, parser error

**Validation Adapter**:
An integration that maps an external validation library's result into the form's **Validation Error** model.
_Avoid_: Core validation engine

**Validation Adapter Crate**:
An optional first-party package that provides a **Validation Adapter** for one external validation library without making that library part of the **Form Core** or **Facade Crate**.
_Avoid_: Facade feature, core dependency

**External Validation Diagnostic**:
A validation finding produced by an external validation library before a **Validation Adapter** maps it into the form's shared **Validation Error** type.
_Avoid_: Validation error, parser error

**External Diagnostic Path**:
A path emitted by an external validation library as part of an **External Validation Diagnostic**, separate from a typed **Field Path** or rendered **Field Name** until a **Validation Adapter** explicitly maps it.
_Avoid_: Field path, field name

**Validation Trigger**:
A semantic form event that determines when validation runs, such as a value change, field blur, submit request, or form initialization.
_Avoid_: DOM event, Dioxus event

**Validation Mode**:
A form-level policy that determines which **Validation Triggers** run automatically during interaction, such as validating on blur, on change, or after a submit attempt.
_Avoid_: Validator trigger set, error visibility

**Error Visibility**:
The presentation decision that determines when stored **Validation Errors** should be shown to a user, including which **Submit Intent** made submit-scoped errors relevant in intentful forms.
_Avoid_: Validation trigger, validation state

**Validation Source**:
The origin category of a stored **Validation Error**, such as a specific validation trigger, submit attempt, server response, field validator, or form validator. A **Validator Source** is the registered-validator form of a **Validation Source**.
_Avoid_: Flattened error list

**Validation Lifecycle**:
The state-machine behavior that moves validator sources through trigger eligibility, **Validation Chain** execution, **Validation Status** transitions, result storage, and submit relevance.
_Avoid_: Validator helper, validation plumbing

**Stale Submit Error**:
A submit-sourced **Validation Error** that refers to a field value before the field changed.
_Avoid_: Current validation error

**Async Validation**:
Validation whose result may arrive after newer form interaction has occurred and must not overwrite newer validation state.
_Avoid_: Synchronous validation

**Stale Validation Result**:
An **Async Validation** result from an older validation run that is no longer current for the field or form state it checked.
_Avoid_: Current validation result

**Validation Runtime**:
The execution boundary that supplies task spawning and timers for **Async Validation** without making the **Form Core** depend on a renderer or async runtime. The two halves vary independently: timers are application-injected as delay futures through a delay-factory closure, while task spawning is abstracted behind a **Task Spawner** seam so **Async Validation** can run under Dioxus in production and inline in tests. See [ADR-0009](adr/0009-add-task-spawner-seam-for-async-validation.md).
_Avoid_: Form core, Dioxus adapter

**Task Spawner**:
The narrow seam within the **Validation Runtime** that spawns and cancels **Async Validation** tasks, satisfied by a Dioxus adapter in production and an inline adapter in tests. It does not cover timers, which are injected separately as delay futures.
_Avoid_: Async runtime, timer, Dioxus task

**Debounced Validation**:
Validation delayed until interaction settles, while still being flushed when needed for submission correctness.
_Avoid_: Delayed error display

**Submitted Value**:
The validated **Form Model** value handed to application submission behavior.
_Avoid_: Transformed output, command object

**Submit Error**:
A structured form-level or field-level **Validation Error** returned by submission behavior, associated with the **Submission Snapshot** that produced it, and stored with a submit-related **Validation Source**.
_Avoid_: Exception, transport failure

**Server Submit Rejection**:
An application-defined server response that rejects a **Submitted Value** with field-level or form-level reasons and is explicitly mapped into structured **Submit Errors**.
_Avoid_: Transport failure, exception

**Transport Submit Failure**:
A failure to invoke, send, receive, serialize, or deserialize submission transport before a **Server Submit Rejection** can be interpreted; it is not a **Submit Error** unless the application explicitly maps it into one.
_Avoid_: Submit error, validation error

**Submission**:
The lifecycle that validates a **Form Draft**, produces a **Submitted Value**, runs application submit behavior, and records submit-related state and errors.
_Avoid_: Validation, input parsing

**Submit Intent**:
An application-defined typed value captured for one **Submission** that distinguishes why the submit was started, such as saving a draft or publishing. Submit-triggered validation and application submit behavior may use it when rules differ by submit purpose, and intentful submit triggers name their purpose explicitly.
_Avoid_: Submit metadata, submit action, arbitrary event metadata

**Submission Snapshot**:
An owned submit-time capture for one **Submission** that carries the **Submitted Value** and **Submit Intent** used by application submit behavior and stale-submit-error protection.
_Avoid_: Submitted value, command object

**Last Submit Status**:
The latest meaningful outcome of a submit attempt, associated with the attempted **Submit Intent** when the form is intentful. Intentful forms may read the latest status globally or for one **Submit Intent**.
_Avoid_: Global success flag

**Reset**:
An explicit return of a form to its initial values and fresh interaction, validation, and submission state.
_Avoid_: Re-render, refresh

**Reinitialization**:
An explicit replacement of a form's initial values and **Form Draft** with a new value from outside the form.
_Avoid_: Automatic prop sync

**Baseline Value**:
The value a **Form Draft** is compared against for dirty tracking and restored to by **Reset**.
_Avoid_: Original prop value

**Dirty Field**:
A **Field** whose current value differs from the corresponding **Baseline Value**.
_Avoid_: Touched field, edited field

**Touched Field**:
A **Field** the user has interacted with, whether or not its value differs from the **Baseline Value**.
_Avoid_: Dirty field, blurred field

**Blurred Field**:
A **Field** that has lost focus at least once during form interaction.
_Avoid_: Touched field

**Programmatic Update**:
A form value change initiated by application code rather than direct user interaction with the field.
_Avoid_: User input

**Field Registration**:
The creation of stored metadata for a **Field** when it is first used or receives state such as validation errors.
_Avoid_: Required declaration

**Form Selector**:
A focused read of form or field state intended to subscribe only to the state a UI actually needs.
_Avoid_: Whole-form read

**Central Validator**:
A validator registered as durable form behavior rather than tied to a rendered field component's lifecycle.
_Avoid_: Field-local validator

**Field-Local Validator**:
A validator registered by field UI and removed when that field UI unmounts.
_Avoid_: Central validator

**Validator Source**:
The identity of one registered validator used to replace, retain, or clear only the errors and pending results produced by that validator. It is a specialized **Validation Source** for field and form validators.
_Avoid_: Validation trigger

**Validation Chain**:
The ordered set of validators for a field or form and trigger, where synchronous validators run before asynchronous validators by default.
_Avoid_: Single validator

**Validator Context**:
Read-only information supplied to a validator, such as the current form snapshot, field path, validation trigger, **Submit Intent** for submit-triggered validation, and metadata relevant to the validation run.
_Avoid_: Form mutation API

**Form Snapshot**:
An owned view of form values captured at a point in time for async validation, a **Submission Snapshot**, or application reads.
_Avoid_: Live draft reference

**In-Flight Submission**:
A **Submission** whose submit lifecycle has been accepted and has not yet completed, including submit-relevant async validation waiting and application submit behavior. Its **Submit Intent** identifies which intent is currently in flight for application-facing state, and application submit behavior uses a **Submission Snapshot** rather than the live **Form Draft**.
_Avoid_: Live draft submission

**File Field**:
A form-scoped file-selection control addressed by a **File Field Key**, treated as field-like for metadata, errors, accessibility, and submit availability but not as an ordinary **Field** because selected files are outside the **Form Model** and **Form Draft**. A **File Field** has a cardinality policy: single-file fields retain at most one file, and multi-file fields retain an ordered list.
_Avoid_: Ordinary field, field path, model file value

**File Field Key**:
A typed form-scoped key that identifies one **File Selection** for a **Form Model** without providing access to a value inside the model. The key carries the **File Field** cardinality policy.
_Avoid_: Field path, DOM id, file handle

**File Selection**:
The adapter-owned ordered files for a **File Field**, constrained by the field's cardinality policy, captured in a **File Submission Snapshot** for file-aware **Dioxus-Managed Submission**, validated by sync or async file-selection validators, and cleared by **Reset** or **Reinitialization**. Pending async file-selection validation participates in submit **PendingValidation**.
_Avoid_: Form draft value, raw input state, browser File object

**Selected File**:
One user-selected file represented in public form APIs by cloneable metadata such as name, byte size, and media type, optionally paired with a platform file handle owned outside **Form Core**.
_Avoid_: Serialized file, cloned file contents

**File Submission Snapshot**:
A submit-time capture of **File Selections** handed to file-aware submit behavior alongside the **Submission Snapshot**.
_Avoid_: Form snapshot, serialized file upload

**Hydration Support**:
Deterministic form initialization across server-rendered and client-rendered Dioxus output without serializing the full form draft and metadata.
_Avoid_: Draft persistence, form-state serialization

**Form State Serialization**:
An explicit, opt-in, value-bearing transfer of selected form state, such as a **Form Draft**, field metadata, non-submit **Validation Errors**, validation result state, and collection identities, across a boundary.
_Avoid_: Hydration support, live form instance serialization, automatic draft persistence

**Field Name**:
A stable string representation of a **Field Path** used for HTML interoperability, not as the primary typed addressing mechanism.
_Avoid_: Field path, string selector

**Field Name Override**:
A form-specific rename of a **Field Name** segment while preserving the typed Rust **Field Path**.
_Avoid_: Serde rename

**Field Name Policy**:
A form-specific rule for deriving rendered **Field Name** segments from Rust field identifiers while preserving **Field Identity**.
_Avoid_: Serde rename policy, field identity policy

**Field Binding**:
Headless **Dioxus Adapter** behavior that connects application-rendered control interaction to a **Field**, exposing rendered identity, accessibility helpers, value updates, and binding-owned parsing state when needed without owning visual markup.
_Avoid_: Input component, form component, binding module

**Field Binding Lifecycle Listener**:
A **Form Listener** scoped to hook-owned **Field Binding** mount and unmount events for one **Field**, independent of whether the listener hook runs before or after the binding hook in the same component.
_Avoid_: Validator, input component lifecycle owner

**Variant Field**:
A **Field** whose value is an enum variant, treated as a whole value until variant-inner paths receive dedicated conditional-field semantics.
_Avoid_: Nested struct field

**Named Form Struct**:
A struct with named fields that can produce derived **Field Paths**.
_Avoid_: Tuple struct, tuple field form

**Custom Field Value**:
A non-primitive Rust value used directly as a **Field** value, with input parsing or rendering supplied explicitly when needed.
_Avoid_: String-only field

**Raw Input State**:
Rendered input text or control state that cannot currently be parsed into the typed value of a **Field**.
_Avoid_: Form draft value, validation error

**Parse Error**:
A binding-level failure to convert **Raw Input State** into a typed **Field** value, separate from validation errors but able to block submission while unresolved.
_Avoid_: Validation error

**Parse Blocker**:
A mounted input binding's unresolved **Parse Error** that prevents adapter-mediated submission until the binding parses successfully or unmounts.
_Avoid_: Validation error, central validator

**Accessibility Helper**:
An optional headless helper that derives identifiers or ARIA attributes from form state without prescribing markup or styling.
_Avoid_: Form component

**Form ID Namespace**:
A per-form identifier prefix used to derive stable, collision-resistant element IDs for fields, help text, and errors.
_Avoid_: Field name

**Form Observer**:
An optional diagnostic stream of form state transitions used for logging, tests, or future devtools without exposing field values and without including raw **Submit Intent** values by default.
_Avoid_: Devtools UI, audit log

**Form Listener**:
Application-owned side-effect behavior registered for semantic form events, distinct from **Field Validation**, **Form Validation**, and **Form Observer** diagnostics.
_Avoid_: Validator, observer, input component callback

**Field Listener**:
A **Form Listener** scoped to one **Field** through a typed **Field Path**.
_Avoid_: Field-local validator, Dioxus event handler

**Form Listener Event**:
A semantic form event delivered to a **Form Listener** with contextual metadata such as **Field Identity**, rendered **Field Name**, event kind, and update origin while avoiding field-value exposure by default.
_Avoid_: DOM event, validation trigger, observer event

**Debounced Listener**:
A **Form Listener** scheduled with an application-supplied delay so stale scheduled callbacks are ignored when newer matching listener events arrive; it does not affect validation status, **Submit Availability**, or submission correctness.
_Avoid_: Debounced Validation, submit blocker

**Facade Crate**:
The user-facing Dioxus crate that re-exports common core types and the derive macro while preserving internal crate boundaries.
_Avoid_: Core crate

**Form Handle**:
A Dioxus-facing, cheap-to-pass reference to form state and behavior returned by form hooks.
_Avoid_: Form core state

**Form Context Scope**:
A typed Dioxus context identity that selects one provided **Form Handle** within a subtree without relying on global state, rendered names, or **Form ID Namespaces**.
_Avoid_: Global form key, field name, form ID namespace

**Form Context Consumer**:
A renderless **Dioxus Adapter** access point that retrieves a scoped **Form Handle** from Dioxus context.
_Avoid_: Form component, styled input, global form access

**Renderless Form Access**:
Headless access to form, field, or binding behavior without owning markup, styling, layout, or UX copy.
_Avoid_: Component library, styled component

**Static Field Path**:
A **Field Path** for compile-time struct fields, represented without captured runtime state.
_Avoid_: Dynamic field path

**Field Replacement**:
An explicit replacement of a **Field** value through the form API so the form can update metadata, validation state, and observers consistently.
_Avoid_: Untracked mutable access

**Successful Submission**:
A **Submission** that completes without returned submit errors and leaves reset or baseline changes to the application.
_Avoid_: Automatic reset, automatic save marker

**Dioxus-Managed Submission**:
A **Submission** handled through the Dioxus adapter rather than native browser form posting.
_Avoid_: Native form POST

**Native Browser Submission**:
A browser-owned form submission where rendered controls are serialized and posted by the browser rather than by Dioform's typed submission lifecycle.
_Avoid_: Dioxus-managed submission, typed submission handler

**Progressive Submission**:
A browser-owned form submission that may be blocked by hydrated client-side form preflight, while still falling through to native browser posting when preflight allows it.
_Avoid_: Dioxus-managed submission, async managed submit

**Browser Submit Preflight**:
A hydrated client-side check that may block **Progressive Submission** because current form state has known blockers such as **Parse Errors**, submit-scoped **Validation Errors**, pending submit-relevant validation, or an in-flight submission.
_Avoid_: Final submit authorization, native server validation

**Dioxus Fullstack Submit Adapter**:
A first-party integration that connects Dioxus Fullstack server functions to **Dioxus-Managed Submission** without making the **Form Core** depend on Fullstack transport.
_Avoid_: Form core submit behavior, native form POST

**Form Cleanup**:
The adapter lifecycle step that prevents pending async validation or submission results from mutating a form after its UI instance is gone.
_Avoid_: Submit cancellation guarantee, async validation cancellation guarantee

**Form Configuration**:
The durable setup used to create a form, including initial values, central validators, validation policies, and submission behavior.
_Avoid_: Live form state

**Form Initialization**:
The lifecycle point when a form is created from its **Form Configuration**, distinct from Dioxus component mounting.
_Avoid_: Component mount

**Validation Status**:
The source-level state of validation, such as unknown, valid, invalid, pending, skipped, or stale, with field and form status derived from validator sources.
_Avoid_: Boolean validity only

**Submit Availability**:
A UI-oriented indication that submission for a given **Submit Intent** has no current known blockers such as validation errors, parse blockers, required pending validation, or an in-flight submission.
_Avoid_: Final submit authorization

**Conditional Field**:
A **Field** whose UI may be hidden or unmounted while its value remains part of the **Form Draft**.
_Avoid_: Removed field

## Example Dialogue

Developer: Should Dioform provide a styled text input?

Domain expert: No. Dioform is a **Headless Form Library**. It should expose form and field behavior through the **Dioxus Adapter**, but applications own their visual components.

Developer: Should surveys loaded from JSON drive the initial API design?

Domain expert: No. The primary path is a compile-time **Form Model**. A **Dynamic Form** may be supported later through a separate lower-level API.

Developer: Who owns the editable value during normal form interaction?

Domain expert: The form owns the **Form Draft**. An **Application-Owned Draft** may be exposed later as a lower-level integration style, but it is not the primary API.

Developer: Should normal field access use strings like `"email"`?

Domain expert: No. Normal field access uses a **Field Path** so the compiler can check field existence and field value type.

Developer: Is a rendered field name the same as the field's internal identity?

Domain expert: No. **Field Identity** is internal structured metadata; **Field Name** is derived from it for HTML interoperability.

Developer: Is a field only something rendered as an input?

Domain expert: No. A **Field** can be any addressable value inside the **Form Model**. Leaf fields commonly bind to inputs, while nested objects and collections can still carry validation, errors, and metadata.

Developer: If an optional nested value is absent, should editing a nested field create it automatically?

Domain expert: No. An **Optional Field** is explicit about absence. The form should not invent missing nested values while traversing fields.

Developer: If a user edits the second address and then moves it to the first position, where should its metadata go?

Domain expert: A **Collection Field** preserves item metadata with the moved item rather than leaving it attached to the old index.

Developer: Does the application need to add an ID field to every repeated draft item so metadata can follow it?

Domain expert: No. **Collection Item Identity** is library-owned internal metadata, so even draft-only or duplicate-valued items can keep distinct metadata.

Developer: Is a failed number conversion a validation error?

Domain expert: No. **Input Parsing** determines whether rendered input can become a typed field value. Validation determines whether a typed value is acceptable.

Developer: Where does a password confirmation rule belong?

Domain expert: It belongs in **Form Validation** because it compares multiple fields in the **Form Draft**.

Developer: Can one validator return strings while another returns a custom enum in the same form?

Domain expert: No. A form has one **Validation Error** type. Simple forms can use strings, while richer forms can use an enum or struct.

Developer: Is `garde` part of the form core?

Domain expert: No. External validation libraries integrate through a **Validation Adapter** that maps their results into the form's **Validation Error** type.

Developer: Does the form core validate on Dioxus `oninput` directly?

Domain expert: No. The core validates on a **Validation Trigger**. The **Dioxus Adapter** maps Dioxus events such as `oninput`, `onchange`, `onblur`, and `onsubmit` onto those triggers.

Developer: If a required field is invalid on mount, must the page immediately show its error?

Domain expert: No. **Error Visibility** is separate from whether validation has run and whether a field is currently valid.

Developer: Should a Save Draft attempt make Publish-only validation errors visible?

Domain expert: No. Intentful **Error Visibility** follows the **Submit Intent** whose submit attempt made those errors relevant.

Developer: Can one aggregate error view explain every submit button in an intentful form?

Domain expert: No. Intentful UIs need error views filtered to the relevant **Submit Intent**.

Developer: Should an intentful UI ask one global question, "can this form submit?"

Domain expert: No. Intentful **Submit Availability** is read for a specific **Submit Intent**.

Developer: Should all field errors be stored as one flat list?

Domain expert: No. Errors retain their **Validation Source** so rerunning or clearing one source does not accidentally erase unrelated errors.

Developer: What happens to a server error after the user changes the rejected field value?

Domain expert: It becomes a **Stale Submit Error** and should be cleared for that field by default.

Developer: If an older username availability check returns after a newer one, can it update the field?

Domain expert: No. It is a **Stale Validation Result** and must not overwrite newer **Async Validation** state.

Developer: Does the core call Dioxus `spawn` to run async validators?

Domain expert: No. **Async Validation** runs through a **Validation Runtime** boundary so the **Form Core** remains renderer-agnostic.

Developer: If a user submits before a debounce delay finishes, should the form wait out the delay?

Domain expert: No. **Debounced Validation** that affects submit correctness should flush immediately on submit.

Developer: Does validation transform the form into a different submit type?

Domain expert: No. The **Submitted Value** is the validated **Form Model**. Applications can transform it after submission begins.

Developer: How should server-side field rejection be represented?

Domain expert: As a **Submit Error** attached to the relevant **Field Path** or to the form as a whole.

Developer: Is a failed server function request itself a Submit Error?

Domain expert: No. It is a **Transport Submit Failure** unless the application explicitly maps it into a form-level **Submit Error**.

Developer: Should a Dioxus Fullstack server rejection serialize Dioform's SubmitError type directly?

Domain expert: No. The server returns an application-defined **Server Submit Rejection**, and the **Dioxus Fullstack Submit Adapter** maps that rejection into structured **Submit Errors**.

Developer: Does a Publish server rejection block Save Draft?

Domain expert: No. A **Submit Error** belongs to the **Submission Snapshot** and **Submit Intent** that produced it.

Developer: If the latest status is submitted, should the application infer which button caused it?

Domain expert: No. **Last Submit Status** carries the **Submit Intent** that produced the outcome in an intentful form.

Developer: If a Save Draft attempt is blocked because Publish is already in flight, which intent does the latest status carry?

Domain expert: **Last Submit Status** carries the attempted **Submit Intent**, so the blocked attempt is recorded as Save Draft.

Developer: Can the UI ask what last happened for Publish after a later Save Draft attempt?

Domain expert: Yes. Intentful forms can read **Last Submit Status** for one **Submit Intent** as well as the global latest attempt.

Developer: How should one form distinguish Save Draft from Publish when both submit the same draft?

Domain expert: The chosen button provides a **Submit Intent** captured with that **Submission**, not an extra **Field** in the **Form Draft** or unrelated application event state.

Developer: Can publish-only validation rules inspect the button choice?

Domain expert: Yes. Submit-triggered validation may use the **Submit Intent** when validation rules differ by submit purpose.

Developer: Can a Publish validation result satisfy a later Save Draft submission if the draft did not change?

Domain expert: No. Submit-triggered validation is scoped to the **Submit Intent** that produced it.

Developer: Should Publish validation errors globally block Save Draft?

Domain expert: No. Submit-triggered validation errors remain associated with the **Submit Intent** that produced them.

Developer: Does pending Publish validation block a later Save Draft attempt?

Domain expert: No, unless an **In-Flight Submission** is already blocking duplicate attempts. Pending submit validation is scoped to the **Submit Intent** that produced it.

Developer: Should an intentful form silently use a default intent when a submit trigger omits one?

Domain expert: No. Each intentful submit trigger should state its **Submit Intent** explicitly.

Developer: Should the library infer intent from a submit button's rendered name or value?

Domain expert: No. Intentful **Dioxus-Managed Submission** uses a typed **Submit Intent** rather than parsing rendered form data.

Developer: Should a Submit Intent carry analytics context or mouse coordinates from the click?

Domain expert: No. **Submit Intent** models the submission purpose for the current **Form Draft**; unrelated UI event data belongs in ordinary application event state.

Developer: Should a Submit Intent carry field values or other rich payload data?

Domain expert: No. The **Submission Snapshot** carries the **Submitted Value**; **Submit Intent** should stay focused on the submit purpose.

Developer: Can the same form submit twice at the same time by default?

Domain expert: No. A **Submission** should block concurrent submissions by default.

Developer: What should reset do?

Domain expert: **Reset** restores the initial values and clears interaction, validation, and submission state.

Developer: If parent data refreshes, should the form automatically overwrite the draft?

Domain expert: No. **Reinitialization** is explicit so outside data changes do not accidentally destroy user edits.

Developer: If a user changes a value and then changes it back, is it still dirty?

Domain expert: No. A **Dirty Field** is dirty only while its value differs from the current **Baseline Value**.

Developer: If a user focuses and blurs a field without changing it, what metadata changes?

Domain expert: It becomes a **Touched Field** and a **Blurred Field**, but not a **Dirty Field**.

Developer: Does application code setting a value mean the user touched that field?

Domain expert: No. A **Programmatic Update** can make a field dirty if the value differs from the **Baseline Value**, but it does not mark the field touched by default.

Developer: Must every field be registered before use?

Domain expert: No. **Field Registration** happens implicitly when field metadata is first needed.

Developer: Should a field component subscribe to the whole form?

Domain expert: No. A **Form Selector** should let UI subscribe only to the field or form state it reads.

Developer: Where should business validation rules live?

Domain expert: In a **Central Validator** so they do not depend on whether a field component is currently mounted.

Developer: What happens to a component-scoped validator when its field UI unmounts?

Domain expert: A **Field-Local Validator** is removed with that field UI.

Developer: What happens to errors from a field-local validator when it unmounts?

Domain expert: The **Validator Source** is removed, so errors and pending results from that source are cleared without affecting other sources.

Developer: Can a field have several validators for the same trigger?

Domain expert: Yes. They form a **Validation Chain** and can produce multiple errors.

Developer: Should an async availability check run when sync validation already says the username is too short?

Domain expert: Not by default. In a **Validation Chain**, synchronous validators run first and can prevent asynchronous validators for the same trigger from running.

Developer: Should form-level validation overwrite field-level validation on the same field?

Domain expert: No. Errors from different **Validator Sources** can coexist and presentation policy decides what to show.

Developer: Can validators inspect the rest of the form?

Domain expert: Yes. A **Validator Context** can provide read-only access to the current form snapshot and validation metadata.

Developer: Can a value-change validator read the last clicked submit button?

Domain expert: No. **Submit Intent** is available in **Validator Context** only for submit-triggered validation.

Developer: Can async validation or submission keep borrowing the live draft while it waits?

Domain expert: No. Async validation uses a **Form Snapshot**, and application submission uses a **Submission Snapshot**, so later draft changes do not alter the in-flight operation.

Developer: Can the user edit while submission is still running?

Domain expert: Yes. Once application submit behavior starts, an **In-Flight Submission** uses its own **Submission Snapshot**, while the live **Form Draft** may continue changing.

Developer: Can the UI know which submit button started the in-flight submission?

Domain expert: Yes. Application-facing submit state exposes the **Submit Intent** of the **In-Flight Submission**.

Developer: If the server rejects an old submitted field value after the user changed it, should that field error appear?

Domain expert: No. Field-level submit errors from an **In-Flight Submission** should not attach to a field whose current value no longer matches the **Submission Snapshot**.

Developer: Are file inputs normal form model fields?

Domain expert: No. A **File Field** uses a **File Field Key** and stores its **File Selection** outside the **Form Draft** because files are platform-specific and not ordinary cloneable form values.

Developer: Does hydration support mean serializing all form metadata from server to client?

Domain expert: No. **Hydration Support** means deterministic initialization across server and client, not full form-state serialization.

Developer: Is form-state serialization a dump of the entire live form instance?

Domain expert: No. **Form State Serialization** is explicit, opt-in transfer of selected value-bearing form state. Live behavior such as validators, observers, tasks, timers, and in-flight submissions is recreated from normal form configuration rather than serialized.

Developer: Does the form model need serde to work?

Domain expert: No. Serialization is optional and belongs to persistence, transport, or interop features rather than core form behavior.

Developer: Why expose string names if field paths are typed?

Domain expert: A **Field Name** supports HTML interoperability while **Field Path** remains the primary typed API.

Developer: Should field names be tied to serde rename rules?

Domain expert: No. A **Field Name Override** is form-specific so serialization remains optional and independent.

Developer: Can the first version address fields inside enum variants?

Domain expert: No. A **Variant Field** is treated as a whole value until variant-inner paths have explicit conditional semantics.

Developer: Which Rust data shapes generate field paths in the first version?

Domain expert: A **Named Form Struct** can generate derived **Field Paths**. Tuple structs and tuple fields are treated as whole values unless manually addressed later.

Developer: Can a field value be a domain-specific Rust type?

Domain expert: Yes. A **Custom Field Value** is valid form data; only input parsing and rendering need explicit support.

Developer: What happens when a text input cannot parse into the typed field value?

Domain expert: The typed **Form Draft** keeps the last valid value while **Raw Input State** records the unparsable rendered input.

Developer: Does the form core store raw text from invalid inputs?

Domain expert: No. **Raw Input State** belongs to input bindings, while the core stores typed values.

Developer: Is a failed parse part of the validation error list?

Domain expert: No. A **Parse Error** is separate from **Validation Error**, though unresolved parse errors can block submission.

Developer: How does invalid raw input stop a typed value from being submitted?

Domain expert: A mounted binding registers a **Parse Blocker** so adapter-mediated submission does not submit the last valid typed value while visible input is unparsable.

Developer: Can a Delete intent bypass parse blockers because it does not care about the current field values?

Domain expert: No. If it is modeled as **Dioxus-Managed Submission**, every **Submit Intent** is blocked by mounted **Parse Blockers**; actions that do not need the submission lifecycle should use ordinary application events.

Developer: Does an unmounted input binding keep blocking submission because it once had a parse error?

Domain expert: No. A **Parse Blocker** is lifecycle-bound to its mounted binding.

Developer: Does headless mean no accessibility support?

Domain expert: No. An **Accessibility Helper** can provide IDs and ARIA values without providing visual components.

Developer: How do field IDs avoid collisions when the same form appears twice?

Domain expert: A **Form ID Namespace** prefixes IDs derived from field names.

Developer: Should debugging hooks log passwords and other field values automatically?

Domain expert: No. A **Form Observer** reports transitions without field values by default; value inspection must be explicit.

Developer: Should observer diagnostics log raw Save Draft or Publish intent values automatically?

Domain expert: No. Raw **Submit Intent** values remain application-local and are omitted from **Form Observer** events by default.

Developer: Should autosave and dependent-field resets be implemented as validators?

Domain expert: No. Validators remain validation-only. Application side effects belong in **Form Listeners** or ordinary application event handlers.

Developer: Is a listener the same thing as a diagnostic observer?

Domain expert: No. A **Form Listener** is application-owned side-effect behavior, while a **Form Observer** is value-redacted diagnostics for logging, tests, or future devtools.

Developer: Should a form-level listener receive field values by default?

Domain expert: No. A **Form Listener Event** carries contextual metadata by default; applications can explicitly read values through their **Form Handle** when the side effect needs them.

Developer: Should Dioxus users need to import three crates for normal use?

Domain expert: No. A **Facade Crate** provides the ergonomic Dioxus API while keeping core and macro crates separate internally.

Developer: Is a form passed through Dioxus UI as a mutable borrowed state object?

Domain expert: No. UI code uses a **Form Handle** that is cheap to pass to event handlers and child components.

Developer: Should Dioxus context lookup for forms use only the form model type?

Domain expert: No. Context lookup uses a **Form Context Scope** so two forms with the same **Form Model** can coexist without ambiguity.

Developer: Does context access mean the library renders field markup?

Domain expert: No. A **Form Context Consumer** is **Renderless Form Access**; applications still own markup, styling, layout, and copy.

Developer: How are ordinary struct fields addressed at runtime?

Domain expert: A **Static Field Path** carries typed accessors and identity for compile-time struct fields without captured runtime state.

Developer: Can application code hold a mutable reference into the form draft indefinitely?

Domain expert: No. A **Field Replacement** happens through explicit replacement APIs so the form can preserve its invariants.

Developer: Should setting a field from application code mark it touched and validate automatically?

Domain expert: No. A **Programmatic Update** changes the value and dirty state, while validation and touched state require explicit intent or user-event helpers.

Developer: Does a successful save automatically make the form clean?

Domain expert: No. A **Successful Submission** does not reset or change the **Baseline Value** unless the application explicitly does so.

Developer: Does the first version submit through native browser POST?

Domain expert: No. The first version uses **Dioxus-Managed Submission**, while field names preserve future HTML interoperability.

Developer: Does the form core require Dioxus Fullstack or serde transport?

Domain expert: No. Submission behavior is generic application code; fullstack integration can be added later without changing the core boundary.

Developer: Can an async result update a form after its component unmounts?

Domain expert: No. **Form Cleanup** prevents late async validation or submission results from mutating a destroyed form instance.

Developer: Should the Dioxus adapter support old Dioxus versions from day one?

Domain expert: No. The adapter targets Dioxus `0.7` for the first release while the core remains insulated from Dioxus version churn.

Developer: Where do durable validators belong?

Domain expert: In **Form Configuration** or central registration, so they exist independently from rendered field components.

Developer: Does registering a validator mean it runs immediately?

Domain expert: No. Registration defines future validation behavior; **Form Initialization** validation runs only when explicitly configured for that lifecycle trigger.

Developer: Does creating a form automatically run validation during SSR or hydration?

Domain expert: No. **Form Initialization** does not imply validation unless initialization validation is explicitly configured.

Developer: Is validity just a boolean?

Domain expert: No. **Validation Status** distinguishes unknown, valid, invalid, pending, skipped, and stale validation states at the validator-source level.

Developer: Does `can_submit` mean the form is guaranteed valid forever?

Domain expert: No. **Submit Availability** is a UI convenience based on current known blockers; submission still performs required validation before calling application behavior.

Developer: Does asking for Publish availability run Publish validation?

Domain expert: No. **Submit Availability** is a read-only signal; the submit attempt runs validation for the **Submit Intent**.

Developer: Can Save Draft be unavailable just because Publish has validation errors?

Domain expert: No. In an intentful form, **Submit Availability** is evaluated for the relevant **Submit Intent**.

Developer: Do value-change validation errors become intent-specific too?

Domain expert: No. Non-submit **Validation Errors** remain conservative known blockers for **Submit Availability** across intents.

Developer: If a field is hidden, should its value disappear from the draft?

Domain expert: No. A **Conditional Field** may be hidden or unmounted without mutating the **Form Draft**.
