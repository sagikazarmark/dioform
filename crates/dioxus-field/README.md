# dioxus-field

`dioxus-field` is a form-library-agnostic field convention for Dioxus. It lets widget libraries
accept reactive values, change callbacks, and interaction commits without depending on a form
library or prescribing rendered controls.

`FieldMeta` and the headless field parts are independently usable with a bare Dioxus signal:

```rust,no_run
use std::rc::Rc;

use dioxus::prelude::*;
use dioxus_field::{
    Binding, Field, FieldContext, FieldDescription, FieldError, FieldMetaValues, Label,
    use_field_meta_state,
};

fn ProfileName() -> Element {
    let mut name = use_signal(String::new);
    let binding: Binding<String> = name.into();
    let meta = use_field_meta_state(FieldMetaValues {
        id: Some(Rc::from("profile-name")),
        name: Some(Rc::from("name")),
        required: true,
        ..FieldMetaValues::default()
    });

    rsx! {
        Field {
            context: FieldContext::new(binding).with_meta(meta),
            Label { "Name" }
            input {
                value: name,
                oninput: move |event| name.set(event.value()),
                ..meta.attributes(),
            }
            FieldDescription { id: "profile-name-help", "Shown on your profile" }
            FieldError { id: "profile-name-error" }
        }
    }
}
```

On Dioxus 0.7.10, forward listeners through an explicit `attributes: vec![...]` collection or an
explicit `Option<EventHandler<_>>` prop. Bare listener props passed through `extends` are not yet a
safe forwarding mechanism, and duplicate listeners on one element silently keep the first.

## Conformance testing

Widget registries can use the public `dioxus_field::testing` module from ordinary integration tests;
no browser renderer or form library is required. Create the probe outside the `VirtualDom`, obtain
its Dioxus callbacks while rendering the test component, drive the registry component through its
normal interaction path, then call the assertion after rendering.

Keep these five named tests in every registry:

| Test | Kit API | Registry adapter responsibility |
| --- | --- | --- |
| `commit_is_synchronously_observable_before_submit_handling_runs` | `CommitOrderProbe` | Wire `on_commit()` to the widget commit path and `on_submit()` to the containing submit handler. |
| `writes_carry_their_change_origin` | `ChangeOriginProbe` | Give the produced binding to the widget and drive user and programmatic writes. |
| `binding_resolution_precedence_holds_for_values_and_meta_flags` | `assert_binding_resolution_precedence`, `assert_meta_resolution_precedence`, `assert_meta_flag_precedence` | Exercise explicit, Field Context, and internal sources; report flags from actual rendered state or attributes. |
| `focus_request_round_trips_to_the_widget_control` | `FocusRoundTripProbe` | Register `on_focus()` through the widget's normal focus registration and request focus through Field Context. |
| `error_and_description_ids_appear_on_mount_and_vanish_on_drop` | `assert_field_part_ids` | Mount and drop the registry's description and error parts around the same `FieldMeta`. |

The test adapter is intentionally registry-owned. It may dispatch DOM events or expose the same
handlers the rendered control uses, but it should not reproduce binding or metadata resolution in
test-only code. This keeps the assertions shared while allowing checkbox, select, slider, and other
widgets to retain their native interaction semantics.

The crate is currently incubating and its API is not yet stable.
