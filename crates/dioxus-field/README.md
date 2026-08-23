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

The crate is currently incubating and its API is not yet stable.
