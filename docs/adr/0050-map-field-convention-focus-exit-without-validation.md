# Map Field Convention Focus Exit without validation

Dioform's `dioxus-field` adapter maps **Commit** to the existing Blur **Validation Trigger** and
maps **Focus Exit** to exact **Blurred Field** and touched metadata plus blur listeners through the
non-validating blur path. The two reports remain independent: a switch can Commit while focused,
and leaving a widget can report Focus Exit after an earlier Commit. This gives **Widget Registries**
an exact way to update focus-derived form state without inferring focus movement from Commit.

Running the full blur path for Focus Exit would validate changed native controls twice, because a
registry reports its write and Commit before Focus Exit. Moving validation from Commit to Focus Exit
would instead stop validating interactions such as switch toggles and slider releases that Commit
while focus stays inside the widget. Keeping validation on Commit and state/listeners on Focus Exit
preserves both facts and their ordering.

The Dioform 0.3 API retains its legacy Blur trigger and listener names. Renaming the validation
concept around Commit is a separate breaking change; this compatibility step does not change
validator reach, default visibility, or serialized metadata meaning. A **Blurred Field** still means
the user left that Field's logical focus scope. See
[dioxus-field#8](https://github.com/sagikazarmark/dioxus-field/issues/8) and
[dioxus-daisyui-registry#105](https://github.com/sagikazarmark/dioxus-daisyui-registry.orig/issues/105).
