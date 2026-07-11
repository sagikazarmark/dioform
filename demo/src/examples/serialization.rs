use std::cell::RefCell;
use std::rc::Rc;

use dioform::advanced::FormStateSnapshot;
use dioform::prelude::*;
use dioxus::prelude::*;

/// A full form-state snapshot captures more than the model: the draft, per-field
/// metadata, submit state, and library-owned collection item identity. Restoring
/// it reinstates all of that exactly; the round trip is opt-in and lossless,
/// which is what makes it safe to serialize and rehydrate. Capture, edit, then
/// restore to see the draft and identities come back.
#[derive(Clone, Debug, PartialEq, Form)]
struct Doc {
    title: String,
    tags: Vec<Tag>,
}

#[derive(Clone, Debug, PartialEq, Form)]
struct Tag {
    label: String,
}

fn sample() -> Doc {
    Doc {
        title: "Draft post".into(),
        tags: vec![
            Tag {
                label: "rust".into(),
            },
            Tag {
                label: "forms".into(),
            },
        ],
    }
}

#[component]
pub fn SerializationExample() -> Element {
    let form = use_form(sample());
    let saved: Rc<RefCell<Option<FormStateSnapshot<Doc, String>>>> =
        use_hook(|| Rc::new(RefCell::new(None)));
    let mut captured_version = use_signal(|| None::<u32>);
    let mut status = use_signal(String::new);

    let title = form.text(Doc::fields().title());
    let title_oninput = title.clone();
    let tags = form.collection(Doc::fields().tags());
    let items = tags.items();
    let add = tags.clone();

    let capture_form = form.clone();
    let saved_capture = Rc::clone(&saved);
    let restore_form = form.clone();
    let saved_restore = Rc::clone(&saved);

    rsx! {
        label { class: "block",
            span { class: "mb-1 block text-sm font-medium", "Title" }
            input {
                class: "input input-bordered w-full",
                name: title.name(),
                value: title.value(),
                oninput: move |e| title_oninput.on_input(e.value()),
            }
        }
        ul { class: "mt-3 space-y-2",
            for item in items.iter().cloned() {
                {
                    let index = item.index();
                    let label = item.text(Tag::fields().label());
                    let label_oninput = label.clone();
                    let remove = tags.clone();
                    let id = item.identity();
                    rsx! {
                        li { key: "{index}", class: "flex items-center gap-2",
                            input {
                                class: "input input-bordered input-sm flex-1",
                                name: label.name(),
                                value: label.value(),
                                oninput: move |e| label_oninput.on_input(e.value()),
                            }
                            button {
                                class: "btn btn-xs btn-outline btn-error",
                                onclick: move |_| { remove.remove(id); },
                                "remove"
                            }
                        }
                    }
                }
            }
        }
        button {
            class: "btn btn-sm btn-outline mt-2",
            onclick: move |_| { add.append(Tag { label: String::new() }); },
            "add tag"
        }

        div { class: "mt-4 flex flex-wrap gap-2 border-t border-base-300 pt-4",
            button {
                class: "btn btn-sm btn-primary",
                onclick: move |_| {
                    let snapshot = capture_form.state_snapshot();
                    captured_version.set(Some(snapshot.version()));
                    saved_capture.borrow_mut().replace(snapshot);
                    status.set("Captured. Now edit fields or add/remove tags, then restore.".into());
                },
                "capture state"
            }
            button {
                class: "btn btn-sm btn-outline",
                disabled: captured_version().is_none(),
                onclick: move |_| {
                    if let Some(snapshot) = saved_restore.borrow().clone() {
                        match restore_form.restore_state_snapshot(snapshot) {
                            Ok(()) => status.set("Restored draft, metadata, and item identities.".into()),
                            Err(error) => status.set(format!("Restore failed: {error:?}")),
                        }
                    }
                },
                "restore state"
            }
        }
        if let Some(version) = captured_version() {
            p { class: "mt-2 text-xs text-base-content/55", "Snapshot format v{version} captured." }
        }
        if !status.read().is_empty() {
            p { class: "mt-1 text-sm text-base-content/75", "{status}" }
        }
    }
}
