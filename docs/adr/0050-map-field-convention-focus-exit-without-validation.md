# Map Field Convention Focus Exit without validation

Dioform's `dioxus-field` adapter maps **Commit** to the Commit **Validation Trigger** and
maps **Focus Exit** to exact **Blurred Field** and touched metadata plus blur listeners through the
non-validating blur path. The two reports remain independent: a switch can Commit while focused,
and leaving a widget can report Focus Exit after an earlier Commit. This gives **Widget Registries**
an exact way to update focus-derived form state without inferring focus movement from Commit.

Running the full blur path for Focus Exit would validate changed native controls twice, because a
registry reports its write and Commit before Focus Exit. Moving validation from Commit to Focus Exit
would instead stop validating interactions such as switch toggles and slider releases that Commit
while focus stays inside the widget. Keeping validation on Commit and state/listeners on Focus Exit
preserves both facts and their ordering.

The Dioform 0.4 API completes the breaking validation rename: `ValidationTrigger::Commit`,
Commit-named `ValidationMode` APIs, and binding `on_commit()` entry points replace the legacy
Blur-named validation surfaces without compatibility aliases. Blur listener APIs retain their names
because they report **Focus Exit**, and native `onblur()` helpers compose Commit and then Focus Exit.
This rename does not change validator reach. [ADR-0051](0051-reveal-field-errors-after-commit-without-marking-fields-blurred.md)
adds exact committed metadata and commit-aware default visibility without changing this event
mapping. A **Blurred Field** still means the user left that Field's logical focus scope. See
[dioxus-field#8](https://github.com/sagikazarmark/dioxus-field/issues/8) and
[dioxus-daisyui-registry#105](https://github.com/sagikazarmark/dioxus-daisyui-registry.orig/issues/105).

## 0.4 migration

- Replace `ValidationTrigger::Blur` with `ValidationTrigger::Commit`.
- Replace `ValidationMode::on_blur()` and `on_blur_or_submit()` with `on_commit()` and
  `on_commit_or_submit()`.
- Replace `validate_on_blur`, `with_blur_validation`, `validates_on_blur`, and
  `should_validate_on_blur` with their Commit-named counterparts.
- Replace semantic binding `on_blur()` calls that meant “finish this interaction and validate” with
  `on_commit()`. Report actual focus departure separately with `on_focus_exit()`.
- Replace `FormCore::mark_field_blurred` or `FormHandle::mark_field_blurred` calls that relied on
  validation with `commit_field`. Keep or separately call `mark_field_blurred` only when the same
  integration also needs exact touched and **Blurred Field** metadata plus blur listeners.
- Replace `ErrorVisibilityPolicy::CommitOrBlurOrSubmit` with `CommitOrSubmit`. Choose
  `BlurOrSubmit` explicitly when presentation should follow Focus Exit instead of Commit.
- Keep `is_field_blurred`, blur listeners, and blur listener context types unchanged; they still
  describe Focus Exit rather than validation.

There are no aliases for the 0.3 validation names. Form State Snapshot v5 payloads are rejected;
recreate state under v6 rather than interpreting old interaction metadata under Commit semantics.
